# Issue Tracker

> **Status (2026-07-23):** active tracker, moved from repo root to `docs/`; `scripts/auto-fix-issues.sh` paths updated to match.

This file is **your input** to the automated fixer. You own it — edit it (ideally
on your Windows machine) and push. The automation reads it but never writes it;
it records progress in `AUTOMATION_STATUS.md` instead.

## How to use
- Add issues under **Open**, each with a unique short ID (`#1`, `#2`, …). Don't
  reuse IDs.
- Give each a `Priority:` (High / Medium / Low) — the automation works High first.
- Use labels in brackets: `[bug]`, `[feature]`, `[chore]`, `[docs]`.
- Delete completed issues from this file rather than marking them checked. For work
  committed directly to `master`, remove the issue in the implementation commit; for a
  `fix/issue-N` branch, remove it immediately after merge. Before reporting completion,
  reconcile dependency wording in the remaining issues. Historical status stays in
  `AUTOMATION_STATUS.md`.
- Workflow: you add issues here → the box (cron) fixes them on `fix/issue-N`
  branches and pushes them → you pull, UI-test, and merge to `master`. Status and
  per-branch manual UI test steps live in `AUTOMATION_STATUS.md` / the branch
  commit message.

---

## Open

- [ ] #44 [chore] Re-measure card coverage after the P1-P5 primitive expansion
  - Details: Repeat the calibration triage against 150-200 unimplemented modern-core cards now that the card-coverage expansion's P1-P5 substrate has shipped. Bucket every skip by missing mechanic and recompute the fully implementable and fully-or-partially implementable hit rates against the previous baseline of roughly 14% full / 29% including partials. Record the sample and results, then use them to decide whether the next increment should batch-author the newly unblocked data-tier cards or invest in another recurring primitive gap. Do not start another structural primitive project from anecdotal card requests; require the new sample to identify the highest-leverage gap and satisfy the two-real-card reuse rule.
  - Priority: Medium

- [ ] #38 [bug] Tap/untap animation stutters when phases pass during the sweep
  - Details: Follow-up to #5, which restored the animation by carrying the live tap angle across the `Player::processPlayerInfo` rebuild (`CardItem::triggerTapAnimationFrom`). The sweep is now correct in every case but *stutters* when several phases roll past during it — the common case being the untap step, where untap/upkeep/draw each broadcast a full game state. Each broadcast destroys and recreates every table `CardItem`, and because `CardItem::animationEvent` advances a fixed `ROTATION_DEGREES_PER_FRAME` (10°) *per frame* rather than per unit of time, every rebuild costs animation progress. Measured: 90°→50° took 177 ms during a phase burst, versus a full 90° in 90 ms undisturbed. Untapping inside a single phase (no phase change mid-sweep) is smooth. Three candidate fixes, in increasing scope: (1) make `animationEvent` time-based — advance by elapsed wall-clock ms and carry the sweep's start time across the rebuild, so the sweep always completes in ~90 ms regardless of interruptions; note this is a shared upstream function, so it changes freeform too (identical at normal frame rates, since 10°/10 ms is already 1°/ms — they diverge only when frames arrive late). (2) Additionally suppress redundant resyncs in the ruled relay: `RuledGameDriver` broadcasts full game state ~3× per phase; only broadcast on real change (also cuts needless traffic, but cannot eliminate all rebuilds — the draw step legitimately changes the hand). (3) Diff the table zone in `processPlayerInfo` and reuse unchanged `CardItem`s instead of `clearContents()`-and-recreate — best visual result since the animation is never interrupted, but it restructures an upstream path, against the fork's extraction-not-restructuring rule.
  - Priority: Low

