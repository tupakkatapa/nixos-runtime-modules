use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{exit, Command};

// Constants
const SYSTEM_MODULES_DIR: &str = "/run/runtime-modules";
const MODULES_JSON: &str = "/run/runtime-modules/modules.json";
const MODULES_FILE: &str = "/run/runtime-modules/runtime-modules.nix";

// CLI arguments parsing structure
#[derive(Parser)]
#[command(author, version, about, long_about = None, disable_help_subcommand = true)]
struct Cli {
    /// Output results in JSON format
    #[arg(short = 'j', long)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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

// Module registry structure
#[derive(Serialize, Deserialize, Debug)]
struct ModuleRegistry {
    modules: Vec<Module>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Module {
    name: String,
    path: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ModuleStatus {
    name: String,
    path: String,
    enabled: bool,
}

fn main() {
    // Parse command line arguments
    let cli = Cli::parse();

    // Call the appropriate command handler
    match &cli.command {
        Commands::List => cmd_list(cli.json),
        Commands::Reset => {
            require_sudo("reset", &[]);
            cmd_reset();
        }
        Commands::Enable { modules } => {
            verify_modules(modules);
            require_sudo("enable", modules);
            cmd_enable(modules);
        }
        Commands::Disable { modules } => {
            verify_modules(modules);
            require_sudo("disable", modules);
            cmd_disable(modules);
        }
        Commands::Status { modules } => {
            verify_modules(modules);
            cmd_status(modules, cli.json);
        }
    }
}

// Ensure we have sudo access when needed
fn require_sudo(action: &str, args: &[String]) {
    if unsafe { libc::geteuid() } != 0 {
        println!("info: elevated privileges are required for this action");

        let program = env::current_exe().unwrap();
        let program_path = program.to_str().unwrap();

        // Prepare arguments for sudo command
        let mut sudo_args = vec![program_path, action];
        for arg in args {
            sudo_args.push(arg);
        }

        // Execute sudo with the current program
        let status = Command::new("sudo")
            .args(&sudo_args)
            .status()
            .expect("failed to execute sudo command");

        exit(status.code().unwrap_or(1));
    }
}

// Load modules registry from JSON
fn load_modules_registry() -> ModuleRegistry {
    let json_content = fs::read_to_string(MODULES_JSON).expect("failed to read modules.json");

    serde_json::from_str(&json_content).expect("failed to parse modules.json")
}

// Check if modules are exist
fn verify_modules(modules: &[String]) {
    let registry = load_modules_registry();
    let available_modules: HashSet<_> = registry.modules.iter().map(|m| &m.name).collect();

    for module in modules {
        if !available_modules.contains(module) {
            eprintln!("error: module '{module}' not found");
            cmd_list(false);
            exit(1);
        }
    }
}

// Get module path from the registry
fn get_module_path(module_name: &str) -> Option<String> {
    let registry = load_modules_registry();

    for module in registry.modules {
        if module.name == module_name {
            return Some(module.path);
        }
    }

    None
}

// Check if a module is enabled
fn is_module_enabled(module_name: &str) -> bool {
    if !Path::new(MODULES_FILE).exists() {
        return false;
    }

    let content = fs::read_to_string(MODULES_FILE).expect("failed to read modules file");

    // Look for lines matching the store path pattern ending with "# module_name"
    for line in content.lines() {
        let line = line.trim();
        // Check if this is a store path line and ends with the exact module name
        if line.contains("/nix/store/")
            && line.contains("-source/")
            && line.ends_with(&format!("# {module_name}"))
        {
            return true;
        }
    }

    false
}

// Get list of all active modules
fn get_active_modules() -> Vec<String> {
    if !Path::new(MODULES_FILE).exists() {
        return Vec::new();
    }

    let content = fs::read_to_string(MODULES_FILE).expect("failed to read modules file");

    let mut active_modules = Vec::new();

    // Extract module names from nix store path lines
    for line in content.lines() {
        let line = line.trim();
        // Only process lines that match our store path pattern
        if line.contains("/nix/store/") && line.contains("-source/") {
            if let Some(comment_pos) = line.find('#') {
                let comment_part = &line[comment_pos + 1..];
                let module_name = comment_part.trim();

                if !module_name.is_empty() {
                    active_modules.push(module_name.to_string());
                }
            }
        }
    }

    active_modules
}

// Create or update the runtime-modules.nix file with the specified modules
fn generate_modules_file(modules: &[String]) {
    let mut content = String::from("# This file is generated by runtime-module script\n");
    content.push_str("{ ... }:\n");
    content.push_str("{\n");

    if modules.is_empty() {
        content.push_str("  # No active modules\n");
    } else {
        content.push_str("  imports = [\n");

        // Add each module path
        for module in modules {
            if let Some(module_path) = get_module_path(module) {
                content.push_str(&format!("    \"{module_path}\" # {module}\n"));
            } else {
                eprintln!("warning: could not find path for module {module}");
            }
        }

        content.push_str("  ];\n");
    }

    content.push_str("}\n");

    // Write the file
    fs::write(MODULES_FILE, content).expect("failed to write modules file");

    // Fix permissions - set to 644 (rw-r--r--)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(MODULES_FILE)
            .expect("failed to get file metadata")
            .permissions();
        perms.set_mode(0o644);
        fs::set_permissions(MODULES_FILE, perms).expect("failed to set file permissions");
    }

    println!("generated modules file at '{MODULES_FILE}'");
}

// Apply the current configuration
fn apply_configuration() -> bool {
    println!("applying configuration...");

    // Change to the system modules directory
    env::set_current_dir(SYSTEM_MODULES_DIR).expect("failed to change to system modules directory");

    // Update flake before rebuild
    println!("updating flake...");
    let update_status = Command::new("nix")
        .args(["flake", "update", "--accept-flake-config", "--impure"])
        .status()
        .expect("failed to run nix flake update");

    if !update_status.success() {
        eprintln!("warning: flake update returned non-zero status");
    }

    // Run nixos-rebuild
    let rebuild_status = Command::new("nixos-rebuild")
        .args([
            "test",
            "--accept-flake-config",
            "--impure",
            "--flake",
            ".#runtime",
        ])
        .status()
        .expect("failed to run nixos-rebuild");

    let success = rebuild_status.success();

    if success {
        println!("configuration applied successfully");
    } else {
        println!(
            "warning: configuration applied with warnings (some changes may not be fully applied)"
        );
    }

    success
}

// Command implementations
fn cmd_list(json_output: bool) {
    let registry = load_modules_registry();

    if json_output {
        // Create a list of modules with enabled status
        let modules_with_status: Vec<ModuleStatus> = registry
            .modules
            .iter()
            .map(|module| {
                let name = &module.name;
                let enabled = is_module_enabled(name);
                ModuleStatus {
                    name: name.clone(),
                    path: module.path.clone(),
                    enabled,
                }
            })
            .collect();

        // Output as JSON
        let json = serde_json::to_string_pretty(&modules_with_status)
            .expect("failed to serialize modules to JSON");
        println!("{json}");
    } else {
        println!("Available modules:");

        for module in registry.modules {
            let name = &module.name;
            if is_module_enabled(name) {
                println!("  [✓] {name}");
            } else {
                println!("  [ ] {name}");
            }
        }
    }
}

fn cmd_reset() {
    println!("resetting to base system...");

    // Create empty modules file (no modules enabled)
    generate_modules_file(&[]);

    // Apply the configuration
    apply_configuration();
}

fn cmd_enable(modules: &[String]) {
    // Get current enabled modules
    let mut active_modules = get_active_modules();

    // Track if any changes are needed
    let mut changes_needed = false;

    // Process each module
    for module in modules {
        if is_module_enabled(module) {
            println!("module {module} is already enabled");
        } else {
            active_modules.push(module.clone());
            changes_needed = true;
        }
    }

    // Only rebuild if there were changes
    if changes_needed {
        // Generate modules file with updated modules list
        generate_modules_file(&active_modules);

        // Apply the configuration
        apply_configuration();
        println!("modules enabled successfully");
    } else {
        println!("no changes needed, skipping rebuild");
    }
}

fn cmd_disable(modules: &[String]) {
    // Get current enabled modules
    let active_modules = get_active_modules();

    // Prepare set of modules to disable
    let disable_set: HashSet<_> = modules.iter().collect();

    // Create new list excluding modules being disabled
    let mut new_enabled_modules = Vec::new();

    // Track if any changes are needed
    let mut changes_needed = false;

    for module in &active_modules {
        if disable_set.contains(module) {
            println!("disabling module {module}...");
            changes_needed = true;
        } else {
            new_enabled_modules.push(module.clone());
        }
    }

    // Check if any modules to disable were not enabled
    for module in modules {
        if !active_modules.contains(module) {
            println!("module {module} is already disabled");
        }
    }

    // Only rebuild if there were changes
    if changes_needed {
        // Generate modules file with updated modules list
        generate_modules_file(&new_enabled_modules);

        // Apply the configuration
        apply_configuration();
        println!("modules disabled successfully");
    } else {
        println!("no changes needed, skipping rebuild");
    }
}

fn cmd_status(modules: &[String], json_output: bool) {
    let mut exit_code = 0;

    if json_output {
        let status_list: Vec<_> = modules
            .iter()
            .map(|module| {
                let enabled = is_module_enabled(module);
                if !enabled {
                    exit_code = 1;
                }

                let path = get_module_path(module).unwrap_or_default();

                ModuleStatus {
                    name: module.clone(),
                    path,
                    enabled,
                }
            })
            .collect();

        // Output as JSON
        let json =
            serde_json::to_string_pretty(&status_list).expect("failed to serialize status to JSON");
        println!("{json}");
    } else {
        for module in modules {
            if is_module_enabled(module) {
                println!("enabled");
            } else {
                println!("disabled");
                exit_code = 1;
            }
        }
    }

    exit(exit_code);
}
