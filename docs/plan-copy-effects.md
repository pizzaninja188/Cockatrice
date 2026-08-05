# Design Plan — Copy effects (CR 707), trimmed to remaining phases

> **Status (verified 2026-08-05):** Phase 1 (spell copy / Twincast) is fully shipped, including
> target restrictions and target reselection. Phases 2–3 (permanent + token copy) remain TODO;
> token creation itself is already shipped.

## Status

- **Phase 1 — copying a spell on the stack (Twincast class): DONE.**
  `SpellEffectKind::CopyTargetSpell { count, spell_filter }`, `StackItem.is_copy`, copied-spell
  printing/source identity, ruled stack presentation, and click-to-select source targeting are
  shipped. The copy's controller may choose new legal targets under CR 707.10c; copies retain
  chosen X/face/mode where applicable, cease to exist on resolution, and do not fire cast
  triggers. Scenario, client, relay, and conformance coverage landed.
- **Phase 2 — permanent copy (Clone class): TODO** (below).
- **Phase 3 — token copy (Populate / "create a token copy"): TODO.** The generic token creation
  substrate is shipped; this phase now depends on Phase 2's copiable-characteristics model and a
  representation that can snapshot inline token identity.

## Context

Copying splits into two distinct mechanics, both common. Phase 1 covered the first; the second remains:

- **Copying a permanent** (CR 707.2): Clone, Vesuvan Doppelganger, Phantasmal Image, Spark Double,
  Phyrexian Metamorph; and token copies (Populate, "create a token copy").

**Why structural:** the engine has no notion of an object whose characteristics are *derived from another object* rather than from its own `card_id`. `GameObject.card_id` is a single fixed registry id; characteristics are looked up from that id. A permanent copy must present another object's **copiable values** (CR 707.2: name, types, P/T, abilities, mana cost — but not counters, auras, damage, or non-copy continuous effects). This is a layer-1 effect (CR 613.2) that sits beneath everything else and is the deepest characteristic-layering change in the roadmap. (Spell copies in Phase 1 dodged this: a spell copy keeps the original's `card_id` outright, so no per-characteristic indirection was needed.)

## Current-state grounding

- `GameEngine::characteristics` (`engine/characteristics.rs`) is the single derived-characteristic
  pipeline. It has explicit CR 613 layer slots; `apply_layer_1_copy` is intentionally an identity
  stub and is the insertion point for this plan.
- `GameObject.card_id` still identifies the underlying card. No permanent state records a copied
  identity or copiable-characteristics snapshot.
- Layer 6 keyword grants and layer 7c/7d modifiers/counters already run through the pipeline, so a
  layer-1 copy value can feed every later implemented layer without separate P/T handling.
- The resumable tier-3 machinery (`PendingResolution` / `ResolutionChoiceRequired` / `SubmitResolutionChoice`, see the completed custom-Rust tier) exists and is reused for the "choose what to copy" mid-resolution choice.
- `CreateTokens` and inline token definitions are shipped. A token copy therefore does not need a
  new token lifecycle, but its snapshot cannot assume every source has a registry `CardId`.

## Design

The unifying idea: a **copiable-characteristics resolver**. Instead of "characteristics = registry[card_id]", introduce `effective_card_id(object) -> CardId` (or a richer `CopySource`) that copy effects can override, evaluated at CR layer 1 before everything else. Most objects return their own id; a copy returns its source's. This keeps the change localized to the characteristic-query seam rather than scattering copy-awareness through the engine.

### Phase 2 — Copying a permanent (Clone class)

A `CopyPermanent` continuous effect (layer 1, `ContinuousEffectKind` extension) on the copying object: `{ copies: CardId-snapshot }`. CR 707.2 says a copy uses the *copiable values* of the original as printed (plus other copy effects, but not counters/auras/damage). Cleanest representation: snapshot the source's **copiable card id + any copy-layer modifications** at the moment the copy is created (CR 707.2 evaluates copiable values at that point), store it on the copying permanent, and have `effective_card_id` return it. Then layers 2–7 (including this permanent's own counters and continuous effects) apply on top normally.

- **As a spell entering as a copy** (Clone is cast normally; it's an ETB replacement choosing what to copy): on resolution/ETB, the controller chooses a battlefield creature; the engine records the copy source. This is a **mid-resolution choice over live objects** → it leans on the existing tier-3 `ResolutionChoiceRequired` machinery for the "choose what to copy" prompt.
- **Token copy** (Phase 3, Populate / "create a token copy of"): mint a token whose copy-layer
  snapshot is the copied permanent's copiable identity. The representation must support both
  registry-backed cards and inline token definitions.

### The layer-1 seam

Implement the existing CR layer-1 identity slot so a copiable snapshot replaces the printed base
before all later layers read it. Preserve the documented ordering (CR 613.2: layer 1 copy → 2
control → 3 text → 4 type → 5 color → 6 ability → 7 P/T). Most layers beyond 1, 2, 6, and 7
remain identity stages; this plan needs only layer 1 plus the existing later stages.

### Phase 3 — Token copy

Bridges the shipped token lifecycle with permanent copy. Populate / "create a token copy" mints a
token whose copiable snapshot reuses the Phase 2 layer-1 seam. It is blocked on Phase 2, not on
token creation.

## Proto / relay / UI (remaining)

- **Proto/engine view:** `BattlefieldObject` needs effective display identity for permanent copies,
  supporting both catalog-backed cards and inline token identity. `StackPushed.is_copy` and
  `copy_source_object_id` already cover Phase 1 only.
- **Relay:** extend the fork-owned `RuledGameDriver`/`RuledPlayerBinding` battlefield mapping to
  resolve effective catalog identity without using Oracle for a rules decision. Preserve the
  underlying physical card binding while presenting the copied identity.
- **Client:** extend the ruled dispatcher/state/host path to render the copied image/name and a
  ruled-only copy annotation. Freeform remains unchanged.

## Tests (remaining)

- `scenario.rs`: Clone enters as a chosen creature (P/T, types match the source); a +1/+1 counter later on the Clone stacks on the copied base (proves layer 1 < layer 7d). Copying a creature that is itself pumped copies only copiable values (ignores the source's until-end-of-turn pump — CR 707.2).
- `conformance.rs`: copy cards resolve without panic.

## Out of scope

- Copying with modifications ("a copy of ~ except it's a 9/9", "except it gains haste") — an
  additive layer on the copy effect; add per first card. Copying *cards in other zones*, copying
  activated/triggered abilities on the stack, legend-rule interaction for permanent copies, and
  face-down copies remain deferred. Phase 1's spell-type restriction and CR 707.10c target
  reselection are complete and are no longer deferrals.

## MTG applicability

CR 707 governs copies: 707.2 copiable values for permanents (the layer-1 snapshot, excluding
counters/auras/damage), 707.10 spell copies on the stack (Phase 1, done) with no casting. CR 613.2
fixes copy as characteristic layer 1, beneath all others; the existing identity slot is where
Phase 2 will implement that ordering. The "copiable values as printed, modified by other copy
effects only" rule is why the design snapshots copy-layer identity, not current P/T. Each card
cites Oracle text; modification clauses are deferred.
