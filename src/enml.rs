//! ENML (Evernote Markup Language, an XHTML dialect) conversion.
//!
//! ENML is XHTML wrapped in `<en-note>` with a handful of Evernote-specific
//! elements (`<en-todo>`, `<en-media>`, `<en-crypt>`). We lower those special
//! elements to plain readable placeholders, then let `html2text` render the
//! remaining standard HTML to text or markdown.

use html2text::from_read;
use html2text::render::text_renderer::TrivialDecorator;

/// Render width passed to `html2text`. Wide enough to avoid wrapping the kind
/// of short prose found in notes while still bounding pathological input.
const RENDER_WIDTH: usize = 100;

/// Convert an ENML document to clean plain text.
///
/// Strips the `<en-note>` wrapper and all markup, lowering Evernote-specific
/// elements to placeholders (`[x]`/`[ ]` for todos, `[attachment]` for media,
/// `[encrypted]` for crypt blocks). Links render as their visible text only.
pub fn enml_to_text(enml: &str) -> String {
    let html = lower_evernote_elements(enml);
    let rendered =
        html2text::from_read_with_decorator(html.as_bytes(), RENDER_WIDTH, TrivialDecorator::new());
    rendered.trim().to_string()
}

/// Convert an ENML document to markdown.
///
/// Preserves headings (`#`), lists (`*`/`1.`), emphasis, and links (rendered as
/// `[text][n]` with reference definitions) via `html2text`'s default decorator,
/// while lowering Evernote-specific elements to placeholders.
pub fn enml_to_markdown(enml: &str) -> String {
    let html = lower_evernote_elements(enml);
    from_read(html.as_bytes(), RENDER_WIDTH).trim().to_string()
}

/// Replace Evernote-specific void/special elements with plain-HTML text nodes
/// so a standard HTML renderer can handle the rest of the document.
///
/// - `<en-todo checked="true"/>` -> `[x] `, otherwise `[ ] `
/// - `<en-media .../>`           -> `[attachment]`
/// - `<en-crypt ...>...</en-crypt>` -> `[encrypted]`
///
/// The surrounding `<en-note>` wrapper is left in place; the HTML parser treats
/// it as a generic element and emits only its children.
fn lower_evernote_elements(enml: &str) -> String {
    let no_crypt = replace_element_blocks(enml, "en-crypt", "[encrypted]");
    let no_media = replace_self_closing(&no_crypt, "en-media", |_attrs| "[attachment]".to_string());
    replace_self_closing(&no_media, "en-todo", |attrs| {
        if attr_is_true(attrs, "checked") {
            "[x] ".to_string()
        } else {
            "[ ] ".to_string()
        }
    })
}

/// Returns true when the named attribute is present with the value `"true"`.
fn attr_is_true(attrs: &str, name: &str) -> bool {
    // Look for `name="true"` (or `name='true'`), case-insensitively on the value.
    let mut rest = attrs;
    while let Some(pos) = rest.find(name) {
        let after = &rest[pos + name.len()..];
        let trimmed = after.trim_start();
        if let Some(eq) = trimmed.strip_prefix('=') {
            let val = eq.trim_start();
            let val = val
                .strip_prefix('"')
                .or_else(|| val.strip_prefix('\''))
                .unwrap_or(val);
            let end = val.find(['"', '\'', ' ', '/', '>']).unwrap_or(val.len());
            return val[..end].eq_ignore_ascii_case("true");
        }
        rest = after;
    }
    false
}

