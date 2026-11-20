#!/usr/bin/env python3
"""Write the queue-v50 reviewed RON definitions (batch C: 07-09)."""
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


BASIC_NOTE = (
    "// CR 603.1 (entry trigger), 701.20a/701.23e (reveal the found card), 701.23\n"
    "// (search) and 701.24 (shuffle)."
)

SEARCH_PROGRAM = '''  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([{line}]),
    trigger: WhenSelfEntersBattlefield,
    program: Some((body: (steps: [
      (id: "search_choice", kind: Choose(
        choice: "search",
        chooser: Controller,
        optional: true,
        branches: [(
          branch_id: "search_for_a_land",
          presentation: Fallback,
          cost: None,
          body: (steps: [
            (id: "search_hand", kind: Perform(SearchLibrary(
              filter: Some({filter}),
              zones: Fixed([Library]),
              reveal: true,
              selection_bounds: Some((min: 0, max: 1)),
            )))
          ]),
        )],
        otherwise: (),
      )),
    ]))),
  )],
'''

BASIC_FILTER = '(card_type: Some(BasicLand))'
GATE_FILTER = '(any_of: Some([(card_type: Some(BasicLand)), (required_subtypes: ["Gate"])]))'
DESERT_FILTER = '(any_of: Some([(card_type: Some(BasicLand)), (required_subtypes: ["Desert"])]))'
CAVE_FILTER = '(any_of: Some([(card_type: Some(BasicLand)), (required_subtypes: ["Cave"])]))'

# ── 07 optional basic-land search into hand ────────────────────────────────

write(
    "gatecreeper_vine",
    header(
        "0f539127-535f-4e0d-abaa-e884521098d2",
        BASIC_NOTE + " CR 702.3 (defender). No pinned ruling.",
    )
    + '''(
  id: "gatecreeper_vine",
  name: "Gatecreeper Vine",
  face_id: "gatecreeper_vine",
  mana_cost: "{1}{G}",
  types: ["Creature", "Plant"],
  power: 0,
  toughness: 2,
  keywords: [Defender],
'''
    + SEARCH_PROGRAM.format(line=2, filter=GATE_FILTER)
    + ''')\n''',
)

