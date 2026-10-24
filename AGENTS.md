# Cockatrice fork — agent context

This is the entry point for repository work. Infer phase, scope and completion gates from the
current request; carry forward authorization and acceptance deferrals. Planning stays read-only.

## Required workflow

- Preserve unrelated edits. Keep one coherent outcome; avoid unrelated cleanup or optional polish.
- Use Windows/PowerShell. For testable behavior, prove an intended focused red before implementation,
  then focused green after each coherent increment. Once stable, run the full affected-side gate.
  Check exit codes. Documentation and mechanical changes need their actual contract checks, not
  manufactured executable tests.
- Trace changed ruled contracts through engine, protobuf, relay and client. Change and verify every
  affected producer/consumer; explain unaffected surfaces briefly as N/A. Proto changes require both
  Rust and C++. Preserve freeform gating.
- Reuse valid final verification on unchanged content, dependencies and environment. A later commit
  request alone does not require another run. Fix material defects; defer optional polish after green.
- Default GUI acceptance to the user; provide concise setup, actions and expected results for affected
  interaction, networking, privacy or physical identity cases. Run agent-operated GUI checks only when
  explicitly requested. Keep acceptance pending until the user confirms it, or record explicit deferral.
  Distinguish automated, agent-operated GUI, user-confirmed, deferred and N/A evidence. Manual deferral
  permits applicable automated work and already-authorized delivery; it does not establish acceptance.

## Authority and architecture

Tricerules is the single ruled-state writer. Servatrice authenticates, relays, binds physical cards
and filters private information. Cockatrice displays engine state and submits engine-offered actions;
Oracle display data never decides legality. State-affecting choices remain logged and deterministic.
Keep definition IDs, engine ObjectIds/generations, Oracle names and physical card IDs distinct.

Extract substantial fork logic from upstream files into fork-owned files; keep short guarded hooks.
New fork-owned C++ uses `ruled_` (legacy `rules_relay` excepted); client files belong under
`cockatrice/src/game/ruled/`. Preserve player-set-generic behavior.

For a reusable primitive/public protocol term, compare the closest existing API and name two real
cards or mechanics it supports. If only one use fits, justify necessary specialized behavior rather
than inventing uses or parameters. Private helpers/state need a concrete purpose.

Oracle governs cards; current official Comprehensive Rules govern mechanics. Fetch relevant rulings
and verify exact CR citations. Substantive ruled reports include MTG applicability/compliance or
deferral; other work states “No MTG rules surface area.”

## Load only the relevant guidance

Read applicable instructions once in a fresh context; reuse instructions already present and read
changed guidance on continuation. Search reference sections for the current differences. A link
does not require loading every linked document, example or historical record. Routine card authoring
defaults to the reuse-first path below; do not repeat preparation already valid for the task.

| Task | Read |
|---|---|
| Documentation/mechanical changes | Relevant documents; [documentation checks](docs/AGENT-VERIFICATION.md#verification-ladder) |
| Rust engine | [Rust guidance](tricerules/AGENTS.md); relevant [verification](docs/AGENT-VERIFICATION.md) sections |
| Card authoring/capabilities | [Rust guidance](tricerules/AGENTS.md), [execution path](docs/AUTHORING-WORKFLOW.md); authoring reference sections selected there |
| Game UI/manual checks | [Game guidance](cockatrice/src/game/AGENTS.md); touched class/interaction sections of its README |
| Structural/cross-component changes | Relevant [architecture](docs/ARCHITECTURE.md) sections; [interaction checklist](docs/RULES-INTERACTION-CHECKLIST.md) before finalizing a substantive plan |
| Independent card review | [Reviewer contract](docs/CARD-REVIEW.md), when delegation is authorized |
| Selecting work/dependencies | Live [GitHub Issues](https://github.com/pizzaninja188/Cockatrice/issues), current code, history and worktree |

Verification commands, metadata preparation and evidence reuse are owned by
[AGENT-VERIFICATION.md](docs/AGENT-VERIFICATION.md). Use its quiet runner for noisy commands and
retain full failure logs. Updating external source datasets requires separate authorization.
If a running executable blocks linking, stop only its exact process and rebuild.

## Manual Windows interaction

When agent-operated GUI checks are requested, use trusted `node_repl` → `@oai/sky`: initialize,
enumerate apps/windows, select returned windows,
and refresh state after actions. Do not use browser-only `cua_repl` for native control. Launch
`scripts/launch-ruled-game.ps1 -Dev` in the interactive user context; clients owned by
`CodexSandboxOffline` do not establish visible acceptance. See the game guide for launch/stop steps.

## Delivery

Implement an approved design without reopening settled decisions. Ask only when unresolved scope
or correctness materially changes the outcome; continue independent authorized work meanwhile.
Do not infer commit/push authorization from implementation alone. When authorized, stage only
reviewed paths/hunks, inspect the staged diff, commit, push to the authorized destination, verify
the remote SHA and reconcile directly affected issues within scope. Preserve unrelated work.
Git lock permission errors require supported approval; never remove an active or ambiguous lock.
Confirmed stale recovery must use the authorized narrow recovery path. Do not create another tracker.
