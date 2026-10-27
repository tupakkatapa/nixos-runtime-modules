use anyhow::Result;
use clap::Parser;

mod cli;
mod module_manager;
mod system;

use cli::{Cli, execute_command};

fn main() -> Result<()> {
    // Check for deprecated invocation name
    if let Some(name) = std::env::args().next() {
        if name.ends_with("runtime-module") {
            eprintln!("warning: 'runtime-module' is deprecated, use 'runtime-modules' instead");
        }
    }

    // Parse command line arguments
    let cli = Cli::parse();

    // Execute the appropriate command
    execute_command(&cli)
}
