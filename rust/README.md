# dsh-rust — DeepSeek Harness for Rust

A high-performance, type-safe AI agent harness framework built in Rust.

## Overview

dsh-rust provides a complete toolkit for building, deploying, and managing AI agents with:

- **Multi-LLM Support** — DeepSeek, OpenAI, Anthropic with structured function calling
- **Type-Safe Agent Loop** — Compile-time guarantees for session log invariants
- **Real Agent Tools** — bash, file read/write/edit, search, list files
- **Multi-Agent Orchestration** — Sequential, fan-out, and fan-in patterns
- **Workflow Engine** — DAG-based multi-step workflows with failure propagation
- **Agent Memory** — Long-term memory with semantic vector search
- **Production Ready** — Auth, rate limiting, quotas, observability, distributed sessions
- **Cross-Platform** — macOS, Linux, Windows sandbox support

## Quick Start

### Prerequisites

- Rust 1.77+ (stable toolchain)
- macOS, Linux, or Windows

### Build

```bash
cd rust
cargo build
```

### Run

```bash
# Interactive mode
cargo run -p dsh-headless -- --interactive --api-key sk-xxx

# Single task
cargo run -p dsh-headless -- --task "What is 2+2?" --api-key sk-xxx

# Web server
cargo run -p dsh-web -- --api-key sk-xxx --port 3000
```

### Test

```bash
cargo test --workspace           # All tests (396)
cargo test -p dsh-e2e-tests      # E2E integration tests
cargo bench --workspace          # Performance benchmarks
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  L4 Surface                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  dsh-headless│  │   dsh-web    │  │  Python/Node │  │
│  │     CLI      │  │  HTTP+SSE    │  │    SDKs      │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
├─────────────────────────────────────────────────────────┤
│  L3 Composition                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  bundle-base │  │ bundle-web   │  │bundle-headless│ │
│  │  (feature    │  │              │  │              │  │
│  │   flags)     │  │              │  │              │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
├─────────────────────────────────────────────────────────┤
│  L2 Capability Seams                                    │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ ┌──────┐ │
│  │sandbox │ │   fs   │ │subproc │ │persist │ │ auth │ │
│  │mac/linux│ │        │ │        │ │json/sql│ │rbac  │ │
│  │windows │ │        │ │        │ │        │ │      │ │
│  └────────┘ └────────┘ └────────┘ └────────┘ └──────┘ │
├─────────────────────────────────────────────────────────┤
│  L1 Substrate                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  core-session│  │  core-tools  │  │core-agent-loop│ │
│  │  (invariant) │  │  (pipeline)  │  │  (state mach) │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ llm-adapter  │  │llm-deepseek  │  │ llm-openai   │ │
│  │   (trait)    │  │llm-anthropic │  │              │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
├─────────────────────────────────────────────────────────┤
│  L0 Effect & Context                                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ core-context │  │core-evolution│  │core-orchestr.│  │
│  │ (ScopeGuard) │  │  (Gene/Caps) │  │  (multi-agn) │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│  ┌──────────────┐  ┌──────────────┐                     │
│  │ core-workflow│  │ core-memory  │                     │
│  │    (DAG)     │  │  (vector)    │                     │
│  └──────────────┘  └──────────────┘                     │
└─────────────────────────────────────────────────────────┘
```

## Crate Map (55 crates)

### Core Runtime (9 crates)

| Crate | LOC | Tests | Purpose |
|-------|-----|-------|---------|
| `core-context` | 617 | 17 | Service container, ScopeGuard, EventBus |
| `core-session` | 251 | 6 | Session log, invariant assertion |
| `core-tools` | 153 | 2 | Tool trait, pipeline middleware |
| `core-agent-loop` | 657 | 3 | Agent turn state machine |
| `core-orchestrator` | 539 | 5 | Multi-agent orchestration |
| `core-workflow` | 520 | 7 | DAG workflow engine |
| `core-memory` | 526 | 15 | Agent memory + semantic search |
| `core-evolution` | 473 | 8 | Gene/Capsule evolution |
| `core-plugin` | 516 | 7 | Plugin system, hot reload |

### LLM Providers (4 crates)

| Crate | LOC | Tests | Purpose |
|-------|-----|-------|---------|
| `llm-adapter` | 224 | 1 | LLM trait definition |
| `llm-deepseek` | 412 | 6 | DeepSeek SSE streaming + tool_calls |
| `llm-openai` | 417 | 6 | OpenAI tool_calls support |
| `llm-anthropic` | 560 | 8 | Anthropic tool_use content blocks |

### Data Layer (9 crates)

