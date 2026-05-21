use crate::model::{NoteContent, NoteSummary, Notebook, Timestamp};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum Format {
    Json,
    Text,
    Markdown,
}

/// Format an Evernote millisecond-epoch timestamp as a readable UTC datetime.
///
/// Returns `"—"` for a missing timestamp and `"(invalid timestamp)"` for one
/// that falls outside the representable range.
fn format_ts(ts: Option<Timestamp>) -> String {
    match ts {
        None => "—".to_string(),
        Some(ms) => match DateTime::<Utc>::from_timestamp_millis(ms) {
            Some(dt) => dt.format("%Y-%m-%d %H:%M UTC").to_string(),
            None => "(invalid timestamp)".to_string(),
        },
    }
}

/// Serialize any serde value to pretty JSON, falling back to an error object on
/// the (practically impossible) chance serialization fails.
fn to_pretty_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value)
        .unwrap_or_else(|e| format!("{{\n  \"error\": \"failed to serialize: {e}\"\n}}"))
}

/// Render a list of note summaries in the requested format.
pub fn render_summaries(items: &[NoteSummary], fmt: Format) -> String {
    match fmt {
        Format::Json => to_pretty_json(&items),
        Format::Text => summaries_text(items),
        Format::Markdown => summaries_markdown(items),
    }
}

/// Render a single note in the requested format.
pub fn render_note(note: &NoteContent, fmt: Format) -> String {
    match fmt {
        Format::Json => to_pretty_json(note),
        Format::Text => note_text(note),
        Format::Markdown => note_markdown(note),
    }
}

/// Render a list of notebooks in the requested format.
pub fn render_notebooks(items: &[Notebook], fmt: Format) -> String {
    match fmt {
        Format::Json => to_pretty_json(&items),
        Format::Text => notebooks_text(items),
        Format::Markdown => notebooks_markdown(items),
    }
}

// ---------------------------------------------------------------------------
// Text rendering
// ---------------------------------------------------------------------------

fn summaries_text(items: &[NoteSummary]) -> String {
    if items.is_empty() {
        return "(no notes)".to_string();
    }
    let mut blocks = Vec::with_capacity(items.len());
    for s in items {
        let mut lines = vec![
            s.title.clone(),
            format!("  guid:     {}", s.guid),
            format!("  created:  {}", format_ts(s.created)),
            format!("  updated:  {}", format_ts(s.updated)),
            format!("  notebook: {}", s.notebook_guid.as_deref().unwrap_or("—")),
        ];
        if let Some(snippet) = &s.snippet {
            lines.push(format!("  snippet:  {snippet}"));
        }
        blocks.push(lines.join("\n"));
    }
    blocks.join("\n\n")
}

fn note_text(note: &NoteContent) -> String {
    let s = &note.summary;
    let mut lines = vec![
        s.title.clone(),
        format!("guid:     {}", s.guid),
        format!("created:  {}", format_ts(s.created)),
        format!("updated:  {}", format_ts(s.updated)),
        format!("notebook: {}", s.notebook_guid.as_deref().unwrap_or("—")),
    ];
    if !s.tag_guids.is_empty() {
        lines.push(format!("tags:     {}", s.tag_guids.join(", ")));
    }
    lines.push(String::new());
    lines.push(note.text.clone());
    lines.join("\n")
}

