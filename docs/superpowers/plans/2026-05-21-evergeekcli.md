# evergeekcli Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Every task is TDD: write the failing test, see it fail, implement minimally, see it pass, commit.

**Goal:** A Rust CLI (`evergeek`) that authenticates to Evernote via OAuth and reads/searches notes, with a `yesterday` command for a daily-summary Claude skill. Read+search now; write later.

**Architecture:** Thin binary over a testable library. The `EvernoteClient` trait is the single boundary — nothing outside `evernote/` touches Thrift/EDAM types. Thrift-over-HTTP transport runs the binary protocol via `reqwest`. OAuth and Thrift are each sealed in one module. JSON-first output.

**Tech Stack:** Rust, `clap`, `reqwest` (blocking), `thrift`, `serde`/`serde_json`, `toml`, `directories`, `anyhow`, `thiserror`, `html2text`, `webbrowser`. Apache Thrift compiler at build time only.

**Spec:** `docs/superpowers/specs/2026-05-21-evergeekcli-design.md`

---

## Dependency graph / parallelization

```
Phase 0 (SERIAL — freezes all interfaces, must land first):
  T0 scaffold → T1 model → T2 client trait + errors

Phase 1 (PARALLEL — each owns disjoint files, depends only on T1/T2):
  T3 enml      T4 output      T5 config      T6 mock client

Phase 2 (after T4+T6): T7 cli skeleton → T8 search/recent/yesterday → T9 note/notebooks
Phase 3 (after T5):    T10 oauth → T11 auth commands
Phase 4 (after T1):    T12 thrift codegen → T13 http_transport → T14 thrift_client
Phase 5 (after all):   T15 integrate real client + live sandbox validation (gated on API key)
```

**Agents work in separate git worktrees per the using-git-worktrees skill.** Phase 0 lands on `main` first so every worktree branches from frozen interfaces. Within a phase, agents own disjoint files; `Cargo.toml` deps and `src/lib.rs` module declarations are fully pre-populated in T0 so no agent edits shared files.

---

## FROZEN INTERFACES (all tasks must match these exactly)

`src/model.rs`:
```rust
use serde::{Serialize, Deserialize};

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
```

`src/error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvergeekError {
    #[error("not authenticated — run `evergeek auth login`")]
    NotAuthenticated,
    #[error("authentication token expired — run `evergeek auth login`")]
    TokenExpired,
    #[error("Evernote API error: {0}")]
    Api(String),
    #[error("rate limited by Evernote — retry in {0} seconds")]
    RateLimited(i32),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("config error: {0}")]
    Config(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, EvergeekError>;
```

`src/evernote/mod.rs`:
```rust
use crate::model::{NoteSummary, NoteContent, Notebook, NoteQuery};
use crate::error::Result;

pub trait EvernoteClient {
    fn search_notes(&self, query: &NoteQuery) -> Result<Vec<NoteSummary>>;
    fn get_note(&self, guid: &str) -> Result<NoteContent>;
    fn list_notebooks(&self) -> Result<Vec<Notebook>>;
    // Future (v2): fn create_note(&self, ...) -> Result<NoteSummary>;
}
```

`src/config.rs` (the persisted credentials shape):
```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Credentials {
    pub auth_token: String,        // SECRET
    pub note_store_url: String,
    pub web_api_url_prefix: String,
    pub user_id: i32,
    pub shard: String,
    pub expires: i64,              // ms since epoch
    pub sandbox: bool,
}
```

`src/output.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum Format { Json, Text, Markdown }
// pub fn render_summaries(items: &[NoteSummary], fmt: Format) -> String
// pub fn render_note(note: &NoteContent, fmt: Format) -> String
// pub fn render_notebooks(items: &[Notebook], fmt: Format) -> String
```

---

## Phase 0 — Foundation (SERIAL)

### Task 0: Scaffold
**Files:** Create `Cargo.toml`, `src/main.rs`, `src/lib.rs`, `README.md`, `LICENSE` (MIT), `.github/workflows/ci.yml`, `rust-toolchain.toml`.

