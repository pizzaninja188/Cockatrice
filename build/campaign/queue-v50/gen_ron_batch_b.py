#!/usr/bin/env python3
"""Write the queue-v50 reviewed RON definitions (batch B: 04-06)."""
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


SHRINK_NOTE = (
    "// CR 603.6a (enters-the-battlefield trigger), 115.1 (target), 611.2a/514.2\n"
    "// and 613.4c (until-end-of-turn -N/-N)."
)

# ── 04 enters with a targeted shrink ──────────────────────────────────────

write(
    "eyeblight_assassin",
    header(
        "09cacd93-5c20-4993-9ee2-a756204796f7",
        SHRINK_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "eyeblight_assassin",
  name: "Eyeblight Assassin",
  face_id: "eyeblight_assassin",
  mana_cost: "{2}{B}",
  types: ["Creature", "Elf", "Assassin"],
  power: 2,
  toughness: 2,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [PumpTarget(power: -1, toughness: -1, subject: Chosen((kind: Creature, controller: Opponent)))],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature an opponent controls", effect_indices: [0])])),
  )],
)
''',
)

write(
    "black_dragon",
    header(
        "ad804712-18ba-49d7-9af2-4361edaa6cf8",
        SHRINK_NOTE + " CR 702.9 (flying). No pinned ruling.",
    )
    + '''(
  id: "black_dragon",
  name: "Black Dragon",
  face_id: "black_dragon",
  mana_cost: "{5}{B}{B}",
  types: ["Creature", "Dragon"],
  power: 4,
  toughness: 4,
  keywords: [Flying],
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([2]),
    trigger: WhenSelfEntersBattlefield,
    effect: [PumpTarget(power: -3, toughness: -3, subject: Chosen((kind: Creature, controller: Opponent)))],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature an opponent controls", effect_indices: [0])])),
  )],
)
''',
)

write(
    "faerie_duelist",
    header(
        "ccbf7141-69c9-407d-b68e-c1c63ddef213",
        SHRINK_NOTE + " CR 702.8 (flash) and 702.9 (flying). No pinned ruling.",
    )
    + '''(
  id: "faerie_duelist",
  name: "Faerie Duelist",
  face_id: "faerie_duelist",
  mana_cost: "{1}{U}",
  types: ["Creature", "Faerie", "Rogue"],
  power: 1,
  toughness: 2,
  keywords: [Flash, Flying],
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([3]),
    trigger: WhenSelfEntersBattlefield,
    effect: [PumpTarget(power: -2, toughness: 0, subject: Chosen((kind: Creature, controller: Opponent)))],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature an opponent controls", effect_indices: [0])])),
  )],
)
''',
)

write(
    "brinebarrow_intruder",
    header(
        "eba73313-5ec5-4004-8040-658861b71a8f",
        SHRINK_NOTE + " CR 702.8 (flash). No pinned ruling.",
    )
    + '''(
  id: "brinebarrow_intruder",
  name: "Brinebarrow Intruder",
  face_id: "brinebarrow_intruder",
  mana_cost: "{U}",
  types: ["Creature", "Human", "Rogue"],
  power: 1,
  toughness: 2,
  keywords: [Flash],
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([2]),
    trigger: WhenSelfEntersBattlefield,
    effect: [PumpTarget(power: -2, toughness: 0, subject: Chosen((kind: Creature, controller: Opponent)))],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature an opponent controls", effect_indices: [0])])),
  )],
)
''',
)

# ── 05 enters with a targeted pump ────────────────────────────────────────

PUMP_NOTE = (
    "// CR 603.6a (entry trigger), 115.1 (target), 611.2c/613.4c (until-end-of-turn\n"
    "// pump) and 514.2 (cleanup)."
)

write(
    "haazda_officer",
    header(
        "35023c04-ae62-47bf-80a0-faa2d7a67788",
        PUMP_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "haazda_officer",
  name: "Haazda Officer",
  face_id: "haazda_officer",
  mana_cost: "{2}{W}",
  types: ["Creature", "Human", "Soldier"],
  power: 3,
  toughness: 2,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [PumpTarget(power: 1, toughness: 1, subject: Chosen((kind: Creature, controller: You)))],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature you control", effect_indices: [0])])),
  )],
)
''',
)

