use anyhow::{anyhow, Context, Result};
use std::env;
use std::process::{exit, Command};

// Constants
const SYSTEM_MODULES_DIR: &str = "/run/runtime-modules";

// Ensure we have sudo access when needed
pub fn require_sudo(action: &str, args: &[String], force: bool) -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        println!("info: elevated privileges are required for this action");

        let program = env::current_exe().context("failed to get current executable path")?;
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
            .context("failed to execute sudo command")?;

        exit(status.code().unwrap_or(1));
    }

    Ok(())
}

// Apply the current configuration
pub fn apply_configuration() -> Result<()> {
    println!("applying configuration...");

    // Change to the system modules directory
    env::set_current_dir(SYSTEM_MODULES_DIR).with_context(|| {
        format!("failed to change to system modules directory: {SYSTEM_MODULES_DIR}")
    })?;

    // Update flake before rebuild
    println!("updating flake...");
    let update_status = Command::new("nix")
        .args(["flake", "update", "--accept-flake-config", "--impure"])
        .status()
        .context("failed to run nix flake update")?;

    if !update_status.success() {
        eprintln!("warning: flake update returned non-zero status");
        // We continue despite warnings from flake update
    }

    // Run nixos-rebuild
    let rebuild_args = [
        "test",
        "--accept-flake-config",
        "--impure",
        "--flake",
        ".#runtime",
    ];

    let rebuild_status = Command::new("nixos-rebuild")
        .args(rebuild_args)
        .status()
        .context("failed to run nixos-rebuild")?;

    if rebuild_status.success() {
        println!("configuration applied successfully");
        Ok(())
    } else {
        Err(anyhow!(
            "warning: configuration applied with warnings (some changes may not be fully applied)"
        ))
    }
}
