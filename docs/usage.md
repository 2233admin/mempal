# mempal Usage Guide

This guide focuses on the behavior that exists in the repository today: local CLI workflows, MCP usage, AAAK output, and the optional REST API.

## Install

Local CLI install:

```bash
cargo install --path crates/mempal-cli --locked
```

Install with REST support:

```bash
cargo install --path crates/mempal-cli --locked --features rest
```

For development without installation:

```bash
cargo run -p mempal-cli -- --help
cargo run -p mempal-cli --features rest -- serve --help
```

## Configure

Config file path:

```text
~/.mempal/config.toml
```

Defaults:

```toml
db_path = "~/.mempal/palace.db"

[embed]
backend = "onnx"
```

Use an external embedding service instead of ONNX:

```toml
db_path = "~/.mempal/palace.db"

[embed]
backend = "api"
api_endpoint = "http://localhost:11434/api/embeddings"
api_model = "nomic-embed-text"
```

Notes:

- ONNX is the default backend.
- First ONNX use downloads `all-MiniLM-L6-v2` model assets.
- If `config.toml` is missing, mempal still works with defaults.

## Initialize A Project

`init` scans the project tree, infers room names from directories, and seeds taxonomy entries.

```bash
mempal init ~/code/myapp
```

Typical output:

```text
wing: myapp
rooms:
- auth
- deploy
```

## Ingest Memory

Ingest a project tree into one `wing`:

```bash
mempal ingest ~/code/myapp --wing myapp
```

The current CLI accepts `--format convos` as an optional explicit format selector, but normal usage can omit it:

```bash
mempal ingest ~/code/myapp --wing myapp --format convos
```

The command reports file, chunk, and skip counts:

```text
files=12 chunks=34 skipped=2
```

## Search

Basic search:

```bash
mempal search "auth decision clerk"
```

Search with JSON output:

```bash
mempal search "auth decision clerk" --json
```

Search within a wing:

```bash
mempal search "database decision" --wing myapp
```

Search within a wing and room:

```bash
mempal search "token refresh bug" --wing myapp --room auth
```

What you get back:

- `drawer_id`
- `content`
- `wing`
- `room`
- `source_file`
- `similarity`
- `route`

`route` explains whether the query used explicit filters or taxonomy routing.

## Wake-Up Summaries

Default wake-up output is a compact context refresh for agents:

```bash
mempal wake-up
```

AAAK-formatted wake-up:

```bash
mempal wake-up --format aaak
```

## AAAK Compression

Compress arbitrary text into AAAK format:

```bash
mempal compress "Kai recommended Clerk over Auth0 based on pricing and DX"
```

AAAK is an output formatter only. It does not replace raw drawer storage in SQLite.

## Taxonomy

List taxonomy entries:

```bash
mempal taxonomy list
```

Edit or add taxonomy keywords:

```bash
mempal taxonomy edit myapp auth --keywords "auth,login,clerk"
```

This improves automatic routing for future searches.

## Status

Show storage stats:

```bash
mempal status
```

The command reports:

- total drawer count
- taxonomy entry count
- DB file size
- per-`wing` and per-`room` drawer counts

## Serve MCP And REST

### MCP-only mode

Run stdio MCP explicitly:

```bash
mempal serve --mcp
```

If mempal was built without the `rest` feature, plain `mempal serve` also behaves this way.

### MCP + REST mode

Build with REST enabled and start both interfaces:

```bash
mempal serve
```

Behavior with `--features rest`:

- MCP runs over stdio.
- REST listens on `127.0.0.1:3080`.
- CORS allows localhost origins only.

### MCP Tool Names

The server exposes four tools:

- `mempal_status`
- `mempal_search`
- `mempal_ingest`
- `mempal_taxonomy`

Example request shapes:

```json
{
  "query": "auth decision clerk",
  "wing": "myapp",
  "room": "auth",
  "top_k": 5
}
```

```json
{
  "content": "decided to use Clerk for auth",
  "wing": "myapp",
  "room": "auth",
  "source": "/repo/README.md"
}
```

```json
{
  "action": "edit",
  "wing": "myapp",
  "room": "auth",
  "keywords": ["auth", "login", "clerk"]
}
```

## REST API

### `GET /api/status`

```bash
curl 'http://127.0.0.1:3080/api/status'
```

Example response:

```json
{
  "drawer_count": 1,
  "taxonomy_count": 1,
  "db_size_bytes": 1658880,
  "wings": [
    {
      "wing": "myapp",
      "room": null,
      "drawer_count": 1
    }
  ]
}
```

### `GET /api/search`

```bash
curl 'http://127.0.0.1:3080/api/search?q=clerk&wing=myapp'
```

Supported query params:

- `q`
- `wing`
- `room`
- `top_k`

### `POST /api/ingest`

```bash
curl -X POST 'http://127.0.0.1:3080/api/ingest' \
  -H 'content-type: application/json' \
  -d '{"content":"decided to use Clerk","wing":"myapp","room":"auth"}'
```

Example response:

```json
{
  "drawer_id": "drawer_myapp_auth_1234abcd"
}
```

### `GET /api/taxonomy`

```bash
curl 'http://127.0.0.1:3080/api/taxonomy'
```

Returns the current `wing`/`room` taxonomy entries with keywords.

## Verify Changes

After modifying behavior, the repo currently uses these commands for validation:

```bash
cargo test --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-features -- -D warnings
```
