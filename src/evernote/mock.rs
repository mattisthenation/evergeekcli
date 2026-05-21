//! Fixture-backed [`EvernoteClient`] for tests.
//!
//! Implemented in Task 6.

use crate::error::Result;
use crate::evernote::EvernoteClient;
use crate::model::{NoteContent, NoteQuery, NoteSummary, Notebook};

pub struct MockClient {
    pub notes: Vec<NoteContent>,
    pub notebooks: Vec<Notebook>,
}

impl EvernoteClient for MockClient {
    fn search_notes(&self, _query: &NoteQuery) -> Result<Vec<NoteSummary>> {
        todo!("MockClient::search_notes is implemented in Task 6")
    }

    fn get_note(&self, _guid: &str) -> Result<NoteContent> {
        todo!("MockClient::get_note is implemented in Task 6")
    }

    fn list_notebooks(&self) -> Result<Vec<Notebook>> {
        todo!("MockClient::list_notebooks is implemented in Task 6")
    }
}