| Crate | LOC | Tests | Purpose |
|-------|-----|-------|---------|
| `core-eventstore` | 588 | 10 | Event sourcing + CQRS + message bus |
| `core-cqrs` | 161 | 2 | Command/Query separation |
| `core-saga` | 327 | 3 | Saga pattern, compensation |
| `core-vectordb` | 301 | 6 | Vector storage + embedding + learning |
| `core-knowledge` | 396 | 6 | Knowledge graph + reasoning |
| `core-crossmodal` | 248 | 4 | Multimodal + cross-modal retrieval |
| `core-rag` | 154 | 1 | RAG pipeline |
| `core-incremental` | 223 | 4 | Incremental computation + streaming |
| `core-federated` | 145 | 2 | Federated learning |

### Capability Seams (27 crates)

| Crate | LOC | Tests | Purpose |
|-------|-----|-------|---------|
| `seam-auth` | 639 | 15 | API key auth, RBAC, rate limiting, quota |
| `seam-sandbox` | 533 | 12 | macOS/Linux/Windows sandbox |
| `seam-persistence` | 543 | 7 | JSONL + SQLite session store |
| `seam-observability` | 617 | 8 | Metrics + tracing |
| `seam-tools` | 546 | 11 | bash/read/write/edit/search/list tools |
| `seam-multitenant` | 481 | 8 | Tenant isolation |
| `seam-gateway` | 451 | 9 | API Gateway + routing |
| `seam-traffic` | 339 | 8 | Priority routing + throttling |
| `seam-resilience` | 415 | 11 | CircuitBreaker + RetryPolicy |
| `seam-cache` | 370 | 11 | LRU/TTL cache |
| `seam-canary` | 376 | 10 | Canary deployment |
| `seam-scheduler` | 642 | 15 | Task scheduling + delayed queue |
| `seam-migration` | 842 | 13 | Database migration + backup |
| `seam-monitor` | 569 | 12 | Latency + throughput + resources |
| `seam-autoscale` | 328 | 10 | Auto-scaling |
| `seam-tracing` | 662 | 19 | Distributed tracing + logging |
| `seam-grpc` | 280 | 7 | gRPC + service mesh |
| `seam-edge` | 260 | 5 | Cluster + edge computing |
| `seam-multiregion` | 213 | 4 | Multi-region + CDN |
| `seam-compliance` | 174 | 2 | Compliance audit |
| `seam-session-distributed` | 316 | 4 | Distributed session store |
| `seam-media` | 126 | 3 | Image/audio processing |
| `seam-fs` | 96 | 1 | Filesystem abstraction |
| `seam-subprocess` | 83 | 3 | Subprocess execution |
| `seam-a11y` | 310 | 8 | Accessibility (WCAG) |
| `seam-mobile` | 327 | 8 | Mobile adaptation |
| `core-i18n` | 387 | 9 | Internationalization |

### Application Layer (5 crates)

| Crate | LOC | Tests | Purpose |
|-------|-----|-------|---------|
| `bundle-base` | 299 | 5 | Base assembly with feature flags |
| `bundle-web-app` | 283 | 3 | axum HTTP surface |
| `bundle-headless` | 23 | 0 | CLI runner |
| `mcp-client` | 131 | 1 | MCP tool adapter |
| `test-support` | 73 | 0 | Mock LLM + fixtures |

## Feature Flags

The `bundle-base` crate supports compile-time feature selection:

```toml
[features]
default = ["llm-deepseek", "sandbox", "persistence-jsonl"]

llm-deepseek = []      # DeepSeek provider
llm-openai = []        # OpenAI provider
llm-anthropic = []     # Anthropic provider
sandbox = []           # Sandbox capability
persistence-jsonl = [] # JSONL persistence
persistence-sqlite = [] # SQLite persistence
mcp = []               # MCP client
```

Build with specific features:

```bash
cargo build -p dsh-bundle-base --features "llm-openai,persistence-sqlite"
```

## Testing

### Test Pyramid

| Tier | Count | Tool | Purpose |
|------|-------|------|---------|
| Unit | 396 | `cargo test` | Per-crate correctness |
| E2E | 22 | `cargo test -p dsh-e2e-tests` | Full-stack integration |
| Snapshot | 3 | `insta` | Agent output regression |
| Benchmark | 15 | `criterion` | Performance baselines |

### Run All Tests

```bash
cargo test --workspace
```

### Run Benchmarks

```bash
cargo bench --workspace
# Results in target/criterion/reports/index.html
```

### Coverage

```bash
cargo install cargo-llvm-cov
./scripts/coverage.sh --open
```

## License

MIT
