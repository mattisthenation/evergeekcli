//! Fixture-backed [`EvernoteClient`] for tests.
//!
//! Implemented in Task 6.

use crate::error::{EvergeekError, Result};
use crate::evernote::EvernoteClient;
use crate::model::{NoteContent, NoteQuery, NoteSummary, Notebook};

/// A fixture-backed [`EvernoteClient`] for use in tests.
///
/// All data lives in-memory; no network or persistence is involved. Searching
/// is a simple case-insensitive substring match over note titles and bodies.
pub struct MockClient {
    pub notes: Vec<NoteContent>,
    pub notebooks: Vec<Notebook>,
}

impl EvernoteClient for MockClient {
    /// Returns the summary of every note whose title or plain-text body
    /// contains `query.words` (case-insensitive substring). An empty `words`
    /// matches every note. When `query.notebook_guid` is set, only notes in that
    /// notebook are returned. When `query.limit > 0`, the result is truncated to
    /// at most that many summaries. Results preserve `notes` order.
    fn search_notes(&self, query: &NoteQuery) -> Result<Vec<NoteSummary>> {
        let needle = query.words.to_lowercase();
        let mut matches: Vec<NoteSummary> = self
            .notes
            .iter()
            .filter(|note| {
                let words_match = needle.is_empty()
                    || note.summary.title.to_lowercase().contains(&needle)
                    || note.text.to_lowercase().contains(&needle);
                let notebook_match = query
                    .notebook_guid
                    .as_deref()
                    .is_none_or(|wanted| note.summary.notebook_guid.as_deref() == Some(wanted));
                words_match && notebook_match
            })
            .map(|note| note.summary.clone())
            .collect();

        if query.limit > 0 {
            matches.truncate(query.limit as usize);
        }

        Ok(matches)
    }

    /// Finds a note by `summary.guid`, returning [`EvergeekError::NotFound`]
    /// (carrying the requested guid) when no note matches.
    fn get_note(&self, guid: &str) -> Result<NoteContent> {
        self.notes
            .iter()
            .find(|note| note.summary.guid == guid)
            .cloned()
            .ok_or_else(|| EvergeekError::NotFound(guid.into()))
    }

    /// Returns a clone of the configured notebooks.
    fn list_notebooks(&self) -> Result<Vec<Notebook>> {
        Ok(self.notebooks.clone())
    }
}