write(
    "herald_of_the_fair",
    header(
        "6bb7d985-b033-46a2-9bbb-6f7ee62aaaff",
        PUMP_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "herald_of_the_fair",
  name: "Herald of the Fair",
  face_id: "herald_of_the_fair",
  mana_cost: "{2}{W}",
  types: ["Creature", "Human"],
  power: 3,
  toughness: 2,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [PumpTarget(power: 1, toughness: 1, subject: Chosen((kind: Creature, controller: You)))],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature you control", effect_indices: [0])])),
  )],
)
''',
)

write(
    "rage-scarred_berserker",
    header(
        "658d54d4-e967-490c-b958-3743b3b1ef45",
        PUMP_NOTE + " CR 702.12 (indestructible). No pinned ruling.",
    )
    + '''(
  id: "rage-scarred_berserker",
  name: "Rage-Scarred Berserker",
  face_id: "rage_scarred_berserker",
  mana_cost: "{4}{B}",
  types: ["Creature", "Minotaur", "Berserker"],
  power: 5,
  toughness: 4,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [
      PumpTarget(power: 1, toughness: 0, subject: Chosen((kind: Creature, controller: You))),
      GrantKeywords(subject: Chosen((kind: Creature, controller: You)), keywords: [Indestructible]),
    ],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target creature you control", effect_indices: [0, 1])])),
  )],
)
''',
)

# ── 06 enters with a counter on another typed creature ────────────────────

COUNTER_NOTE = (
    "// CR 603.6a (entry trigger), 115.1 (target), 122.1 (placing a +1/+1 counter)\n"
    "// 205.3 (creature subtypes) and 400.7 (\"another\" is a different object)."
)

write(
    "stromkirk_mentor",
    header(
        "17cbeec4-b180-48e9-9156-42271f918a9d",
        COUNTER_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "stromkirk_mentor",
  name: "Stromkirk Mentor",
  face_id: "stromkirk_mentor",
  mana_cost: "{3}{B}",
  types: ["Creature", "Vampire", "Soldier"],
  power: 4,
  toughness: 2,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [PutCounters(
      counter: PlusOnePlusOne,
      count: 1,
      subject: Chosen((kind: Creature, controller: You, required_subtypes: ["Vampire"], excluded_objects: [Source])),
    )],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose another target Vampire you control", effect_indices: [0])])),
  )],
)
''',
)

write(
    "sanguine_glorifier",
    header(
        "391f1331-7577-4bf4-a9ed-2f55ded4cbd8",
        COUNTER_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "sanguine_glorifier",
  name: "Sanguine Glorifier",
  face_id: "sanguine_glorifier",
  mana_cost: "{3}{W}",
  types: ["Creature", "Vampire", "Cleric"],
  power: 3,
  toughness: 3,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [PutCounters(
      counter: PlusOnePlusOne,
      count: 1,
      subject: Chosen((kind: Creature, controller: You, required_subtypes: ["Vampire"], excluded_objects: [Source])),
    )],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose another target Vampire you control", effect_indices: [0])])),
  )],
)
''',
)

write(
    "jade_bearer",
    header(
        "72ddbc9b-f584-433f-9d2d-6e4c374129c4",
        COUNTER_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "jade_bearer",
  name: "Jade Bearer",
  face_id: "jade_bearer",
  mana_cost: "{G}",
  types: ["Creature", "Merfolk", "Shaman"],
  power: 1,
  toughness: 1,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [PutCounters(
      counter: PlusOnePlusOne,
      count: 1,
      subject: Chosen((kind: Creature, controller: You, required_subtypes: ["Merfolk"], excluded_objects: [Source])),
    )],
    targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose another target Merfolk you control", effect_indices: [0])])),
  )],
)
''',
)

print("wrote batch B definitions")
