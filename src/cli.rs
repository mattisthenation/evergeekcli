//! Command-line interface definitions.
//!
//! This is the frozen command surface (per the spec command table). Parallel
//! agents implement the command bodies against the signatures referenced here.

use clap::{Parser, Subcommand};

use crate::output::Format;

#[derive(Debug, Parser)]
#[command(
    name = "evergeek",
    version,
    about = "Search and summarize Evernote notes"
)]
pub struct Cli {
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Json, global = true)]
    pub format: Format,

    /// Use the Evernote sandbox instead of production.
    #[arg(long, global = true)]
    pub sandbox: bool,

    /// Verbose logging.
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Authentication subcommands (login/status/logout).
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Search notes using Evernote's search grammar.
    Search {
        /// Search words (Evernote search grammar).
        query: String,
        /// Restrict to a notebook (by name).
        #[arg(long)]
        notebook: Option<String>,
        /// Only notes since this point: `1d`/`Nd` or `YYYY-MM-DD`.
        #[arg(long)]
        since: Option<String>,
        /// Maximum number of results.
        #[arg(long, default_value_t = 50)]
        limit: i32,
    },
    /// Show recently updated notes.
    Recent {
        /// Lookback window: `1d`/`Nd` or `YYYY-MM-DD`.
        #[arg(long, default_value = "1d")]
        since: String,
        /// Maximum number of results.
        #[arg(long, default_value_t = 50)]
        limit: i32,
    },
    /// Show notes with activity yesterday.
    Yesterday {
        /// Restrict to a notebook (by name).
        #[arg(long)]
        notebook: Option<String>,
        /// Maximum number of results.
        #[arg(long, default_value_t = 50)]
        limit: i32,
        /// Fetch full note content for each result.
        #[arg(long)]
        full: bool,
    },
    /// Fetch a single note.
    Note {
        #[command(subcommand)]
        action: NoteAction,
    },
    /// List notebooks.
    Notebooks,
}

#[derive(Debug, Subcommand)]
pub enum AuthAction {
    /// Authenticate with Evernote via OAuth.
    Login,
    /// Show current authentication status.
    Status,
    /// Remove stored credentials.
    Logout,
}

#[derive(Debug, Subcommand)]
pub enum NoteAction {
    /// Fetch a note by GUID.
    Get {
        /// Note GUID.
        guid: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_yesterday_with_full_and_limit() {
        let cli = Cli::try_parse_from(["evergeek", "yesterday", "--full", "--limit", "10"])
            .expect("yesterday should parse");
        match cli.command {
            Commands::Yesterday {
                notebook,
                limit,
                full,
            } => {
                assert_eq!(notebook, None);
                assert_eq!(limit, 10);
                assert!(full);
            }
            other => panic!("expected Yesterday, got {other:?}"),
        }
    }

    #[test]
    fn parses_search_with_notebook() {
        let cli =
            Cli::try_parse_from(["evergeek", "search", "project alpha", "--notebook", "Work"])
                .expect("search should parse");
        match cli.command {
            Commands::Search {
                query,
                notebook,
                since,
                limit,
            } => {
                assert_eq!(query, "project alpha");
                assert_eq!(notebook.as_deref(), Some("Work"));
                assert_eq!(since, None);
                assert_eq!(limit, 50);
            }
            other => panic!("expected Search, got {other:?}"),
        }
    }
}
