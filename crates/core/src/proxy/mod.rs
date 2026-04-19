/// UDP proxy core — bidirectional relay with session mapping.
///
/// This module is Tauri-independent. Stats are reported via a `tokio::sync::watch` channel.
/// The caller (CLI or Tauri) consumes stats from the channel.
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

use crate::discovery;
use crate::profile::Profile;

/// Tracks a single proxy session (local client ↔ remote server).
#[derive(Debug)]
pub struct Session {
    pub _remote_addr: SocketAddr,
    pub last_active: Instant,
}

/// Traffic statistics snapshot for the running proxy.
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct TrafficStats {
    pub pps_in: u64,
    pub pps_out: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub active_sessions: u32,
    pub clients: Vec<String>,
}

/// Shared proxy state.
pub struct ProxyState {
    pub stats: ProxyCounters,
    /// Maps local client addr → session
    pub sessions: DashMap<SocketAddr, Session>,
    /// Maps remote server addr → local client addr (for routing responses back)
    pub reverse_map: DashMap<SocketAddr, SocketAddr>,
}

/// Lock-free traffic counters.
pub struct ProxyCounters {
    pub pps_in: AtomicU64,
    pub pps_out: AtomicU64,
    pub bytes_in: AtomicU64,
    pub bytes_out: AtomicU64,
}

impl Default for ProxyState {
    fn default() -> Self {
        Self {
            stats: ProxyCounters {
                pps_in: AtomicU64::new(0),
                pps_out: AtomicU64::new(0),
                bytes_in: AtomicU64::new(0),
                bytes_out: AtomicU64::new(0),
            },
            sessions: DashMap::new(),
            reverse_map: DashMap::new(),
        }
    }
}

/// Maximum session idle time before cleanup (seconds).
const SESSION_TIMEOUT_SECS: u64 = 30;

/// MTU cap for RakNet connections.
pub const MTU_CAP: u16 = 1400;

/// Run the UDP proxy loop with bidirectional relay.
///
/// Returns a `(watch::Receiver<TrafficStats>, JoinHandle)` so the caller can
/// monitor stats without polling.
pub fn spawn_proxy(
    profile: Profile,
    cancel: CancellationToken,
) -> anyhow::Result<(tokio::sync::watch::Receiver<TrafficStats>, Arc<ProxyState>)> {
    let state = Arc::new(ProxyState::default());
    let (stats_tx, stats_rx) = tokio::sync::watch::channel(TrafficStats::default());

    let state_clone = state.clone();
    let server_guid: i64 = 0x12345678_9ABCDEF0;

    tokio::spawn(async move {
        if let Err(e) = run_proxy_inner(state_clone, profile, cancel, stats_tx, server_guid).await {
            tracing::error!("Proxy error: {e}");
        }
    });

    Ok((stats_rx, state))
}