- [ ] #45 [feature] Permanent copy effects — Clone and CR 613 layer 1
  - Details: Implement the remaining permanent-copy phase. `GameEngine::characteristics` already has an identity `apply_layer_1_copy` slot, but `GameObject.card_id` is still the only base identity. Add a copy-layer snapshot representation that preserves the underlying physical object's identity while replacing its copiable characteristics before layers 2–7. The snapshot must include the source's printed values plus existing copy-layer modifications, but exclude counters, auras, damage, and non-copy continuous effects (CR 707.2); design it to represent both registry-backed cards and inline token definitions so #46 can reuse it. First card: Clone. Treat “enters as a copy” as an ETB replacement with an optional mid-resolution choice over live battlefield creatures, including a decline path, using the existing `ResolutionChoiceRequired` / `SubmitResolutionChoice` machinery; illegal or stale object choices return `EngineError::Illegal`. Extend `BattlefieldObject` with engine-owned effective display identity, update the fork-owned relay/binding without consulting Oracle for rules, and update the ruled client to repaint the copied name/image with a compact copy annotation while preserving the physical card binding; freeform remains unchanged. Scenario coverage: Clone may enter unchanged; Clone copies P/T, types, name, and abilities when chosen; a counter placed on Clone applies above the copied base; an until-EOT pump, counter, aura, or damage on the source is not copied; copying an already-copied permanent preserves its copiable values; illegal/stale choices are rejected. Add proto/relay/client coverage and conformance coverage. Defer copy-with-modifications, cards copied in other zones, ability copies, face-down copies, and copy-specific legend-rule work until their first cards.
  - Priority: Low

- [ ] #46 [feature] Token copy effects — Populate and create-a-copy tokens
  - Details: Depends on #45. Bridge the shipped token lifecycle and `CreateTokens` support to the layer-1 copiable snapshot. Support both targeted “create a token that's a copy of target permanent” effects and untargeted Populate-style choices without conflating the latter with CR 115 targeting; reuse one snapshot/minting helper and keep player sets generic. A copied token must receive the chosen permanent's copiable values, including existing copy effects, while excluding counters, damage, attachments, and non-copy continuous effects; copying an inline token must not require a registry `CardId`. Reuse #45's effective battlefield display identity through proto, relay, and ruled client paths. Scenario coverage: copy a registry-backed permanent; copy an inline token; copy an already-copied permanent; prove counters and temporary pumps are excluded; reject illegal/stale targets or choices; verify token ownership, controller, zone changes, and cease-to-exist behavior. Add conformance and end-to-end display coverage. Double-faced tokens and copy-with-modifications remain deferred.
  - Priority: Low

- [ ] #47 [feature] Trigger when a permanent becomes the target of a spell
  - Details: Introduce a reusable `WheneverSelfBecomesTargetOfSpell { caster }` trigger condition for Bonecrusher Giant and heroic cards such as Favored Hoplite. Carry the spell's targets and caster through the cast event, deduplicate a permanent targeted more than once by the same spell, and expose the caster as the trigger's affected player. Cover the trigger being put on the stack above the triggering spell and persisting when that spell is countered. This completes Bonecrusher Giant's omitted "Whenever Bonecrusher Giant becomes the target of a spell, Bonecrusher Giant deals 2 damage to that spell's controller" ability.
  - Priority: Low

- [ ] #37 [feature] Continuous control-change effects (Mind Control, Threaten)
  - Details: Issue #20 filled the CR 613 layer-2 slot for control decided **at battlefield entry** (`GameObject::controller`, read by `characteristics()`; the per-player `battlefield` list is the control index). Effects that change control of a permanent already on the battlefield are still unimplemented: `apply_layer_2_control` in `tricerules-core/src/engine/characteristics.rs` is an empty stub and its doc comment records the two traps. (1) It cannot use `ordered_effects` — that runs after layer 5 and its `effect_affects` reads `pre_layer_6.controller`, which is circular for a `CreaturesMatching { controller }` scope; this is exactly CR 613.8 dependency ordering. It needs its own earlier pass over `AffectedScope::Single` effects on the same `(timestamp, index)` key. (2) Once the derived controller can differ from the `controller` field, the battlefield lists stop being a valid control index and must be rebuilt whenever `continuous_effects` changes (add `reindex_battlefield_control()`, called from `apply_sbas`); the `debug_assert_battlefield_control_index` check in `state_based.rs` currently holds the two in sync and will start failing. Cards: Mind Control (aura, needs #10), Threaten / Act of Treason (until-EOT control + untap + haste), Confiscate, Ray of Command. The relay and client already handle a permanent sitting on a non-owner's table, so this is engine-side plus the existing `Owner:` annotation.
  - Priority: Low

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul
