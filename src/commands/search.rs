//! `search` / `recent` / `yesterday` command group.
//!
//! Implemented in Task 8. Signatures are frozen per the FROZEN COMMAND
//! CONTRACT so parallel agents can rely on them.

use crate::error::{EvergeekError, Result};
use crate::evernote::EvernoteClient;
use crate::model::{NoteContent, NoteQuery, NoteSummary};
use crate::output::{self, Format};

pub fn search(
    client: &dyn EvernoteClient,
    words: &str,
    notebook: Option<&str>,
    since: Option<&str>,
    limit: i32,
    fmt: Format,
) -> Result<String> {
    let query_words = build_search_words(words, since)?;
    let notebook_guid = resolve_notebook(client, notebook)?;
    let query = NoteQuery {
        words: query_words,
        notebook_guid,
        limit,
    };
    let results = client.search_notes(&query)?;
    Ok(output::render_summaries(&results, fmt))
}

pub fn recent(client: &dyn EvernoteClient, since: &str, limit: i32, fmt: Format) -> Result<String> {
    let query = NoteQuery {
        words: build_recent_words(since)?,
        notebook_guid: None,
        limit,
    };
    let results = client.search_notes(&query)?;
    Ok(output::render_summaries(&results, fmt))
}

pub fn yesterday(
    client: &dyn EvernoteClient,
    notebook: Option<&str>,
    limit: i32,
    full: bool,
    fmt: Format,
) -> Result<String> {
    let notebook_guid = resolve_notebook(client, notebook)?;

    // Two passes: notes created yesterday and notes updated yesterday. Use
    // limit 0 on each so nothing is dropped before the merge.
    let created = client.search_notes(&NoteQuery {
        words: "created:day-1 -created:day".to_string(),
        notebook_guid: notebook_guid.clone(),
        limit: 0,
    })?;
    let updated = client.search_notes(&NoteQuery {
        words: "updated:day-1 -updated:day".to_string(),
        notebook_guid,
        limit: 0,
    })?;

    let mut merged = merge_yesterday(created, updated);
    if limit > 0 {
        merged.truncate(limit as usize);
    }

    if full {
        let notes: Vec<NoteContent> = merged
            .iter()
            .map(|s| client.get_note(&s.guid))
            .collect::<Result<_>>()?;
        Ok(render_notes(&notes, fmt))
    } else {
        Ok(output::render_summaries(&merged, fmt))
    }
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Resolve a notebook *name* to its guid via `list_notebooks`, matching the
/// name case-insensitively. `None` input yields `None` (no filter). An
/// unmatched name is a [`EvergeekError::NotFound`].
fn resolve_notebook(client: &dyn EvernoteClient, notebook: Option<&str>) -> Result<Option<String>> {
    match notebook {
        None => Ok(None),
        Some(name) => {
            let wanted = name.to_lowercase();
            client
                .list_notebooks()?
                .into_iter()
                .find(|nb| nb.name.to_lowercase() == wanted)
                .map(|nb| Some(nb.guid))
                .ok_or_else(|| EvergeekError::NotFound(format!("notebook: {name}")))
        }
    }
}

/// Build the Evernote search-grammar `words` string for `search`.
///
/// `since` may be `"<N>d"` (relative days → `updated:day-N`) or `"YYYY-MM-DD"`
/// (absolute → `updated:YYYYMMDD`). When present, the date clause is prepended
/// to `words`. A missing/empty `words` produces just the date clause (no
/// trailing space). An invalid `since` is an [`EvergeekError::Config`].
fn build_search_words(words: &str, since: Option<&str>) -> Result<String> {
    match since {
        None => Ok(words.to_string()),
        Some(s) => {
            let clause = since_clause(s)?;
            if words.is_empty() {
                Ok(clause)
            } else {
                Ok(format!("{clause} {words}"))
            }
        }
    }
}

/// Build the `words` string for `recent`: a `"<N>d"` relative-day spec maps to
/// `updated:day-N`. Anything else is an [`EvergeekError::Config`].
fn build_recent_words(since: &str) -> Result<String> {
    let n = parse_relative_days(since)
        .ok_or_else(|| EvergeekError::Config(format!("invalid --since value: {since}")))?;
    Ok(format!("updated:day-{n}"))
}

/// Render the `updated:` clause for a `since` value, accepting either a
/// relative `"<N>d"` spec or an absolute `"YYYY-MM-DD"` date.
fn since_clause(since: &str) -> Result<String> {
    if let Some(n) = parse_relative_days(since) {
        return Ok(format!("updated:day-{n}"));
    }
    if let Some(compact) = parse_iso_date(since) {
        return Ok(format!("updated:{compact}"));
    }
    Err(EvergeekError::Config(format!(
        "invalid --since value: {since}"
    )))
}

/// Parse a `"<N>d"` relative-day spec (e.g. `"7d"`) into `N`. Returns `None`
/// if the string is not exactly digits followed by a trailing `d`.
fn parse_relative_days(s: &str) -> Option<u32> {
    let digits = s.strip_suffix('d')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()
}

/// Parse a `"YYYY-MM-DD"` date into Evernote's compact `"YYYYMMDD"` form.
/// Validates the shape and digit/range constraints; returns `None` otherwise.
fn parse_iso_date(s: &str) -> Option<String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    if y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    if !s.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        return None;
    }
    let month: u32 = m.parse().ok()?;
    let day: u32 = d.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{y}{m}{d}"))
}

/// Merge the `created`-yesterday and `updated`-yesterday result sets into a
/// single deduped list keyed by `guid`.
///
/// A note present in the `created` set gets `created_yesterday = Some(true)`;
/// one in the `updated` set gets `edited_yesterday = Some(true)`; a note in
/// both gets both flags set. The result is sorted by `updated` descending,
/// with missing `updated` timestamps sorting last.
fn merge_yesterday(created: Vec<NoteSummary>, updated: Vec<NoteSummary>) -> Vec<NoteSummary> {
    // Insertion-ordered accumulation by guid so the pre-sort order is stable.
    let mut order: Vec<String> = Vec::new();
    let mut by_guid: std::collections::HashMap<String, NoteSummary> =
        std::collections::HashMap::new();

    for mut s in created {
        s.created_yesterday = Some(true);
        match by_guid.get_mut(&s.guid) {
            Some(existing) => existing.created_yesterday = Some(true),
            None => {
                order.push(s.guid.clone());
                by_guid.insert(s.guid.clone(), s);
            }
        }
    }
    for mut s in updated {
        s.edited_yesterday = Some(true);
        match by_guid.get_mut(&s.guid) {
            Some(existing) => existing.edited_yesterday = Some(true),
            None => {
                order.push(s.guid.clone());
                by_guid.insert(s.guid.clone(), s);
            }
        }
    }

    let mut merged: Vec<NoteSummary> = order
        .into_iter()
        .map(|g| by_guid.remove(&g).expect("guid present"))
        .collect();

    // Sort by `updated` descending; None sorts last. Use a stable sort so ties
    // keep insertion order.
    merged.sort_by(|a, b| match (a.updated, b.updated) {
        (Some(x), Some(y)) => y.cmp(&x),                // descending
        (Some(_), None) => std::cmp::Ordering::Less,    // a has a value → before None
        (None, Some(_)) => std::cmp::Ordering::Greater, // None → after a value
        (None, None) => std::cmp::Ordering::Equal,
    });

    merged
}

