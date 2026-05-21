//! `evergeek` binary entry point.
//!
//! Thin shell: parse args, dispatch to a command handler, print the result to
//! stdout. On error, print to stderr and exit non-zero so stdout stays clean
//! for JSON consumers (the daily-summary skill).

use std::process::ExitCode;

use clap::Parser;

use evergeekcli::cli::{Cli, Commands};

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatch a parsed CLI invocation to its handler, returning the rendered
/// output to print to stdout.
///
/// Real per-command dispatch arrives in Tasks 7–11; for now this just routes to
/// the (stubbed) command groups.
fn run(cli: Cli) -> evergeekcli::error::Result<String> {
    match cli.command {
        Commands::Auth => todo!("auth dispatch arrives in Task 11"),
        Commands::Search => todo!("search dispatch arrives in Task 8"),
        Commands::Recent => todo!("recent dispatch arrives in Task 8"),
        Commands::Yesterday => todo!("yesterday dispatch arrives in Task 8"),
        Commands::Note => todo!("note dispatch arrives in Task 9"),
        Commands::Notebooks => todo!("notebooks dispatch arrives in Task 9"),
    }
}