write(
    "district_guide",
    header(
        "1eb4c001-1115-4b99-bbbf-703eb551a381",
        BASIC_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "district_guide",
  name: "District Guide",
  face_id: "district_guide",
  mana_cost: "{2}{G}",
  types: ["Creature", "Elf", "Scout"],
  power: 2,
  toughness: 2,
'''
    + SEARCH_PROGRAM.format(line=1, filter=GATE_FILTER)
    + ''')\n''',
)

write(
    "farfinder",
    header(
        "7d306de8-a93b-429b-9436-0501be97dc8d",
        BASIC_NOTE + " CR 702.20 (vigilance). No pinned ruling.",
    )
    + '''(
  id: "farfinder",
  name: "Farfinder",
  face_id: "farfinder",
  mana_cost: "{3}",
  types: ["Creature", "Fox"],
  power: 1,
  toughness: 1,
  keywords: [Vigilance],
'''
    + SEARCH_PROGRAM.format(line=2, filter=BASIC_FILTER)
    + ''')\n''',
)

write(
    "pilgrims_eye",
    header(
        "66155010-cc2c-449a-85d5-92c95aade514",
        BASIC_NOTE + " CR 702.9 (flying). No pinned ruling.",
    )
    + '''(
  id: "pilgrims_eye",
  name: "Pilgrim's Eye",
  face_id: "pilgrims_eye",
  mana_cost: "{3}",
  types: ["Artifact", "Creature", "Thopter"],
  power: 1,
  toughness: 1,
  keywords: [Flying],
'''
    + SEARCH_PROGRAM.format(line=2, filter=BASIC_FILTER)
    + ''')\n''',
)

# ── 08 Treasure on death or entry ─────────────────────────────────────────

TREASURE_NOTE = (
    "// CR 603.6a/603.6c/603.10a (entry and battlefield-to-graveyard triggers)\n"
    "// and 111.1 (Treasure token creation)."
)

write(
    "dire_fleet_hoarder",
    header(
        "7d48ad95-d428-4e2d-b697-56907c3ce579",
        TREASURE_NOTE + " No pinned ruling.",
    )
    + '''(
  id: "dire_fleet_hoarder",
  name: "Dire Fleet Hoarder",
  face_id: "dire_fleet_hoarder",
  mana_cost: "{1}{B}",
  types: ["Creature", "Human", "Pirate"],
  power: 2,
  toughness: 1,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfDies,
    effect: [CreateTokens(token: "treasure", count: 1)],
  )],
)
''',
)

write(
    "jewel-eyed_cobra",
    header(
        "bbfee53c-30a6-4ed0-beef-5c254be75fc5",
        TREASURE_NOTE + " CR 702.2 (deathtouch). No pinned ruling.",
    )
    + '''(
  id: "jewel-eyed_cobra",
  name: "Jewel-Eyed Cobra",
  face_id: "jewel_eyed_cobra",
  mana_cost: "{2}{G}",
  types: ["Creature", "Snake"],
  power: 3,
  toughness: 1,
  keywords: [Deathtouch],
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([2]),
    trigger: WhenSelfDies,
    effect: [CreateTokens(token: "treasure", count: 1)],
  )],
)
''',
)

write(
    "patron_of_the_arts",
    header(
        "47f8df80-abc9-4d46-8078-8fbc23430259",
        TREASURE_NOTE
        + " The one printed ability observes both the entry event and the\n"
        "// battlefield-to-graveyard event.",
    )
    + '''(
  id: "patron_of_the_arts",
  name: "Patron of the Arts",
  face_id: "patron_of_the_arts",
  mana_cost: "{2}{R}",
  types: ["Creature", "Dragon", "Noble"],
  power: 3,
  toughness: 1,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersOrIsPutIntoGraveyardFromBattlefield,
    effect: [CreateTokens(token: "treasure", count: 1)],
  )],
)
''',
)

# ── 09 destroy artifact or enchantment (and optional flying creature) ─────

write(
    "naturalize",
    header(
        "bdb3ca68-ec1f-4e16-81cc-d23f8f52c728",
        "// CR 115.1 (target), 601.2c (target at cast) and 701.8 (destroy). No pinned ruling.",
    )
    + '''(
  id: "naturalize",
  name: "Naturalize",
  face_id: "naturalize",
  mana_cost: "{1}{G}",
  types: ["Instant"],
  spell_effect: [Destroy(subject: Chosen((
    any_of: Some([
      (kind: AnyPermanent, permanent_types: [Artifact]),
      (kind: AnyPermanent, permanent_types: [Enchantment]),
    ]),
  )))],
)
''',
)

write(
    "natures_chant",
    header(
        "46e4ace2-e8bf-4b72-81de-286215886a10",
        "// CR 115.1 (target), 601.2c (target at cast) and 701.8 (destroy). No pinned ruling.",
    )
    + '''(
  id: "natures_chant",
  name: "Nature's Chant",
  face_id: "natures_chant",
  mana_cost: "{1}{G/W}",
  types: ["Instant"],
  spell_effect: [Destroy(subject: Chosen((
    any_of: Some([
      (kind: AnyPermanent, permanent_types: [Artifact]),
      (kind: AnyPermanent, permanent_types: [Enchantment]),
    ]),
  )))],
)
''',
)

write(
    "return_to_the_earth",
    header(
        "722f2965-a1ec-4b97-a4bd-b99364f58b1e",
        "// CR 115.1 (target), 601.2c (target at cast) and 701.8 (destroy).",
    )
    + '''(
  id: "return_to_the_earth",
  name: "Return to the Earth",
  face_id: "return_to_the_earth",
  mana_cost: "{3}{G}",
  types: ["Instant"],
  spell_effect: [Destroy(subject: Chosen((
    any_of: Some([
      (kind: AnyPermanent, permanent_types: [Artifact]),
      (kind: AnyPermanent, permanent_types: [Enchantment]),
      (kind: Creature, required_keywords: [Flying]),
    ]),
  )))],
)
''',
)

print("wrote batch C definitions")
