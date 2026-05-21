//! Command-line interface definitions.
//!
//! This is a minimal compiling skeleton; the full argument surface (per the
//! spec command table) is fleshed out in Task 7.

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
    Auth,
    /// Search notes using Evernote's search grammar.
    Search,
    /// Show recently updated notes.
    Recent,
    /// Show notes with activity yesterday.
    Yesterday,
    /// Fetch a single note.
    Note,
    /// List notebooks.
    Notebooks,
}