- [x] Init crate: package `evergeekcli`, `[[bin]] name = "evergeek" path = "src/main.rs"`, plus a lib (`src/lib.rs`).
- [x] `Cargo.toml` declares ALL deps from the Tech Stack up front (so no later task edits it): clap (derive), reqwest (blocking, rustls-tls, no default-features bloat), thrift, serde, serde_json, toml, directories, anyhow, thiserror, html2text, webbrowser, url, time (or chrono). Add `[dev-dependencies]`: `httpmock` (or `wiremock`), `tempfile`.
- [x] `src/lib.rs` declares every module up front, each pointing at a stub that compiles: `pub mod model; pub mod error; pub mod config; pub mod output; pub mod enml; pub mod cli; pub mod auth; pub mod evernote; pub mod commands;`
- [x] Create compiling stub files for every module/submodule listed in the spec layout (empty or `todo!()` bodies behind real signatures from FROZEN INTERFACES where known). Goal: `cargo build` succeeds.
- [x] `main.rs`: parse args via `cli`, dispatch, print, map errors to stderr + non-zero exit (stdout stays clean for JSON).
- [x] CI runs `cargo build`, `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` on stable.
- [x] README stub: what it is, the `EVERGEEK_CONSUMER_KEY`/`_SECRET` env requirement, "never commit secrets" note, build/run.
- [x] Verify: `cargo build && cargo clippy -- -D warnings && cargo fmt --check` all pass. Commit.

### Task 1: Domain model
**Files:** `src/model.rs` (replace stub with FROZEN INTERFACE above). Test: `cargo test model`.
- [x] Paste the frozen `model.rs`. Add a unit test asserting `NoteSummary` round-trips through `serde_json` and that `created_yesterday: None` is omitted from JSON. Commit.

### Task 2: Client trait + errors
**Files:** `src/error.rs`, `src/evernote/mod.rs` (frozen interfaces above).
- [x] Paste frozen `error.rs` and `evernote/mod.rs`. Build. Commit. (No behavior to test yet; the mock in T6 exercises the trait.)

---

## Phase 1 — Parallel leaf modules (PARALLEL after Phase 0)

### Task 3: ENML conversion
**Files:** `src/enml.rs`, `fixtures/*.enml`. Test: `cargo test enml`.
Public API: `pub fn enml_to_text(enml: &str) -> String` and `pub fn enml_to_markdown(enml: &str) -> String`.
- [x] Add 2-3 fixtures: a simple note, one with `<en-todo checked="true"/>`/`<en-todo/>`, one with `<en-media .../>`.
- [x] TDD: `enml_to_text` strips the `<en-note>` wrapper and tags, returns readable text (use `html2text`). `<en-todo>` → `[x]`/`[ ]`, `<en-media>` → `[attachment]` placeholder, `<en-crypt>` → `[encrypted]`.
- [x] TDD: `enml_to_markdown` produces markdown (headings/lists/links preserved).
- [x] Tests assert against fixture expectations. Commit per cycle. ✅ DONE (commit 60de4c2, 7 tests)

### Task 4: Output rendering
**Files:** `src/output.rs`. Test: `cargo test output`.
- [x] Implement `Format` enum + `render_summaries`/`render_note`/`render_notebooks`.
- [x] JSON: `serde_json::to_string_pretty`. Text/Markdown: human tables/sections; note get in text = title + metadata + `text`; in markdown = `# title` + metadata + `markdown`.
- [x] TDD each renderer against a fixed `NoteSummary`/`NoteContent`. Assert JSON omits `created_yesterday` when None. Commit per cycle. ✅ DONE (commit 4d0b618, 12 tests)

