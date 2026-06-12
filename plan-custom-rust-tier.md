# Design Plan — Custom Rust card tier (model tier 3)

## Context

The card model has three tiers, documented in `CLAUDE.md` and `tricerules-card-model` memory, in order of preference:

1. **Data (RON)** — `spell_effect: Vec<SpellEffectKind>`, parameters in the RON.
2. **Generic primitives** (`tricerules-cards/src/primitives.rs`) — `SpellEffectKind` / `TargetFilter` / `AbilityCost` / `TriggerCondition`, parameterized, never card-specific.
3. **Custom Rust (`custom/` `CardEffect`)** — *not built yet.* The escape hatch for logic the data tiers can't express.

Tiers 1 and 2 are mature (15 `SpellEffectKind` variants, composable `TargetFilter`, layered continuous effects). **Tier 3 does not exist.** Every card to date has been forced through tiers 1–2, which works for vanilla/french-vanilla (705 generated cards) and simple targeted spells, but the moment a card requires a *resolution algorithm* rather than `(effect_kind, parameters)` static data, there is nowhere to put it. This plan builds tier 3.

**Why now:** The vanilla seam is mined out. The *typical* next interesting card needs a mid-resolution player choice over live objects (Brainstorm: draw 3, choose 2 from hand to put back in an order) or interdependent multi-player choice (Gifts Ungiven: you search 4, opponent picks 2 for your graveyard). CLAUDE.md already names these as the canonical tier-3 cases. Forcing them into new `SpellEffectKind` variants would violate the "name two cards" reuse rule — they are genuinely one-off algorithms.

## The boundary (from CLAUDE.md — do not blur it)

- **Primitive** when the effect is fully described by `(effect_kind, parameters)` static data — even if wide-reaching (`DestroyAll`, `DamageAll`, `SearchLibrary { filter, destination, shuffle }`, graveyard-scoped `TargetFilter`). Reanimate, Wrath, Demonic Tutor stay tier 1/2.
- **Custom** only when the *resolution algorithm itself is unique*: a mid-resolution choice over live objects, or multiple players choosing interdependently over one revealed set.

The risk of tier 3 is that it becomes a dumping ground that re-implements scripting per card. The mitigation is a **narrow trait surface** plus a hard review rule: a card may only land in `custom/` if a reviewer agrees no `(effect_kind, parameters)` description exists. Prefer widening a primitive every time it's close.

## Current-state grounding

- `CardDefinition` (`tricerules-cards/src/card_def.rs`) holds `spell_effect: Vec<SpellEffectKind>`, `activated_abilities`, `triggered_abilities` — all *data*. There is no hook for code.
- Spell resolution lives in the engine (`tricerules-core/src/engine.rs`), which `match`es on `SpellEffectKind` and mutates `GameState`. The engine owns the rules; `tricerules-cards` owns the data + primitive definitions.
- Resolution is currently **atomic**: a spell resolves fully in one engine step. There is no machinery to *pause* resolution, hand a choice to a player, and resume — which is exactly what Brainstorm-class cards need.

That second point is the real work. Tier 3 is not just "call a function"; it is **resumable resolution**.

## Design

### 1. The effect trait (where custom logic plugs in)

A new crate module `tricerules-cards/src/custom/` exporting:

```rust
/// A card-specific resolution algorithm that the data tiers cannot express.
/// Implementations live one-per-file under `custom/` and are registered by card id.
pub trait CardEffect: Send + Sync {
    /// Begin resolving this effect. Returns either a completed mutation set or a
    /// request for player input (a `ResolutionInterrupt`). Pure w.r.t. RNG: any
    /// randomness comes from the engine-provided seeded source.
    fn begin(&self, ctx: &mut ResolutionCtx) -> ResolutionStep;

    /// Resume after the engine collected the player's choice for a prior interrupt.
    fn resume(&self, ctx: &mut ResolutionCtx, choice: ResolutionChoice) -> ResolutionStep;
}

pub enum ResolutionStep {
    Done,
    NeedsChoice(ResolutionInterrupt),
}
```

