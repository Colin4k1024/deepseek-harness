# Test Plan — dsh-rust v0→v21 Review

> State: review | Role: qa-engineer | Date: 2026-08-15

## Test Scope

| 类别 | 数量 | 覆盖范围 |
|------|------|---------|
| Unit Tests | 340 | 全部 70 crates 的核心功能 |
| E2E Tests | 22 | 全栈：agent loop + tools + session + auth + web API |
| Load/Stress Tests | 9 | 并发：100 agent turns, 500 tool calls, rate limiter, auth, memory, context |
| Snapshot Tests | 3 | Agent 输出回归 |
| Benchmarks | 15 | Core + Memory + Auth 性能基线 |

## 测试矩阵

| 层级 | 测试项 | 数量 |
|------|--------|------|
| L0 Context | provide/resolve/scope/undo/event/provider | 17 |
| L1 Session | derive/derive_messages/invariant/persist | 6 |
| L1 Tools | pipeline/pre_hook/reject | 2 |
| L1 Agent Loop | parse_tool_call/agent_turn/snapshot | 4 |
| L2 Sandbox | macOS/Linux/Windows/resolve | 9 |
| L2 Auth | API key/rate limit/quota | 15 |
| L2 Persistence | JSONL/SQLite | 7 |
| L2 Observability | metrics/tracing | 8 |
| L3 Composition | profile/feature flags | 5 |
| L4 Web API | health/chat/chat_stream | 3 |
| E2E | full stack integration | 22 |
| Load/Stress | concurrent operations | 9 |

## 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| LLM provider 真实 API 行为与 mock 不同 | Medium | High | v1 roadmap: 添加真实 API e2e tests |
| 高并发下 DashMap 性能瓶颈 | Low | Medium | 已有 load test 覆盖 500 并发 |
| SQLite 在多进程场景下的锁竞争 | Low | Low | 单进程部署为主 |

## Go / No-Go 检查项

- [x] 全部 340 tests 通过
- [x] clippy 零 warning
- [x] 零 unsafe blocks
- [x] 架构一致性验证
- [x] 文档完整性
- [ ] 真实 API e2e 测试（非阻塞，v1 roadmap）
- [ ] 负载测试（非阻塞，已有并发单元测试）
