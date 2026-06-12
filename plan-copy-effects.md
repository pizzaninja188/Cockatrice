# Design Plan — Copy effects (CR 707)

## Context

Copy effects are in `fable-refactor.md`'s out-of-scope list. They split into two distinct mechanics, both common:

- **Copying a spell/ability on the stack** (CR 707.10): Twincast, Fork, Reverberate, Increasing Vengeance, storm-style copies. A new spell object that is a copy of the original, with the copy controller choosing new targets.
- **Copying a permanent** (CR 707.2): Clone, Vesuvan Doppelganger, Phantasmal Image, Spark Double, Phyrexian Metamorph; and token copies (Populate, "create a token copy" — the bridge to [[plan-tokens]]).

**Why structural:** the engine has no notion of an object whose characteristics are *derived from another object* rather than from its own `card_id`. `GameObject.card_id` and `StackItem.card_id` are single fixed registry ids; characteristics are looked up from that id. A copy must present another object's **copiable values** (CR 707.2: name, types, P/T, abilities, mana cost — but not counters, auras, damage, or non-copy continuous effects). This is a layer-1 effect (CR 613.2) that sits beneath everything else and is the deepest characteristic-layering change in the roadmap.

## Current-state grounding

- `GameObject` (`state.rs:59`): characteristics come from `card_id` → `CardRegistry::get`. No "this object copies object N" indirection.
- P/T computation: base (from `card_id`) → `continuous_effects` (layer 7c) → [future counters 7d]. There is **no layer 1 (copy)** in the chain; copy must be inserted *beneath* base, redefining what "base" even is.
- `StackItem` (`state.rs:153`): fixed `card_id` + `targets`. No way to mark a stack item as a copy with independently chosen targets.
- The continuous-effect system (`ContinuousEffect`, `ContinuousEffectKind`) is extensible (the `// Future:` notes) but currently only does P/T modification.

## Design

The unifying idea: a **copiable-characteristics resolver**. Instead of "characteristics = registry[card_id]", introduce `effective_card_id(object) -> CardId` (or a richer `CopySource`) that copy effects can override, evaluated at CR layer 1 before everything else. Most objects return their own id; a copy returns its source's. This keeps the change localized to the characteristic-query seam rather than scattering copy-awareness through the engine.

### 1. Copying a permanent (Clone class)

A `CopyPermanent` continuous effect (layer 1, `ContinuousEffectKind` extension) on the copying object: `{ copies: CardId | ObjectId-snapshot }`. CR 707.2 says a copy uses the *copiable values* of the original as printed (plus other copy effects, but not counters/auras/damage). Cleanest representation: snapshot the source's **copiable card id + any copy-layer modifications** at the moment the copy is created (CR 707.2 evaluates copiable values at that point), store it on the copying permanent, and have `effective_card_id` return it. Then layers 2–7 (including this permanent's own counters and continuous effects) apply on top normally.

- **As a spell entering as a copy** (Clone is cast normally; it's an ETB replacement choosing what to copy): on resolution/ETB, the controller chooses a battlefield creature; the engine records the copy source. This is a **mid-resolution choice over live objects** → it leans on [[plan-custom-rust-tier]]'s `ResolutionChoiceRequired` machinery for the "choose what to copy" prompt.
- **Token copy** (Populate / "create a token copy of"): mint a token ([[plan-tokens]]) whose `effective_card_id` is the copied permanent's. The two plans meet here.

### 2. Copying a spell (Twincast class)

A `CopyTargetSpell { count: u32 }` primitive-ish effect targeting a spell on the stack. On resolution it pushes `count` new `StackItem`s that are copies of the target: same `card_id`, `chosen_x`, and copiable choices, but **controlled by the copy's controller**, who may choose new targets (CR 707.10c). `StackItem` gains `is_copy: bool` (a copy is not cast — no mana, doesn't count for storm/cast-triggers, never leaves a zone on resolution, mirroring the existing ability-on-stack handling which already resolves without a zone move). New-target selection reuses the existing targeting prompt path.

Names two cards: Twincast (`CopyTargetSpell { count: 1 }`) + Fork. Reverberate/Increasing Vengeance follow.

### 3. The layer-1 seam

Add CR layer 1 to the characteristic computation: `effective_card_id` resolves copy effects first, then all existing layers read characteristics through it. Document the ordering (CR 613.2: layer 1 copy → 2 control → 3 text → 4 type → 5 color → 6 ability → 7 P/T). Most layers beyond 1 and 7 don't exist yet; this plan only needs layer 1 + the existing 7-stack to read through it.

## Proto / relay / UI

- **Proto** (`ruled_v1.proto`): `StackPushed`/battlefield map need to convey *effective* identity for copies so the relay/client show the copied card. Add an optional `effective_card_id` (and for token copies, reuse [[plan-tokens]]'s inline identity). The relay maps `effective_card_id` through the catalog the same way it maps `card_id`. A copy of a token, or of a card not in the deck catalog, uses inline identity.
- **Relay** (`server_game.cpp`): when an object reports an `effective_card_id` differing from `card_id`, resolve display name via the effective id; otherwise unchanged. No Oracle rules lookup.
- **UI:** render copies as the copied card (image/name from the effective id via Oracle `cards.xml`); a "copy" annotation overlay. New-target choice for copied spells uses the existing target prompt. Ruled-only.

## Phasing

1. **Spell copy first** (`CopyTargetSpell`, `StackItem.is_copy`, new-target prompt) — it reuses the stack/targeting machinery and needs no layer-1 characteristic surgery. First card: Twincast.
2. **Permanent copy** (layer-1 `effective_card_id` seam + `CopyPermanent` effect + "choose what to copy" via [[plan-custom-rust-tier]]). First card: Clone.
3. **Token copy** — bridges [[plan-tokens]] + permanent copy. Populate / "create a token copy."

## Tests

- `scenario.rs`: Twincast on a Lightning Bolt creates one copy; the copy's controller retargets it; both deal damage; copying a countered/illegal spell fizzles per CR. Clone enters as a chosen creature (P/T, types match the source); a +1/+1 counter later on the Clone stacks on the copied base (proves layer 1 < layer 7d). Copying a creature that is itself pumped copies only copiable values (ignores the source's until-end-of-turn pump — CR 707.2).
- `conformance.rs`: copy cards resolve without panic.

## Out of scope

- Copying with modifications ("a copy of ~ except it's a 9/9", "except it gains haste") — an additive layer on the copy effect; add per first card. Copying *cards in other zones*, copying activated/triggered *abilities* on the stack (CR 707.10 also covers ability copies — same `is_copy` stack mechanic, add when a card needs it), legend-rule interaction for permanent copies (CR 704 — Clone of a legend), face-down copies.

## MTG applicability

CR 707 governs copies: 707.2 copiable values for permanents (the layer-1 snapshot, excluding counters/auras/damage), 707.10 spell copies on the stack with new-target choice and no casting. CR 613.2 fixes copy as characteristic layer 1, beneath all others — the plan's `effective_card_id` seam implements exactly that ordering. The "copiable values as printed, modified by other copy effects only" rule (707.2) is why the design snapshots the source's effective copy-layer id, not its full current P/T. Each card cites Oracle text; modification clauses are deferred.
