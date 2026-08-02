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
- When a `fix/issue-N` branch is merged, remove that issue from here (its status is
  tracked in `AUTOMATION_STATUS.md` until then).
- Workflow: you add issues here → the box (cron) fixes them on `fix/issue-N`
  branches and pushes them → you pull, UI-test, and merge to `master`. Status and
  per-branch manual UI test steps live in `AUTOMATION_STATUS.md` / the branch
  commit message.

---

## Open

- [ ] #38 [bug] Tap/untap animation stutters when phases pass during the sweep
  - Details: Follow-up to #5, which restored the animation by carrying the live tap angle across the `Player::processPlayerInfo` rebuild (`CardItem::triggerTapAnimationFrom`). The sweep is now correct in every case but *stutters* when several phases roll past during it — the common case being the untap step, where untap/upkeep/draw each broadcast a full game state. Each broadcast destroys and recreates every table `CardItem`, and because `CardItem::animationEvent` advances a fixed `ROTATION_DEGREES_PER_FRAME` (10°) *per frame* rather than per unit of time, every rebuild costs animation progress. Measured: 90°→50° took 177 ms during a phase burst, versus a full 90° in 90 ms undisturbed. Untapping inside a single phase (no phase change mid-sweep) is smooth. Three candidate fixes, in increasing scope: (1) make `animationEvent` time-based — advance by elapsed wall-clock ms and carry the sweep's start time across the rebuild, so the sweep always completes in ~90 ms regardless of interruptions; note this is a shared upstream function, so it changes freeform too (identical at normal frame rates, since 10°/10 ms is already 1°/ms — they diverge only when frames arrive late). (2) Additionally suppress redundant resyncs in the ruled relay: `RuledGameDriver` broadcasts full game state ~3× per phase; only broadcast on real change (also cuts needless traffic, but cannot eliminate all rebuilds — the draw step legitimately changes the hand). (3) Diff the table zone in `processPlayerInfo` and reuse unchanged `CardItem`s instead of `clearContents()`-and-recreate — best visual result since the animation is never interrupted, but it restructures an upstream path, against the fork's extraction-not-restructuring rule.
  - Priority: Low

