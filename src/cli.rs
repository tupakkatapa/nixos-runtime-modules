use clap::{Parser, Subcommand};
use std::process::exit;

use crate::module_manager::ModuleManager;
use crate::system::require_sudo;
use runtime_module::ModuleError;

// CLI arguments parsing structure
#[derive(Parser)]
#[command(author, version, about, long_about = None, disable_help_subcommand = true)]
pub struct Cli {
    /// Output results in JSON format
    #[arg(short = 'j', long)]
    pub json: bool,

    /// Force rebuild even if no changes are detected
    #[arg(short = 'f', long)]
    pub force: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build and enable one or more modules
    Enable {
        /// Module names to enable
        #[arg(required = true)]
        modules: Vec<String>,
    },
    /// Disable one or more specific modules
    Disable {
        /// Module names to disable
        #[arg(required = true)]
        modules: Vec<String>,
    },
    /// Disable all modules (revert to base system)
    Reset,
    /// Show module status (enabled/disabled)
    Status {
        /// Module names to check status
        #[arg(required = true)]
        modules: Vec<String>,
    },
    /// List all available modules
    List,
}

// Execute the selected command
pub fn execute_command(cli: &Cli) -> Result<(), ModuleError> {
    match &cli.command {
        Commands::List => cmd_list(cli.json),
        Commands::Reset => {
            require_sudo("reset", &[], cli.force);
            cmd_reset(cli.force)
        }
        Commands::Enable { modules } => {
            cmd_verify_modules(modules)?;
            require_sudo("enable", modules, cli.force);
            cmd_enable(modules, cli.force)
        }
        Commands::Disable { modules } => {
            cmd_verify_modules(modules)?;
            require_sudo("disable", modules, cli.force);
            cmd_disable(modules, cli.force)
        }
        Commands::Status { modules } => {
            cmd_verify_modules(modules)?;
            cmd_status(modules, cli.json)
        }
    }
}

// Command implementations
fn cmd_verify_modules(modules: &[String]) -> Result<(), ModuleError> {
    let manager = ModuleManager::new()?;

    if !manager.verify_modules_exist(modules) {
        eprintln!("error: one or more modules not found");
        cmd_list(false)?;
        exit(1);
    }

    Ok(())
}

fn cmd_list(json_output: bool) -> Result<(), ModuleError> {
    let manager = ModuleManager::new()?;
    let modules_with_status = manager.get_all_status();

    if json_output {
        // Output as JSON
        let json = serde_json::to_string_pretty(&modules_with_status)
            .map_err(|e| ModuleError::ParseError(e.to_string()))?;
        println!("{json}");
    } else {
        println!("Available modules:");

        for status in modules_with_status {
            if status.enabled {
                println!("  [✓] {}", status.name);
            } else {
                println!("  [ ] {}", status.name);
            }
        }
    }

    Ok(())
}

fn cmd_reset(force: bool) -> Result<(), ModuleError> {
    let mut manager = ModuleManager::new()?;
    manager.reset(force)
}

fn cmd_enable(modules: &[String], force: bool) -> Result<(), ModuleError> {
    let mut manager = ModuleManager::new()?;
    manager.enable_modules(modules, force)?;
    Ok(())
}

fn cmd_disable(modules: &[String], force: bool) -> Result<(), ModuleError> {
    let mut manager = ModuleManager::new()?;
    manager.disable_modules(modules, force)?;
    Ok(())
}

fn cmd_status(modules: &[String], json_output: bool) -> Result<(), ModuleError> {
    let manager = ModuleManager::new()?;
    let status_list = manager.get_status(modules);
    let any_disabled = status_list.iter().any(|status| !status.enabled);

    if json_output {
        // Output as JSON
        let json = serde_json::to_string_pretty(&status_list)
            .map_err(|e| ModuleError::ParseError(e.to_string()))?;
        println!("{json}");
    } else {
        for status in &status_list {
            if status.enabled {
                println!("enabled");
            } else {
                println!("disabled");
            }
        }
    }

    // Exit with non-zero status if any module is disabled
    if any_disabled {
        exit(1);
    }

    Ok(())
}
