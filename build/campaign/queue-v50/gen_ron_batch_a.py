#!/usr/bin/env python3
"""Write the queue-v50 reviewed RON definitions (batch A: 01-03)."""
from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
DATA = ROOT / "tricerules" / "tricerules-cards" / "data"
SHA = "a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853"


def header(oid: str, note: str) -> str:
    return (
        f"// Oracle {oid}; source/rulings checked 2026-11-20 against the pinned Scryfall bulk\n"
        f"// SHA-256 {SHA}.\n{note}\n"
    )


def write(card_id: str, body: str) -> None:
    (DATA / f"{card_id}.ron").write_text(body, encoding="utf-8")


TOP_NOTE = (
    "// CR 400.3 (owner's zone) and 608.2b (departed target fizzles)."
)

# ── 01 put target on top of its owner's library ────────────────────────────

write(
    "repel",
    header(
        "6e072b3a-7a83-4d77-bae3-72de0e646b81",
        TOP_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "repel",
  name: "Repel",
  face_id: "repel",
  mana_cost: "{3}{U}",
  types: ["Instant"],
  spell_program: Some((
    body: (steps: [
      (id: "put_in_owners_library",
        kind: Perform(PutInOwnersLibrary(
          subject: Chosen((kind: Creature)), placement: Top,
        ))),
    ]),
    target_bindings: [(instruction_id: "put_in_owners_library", role: Subject, target_group_index: 0)],
  )),
  targeting: Some((groups: [(min: 1, max: 1, effect_indices: [])])),
)
''',
)

write(
    "grasp_of_phantoms",
    header(
        "0d96e4bc-ff4e-4f37-89b6-8a4e3c1069a0",
        TOP_NOTE
        + " Pinned rulings: flashback still follows timing permissions and a spell cast\n"
        "// with flashback is exiled instead of going anywhere else when it leaves the stack.",
    )
    + '''(
  id: "grasp_of_phantoms",
  name: "Grasp of Phantoms",
  face_id: "grasp_of_phantoms",
  mana_cost: "{3}{U}",
  flashback_cost: Some("{7}{U}"),
  types: ["Sorcery"],
  spell_program: Some((
    body: (steps: [
      (id: "put_in_owners_library",
        kind: Perform(PutInOwnersLibrary(
          subject: Chosen((kind: Creature)), placement: Top,
        ))),
    ]),
    target_bindings: [(instruction_id: "put_in_owners_library", role: Subject, target_group_index: 0)],
  )),
  targeting: Some((groups: [(min: 1, max: 1, effect_indices: [])])),
)
''',
)

write(
    "run_aground",
    header(
        "6ef9bf18-6b09-4398-a502-6867d999ab0c",
        TOP_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "run_aground",
  name: "Run Aground",
  face_id: "run_aground",
  mana_cost: "{3}{U}",
  types: ["Instant"],
  spell_program: Some((
    body: (steps: [
      (id: "put_in_owners_library",
        kind: Perform(PutInOwnersLibrary(
          subject: Chosen((any_of: Some([
            (kind: AnyPermanent, permanent_types: [Artifact]),
            (kind: Creature),
          ]))),
          placement: Top,
        ))),
    ]),
    target_bindings: [(instruction_id: "put_in_owners_library", role: Subject, target_group_index: 0)],
  )),
  targeting: Some((groups: [(min: 1, max: 1, effect_indices: [])])),
)
''',
)

