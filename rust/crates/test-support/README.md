# dsh-test-support

Test utilities: mock LLM, fixture loader, and snapshot harness.

## Purpose

Provides test doubles and fixture infrastructure for writing integration and snapshot tests.

## Key Types

- **`MockLlm`** — Implements `LlmAdapter` by replaying pre-recorded `LlmChunk`s.
- **`FixtureLoader`** — Loads `SessionEvent`s from JSONL fixture files.
