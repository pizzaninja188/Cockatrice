# Design Plan — Multi-face cards (remaining phases)

> **Status (verified 2026-08-05):** §§1–2 (battlefield face state and MDFC) shipped. §3
> Adventure and §5 generator ingestion remain TODO. §4 Transform/Flip has engine/proto scaffolding
> but still lacks card-driven effects/triggers and C++ display consumption. Remaining phases are
> tracked as issues #33–#35 in [issues.md](issues.md).

## Status

The **faces substrate** and **split** layout are implemented (commit `c97582b4`,
"Multi-face cards: faces substrate + split casting"). This plan covers what is left.

**Already shipped:**

- `CardDefinition { layout: Layout, faces: Vec<CardFace>, .. }` with a uniform
  `face(i)` / `primary_face()` / `faces_iter()` view (`tricerules-cards/src/card_def.rs`).
  Single-face cards keep their flat fields as face 0 — no corpus migration.
- Registry validates every face and indexes the whole-card name **and** each face name → id;
  `slugify` collapses `//` (`"Fire // Ice"` → `fire_ice`).
- Proto `CastSpell.face_index` + `CardCatalog.Entry.face_names`.
- Engine: `cast_spell` / `resolve_top_of_stack` use the chosen face's cost/effects/permanence/name;
  `StackItem.face_index`; per-face legal labels; hand-slot targets unioned across faces.
- Relay indexes face names; Cockatrice shows a face picker for `"A // B"` cards and sends `face_index`.
- **Split (CR 709)**: Fire // Ice, each half independently castable (`data/multiface/fire_ice.ron`).

## Current substrate and remaining design

Everything below builds on the shipped `faces` model and the `face_index` plumbing.

### 1. The shared prerequisite: in-place face state on permanents

> **Shipped.** `GameObject.face_up_index`, face-aware characteristics/abilities, battlefield view
> state, and active-face relay/client presentation landed with the MDFC work.

MDFC permanent faces, transform, and flip all need a permanent to *be* a specific face on the
battlefield, not just on the stack. The shipped implementation adds `face_up_index: usize` to
`GameObject` (0 = front) and routes battlefield characteristic queries through it:

- `GameEngine::characteristics`, continuous/static ability reads, legal actions, and zone views
  resolve `definition.face(object.face_up_index)`.
- Resolution copies `StackItem.face_index` into the entering permanent's `face_up_index`.
- Proto `BattlefieldObject.face_up_index` carries the active face through the relay/client view;
  Oracle `cards.xml` supplies the display faces but never rules decisions.

This was the pervasive bit the split layout did **not** need (split halves never become
permanents). It is now the common substrate for the remaining phases.

### 2. MDFC — modal double-faced (CR 712)

> **Shipped.** Either face can be cast or played from hand where its type permits, and a permanent
> enters and renders with the selected face. Pathway lands have focused scenario/client coverage.

With §1 in place, the cast and land-play paths are face-aware and carry `face_index` into
`face_up_index` when the selected face becomes a permanent.

### 3. Adventure (CR 715)

> **TODO — issue #33.** The card model recognizes the `Adventure` layout, but no exile permission
> or cast-from-exile gameplay path exists.

Most stateful layout. Casting the adventure (spell) half puts the card into **exile on resolution
with permission** to later cast the creature half from exile:

- An "exiled with adventure" marker on the object (or an exile sub-zone) + a cast-from-exile
  permission keyed to the creature face.
- `cast_spell` must accept casting a face from exile when that permission is present (today it only
  reads from hand). First card: Bonecrusher Giant // Stomp.

### 4. TDFC / Flip (CR 710 / 712.8) — lowest priority

> **Partially scaffolded — issue #34.** The engine validates `TransformPermanent` for only
> `Transform`/`Flip` layouts, changes `face_up_index` in place, rejects MDFCs, and emits the public
> `FaceChanged` event. No card effect/trigger invokes this command during normal play, werewolf
> day/night is absent, and the C++ ruled path does not yet consume `FaceChanged` to rename/repaint
> the physical card.

A card-driven `TransformPermanent` effect/trigger must reuse the scaffold and preserve in-place
identity. Characteristic queries already read the active face (§1). **CR 712.8: transforming does
not trigger ETB.** Flip (710) uses the same mechanism. Werewolf day/night triggers and C++ display
updates are the remaining end-to-end work.

### 5. Phase-6 generator

> **TODO — issue #35.** `gen-cards` still rejects every Scryfall object whose
> `layout != "normal"`.

Update the generator filter (currently `layout == "normal"`) to ingest qualifying multi-face
vanilla/keyword faces, authoring a `faces` vec from Scryfall's `card_faces` array.

## Tests

- **Existing:** focused scenarios cast/play MDFC faces, verify the selected battlefield face and
  active-face abilities, and reject transforming a Modal DFC. Conformance sweeps every authored
  face.
- **Remaining:** Adventure must resolve to exile and then cast its creature face exactly once;
  Transform/Flip must swap P/T/types/keywords in place without an ETB trigger; generator tests must
  cover qualifying and rejected multi-face records before the conformance sweep consumes output.

## Out of scope

Fuse (split, both halves), Aftermath, meld (CR 712.13), double-faced *tokens*, room/case layouts.

## MTG applicability

CR governs each layout: flip 710, double-faced 712 (MDFC 712.x, transform 712.8 — does **not**
trigger ETB), adventure 715. The `faces` model is the CR 712.4 substrate (a card has the
characteristics of its current face only). Names from Oracle, mechanics from tricerules.
Each face copies its `mana_cost` / `type_line` verbatim from Scryfall's `card_faces` array.
