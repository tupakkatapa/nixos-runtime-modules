use anyhow::{Context, Result};
use clap::Parser;

mod cli;
mod module_manager;
mod system;

use cli::{execute_command, Cli};

fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Execute the appropriate command
    execute_command(&cli).with_context(|| "command execution failed")
}
