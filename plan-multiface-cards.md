# Design Plan — Multi-face cards (split, MDFC, transform, adventure, flip)

## Context

Multi-face cards are in `fable-refactor.md`'s out-of-scope list. They are several related layouts under one structural problem — a single card with more than one set of characteristics:

- **Split** (CR 709): two halves, cast either (or both, with Fuse). Fire // Ice.
- **Modal double-faced (MDFC)** (CR 712, `layout: "modal_dfc"`): cast either face; the back is a real card. Valki // Tibalt-style, pathway lands.
- **Transforming double-faced (TDFC)** (CR 712): front face cast; transforms in place to the back. Werewolves, Delver.
- **Adventure** (CR 715, `layout: "adventure"`): cast the adventure (spell) half, then later the creature half from exile. Bonecrusher Giant.
- **Flip** (CR 710): one card, two states stacked on one face. Older Kamigawa.

The Phase 6 generator explicitly filters to `layout == "normal"`, so **every multi-face card is currently excluded from ingestion**. Supporting even split + MDFC + adventure covers a large modern slice.

**Why structural:** `CardDefinition` (`tricerules-cards/src/card_def.rs`) models exactly one face — one name, one mana cost, one type line, one effect set. The engine-owned identity model resolves a single Oracle name per object. Multi-face needs (a) a card with multiple faces in the data model, (b) a cast path that chooses a face, (c) for TDFC/flip, an in-place characteristic switch, (d) IPC/identity handling for `//` names. The Oracle display DB (`cards.xml`) already stores `//` split names, so the *display* side is partly handled; the *rules* side is greenfield.

## Current-state grounding

- `CardDefinition`: single-faced. Identity crosses IPC as one Oracle name (`PlayerDeck.mainboard_card_name` → `CardCatalog.Entry { card_id, name, types, is_permanent }`, `ruled_v1.proto:216`).
- `id == slugify(name)` is enforced by a registry test (`tricerules-cards/src/slug.rs`); a `//` name slugs to one id today.
- Casting (`CastSpell`, `ruled_v1.proto:81`) identifies the card by `hand_card_index` — no face selector.
- `GameObject.card_id` is one id; no "which face is up" state. P/T/types all key off the single id.
- The conformance test and generator assume one face per file.

## Design

### 1. Data model: faces

```rust
pub struct CardDefinition {
    pub id: String,
    pub layout: Layout,            // Normal | Split | ModalDfc | Transform | Adventure | Flip
    pub faces: Vec<CardFace>,      // len 1 for Normal; 2 for the multi-face layouts
    // ...shared/whole-card fields...
}
pub struct CardFace {
    pub name: String,
    pub mana_cost: ManaCost,
    pub types: Vec<String>,
    pub supertypes: Vec<String>,
    pub power: Option<u32>,
    pub toughness: Option<u32>,
    pub keywords: Vec<Keyword>,
    pub spell_effect: Vec<SpellEffectKind>,
    pub activated_abilities: Vec<ActivatedAbilityDef>,
    pub triggered_abilities: Vec<TriggeredAbilityDef>,
}
```

For `Normal`, `faces` has length 1 and existing single-face accessors delegate to `faces[0]` — keep a `primary_face()` helper so the 758 existing cards and all engine call sites need minimal change (ideally the migration makes `CardDefinition::name()` etc. forward to the active/primary face). The 705 generated + ~50 authored single-face RON should migrate via a serde-compatible shape (a `faces` default that wraps the existing flat fields) so the corpus isn't hand-rewritten — or a one-off mechanical migration script per CLAUDE.md's "breaking shape changes ship with a repo-wide migration script."

### 2. Identity & `id == slugify(name)`

The slug invariant must accommodate `//` names. Options: keep one card id (`slugify("Fire // Ice")` → `fire_ice`) and expose **per-face names** through the catalog. The `CardCatalog.Entry` gains repeated face entries (or a parallel `faces` message) so the relay can map *either* face name → the card id, and map a cast face back to display. Decks still reference the card by its full `//` Oracle name (what `cards.xml` stores), so deck validation/`id_for_name` resolves the whole card; face choice happens at cast.

### 3. Casting a chosen face

- `CastSpell` gains `uint32 face_index` (0 default — back-compatible for normal cards). The engine validates the chosen face is castable from the current zone (CR 712/709/715 rules per layout) and uses that face's mana cost + effect.
- **Split** (709): each half is an independent castable face; Fuse (casting both) is a later extension.
- **MDFC** (712): either face castable from hand; a permanent face enters as that face.
- **Adventure** (715): casting the adventure face puts the card into exile on resolution with permission to later cast the creature face from exile — needs an "exiled with adventure" marker and a cast-from-exile permission. This is the most stateful face layout.

### 4. In-place transform (TDFC/flip)

`GameObject` gains `face_up_index: usize` (0 = front). A `TransformPermanent` effect/keyword flips it; characteristic queries read `faces[face_up_index]`. CR 712.8 transforms don't trigger ETB; flip (710) is similar. Werewolf day/night triggers and the transform action are the engine work here. This is the lowest-priority layout (oldest/niche post-MDFC) and can come last.

## Proto / relay / UI

- **Proto:** `CastSpell.face_index`; `CardCatalog` per-face names; `GameObject`/battlefield map carry `face_up_index` for transformed permanents; `StackPushed` indicates which face is on the stack.
- **Relay** (`server_game.cpp`): map face names through the extended catalog; display the cast/active face. No Oracle rules logic (display name from face, mechanics from engine).
- **UI:** for split/MDFC, a face picker at cast time (the card has two castable halves); for TDFC, render the active face image (Oracle `cards.xml` has both face images under the `//` entry). Ruled + non-replay only.

## Phasing

1. **Data-model migration first** (`faces` vec + `primary_face()` delegation + corpus migration), keeping all 758 cards passing with zero behavior change. This is the big mechanical step; everything else builds on it.
2. **Split + MDFC** (face choice at cast, no new in-place state). First cards: Fire // Ice, a pathway land or simple MDFC.
3. **Adventure** (exile-with-permission + cast-from-exile). First card: Bonecrusher Giant.
4. **TDFC / Flip** (`face_up_index` + transform action + day/night). Lowest priority.
5. Update the Phase-6 generator filter to ingest qualifying multi-face vanilla/keyword faces once the model supports them.

## Tests

- `registry.rs`: a multi-face RON loads; the slug invariant test is updated for `//` names; per-face name index resolves both faces to the card id; existing single-face cards still satisfy `id == slugify(name)`.
- `scenario.rs`: cast each half of a split card independently; cast each face of an MDFC; adventure half resolves to exile then the creature is cast from exile; (phase 4) a transform swaps P/T/types in place without an ETB trigger.
- `conformance.rs`: every face of every multi-face card resolves without panic (the conformance sweep must iterate faces).

## Out of scope

- Fuse (split, casting both halves), Aftermath (709 variant), meld (CR 712.13 — two cards combine), double-faced *tokens*, room/case and newer bespoke layouts. Each is an extension once the `faces` model exists.

## MTG applicability

CR governs each layout: split 709, flip 710, double-faced 712 (MDFC 712.x, transform 712.8 — note: transforming does **not** trigger ETB), adventure 715. The `faces` model is the substrate CR 712.4 implies (a DFC has the characteristics of its current face only). Identity fidelity to Oracle `//` names is the CLAUDE.md requirement (names from Oracle, mechanics from tricerules). Each card copies its faces' `mana_cost`/`type_line` verbatim from Scryfall (which exposes a `card_faces` array — the authoring source for face data).
