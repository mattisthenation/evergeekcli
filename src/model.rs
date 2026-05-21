use serde::{Deserialize, Serialize};

/// Milliseconds since Unix epoch (Evernote's timestamp unit).
pub type Timestamp = i64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteSummary {
    pub guid: String,
    pub title: String,
    pub notebook_guid: Option<String>,
    pub tag_guids: Vec<String>,
    pub created: Option<Timestamp>,
    pub updated: Option<Timestamp>,
    /// Server-provided search snippet, when available.
    pub snippet: Option<String>,
    /// Set by the `yesterday` command; None elsewhere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_yesterday: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edited_yesterday: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteContent {
    pub summary: NoteSummary,
    /// Plain-text rendering of the ENML body.
    pub text: String,
    /// Markdown rendering of the ENML body.
    pub markdown: String,
    /// Raw ENML, included for fidelity/debugging.
    pub enml: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notebook {
    pub guid: String,
    pub name: String,
    pub stack: Option<String>,
}

/// Built by commands, rendered to an Evernote search-grammar string by the client.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NoteQuery {
    /// Raw Evernote search grammar (may be empty).
    pub words: String,
    pub notebook_guid: Option<String>,
    pub limit: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_summary() -> NoteSummary {
        NoteSummary {
            guid: "guid-123".to_string(),
            title: "A test note".to_string(),
            notebook_guid: Some("nb-1".to_string()),
            tag_guids: vec!["tag-a".to_string(), "tag-b".to_string()],
            created: Some(1_700_000_000_000),
            updated: Some(1_700_000_500_000),
            snippet: Some("a snippet".to_string()),
            created_yesterday: None,
            edited_yesterday: None,
        }
    }

    #[test]
    fn note_summary_round_trips_through_serde_json() {
        let original = sample_summary();
        let json = serde_json::to_string(&original).expect("serialize");
        let back: NoteSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, back);
    }

    #[test]
    fn created_yesterday_none_is_omitted_from_json() {
        let summary = sample_summary();
        let json = serde_json::to_string(&summary).expect("serialize");
        assert!(
            !json.contains("created_yesterday"),
            "created_yesterday should be omitted when None, got: {json}"
        );
        assert!(
            !json.contains("edited_yesterday"),
            "edited_yesterday should be omitted when None, got: {json}"
        );
    }

    #[test]
    fn yesterday_flags_are_serialized_when_set() {
        let mut summary = sample_summary();
        summary.created_yesterday = Some(true);
        summary.edited_yesterday = Some(false);
        let json = serde_json::to_string(&summary).expect("serialize");
        assert!(json.contains("created_yesterday"));
        assert!(json.contains("edited_yesterday"));
        let back: NoteSummary = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(summary, back);
    }
}
