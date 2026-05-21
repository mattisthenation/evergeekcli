//! `note get` command.
//!
//! Implemented in Task 9. Signature is frozen per the FROZEN COMMAND CONTRACT.

use crate::error::Result;
use crate::evernote::EvernoteClient;
use crate::output::{self, Format};

/// Fetch a single note by GUID and render it in the requested format.
///
/// Propagates [`crate::error::EvergeekError::NotFound`] (and any other client
/// error) unchanged.
pub fn get(client: &dyn EvernoteClient, guid: &str, fmt: Format) -> Result<String> {
    let note = client.get_note(guid)?;
    Ok(output::render_note(&note, fmt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::EvergeekError;
    use crate::evernote::mock;

    #[test]
    fn get_json_parses_and_contains_guid() {
        let client = mock::sample();
        let guid = &client.notes[0].summary.guid.clone();
        let out = get(&client, guid, Format::Json).expect("get should succeed");
        let value: serde_json::Value = serde_json::from_str(&out).expect("output should be JSON");
        assert_eq!(
            value["summary"]["guid"].as_str(),
            Some(guid.as_str()),
            "JSON should contain the note guid: {out}"
        );
    }

    #[test]
    fn get_text_contains_title() {
        let client = mock::sample();
        let note = &client.notes[0];
        let out = get(&client, &note.summary.guid, Format::Text).expect("get should succeed");
        assert!(
            out.contains(&note.summary.title),
            "text output should contain the title: {out}"
        );
    }

    #[test]
    fn get_markdown_contains_title() {
        let client = mock::sample();
        let note = &client.notes[0];
        let out = get(&client, &note.summary.guid, Format::Markdown).expect("get should succeed");
        assert!(
            out.contains(&note.summary.title),
            "markdown output should contain the title: {out}"
        );
    }

    #[test]
    fn get_missing_guid_is_not_found() {
        let client = mock::sample();
        let err = get(&client, "does-not-exist", Format::Json).expect_err("should error");
        match err {
            EvergeekError::NotFound(g) => assert_eq!(g, "does-not-exist"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