### Task 5: Config & secrets
**Files:** `src/config.rs`. Test: `cargo test config` (use `tempfile` + `EVERGEEK_CONFIG_DIR`).
Public API: `Credentials` (frozen), `pub fn config_dir() -> Result<PathBuf>` (honors `EVERGEEK_CONFIG_DIR`, else `directories`), `pub fn load_credentials() -> Result<Option<Credentials>>`, `pub fn save_credentials(&Credentials) -> Result<()>` (writes `credentials.toml`, chmod `0600` on unix), `pub fn delete_credentials() -> Result<()>`, and consumer-key/secret readers from env: `pub fn consumer_key() -> Result<String>`, `pub fn consumer_secret() -> Result<String>` (read `EVERGEEK_CONSUMER_KEY`/`_SECRET`, error `Config` if missing).
- [x] TDD: save then load round-trips. On unix, saved file mode is `0600`. delete removes it. Missing file → `Ok(None)`. Missing env var → `Config` error. Commit per cycle. ✅ DONE (commit 397a63d + review fix eb815c8). Review hardening: write is atomic temp+rename at 0600; `Credentials` has a manual `Debug` that redacts `auth_token`.

### Task 6: Mock client
**Files:** `src/evernote/mock.rs`. Test: `cargo test mock`.
Public: `pub struct MockClient { pub notes: Vec<NoteContent>, pub notebooks: Vec<Notebook> }` impl `EvernoteClient`. `search_notes` filters `notes` whose `summary.title`/text contains `query.words` (simple substring; honors `limit`), returns their summaries. `get_note` finds by guid (else `NotFound`). `list_notebooks` returns `notebooks`. Add `pub fn sample() -> MockClient` with deterministic fixture data for downstream tests.
- [x] TDD each method. Commit per cycle. ✅ DONE (commit fa987b4 + review fix eb815c8). Note: `search_notes` also filters by `query.notebook_guid`; `sample()` notebook guids are `nb-work` / `nb-personal`.

---

## Phase 2 — Commands

### FROZEN COMMAND CONTRACT (T7 defines; T8/T9/T11 implement bodies)

Each command function returns the already-rendered output `String`; `main` prints it to stdout, errors to stderr. Data commands are generic over the client via `&dyn EvernoteClient` so tests pass a `MockClient`. T7 creates these as compiling stubs (`todo!()` bodies) with EXACTLY these signatures; later tasks fill the bodies.

```rust
use crate::evernote::EvernoteClient;
use crate::output::Format;
use crate::error::Result;

// commands/search.rs  (T8)
pub fn search(client: &dyn EvernoteClient, words: &str, notebook: Option<&str>,
              since: Option<&str>, limit: i32, fmt: Format) -> Result<String>;
pub fn recent(client: &dyn EvernoteClient, since: &str, limit: i32, fmt: Format) -> Result<String>;
pub fn yesterday(client: &dyn EvernoteClient, notebook: Option<&str>, limit: i32,
                 full: bool, fmt: Format) -> Result<String>;

// commands/note.rs  (T9)
pub fn get(client: &dyn EvernoteClient, guid: &str, fmt: Format) -> Result<String>;

// commands/notebooks.rs  (T9)
pub fn list(client: &dyn EvernoteClient, fmt: Format) -> Result<String>;

// commands/auth.rs  (T11)  — no client; uses auth::oauth + config
pub fn login(sandbox: bool) -> Result<String>;
pub fn status() -> Result<String>;
pub fn logout() -> Result<String>;
```

`main` builds the data-command client via a helper `fn build_client(sandbox: bool) -> Result<Box<dyn EvernoteClient>>` that loads `Credentials` and constructs a `ThriftClient` (stub until Phase 4 — `--notebook`/`--since` parsing and command logic are still fully unit-testable via `MockClient` against the functions above).

