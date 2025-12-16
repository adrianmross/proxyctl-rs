use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

mod config;
mod db;
mod defaults;
mod detect;
mod doctor;
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
    /// Enable proxy configuration and add SSH hosts
    On {
        /// Proxy server URL (optional, will detect if not provided)
        #[arg(short, long)]
        proxy: Option<String>,
    },
    /// Disable proxy configuration and remove SSH hosts
    Off,
    /// Manage proxy configuration without touching SSH
    Proxy {
        #[command(subcommand)]
        action: ProxyCommands,
    },
    /// Detect and display the best regional proxy
    Detect,
    /// Manage SSH configuration for proxy hosts
    Ssh {
        #[command(subcommand)]
        action: SshCommands,
    },
    /// Show current status information
    Status {
        #[command(subcommand)]
        action: Option<StatusCommands>,
    },
    /// Run diagnostics or inspect configuration state
    Doctor {
        #[command(subcommand)]
        action: Option<DoctorCommands>,
    },
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

#[derive(Subcommand)]
enum ProxyCommands {
    /// Enable proxy configuration only
    On {
        /// Proxy server URL (optional, will detect if not provided)
        #[arg(short, long)]
        proxy: Option<String>,
    },
    /// Disable proxy configuration only
    Off,
}

#[derive(Subcommand, Clone)]
enum DoctorCommands {
    /// Run diagnostics for configuration and database
    Run,
    /// Display the current and default configuration values
    Config,
}

#[derive(Subcommand, Clone)]
enum StatusCommands {
    /// Show only proxy status details
    Proxy,
    /// Show only SSH status details
    Ssh,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if it exists
    let _ = dotenvy::dotenv();

    // Initialize config directory and files
    config::initialize_config()?;
    db::init_db(&db::get_db_path()).await?;

    let cli = Cli::parse();

    match cli.command {
        Commands::On { proxy } => {
            let resolved = configure_proxy(proxy.as_deref()).await?;
            let hosts_file = config::get_hosts_file_path()?.to_string_lossy().to_string();
            config::add_ssh_hosts(&hosts_file, &resolved.proxy_host)?;
            println!("Proxy enabled and SSH hosts added");
        }
        Commands::Off => {
            proxy::disable_proxy().await?;
            config::remove_ssh_hosts()?;
            println!("Proxy disabled and SSH hosts removed");
        }
        Commands::Proxy { action } => match action {
            ProxyCommands::On { proxy } => {
                configure_proxy(proxy.as_deref()).await?;
                println!("Proxy enabled");
            }
            ProxyCommands::Off => {
                proxy::disable_proxy().await?;
                println!("Proxy disabled");
            }
        },
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
        Commands::Status { action } => match action {
            Some(StatusCommands::Proxy) => {
                print_proxy_status().await?;
            }
            Some(StatusCommands::Ssh) => {
                print_ssh_status()?;
            }
            None => {
                print_proxy_status().await?;
                println!();
                print_ssh_status()?;
            }
        },
        Commands::Doctor { action } => match action.unwrap_or(DoctorCommands::Run) {
            DoctorCommands::Run => {
                doctor::run().await?;
            }
            DoctorCommands::Config => {
                doctor::print_config()?;
            }
        },
    }

    Ok(())
}

async fn configure_proxy(proxy: Option<&str>) -> Result<proxy::ResolvedProxy> {
    let resolved = proxy::resolve_proxy(proxy).await?;
    proxy::set_proxy(&resolved.proxy_url).await?;
    Ok(resolved)
}

async fn print_proxy_status() -> Result<()> {
    let status = proxy::get_status().await?;
    println!("{status}");
    Ok(())
}

fn print_ssh_status() -> Result<()> {
    let status = config::get_ssh_status()?;
    println!("{}", format_ssh_status(&status));
    Ok(())
}

fn format_ssh_status(status: &config::SshStatus) -> String {
    let mut lines = Vec::new();

    let state_label = if !status.hosts_file_exists {
        "Hosts file missing".red().bold().to_string()
    } else if !status.config_exists {
        "SSH config missing".red().bold().to_string()
    } else if status.hosts.is_empty() {
        "No hosts configured".yellow().bold().to_string()
    } else if status.missing_hosts.is_empty() {
        "Configured".green().bold().to_string()
    } else {
        "Partially configured".yellow().bold().to_string()
    };

    lines.push(format!("{}: {}", "SSH Config".bold(), state_label));
    lines.push(format!(
        "Config file: {}{}",
        status.config_path.display(),
        if status.config_exists {
            ""
        } else {
            " (missing)"
        }
    ));
    lines.push(format!(
        "Hosts file: {}{}",
        status.hosts_path.display(),
        if status.hosts_file_exists {
            ""
        } else {
            " (missing)"
        }
    ));

    if status.hosts_file_exists {
        if status.hosts.is_empty() {
            lines.push("No hosts listed in hosts file".to_string());
        } else {
            lines.push(format!("Tracked hosts ({}):", status.hosts.len()));
            for host in &status.hosts {
                let indicator = if status
                    .configured_hosts
                    .iter()
                    .any(|configured| configured.eq_ignore_ascii_case(host))
                {
                    "✓".green().to_string()
                } else {
                    "✗".red().to_string()
                };
                lines.push(format!("  {indicator} {host}"));
            }
        }

        if !status.missing_hosts.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "Missing hosts: {}",
                status.missing_hosts.join(", ")
            ));
        }
    }

    lines.join("\n")
}
