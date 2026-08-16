# dsh-seam-sandbox

L2: Sandbox capability seam.

## Purpose

Provides process confinement via platform-specific sandbox backends. The `SandboxResolution` enum forces callers to handle the case where sandboxing is unavailable.

## Key Types

- **`SandboxBackend`** — Trait: `confine(policy)` + `name()`.
- **`SandboxPolicy`** — `Minimal` / `Confined` / `Strict`.
- **`SandboxResolution`** — `Confined(Box<dyn SandboxBackend>)` or `Unavailable { requested }`.
- **`SeatbeltBackend`** — macOS sandbox-exec (3 profile levels).
- **`LandlockBackend`** — Linux Landlock (kernel 5.13+), conditional compilation.

## Usage

```rust
use dsh_seam_sandbox::{resolve, SandboxPolicy, SandboxResolution};

match resolve(SandboxPolicy::Confined) {
    SandboxResolution::Confined(backend) => {
        backend.confine(SandboxPolicy::Confined).await?;
    }
    SandboxResolution::Unavailable { .. } => {
        // Compiler forces you to handle this case
    }
}
```
