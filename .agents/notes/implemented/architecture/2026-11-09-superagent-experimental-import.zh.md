# Agent Note：Superagent 以单一实验包导入

Status: implemented

[English](2026-11-09-superagent-experimental-import.md) | 中文

## Problem

`dsh-superagent` 研究原型位于 harness 仓库之外，作为独立 bundle 存在。发布与组合可以通过 `dsh plugin` 完成，但代码不经过仓库的类型检查、构建、测试与结构门禁，原型也无法与其依赖的包共同演进。

## Decision

将原型导入 `packages/experimental/superagent`，命名为 `@deepseek-ai/dsh-experimental-superagent`，遵循[实验包策略](../../../../packages/experimental/README.zh.md#status)：公开、预稳定、不附带产品支持承诺。该包保持单一 bundle，同时拥有 Host 插件入口（`index`、`github-watch`、`feishu-watch`）与 Web 面板，并像 Inspector 一样拆分 `tsconfig.host.json`/`tsconfig.client.json` 两个 face。

bundle patch 行只把模块名改为新包名。持久标识符保持不变：插件记录名（`dsh-superagent`、`dsh-superagent-github`、`dsh-superagent-feishu`）、agent preset id、技能前缀、状态目录 schema 以及配置行 id `dsh-superagent`。已有的 profile patch 与存储状态可以继续工作。

分发方式保持为 profile patch 加 [SETUP.md](../../../../packages/experimental/superagent/SETUP.md)；该 bundle 不加入 `OPTIONAL_BUNDLES`，因为它需要 `stateDir`、仓库账号与安装配置才能可用，不满足"开关即用"的准入规则。它加入 `ON_DEMAND_BUNDLES`，携带包自有本地化元信息与图标，插件页可离线发现；选择安装不会免除安装配置。GitHub 与飞书保持轮询式 Source；迁移到 `dsh-webhook` 是后续工作，不属于本次导入。

## Alternatives considered

**只保留独立仓库。** 今天就能用，但原型永远不进入仓库门禁，并与依赖的包逐渐脱节。

**像 Agent Teams 一样拆成 host、client、bundle 三个包。** face 更干净，但原型的 client 与 host 两半按设计共享领域模型；拆分只会重写导入结构而不改变行为。

**把持久标识符改名为实验命名空间。** 命名更整齐，但会破坏存储状态、已保存的 profile patch 与已发布原型的升级路径，没有功能收益。

**导入期间把 Source 迁移到 webhook 入口。** 方向正确，但会改变触发、恢复与批处理语义；导入必须保持行为不变。

## Consequences

导入除仓库新增的严格性外保持行为不变：共享模块的 `exactOptionalPropertyTypes` 修复、面板 fixture 归入 client 程序、以及按 face 归属把测试文件命名为 `*.client.spec.*`。面板的 `src/navigation.ts` 移到 `src/client/` 之下，因为 client 程序不得编译仅限 Host 的模块。安装指南、提示词、技能与监听入口的发布项已在 workspace constraints 门禁登记。

已知的后续工作：逐文件覆盖率仍低于仓库标准。导入源码已通过仓库 oxlint，1,197 个测试的当前覆盖率为语句 94.73%、分支 86.59%。该包保持实验状态期间，仓库覆盖率门禁排除其源码；升格前必须移除该排除并恢复逐文件 100% 覆盖。
