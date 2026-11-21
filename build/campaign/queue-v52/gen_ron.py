#!/usr/bin/env python3
"""Write the queue-v52 reviewed RON definitions."""
from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
DATA = ROOT / "tricerules" / "tricerules-cards" / "data"
SHA = "a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853"

TARGET = '''(any_of: Some([
            (kind: AnyPermanent, permanent_types: [Artifact]),
            (kind: AnyPermanent, permanent_types: [Land]),
          ]))'''

cards = [
    ("demolish", "08c6d2fd-d9ae-4f95-bb9d-7e16b1039814", "Demolish", "{3}{R}", ["Sorcery"],
     "// CR 701.8 (destroy) and 115.1/601.2c (one artifact-or-land target). No pinned ruling.",
     [('Destroy(subject: Chosen(%s))' % TARGET, None)]),
    ("sunder_from_within", "09f75f28-09b9-4a6e-a1b0-671208d274d3", "Sunder from Within", "{2}{R}{R}", ["Sorcery", "Arcane"],
     "// CR 701.8 (destroy), 115.1/601.2c (one artifact-or-land target) and 205.3k (Arcane\n// spell subtype). No pinned ruling.",
     [('Destroy(subject: Chosen(%s))' % TARGET, None)]),
    ("pillage", "0b137853-7cb9-424b-8285-12938991eafb", "Pillage", "{1}{R}{R}", ["Sorcery"],
     "// CR 701.8 and 701.19c (destruction that prevents regeneration) and 115.1/601.2c\n// (one artifact-or-land target). No pinned ruling.",
     [('DestroyPreventingRegeneration(subject: Chosen(%s))' % TARGET, None)]),
    ("victorious_destruction", "fa5d6c02-53b2-4144-87cb-e223b529ba7d", "Victorious Destruction", "{4}{R}", ["Sorcery"],
     "// CR 701.8 (destroy), 119.3 (its controller loses 1 life) and 115.1/601.2c (one\n// artifact-or-land target). No pinned ruling.",
     [('Destroy(subject: Chosen(%s))' % TARGET, None),
      ('LoseLife(amount: Fixed(1), who: ControllerOfTargetGroup(group_index: 0))', None)]),
    ("violent_impact", "d48d9e93-72a2-48f2-8164-8032615ee217", "Violent Impact", "{3}{R}", ["Sorcery"],
     "// CR 701.8 (destroy), 702.29a-b (cycling) and 115.1/601.2c (one artifact-or-land\n// target). No pinned ruling.",
     [('Destroy(subject: Chosen(%s))' % TARGET, None)]),
]

for card_id, oid, name, mana, types, note, effects in cards:
    lines = [
        f"// Oracle {oid}; source/rulings checked 2026-11-21 against the pinned Scryfall bulk",
        f"// SHA-256 {SHA}.",
        note,
        "(",
        f'  id: "{card_id}",',
        f'  name: "{name}",',
        f'  face_id: "{card_id}",',
        f'  mana_cost: "{mana}",',
        '  types: [' + ", ".join(f'"{t}"' for t in types) + "],",
        "  spell_effect: [",
    ]
    for effect, _ in effects:
        lines.append(f"    {effect},")
    lines.append("  ],")
    if card_id == "violent_impact":
        lines += [
            "  activated_abilities: [(",
            '    ability_id: "activated_01",',
            "    presentation: OracleLines([2]),",
            "    source_zone: Hand,",
            '    costs: [Mana("{2}"), DiscardSelf],',
            "    effect: [Draw(count: 1)],",
            "  )],",
        ]
    lines.append(")")
    (DATA / f"{card_id}.ron").write_text("\n".join(lines) + "\n", encoding="utf-8")
print("wrote", len(cards), "cards")