write(
    "banishment_decree",
    header(
        "567b676b-54f3-44c7-8654-4d2b0262ea12",
        TOP_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "banishment_decree",
  name: "Banishment Decree",
  face_id: "banishment_decree",
  mana_cost: "{3}{W}{W}",
  types: ["Instant"],
  spell_program: Some((
    body: (steps: [
      (id: "put_in_owners_library",
        kind: Perform(PutInOwnersLibrary(
          subject: Chosen((any_of: Some([
            (kind: AnyPermanent, permanent_types: [Artifact]),
            (kind: Creature),
            (kind: AnyPermanent, permanent_types: [Enchantment]),
          ]))),
          placement: Top,
        ))),
    ]),
    target_bindings: [(instruction_id: "put_in_owners_library", role: Subject, target_group_index: 0)],
  )),
  targeting: Some((groups: [(min: 1, max: 1, effect_indices: [])])),
)
''',
)

write(
    "temporal_eddy",
    header(
        "3a35e772-2547-41a3-a2a2-125b48c23aa5",
        TOP_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "temporal_eddy",
  name: "Temporal Eddy",
  face_id: "temporal_eddy",
  mana_cost: "{2}{U}{U}",
  types: ["Sorcery"],
  spell_program: Some((
    body: (steps: [
      (id: "put_in_owners_library",
        kind: Perform(PutInOwnersLibrary(
          subject: Chosen((any_of: Some([
            (kind: Creature),
            (kind: AnyPermanent, permanent_types: [Land]),
          ]))),
          placement: Top,
        ))),
    ]),
    target_bindings: [(instruction_id: "put_in_owners_library", role: Subject, target_group_index: 0)],
  )),
  targeting: Some((groups: [(min: 1, max: 1, effect_indices: [])])),
)
''',
)

write(
    "fallow_earth",
    header(
        "05a4acfd-3bf3-4ec2-a28a-bc43ba7c6fbf",
        TOP_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "fallow_earth",
  name: "Fallow Earth",
  face_id: "fallow_earth",
  mana_cost: "{2}{G}",
  types: ["Sorcery"],
  spell_program: Some((
    body: (steps: [
      (id: "put_in_owners_library",
        kind: Perform(PutInOwnersLibrary(
          subject: Chosen((kind: AnyPermanent, permanent_types: [Land])),
          placement: Top,
        ))),
    ]),
    target_bindings: [(instruction_id: "put_in_owners_library", role: Subject, target_group_index: 0)],
  )),
  targeting: Some((groups: [(min: 1, max: 1, effect_indices: [])])),
)
''',
)

# ── 02 filtered graveyard cards to hand ────────────────────────────────────

GY_NOTE = (
    "// CR 115.1 (one graveyard-card target), 400.7 (the returned card is a new object),\n"
    "// 404.1 (graveyard ownership) and 608.2b (a stale sole target fizzles)."
)

write(
    "boggart_birth_rite",
    header(
        "030e9e3e-ce78-4c3b-bd83-969c07c70459",
        GY_NOTE
        + " Kindred is a card type; the Goblin subtype is on the card, not a creature type\n"
        "// requirement while it is in the graveyard. Pinned rulings confirm the Kindred update.",
    )
    + '''(
  id: "boggart_birth_rite",
  name: "Boggart Birth Rite",
  face_id: "boggart_birth_rite",
  mana_cost: "{B}",
  types: ["Kindred", "Sorcery", "Goblin"],
  spell_effect: [MoveGraveyardCards(
    filter: (card: Some((required_subtypes: ["Goblin"]))),
    destination: Hand,
  )],
)
''',
)

write(
    "argivian_find",
    header(
        "0b6c74a7-d7d3-4c05-84b1-bcb580bb476f",
        GY_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "argivian_find",
  name: "Argivian Find",
  face_id: "argivian_find",
  mana_cost: "{W}",
  types: ["Instant"],
  spell_effect: [MoveGraveyardCards(
    filter: (card: Some((any_of: Some([
      (card_type: Some(Artifact)),
      (card_type: Some(Enchantment)),
    ])))),
    destination: Hand,
  )],
)
''',
)

write(
    "natures_spiral",
    header(
        "565922d6-8152-411f-8d4d-dc7c8523a49b",
        GY_NOTE
        + " \"Permanent card\" is every card type except instant and sorcery; lands and\n"
        "// nonland permanents are one disjunction.",
    )
    + '''(
  id: "natures_spiral",
  name: "Nature's Spiral",
  face_id: "natures_spiral",
  mana_cost: "{1}{G}",
  types: ["Sorcery"],
  spell_effect: [MoveGraveyardCards(
    filter: (card: Some((any_of: Some([
      (card_type: Some(Land)),
      (card_type: Some(NonlandPermanent)),
    ])))),
    destination: Hand,
  )],
)
''',
)

