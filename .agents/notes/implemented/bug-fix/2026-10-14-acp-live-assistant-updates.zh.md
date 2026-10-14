# Agent Note: ACP 实时助手更新

Status: implemented

[English](2026-10-14-acp-live-assistant-updates.md) | 中文

## Problem

模型运行时，Agent 循环会发布 `agent/assistant-stream` 帧，但 ACP 直到回答结束后的 `assistant/message` 才投影输出。因此即使模型逐段生成，自动化客户端也只能在最后收到整块文本。

## Decision

ACP 会话把所属 Agent 的临时 `agent/assistant-stream` 文本与推理增量，依序转换成标准 `agent_message_chunk` 与 `agent_thought_chunk` 更新。它按模型步骤和内容块顺序记录已交付文本，再与最终 `assistant/message` 对账。相同文本不重复发送；最终新增的尾段、图片与用量仍按顺序交付。步骤完成后清除暂存状态，并在提示词结算前排空通知。

若模型尝试失败、取消或重试，已交付的文本仍可见。重试会重置新内容块序列的对账状态，但不会撤回已经发出的文本。本决策只改变[仅面向自动化的 ACP 协议](../simplification/2026-07-23-acp-automation-only-protocol.zh.md)输出时序；桥接层仍不发送 DSH 展示数据。

## Alternatives considered

**在 UI 中定时拆分最终文本。** 放弃，因为这会延迟完整回答，而且不能反映模型的实际进度。

**同时发送实时增量和完整最终消息。** 放弃，因为客户端会显示重复文本。

**等成功后才发送部分文本。** 放弃，因为客户端再次失去实时进度和取消前的可见内容。

## Consequences

ACP 客户端可以在模型运行时显示文本。最终提交消息仍是剩余内容和用量的权威来源。失败或重试的尝试可能留下部分文本；客户端应将其视为部分输出，而不是完整回答。
