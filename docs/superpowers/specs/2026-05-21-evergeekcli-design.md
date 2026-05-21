# evergeekcli — Design Spec

**Date:** 2026-05-21
**Status:** Approved design, pre-implementation
**Author:** Matt + Claude

## 1. Purpose

A Rust CLI for interacting with Evernote, primarily to **search and summarize notes**
for a daily-summary Claude skill. First release is **read + search only**, but the
architecture must make **writing** a clean future addition. The repo is **public**, so
no secrets ever land in git.

- Package: `evergeekcli` · binary: `evergeek`
- Repo: `mattisthenation/evergeekcli` (new, fresh git history)
- Reference implementation: the Python `geeknote` fork at
  `github.com/mattisthenation/evergeeknote` (reference only — no code copied; it is GPL)

## 2. Goals / Non-goals

**Goals (v1)**
- Authenticate via OAuth 1.0a and persist credentials securely.
- Search notes (raw Evernote grammar + convenience flags).
- Fetch a note's full content, converted to clean text/markdown.
- List notebooks.
- A `yesterday` command tailored to the daily-summary use case.
- JSON-first output for programmatic (skill) consumption.

**Non-goals (v1)**
- Creating/editing/deleting notes (architected for, not built).
- Sync, attachments/resources download, business/shared notebooks, reminders.
- A TUI. This is a scriptable, non-interactive CLI (except the one-time OAuth browser step).

## 3. Authentication

Evernote no longer issues developer tokens to individuals ("unavailable except for proven
necessity"), so v1 uses **OAuth 1.0a, 3-legged, PLAINTEXT signatures** (valid over HTTPS;
avoids HMAC base-string construction).

**API credentials (consumer key/secret)** are obtained from Evernote via a manually
reviewed request (~5 business days; **Basic** access is sufficient for read+search). They
are supplied **only via environment variables** and never written to the repo:

- `EVERGEEK_CONSUMER_KEY`
- `EVERGEEK_CONSUMER_SECRET`

**Flow (`evergeek auth login`):**
1. Bind a `TcpListener` on `127.0.0.1:0` (random free port) → callback `http://localhost:<port>/`.
2. `GET {base}/oauth` with `oauth_consumer_key`, `oauth_signature={secret}%26`,
   `oauth_signature_method=PLAINTEXT`, `oauth_timestamp`, `oauth_nonce`, `oauth_callback`
   → parse temporary `oauth_token`.
3. Open the browser to `{base}/OAuth.action?oauth_token=<temp>` (fallback: print the URL).
4. Local server captures the redirect's `oauth_verifier`; respond with a "you can close this tab" page.
5. `GET {base}/oauth` with the temp token + verifier → parse the access token response:
   `oauth_token` (the auth token), `edam_shard`, `edam_userId`, `edam_expires`,
   `edam_noteStoreUrl`, `edam_webApiUrlPrefix`.
6. Persist to `credentials.toml` (see §7), `chmod 0600`.

`{base}` is `https://www.evernote.com` (prod) or `https://sandbox.evernote.com` (`--sandbox`).
The OAuth response gives us `noteStoreUrl` directly — no separate shard lookup needed.

- `evergeek auth status` — print user id, token expiry, note-store host, sandbox/prod. **Never prints the token.**
- `evergeek auth logout` — delete `credentials.toml`.

## 4. Evernote API client (primary technical risk, owned)

There is no maintained Rust Evernote SDK. The Evernote API is **Thrift (EDAM)**: `UserStore`
and `NoteStore` services, binary protocol over HTTP POST (`Content-Type: application/x-thrift`).
The auth token is passed as a **method parameter** to each call (not an HTTP header).

**Approach:**
- Vendor Evernote's official `.thrift` IDL (`UserStore`, `NoteStore`, `Types`, `Errors`,
  `Limits`) under `thrift/`, preserving Evernote's license header.
- Generate Rust with the Apache Thrift compiler (`thrift --gen rs`), **commit** the output
  under `src/edam/`, and document regeneration (pin compiler + `thrift` crate versions).