fn notebooks_text(items: &[Notebook]) -> String {
    if items.is_empty() {
        return "(no notebooks)".to_string();
    }
    items
        .iter()
        .map(|nb| match &nb.stack {
            Some(stack) => format!("{} [{}]\n  guid: {}", nb.name, stack, nb.guid),
            None => format!("{}\n  guid: {}", nb.name, nb.guid),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

// ---------------------------------------------------------------------------
// Markdown rendering
// ---------------------------------------------------------------------------

fn summaries_markdown(items: &[NoteSummary]) -> String {
    if items.is_empty() {
        return "_No notes._".to_string();
    }
    let mut out = Vec::with_capacity(items.len());
    for s in items {
        let mut item = format!("- **{}** (`{}`)", s.title, s.guid);
        item.push_str(&format!(
            "\n  - created: {} · updated: {}",
            format_ts(s.created),
            format_ts(s.updated)
        ));
        if let Some(nb) = &s.notebook_guid {
            item.push_str(&format!("\n  - notebook: `{nb}`"));
        }
        if let Some(snippet) = &s.snippet {
            item.push_str(&format!("\n  - {snippet}"));
        }
        out.push(item);
    }
    out.join("\n")
}

fn note_markdown(note: &NoteContent) -> String {
    let s = &note.summary;
    let mut lines = vec![format!("# {}", s.title), String::new()];
    let mut meta = vec![
        format!("- guid: `{}`", s.guid),
        format!("- created: {}", format_ts(s.created)),
        format!("- updated: {}", format_ts(s.updated)),
    ];
    if let Some(nb) = &s.notebook_guid {
        meta.push(format!("- notebook: `{nb}`"));
    }
    if !s.tag_guids.is_empty() {
        meta.push(format!("- tags: {}", s.tag_guids.join(", ")));
    }
    lines.extend(meta);
    lines.push(String::new());
    lines.push(note.markdown.clone());
    lines.join("\n")
}

fn notebooks_markdown(items: &[Notebook]) -> String {
    if items.is_empty() {
        return "_No notebooks._".to_string();
    }
    items
        .iter()
        .map(|nb| match &nb.stack {
            Some(stack) => format!("- **{}** _(stack: {})_ — `{}`", nb.name, stack, nb.guid),
            None => format!("- **{}** — `{}`", nb.name, nb.guid),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NoteContent, NoteSummary, Notebook};

    // Fixed timestamps (ms since epoch), UTC:
    //   created: 2023-11-14T22:13:20Z
    //   updated: 2023-11-14T22:21:40Z
    const CREATED_MS: i64 = 1_700_000_000_000;
    const UPDATED_MS: i64 = 1_700_000_500_000;

    fn sample_summary() -> NoteSummary {
        NoteSummary {
            guid: "guid-123".to_string(),
            title: "A test note".to_string(),
            notebook_guid: Some("nb-1".to_string()),
            tag_guids: vec!["tag-a".to_string(), "tag-b".to_string()],
            created: Some(CREATED_MS),
            updated: Some(UPDATED_MS),
            snippet: Some("a short snippet".to_string()),
            created_yesterday: None,
            edited_yesterday: None,
        }
    }

    fn sample_note() -> NoteContent {
        NoteContent {
            summary: sample_summary(),
            text: "Plain text body.\nSecond line.".to_string(),
            markdown: "# Heading\n\nMarkdown **body**.".to_string(),
            enml: "<en-note><div>Plain text body.</div></en-note>".to_string(),
        }
    }

    fn sample_notebooks() -> Vec<Notebook> {
        vec![
            Notebook {
                guid: "nb-1".to_string(),
                name: "Inbox".to_string(),
                stack: None,
            },
            Notebook {
                guid: "nb-2".to_string(),
                name: "Projects".to_string(),
                stack: Some("Work".to_string()),
            },
        ]
    }

    // ---- JSON ----

    #[test]
    fn summaries_json_is_pretty_and_round_trips() {
        let items = vec![sample_summary()];
        let out = render_summaries(&items, Format::Json);
        // pretty printing introduces indentation/newlines
        assert!(out.contains('\n'));
        assert!(out.contains("  "));
        let back: Vec<NoteSummary> = serde_json::from_str(&out).expect("valid json");
        assert_eq!(back, items);
    }

    #[test]
    fn summaries_json_omits_none_yesterday_flags() {
        let items = vec![sample_summary()];
        let out = render_summaries(&items, Format::Json);
        assert!(
            !out.contains("created_yesterday"),
            "should omit created_yesterday when None, got: {out}"
        );
        assert!(
            !out.contains("edited_yesterday"),
            "should omit edited_yesterday when None, got: {out}"
        );
    }

    #[test]
    fn summaries_json_includes_yesterday_flags_when_set() {
        let mut s = sample_summary();
        s.created_yesterday = Some(true);
        s.edited_yesterday = Some(false);
        let out = render_summaries(&[s], Format::Json);
        assert!(out.contains("created_yesterday"));
        assert!(out.contains("edited_yesterday"));
    }

    #[test]
    fn note_json_round_trips() {
        let note = sample_note();
        let out = render_note(&note, Format::Json);
        let back: NoteContent = serde_json::from_str(&out).expect("valid json");
        assert_eq!(back, note);
    }

    #[test]
    fn notebooks_json_round_trips() {
        let items = sample_notebooks();
        let out = render_notebooks(&items, Format::Json);
        let back: Vec<Notebook> = serde_json::from_str(&out).expect("valid json");
        assert_eq!(back, items);
    }

    // ---- Text ----

    #[test]
    fn summaries_text_has_fields_and_readable_dates() {
        let items = vec![sample_summary()];
        let out = render_summaries(&items, Format::Text);
        assert!(out.contains("A test note"), "title missing: {out}");
        assert!(out.contains("guid-123"), "guid missing: {out}");
        assert!(out.contains("nb-1"), "notebook_guid missing: {out}");
        assert!(out.contains("a short snippet"), "snippet missing: {out}");
        // ms-epoch timestamps formatted to a readable date, not the raw number
        assert!(out.contains("2023-11-14"), "created date missing: {out}");
        assert!(
            !out.contains("1700000000000"),
            "raw ms timestamp should not appear: {out}"
        );
    }

    #[test]
    fn note_text_has_title_metadata_and_text_body() {
        let note = sample_note();
        let out = render_note(&note, Format::Text);
        assert!(out.contains("A test note"), "title missing: {out}");
        assert!(out.contains("guid-123"), "guid missing: {out}");
        assert!(out.contains("Plain text body."), "text body missing: {out}");
        // text rendering should use `text`, not `markdown`
        assert!(
            !out.contains("Markdown **body**"),
            "should not include markdown body in text: {out}"
        );
    }

    #[test]
    fn notebooks_text_has_name_stack_and_guid() {
        let items = sample_notebooks();
        let out = render_notebooks(&items, Format::Text);
        assert!(out.contains("Inbox"), "name missing: {out}");
        assert!(out.contains("nb-1"), "guid missing: {out}");
        assert!(out.contains("Projects"), "second name missing: {out}");
        assert!(out.contains("nb-2"), "second guid missing: {out}");
        assert!(out.contains("Work"), "stack missing: {out}");
    }

    // ---- Markdown ----

    #[test]
    fn summaries_markdown_lists_titles_as_links_or_items() {
        let items = vec![sample_summary()];
        let out = render_summaries(&items, Format::Markdown);
        assert!(out.contains("A test note"), "title missing: {out}");
        assert!(out.contains("guid-123"), "guid missing: {out}");
        // markdown list/table marker present
        assert!(
            out.contains("- ") || out.contains('|'),
            "expected a markdown list or table: {out}"
        );
        assert!(out.contains("2023-11-14"), "readable date missing: {out}");
    }

    #[test]
    fn note_markdown_has_h1_title_and_markdown_body() {
        let note = sample_note();
        let out = render_note(&note, Format::Markdown);
        assert!(out.contains("# A test note"), "h1 title missing: {out}");
        assert!(
            out.contains("Markdown **body**"),
            "markdown body missing: {out}"
        );
        // markdown rendering should use `markdown`, not plain `text`
        assert!(
            !out.contains("Plain text body."),
            "should not include plain text body in markdown: {out}"
        );
    }

    #[test]
    fn notebooks_markdown_is_a_list() {
        let items = sample_notebooks();
        let out = render_notebooks(&items, Format::Markdown);
        assert!(out.contains("- "), "expected a markdown list: {out}");
        assert!(out.contains("Inbox"), "name missing: {out}");
        assert!(out.contains("Projects"), "second name missing: {out}");
        assert!(out.contains("Work"), "stack missing: {out}");
        assert!(out.contains("nb-1"), "guid missing: {out}");
    }

    #[test]
    fn empty_inputs_do_not_panic() {
        assert!(!render_summaries(&[], Format::Json).is_empty());
        let _ = render_summaries(&[], Format::Text);
        let _ = render_summaries(&[], Format::Markdown);
        let _ = render_notebooks(&[], Format::Text);
        let _ = render_notebooks(&[], Format::Markdown);
        assert!(!render_notebooks(&[], Format::Json).is_empty());
    }
}
