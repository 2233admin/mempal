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
mempal ingest ~/code/myapp --wing myapp --dry-run
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
- `ingest`: detect files, normalize content, chunk, embed, and store drawers. `--dry-run` previews file/chunk/skip counts without writing drawers or vectors.
- `search`: vector search with optional `wing` and `room` filters.
- `wake-up`: emit a short memory summary for agent context refresh.
- `compress`: convert arbitrary text into AAAK output.
- `bench`: run benchmark adapters against external evaluation datasets.
- `taxonomy`: list or edit taxonomy entries.
- `serve`: run MCP stdio, and with `rest` enabled also run the local REST API.
- `status`: print drawer counts, taxonomy counts, DB size, and per-scope counts.
- `status`: print schema version, drawer counts, taxonomy counts, DB size, and per-scope counts.

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

### Benchmarking

`mempal` can run a native LongMemEval harness while reusing the same dataset shape and retrieval metrics documented in `mempalace`.

```bash
mempal bench longmemeval /path/to/longmemeval_s_cleaned.json
mempal bench longmemeval /path/to/longmemeval_s_cleaned.json --mode aaak
mempal bench longmemeval /path/to/longmemeval_s_cleaned.json --mode rooms --limit 20
mempal bench longmemeval /path/to/longmemeval_s_cleaned.json --granularity turn --out benchmarks/results_longmemeval.jsonl
```

Supported modes:

- `raw`: ingest raw user text
- `aaak`: ingest AAAK-formatted text, query with raw questions
- `rooms`: install benchmark taxonomy rooms and let `mempal` route by taxonomy

### MCP

`mempal serve --mcp` runs the MCP server over stdio.

Available tools:

- `mempal_status` — returns counts, DB size, scope breakdown, dynamically generated `aaak_spec`, and `memory_protocol` (a behavioral guide teaching the AI when to search/save). AI learns its own workflow on first call; zero system prompt configuration.
- `mempal_search` — vector search with optional wing/room filters, every result carries `drawer_id` + `source_file`
- `mempal_ingest` — store a single memory drawer from raw content
- `mempal_taxonomy` — list or edit taxonomy entries

If mempal is built without the `rest` feature, plain `mempal serve` also runs MCP stdio only.

### Memory Protocol and Identity

mempal teaches AI agents their workflow through two self-describing outputs:

1. **Memory protocol** — embedded in `mempal_status` response and `mempal wake-up` output. Tells the AI when to verify facts, when to save decisions, and how to cite sources.
2. **L0 identity** — read from `~/.mempal/identity.txt`. A user-edited plain text file describing role, working style, and key projects. Loaded into wake-up output automatically.

Create an identity file (optional but recommended):

```bash
mkdir -p ~/.mempal
$EDITOR ~/.mempal/identity.txt
```

Example content:

```
Role: Rust backend engineer at Acme.
Current focus: auth rewrite, Clerk migration.
Working style: small reversible edits, verify before asserting.
```

### Optional: Claude Code Hooks

For AIs that forget to save proactively, mempal ships reference hook scripts in `hooks/`:

- `hooks/mempal_save_hook.sh` — a `Stop` hook that reminds the AI to save decisions every Nth conversation turn (configurable via `MEMPAL_SAVE_INTERVAL`, default 10).
- `hooks/mempal_precompact_hook.sh` — a `PreCompact` hook that forces an emergency save before context compression.

Both hooks are **optional**. mempal works without them — the memory protocol embedded in `mempal_status` is the primary mechanism for teaching the AI to self-manage memory. The hook scripts exist as a safety net.

Install by adding to `~/.claude/settings.json` or project-level `.claude/settings.local.json`:

```json
{
  "hooks": {
    "Stop": [{
      "matcher": "*",
      "hooks": [{"type": "command", "command": "/absolute/path/to/mempal/hooks/mempal_save_hook.sh"}]
    }],
    "PreCompact": [{
      "hooks": [{"type": "command", "command": "/absolute/path/to/mempal/hooks/mempal_precompact_hook.sh"}]
    }]
  }
}
```

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

## AAAK Format

AAAK is a compressed memory dialect readable by any LLM without decoding. It is **output-only** — raw text always stays in the drawer, AAAK just reformats it for compact context windows.

### Format Example

```
V1|myapp|auth|2026-04-08|readme
0:KAI+CLK|clerk_auth|"Kai recommended Clerk over Auth0 based on pricing"|★★★★|determ|DECISION
```

Each line is a Zettel (memory card) with pipe-separated fields: entity codes, topics, quoted content, importance stars, emotion, and semantic flags. Documents can also include `T:` (tunnel/link) and `ARC:` (emotion arc) lines.

### AAAK CLI Usage

```bash
# Compress arbitrary text
mempal compress "We chose Clerk over Auth0 because pricing was better"

# Wake-up summary in AAAK format
mempal wake-up --format aaak
```

### Chinese Support

AAAK uses **jieba-rs** for real Chinese word segmentation and POS tagging — not bigram heuristics. It correctly identifies person names, places, organizations, and content words:

```bash
mempal compress "阿里巴巴集团在杭州发布了新的云服务产品"
# → entities: 阿里巴巴 (nz), 杭州 (ns)
# → topics: 集团, 发布, 服务

mempal compress "张三推荐Clerk替换Auth0，因为价格更优"
# → entities: 张三 (nr), CLK, AUT
# → topics: 推荐, 替换, 价格
```

Mixed Chinese-English text, fullwidth punctuation, and Chinese emotion/flag keywords (决定, 架构, 部署, etc.) all work naturally. Jieba's dictionary is lazy-loaded on first use.

For the full format spec, see [`docs/aaak-dialect.md`](docs/aaak-dialect.md).

## Architecture Notes

- Storage is always raw-first: drawer text lives in `drawers`, vectors live in `drawer_vectors`.
- SQLite schema version is tracked via `PRAGMA user_version`; opening the database applies bundled forward migrations up to the current binary's supported version.
- AAAK is output-only and is not part of ingest or search internals.
- Search results are citation-bearing by construction.
- `source_file` values are stored relative to the ingest root, so re-ingesting the same tree via absolute or relative paths stays citation-stable.
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
- AAAK dialect: [`docs/aaak-dialect.md`](docs/aaak-dialect.md)
- Specs: [`specs/`](specs)
- Implementation plans: [`docs/plans/`](docs/plans)
