# evergeek

A Rust CLI (`evergeek`) for **searching and summarizing Evernote notes**. The
first release is **read + search only** — it authenticates via OAuth, searches
notes, fetches note content (converted to clean text/markdown), and lists
notebooks. It ships a `yesterday` command tailored to a daily-summary Claude
skill, and emits **JSON by default** for programmatic consumption.

- Package: `evergeekcli` · binary: `evergeek`
- License: MIT

> **Status:** under construction. The full CLI surface, command logic, output
> rendering, ENML→text/markdown conversion, secure credential storage, and the
> OAuth 1.0a flow are implemented and unit-tested (97 tests). What's left is the
> **Evernote Thrift/EDAM client** that talks to the live API — so the data
> commands parse args and run their logic against tests, but cannot yet reach a
> real account until that client is wired (and an Evernote API key is approved).
> See `docs/superpowers/plans/2026-05-21-evergeekcli.md` for the remaining tasks.

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

## API credentials (required)

Evernote no longer issues developer tokens to individuals, so `evergeek` uses
OAuth 1.0a. You must first obtain an Evernote **API consumer key/secret** (Basic
access is enough for read + search) and supply them **only via environment
variables**:

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

### Authenticating (once available)

```sh
export EVERGEEK_CONSUMER_KEY=...   # never commit these
export EVERGEEK_CONSUMER_SECRET=...

evergeek --sandbox auth login      # opens a browser for the OAuth flow
evergeek --sandbox auth status     # shows auth state (never the token)
evergeek --sandbox notebooks
```

Use `--sandbox` against `sandbox.evernote.com` while testing; omit it for
production once your key is activated.
