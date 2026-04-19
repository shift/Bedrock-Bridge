use anyhow::Result;
use bedrock_bridge_core::{JsonFileStore, Profile, ProfileStore, TrafficStats};
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(
    name = "bedrock-bridge",
    version,
    about = "UDP relay for Minecraft Bedrock Edition"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the UDP proxy relay
    Run {
        /// Display name shown in LAN discovery
        #[arg(short, long, default_value = "Bedrock Bridge")]
        label: String,

        /// Remote server host to proxy to
        #[arg(long)]
        host: String,

        /// Remote server port
        #[arg(short, long, default_value_t = 19132)]
        port: u16,
    },
    /// Manage server profiles
    Profiles {
        #[command(subcommand)]
        action: ProfileActions,
    },
}

#[derive(Subcommand)]
enum ProfileActions {
    /// List all saved profiles
    List,
    /// Add a new profile
    Add {
        /// Profile label
        #[arg(short, long)]
        label: String,
        /// Remote host
        #[arg(long)]
        host: String,
        /// Remote port
        #[arg(short, long, default_value_t = 19132)]
        port: u16,
    },
    /// Remove a profile by ID or label
    Remove {
        /// Profile ID or label
        id_or_label: String,
    },
    /// Start proxy using a saved profile
    Start {
        /// Profile ID or label
        id_or_label: String,
    },
}

fn get_store() -> Result<Arc<dyn ProfileStore>> {
    Ok(Arc::new(JsonFileStore::new(JsonFileStore::default_path())))
}

fn find_profile(store: &dyn ProfileStore, id_or_label: &str) -> Result<Profile> {
    // Try by ID first
    if let Ok(p) = store.get(id_or_label) {
        return Ok(p);
    }
    // Fallback: search by label
    let profiles = store.list().map_err(|e| anyhow::anyhow!(e))?;
    profiles
        .into_iter()
        .find(|p| p.label == id_or_label)
        .ok_or_else(|| anyhow::anyhow!("profile '{}' not found", id_or_label))
}

fn format_bytes(b: u64) -> String {
    if b < 1024 {
        format!("{} B", b)
    } else if b < 1_048_576 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{:.1} MB", b as f64 / 1_048_576.0)
    }
}

fn print_stats(stats: &TrafficStats) {
    print!(
        "\r\x1b[K↑{} pkt/s ↓{} pkt/s | ↑{} ↓{} | {} sessions",
        stats.pps_in,
        stats.pps_out,
        format_bytes(stats.bytes_out),
        format_bytes(stats.bytes_in),
        stats.active_sessions,
    );
    use std::io::Write;
    std::io::stdout().flush().ok();
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { label, host, port } => {
            run_proxy(&label, &host, port).await?;
        }
        Commands::Profiles { action } => match action {
            ProfileActions::List => {
                let store = get_store()?;
                let profiles = store.list().map_err(|e| anyhow::anyhow!(e))?;
                if profiles.is_empty() {
                    println!("No profiles saved.");
                } else {
                    println!("{:<36} {:<20} {:<20} {}", "ID", "Label", "Host", "Port");
                    for p in &profiles {
                        println!("{:<36} {:<20} {:<20} {}", p.id, p.label, p.host, p.port);
                    }
                }
            }
            ProfileActions::Add { label, host, port } => {
                let store = get_store()?;
                let profile = Profile::new(&label, &host, port);
                store.add(&profile).map_err(|e| anyhow::anyhow!(e))?;
                println!(
                    "✅ Added profile '{}' ({}:{})",
                    profile.label, profile.host, profile.port
                );
                println!("   ID: {}", profile.id);
            }
            ProfileActions::Remove { id_or_label } => {
                let store = get_store()?;
                let profile = find_profile(store.as_ref(), &id_or_label)?;
                store.delete(&profile.id).map_err(|e| anyhow::anyhow!(e))?;
                println!("🗑️  Removed profile '{}' ({})", profile.label, profile.id);
            }
            ProfileActions::Start { id_or_label } => {
                let store = get_store()?;
                let profile = find_profile(store.as_ref(), &id_or_label)?;
                println!(
                    "🚀 Starting proxy for '{}' → {}:{}",
                    profile.label, profile.host, profile.port
                );
                run_proxy(&profile.label, &profile.host, profile.port).await?;
            }
        },
    }

    Ok(())
}

async fn run_proxy(label: &str, host: &str, port: u16) -> Result<()> {
    let profile = Profile::new(label, host, port);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();

    // Handle Ctrl+C
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Ctrl+C received, shutting down...");
        cancel_clone.cancel();
    });

    let (mut stats_rx, _state) = bedrock_bridge_core::spawn_proxy(profile, cancel)?;

    println!("⛏️  Bedrock Bridge proxy running on UDP 19132");
    println!("   Forwarding to {}:{} ({})", host, port, label);
    println!("   Press Ctrl+C to stop\n");

    // Print live stats
    loop {
        tokio::select! {
            result = stats_rx.changed() => {
                if result.is_err() {
                    break;
                }
                let stats = stats_rx.borrow_and_update();
                print_stats(&stats);
            }
        }
    }

    println!("\n\nProxy stopped.");
    Ok(())
}
