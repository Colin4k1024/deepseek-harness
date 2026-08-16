# dsh-seam-fs

L2: Filesystem capability seam.

## Purpose

Defines the `FileSystem` trait and a local filesystem provider for file operations within the agent sandbox.

## Key Types

- **`FileSystem`** — Async trait: `read()`, `write()`, `list()`, `exists()`.
- **`LocalFileSystem`** — Standard filesystem implementation using `tokio::fs`.