`ResolutionCtx` is a **capability-narrowed** view of the engine: the methods custom code is allowed to call (move objects between zones, draw, reveal, query characteristics via `CardRegistry`, read the seeded RNG). It is *not* `&mut GameState` — that would let custom code corrupt invariants. It exposes only audited mutators that already maintain zone integrity (the same helpers the engine's primitive resolution uses; refactor them onto `ResolutionCtx`). This keeps the "engine is the single writer of state" rule intact: custom code requests well-formed mutations, the context applies them.

### 2. Resumable resolution (the hard part — shared with X-spells/copy)

The engine needs a **pending-resolution** slot, mirroring the existing `pending_triggers: VecDeque<PendingTrigger>` pattern in `GameState` (`state.rs:283`):

```rust
// GameState
pub pending_resolution: Option<PendingResolution>,
```

`PendingResolution` stores the in-flight `StackItem`, the `CardEffect` handle (by card id — re-looked-up, not boxed in state, to keep `GameState: Clone`), and an opaque per-card continuation token (an enum the specific `CardEffect` defines for its own steps; serialized as bytes or a small typed payload). When a custom effect returns `NeedsChoice`, the engine:

1. emits a `ResolutionChoiceRequired` event to the deciding player (proto, below),
2. parks the `StackItem` in `pending_resolution`,
3. blocks priority until the choice arrives (like a pending trigger blocks the stack),
4. on the choice command, calls `effect.resume(ctx, choice)` and loops until `Done`.

Determinism (CLAUDE.md: `(seed, command log) → state`) is preserved because every player choice is a logged command; replay re-feeds them.

### 3. Registration

`CardDefinition` gains an opt-in marker so the loader knows a card resolves via tier 3:

```ron
// brainstorm.ron
spell_effect: [],          // empty: the custom effect owns resolution
custom_effect: "brainstorm" // key into the custom registry
```

`custom/mod.rs` holds a `fn lookup(card_id: &str) -> Option<&'static dyn CardEffect>` backed by a `match` (or a `Lazy<HashMap>`), registered the same fail-fast way the registry validates data cards. Startup validation (`registry.rs`) asserts that every card with `custom_effect: Some(_)` resolves in the lookup, and that no card has *both* a non-empty `spell_effect` and a `custom_effect` (one resolution owner).

## Proto / relay / UI (end-to-end per CLAUDE.md)

- **Proto** (`ruled_v1.proto`): new `RuledEvent.resolution_choice_required = N` carrying `{ source_object_id, prompt_text, choice_kind, candidate_object_ids / candidate_card_ids, min, max, ordered }` — a **generic** input request reused by *every* tier-3 card (and later by X-spells and modal spells), satisfying the two-mechanics rule. New `RuledCommand.submit_resolution_choice = N` carrying the chosen ids (and order, when `ordered`).
- **Relay** (`server_game.cpp`): route the new event/command like the existing `TriggerNeedsTarget` / `ChooseTriggerTarget` pair; the choice references object ids already in the per-player view, and the prompt text is server-only display, no Oracle lookup.
- **UI** (`game_prompt_widget.{h,cpp}`): render `resolution_choice_required` as an ordered/multi-select pick from the named candidates, reusing the prompt dock; wire submission through the existing prompt signal path. Ruled + non-replay only.

## Phasing

1. **Resumable-resolution machinery only**, proven with one card. Add `pending_resolution`, the proto event/command pair, the relay route, and the prompt UI. Implement **Brainstorm** as the first `CardEffect` (smallest interesting algorithm: draw 3 → choose 2 of hand → order them on top). This validates the whole spine end-to-end.
2. **Second card to prove generality**: Gifts Ungiven (interdependent two-player choice — the *opponent* is the decider for part of it; exercises "deciding player ≠ controller"). If the trait + interrupt model survives both without per-card proto, the tier is sound.
3. Document the boundary and the review rule in CLAUDE.md's card-model section.

## Tests

- `tricerules-core/tests/scenario.rs`: Brainstorm happy path (3 drawn, 2 returned in chosen order, top of library asserted), illegal path (returning a card not in hand → `EngineError::Illegal`, no state mutation). Determinism test: same seed + same choice commands → identical post-state.
- `tricerules-cards`: registry validation rejects a card with both `spell_effect` and `custom_effect`; rejects a `custom_effect` key with no registered impl.

## Out of scope

- Mana-cost custom payment (alternative/additional costs) — separate from resolution; revisit with [[plan-hybrid-phyrexian-mana]].
- Boxing `dyn CardEffect` into `GameState` — explicitly avoided; re-lookup by card id keeps state cloneable for replay/snapshot.

## MTG applicability

CR governs each tier-3 card individually (Brainstorm: CR 601/120; Gifts Ungiven: CR 701-class search + opponent choice). The trait itself has no MTG surface — it is the mechanism by which Oracle-faithful per-card algorithms attach to the engine. Each `CardEffect` impl must cite its card's Oracle text + relevant CR in a header comment and carry happy + illegal scenario coverage, same standard as engine changes.
