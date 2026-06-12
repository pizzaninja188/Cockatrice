# Design Plan — Counters on objects (CR 122)

## Context

Counters are listed in `fable-refactor.md`'s out-of-scope section. They unlock an enormous span of the pool: +1/+1 and -1/-1 counters (every +1/+1-matters creature, modular, graft, outlast, adapt, bolster, proliferate), loyalty counters (all planeswalkers), and keyword/charge/age counters (Chalice of the Void, Everflowing Chalice, suspend, vanishing). At least two cards per kind is trivially satisfied.

**Why structural:** `GameObject` (`state.rs:59`) has no counter storage. +1/+1 counters are not a continuous effect — they live in CR layer 7d, *after* the `continuous_effects` (layer 7c) the engine already applies, and they persist on the object rather than expiring at cleanup. -1/-1 and +1/+1 counters annihilate as an SBA (CR 122.3). Loyalty counters gate planeswalker abilities and are paid/added as ability costs. So counters touch the P/T computation, the SBA pass, and ability costs — three engine subsystems.

## Current-state grounding

- `GameObject`: `power/toughness: Option<u32>` are printed base; effects layer via `continuous_effects: Vec<ContinuousEffect>` (`state.rs:288`). `ContinuousEffectKind::PtModify { delta_power, delta_toughness }` is layer 7c (`primitives.rs:435`) with a `// Future: Layer7bSetPt …` note.
- P/T is recomputed on demand from base + continuous effects (per the `continuous_effects` doc comment). Counters must slot into that recomputation at the correct layer.
- SBAs run in the engine; no counter-annihilation pass exists.
- `AbilityCost` (`primitives.rs:295`): `Tap | Mana | TapAndMana | Sacrifice`. No counter cost (needed for planeswalker abilities and outlast/`{T}` + remove-counter).

## Design

### 1. Storage

```rust
// GameObject
pub counters: BTreeMap<CounterKind, u32>, // BTreeMap for deterministic iteration/serialization
```

```rust
pub enum CounterKind {
    PlusOnePlusOne,
    MinusOneMinusOne,
    Loyalty,
    Charge,
    Keyword(Keyword), // e.g. a flying counter — reuses the existing Keyword enum
    Generic(String),  // age/fade/etc. named counters with no rules interaction yet
}
```

`Keyword(Keyword)` reuses `primitives.rs` `Keyword` so a "+flying counter" needs no new variant — names two mechanics (keyword counters + the P/T pair) and bounds growth.

### 2. P/T integration (CR 613 layer 7d)

In the on-demand P/T computation, after applying layer-7c `continuous_effects`, add `counters[PlusOnePlusOne]` and subtract `counters[MinusOneMinusOne]` (net, as deltas). This is a small, localized change to the existing recompute function — base → 7a/7b set → 7c PtModify → **7d counters**. Document the layer ordering inline (CR 613.4 sub-layers).

### 3. SBA: counter annihilation (CR 122.3)

New SBA: for each permanent, `min(plus, minus)` of +1/+1 and -1/-1 counters are removed in pairs. Runs in the existing SBA pass before the "toughness ≤ 0 dies" check (so a creature with three +1/+1 and three -1/-1 nets to zero counters and then dies only if base toughness ≤ 0).

### 4. The primitives

```rust
// SpellEffectKind
PutCounters { kind: CounterKind, count: u32, target: TargetFilter }, // targeted: Hardened Scales-style, +1/+1 spells
RemoveCounters { kind: CounterKind, count: u32, target: TargetFilter },
Proliferate, // CR 701.27 — untargeted-ish multi-choice; see note
```

`PutCounters`/`RemoveCounters` cover most cards (any "put N +1/+1 counters on target creature", -1/-1 removal spells, charge-counter chargers). **Proliferate is a mid-resolution multi-choice over live objects** ("choose any number of permanents/players with a counter, give each another of a kind already there") — that is a [[plan-custom-rust-tier]] `CardEffect`, not a static primitive. Note the dependency; don't force it into a data variant.

### 5. Counter ability costs

Extend `AbilityCost`:

```rust
RemoveCounter { kind: CounterKind, count: u32 }, // outlast, level-up-style, planeswalker minus
AddCounter { kind: CounterKind, count: u32 },    // planeswalker plus (loyalty as a cost-add)
```

Loyalty abilities are activated abilities whose cost adds/removes loyalty counters and which can only be activated at sorcery speed, once per turn (CR 606). That speed/frequency restriction is a separate gate on the activated-ability path — note it; planeswalkers as a card type also need a type + the "damage to planeswalker redirects" rules, which is a larger follow-on. **This plan delivers counters as a substrate; full planeswalker support is a dependent plan.**

## Proto / relay / UI

- **Proto** (`ruled_v1.proto`): add per-permanent counter summary to the battlefield object map (`repeated CounterEntry { kind, count }`), so the client can render counter pips. P/T already crosses the wire as effective values, so the engine keeps sending computed P/T; counters are shown as annotations.
- **Relay** (`server_game.cpp`): pass-through of the counter summary; no Oracle lookup, no hidden-info concern (counters are public).
- **UI** (`game_event_handler` + card rendering): render counter counts on the permanent (small overlay, like the existing ability-annotation overlay in `CardItem::paint`). Ruled-only.

## Phasing

1. `counters` field + `PutCounters`/`RemoveCounters` primitives + layer-7d P/T integration + annihilation SBA. First cards: a "+1/+1 counter on target creature" spell and a -1/-1 removal — proves both P/T and SBA.
2. Counter `AbilityCost` variants (outlast/charge cards).
3. (Dependent) Proliferate via [[plan-custom-rust-tier]].
4. (Dependent) Planeswalker type + loyalty-ability speed/frequency gating + redirect rules — separate plan.

## Tests

- `scenario.rs`: put two +1/+1 on a 2/2 → 4/4 (P/T layer). +1/+1 and -1/-1 annihilate to net before the death check. A -1/-1 counter dropping toughness to 0 kills via SBA. Counter cost: an outlast-style ability removes the counter exactly once and rejects when absent (`EngineError::Illegal`). Counters survive cleanup (not an until-end-of-turn effect).
- `conformance.rs`: counter-bearing cards resolve without panic.

## Out of scope

- Proliferate (→ custom tier), planeswalkers as a type, energy/experience/poison *player* counters (CR 122 also covers player counters — add a `players[].counters` map later with the same `CounterKind` machinery when the first card needs it), counters that grant abilities beyond P/T/keyword.

## MTG applicability

CR 122 governs counters (122.1 kinds, 122.3 +1/+1/-1/-1 annihilation SBA, 122.6 proliferate reference). P/T from counters is CR 613.4 layer 7d — the plan places them after layer-7c continuous effects, which is the CR-correct ordering, not a shortcut. Loyalty abilities are CR 606 (sorcery-speed, once-per-turn) — noted as a dependent gate, not delivered here. Each card cites its Oracle text for exact counter kind/count.
