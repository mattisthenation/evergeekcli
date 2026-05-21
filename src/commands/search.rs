//! `search` / `recent` / `yesterday` command group.
//!
//! Implemented in Task 8. Signatures are frozen per the FROZEN COMMAND
//! CONTRACT so parallel agents can rely on them.

use crate::error::Result;
use crate::evernote::EvernoteClient;
use crate::output::Format;

pub fn search(
    client: &dyn EvernoteClient,
    words: &str,
    notebook: Option<&str>,
    since: Option<&str>,
    limit: i32,
    fmt: Format,
) -> Result<String> {
    let _ = (client, words, notebook, since, limit, fmt);
    todo!("search implemented in Task 8")
}

pub fn recent(client: &dyn EvernoteClient, since: &str, limit: i32, fmt: Format) -> Result<String> {
    let _ = (client, since, limit, fmt);
    todo!("recent implemented in Task 8")
}

pub fn yesterday(
    client: &dyn EvernoteClient,
    notebook: Option<&str>,
    limit: i32,
    full: bool,
    fmt: Format,
) -> Result<String> {
    let _ = (client, notebook, limit, full, fmt);
    todo!("yesterday implemented in Task 8")
}