write(
    "deja_vu",
    header(
        "7408b9c5-7266-4627-be4e-b691cf5c622c",
        GY_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "deja_vu",
  name: "Déjà Vu",
  face_id: "deja_vu",
  mana_cost: "{2}{U}",
  types: ["Sorcery"],
  spell_effect: [MoveGraveyardCards(
    filter: (card: Some((card_type: Some(Sorcery)))),
    destination: Hand,
  )],
)
''',
)

# ── 03 tap plus stun counters ──────────────────────────────────────────────

STUN_NOTE = (
    "// CR 122.1 (counters), 502.3 and 701.26 (tap/untap), 614.1d and 614.5\n"
    "// (stun-counter replacement)."
)

write(
    "cryogenic_stasis",
    header(
        "846ba66c-a4eb-4d00-a0f1-d9f9524b9695",
        STUN_NOTE + " CR 121.1: the controller draws the card.",
    )
    + '''(
  id: "cryogenic_stasis",
  name: "Cryogenic Stasis",
  face_id: "cryogenic_stasis",
  mana_cost: "{1}{U}",
  types: ["Instant"],
  spell_effect: [
    Tap(subject: Chosen((kind: Creature))),
    PutCounters(counter: Stun, count: 1, subject: Chosen((kind: Creature))),
    Draw(count: 1),
  ],
  targeting: Some((groups: [(
    min: 1,
    max: 1,
    prompt: "Choose target creature",
    effect_indices: [0, 1],
  )])),
)
''',
)

write(
    "impede_momentum",
    header(
        "c0239ff1-6ae6-431a-806e-7321d8272a1d",
        STUN_NOTE + " Pinned rulings also confirm only one stun counter is removed per untap.",
    )
    + '''(
  id: "impede_momentum",
  name: "Impede Momentum",
  face_id: "impede_momentum",
  mana_cost: "{1}{U}",
  types: ["Sorcery"],
  spell_effect: [
    Tap(subject: Chosen((kind: Creature))),
    PutCounters(counter: Stun, count: 3, subject: Chosen((kind: Creature))),
    Scry(count: 1),
  ],
  targeting: Some((groups: [(
    min: 1,
    max: 1,
    prompt: "Choose target creature",
    effect_indices: [0, 1],
  )])),
)
''',
)

write(
    "tranquilize",
    header(
        "1133633a-b717-42cb-b477-4e18584d876f",
        STUN_NOTE + " Pinned ruling: an already-tapped target still gets three stun counters.",
    )
    + '''(
  id: "tranquilize",
  name: "Tranquilize",
  face_id: "tranquilize",
  mana_cost: "{1}{U}",
  types: ["Sorcery"],
  spell_effect: [
    Tap(subject: Chosen((kind: Creature, controller: Opponent))),
    PutCounters(counter: Stun, count: 3, subject: Chosen((kind: Creature, controller: Opponent))),
  ],
  targeting: Some((groups: [(
    min: 1,
    max: 1,
    prompt: "Choose target creature an opponent controls",
    effect_indices: [0, 1],
  )])),
)
''',
)

write(
    "freeze_in_place",
    header(
        "8bf90bb0-f9c1-49c8-905e-32747e921e92",
        STUN_NOTE
        + " Pinned rulings: a tapped target still gets the counters; a stale target\n"
        "// fizzles and the scry does not happen.",
    )
    + '''(
  id: "freeze_in_place",
  name: "Freeze in Place",
  face_id: "freeze_in_place",
  mana_cost: "{1}{U}",
  types: ["Sorcery"],
  spell_effect: [
    Tap(subject: Chosen((kind: Creature, controller: Opponent))),
    PutCounters(counter: Stun, count: 3, subject: Chosen((kind: Creature, controller: Opponent))),
    Scry(count: 2),
  ],
  targeting: Some((groups: [(
    min: 1,
    max: 1,
    prompt: "Choose target creature an opponent controls",
    effect_indices: [0, 1],
  )])),
)
''',
)

print("wrote batch A definitions")
