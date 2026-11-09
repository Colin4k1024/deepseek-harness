---
description: "面向维护者的单仓库主动式 agent：经策略把关的 GitHub 与飞书请求入口、每工单独立会话，以及 Web 管理面板。"
kind: "package-reference"
---

# @deepseek-ai/dsh-experimental-superagent

[English](README.md) | 中文

## 概述

`dsh-experimental-superagent` 监听一个 GitHub 仓库，可选接入飞书。被允许的请求会变成一个工单（Ticket），在各自独立的 DSH 会话与检出目录中执行，并回到原始对话中答复，例如以一个拉取请求收尾。策略（Policy）决定 agent 可以自行做什么；Web 面板展示需要人处理的事项。完整概念与安装说明见包内编号文档 [docs](docs/000-philosophy.md) 与 [SETUP.md](SETUP.md)。

## 目录

- [组合](#composition)
- [模型体验](#model-experience)
- [已知限制与延期工作](#known-limitations-and-deferred-work)
- [开发备注](#dev-note)

-----

<a id="composition"></a>
## 组合

组合包插入 Host 插件、GitHub Source 监听器、默认关闭的飞书 Source 监听器，以及其会话运行所需的两个 agent preset。profile 必须提供 `dsh-superagent` 行的 `stateDir`；缺少该配置时插件拒绝启动。参见 [examples/profile.cordis.patch.yml](examples/profile.cordis.patch.yml) 与 [docs/006-deployment.md](docs/006-deployment.md)。

<a id="model-experience"></a>
## 模型体验

### 执行会话提示词

#### 模型看到什么

每个工单在自己的会话中按执行器 preset 运行。模型看到的任务文本由工单状态、命中规则的限制、仓库的 AGENTS.md 与团队提示词段落拼装而成；见 [docs/004-prompts.md](docs/004-prompts.md)。

#### Token 影响

任务与上下文文本是普通会话历史，在压缩替换之前持续消耗 token。

#### KV Cache 影响

每个工单开启新会话，其提示词建立而非失效可复用的请求前缀。

<a id="known-limitations-and-deferred-work"></a>
## 已知限制与延期工作

- **研究原型** — 行为与配置可能变化，不构成产品支持承诺。
- **每套安装只支持一个仓库** — 引擎恰好拥有一个仓库及其账号。
- **轮询式 Source** — GitHub 与飞书使用轮询而非 webhook；迁移到 `dsh-webhook` 是后续工作。
- **状态目录锁** — 每个状态目录仅允许一个实例；并发实例会立刻失败。

<a id="dev-note"></a>
### 开发备注

<details>
<summary>维护者的工作上下文——点击展开</summary>

从 `dsh-superagent` 研究原型导入；持久标识符（插件名、preset id、技能前缀、状态 schema）保持不变。

</details>
