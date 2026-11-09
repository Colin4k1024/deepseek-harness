# Agent Note: Superagent imported as one experimental package

Status: implemented

English | [中文](2026-11-09-superagent-experimental-import.zh.md)

## Problem

The `dsh-superagent` research prototype lives outside the harness repository as a standalone bundle. Publishing and composition work through `dsh plugin`, but the code does not receive the repository's typecheck, build, tests, or structural gates, and the prototype cannot evolve with the packages it builds on.

## Decision

Import the prototype into `packages/experimental/superagent` as `@deepseek-ai/dsh-experimental-superagent`, following the [experimental package policy](../../../../packages/experimental/README.md#status): public, pre-stable, without a product-support commitment. The package keeps one bundle that owns both Host plugin entries (`index`, `github-watch`, `feishu-watch`) and the Web panel, with face-specific `tsconfig.host.json`/`tsconfig.client.json` leaves like the Inspector.

Bundle patch rows rename only their module specifiers to the new package name. Durable identifiers stay unchanged: plugin record names (`dsh-superagent`, `dsh-superagent-github`, `dsh-superagent-feishu`), agent preset ids, skill prefixes, state directory schemas, and the config row id `dsh-superagent`. Existing profile patches and stored state keep working.

Distribution stays a profile patch plus [SETUP.md](../../../../packages/experimental/superagent/SETUP.md); the bundle does not join `OPTIONAL_BUNDLES`, because it requires `stateDir`, repository accounts, and setup before it is usable, which violates the switch-on admission rule. It joins `ON_DEMAND_BUNDLES` with package-owned localized metadata and an icon, so the Plugins page can discover it offline; selection installs the package but does not remove the setup requirement. GitHub and Feishu keep polling Sources; migration to `dsh-webhook` is future work, not part of the import.

## Alternatives considered

**Keep the standalone repository only.** Usable today, but the prototype never meets repository gates and drifts from the packages it depends on.

**Split into host, client, and bundle packages like Agent Teams.** Cleaner faces, but the prototype's client and host halves share the domain model by design; a split would rewrite import structure without changing behavior.

**Rename the durable identifiers to the experimental namespace.** Cleaner naming, but breaks stored state, saved profile patches, and the published prototype's upgrade path for no functional gain.

**Migrate Sources to webhook ingress during the import.** Correct direction eventually, but it changes trigger, recovery, and batching semantics; the import must stay behavior-preserving.

## Consequences

The import is behavior-preserving except for strictness the repository adds: `exactOptionalPropertyTypes` fixes in shared modules, the panel fixture's route to the client program, and test files named `*.client.spec.*` for face assignment. The panel's `src/navigation.ts` moved under `src/client/` because the client program must not compile host-only modules. Publication extras for the setup guides, prompts, skills, and watcher entries are registered in the workspace constraints gate.

Known deferred work: per-file coverage remains below the repository bar. The imported source now passes repository oxlint. Coverage is tracked as a follow-up integration phase before any promotion.
