# mempal — 设计文档

**日期**：2026-04-08
**状态**：Approved — ready for implementation planning

## 一句话定位

> `cargo install mempal` — 单二进制项目记忆工具，让任何 coding agent 在 10 秒内带出处找回历史决策。

## 设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 定位 | coding agent 项目记忆 | 锚定真实用户场景，不做通用 AI 记忆平台 |
| 语言 | Rust | 单二进制分发，零依赖，性能 |
| 存储 | SQLite + sqlite-vec | 单 .db 文件 = 整个记忆，备份即复制 |
| 嵌入 | ONNX 默认 + 可选外部 API | 离线优先，灵活可扩展 |
| 接口 | MCP + CLI + REST API | 覆盖所有 coding agent（Claude Code / Cursor / Gemini CLI / Codex / 管道） |
| AAAK | 输出格式化器，非存储编码器 | 数据永远 raw 存储，AAAK 只在输出侧可选压缩 |
| AAAK 实现 | 完整重做：BNF 语法 + 编码器 + 解码器 + 往返验证 | 修复 MemPalace 的实现缺陷 |

## 借鉴 MemPalace 的部分

| 概念 | 借鉴 | 改进 |
|------|------|------|
| Wing/Room 结构 | 语义分区降低检索难度（+34%） | 加可编辑 taxonomy + 路由置信度 |
| Verbatim 存储 | 原文不丢，搜索直接命中原文 | 不变 |
| 多格式归一化 | Claude JSONL / ChatGPT / Slack / 纯文本 | 同样支持，Rust 实现 |
| QA 对分块 | 对话按问答对切分 | 不变 |
| MCP 工具 | AI 通过 MCP 使用记忆 | 精简到必要工具，加路由透明性 |
| AAAK | 极度缩写的英语，LLM 即解码器 | 加形式语法、解码器、往返测试 |

## 不借鉴的部分

| MemPalace 概念 | 不做 | 理由 |
|---------------|------|------|
| Hall 分类层 | 不做 | 当前实现中非默认路径，增加复杂度无明确收益 |
| Closet 中间层 | 不做 | AAAK 在输出侧解决同样问题，不需要额外存储层 |
| Specialist agents | 不做 | v1 不需要，等用户确实有需求再做 |
| 矛盾检测 | 不做 | 依赖 KG 完整度，v1 聚焦搜索 |
| 时态知识图谱 | 保留 schema 但 v1 不做自动抽取 | schema 预留，手动写入，不做 LLM 自动抽取 |

## 架构

### Workspace 结构

```
mempal/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── mempal-core/              # 数据模型 + SQLite schema + taxonomy
│   ├── mempal-ingest/            # 导入管道：格式检测 → 归一化 → 分块 → 写入
│   ├── mempal-search/            # 搜索引擎：路由 → 向量检索 → 引用组装
│   ├── mempal-embed/             # 嵌入层：ONNX 默认 + 外部 API（trait 抽象）
│   ├── mempal-aaak/              # AAAK 编解码 + BNF 语法 + 往返验证
│   ├── mempal-mcp/               # MCP 服务器（rmcp）
│   ├── mempal-api/               # REST API（axum，feature-gated）
│   └── mempal-cli/               # CLI 入口（clap）
├── models/                       # ONNX 模型文件
├── docs/specs/                   # 设计文档
└── tests/                        # 集成测试
```

### 依赖关系

```
mempal-cli ──→ mempal-search
           ──→ mempal-ingest
           ──→ mempal-aaak (输出格式化，可选)

mempal-mcp ──→ mempal-search
           ──→ mempal-ingest
           ──→ mempal-aaak (输出格式化，可选)

mempal-api ──→ mempal-search
           ──→ mempal-ingest

mempal-search ──→ mempal-core
              ──→ mempal-embed

mempal-ingest ──→ mempal-core
              ──→ mempal-embed

mempal-aaak ──→ mempal-core (只依赖数据类型)
```

**关键约束**：`mempal-aaak` 不被 `mempal-ingest` 或 `mempal-search` 依赖。AAAK 的 bug 不影响存储和检索。

### 存储 Schema

单文件 `~/.mempal/palace.db`：

```sql
-- 文本内容 + 元数据（verbatim 存储）
CREATE TABLE drawers (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    wing TEXT NOT NULL,
    room TEXT,
    source_file TEXT,
    source_type TEXT CHECK(source_type IN ('project', 'conversation', 'manual')),
    added_at TEXT NOT NULL,
    chunk_index INTEGER
);

-- 向量索引（sqlite-vec）
CREATE VIRTUAL TABLE drawer_vectors USING vec0(
    id TEXT PRIMARY KEY,
    embedding FLOAT[384]
);

-- 时态知识图谱（schema 预留，v1 手动写入）
CREATE TABLE triples (
    id TEXT PRIMARY KEY,
    subject TEXT NOT NULL,
    predicate TEXT NOT NULL,
    object TEXT NOT NULL,
    valid_from TEXT,
    valid_to TEXT,
    confidence REAL DEFAULT 1.0,
    source_drawer TEXT REFERENCES drawers(id)
);

-- 可编辑 taxonomy
CREATE TABLE taxonomy (
    wing TEXT NOT NULL,
    room TEXT NOT NULL DEFAULT '',
    display_name TEXT,
    keywords TEXT,  -- JSON array
    PRIMARY KEY (wing, room)
);

-- 索引
CREATE INDEX idx_drawers_wing ON drawers(wing);
CREATE INDEX idx_drawers_wing_room ON drawers(wing, room);
CREATE INDEX idx_triples_subject ON triples(subject);
CREATE INDEX idx_triples_object ON triples(object);
```

