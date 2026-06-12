# Design Plan — Tokens (`CreateToken` primitive + CR 111)

## Context

Tokens are listed in `fable-refactor.md`'s out-of-scope section as a future structural design. They block a large fraction of the card pool: token-makers span every color and rarity (Saproling/Goblin/Soldier/Spirit/Thopter producers, Llanowar Elves-style? no — but Raise the Alarm, Lingering Souls, Bitterblossom, Hornet Queen, Krenko). Without tokens, most "go-wide" strategies are unrepresentable.

**Why it's structural and not just another primitive:** every `GameObject` today is backed by a real registry card (`card_id: String`, `state.rs:62`) that resolves to an Oracle name across IPC. A token has **no Oracle card and no deck entry** — its identity (name, types, P/T, colors, keywords) is created at resolution. The engine-owned-identity model (`PlayerDeck.mainboard_card_name` → `CardCatalog`) assumes every object's identity is known at session start. Tokens break that assumption, so the catalog/relay path needs a dynamic-identity extension.

## Current-state grounding

- `GameObject` (`state.rs:59`): `id`, `owner`, `card_id`, `zone`, `tapped`, `summoning_sick`, `power/toughness: Option<u32>`, `damage`. No `is_token` flag. P/T are printed-base values; effects layer on top via `continuous_effects`.
- `CardRegistry` is `&'static`, loaded from embedded RON. Token definitions cannot be deck cards but *can* live in the registry as a distinct namespace.
- Identity crosses IPC as **names** resolved through `CardCatalog` (`ruled_v1.proto:216`, `Entry { card_id, name, types, is_permanent }`). `PermanentMoved` / `StackPushed` carry `card_id`; the relay maps id↔name via per-game catalog maps on `Server_Game`. A token's `card_id` will not be in the session catalog (it wasn't in any deck).
- State-based actions run in the engine; CR 111.7 (a token in any zone other than the battlefield ceases to exist as an SBA) and CR 111.8 (a token that has left the battlefield can't return) need new SBA handling.

## Design

### 1. Token definitions: a registry sub-namespace

Add `TokenDefinition { name, types, supertypes, colors, power, toughness, keywords }` and a `token_defs: HashMap<TokenId, TokenDefinition>` to the registry, loaded from `data/tokens/*.ron` (the recursive `build.rs` walk already picks up subdirectories). `TokenId` is `slugify(name)` plus a disambiguator when two tokens share a name but differ in P/T/types/color (e.g. multiple 1/1 Spirits) — key on the full characteristic tuple, not just name. A token created by a card references its def by id.

Rationale for registry storage over inline-on-effect: token characteristics are shared across many makers (a 1/1 white Soldier is made by dozens of cards), and storing them as data keeps the "two cards" reuse rule satisfied and lets the conformance test validate them.

### 2. The primitive

```rust
// SpellEffectKind
CreateTokens {
    token: TokenId,
    count: u32,
    controller: TokenController, // Controller | EachPlayer (e.g. symmetrical token effects)
}
```

Names two+ cards immediately (Raise the Alarm: 2× Soldier; Lingering Souls: Spirit; any X-less token maker). `count` and `controller` are parameters; the characteristics come from the `TokenDefinition`. Token *copies* of existing permanents (CR 707, Populate, "create a copy") are **not** this primitive — that's [[plan-copy-effects]].

### 3. Engine: object creation + SBA

- A new `GameObject` is minted with a fresh `ObjectId`, `card_id` set to the token's `TokenId`, `is_token: true` (new field), placed on the battlefield under the chosen controller. ETB triggers fire through the existing `fire_etb_triggers` path — tokens entering must trigger Soul Warden et al., so route token creation through the same entry hook as normal ETB.
- Add `is_token: bool` to `GameObject`. SBA additions (CR 704): a token not on the battlefield is removed from the game (handle in the SBA pass after any zone move — when a token dies/bounces/exiles, it first moves, ETB/dies triggers see it, then the SBA deletes the object). Implement as "schedule removal after triggers resolve" to respect CR 111.7's timing.
- P/T for a token comes from its `TokenDefinition`, not a registry `CardDefinition`. The P/T computation path (`GameObject` base + `continuous_effects`) must read base P/T from whichever source backs the object. Cleanest: have the registry expose a uniform `characteristics(card_or_token_id)` that both `CardDefinition` and `TokenDefinition` answer, so the engine never branches on token-ness for characteristic queries.

### 4. Identity across IPC (the relay-critical part)

Tokens aren't in the session `CardCatalog`. Two options; **prefer (b)**:

- (a) Append token defs to the `CardCatalog` at session start for every token any deck card *could* make — brittle (requires statically enumerating makers' outputs) and leaks information.
- (b) **Emit token identity inline when the token appears.** Extend `PermanentMoved` / the battlefield object map with an optional `token_identity { name, types, power, toughness, colors }` populated only for tokens. The relay, on seeing a `card_id` not in its catalog, reads the inline identity instead of the id→name map. This keeps the catalog deck-scoped and makes tokens self-describing on the wire. Add a `bool is_token` to the relevant object map entry so the client can render "(Token)" and suppress card-image lookups that would fail against Oracle `cards.xml`.

### 5. Client display

Tokens have no Oracle `cards.xml` entry, so `CardDatabaseManager` image lookup will miss. The client renders tokens from the inline identity (name + P/T + types as text, a generic token frame). This is display-only and stays on the freeform-untouched, ruled-only path.

## Proto / relay / UI summary

- **Proto:** `is_token` + optional `token_identity` on the battlefield object map entry and on `PermanentMoved`; no new command (tokens are never cast/activated into existence by the player directly — they come from resolving effects).
- **Relay** (`server_game.cpp`): catalog-miss fallback to inline identity; strip nothing extra (token identity is public once on the battlefield).
- **UI:** generic token rendering from inline identity; "(Token)" label.

## Tests

- `scenario.rs`: Raise the Alarm creates two 1/1 Soldiers under the caster (assert count, P/T, controller, battlefield zone). A token that dies leaves no object (SBA removal) and triggers a dies-watcher exactly once before vanishing. A bounced token ceases to exist rather than going to hand (CR 111.7). Anthem (`AllCreatures` continuous effect) correctly buffs a token (proves shared characteristic path).
- `conformance.rs`: every `data/tokens/*.ron` def validates; a token maker resolves without panic and leaves zone integrity intact (the conformance invariant must be taught that tokens legitimately appear/disappear).

## Out of scope

- Token **copies** of existing permanents (CR 707) → [[plan-copy-effects]].
- Tokens that enter with counters → depends on [[plan-counters]] (e.g. Bitterblossom is fine; Hangarback-style needs counters first).
- Predefined named tokens with their own abilities (e.g. activated-ability tokens) work via `TokenDefinition` referencing the same ability data as cards, but verify the ability-source path handles a token source.

## MTG applicability

CR 111 governs tokens end-to-end: creation (111.1), characteristics from the creating effect (111.4), and the cease-to-exist SBA (111.7–111.8, CR 704.5d). ETB-trigger interaction is CR 603.6. The plan's SBA timing (move → triggers see it → delete) is the CR-mandated ordering, not a simplification. Token *copy* values (CR 707.2) are deferred. Implementation must cite the creating card's Oracle text for exact token characteristics (never from memory) per CLAUDE.md.
