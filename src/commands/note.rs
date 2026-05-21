//! `note get` command.
//!
//! Implemented in Task 9. Signature is frozen per the FROZEN COMMAND CONTRACT.

use crate::error::Result;
use crate::evernote::EvernoteClient;
use crate::output::Format;

pub fn get(client: &dyn EvernoteClient, guid: &str, fmt: Format) -> Result<String> {
    let _ = (client, guid, fmt);
    todo!("note get implemented in Task 9")
}
