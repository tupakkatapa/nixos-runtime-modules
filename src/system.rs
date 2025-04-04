use runtime_module::ModuleError;
use std::env;
use std::path::Path;
use std::process::{exit, Command};

// Constants
const SYSTEM_MODULES_DIR: &str = "/run/runtime-modules";

// Ensure we have sudo access when needed
pub fn require_sudo(action: &str, args: &[String], force: bool) {
    if unsafe { libc::geteuid() } != 0 {
        println!("info: elevated privileges are required for this action");

        let program =
            env::current_exe().unwrap_or_else(|_| Path::new("runtime-module").to_path_buf());
        let program_path = program.to_str().unwrap_or("runtime-module");

        // Prepare arguments for sudo command
        let mut sudo_args = vec![program_path];

        // Add force flag if requested
        if force {
            sudo_args.push("--force");
        }

        sudo_args.push(action);
        sudo_args.extend(args.iter().map(String::as_str));

        // Execute sudo with the current program
        let status = Command::new("sudo")
            .args(&sudo_args)
            .status()
            .expect("failed to execute sudo command");

        exit(status.code().unwrap_or(1));
    }
}

// Apply the current configuration
pub fn apply_configuration() -> Result<(), ModuleError> {
    println!("applying configuration...");

    // Change to the system modules directory
    if let Err(e) = env::set_current_dir(SYSTEM_MODULES_DIR) {
        let msg = format!("failed to change to system modules directory: {e}");
        eprintln!("{msg}");
        return Err(ModuleError::RebuildError(msg));
    }

    // Update flake before rebuild
    println!("updating flake...");
    let update_status = Command::new("nix")
        .args(["flake", "update", "--accept-flake-config", "--impure"])
        .status();

    match update_status {
        Ok(status) if !status.success() => {
            eprintln!("warning: flake update returned non-zero status");
            // We continue despite warnings from flake update
        }
        Err(e) => {
            let msg = format!("failed to run nix flake update: {e}");
            eprintln!("{msg}");
            return Err(ModuleError::RebuildError(msg));
        }
        _ => {}
    }

    // Run nixos-rebuild
    let rebuild_args = [
        "test",
        "--accept-flake-config",
        "--impure",
        "--flake",
        ".#runtime",
    ];

    match Command::new("nixos-rebuild").args(rebuild_args).status() {
        Ok(status) => {
            if status.success() {
                println!("configuration applied successfully");
                Ok(())
            } else {
                let msg = "warning: configuration applied with warnings (some changes may not be fully applied)".to_string();
                println!("{msg}");
                Err(ModuleError::RebuildError(msg))
            }
        }
        Err(e) => {
            let msg = format!("failed to run nixos-rebuild: {e}");
            eprintln!("{msg}");
            Err(ModuleError::RebuildError(msg))
        }
    }
}