async fn run_proxy_inner(
    state: Arc<ProxyState>,
    profile: Profile,
    cancel: CancellationToken,
    stats_tx: tokio::sync::watch::Sender<TrafficStats>,
    server_guid: i64,
) -> anyhow::Result<()> {
    let local_socket = UdpSocket::bind("0.0.0.0:19132").await?;
    let relay_socket = UdpSocket::bind("0.0.0.0:0").await?;

    let relay_addr = relay_socket.local_addr()?;
    tracing::info!(
        "UDP proxy listening on 0.0.0.0:19132, relay on {}",
        relay_addr
    );

    let remote_addr: SocketAddr = format!("{}:{}", profile.host, profile.port)
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid remote addr {}: {e}", profile.host))?;

    tracing::info!("Forwarding to {} (label: {})", remote_addr, profile.label);

    let mut local_buf = [0u8; 2048];
    let mut relay_buf = [0u8; 2048];

    let stats_interval = tokio::time::interval(std::time::Duration::from_millis(500));
    tokio::pin!(stats_interval);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                tracing::info!("Proxy shutting down");
                break Ok(());
            }
            // === Inbound from local consoles ===
            result = local_socket.recv_from(&mut local_buf) => {
                let (len, src_addr) = result?;
                let data = &local_buf[..len];

                state.stats.bytes_in.fetch_add(len as u64, Ordering::Relaxed);
                state.stats.pps_in.fetch_add(1, Ordering::Relaxed);

                // Handle discovery ping
                if discovery::is_unconnected_ping(data) {
                    let motd = discovery::build_motd(&profile.label, server_guid);
                    let ts = i64::from_be_bytes(data[1..9].try_into().unwrap_or([0; 8]));
                    let pong = discovery::build_pong(ts, server_guid, &motd);

                    local_socket.send_to(&pong, src_addr).await?;
                    state.stats.bytes_out.fetch_add(pong.len() as u64, Ordering::Relaxed);
                    state.stats.pps_out.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                // MTU management: cap RakNet connection packets
                let data = discovery::cap_mtu(data, MTU_CAP);
                let data = data.as_slice();

                // Create or update session
                state.sessions.entry(src_addr).or_insert(Session {
                    _remote_addr: remote_addr,
                    last_active: Instant::now(),
                }).value_mut().last_active = Instant::now();

                // Map remote → local for response routing
                state.reverse_map.insert(remote_addr, src_addr);

                relay_socket.send_to(data, remote_addr).await?;
                state.stats.bytes_out.fetch_add(len as u64, Ordering::Relaxed);
                state.stats.pps_out.fetch_add(1, Ordering::Relaxed);
            }
            // === Inbound from remote server (responses) ===
            result = relay_socket.recv_from(&mut relay_buf) => {
                let (len, from_addr) = result?;
                let data = &relay_buf[..len];

                if let Some(local_addr) = state.reverse_map.get(&from_addr) {
                    local_socket.send_to(data, *local_addr).await?;
                    state.stats.bytes_out.fetch_add(len as u64, Ordering::Relaxed);
                    state.stats.pps_out.fetch_add(1, Ordering::Relaxed);

                    if let Some(mut session) = state.sessions.get_mut(&*local_addr) {
                        session.last_active = Instant::now();
                    }
                } else {
                    tracing::warn!("Received response from unknown remote: {}", from_addr);
                }
            }
            // === Periodic stats emission + session cleanup ===
            _ = stats_interval.tick() => {
                // Clean stale sessions first
                let now = Instant::now();
                state.sessions.retain(|_, s| now.duration_since(s.last_active).as_secs() < SESSION_TIMEOUT_SECS);

                let clients: Vec<String> = state.sessions
                    .iter()
                    .map(|entry| entry.key().to_string())
                    .collect();

                let stats = TrafficStats {
                    pps_in: state.stats.pps_in.swap(0, Ordering::Relaxed),
                    pps_out: state.stats.pps_out.swap(0, Ordering::Relaxed),
                    bytes_in: state.stats.bytes_in.load(Ordering::Relaxed),
                    bytes_out: state.stats.bytes_out.load(Ordering::Relaxed),
                    active_sessions: state.sessions.len() as u32,
                    clients,
                };

                let _ = stats_tx.send(stats);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_state_default() {
        let state = ProxyState::default();
        assert!(state.sessions.is_empty());
        assert!(state.reverse_map.is_empty());
    }

    #[test]
    fn test_counter_operations() {
        let state = ProxyState::default();
        state.stats.bytes_in.fetch_add(100, Ordering::Relaxed);
        state.stats.bytes_in.fetch_add(50, Ordering::Relaxed);
        assert_eq!(state.stats.bytes_in.load(Ordering::Relaxed), 150);
        let old = state.stats.bytes_in.swap(0, Ordering::Relaxed);
        assert_eq!(old, 150);
        assert_eq!(state.stats.bytes_in.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_session_cleanup() {
        let state = ProxyState::default();
        let addr: SocketAddr = "192.168.1.50:12345".parse().unwrap();
        let remote: SocketAddr = "10.0.0.1:19132".parse().unwrap();

        state.sessions.entry(addr).or_insert(Session {
            _remote_addr: remote,
            last_active: Instant::now() - std::time::Duration::from_secs(60),
        });
        assert_eq!(state.sessions.len(), 1);

        let now = Instant::now();
        state
            .sessions
            .retain(|_, s| now.duration_since(s.last_active).as_secs() < SESSION_TIMEOUT_SECS);
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn test_traffic_stats_serialization() {
        let stats = TrafficStats {
            pps_in: 100,
            pps_out: 50,
            bytes_in: 1024,
            bytes_out: 2048,
            active_sessions: 3,
            clients: vec!["192.168.1.50:12345".into()],
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"pps_in\":100"));
        assert!(json.contains("\"active_sessions\":3"));
    }

    #[test]
    fn test_reverse_mapping() {
        let state = ProxyState::default();
        let local: SocketAddr = "192.168.1.50:12345".parse().unwrap();
        let remote: SocketAddr = "10.0.0.1:19132".parse().unwrap();

        state.reverse_map.insert(remote, local);
        assert_eq!(state.reverse_map.get(&remote).map(|r| *r), Some(local));
    }
}
