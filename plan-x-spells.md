# Design Plan — X-spell casting (CR 107.3)

## Context

`{X}` mana costs are *representable but rejected*: `ManaSymbol::X` parses and `ManaCost::has_x()` exists (`mana.rs:25,114`), but casting errors out — `pay_mana` returns `EngineError::Illegal("X costs not yet supported")` (`engine.rs:4438`) and cast-legality rejects the same (`engine.rs:5016`). This was a deliberate Phase 3 deferral. Lifting it unlocks Fireball, Hydra-style creatures, Blue Sun's Zenith, Chalice of the Void, and every "X damage / X counters / draw X" card — a broad, high-value class.

**Why structural, not a one-liner:** X is *chosen as the spell is cast* (CR 601.2b) and that chosen value must (a) be paid for in mana, (b) become the spell's mana value while on the stack (CR 107.3b), and (c) feed the *effect* ("deal X damage" / "create X tokens" / "draw X"). Today effect amounts are fixed `u32` constants baked in the RON (`DamageTarget { amount: 3 }`). There is no path for an amount that is *determined at cast time*. So X touches the cast command (a new chosen value), the stack item (storing X), mana payment, and every amount-bearing effect (which must be able to say "use the cast-time X").

## Current-state grounding

- `pay_mana` (`engine.rs:4412`) sums pips into per-color needs; the `X` arm hard-errors.
- `StackItem` (`state.rs:153`) has no field for a chosen numeric value.
- `CastSpell` proto (`ruled_v1.proto:81`): `{ hand_card_index, targets }` — no X field.
- `SpellEffectKind` amounts are fixed literals (`amount: u32`, `count: u32`, `power/toughness: i32`). No "amount = X" representation.
- `ManaCost::mana_value()` treats X as 0 (correct off the stack, CR 107.3a) — on the stack it must reflect the chosen value.

## Design

### 1. Amount source: `Amount` enum replacing bare counts on X-capable effects

Introduce a small value type so an effect amount can be a literal or the cast-time X:

```rust
pub enum Amount {
    Fixed(u32),
    X, // resolved from the StackItem's chosen_x at resolution
}
impl Amount { fn resolve(&self, x: u32) -> u32 { match self { Fixed(n) => *n, X => x } } }
```

Migrate the amount-bearing variants that can legally use X to `Amount` with `#[serde(...)]` so existing RON (`amount: 3`) still deserializes as `Fixed(3)` — add a `From<u32>` / untagged deserialize so the corpus is untouched. Initially apply to `DamageTarget`, `Draw`, `DamageAll`, `PutCounters` (once [[plan-counters]] lands), `CreateTokens` (once [[plan-tokens]] lands), and `GainLife`. The "name two cards" rule: Fireball (`DamageTarget { amount: X }`) + Blue Sun's Zenith (`Draw { count: X }`).

### 2. Cast-time X choice

- `CastSpell` proto gains `uint32 x_value = N` (0 when the cost has no X).
- `StackItem` gains `chosen_x: u32` (0 for non-X spells).
- Engine `cmd_cast`: if `card.mana_cost.has_x()`, require `x_value` present and pay `mana_value(fixed_pips) + x_value` worth of mana — i.e. expand each `{X}` pip into `x_value` generic before `pay_mana`, or pass `x_value` into `pay_mana` and have it treat `X` as `Generic(x_value)`. Store `chosen_x` on the `StackItem`. Reject `x_value` on a spell with no `{X}` (or ignore — pick reject for strictness).
- On the stack, the spell's mana value is `fixed_mv + chosen_x` (CR 107.3b) — expose via a `StackItem` mv helper for any card that cares (Chalice of the Void counters by mana value).

### 3. Resolution

When resolving, the engine passes `stack_item.chosen_x` into effect resolution; `Amount::X` resolves to it. Multiple `{X}` in one cost all share the single chosen value (CR 107.3, e.g. {X}{X} costs — there is exactly one X for the spell).

## Proto / relay / UI (end-to-end)

- **Proto:** `CastSpell.x_value`; optionally `StackPushed` carries the chosen X for display ("Fireball (X=4)").
- **Relay** (`server_game.cpp`): pass `x_value` through to the sidecar; no rules logic in the relay.
- **UI** (`game_prompt_widget` + cast flow in `cockatrice/src`): when the player casts a card whose tricerules cost contains `{X}` (the client already parses brace costs via `PlayerActions::parseSimpleManaCost`), prompt for X **before** target selection / mana payment (CR 601.2b ordering: choose X, then targets, then costs). A simple numeric spinner in the prompt dock. The client computes total mana need as `fixed + X` to drive the existing mana-payment prompt. Ruled + non-replay only.

## Phasing

1. `Amount` type + RON-compatible deserialize, applied to `DamageTarget`/`Draw`/`GainLife` only (no token/counter dependency). `StackItem.chosen_x`, `CastSpell.x_value`, `pay_mana` X handling, the prompt-for-X UI. First card: **Fireball** (single target, `DamageTarget { amount: X }`) — smallest end-to-end X card.
2. Extend `Amount` to `DamageAll` (Fireball's "divided" mode and X-sweepers like Rolling Thunder need divided damage — note: *divided* X damage among targets is a multi-target-choice algorithm closer to [[plan-custom-rust-tier]]; the single-target X case ships first).
3. Wire `Amount::X` into `CreateTokens`/`PutCounters` once those plans land.
4. Chalice-of-the-Void-style "by mana value" interactions via the stack mv helper.

## Tests

- `mana.rs` / `engine.rs`: X=4 Fireball pays 4 generic + the colored pips; insufficient mana for the chosen X rejected cleanly; X=0 legal (CR 107.3 allows 0). On-stack mana value reflects fixed + X.
- `scenario.rs`: Fireball with X=3 deals 3 to target; casting with X but no `x_value` on the wire → `Illegal`; a fixed-amount card still resolves (Amount::Fixed path unbroken — regression guard for the corpus migration).
- Determinism: X is a logged command field; replay reproduces.

## Out of scope

- **Divided** X (Fireball's multi-target split, Rolling Thunder) — a cast-time distribution choice; route through [[plan-custom-rust-tier]] or a dedicated divided-damage target structure.
- X in **activated-ability** costs (e.g. {X}, {T}: ...) — same `Amount`/chosen-X machinery on `ActivateAbility`; add when the first such card is implemented.
- X*2 / "twice X" effects — `Amount` can grow a `Times(u32)` later.

## MTG applicability

CR 107.3 governs X: chosen on cast (107.3b, with CR 601.2b ordering), counts as that value on the stack (107.3b), 0 elsewhere (107.3a) — `mana_value()` already encodes the off-stack rule; this plan adds the on-stack value. One X per spell shared across all `{X}` pips (CR 107.3c). Each X card cites Oracle text; divided-damage X (CR 601.2d distribution) is explicitly deferred.
