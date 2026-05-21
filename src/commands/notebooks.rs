//! `notebooks` command.
//!
//! Implemented in Task 9. Signature is frozen per the FROZEN COMMAND CONTRACT.

use crate::error::Result;
use crate::evernote::EvernoteClient;
use crate::output::Format;

pub fn list(client: &dyn EvernoteClient, fmt: Format) -> Result<String> {
    let _ = (client, fmt);
    todo!("notebooks list implemented in Task 9")
}
