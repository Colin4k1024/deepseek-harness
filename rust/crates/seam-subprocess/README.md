# dsh-seam-subprocess

L2: Subprocess capability seam.

## Purpose

Defines the `Subprocess` trait and a local process-tree provider for spawning and managing child processes.

## Key Types

- **`Subprocess`** — Async trait: `exec()` (wait for completion), `spawn()` (background).
- **`LocalSubprocess`** — Implementation using `tokio::process::Command`.