### Task 7: CLI skeleton
**Files:** `src/cli.rs`. Public: a clap derive `Cli` with global `--format` (default Json), `--sandbox`, `--verbose`, and a `Commands` enum: `Auth{..}`, `Search{..}`, `Recent{..}`, `Yesterday{..}`, `Note{..}`, `Notebooks`. `main.rs` builds a client and dispatches into `commands::`.
- [x] Define args per the spec command table. `--since` accepts `1d`/`Nd` or `YYYY-MM-DD`. Build. A test parsing representative argv via `Cli::try_parse_from`. Commit.

### Task 8: search / recent / yesterday
**Files:** `src/commands/search.rs` (+ a `query` helper). Test: `cargo test commands` with `MockClient`.
Query building rules:
- `search <words>`: `NoteQuery.words = words`; `--notebook` resolves name→guid via `list_notebooks`; `--since 1d`→prepend `updated:day-1`, `--since YYYY-MM-DD`→`updated:YYYYMMDD`; `--limit` default 50.
- `recent`: `--since 1d` default → `words = "updated:day-N"`.
- `yesterday`: run TWO `search_notes` calls — `created:day-1 -created:day` and `updated:day-1 -updated:day` — merge by `guid`, set `created_yesterday`/`edited_yesterday` flags accordingly, sort by `updated` desc, apply `--limit`. `--full` then calls `get_note` per result and renders `NoteContent`s.
- [x] TDD the query-string builder as a pure function (`fn build_search_words(...) -> String`) with table tests for each flag combination.
- [x] TDD `yesterday` merge/dedup/flagging logic against a `MockClient` returning overlapping sets (pure merge fn `fn merge_yesterday(created: Vec<NoteSummary>, updated: Vec<NoteSummary>) -> Vec<NoteSummary>`). Commit per cycle.

### Task 9: note get / notebooks
**Files:** `src/commands/note.rs`, `src/commands/notebooks.rs`.
- [x] `note get <guid>`: call `get_note`, render per `--format`. TDD against `MockClient` (incl. `NotFound`).
- [x] `notebooks`: call `list_notebooks`, render. TDD. Commit per cycle.

---

## Phase 3 — Auth

### Task 10: OAuth 1.0a flow
**Files:** `src/auth/oauth.rs`. Test: `cargo test oauth` (no live network — test pure builders/parsers; mock HTTP with `httpmock`).

> ⚠️ **Deferred review note (from Phase 1 review):** `config::tests` has a private `ENV_LOCK` mutex that only serializes env-var mutation *within that module*. If any oauth/command test also mutates `EVERGEEK_*` env vars, the whole test binary runs threads in parallel and they will race. Before adding env-mutating tests here, either add the `serial_test` crate (mark such tests `#[serial]`) or hoist a shared lock both modules use. Prefer pure helpers that take key/secret as args (as factored below) so most tests need no env at all.
Public: `pub fn login(sandbox: bool) -> Result<Credentials>` orchestrates the flow; but factor pure, testable helpers:
- `fn temp_token_url(base, key, secret, callback, ts, nonce) -> String` (PLAINTEXT: `oauth_signature={secret}%26`).
- `fn parse_oauth_response(body: &str) -> HashMap<String,String>` (urlencoded `k=v&...`).
- `fn access_token_url(base, key, secret, temp_token, verifier, ts, nonce) -> String`.
- `fn credentials_from_access_response(map, sandbox) -> Result<Credentials>` (reads `oauth_token`, `edam_*`).
- `login()` then: bind `127.0.0.1:0`, GET temp url, open browser to `{base}/OAuth.action?oauth_token=...` via `webbrowser`, accept one HTTP request on the listener, extract `oauth_verifier`, GET access url, build `Credentials`.
- [x] TDD the four pure helpers with known inputs/outputs. TDD the temp/access GETs against `httpmock`. Commit per cycle.

### Task 11: auth commands
**Files:** `src/commands/auth.rs`.
- [x] `login` → `oauth::login` → `config::save_credentials`. `status` → load creds, print user_id/expiry(human)/note-store host/sandbox, **never the token**; if none → `NotAuthenticated`. `logout` → `delete_credentials`. TDD `status` rendering + redaction with a fixture `Credentials`. Commit per cycle.

