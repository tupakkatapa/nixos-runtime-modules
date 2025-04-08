use clap::{Parser, Subcommand};
use serde::Serialize;
use std::process::exit;

use crate::module_manager::ModuleManager;
use crate::system::require_sudo;
use runtime_module::{ModuleError, ModuleStatus};

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

// Structure for categorized output
#[derive(Serialize)]
struct CategorizedModules {
    user_modules: Vec<ModuleStatus>,
    upstream_modules: Vec<ModuleStatus>,
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
    /// Rebuild the system with currently enabled modules
    Rebuild,
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
        Commands::Rebuild => {
            require_sudo("rebuild", &[], cli.force);
            cmd_rebuild(cli.force)
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

    // Split modules into rt modules and user modules
    let (rt_modules, user_modules): (Vec<_>, Vec<_>) = modules_with_status
        .into_iter()
        .partition(|status| status.name.starts_with("rt."));

    if json_output {
        // Output as JSON
        let categorized = CategorizedModules {
            user_modules,
            upstream_modules: rt_modules,
        };

        let json = serde_json::to_string_pretty(&categorized)
            .map_err(|e| ModuleError::ParseError(e.to_string()))?;
        println!("{json}");
    } else {
        // Check if both module lists are empty
        if user_modules.is_empty() && rt_modules.is_empty() {
            println!("no modules available");
            return Ok(());
        }

        // Find the longest module name for alignment
        let max_name_length = user_modules
            .iter()
            .chain(rt_modules.iter())
            .map(|status| status.name.len())
            .max()
            .unwrap_or(0);

        println!("\u{001b}[4mAvailable modules:\u{001b}[0m");

        // Print user modules if any exist
        if !user_modules.is_empty() {
            for status in &user_modules {
                print_module_status(status, max_name_length);
            }
            if !rt_modules.is_empty() {
                println!("\n\u{001b}[4mUpstream modules:\u{001b}[0m");
            }
        }

        // Print rt modules if any exist
        if !rt_modules.is_empty() {
            for status in &rt_modules {
                print_module_status(status, max_name_length);
            }
        }
    }

    Ok(())
}

// Helper function to print a module status with proper formatting
fn print_module_status(status: &ModuleStatus, max_name_length: usize) {
    let status_marker = if status.enabled { "[✓]" } else { "[ ]" };

    // Create padded name for alignment
    let padded_name = format!("{:<width$}", status.name, width = max_name_length);

    // Format the output to include description
    if status.desc.is_empty() {
        println!("  {status_marker} {padded_name}");
    } else {
        println!("  {status_marker} {padded_name}  {}", status.desc);
    }
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

fn cmd_rebuild(force: bool) -> Result<(), ModuleError> {
    let manager = ModuleManager::new()?;
    let _ = manager.rebuild(force);
    Ok(())
}
