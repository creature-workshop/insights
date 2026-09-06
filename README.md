# insights

Shared memory for coding agents. `insights` is a knowledge management tool for
teams that work with Claude Code, Cursor and the agents like them: an agent
stores what it learns as a small markdown insight filed under a topic, searches
the store before it starts the next task, and corrects what it finds stale. The
store is a directory of files a team shares through git, and the rule that
`insights setup` installs tells each agent when to read it, write to it, and
keep it honest.

## Why

An agent begins every session knowing nothing about your codebase beyond what
is checked in. The fixture that only works with one flag, the deploy step whose
error message lies, the convention nobody wrote down: each is rediscovered at
full price, by every agent, in every session. An insight is the note the agent
leaves for next time, and a search at the start of a task is how next time
collects it.

## What it does

- **A CLI agents call.** `search`, `add`, `get`, `update`, `delete`, `pin`,
  `list`, `topics`. Every command health-checks the local server and starts it
  when it is not running.
- **A rule for the agent.** `insights setup` installs the use-insights rule for
  Claude Code (`~/.claude/CLAUDE.md`), Cursor (`.cursor/rules`, global or per
  project), Windsurf and Zed. The rule says: search at the start of work, add
  on a discovery, update on stale information, and close with one summary
  insight.
- **A store the team shares.** `insights sync` makes the store a git repository
  and keeps every machine in step. A conflict stops the sync and names the
  insight instead of leaving markers behind.
- **Ranking that learns from use.** Retrieval and search-hit counts feed
  relevance. Pinned insights rank first and are protected from pruning.
- **Nothing to run by hand.** The CLI spawns `insights_server`, a local REST
  server on `127.0.0.1:2020`, on demand.

## Quick start

```bash
cargo install insights
insights setup
```

The wizard offers the rule destinations it detects, an optional shell hook,
and a git remote for the store. From there the agent does the rest, or seed
the store yourself:

```bash
insights add rust-workspace test-isolation \
  "Integration tests share INSIGHTS_ROOT and run serially." \
  "cargo test is single-threaded on purpose; a parallel run corrupts the store under test."
insights search test isolation
```

## What a session looks like

An agent asked to fix a flaky test runs `insights search flaky test <area>`
before it opens a file, follows the leads it finds, adds an insight when it
learns something the store lacks, updates the one that turned out to be wrong,
and records a one-line summary when it is done. Nothing in that loop is
specific to one agent: Claude Code and Cursor read the same store, and the
store outlives every session.

## Commands

```bash
insights add <topic> <name> <overview> <details>
insights search <terms>... [-t <topic>] [-e] [-s] [-x <term>]... [--since 7d] [--until <date>] [--sort relevance|updated|created|least-accessed]
insights get <topic> <name> [-o]
insights list [topic] [-v] [--pinned] [--sort ...]
insights update <topic> <name> [-o <overview>] [-d <details>]
insights delete <topic> <name> [-f]
insights pin <topic> <name>
insights unpin <topic> <name>
insights topics
insights sync [--remote <url> | --forget-remote]
insights setup
insights logs [--level info|warn|error|all]
```

Search also takes a result cap and a case-sensitive switch; `--help` on any
command lists the rest. Run by hand, the server binds `127.0.0.1:2020`;
override with `--bind` or `INSIGHTS_SERVER_URL`. The REST surface is defined in
`src/server/routing.rs`.

## Sharing a store between machines

`insights sync` commits what this machine wrote, replays it on top of what the
others wrote, and pushes. Point it at a remote you own the first time, then
run it bare after that:

```bash
insights sync --remote git@example.com:you/insights.git
insights sync
```

Local work is committed before anything is pulled, so an interrupted sync
cannot lose an insight. When the same insight was edited on two machines the
sync stops and names it, leaving the store exactly as it was; the reasoning is
recorded in
[docs/decisions/000001](docs/decisions/000001-sync-stops-on-conflict.md).
`insights sync --forget-remote` stops the store syncing and keeps its history.

## Storage

Insights live on disk as `<root>/<topic>/<name>.insight.md`: YAML frontmatter
and a details body. A topic or name may not contain a path separator. The root
is `$INSIGHTS_ROOT` when set, otherwise `$XDG_DATA_HOME/insights` (typically
`~/.local/share/insights`), or `~/.blizz/persistent/insights` where that
directory already exists.

This machine's usage counters live outside the store, in
`~/.blizz/volatile/insights/usage.json` (`INSIGHTS_USAGE_PATH` overrides), so
reading an insight never produces a change to sync.

## Search

- Term search over the stored files is always available, with filters for
  topic, date range, exclusions, and result caps.
- `--semantic` adds lexical similarity matching (Jaccard word overlap plus
  term frequency); no model is involved.
- Neural vector search, LanceDB plus ONNX embeddings
  (`onnx-community/embeddinggemma-300m-ONNX`, fetched from Hugging Face at
  runtime), activates only in builds with `ml-features`; `insights index`
  recomputes the embeddings. Without the feature, search degrades to term
  matching.

## Feature flags

| Flag | Default | Description |
|------|---------|-------------|
| `ml-features` | off | LanceDB vector store + ONNX Runtime embeddings |
| `download-onnx-binaries` | off | Auto-download ONNX Runtime binaries; no-op without `ml-features` |

```bash
cargo build --features ml-features,download-onnx-binaries
```

Released binaries are built without ML features; build from source for neural
search.

## Environment

| Variable | Default | Purpose |
|----------|---------|---------|
| `INSIGHTS_ROOT` | XDG data dir | Storage root |
| `INSIGHTS_USAGE_PATH` | `~/.blizz/volatile/insights/usage.json` | This machine's usage counters |
| `INSIGHTS_SERVER_URL` | `http://127.0.0.1:2020` | Server address for the client, and the server's bind fallback |
| `INSIGHTS_TIMEOUT_SECS` | `30` | Client HTTP timeout |
| `INSIGHTS_LOG_LEVEL` | `info` | Server log level |
| `INSIGHTS_RERANK_INITIAL_LIMIT` | `128` | Vector-recall candidate pool before reranking |
| `INSIGHTS_RERANK_INITIAL_THRESHOLD` | `0.35` | Minimum vector similarity for a candidate |
| `INSIGHTS_RERANK_FINAL_LIMIT` | `7` | Results kept after reranking |

There is no config file; everything is flags and environment variables.

## Components

- **`insights`**, the CLI client
- **`insights_server`**, the REST API server (Axum) with JSONL daemon logs via
  bentley (`~/.blizz/persistent/insights/server-logs.jsonl`)
- **`install_insights_cuda_dependencies`**, the GPU setup helper for
  `ml-features` builds; Ubuntu/apt only, and skips itself everywhere else

The crate also ships as a library exposing the same surface (`insights::cli`,
`insights::server`).

## Development

The shellops toolchain lives in the `shell/.ops` submodule:

```bash
git clone --recurse-submodules <repo-url> && shell/.ops/init
```

`cargo test` runs the suite (tests mutate `INSIGHTS_ROOT` and run serially).
Pre-commit hooks enforce `cargo fmt` and `cargo clippy -- -D warnings`. Work
is tracked in Linear under the INS team.

## License

MIT
