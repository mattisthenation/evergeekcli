//! `evergeek` binary entry point.
//!
//! Thin shell: parse args, dispatch to a command handler, print the result to
//! stdout. On error, print to stderr and exit non-zero so stdout stays clean
//! for JSON consumers (the daily-summary skill).

use std::process::ExitCode;

use clap::Parser;

use evergeekcli::cli::{AuthAction, Cli, Commands, NoteAction};
use evergeekcli::commands;
use evergeekcli::evernote::EvernoteClient;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Build the data-command Evernote client.
///
/// Loads `Credentials` and constructs a `ThriftClient`. The real client is
/// wired in Phase 4/5; this stub keeps the dispatch surface compiling.
fn build_client(sandbox: bool) -> evergeekcli::error::Result<Box<dyn EvernoteClient>> {
    let _ = sandbox;
    todo!("real client wired in Phase 4/5")
}

/// Dispatch a parsed CLI invocation to its handler, returning the rendered
/// output to print to stdout.
fn run(cli: Cli) -> evergeekcli::error::Result<String> {
    let fmt = cli.format;
    match cli.command {
        Commands::Auth { action } => match action {
            AuthAction::Login => commands::auth::login(cli.sandbox),
            AuthAction::Status => commands::auth::status(),
            AuthAction::Logout => commands::auth::logout(),
        },
        Commands::Search {
            query,
            notebook,
            since,
            limit,
        } => {
            let client = build_client(cli.sandbox)?;
            commands::search::search(
                client.as_ref(),
                &query,
                notebook.as_deref(),
                since.as_deref(),
                limit,
                fmt,
            )
        }
        Commands::Recent { since, limit } => {
            let client = build_client(cli.sandbox)?;
            commands::search::recent(client.as_ref(), &since, limit, fmt)
        }
        Commands::Yesterday {
            notebook,
            limit,
            full,
        } => {
            let client = build_client(cli.sandbox)?;
            commands::search::yesterday(client.as_ref(), notebook.as_deref(), limit, full, fmt)
        }
        Commands::Note { action } => match action {
            NoteAction::Get { guid } => {
                let client = build_client(cli.sandbox)?;
                commands::note::get(client.as_ref(), &guid, fmt)
            }
        },
        Commands::Notebooks => {
            let client = build_client(cli.sandbox)?;
            commands::notebooks::list(client.as_ref(), fmt)
        }
    }
}
