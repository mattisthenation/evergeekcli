use crate::model::{NoteContent, NoteSummary, Notebook};

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum Format {
    Json,
    Text,
    Markdown,
}

/// Render a list of note summaries in the requested format.
///
/// Implemented in Task 4.
pub fn render_summaries(_items: &[NoteSummary], _fmt: Format) -> String {
    todo!("render_summaries is implemented in Task 4")
}

/// Render a single note in the requested format.
///
/// Implemented in Task 4.
pub fn render_note(_note: &NoteContent, _fmt: Format) -> String {
    todo!("render_note is implemented in Task 4")
}

/// Render a list of notebooks in the requested format.
///
/// Implemented in Task 4.
pub fn render_notebooks(_items: &[Notebook], _fmt: Format) -> String {
    todo!("render_notebooks is implemented in Task 4")
}
