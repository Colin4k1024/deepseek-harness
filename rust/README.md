# dsh-rust — DeepSeek Harness for Rust

A high-performance, type-safe AI agent harness framework built in Rust.

## Overview

dsh-rust provides a complete toolkit for building, deploying, and managing AI agents with:

- **Multi-LLM Support** — DeepSeek, OpenAI, Anthropic, and any OpenAI-compatible endpoint
- **Type-Safe Agent Loop** — Compile-time guarantees for session log invariants
- **Tool Ecosystem** — MCP client integration, extensible tool trait
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
# Headless CLI
cargo run -p dsh-headless -- --task "What is 2+2?" --api-key sk-xxx

# Web server
cargo run -p dsh-web -- --api-key sk-xxx --port 3000
```

### Test

```bash
cargo test --workspace           # All tests
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
│  │   (trait)    │  │llm-anthropic │  │              │  │
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

## Crate Map

| Crate | Layer | Purpose |
|-------|-------|---------|
| `core-context` | L0 | Service container, ScopeGuard, EventBus |
| `core-session` | L1 | Session log, invariant assertion |
| `core-tools` | L1 | Tool trait, pipeline middleware |
| `core-agent-loop` | L1 | Agent turn state machine |
| `llm-adapter` | L1 | LLM trait definition |
| `llm-deepseek` | L1 | DeepSeek API provider |
| `llm-openai` | L1 | OpenAI + compatible endpoints |
| `llm-anthropic` | L1 | Anthropic Claude provider |
| `seam-sandbox` | L2 | macOS/Linux/Windows sandbox |
| `seam-fs` | L2 | Filesystem capability |
| `seam-subprocess` | L2 | Subprocess capability |
| `seam-persistence` | L2 | JSONL + SQLite session store |
| `seam-auth` | L2 | API key auth, rate limiting, quota |
| `seam-observability` | L2 | Tracing + metrics |
| `seam-session-distributed` | L2 | Distributed session store |
| `bundle-base` | L3 | Base assembly with feature flags |
| `bundle-web-app` | L3 | axum HTTP surface |
| `bundle-headless` | L3 | CLI runner |
| `core-evolution` | L0 | Gene/Capsule evolution primitives |
| `core-orchestrator` | L0 | Multi-agent orchestration |
| `core-workflow` | L0 | DAG workflow engine |
| `core-memory` | L0 | Agent memory + semantic search |
| `mcp-client` | L1 | MCP tool adapter |
| `test-support` | — | Mock LLM + fixtures |

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
| Unit | 112 | `cargo test` | Per-crate correctness |
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
