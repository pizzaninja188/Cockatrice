# Design Plan — Multi-face cards (remaining phases)

> **Status (2026-07-23):** §1–§2 (battlefield face state, MDFC) shipped; §3–§5 tracked as issues #33–#35 in [issues.md](issues.md). Moved from repo root to `docs/`.

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

## Remaining design

Everything below builds on the shipped `faces` model and the `face_index` plumbing.

### 1. The shared prerequisite: in-place face state on permanents

MDFC permanent faces, transform, and flip all need a permanent to *be* a specific face on the
battlefield, not just on the stack. Add `face_up_index: usize` to `GameObject` (0 = front) and
route the battlefield characteristic queries through it:

- `GameObject::is_creature` / `has_keyword` and the P/T base read
  `registry.get(card_id).face(face_up_index)` instead of the flat fields.
- ETB sets `face_up_index` to the cast/entering face; a `move_object_to_zone` to the battlefield
  carries it from the resolving `StackItem.face_index`.
- Proto: `BattlefieldObjectMap` / `GameObject` and `StackPushed` carry `face_up_index` so the relay
  renders the active face image (Oracle `cards.xml` has both face images under the `//` entry).

This is the pervasive bit the split layout did **not** need (split halves never become permanents).
Do it first; MDFC/transform/flip are small once it exists.

### 2. MDFC — modal double-faced (CR 712)

With §1 in place: either face castable from hand (already true for the cast path); a permanent face
enters as that face (`face_up_index = face_index`). First cards: a pathway land (land // land) or a
simple creature // spell MDFC. The `play_land` path still reads `def.is_land` (flat) — make it
face-aware for land faces.

### 3. Adventure (CR 715)

Most stateful layout. Casting the adventure (spell) half puts the card into **exile on resolution
with permission** to later cast the creature half from exile:

- An "exiled with adventure" marker on the object (or an exile sub-zone) + a cast-from-exile
  permission keyed to the creature face.
- `cast_spell` must accept casting a face from exile when that permission is present (today it only
  reads from hand). First card: Bonecrusher Giant // Stomp.

### 4. TDFC / Flip (CR 710 / 712.8) — lowest priority

A `TransformPermanent` effect/keyword flips `face_up_index`; characteristic queries already read the
active face (§1). **CR 712.8: transforming does not trigger ETB.** Flip (710) is the same mechanism.
Werewolf day/night triggers and the transform action are the engine work here.

### 5. Phase-6 generator

Update the generator filter (currently `layout == "normal"`) to ingest qualifying multi-face
vanilla/keyword faces, authoring a `faces` vec from Scryfall's `card_faces` array.

## Tests

- `scenario.rs`: cast each face of an MDFC; an MDFC permanent face enters and queries as that face;
  adventure half resolves to exile then the creature is cast from exile; (phase 4) a transform swaps
  P/T/types in place **without** an ETB trigger.
- `conformance.rs`: already sweeps every face; extend the harness to exercise battlefield face state
  once §1 lands.

## Out of scope

Fuse (split, both halves), Aftermath, meld (CR 712.13), double-faced *tokens*, room/case layouts.

## MTG applicability

CR governs each layout: flip 710, double-faced 712 (MDFC 712.x, transform 712.8 — does **not**
trigger ETB), adventure 715. The `faces` model is the CR 712.4 substrate (a card has the
characteristics of its current face only). Names from Oracle, mechanics from tricerules.
Each face copies its `mana_cost` / `type_line` verbatim from Scryfall's `card_faces` array.