### 嵌入层

```rust
#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    fn dimensions(&self) -> usize;
    fn name(&self) -> &str;
}

// 默认：内嵌 ONNX MiniLM
pub struct OnnxEmbedder { session: ort::Session }

// 可选：外部 API
pub struct ApiEmbedder { endpoint: String, api_key: Option<String> }
```

配置文件选择：

```toml
# ~/.mempal/config.toml
[embed]
backend = "onnx"  # 或 "api"
# api_endpoint = "http://localhost:11434/api/embeddings"
# api_model = "nomic-embed-text"
```

### 搜索路由

```rust
pub struct RouteDecision {
    pub wing: Option<String>,
    pub room: Option<String>,
    pub confidence: f32,
    pub reason: String,
}

pub fn route_query(query: &str, taxonomy: &[TaxonomyEntry]) -> RouteDecision {
    // 1. taxonomy 关键词匹配（keywords JSON array）
    // 2. 已知实体名匹配
    // 3. confidence < 0.5 → 退回全局搜索
}
```

路由结果透明返回，agent 可覆盖。

### 搜索结果（强制带引用）

```rust
pub struct SearchResult {
    pub drawer_id: String,
    pub content: String,
    pub wing: String,
    pub room: Option<String>,
    pub source_file: Option<String>,
    pub similarity: f32,
    pub route: RouteDecision,
}
```

### AAAK 架构位置

```
数据流：
  存储侧：原始文本 → 分块 → 嵌入 → SQLite    （不经过 AAAK）
  输出侧：搜索/wake-up → raw 输出（默认）
                        → AAAK 格式化（可选）    （经过 mempal-aaak）
```

AAAK BNF 语法：

```bnf
document    ::= header NEWLINE body
header      ::= "V" version "|" wing "|" room "|" date "|" source
body        ::= line (NEWLINE line)*
line        ::= zettel | tunnel | arc
zettel      ::= zid ":" entities "|" topics "|" quote "|" weight "|" emotions "|" flags
tunnel      ::= "T:" zid "<->" zid "|" label
arc         ::= "ARC:" emotion ("->" emotion)*
entities    ::= entity_code ("+" entity_code)*
entity_code ::= UPPER{3}
emotions    ::= emotion_code ("+" emotion_code)*
emotion_code::= LOWER{3,7}
flags       ::= flag ("+" flag)*
flag        ::= "DECISION" | "ORIGIN" | "CORE" | "PIVOT" | "TECHNICAL" | "SENSITIVE"
weight      ::= "★"{1,5}
quote       ::= '"' TEXT '"'
topics      ::= topic ("_" topic)*
version     ::= DIGIT+
```

AAAK 编解码器必须通过往返测试：

```rust
#[test]
fn roundtrip_preserves_facts() {
    let codec = AaakCodec::default();
    let original = "Kai recommended Clerk over Auth0 based on pricing and DX";
    let encoded = codec.encode(original, &meta);
    let report = codec.verify_roundtrip(original, &encoded);
    assert!(report.coverage >= 0.9);  // 至少 90% 事实断言保留
}
```

### 接口

**CLI**：
```bash
mempal init <dir>                          # 初始化 taxonomy
mempal ingest <dir>                        # 导入项目文件
mempal ingest <dir> --format convos        # 导入对话
mempal search "why clerk over auth0"       # 搜索（默认 raw 输出）
mempal search "auth decision" --wing myapp # 指定 wing
mempal wake-up                             # raw L0+L1
mempal wake-up --format aaak              # AAAK 压缩输出
mempal taxonomy list                       # 查看分类
mempal taxonomy edit <wing> <room>         # 编辑分类
mempal status                              # 宫殿概览
mempal serve                               # 启动 MCP + REST
```

**MCP 工具**（精简）：
- `mempal_status` — 宫殿概览 + taxonomy
- `mempal_search` — 带出处的语义搜索
- `mempal_ingest` — 保存新内容
- `mempal_taxonomy` — 查看/修改分类

**REST API**（feature-gated `--features rest`）：
```
GET  /api/search?q=&wing=&room=
POST /api/ingest
GET  /api/taxonomy
PUT  /api/taxonomy/:wing/:room
GET  /api/status
```

### 关键依赖

| 功能 | Crate | 版本约束 |
|------|-------|---------|
| CLI | clap | 4.x |
| SQLite | rusqlite | bundled feature |
| 向量索引 | sqlite-vec | — |
| 嵌入 | ort | 2.x |
| MCP | rmcp | — |
| REST | axum | 0.8+ |
| 异步 | tokio | 1.x, features=["full"] |
| 序列化 | serde + serde_json | — |
| 文件监控 | notify | 7.x（v2 用） |
| 错误 | anyhow + thiserror | — |
| 配置 | toml | — |
| 双向 map | bimap | —（AAAK 实体编码） |

### 实现优先级

| Phase | 范围 | 核心交付 |
|-------|------|---------|
| **P0** | mempal-core + mempal-embed + mempal-cli（init/ingest/search） | 能跑通 init → ingest → search |
| **P1** | mempal-search 路由 + 引用组装 | 搜索结果带出处、路由可解释 |
| **P2** | mempal-mcp | Claude Code 可用 |
| **P3** | mempal-aaak | wake-up --format aaak + 往返验证 |
| **P4** | mempal-api | REST API |

### 不做的事（YAGNI）

- 不做 Web UI（v1 只有 CLI + MCP + REST）
- 不做团队协作/权限
- 不做自动 KG 抽取（schema 预留，手动写入）
- 不做 specialist agent 系统
- 不做矛盾检测
- 不做 Hall 分类层
- 不做 Closet 存储层
