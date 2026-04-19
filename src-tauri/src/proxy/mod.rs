use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

use bedrock_bridge_core::{self as core, TrafficStats};

/// Holds the cancel token and stats receiver for the Tauri app.
pub struct ProxyHandle {
    pub cancel_token: Mutex<tokio_util::sync::CancellationToken>,
    pub stats_rx: Arc<Mutex<Option<tokio::sync::watch::Receiver<TrafficStats>>>>,
}

impl Default for ProxyHandle {
    fn default() -> Self {
        Self {
            cancel_token: Mutex::new(tokio_util::sync::CancellationToken::new()),
            stats_rx: Arc::new(Mutex::new(None)),
        }
    }
}

#[derive(Clone, serde::Serialize)]
pub struct ProxyStatus {
    pub state: String, // "starting" | "running" | "retrying" | "failed" | "stopped"
    pub message: String,
    pub retry_count: u32,
}

#[tauri::command]
pub async fn start_proxy(
    host: String,
    port: u16,
    label: String,
    handle: State<'_, ProxyHandle>,
    app_handle: AppHandle,
) -> Result<(), String> {
    // Cancel any existing proxy
    {
        let token = handle.cancel_token.lock().await;
        token.cancel();
    }

    let cancel = {
        let mut guard = handle.cancel_token.lock().await;
        *guard = tokio_util::sync::CancellationToken::new();
        guard.clone()
    };

    // Clone the shared state for the background task
    let stats_rx_arc = handle.stats_rx.clone();

    // Spawn the proxy manager task with auto-reconnect
    tokio::spawn(async move {
        let mut retry_count: u32 = 0;
        let max_retries: u32 = 10;
        let base_delay = Duration::from_secs(1);

        loop {
            let profile = core::Profile::new(&label, &host, port);

            let _ = app_handle.emit(
                "proxy-status",
                ProxyStatus {
                    state: if retry_count > 0 { "retrying".into() } else { "starting".into() },
                    message: if retry_count > 0 {
                        format!("Reconnecting to {}:{} (attempt {})", host, port, retry_count)
                    } else {
                        format!("Connecting to {}:{}", host, port)
                    },
                    retry_count,
                },
            );

            match core::spawn_proxy(profile, cancel.clone()) {
                Ok((stats_rx, _state)) => {
                    // Store stats receiver
                    {
                        let mut rx_guard = stats_rx_arc.lock().await;
                        *rx_guard = Some(stats_rx);
                    }

                    retry_count = 0;

                    let _ = app_handle.emit(
                        "proxy-status",
                        ProxyStatus {
                            state: "running".into(),
                            message: format!("Connected to {}:{}", host, port),
                            retry_count: 0,
                        },
                    );

                    // Watch stats channel — when it closes, proxy died
                    let mut rx = {
                        let guard = stats_rx_arc.lock().await;
                        guard.clone()
                    };
                    if let Some(ref mut rx) = rx {
                        loop {
                            tokio::select! {
                                _ = cancel.cancelled() => {
                                    let _ = app_handle.emit(
                                        "proxy-status",
                                        ProxyStatus {
                                            state: "stopped".into(),
                                            message: "Proxy stopped".into(),
                                            retry_count: 0,
                                        },
                                    );
                                    return; // Cancelled — don't retry
                                }
                                result = rx.changed() => {
                                    if result.is_err() {
                                        // Stats channel closed — proxy died
                                        break;
                                    }
                                    let stats = rx.borrow_and_update().clone();
                                    let _ = app_handle.emit("traffic-stats", &stats);
                                }
                            }
                        }
                    }

                    tracing::warn!("Proxy stopped unexpectedly, will retry");
                }
                Err(e) => {
                    tracing::error!("Failed to start proxy: {e}");
                }
            }

            // Check if we were cancelled before retrying
            if cancel.is_cancelled() {
                let _ = app_handle.emit(
                    "proxy-status",
                    ProxyStatus {
                        state: "stopped".into(),
                        message: "Proxy stopped".into(),
                        retry_count: 0,
                    },
                );
                return;
            }

            retry_count += 1;
            if retry_count > max_retries {
                let _ = app_handle.emit(
                    "proxy-status",
                    ProxyStatus {
                        state: "failed".into(),
                        message: format!("Failed after {} retries. Click toggle to retry.", max_retries),
                        retry_count,
                    },
                );
                return;
            }

            // Exponential backoff: 1s, 2s, 4s, 8s, 16s, capped at 30s
            let delay = base_delay * 2u32.saturating_pow(retry_count.min(5));
            let delay = delay.min(Duration::from_secs(30));

            let _ = app_handle.emit(
                "proxy-status",
                ProxyStatus {
                    state: "retrying".into(),
                    message: format!("Retrying in {}s (attempt {}/{})", delay.as_secs(), retry_count, max_retries),
                    retry_count,
                },
            );

            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = cancel.cancelled() => {
                    let _ = app_handle.emit(
                        "proxy-status",
                        ProxyStatus {
                            state: "stopped".into(),
                            message: "Proxy stopped".into(),
                            retry_count: 0,
                        },
                    );
                    return;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn stop_proxy(handle: State<'_, ProxyHandle>) -> Result<(), String> {
    let guard = handle.cancel_token.lock().await;
    guard.cancel();
    Ok(())
}

#[tauri::command]
pub async fn get_traffic_stats(handle: State<'_, ProxyHandle>) -> Result<TrafficStats, String> {
    let guard = handle.stats_rx.lock().await;
    if let Some(rx) = guard.as_ref() {
        Ok(rx.borrow().clone())
    } else {
        Ok(TrafficStats::default())
    }
}