/// Builds a deterministic [`MockClient`] with stable fixture data for tests.
///
/// Contains four notes with distinct guids, varied titles/bodies, and assorted
/// created/updated timestamps, plus two notebooks (one stacked). The data is
/// intentionally stable so downstream command tests can assert against it.
pub fn sample() -> MockClient {
    let work = "nb-work";
    let personal = "nb-personal";

    // Fixed millisecond timestamps (UTC) so tests are reproducible; see the
    // per-field comments below for the corresponding ISO-8601 instants.
    let notes = vec![
        NoteContent {
            summary: NoteSummary {
                guid: "note-001".to_string(),
                title: "Weekly Planning".to_string(),
                notebook_guid: Some(work.to_string()),
                tag_guids: vec!["tag-planning".to_string()],
                created: Some(1_705_309_200_000), // 2024-01-15T09:00:00Z
                updated: Some(1_705_312_800_000), // 2024-01-15T10:00:00Z
                snippet: Some("Goals for the week".to_string()),
                created_yesterday: None,
                edited_yesterday: None,
            },
            text: "Outline goals for the week and review the kubernetes migration.".to_string(),
            markdown: "# Weekly Planning\n\nOutline goals for the week and review the kubernetes migration.".to_string(),
            enml: "<en-note><h1>Weekly Planning</h1><p>Outline goals for the week and review the kubernetes migration.</p></en-note>".to_string(),
        },
        NoteContent {
            summary: NoteSummary {
                guid: "note-002".to_string(),
                title: "Groceries".to_string(),
                notebook_guid: Some(personal.to_string()),
                tag_guids: vec![],
                created: Some(1_705_402_800_000), // 2024-01-16T11:00:00Z
                updated: Some(1_705_406_400_000), // 2024-01-16T12:00:00Z
                snippet: None,
                created_yesterday: None,
                edited_yesterday: None,
            },
            text: "Milk, eggs, bread, coffee.".to_string(),
            markdown: "# Groceries\n\n- Milk\n- eggs\n- bread\n- coffee".to_string(),
            enml: "<en-note><h1>Groceries</h1><ul><li>Milk</li><li>eggs</li><li>bread</li><li>coffee</li></ul></en-note>".to_string(),
        },
        NoteContent {
            summary: NoteSummary {
                guid: "note-003".to_string(),
                title: "Meeting Notes: Q1 Roadmap".to_string(),
                notebook_guid: Some(work.to_string()),
                tag_guids: vec!["tag-planning".to_string(), "tag-meeting".to_string()],
                created: Some(1_705_489_200_000), // 2024-01-17T11:00:00Z
                updated: Some(1_705_500_000_000), // 2024-01-17T14:00:00Z
                snippet: Some("Roadmap discussion".to_string()),
                created_yesterday: None,
                edited_yesterday: None,
            },
            text: "Discussed the Q1 roadmap and assigned owners for each initiative.".to_string(),
            markdown: "# Meeting Notes: Q1 Roadmap\n\nDiscussed the Q1 roadmap and assigned owners for each initiative.".to_string(),
            enml: "<en-note><h1>Meeting Notes: Q1 Roadmap</h1><p>Discussed the Q1 roadmap and assigned owners for each initiative.</p></en-note>".to_string(),
        },
        NoteContent {
            summary: NoteSummary {
                guid: "note-004".to_string(),
                title: "Book Ideas".to_string(),
                notebook_guid: None,
                tag_guids: vec![],
                created: None,
                updated: None,
                snippet: None,
                created_yesterday: None,
                edited_yesterday: None,
            },
            text: "A collection of half-formed ideas for a future novel.".to_string(),
            markdown: "# Book Ideas\n\nA collection of half-formed ideas for a future novel.".to_string(),
            enml: "<en-note><h1>Book Ideas</h1><p>A collection of half-formed ideas for a future novel.</p></en-note>".to_string(),
        },
    ];

    let notebooks = vec![
        Notebook {
            guid: work.to_string(),
            name: "Work".to_string(),
            stack: Some("Professional".to_string()),
        },
        Notebook {
            guid: personal.to_string(),
            name: "Personal".to_string(),
            stack: None,
        },
    ];

    MockClient { notes, notebooks }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::EvergeekError;

    fn query(words: &str, limit: i32) -> NoteQuery {
        NoteQuery {
            words: words.to_string(),
            notebook_guid: None,
            limit,
        }
    }

    fn titles(summaries: &[NoteSummary]) -> Vec<&str> {
        summaries.iter().map(|s| s.title.as_str()).collect()
    }

    #[test]
    fn sample_has_at_least_three_notes_and_two_notebooks() {
        let client = sample();
        assert!(
            client.notes.len() >= 3,
            "expected >=3 notes, got {}",
            client.notes.len()
        );
        assert!(
            client.notebooks.len() >= 2,
            "expected >=2 notebooks, got {}",
            client.notebooks.len()
        );
    }

    #[test]
    fn sample_has_distinct_note_guids() {
        let client = sample();
        let mut guids: Vec<&str> = client
            .notes
            .iter()
            .map(|n| n.summary.guid.as_str())
            .collect();
        let count = guids.len();
        guids.sort_unstable();
        guids.dedup();
        assert_eq!(guids.len(), count, "note guids must be distinct");
    }

    #[test]
    fn sample_is_deterministic() {
        assert_eq!(sample().notes, sample().notes);
        assert_eq!(sample().notebooks, sample().notebooks);
    }

    #[test]
    fn search_empty_words_returns_all_summaries() {
        let client = sample();
        let results = client.search_notes(&query("", 0)).expect("search");
        assert_eq!(results.len(), client.notes.len());
    }

    #[test]
    fn search_matches_title_substring_case_insensitive() {
        let client = sample();
        // "groceries" appears in a title with different casing in the fixture.
        let results = client.search_notes(&query("GROCERIES", 0)).expect("search");
        assert_eq!(results.len(), 1);
        assert!(results[0].title.to_lowercase().contains("groceries"));
    }

    #[test]
    fn search_matches_text_body_substring() {
        let client = sample();
        // "kubernetes" only appears in a note body, not any title.
        let results = client
            .search_notes(&query("kubernetes", 0))
            .expect("search");
        assert_eq!(results.len(), 1);
        assert!(!results[0].title.to_lowercase().contains("kubernetes"));
    }

    #[test]
    fn search_no_match_returns_empty() {
        let client = sample();
        let results = client
            .search_notes(&query("zzz-nonexistent-zzz", 0))
            .expect("search");
        assert!(results.is_empty());
    }

    #[test]
    fn search_respects_positive_limit() {
        let client = sample();
        let results = client.search_notes(&query("", 2)).expect("search");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_zero_or_negative_limit_does_not_truncate() {
        let client = sample();
        let zero = client.search_notes(&query("", 0)).expect("search");
        let neg = client.search_notes(&query("", -1)).expect("search");
        assert_eq!(zero.len(), client.notes.len());
        assert_eq!(neg.len(), client.notes.len());
    }

    #[test]
    fn search_limit_larger_than_results_returns_all() {
        let client = sample();
        let results = client.search_notes(&query("", 1000)).expect("search");
        assert_eq!(results.len(), client.notes.len());
    }

    #[test]
    fn search_returns_summaries_in_note_order() {
        let client = sample();
        let results = client.search_notes(&query("", 0)).expect("search");
        let expected: Vec<&str> = client
            .notes
            .iter()
            .map(|n| n.summary.title.as_str())
            .collect();
        assert_eq!(titles(&results), expected);
    }

    #[test]
    fn search_filters_by_notebook_guid() {
        let client = sample();
        let q = NoteQuery {
            words: String::new(),
            notebook_guid: Some("nb-work".to_string()),
            limit: 0,
        };
        let results = client.search_notes(&q).expect("search");
        // Only the two notes in the Work notebook (note-001, note-003).
        assert_eq!(results.len(), 2);
        assert!(results
            .iter()
            .all(|s| s.notebook_guid.as_deref() == Some("nb-work")));
    }

    #[test]
    fn get_note_returns_matching_note() {
        let client = sample();
        let target = &client.notes[1];
        let got = client.get_note(&target.summary.guid).expect("get_note");
        assert_eq!(&got, target);
    }

    #[test]
    fn get_note_missing_guid_is_not_found() {
        let client = sample();
        let err = client.get_note("does-not-exist").expect_err("should error");
        match err {
            EvergeekError::NotFound(g) => assert_eq!(g, "does-not-exist"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn list_notebooks_returns_clone_of_notebooks() {
        let client = sample();
        let notebooks = client.list_notebooks().expect("list_notebooks");
        assert_eq!(notebooks, client.notebooks);
    }
}