- `evernote/http_transport.rs`: a Thrift transport implementing `Read`/`Write` that buffers
  written bytes and, on `flush()`, POSTs the buffer to the endpoint via `reqwest::blocking`
  and loads the response bytes for subsequent reads (standard "Thrift over HTTP" pattern,
  `TBinaryProtocol`).
- `evernote/thrift_client.rs`: the real `EvernoteClient` impl. Wraps generated `UserStore`/
  `NoteStore` clients and **maps EDAM types → our domain `model` types** so nothing else in
  the codebase depends on Thrift.

**Validation honesty:** the full wire round-trip cannot be confirmed end-to-end until the API
key exists. Until then: code compiles, and `http_transport` is unit-tested against a recorded
Thrift binary fixture. The first thing done when the key lands is a live `getUser` +
`findNotesMetadata` against **sandbox**, then prod after activation.

## 5. Domain model & isolation

`EvernoteClient` (trait) is the single boundary the rest of the app talks to:

```rust
trait EvernoteClient {
    fn search_notes(&self, q: &NoteQuery) -> Result<Vec<NoteSummary>>;
    fn get_note(&self, guid: &str) -> Result<NoteContent>;
    fn list_notebooks(&self) -> Result<Vec<Notebook>>;
    // future: create_note(&self, ...) -> Result<NoteSummary>
}
```

Clean domain types in `model.rs` (all `serde`-serializable):
- `NoteSummary { guid, title, notebook, tags, created, updated, snippet, created_yesterday?, edited_yesterday? }`
- `NoteContent { summary: NoteSummary, text, markdown, enml }`
- `Notebook { guid, name, stack }`
- `NoteQuery { words, notebook, since, limit }` (built by commands, rendered to Evernote grammar)

Two `EvernoteClient` impls: `thrift_client` (real) and `mock` (fixture-backed, for tests).

**Module layout** (thin binary over a testable library; Thrift and OAuth each sealed in one place):

```
src/
  main.rs            # parse args → dispatch → render
  cli.rs             # clap command/arg defs
  config.rs          # config dir, load/save creds (toml, 0600), env overrides
  model.rs           # domain types: NoteSummary, NoteContent, Notebook, NoteQuery (+ serde)
  output.rs          # model → json | text | markdown
  enml.rs            # ENML (XHTML) → text / markdown
  auth/
    mod.rs           # AuthProvider trait, Credentials
    oauth.rs         # OAuth 1.0a PLAINTEXT, 3-legged, localhost callback server
  evernote/
    mod.rs           # EvernoteClient trait
    thrift_client.rs # real impl: generated EDAM clients, maps EDAM → model
    http_transport.rs# Thrift-over-HTTP: buffers writes, POSTs on flush via reqwest
    mock.rs          # fixture-backed impl for tests
  commands/
    auth.rs  search.rs  note.rs  notebooks.rs   # one module per command group
  edam/              # GENERATED Thrift code (committed); regen documented
thrift/              # vendored Evernote .thrift IDL (license header preserved)
fixtures/            # sample ENML + recorded API responses
tests/               # integration tests (mock client; live tests gated on creds)
```

## 6. Command surface (v1: read + search)

Binary `evergeek`. Global flags: `--format json|text|markdown` (default **json**),
`--sandbox`, `--verbose`.

| Command | Behavior |
|---|---|
| `auth login` | OAuth flow (§3), persist creds |
| `auth status` | Show auth state; never the token |
| `auth logout` | Delete stored creds |
| `search <query>` | `findNotesMetadata`; `<query>` is raw Evernote grammar. Flags: `--notebook <name>`, `--since <1d\|YYYY-MM-DD>`, `--limit <n>` (compose into the query) |
| `recent` | Rolling window sugar; `--since 1d` (default), `--limit <n>` → `updated:day-N` |
| `yesterday` | See §6.1 — the daily-summary headline command |
| `note get <guid>` | Full note; ENML → text/markdown |
| `notebooks` | List notebooks |

### 6.1 `yesterday` semantics

"All notes created and edited yesterday" = notes with **activity yesterday** =
**(created yesterday) ∪ (edited yesterday)**, deduped by GUID.

- Runs two `findNotesMetadata` queries and merges:
  - created window: `created:day-1 -created:day`
  - updated window: `updated:day-1 -updated:day`
