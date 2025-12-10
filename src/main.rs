use anyhow::Result;
use clap::{Parser, Subcommand};

mod config;
mod db;
mod defaults;
mod detect;
mod proxy;

#[derive(Parser)]
#[command(name = "proxyctl-rs")]
#[command(about = "A CLI tool for managing proxy configurations")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Enable proxy configuration
    On {
        /// Proxy server URL (optional, will detect if not provided)
        #[arg(short, long)]
        proxy: Option<String>,
    },
    /// Disable proxy configuration
    Off,
    /// Detect and display the best regional proxy
    Detect,
    /// Manage SSH configuration for proxy hosts
    Ssh {
        #[command(subcommand)]
        action: SshCommands,
    },
    /// Show current proxy status
    Status,
}

#[derive(Subcommand)]
enum SshCommands {
    /// Add proxy hosts to SSH config
    Add {
        /// Path to hosts file (optional, uses config default)
        #[arg(long)]
        hosts_file: Option<String>,
    },
    /// Remove proxy hosts from SSH config
    Remove,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if it exists
    let _ = dotenvy::dotenv();

    // Initialize config directory and files
    config::initialize_config()?;
    db::init_db().await?;

    let cli = Cli::parse();

    match cli.command {
        Commands::On { proxy } => {
            let resolved = proxy::resolve_proxy(proxy.as_deref()).await?;
            proxy::set_proxy(&resolved.proxy_url).await?;
            let hosts_file = config::get_hosts_file_path()?.to_string_lossy().to_string();
            config::add_ssh_hosts(&hosts_file, &resolved.proxy_host)?;
            println!("Proxy enabled");
        }
        Commands::Off => {
            proxy::disable_proxy().await?;
            config::remove_ssh_hosts()?;
            println!("Proxy disabled");
        }
        Commands::Detect => {
            let proxy = detect::detect_best_proxy().await?;
            println!("Best regional proxy: {proxy}");
        }
        Commands::Ssh { action } => match action {
            SshCommands::Add { hosts_file } => {
                let resolved = proxy::resolve_proxy(None).await?;
                let file = hosts_file.unwrap_or_else(|| {
                    config::get_hosts_file_path()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "default_hosts.example.txt".to_string())
                });
                config::add_ssh_hosts(&file, &resolved.proxy_host)?;
                println!("SSH hosts added from {file}");
            }
            SshCommands::Remove => {
                config::remove_ssh_hosts()?;
                println!("SSH hosts removed");
            }
        },
        Commands::Status => {
            let status = proxy::get_status().await?;
            println!("{status}");
        }
    }

    Ok(())
}
