# Agent Note: ACP live assistant updates

Status: implemented

English | [中文](2026-10-14-acp-live-assistant-updates.zh.md)

## Problem

The agent loop publishes `agent/assistant-stream` frames while the model runs, but ACP projected only `assistant/message` after the response completed. Automation clients therefore received one full text block at the end, even when the model streamed its output.

## Decision

The ACP session serializes text and reasoning deltas from its owned Agent's transient `agent/assistant-stream` frames into standard `agent_message_chunk` and `agent_thought_chunk` updates. It keeps delivered text by model step and block order, then compares the committed `assistant/message` with that text. Matching text is not sent twice; a committed suffix, images, and usage still follow in order. The session clears step state after completion and drains notifications before prompt settlement.

Already delivered text remains visible if a model attempt fails, is cancelled, or is retried. A retry resets the comparison state for its new block sequence, while preserving text already on the wire. This decision changes only output timing within the [automation-only ACP protocol](../simplification/2026-07-23-acp-automation-only-protocol.md); the bridge still excludes DSH presentation data.

## Alternatives considered

**Split the final text into timed UI fragments.** Rejected because it would delay a completed answer and would not reflect model progress.

**Emit live deltas and the full committed message.** Rejected because clients would display the same text twice.

**Withhold partial text until a successful attempt.** Rejected because the client would again lose live progress and cancellation visibility.

## Consequences

ACP clients can render text while the model is still running. The committed message remains authoritative for remaining content and usage. A failed or retried attempt can leave partial text visible; clients must treat it as partial rather than a completed answer.
