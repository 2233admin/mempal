# mempal

Rust implementation of a project memory tool for coding agents.

`mempal` stores raw project memory in SQLite, indexes embeddings with `sqlite-vec`, and lets agents recover prior decisions with citations in a few commands. The current repository includes the full P0-P4 scope: CLI, ingest pipeline, vector search, routing, MCP server, AAAK formatting, and a feature-gated REST API.

## What It Does

- Stores raw memory drawers in a single SQLite database at `~/.mempal/palace.db` by default.
- Embeds content with a pluggable `Embedder` abstraction.
- Uses ONNX locally by default with `all-MiniLM-L6-v2`; the model and tokenizer are downloaded on first use.
- Searches with required citations: every result includes `drawer_id` and `source_file`.
- Routes queries through taxonomy-aware `wing` and `room` scopes.
- Exposes the same memory through CLI, MCP, and optional REST interfaces.
- Supports AAAK compression as an output-side formatter instead of a storage format.

## Quick Start

Local install:

```bash
cargo install --path crates/mempal-cli --locked
```

Install with REST support:

```bash
cargo install --path crates/mempal-cli --locked --features rest
```

Index a project and search it:

```bash
mempal init ~/code/myapp
mempal ingest ~/code/myapp --wing myapp
mempal search "auth decision clerk" --json
mempal wake-up
```

Typical output flow:

```bash
mempal taxonomy list
mempal taxonomy edit myapp auth --keywords "auth,login,clerk"
mempal search "how did we decide auth?" --wing myapp
mempal wake-up --format aaak
```

## Configuration

Config is loaded from `~/.mempal/config.toml`. If the file is missing, mempal uses built-in defaults.

Default behavior:

- `db_path = "~/.mempal/palace.db"`
- `embed.backend = "onnx"`
- `embed.api_endpoint = None`
- `embed.api_model = None`

Example config:

```toml
db_path = "~/.mempal/palace.db"

[embed]
backend = "onnx"
```

Switch to an external embedding API:

```toml
db_path = "~/.mempal/palace.db"

[embed]
backend = "api"
api_endpoint = "http://localhost:11434/api/embeddings"
api_model = "nomic-embed-text"
```

## Command Overview

`mempal` currently exposes these subcommands:

- `init`: infer taxonomy rooms from a project tree and seed the taxonomy table.
- `ingest`: detect files, normalize content, chunk, embed, and store drawers.
- `search`: vector search with optional `wing` and `room` filters.
- `wake-up`: emit a short memory summary for agent context refresh.
- `compress`: convert arbitrary text into AAAK output.
- `taxonomy`: list or edit taxonomy entries.
- `serve`: run MCP stdio, and with `rest` enabled also run the local REST API.
- `status`: print drawer counts, taxonomy counts, DB size, and per-scope counts.

For exact CLI syntax:

```bash
mempal --help
mempal serve --help
```

## Interfaces

### CLI

The CLI is the primary interface for local indexing and search.

```bash
mempal search "database decision postgresql analytics" --json --wing myproject
```

### MCP

`mempal serve --mcp` runs the MCP server over stdio.

Available tools:

- `mempal_status`
- `mempal_search`
- `mempal_ingest`
- `mempal_taxonomy`

If mempal is built without the `rest` feature, plain `mempal serve` also runs MCP stdio only.

### REST

Build with `--features rest` to enable the REST server.

With the `rest` feature enabled:

- `mempal serve` starts MCP stdio and REST together.
- REST binds to `127.0.0.1:3080`.
- CORS only allows localhost origins.

Endpoints:

- `GET /api/status`
- `GET /api/search?q=...&wing=...&room=...&top_k=...`
- `POST /api/ingest`
- `GET /api/taxonomy`

Example:

```bash
curl 'http://127.0.0.1:3080/api/status'
curl 'http://127.0.0.1:3080/api/search?q=clerk&wing=myapp'
curl -X POST 'http://127.0.0.1:3080/api/ingest' \
  -H 'content-type: application/json' \
  -d '{"content":"decided to use Clerk","wing":"myapp","room":"auth"}'
```

## Architecture Notes

- Storage is always raw-first: drawer text lives in `drawers`, vectors live in `drawer_vectors`.
- AAAK is output-only and is not part of ingest or search internals.
- Search results are citation-bearing by construction.
- Routing is deterministic and explainable through `route.reason` and `route.confidence`.
- The repository is organized as a workspace:

| Crate | Responsibility |
| --- | --- |
| `mempal-core` | Types, config, SQLite schema, taxonomy access |
| `mempal-embed` | `Embedder` trait, ONNX embedder, API embedder |
| `mempal-ingest` | Detection, normalization, chunking, ingest pipeline |
| `mempal-search` | Vector search, filtering, query routing |
| `mempal-aaak` | AAAK encode/decode and roundtrip verification |
| `mempal-mcp` | MCP server with four tools |
| `mempal-api` | Feature-gated REST API |
| `mempal-cli` | CLI entrypoint |

## Development

Common verification commands:

```bash
cargo test --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
```

Useful docs in this repo:

- Design: [`docs/specs/2026-04-08-mempal-design.md`](docs/specs/2026-04-08-mempal-design.md)
- Usage guide: [`docs/usage.md`](docs/usage.md)
- Specs: [`specs/`](specs)
- Implementation plans: [`docs/plans/`](docs/plans)
