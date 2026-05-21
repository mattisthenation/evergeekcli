//! `notebooks` command.
//!
//! Implemented in Task 9. Signature is frozen per the FROZEN COMMAND CONTRACT.

use crate::error::Result;
use crate::evernote::EvernoteClient;
use crate::output::{self, Format};

/// List all notebooks and render them in the requested format.
pub fn list(client: &dyn EvernoteClient, fmt: Format) -> Result<String> {
    let notebooks = client.list_notebooks()?;
    Ok(output::render_notebooks(&notebooks, fmt))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evernote::mock;
    use crate::model::Notebook;

    #[test]
    fn list_json_parses_and_lists_sample_notebooks() {
        let client = mock::sample();
        let out = list(&client, Format::Json).expect("list should succeed");
        let notebooks: Vec<Notebook> =
            serde_json::from_str(&out).expect("output should be JSON array of notebooks");
        let names: Vec<&str> = notebooks.iter().map(|nb| nb.name.as_str()).collect();
        assert!(names.contains(&"Work"), "expected Work notebook: {out}");
        assert!(
            names.contains(&"Personal"),
            "expected Personal notebook: {out}"
        );
    }

    #[test]
    fn list_text_contains_notebook_names() {
        let client = mock::sample();
        let out = list(&client, Format::Text).expect("list should succeed");
        assert!(
            out.contains("Work"),
            "text output should contain Work: {out}"
        );
        assert!(
            out.contains("Personal"),
            "text output should contain Personal: {out}"
        );
    }

    #[test]
    fn list_markdown_contains_notebook_names() {
        let client = mock::sample();
        let out = list(&client, Format::Markdown).expect("list should succeed");
        assert!(
            out.contains("Work"),
            "markdown output should contain Work: {out}"
        );
        assert!(
            out.contains("Personal"),
            "markdown output should contain Personal: {out}"
        );
    }
}
