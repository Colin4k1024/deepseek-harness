---
description: "Proactive single-repository agent for maintainers: policy-gated GitHub and Feishu intake, per-ticket Sessions, and a Web management panel."
kind: "package-reference"
---

# @deepseek-ai/dsh-experimental-superagent

English | [中文](README.zh.md)

## Summary

`dsh-experimental-superagent` listens to one GitHub repository and, optionally, Feishu. An allowed request becomes a Ticket, runs in its own DSH Session and checkout directory, and answers in the original conversation, for example with a pull request. A Policy decides what the agent may do on its own; the Web panel shows what needs a person. The full concept and setup guides live in the package's numbered [docs](docs/000-philosophy.md) and [SETUP.md](SETUP.md).

## Table of Contents

- [Composition](#composition)
- [Model Experience](#model-experience)
- [Known Limitations and Deferred Work](#known-limitations-and-deferred-work)
- [Dev Note](#dev-note)

-----

<a id="composition"></a>
## Composition

The bundle inserts the Host plugin, the GitHub Source watcher, the disabled Feishu Source watcher, and the two agent presets its Sessions run under. The profile must supply the `dsh-superagent` row's `stateDir`; the plugin refuses to start without it. See [examples/profile.cordis.patch.yml](examples/profile.cordis.patch.yml) and [docs/006-deployment.md](docs/006-deployment.md).

<a id="model-experience"></a>
## Model Experience

### Worker Session prompt

#### What the model sees

Each Ticket runs in its own Session under the worker preset. The model sees the assignment text built from Ticket state, matched-rule limits, the repository's AGENTS.md, and the team's prompt sections; see [docs/004-prompts.md](docs/004-prompts.md).

#### Token effect

Assignment and context text is ordinary Session history and contributes tokens until compaction replaces it.

#### KV Cache effect

Each Ticket starts a new Session, so its prompt establishes rather than invalidates a reusable request prefix.

<a id="known-limitations-and-deferred-work"></a>
## Known Limitations and Deferred Work

- **Research prototype** — behavior and configuration may change; it is not a product-support commitment.
- **One repository per installation** — the engine owns exactly one repository and its accounts.
- **Polling Sources** — GitHub and Feishu poll rather than receive webhooks; migration to `dsh-webhook` is future work.
- **State directory lock** — one instance per state directory; concurrent instances fail fast.

<a id="dev-note"></a>
### Dev Note

<details>
<summary>Working context for maintainers — click to expand</summary>

Imported from the `dsh-superagent` research prototype; durable identifiers (plugin names, preset ids, skill prefixes, state schemas) are unchanged.

</details>
