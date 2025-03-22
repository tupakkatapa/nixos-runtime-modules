use clap::Parser;
use std::process::exit;

mod cli;
mod module_manager;
mod system;

use cli::{execute_command, Cli};

fn main() {
    // Parse command line arguments
    let cli = Cli::parse();

    // Execute the appropriate command
    if let Err(e) = execute_command(&cli) {
        eprintln!("Error: {e}");
        exit(1);
    }
}