- Each merged `NoteSummary` is tagged `created_yesterday` / `edited_yesterday` so the skill
  knows why a note appears.
- Sorted by `updated` desc. Flags: `--notebook`, `--limit`, `--full`.
- `--full` inlines each note's converted body (`get_note` per result) for a one-shot daily
  digest — otherwise metadata only, and the skill fetches bodies selectively.
- **Timezone:** the "yesterday" boundary is computed by Evernote in the account's timezone
  via the relative `day-1`/`day` grammar (we don't compute local midnight). Documented caveat;
  a `--tz` override is a future option.

## 7. Config & secrets

- Config dir resolved via `directories` (platform-appropriate), overridable with
  `EVERGEEK_CONFIG_DIR`.
- `credentials.toml` (mode `0600`, in the user's home — **not** the repo): `auth_token`,
  `note_store_url`, `web_api_url_prefix`, `user_id`, `shard`, `expires`, `sandbox`.
- Consumer key/secret: **env only** by default. Documented in README; never committed.
- `.gitignore` covers `target/`, `*.env`, `credentials.toml` (defense in depth even though
  it lives outside the repo).
- `auth status` and all logging redact tokens.

## 8. ENML conversion

ENML is XHTML wrapped in `<en-note>`. `enml.rs`:
- Strip/transform Evernote-specific tags (`<en-media>` → placeholder, `<en-todo>` → `[ ]`/`[x]`,
  `<en-crypt>` → placeholder).
- Produce **text** (via `html2text`) and **markdown**. Clean text is the priority for summarization.
- Pure function over a string → unit-testable against `fixtures/*.enml`.

## 9. Error handling

- Library: `thiserror`. Map EDAM exceptions explicitly:
  - `EDAMUserException` (bad auth/params) → actionable message.
  - `EDAMNotFoundException` → "note/notebook not found".
  - `EDAMSystemException.rateLimitDuration` → "rate limited, retry in Ns" (surface the seconds).
  - Expired token → message pointing to `auth login`.
- Binary/commands: `anyhow`, non-zero exit on error, errors to stderr (stdout stays clean for JSON).

## 10. Testing & CI

- **Unit (no network):** ENML conversion, query building (incl. `yesterday`'s two queries),
  output rendering (json/text/markdown), config load/save + `0600` perms, OAuth response
  parsing, `http_transport` against a recorded Thrift fixture.
- **Command-level:** via the `mock` client.
- **Live integration:** gated behind env credentials (`#[ignore]` / feature), skipped in CI.
- **CI (GitHub Actions, public repo):** `cargo build`, `cargo test`, `cargo clippy -D warnings`,
  `cargo fmt --check`.

## 11. Crates (initial)

`clap` (derive), `reqwest` (blocking), `thrift`, `serde`/`serde_json`, `toml`, `directories`,
`anyhow`, `thiserror`, `html2text`, `webbrowser` (open auth URL), `url`, `time`/`chrono`.
Plus the Apache Thrift compiler as a build-time tool (not a runtime dep).

## 12. Repo, PR workflow & license

- `git init` here; create `mattisthenation/evergeekcli`.
- **All work on feature branches; never commit directly to `main`.**
- Before any merge: open a PR with `commit-commands:commit-push-pr`, run the `/review`
  (pre-landing diff review) skill as the gate. **Claude opens PRs; Matt merges** (unless
  explicitly told otherwise).
- License: **MIT** for our fresh code. Vendored Evernote `.thrift` keeps its own license header.

## 13. Build order

1. Scaffold (`cargo` project, `cli.rs` skeleton, `.gitignore`, README stub) + CI + PR workflow.
2. Config/secrets (`config.rs`) + `model.rs` + `output.rs`.
3. `mock` client + `enml.rs` + command wiring + tests — **all credential-free**, fully testable.
4. OAuth (`auth/oauth.rs`) + `auth` commands.
5. Thrift codegen (`src/edam/`) + `http_transport.rs` + `thrift_client.rs`.
6. Live wiring & validation against **sandbox**, then prod after key activation.

Steps 1–4 proceed in parallel with the API-key request, so the credential gate doesn't block progress.
