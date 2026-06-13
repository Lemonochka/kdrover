use clap::{Parser, Subcommand};
use drover_core::{
    find_installed_discord_dirs, install, load_options, uninstall, InstallSettings, DLL_FILENAME,
    OPTIONS_FILENAME,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "kdrover-cli", about = "KDrover command-line installer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Install {
        #[arg(long)]
        proxy: Option<String>,

        #[arg(long, default_value = "target/release/kdrover_payload.dll")]
        dll: PathBuf,

        #[arg(long)]
        discord_dir: Option<PathBuf>,
    },
    Uninstall {
        #[arg(long)]
        discord_dir: Option<PathBuf>,
    },
    List,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Install { proxy, dll, .. } => {
            let exe_dir = std::env::current_exe()?
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let mut settings = InstallSettings::default();
            if let Some(proxy) = proxy {
                settings = drover_core::InstallSettings::from_proxy(
                    &drover_core::ProxyValue::parse_from_string(&proxy),
                );
            }
            let installed = install(&exe_dir, &settings, Some(&dll))?;
            for dir in installed {
                println!("Installed into {}", dir.display());
            }
        }
        Commands::Uninstall { .. } => {
            for dir in uninstall()? {
                println!("Removed from {}", dir.display());
            }
        }
        Commands::List => list()?,
    }

    Ok(())
}

fn list() -> Result<(), Box<dyn std::error::Error>> {
    let dirs = find_installed_discord_dirs();
    if dirs.is_empty() {
        println!("No Discord installations found");
        return Ok(());
    }

    for dir in dirs {
        let options = load_options(dir.join(OPTIONS_FILENAME));
        let status = if dir.join(DLL_FILENAME).exists() {
            "installed"
        } else {
            "not installed"
        };
        println!(
            "{} [{status}] proxy={}",
            dir.display(),
            if options.proxy.is_empty() {
                "(direct mode)"
            } else {
                options.proxy.as_str()
            }
        );
    }

    Ok(())
}
