# evergeek

A Rust CLI (`evergeek`) for **searching and summarizing Evernote notes**. The
first release is **read + search only** — it authenticates via OAuth, searches
notes, fetches note content (converted to clean text/markdown), and lists
notebooks. It ships a `yesterday` command tailored to a daily-summary Claude
skill, and emits **JSON by default** for programmatic consumption.

- Package: `evergeekcli` · binary: `evergeek`
- License: MIT

## Project status: parked ⚠️

**This project is parked.** It is feature-complete for everything that can be
built and tested *without* a live Evernote connection, but it cannot talk to a
real Evernote account — and likely never will — because of how Evernote now gates
API access (see below).

### What works today (97 passing tests)

- Full CLI surface and argument parsing (`auth`, `search`, `recent`, `yesterday`,
  `note get`, `notebooks`).
- Command logic, output rendering (JSON / text / markdown), ENML→text/markdown
  conversion, and secure credential storage (atomic `0600` write, token-redacting
  `Debug`) — all unit-tested against an in-memory mock Evernote client.
- The OAuth 1.0a (PLAINTEXT) login flow with a localhost callback.

### What's missing

- The **Evernote Thrift/EDAM client** that performs the live API calls
  (`findNotesMetadata`, `getNote`, `listNotebooks`). It is fully specified in
  `docs/superpowers/plans/2026-05-21-evergeekcli.md` (Phase 4) but was never built,
  because there is no way to validate it (see below).

### Why it's parked — Evernote API access (as of May 2026)

Standing up a *new* personal integration on the Evernote API is effectively closed:

- **Developer tokens** are "currently unavailable except for proven necessity" —
  the account-level *Create a developer token* button is disabled.
- **OAuth requires an API key** (consumer key + secret), and new keys go through a
  **manual review (~5 business days)** that, in practice, is not issuing keys to
  individual developers.
- **The sandbox is deprecated**, so there is no instant-access environment to
  develop against either.

With no token and no key, the live client cannot be exercised, so finishing it
would only add unvalidated code. The honest stopping point is here.

### Reviving it

If you already hold an Evernote consumer key/secret (or pick this up somewhere API
access still exists), the path is fully mapped: implement Phase 4 (Thrift client)
and Phase 5 (wire + live validation) from the plan, set the two env vars below,
and run `evergeek auth login`. The auth, command, and rendering layers are already
done and tested.

## Commands

```text
evergeek auth login | status | logout      # OAuth; status never prints the token
evergeek search <query> [--notebook N] [--since 1d|YYYY-MM-DD] [--limit N]
evergeek recent [--since 1d] [--limit N]    # rolling recently-updated window
evergeek yesterday [--notebook N] [--limit N] [--full]   # notes with activity yesterday
evergeek note get <guid>
evergeek notebooks
# global: --format json|text|markdown (default json), --sandbox, --verbose
```

`<query>` accepts Evernote's native search grammar. `yesterday` returns the union
of notes **created** and **edited** yesterday, each tagged with `created_yesterday`
/ `edited_yesterday`; `--full` inlines the converted note bodies for a one-shot
daily digest.

## API credentials (if reviving)

Evernote no longer issues developer tokens to individuals, so `evergeek` uses
OAuth 1.0a. To run it for real you would need an Evernote **API consumer
key/secret** (Basic access is enough for read + search) — see *Why it's parked*
above for why that is no longer practically obtainable. Supply them **only via
environment variables**:

```sh
export EVERGEEK_CONSUMER_KEY="your-consumer-key"
export EVERGEEK_CONSUMER_SECRET="your-consumer-secret"
```

### Never commit secrets

This is a **public repository**. **Never commit your consumer key/secret or your
auth token.**

- The consumer key/secret are read **only from the environment** — never put them
  in a tracked file.
- Your OAuth auth token is persisted to `credentials.toml` in your platform
  config directory (`chmod 0600`), **outside the repo**. The path can be
  overridden with `EVERGEEK_CONFIG_DIR`.
- `.gitignore` already excludes `credentials.toml` and `*.env` as defense in
  depth. Keep it that way.

## Build & run

Requires a stable Rust toolchain (see `rust-toolchain.toml`).

```sh
# Build
cargo build

# Run tests
cargo test

# Lint & format checks (as CI runs them)
cargo clippy -- -D warnings
cargo fmt --check

# Run the CLI
cargo run --bin evergeek -- --help
```

### Authenticating (if reviving)

```sh
export EVERGEEK_CONSUMER_KEY=...   # never commit these
export EVERGEEK_CONSUMER_SECRET=...

evergeek auth login      # opens a browser for the OAuth flow
evergeek auth status     # shows auth state (never the token)
evergeek notebooks       # (requires the Phase 4 Thrift client)
```

A `--sandbox` flag exists for `sandbox.evernote.com`, but note that Evernote has
deprecated the sandbox — production (the default) is the only live target.