---

## Phase 4 — Thrift / EDAM (primary risk)

### Task 12: Vendor IDL + generate EDAM
**Files:** `thrift/*.thrift` (vendored), `src/edam/*` (generated, committed), `docs/regenerating-edam.md`.
- [ ] Fetch Evernote's official `.thrift` (`UserStore`, `NoteStore`, `Types`, `Errors`, `Limits`) from `github.com/evernote/evernote-thrift`, keep license headers.
- [ ] Install Apache Thrift compiler (`brew install thrift`), run `thrift -out src/edam --gen rs thrift/NoteStore.thrift` (and deps). Pin compiler + `thrift` crate versions in the doc. Make `cargo build` compile the generated code. Commit.

### Task 13: Thrift-over-HTTP transport
**Files:** `src/evernote/http_transport.rs`. Test: `cargo test transport` against a recorded binary fixture via `httpmock`.
- [ ] Implement a struct holding endpoint URL + a `reqwest::blocking::Client`, an out-buffer (`Vec<u8>`) and an in-cursor (`Cursor<Vec<u8>>`). `impl Write`: append to out-buffer; `flush()`: POST out-buffer to URL with `Content-Type: application/x-thrift`, load response bytes into the in-cursor, clear out-buffer. `impl Read`: read from in-cursor. Wrap with `TBinaryInputProtocol`/`TBinaryOutputProtocol`.
- [ ] TDD: feed a recorded request/response fixture; assert the POST body equals the written bytes and reads return the response bytes. Commit per cycle.

### Task 14: Real EvernoteClient
**Files:** `src/evernote/thrift_client.rs`. Public: `pub struct ThriftClient { creds: Credentials }` impl `EvernoteClient`.
- [ ] `search_notes`: build EDAM `NoteFilter{ words, notebookGuid, order=UPDATED, ascending=false }` + `NotesMetadataResultSpec{ includeTitle, includeCreated, includeUpdated, includeNotebookGuid, includeTagGuids }`, call `NoteStore.findNotesMetadata(authToken, filter, 0, limit, spec)`, map → `Vec<NoteSummary>` (server snippet if requested).
- [ ] `get_note`: `NoteStore.getNoteWithResultSpec` (content=true), ENML→text/markdown via `enml`, build `NoteContent`.
- [ ] `list_notebooks`: `NoteStore.listNotebooks` → map.
- [ ] Map EDAM exceptions → `EvergeekError` (`EDAMSystemException.rateLimitDuration` → `RateLimited`, not-found → `NotFound`, auth → `TokenExpired`/`NotAuthenticated`). Unit-test the mapping function. Commit per cycle.

---

## Phase 5 — Integration

### Task 15: Wire + live validation (gated on API key)
**Files:** `src/main.rs` (client selection), `README.md` (auth + usage docs).
- [ ] `main` builds `ThriftClient` from loaded `Credentials` for data commands; errors to `auth login` if none.
- [ ] When the API key arrives: `export EVERGEEK_CONSUMER_KEY/_SECRET`, `evergeek --sandbox auth login`, then `evergeek --sandbox notebooks` / `search` / `yesterday` against sandbox. Fix wire issues. Then prod after activation.
- [ ] Document full usage in README. Final commit.

---

## Self-review notes
- **Spec coverage:** auth(§3)→T10/T11; api client(§4)→T12-14; model/isolation(§5)→T1/T2/T6; commands(§6)→T7-9; yesterday(§6.1)→T8; config/secrets(§7)→T5; ENML(§8)→T3; errors(§9)→T2/T14; testing/CI(§10)→T0 + per-task; license/repo/PR(§12)→T0 + workflow. All covered.
- **Type consistency:** all tasks reference the single FROZEN INTERFACES block.
- **Parallelization:** Phase 1 tasks (T3/T4/T5/T6) own disjoint files and depend only on T1/T2 → safe to run concurrently in worktrees.
