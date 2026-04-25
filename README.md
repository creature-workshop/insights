# insights

Knowledge management and insight storage system. Store, search, and retrieve structured insights with optional neural embedding support.

## Components

- **`insights`** — CLI client for managing insights
- **`insights_server`** — REST API server with JSONL logging
- **`install_insights_cuda_dependencies`** — GPU setup helper for ONNX/CUDA inference

## Features

- Markdown-based insight storage organized by topic
- Full-text and semantic search
- REST API with Axum
- Optional neural embeddings via ONNX Runtime (`ml-features`)
- Optional LanceDB vector database for similarity search
- Daemon log infrastructure via bentley

## Usage

```bash
# Start the server
insights_server --bind 127.0.0.1:2020

# CLI commands
insights search "rust async patterns"
insights add --topic rust --name async-patterns
insights list
```

## Feature Flags

| Flag | Default | Description |
|------|---------|-------------|
| `ml-features` | no | Neural embeddings, LanceDB, ONNX Runtime |
| `download-onnx-binaries` | no | Auto-download ONNX Runtime binaries |

Enable ML features for neural embedding search:

```bash
cargo build --features ml-features,download-onnx-binaries
```

## License

MIT