- [ ] #33 [feature] Adventure cards — cast the creature half from exile (plan-multiface-cards.md §3)
  - Details: Adventure (CR 715) is the most stateful multi-face layout and builds on the shipped `faces` model + `face_index` plumbing (§1/§2 done via #18). Casting the adventure (spell) half resolves the card into **exile with permission** to later cast the creature half from exile. Add an "exiled with adventure" marker on the object (or an exile sub-zone) plus a cast-from-exile permission keyed to the creature face; `cast_spell` must accept casting a face from exile when that permission is present (today it only reads from hand). First card: Bonecrusher Giant // Stomp. Relay/client: surface the exiled adventure card as castable (the physical card sits in exile). Add scenario tests: adventure half resolves to exile, the creature is then castable from exile, casting the creature normally from hand still works, and the permission is one-shot.
  - Priority: Low

- [ ] #34 [feature] Transform / Flip permanents — TDFC + werewolves (plan-multiface-cards.md §4)
  - Details: With `GameObject.face_up_index` (#18) and the `FaceChanged` proto event already in place, implement **in-place** face changes. A `TransformPermanent` effect/keyword flips `face_up_index`; characteristic queries already read the active face. **CR 712.8: transforming does NOT trigger ETB.** Flip (CR 710) uses the same mechanism. Wire the engine's `transform_permanent()` (Transform/Flip layouts only — ModalDfc is rejected) to a trigger/effect, implement werewolf day/night transform triggers, and add the client display: the proto field `battlefield_face_up_index` is emitted but **not yet consumed by any C++** — consume it and add a card name-change path (there is no `AttrCardName`, so a rename-in-place mechanism or card re-send is needed; the play-land entry rename in `Server_Game` is the reference). Add scenario tests: transform swaps P/T/types/keywords in place without firing ETB; flip works; werewolf day/night triggers flip correctly. Lowest priority of the multiface phases.
  - Priority: Low

- [ ] #35 [chore] Multi-face card generator ingestion (plan-multiface-cards.md §5)
  - Details: The batch generator (`gen-cards`, `scripts/gen-cards.*`) currently filters `layout == "normal"` only, so no multi-face vanilla/keyword cards are ingested. Extend the filter to author a `faces` vec from Scryfall's `card_faces` array for split / MDFC / transform / adventure cards where **every** face is vanilla or uses only supported keywords, reusing the existing per-face supported-keyword and `ManaSymbol` checks; skip (and report) any card with unsupported text on any face. After generating, `cargo test` (registry load + `conformance` validate every generated face) then the checklist `--check`. Add a `--dry-run` count of qualifying multi-face cards and skip reasons.
  - Priority: Low

- [ ] #37 [feature] Continuous control-change effects (Mind Control, Threaten)
  - Details: Issue #20 filled the CR 613 layer-2 slot for control decided **at battlefield entry** (`GameObject::controller`, read by `characteristics()`; the per-player `battlefield` list is the control index). Effects that change control of a permanent already on the battlefield are still unimplemented: `apply_layer_2_control` in `tricerules-core/src/engine/characteristics.rs` is an empty stub and its doc comment records the two traps. (1) It cannot use `ordered_effects` — that runs after layer 5 and its `effect_affects` reads `pre_layer_6.controller`, which is circular for a `CreaturesMatching { controller }` scope; this is exactly CR 613.8 dependency ordering. It needs its own earlier pass over `AffectedScope::Single` effects on the same `(timestamp, index)` key. (2) Once the derived controller can differ from the `controller` field, the battlefield lists stop being a valid control index and must be rebuilt whenever `continuous_effects` changes (add `reindex_battlefield_control()`, called from `apply_sbas`); the `debug_assert_battlefield_control_index` check in `state_based.rs` currently holds the two in sync and will start failing. Cards: Mind Control (aura, needs #10), Threaten / Act of Treason (until-EOT control + untap + haste), Confiscate, Ray of Command. The relay and client already handle a permanent sitting on a non-owner's table, so this is engine-side plus the existing `Owner:` annotation.
  - Priority: Low

- [ ] #39 [bug] Mass-effect filters ignore `not_color` and `only_controller`
  - Details: `object_matches_mass_filter` in `tricerules-core/src/engine/targeting.rs` is the untargeted counterpart to `target_filter_legal` (DestroyAll / DamageAll, via `battlefield_objects_matching`), but it honors only a subset of `TargetFilter`: `kind`, `not_artifact`, `tapped` and — as of 14955f47 — `excluded_subtypes`. `not_color`, `only_controller` and `permanent_types` are silently ignored, so a mass effect scoped by any of them would hit permanents it must not. Inert today: no shipped card pairs a mass effect with those fields, which is also why it has gone unnoticed. Fix: apply the same predicates as `target_filter_legal`, keeping the deliberate difference that this path ignores hexproof/shroud (CR 702.11e — untargeted effects affect them normally) and never consults `object_targetable_by`. Better still, factor the shared characteristic predicates into one helper both call, so the next `TargetFilter` field cannot land on only one path — that is the actual defect here, the duplication rather than any one missing check. The *cause* of that duplication is a naming one (see #40): `TargetFilter` doubles as the untargeted mass-selection type (`DestroyAll { kind }` — Wrath of God does not target; likewise `DamageAll`, `GrantKeywordsAllPermanents`), so the untargeted path needs a parallel function at all, and `object_matches_mass_filter` has to reject `Player`/`AnyTarget`/`Self_` kinds at registry load because the type it takes is over-broad for its job. Testing note: the function is `pub(super)`, so no integration test can reach it directly; coverage needs a real card (a subtype- or color-scoped Wrath, e.g. Virtue's Ruin "Destroy all white creatures", Perish "Destroy all green creatures") whose scenario test then exercises it end to end. Implementing one of those cards alongside the fix is the cheapest way to close this properly.
  - Priority: Low

- [ ] #40 [chore] `target:` vocabulary on auto-bound self-effects
  - Details: "Target" is a defined game term (CR 115.1), and three effects use it for things that do not target. `PumpTarget`, `Regenerate` and `PutCounters` take a `target: TargetFilter` that five cards fill with `(kind: Self_)` for abilities whose Oracle text never says "target": Fiery Hellhound ("This creature gets +1/+0 until end of turn"), Cudgel Troll and Drudge Skeletons ("Regenerate this creature"), Ajani's Pridemate and Bloodthirsty Aerialist ("put a +1/+1 counter on this creature"). The cost is already visible in the code: `TargetKind::Self_` has to carry a "**Not 'targeting' in the CR sense** (CR 115)" disclaimer in `tricerules-cards/src/primitives/targeting.rs`, and `spell_effect_kind_needs_target` (`tricerules-core/src/engine/targeting.rs`) hardcodes a carve-out naming exactly these three effects so they don't demand a target. That carve-out is the smell — the vocabulary is forcing a special case. The honest model is a subject enum (`{ Source, Chosen(TargetFilter) }`) on the effects that can auto-bind, which deletes the carve-out and lets `Self_` leave `TargetKind` entirely. Inert today, but every new self-referencing ability widens it. Note the correct pattern already exists and now has three instances: `RelativePlayerSet`, `TokenController`, and `PlayerRecipient` (added with `DamagePlayer`), all deliberately kept out of `TargetFilter` because those effects do not target. Verified while adding `PlayerRecipient`: the `TargetPlayer*` family is *not* affected — every card using it (Acolyte of Xathrid, Blood Artist, Bump in the Night, Diabolic Edict, Healing Salve, Mind Sculpt, Tome Scour) literally says "target player", as do `DestroyTarget` / `DamageTarget` / `CounterTargetSpell` / `CopyTargetSpell`, and `TargetKind::AnyTarget` is a real CR 115.4 term. Don't "fix" those.
  - Priority: Low

---

## Backlog (not yet prioritized)

- Own version of MTGO's FACE card rendering
- Targeting visuals overhaul