/// Render a list of full notes. For `Json`, emit a JSON array of `NoteContent`
/// via serde; for text/markdown, join the per-note `render_note` outputs.
fn render_notes(notes: &[NoteContent], fmt: Format) -> String {
    match fmt {
        Format::Json => serde_json::to_string_pretty(notes)
            .unwrap_or_else(|e| format!("{{\n  \"error\": \"failed to serialize: {e}\"\n}}")),
        Format::Text | Format::Markdown => notes
            .iter()
            .map(|n| output::render_note(n, fmt))
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evernote::mock::{self, MockClient};
    use crate::model::{NoteContent, NoteSummary, Notebook};

    // ---- build_search_words ----

    #[test]
    fn build_search_words_no_since_returns_words_verbatim() {
        assert_eq!(
            build_search_words("kubernetes", None).unwrap(),
            "kubernetes"
        );
        assert_eq!(build_search_words("", None).unwrap(), "");
    }

    #[test]
    fn build_search_words_relative_days_prepends_updated_clause() {
        assert_eq!(
            build_search_words("roadmap", Some("1d")).unwrap(),
            "updated:day-1 roadmap"
        );
        assert_eq!(
            build_search_words("roadmap", Some("7d")).unwrap(),
            "updated:day-7 roadmap"
        );
    }

    #[test]
    fn build_search_words_absolute_date_prepends_compact_clause() {
        assert_eq!(
            build_search_words("notes", Some("2024-01-15")).unwrap(),
            "updated:20240115 notes"
        );
    }

    #[test]
    fn build_search_words_empty_words_trims_trailing_space() {
        assert_eq!(build_search_words("", Some("1d")).unwrap(), "updated:day-1");
        assert_eq!(
            build_search_words("", Some("2024-01-15")).unwrap(),
            "updated:20240115"
        );
    }

    #[test]
    fn build_search_words_invalid_since_is_config_error() {
        for bad in [
            "d",
            "xd",
            "1w",
            "abc",
            "2024-1-1",
            "2024-13-01",
            "2024-01-32",
            "20240101",
        ] {
            let err = build_search_words("x", Some(bad)).expect_err(bad);
            assert!(
                matches!(err, EvergeekError::Config(_)),
                "expected Config for {bad:?}, got {err:?}"
            );
        }
    }

    // ---- build_recent_words ----

    #[test]
    fn build_recent_words_maps_relative_days() {
        assert_eq!(build_recent_words("1d").unwrap(), "updated:day-1");
        assert_eq!(build_recent_words("7d").unwrap(), "updated:day-7");
        assert_eq!(build_recent_words("30d").unwrap(), "updated:day-30");
    }

    #[test]
    fn build_recent_words_invalid_is_config_error() {
        for bad in ["", "d", "7", "7days", "2024-01-15", "abc"] {
            let err = build_recent_words(bad).expect_err(bad);
            assert!(
                matches!(err, EvergeekError::Config(_)),
                "expected Config for {bad:?}, got {err:?}"
            );
        }
    }

    // ---- merge_yesterday ----

    fn summ(guid: &str, updated: Option<i64>) -> NoteSummary {
        NoteSummary {
            guid: guid.to_string(),
            title: format!("Title {guid}"),
            notebook_guid: None,
            tag_guids: vec![],
            created: None,
            updated,
            snippet: None,
            created_yesterday: None,
            edited_yesterday: None,
        }
    }

    #[test]
    fn merge_yesterday_sets_flags_dedups_and_sorts() {
        // a: created only; b: both; c: updated only.
        let created = vec![summ("a", Some(100)), summ("b", Some(300))];
        let updated = vec![summ("b", Some(300)), summ("c", Some(200))];

        let merged = merge_yesterday(created, updated);

        // Deduped: a, b, c (b appears once).
        assert_eq!(merged.len(), 3);

        let by_guid = |g: &str| merged.iter().find(|s| s.guid == g).unwrap().clone();

        let a = by_guid("a");
        assert_eq!(a.created_yesterday, Some(true));
        assert_eq!(a.edited_yesterday, None);

        let b = by_guid("b");
        assert_eq!(b.created_yesterday, Some(true));
        assert_eq!(b.edited_yesterday, Some(true));

        let c = by_guid("c");
        assert_eq!(c.created_yesterday, None);
        assert_eq!(c.edited_yesterday, Some(true));

        // Sorted by updated descending: b(300), c(200), a(100).
        let order: Vec<&str> = merged.iter().map(|s| s.guid.as_str()).collect();
        assert_eq!(order, vec!["b", "c", "a"]);
    }

    #[test]
    fn merge_yesterday_none_updated_sorts_last() {
        let created = vec![summ("x", None), summ("y", Some(50))];
        let updated = vec![];
        let merged = merge_yesterday(created, updated);
        let order: Vec<&str> = merged.iter().map(|s| s.guid.as_str()).collect();
        assert_eq!(order, vec!["y", "x"]);
    }

    #[test]
    fn merge_yesterday_empty_inputs_yield_empty() {
        assert!(merge_yesterday(vec![], vec![]).is_empty());
    }

    // ---- search against the mock ----

    #[test]
    fn search_notebook_filters_to_that_notebook() {
        let client = mock::sample();
        let out = search(&client, "", Some("Work"), None, 0, Format::Json).unwrap();
        let notes: Vec<NoteSummary> = serde_json::from_str(&out).unwrap();
        // Only the two notes in the Work notebook.
        assert_eq!(notes.len(), 2);
        assert!(notes
            .iter()
            .all(|s| s.notebook_guid.as_deref() == Some("nb-work")));
    }

    #[test]
    fn search_notebook_name_match_is_case_insensitive() {
        let client = mock::sample();
        let out = search(&client, "", Some("wOrK"), None, 0, Format::Json).unwrap();
        let notes: Vec<NoteSummary> = serde_json::from_str(&out).unwrap();
        assert_eq!(notes.len(), 2);
    }

    #[test]
    fn search_unknown_notebook_is_not_found() {
        let client = mock::sample();
        let err = search(&client, "", Some("Nonexistent"), None, 0, Format::Json)
            .expect_err("should error");
        assert!(matches!(err, EvergeekError::NotFound(_)), "got {err:?}");
    }

    #[test]
    fn search_word_query_filters() {
        let client = mock::sample();
        let out = search(&client, "kubernetes", None, None, 0, Format::Json).unwrap();
        let notes: Vec<NoteSummary> = serde_json::from_str(&out).unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].guid, "note-001");
    }

    #[test]
    fn search_json_output_parses() {
        let client = mock::sample();
        let out = search(&client, "", None, None, 0, Format::Json).unwrap();
        let notes: Vec<NoteSummary> = serde_json::from_str(&out).expect("valid json");
        assert_eq!(notes.len(), client.notes.len());
    }

    #[test]
    fn search_respects_limit() {
        let client = mock::sample();
        let out = search(&client, "", None, None, 1, Format::Json).unwrap();
        let notes: Vec<NoteSummary> = serde_json::from_str(&out).unwrap();
        assert_eq!(notes.len(), 1);
    }

    // ---- recent / yesterday smoke ----

    #[test]
    fn recent_returns_ok_and_valid_json() {
        let client = mock::sample();
        let out = recent(&client, "7d", 0, Format::Json).unwrap();
        // The mock matches by substring, so the day-grammar query matches
        // nothing; we only assert the output is valid JSON.
        let notes: Vec<NoteSummary> = serde_json::from_str(&out).expect("valid json");
        assert!(notes.is_empty());
    }

    #[test]
    fn recent_invalid_since_errors() {
        let client = mock::sample();
        let err = recent(&client, "nope", 0, Format::Json).expect_err("should error");
        assert!(matches!(err, EvergeekError::Config(_)), "got {err:?}");
    }

    #[test]
    fn yesterday_smoke_returns_ok_and_valid_json() {
        let client = mock::sample();
        let out = yesterday(&client, None, 0, false, Format::Json).unwrap();
        let notes: Vec<NoteSummary> = serde_json::from_str(&out).expect("valid json");
        // day-grammar queries match nothing in the substring mock.
        assert!(notes.is_empty());
    }

    #[test]
    fn yesterday_unknown_notebook_is_not_found() {
        let client = mock::sample();
        let err = yesterday(&client, Some("Nonexistent"), 0, false, Format::Json)
            .expect_err("should error");
        assert!(matches!(err, EvergeekError::NotFound(_)), "got {err:?}");
    }

    #[test]
    fn yesterday_text_output_does_not_panic() {
        let client = mock::sample();
        let out = yesterday(&client, None, 0, false, Format::Text).unwrap();
        // Empty result set renders the "no notes" placeholder.
        assert!(!out.is_empty());
    }

    // A mock whose search returns predetermined sets, to exercise yesterday's
    // merge + full rendering paths end-to-end (the substring mock can't hit
    // the day-grammar queries).
    struct ScriptedClient {
        created: Vec<NoteSummary>,
        updated: Vec<NoteSummary>,
        notes: Vec<NoteContent>,
        notebooks: Vec<Notebook>,
    }

    impl EvernoteClient for ScriptedClient {
        fn search_notes(&self, query: &NoteQuery) -> Result<Vec<NoteSummary>> {
            if query.words.starts_with("created:") {
                Ok(self.created.clone())
            } else if query.words.starts_with("updated:") {
                Ok(self.updated.clone())
            } else {
                Ok(vec![])
            }
        }
        fn get_note(&self, guid: &str) -> Result<NoteContent> {
            self.notes
                .iter()
                .find(|n| n.summary.guid == guid)
                .cloned()
                .ok_or_else(|| EvergeekError::NotFound(guid.into()))
        }
        fn list_notebooks(&self) -> Result<Vec<Notebook>> {
            Ok(self.notebooks.clone())
        }
    }

    fn note_for(guid: &str, updated: Option<i64>) -> NoteContent {
        NoteContent {
            summary: summ(guid, updated),
            text: format!("text for {guid}"),
            markdown: format!("# md for {guid}"),
            enml: format!("<en-note>{guid}</en-note>"),
        }
    }

    #[test]
    fn yesterday_merges_scripted_sets_and_applies_limit() {
        let client = ScriptedClient {
            created: vec![summ("a", Some(100)), summ("b", Some(300))],
            updated: vec![summ("b", Some(300)), summ("c", Some(200))],
            notes: vec![],
            notebooks: vec![],
        };
        // limit 2 keeps top-2 by updated desc: b(300), c(200).
        let out = yesterday(&client, None, 2, false, Format::Json).unwrap();
        let notes: Vec<NoteSummary> = serde_json::from_str(&out).unwrap();
        let order: Vec<&str> = notes.iter().map(|s| s.guid.as_str()).collect();
        assert_eq!(order, vec!["b", "c"]);
        // Flags carried through serialization.
        assert_eq!(notes[0].created_yesterday, Some(true));
        assert_eq!(notes[0].edited_yesterday, Some(true));
    }

    #[test]
    fn yesterday_full_json_is_array_of_note_content() {
        let client = ScriptedClient {
            created: vec![summ("a", Some(100))],
            updated: vec![summ("b", Some(300))],
            notes: vec![note_for("a", Some(100)), note_for("b", Some(300))],
            notebooks: vec![],
        };
        let out = yesterday(&client, None, 0, true, Format::Json).unwrap();
        let contents: Vec<NoteContent> = serde_json::from_str(&out).expect("array of NoteContent");
        // Sorted by updated desc: b then a.
        let order: Vec<&str> = contents.iter().map(|c| c.summary.guid.as_str()).collect();
        assert_eq!(order, vec!["b", "a"]);
        assert_eq!(contents[0].text, "text for b");
    }

    #[test]
    fn yesterday_full_text_joins_render_note() {
        let client = ScriptedClient {
            created: vec![summ("a", Some(100))],
            updated: vec![],
            notes: vec![note_for("a", Some(100))],
            notebooks: vec![],
        };
        let out = yesterday(&client, None, 0, true, Format::Text).unwrap();
        assert!(out.contains("text for a"), "got {out}");
    }

    // Ensure the imports for MockClient type are exercised so clippy stays quiet.
    #[test]
    fn mock_client_type_is_usable() {
        let _c: MockClient = mock::sample();
    }
}
