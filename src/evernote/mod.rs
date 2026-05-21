use crate::error::Result;
use crate::model::{NoteContent, NoteQuery, NoteSummary, Notebook};

pub trait EvernoteClient {
    fn search_notes(&self, query: &NoteQuery) -> Result<Vec<NoteSummary>>;
    fn get_note(&self, guid: &str) -> Result<NoteContent>;
    fn list_notebooks(&self) -> Result<Vec<Notebook>>;
    // Future (v2): fn create_note(&self, ...) -> Result<NoteSummary>;
}

pub mod http_transport;
pub mod mock;
pub mod thrift_client;
