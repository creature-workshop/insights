# insights

A drop-in, IDE-agnostic AI knowledge management and retrieval system. An
insight is a small markdown document — an overview plus details, filed under a
topic — that agents and humans store, search, and retrieve through a local
server.

## Components

- **`insights`** — CLI client; health-checks the server before every command
  and spawns it on demand
- **`insights_server`** — REST API server (Axum) with JSONL daemon logs via
  bentley (`~/.blizz/persistent/insights/server-logs.jsonl`)
- **`install_insights_cuda_dependencies`** — GPU setup helper for
  `ml-features` builds; Ubuntu/apt only, and skips itself everywhere else

The crate also ships as a library exposing the same surface (`insights::cli`,
`insights::server`).

## Usage

```bash
insights add <topic> <name> <overview> <details>
insights search <term>... [-t <topic>] [--sort relevance|updated|created|least-accessed]
insights get <topic> <name>
insights list [topic] [--pinned]
insights update <topic> <name> [-o <overview>] [-d <details>]
insights pin <topic> <name>
insights topics
insights setup
```

The CLI starts `insights_server` automatically when it isn't running, so there
is no separate daemon step. Run by hand, the server binds `127.0.0.1:2020`;
override with `--bind` or `INSIGHTS_SERVER_URL`. The REST surface is defined
in `src/server/routing.rs`.

`insights setup` runs an interactive wizard that writes the shell init file,
optionally hooks it into your shell rc, and installs the use-insights rule for
supported agents (Cursor, Claude Code, Windsurf, Zed).

## Storage

Insights live on disk as `<root>/<topic>/<name>.insight.md`: YAML frontmatter,
a details body, and a trailing metadata block tracking usage. Update,
retrieval, and search-hit counts feed relevance ranking, and pinned insights
rank first. The root is `$INSIGHTS_ROOT` when set, otherwise
`$XDG_DATA_HOME/insights` (typically `~/.local/share/insights`), or
`~/.blizz/persistent/insights` where that directory already exists.

## Search

- Term search over the stored files is always available, with filters for
  topic, date range, exclusions, and result caps.
- `--semantic` adds lexical similarity matching (Jaccard word overlap plus
  term frequency) — no model involved.
- Neural vector search — LanceDB plus ONNX embeddings
  (`onnx-community/embeddinggemma-300m-ONNX`, fetched from Hugging Face at
  runtime) — activates only in builds with `ml-features`. Without the
  feature, search degrades to term matching.

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
| `INSIGHTS_SERVER_URL` | `http://127.0.0.1:2020` | Server address for the client, and the server's bind fallback |
| `INSIGHTS_TIMEOUT_SECS` | `30` | Client HTTP timeout |
| `INSIGHTS_LOG_LEVEL` | `info` | Server log level |
| `INSIGHTS_RERANK_INITIAL_LIMIT` | `128` | Vector-recall candidate pool before reranking |
| `INSIGHTS_RERANK_INITIAL_THRESHOLD` | `0.35` | Minimum vector similarity for a candidate |
| `INSIGHTS_RERANK_FINAL_LIMIT` | `7` | Results kept after reranking |

There is no config file; everything is flags and environment variables.

## Development

The shellops toolchain lives in the `shell/.ops` submodule:

```bash
git clone --recurse-submodules <repo-url> && shell/.ops/init
```

`cargo test` runs the suite (tests mutate `INSIGHTS_ROOT` and run serially).
Pre-commit hooks enforce `cargo fmt` and `cargo clippy -- -D warnings`.

## License

MIT
