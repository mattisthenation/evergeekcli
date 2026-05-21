//! `auth` command group: login / status / logout.
//!
//! Implemented in Task 11. Signatures are frozen per the FROZEN COMMAND
//! CONTRACT. These commands take no client; they use `auth::oauth` + `config`.

use crate::error::Result;

pub fn login(sandbox: bool) -> Result<String> {
    let _ = sandbox;
    todo!("auth login implemented in Task 11")
}

pub fn status() -> Result<String> {
    todo!("auth status implemented in Task 11")
}

pub fn logout() -> Result<String> {
    todo!("auth logout implemented in Task 11")
}