/// Replace every `<tag ...>` (self-closing or open form) with the text produced
/// by `make_text`, which receives the raw attribute string.
fn replace_self_closing(input: &str, tag: &str, make_text: impl Fn(&str) -> String) -> String {
    let open = format!("<{tag}");
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = find_element_start(rest, &open) {
        out.push_str(&rest[..pos]);
        let after_name = &rest[pos + open.len()..];
        // Find the closing `>` of this start tag.
        if let Some(gt) = after_name.find('>') {
            let attrs = after_name[..gt].trim_end_matches('/').trim();
            out.push_str(&make_text(attrs));
            rest = &after_name[gt + 1..];
        } else {
            // Malformed: no closing `>`; emit the rest verbatim and stop.
            out.push_str(&rest[pos..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

/// Replace every `<tag ...>...</tag>` block (including its contents) with
/// `replacement`.
fn replace_element_blocks(input: &str, tag: &str, replacement: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = find_element_start(rest, &open) {
        out.push_str(&rest[..pos]);
        let after_name = &rest[pos + open.len()..];
        match (after_name.find('>'), after_name.find(&close)) {
            (Some(gt), Some(close_pos)) if close_pos >= gt => {
                out.push_str(replacement);
                rest = &after_name[close_pos + close.len()..];
            }
            (Some(gt), _) if after_name[..gt].trim_end().ends_with('/') => {
                // Self-closing form with no separate close tag.
                out.push_str(replacement);
                rest = &after_name[gt + 1..];
            }
            _ => {
                out.push_str(&rest[pos..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Find the byte offset of `open` (e.g. `<en-todo`) only when it begins an
/// actual element (the next char is whitespace, `>`, or `/`), so that a tag
/// like `<en-todoextra>` is not matched.
fn find_element_start(haystack: &str, open: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = haystack[from..].find(open) {
        let pos = from + rel;
        let next = haystack[pos + open.len()..].chars().next();
        match next {
            None | Some(' ') | Some('\t') | Some('\n') | Some('\r') | Some('>') | Some('/') => {
                return Some(pos)
            }
            _ => from = pos + open.len(),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE: &str = include_str!("../fixtures/simple.enml");
    const TODO: &str = include_str!("../fixtures/todo.enml");
    const MEDIA: &str = include_str!("../fixtures/media.enml");

    // ----- enml_to_text -----

    #[test]
    fn text_strips_wrapper_and_tags() {
        let out = enml_to_text(SIMPLE);
        assert!(!out.contains("en-note"), "wrapper leaked: {out:?}");
        assert!(!out.contains('<'), "raw tags leaked: {out:?}");
        assert!(out.contains("Grocery List"), "heading missing: {out:?}");
        assert!(out.contains("Apples"), "list item missing: {out:?}");
        assert!(out.contains("week"), "body text missing: {out:?}");
    }

    #[test]
    fn text_renders_link_text_without_markup() {
        let out = enml_to_text(SIMPLE);
        assert!(out.contains("corner store"), "link text missing: {out:?}");
        // Plain text drops markdown header markers.
        assert!(
            !out.lines().any(|l| l.trim_start().starts_with("# ")),
            "markdown header marker leaked into plain text: {out:?}"
        );
    }

    #[test]
    fn text_renders_todos() {
        let out = enml_to_text(TODO);
        assert!(
            out.contains("[x] Ship the release"),
            "checked todo: {out:?}"
        );
        assert!(
            out.contains("[ ] Write the changelog"),
            "unchecked (false) todo: {out:?}"
        );
        assert!(
            out.contains("[ ] Email the team"),
            "unchecked (bare) todo: {out:?}"
        );
        assert!(!out.contains("en-todo"), "en-todo leaked: {out:?}");
    }

    #[test]
    fn text_renders_media_and_crypt_placeholders() {
        let out = enml_to_text(MEDIA);
        assert!(out.contains("[attachment]"), "media placeholder: {out:?}");
        assert!(out.contains("[encrypted]"), "crypt placeholder: {out:?}");
        assert!(!out.contains("en-media"), "en-media leaked: {out:?}");
        assert!(!out.contains("en-crypt"), "en-crypt leaked: {out:?}");
        assert!(
            !out.contains("U2FsdGVkX1"),
            "encrypted ciphertext leaked: {out:?}"
        );
    }

    // ----- enml_to_markdown -----

    #[test]
    fn markdown_preserves_heading_list_and_link() {
        let out = enml_to_markdown(SIMPLE);
        assert!(
            out.lines().any(|l| l.trim() == "# Grocery List"),
            "markdown heading missing: {out:?}"
        );
        assert!(
            out.lines().any(|l| l.trim_start().starts_with("* Apples")),
            "markdown list item missing: {out:?}"
        );
        // Link target preserved somewhere in the markdown.
        assert!(
            out.contains("https://example.com/store"),
            "link target missing: {out:?}"
        );
        assert!(out.contains("corner store"), "link text missing: {out:?}");
        assert!(!out.contains("en-note"), "wrapper leaked: {out:?}");
    }

    #[test]
    fn markdown_renders_todos() {
        let out = enml_to_markdown(TODO);
        assert!(
            out.contains("[x] Ship the release"),
            "checked todo: {out:?}"
        );
        assert!(
            out.contains("[ ] Write the changelog"),
            "unchecked todo: {out:?}"
        );
        assert!(out.contains("[ ] Email the team"), "bare todo: {out:?}");
    }

    #[test]
    fn markdown_renders_media_and_crypt_placeholders() {
        let out = enml_to_markdown(MEDIA);
        assert!(out.contains("[attachment]"), "media placeholder: {out:?}");
        assert!(out.contains("[encrypted]"), "crypt placeholder: {out:?}");
    }
}
