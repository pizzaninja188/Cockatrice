# Design Plan — Hybrid & Phyrexian mana payment (CR 107.4)

## Context

The mana syntax was built to *represent* hybrid/Phyrexian/snow but **reject them at parse time** until the engine can pay them: `mana.rs:13` documents that `{G/U}`, `{B/P}`, `{S}` are "expressible in the brace syntax but rejected at parse time," and `parse_symbol` (`mana.rs:37`) falls through to `"unsupported mana symbol {…}"`. Lifting this unlocks the entire hybrid-mana era (Shadowmoor/Eventide, guild hybrids, Kitchen Finks, Boros/Izzet hybrids) and Phyrexian-mana cards (Gitaxian Probe, Dismember, Mutagenic Growth). Both are large, well-defined classes.

**Why structural:** payment becomes a *choice* rather than a deterministic drain. A hybrid pip `{G/U}` is payable by green **or** blue; `{2/W}` by two generic **or** one white; Phyrexian `{B/P}` by black **or 2 life**. `pay_mana` (`engine.rs:4412`) today is a fixed greedy drain with no alternatives and no life cost. So this touches the symbol type, the parser, mana value, color identity, and the payment algorithm — and adds a *life payment* avenue to casting.

## Current-state grounding

- `ManaSymbol` (`mana.rs:15`): `W U B R G C Generic(u32) X`. No hybrid/Phyrexian.
- `parse_symbol` rejects anything that isn't a known letter or integer.
- `ManaCost::mana_value()` (`mana.rs:78`): colored = 1, generic = n, X = 0. `colors()` (`mana.rs:90`): one color per colored pip.
- `pay_mana` (`engine.rs:4412`): greedy per-color drain, generic paid colorless-first. No alternative-payment branching, no life cost.
- Casting pays mana from the pool only (`pay_mana` comment: client pre-fills the pool via `AddManaToPool`/land taps). Phyrexian's "pay with life" needs a casting-time life deduction the engine currently never does for costs.

## Design

### 1. Symbol additions

```rust
pub enum ManaSymbol {
    // ...existing...
    Hybrid(ColorSym, ColorSym),     // {G/U}: pay either color
    MonoHybrid(u32, ColorSym),      // {2/W}: pay N generic OR one of the color
    Phyrexian(ColorSym),            // {B/P}: pay the color OR 2 life
    // {S} snow stays rejected until snow sources exist — note only
}
```

(`ColorSym` is the existing W/U/B/R/G subset.) Extend `parse_symbol` to recognize `{A/B}`, `{N/A}`, `{A/P}` forms by splitting on `/`.

### 2. Mana value & colors (CR 202.3 / 202.2)

- `mana_value()`: `Hybrid` = 1, `MonoHybrid(n, _)` = n (CR 202.3f — the *larger* of the alternatives; `{2/W}` counts as 2), `Phyrexian` = 1.
- `colors()`: a card with `{G/U}` in its cost **is both green and blue** regardless of how it's paid (CR 202.2b/105.2b — color identity of a hybrid card includes both); `Phyrexian({B})` contributes black. Both halves of a hybrid pip count.

### 3. Payment as constrained choice

The key change to `pay_mana`. Split pips into **fixed** (current deterministic drain) and **flexible** (hybrid/mono-hybrid/Phyrexian). After draining fixed pips, resolve flexible pips. Two viable strategies:

- **(a) Client-chosen** (preferred, matches existing architecture): the client already pre-fills the pool with specific mana and computes the cost. Extend the cast/activate command so each flexible pip carries the player's chosen payment mode (which color, or "pay life" for Phyrexian). The engine validates the choice is affordable and drains accordingly. This mirrors the existing "client taps lands, sends `AddManaToPool`, engine drains" contract — the engine stays a validator, not a solver.
- **(b) Engine auto-solve**: engine tries to satisfy flexible pips from whatever's in the pool. Simpler wire format but the engine must avoid greedy dead-ends (a small constraint solve) and *cannot* decide Phyrexian life-vs-mana for the player (a strategic choice). Rejected as primary.

Go with **(a)**. Phyrexian "pay 2 life" is an explicit per-pip choice in the command; the engine deducts life (CR 107.4 / 117.3 — paying life is a cost) and checks the player has ≥2 life and isn't otherwise prevented.

### 4. Affordability / legality

Cast-legality (`engine.rs`) must compute whether *some* assignment of the player's pool (plus life for Phyrexian) satisfies the cost, to decide castability for `LegalActions`. For the common case (few flexible pips) a direct check suffices; document the bound.

## Proto / relay / UI (end-to-end)

- **Proto** (`ruled_v1.proto`): `CastSpell` / `ActivateAbility` gain a `repeated FlexPipPayment flex_payments` (one per flexible pip, in cost order: chosen color, or a `pay_life` flag for Phyrexian). The `AddManaToPool` path is unchanged.
- **Relay** (`server_game.cpp`): pass-through; no rules logic.
- **UI** (`game_prompt_widget` + cast/activation flow): when a tricerules cost contains a flexible pip, prompt the player to pick each pip's payment (color buttons; for Phyrexian, a "pay 2 life" toggle). The client already parses brace costs (`PlayerActions::parseSimpleManaCost`) — extend it to recognize the new pip shapes. Ruled + non-replay only.

## Phasing

1. **Hybrid only** (`Hybrid`, `MonoHybrid`): symbol + parser + mv/colors + client-chosen payment. No life mechanic. First cards: a two-color hybrid (e.g. a Boros hybrid creature) and a mono-hybrid. Pure mana-pool choice; lowest risk.
2. **Phyrexian** (`Phyrexian`, life payment): adds the life-deduction cost path. First cards: Gitaxian Probe / Mutagenic Growth (single Phyrexian pip). This is the first time casting deducts life — verify SBA/loss interaction (paying to 0 or below is illegal; CR 119.4 you can't pay life you don't have... actually you *can* pay any life as long as result ≥ ... confirm: CR 118.4 — a player can't pay more life than they have; paying to exactly any non-negative is fine, can't go below 0).
3. Snow `{S}` — leave rejected; revisit only with snow-source support.

## Tests

- `mana.rs`: parse `{G/U}`, `{2/W}`, `{B/P}`; mana value of `{2/W}` = 2, `{G/U}` = 1, `{B/P}` = 1; `colors()` of `{G/U}` = [Green, Blue]; round-trip Display.
- `engine.rs`: hybrid pip paid by either color; mono-hybrid paid by 2 generic when color absent; Phyrexian paid by mana, and alternatively by 2 life with life decremented; Phyrexian rejected when life < 2 and no mana; insufficient pool rejected cleanly.
- `scenario.rs`: cast a hybrid spell two ways (each color) → same resolution; cast a Phyrexian spell by life → life drops by 2.

## Out of scope

- Snow mana `{S}` and snow sources. Twobrid in *activated-ability* costs (same machinery, add per first card). Cost *reduction*/increase effects interacting with hybrid (separate cost-modification design). "Spend only X mana" restrictions.

## MTG applicability

CR 107.4 governs hybrid/Phyrexian symbols; CR 202.3f the "larger alternative" mana value; CR 202.2b / 105.2b the both-colors color identity; CR 118.4 the life-payment limit (Phyrexian). The plan keeps the engine a *validator* of client-chosen payment, consistent with the existing pool-drain contract, rather than a mana solver. Each card copies its `mana_cost` verbatim from Scryfall per CLAUDE.md (the brace forms `{G/U}`, `{2/W}`, `{B/P}` are exactly Scryfall's).
