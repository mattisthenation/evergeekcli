//! Real [`EvernoteClient`] implementation backed by the generated EDAM Thrift
//! clients. Maps EDAM types to our domain `model` types so nothing outside this
//! module depends on Thrift.
//!
//! Implemented in Task 14.

use crate::config::Credentials;
use crate::error::Result;
use crate::evernote::EvernoteClient;
use crate::model::{NoteContent, NoteQuery, NoteSummary, Notebook};

pub struct ThriftClient {
    #[allow(dead_code)]
    creds: Credentials,
}

impl ThriftClient {
    pub fn new(creds: Credentials) -> Self {
        Self { creds }
    }
}

impl EvernoteClient for ThriftClient {
    fn search_notes(&self, _query: &NoteQuery) -> Result<Vec<NoteSummary>> {
        todo!("ThriftClient::search_notes is implemented in Task 14")
    }

    fn get_note(&self, _guid: &str) -> Result<NoteContent> {
        todo!("ThriftClient::get_note is implemented in Task 14")
    }

    fn list_notebooks(&self) -> Result<Vec<Notebook>> {
        todo!("ThriftClient::list_notebooks is implemented in Task 14")
    }
}
