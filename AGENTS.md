# Cockatrice fork — agent context

## Mandatory workflow

1. **Build and test after every coherent code-change increment.** An increment may batch inseparable edits needed to reach one compilable or testable state; it does not mean every line edit needs its own build. Prove the relevant focused gate after each increment, then run the full affected-side gate once the implementation is stable. Never report completion until every required command exits 0; check the exit code rather than eyeballing logs. Read-only investigation needs no build.
2. **Use the Windows PowerShell build and test recipes.** This checkout's active agent workflow is Windows-only.
3. **Ruled work is end-to-end.** Unless explicitly scoped backend-only, ship engine, protobuf, Servatrice relay, and Cockatrice UI behavior together. Any `.proto` change must keep both C++ and Rust buildable.
4. **Do not break freeform.** Gate new UI and command paths on ruled mode.
5. **Optimize this pre-release fork for the long term.** Prefer the simplest complete architecture over compatibility layers, speculative abstractions, or stopgaps. Large coherent changes are welcome when every increment leaves a working product.
6. **Extract fork behavior from upstream files instead of restructuring them in place.** Upstream deltas should converge toward a member pointer, one friend declaration, and short ruled call-site guards. New fork-owned C++ files use the `ruled_` prefix (`rules_relay` predates it); client fork files live under `cockatrice/src/game/ruled/`.
7. **Read [docs/REFACTOR-ROADMAP.md](docs/REFACTOR-ROADMAP.md) before a refactor or cross-component structural change.** Preserve player-set-generic behavior and the roadmap's ownership boundaries.
8. **Use red/green TDD for testable behavior changes.** Add a focused regression, run it and confirm the intended failure, then make the smallest coherent fix. Refactors need characterization coverage when existing behavior is not protected. Do not manufacture tests for documentation, formatting, generated output, or purely mechanical work.
9. **Give manual test steps when hands-on behavior matters.** Distinguish steps actually performed from steps recommended to the user.
10. **Complete the [rules interaction checklist](docs/RULES-INTERACTION-CHECKLIST.md) before finalizing a substantive ruled, protocol, relay, or client plan.** Mark genuinely irrelevant sections N/A instead of silently skipping them.

## Architecture and authority

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) before cross-component work. It contains the system diagram, command lifecycle, identity glossary, hidden-information model, effect-ordering ownership, fork-ownership table, and extension recipes.

- **Freeform** is the legacy casual path. **Ruled** is server-authoritative MTG-style play.
- **`tricerules`** is the single writer of ruled state. Rules logic lives there, not in Servatrice or the client.
- **Servatrice** authenticates, relays protobuf, binds physical objects, and filters hidden information. It does not decide rules legality.
- **Cockatrice** displays engine state and submits engine-provided legal actions. It must not infer ruled legality from Oracle display data.
- **Determinism** is `(seed, command log) -> state`; choices and dev commands that affect state must remain logged commands.
- Keep `ruled_v1.proto` aligned across every Rust and C++ consumer. Treat engine `ObjectId`, tricerules `card_id`, Oracle name, `Server_Card.id`, hand slot, and face index as distinct identities.

Before adding a new effect, trigger, cost, keyword, helper, state field, proto field, or legal action, name at least two real cards or two distinct mechanics it supports. Widen the parameters when only one use fits.

Comprehensive Rules govern mechanics and Oracle governs card-specific behavior. Verify exact CR numbers and quotations against the current official rules, and fetch card rulings rather than coding non-obvious interactions from memory. For substantive ruled work, finish with an **MTG applicability** note stating the governed concepts and compliance or deferral; otherwise state “No MTG rules surface area.”

## Guidance routing

Load only the guidance relevant to the task:

| Work area | Required guidance |
|---|---|
| Any build, test, lint, or pre-commit verification | [docs/AGENT-VERIFICATION.md](docs/AGENT-VERIFICATION.md) |
| Rust rules, card data, card generation, or custom effects | [tricerules/AGENTS.md](tricerules/AGENTS.md) |
| Game UI, ruled client state, prompts, or manual two-client checks | [cockatrice/src/game/AGENTS.md](cockatrice/src/game/AGENTS.md) |
| Structural or cross-component work | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md), [docs/REFACTOR-ROADMAP.md](docs/REFACTOR-ROADMAP.md), and [docs/RULES-INTERACTION-CHECKLIST.md](docs/RULES-INTERACTION-CHECKLIST.md) |
| Current work selection and dependency status | `docs/issues.md`; re-check current code before trusting historical plans |

## Verification ladder

Use the exact commands and affected-side matrix in [docs/AGENT-VERIFICATION.md](docs/AGENT-VERIFICATION.md).

1. **Red:** run the smallest regression that proves the missing or broken behavior.
2. **Green:** apply one coherent implementation increment and rerun that regression.
3. **Stabilize:** run the affected package or targeted CTest group while iterating.
4. **Finish:** once stable, run the full build and full suite for every affected side, plus lint, format, generated-data checks, and `git diff --check` as applicable.
5. **Manual:** run or recommend the real two-client flow when UI, networking, hidden information, or physical identity is material.

On Windows, use `scripts/run-quiet-command.ps1` for commands with noisy successful output. It retains the complete log under `build/verification-logs`, prints a concise success line, prints the full log on failure, and preserves the exact exit code. Never suppress a failure log.

If a build fails only because a running executable is locked, stop the exact Cockatrice process holding it and rebuild. Do not treat a successful compile followed by a failed link as a passing build.

## Task discipline

- Keep one task centered on one coherent outcome. Compact long histories at a stable milestone; start a separate task when the outcome changes.
- A request should identify the goal, relevant issue or files, phase (plan or implementation), required behavior, out-of-scope work, and completion gates.
- Batch independent reads and searches. Keep successful command output concise and show detailed logs on failure.
- Separate required work from optional polish. Do not perform unrelated cleanup while implementing an approved plan.
- Once a decision-complete plan is approved, implement it directly without reopening settled design choices.
- When asked to commit, stage only intended files, inspect the staged diff, make one focused commit, and preserve unrelated work.
