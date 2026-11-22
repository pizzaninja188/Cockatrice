#!/usr/bin/env python3
"""Write the queue-v51 reviewed RON definitions and token data."""
from pathlib import Path

ROOT = Path("/home/ubuntu/Cockatrice")
DATA = ROOT / "tricerules" / "tricerules-cards" / "data"
TOKENS = DATA / "tokens"
SHA = "a9a2fd3b550cc5cff4cec9bbc82aaf5fb8e3f781b4b95a896a0d94c15dce7853"


def header(oid: str, note: str) -> str:
    return (
        f"// Oracle {oid}; source/rulings checked 2026-11-20 against the pinned Scryfall bulk\n"
        f"// SHA-256 {SHA}.\n{note}\n"
    )


def write(card_id: str, body: str) -> None:
    (DATA / f"{card_id}.ron").write_text(body, encoding="utf-8")


def token(token_id: str, name: str, types: list[str], colors: list[str] | None,
          power: int, toughness: int, keywords: list[str] | None, note: str) -> None:
    lines = [
        f"// {note}",
        "(",
        f'  id: "{token_id}",',
        f'  name: "{name}",',
        f'  face_id: "{name.lower()}",',
        f'  types: [{", ".join(repr_type(t) for t in types)}],',
    ]
    if colors:
        lines.append(f'  colors: [{", ".join(colors)}],')
    lines += [f"  power: {power},", f"  toughness: {toughness},"]
    if keywords:
        lines.append(f'  keywords: [{", ".join(keywords)}],')
    lines.append(")")
    (TOKENS / f"{token_id}.ron").write_text("\n".join(lines) + "\n", encoding="utf-8")


def repr_type(value: str) -> str:
    return f'"{value}"'


# ── token definitions ─────────────────────────────────────────────────────

token("centaur_g_3_3", "Centaur", ["Creature", "Centaur"], ["Green"], 3, 3, None,
      "3/3 green Centaur creature token (Call of the Conclave).")
token("elemental_ur_4_4", "Elemental", ["Creature", "Elemental"], ["Blue", "Red"], 4, 4, None,
      "4/4 blue and red Elemental creature token (Elemental Summoning).")
token("wurm_g_5_5_trample", "Wurm", ["Creature", "Wurm"], ["Green"], 5, 5, ["Trample"],
      "5/5 green Wurm creature token with trample (Advent of the Wurm).")
token("servo_c_1_1", "Servo", ["Artifact", "Creature", "Servo"], None, 1, 1, None,
      "1/1 colorless Servo artifact creature token (Servo Exhibition).")
token("soldier_w_1_1_lifelink", "Soldier", ["Creature", "Soldier"], ["White"], 1, 1, ["Lifelink"],
      "1/1 white Soldier creature token with lifelink (Sworn Companions).")
token("citizen_w_1_1", "Citizen", ["Creature", "Citizen"], ["White"], 1, 1, None,
      "1/1 white Citizen creature token (Icatian Town).")
token("minotaur_r_2_3_haste", "Minotaur", ["Creature", "Minotaur"], ["Red"], 2, 3, ["Haste"],
      "2/3 red Minotaur creature token with haste (Flurry of Horns).")
token("warrior_w_1_1", "Warrior", ["Creature", "Warrior"], ["White"], 1, 1, None,
      "1/1 white Warrior creature token (Take Up Arms).")
token("golem_c_3_3", "Golem", ["Artifact", "Creature", "Phyrexian", "Golem"], None, 3, 3, None,
      "3/3 colorless Phyrexian Golem artifact creature token (Splicer cycle).")

# ── 01 token spells ───────────────────────────────────────────────────────

TOKEN_NOTE = (
    "// CR 111.1-111.2 (creation and ownership), 111.4 (naming) and 608.2c (resolution\n"
    "// order). No pinned ruling changes the printed behavior."
)

for card_id, oid, name, mana, types, token_id, count in [
    ("call_of_the_conclave", "e1635acd-ed1e-4038-a11a-6518df285253", "Call of the Conclave", "{G}{W}", ["Sorcery"], "centaur_g_3_3", 1),
    ("elemental_summoning", "f9e668f9-21af-4fd8-9a37-6505ab2234e3", "Elemental Summoning", "{3}{U/R}{U/R}", ["Sorcery", "Lesson"], "elemental_ur_4_4", 1),
    ("advent_of_the_wurm", "eb62aa4b-c11b-4195-ae85-cff8f78ce17b", "Advent of the Wurm", "{1}{G}{G}{W}", ["Instant"], "wurm_g_5_5_trample", 1),
    ("spore_swarm", "8fb599a7-2c61-446b-a7a7-cd5367f9b796", "Spore Swarm", "{3}{G}", ["Instant"], "saproling_g_1_1", 3),
    ("masters_call", "d6c9c1f0-050f-40e2-ab6c-6566d0af418c", "Master's Call", "{2}{W}", ["Instant"], "myr_token_c_1_1", 2),
    ("servo_exhibition", "76a6ddc2-f894-4331-89a9-87ca16efd0c0", "Servo Exhibition", "{1}{W}", ["Sorcery"], "servo_c_1_1", 2),
    ("sworn_companions", "d1a9fd14-fecb-4d2a-983c-70143238275b", "Sworn Companions", "{2}{W}", ["Sorcery"], "soldier_w_1_1_lifelink", 2),
    ("icatian_town", "aa8b60b6-cf55-4aaa-9caa-2b17942d8269", "Icatian Town", "{5}{W}", ["Sorcery"], "citizen_w_1_1", 4),
    ("knight_watch", "4965f5fe-a5fe-4976-822c-5cf40fea8c34", "Knight Watch", "{4}{W}", ["Sorcery"], "knight_w_2_2_vigilance", 2),
    ("flurry_of_horns", "0c8ab7bf-7b6a-4a34-b988-9f3f4d84a9ea", "Flurry of Horns", "{4}{R}", ["Sorcery"], "minotaur_r_2_3_haste", 2),
    ("take_up_arms", "61e2d557-f08d-489b-94c8-aff87e5189f9", "Take Up Arms", "{4}{W}", ["Instant"], "warrior_w_1_1", 3),
]:
    type_line = ", ".join(f'"{t}"' for t in types)
    write(card_id, header(oid, TOKEN_NOTE) + f'''(
  id: "{card_id}",
  name: "{name}",
  face_id: "{card_id.replace(chr(45), chr(95))}",
  mana_cost: "{mana}",
  types: [{type_line}],
  spell_effect: [CreateTokens(token: "{token_id}", count: {count})],
)
''')

# ── 02 splicer cycle ──────────────────────────────────────────────────────

for card_id, oid, name, mana, ability in [
    ("wing_splicer", "7fbf6afc-8a16-447b-bdef-b7d0b640f76c", "Wing Splicer", "{3}{U}",
     'AnthemKeyword(filter: (controller: YouControl, subtype: "Golem"), keyword: Flying)'),
    ("sensor_splicer", "9676da93-1e17-4f6c-b610-a242b78c71ab", "Sensor Splicer", "{4}{W}",
     'AnthemKeyword(filter: (controller: YouControl, subtype: "Golem"), keyword: Vigilance)'),
    ("master_splicer", "9e0c5919-aed6-4d98-85c8-2658be78bdf0", "Master Splicer", "{3}{W}",
     'AnthemPt(filter: (controller: YouControl, subtype: "Golem"), delta_power: 1, delta_toughness: 1)'),
    ("blade_splicer", "194166ad-0179-42a3-86b9-ba7f322ec576", "Blade Splicer", "{2}{W}",
     'AnthemKeyword(filter: (controller: YouControl, subtype: "Golem"), keyword: FirstStrike)'),
    ("vital_splicer", "e1899133-635d-46ff-b90d-6f8d46a1ffe1", "Vital Splicer", "{3}{G}",
     None),
]:
    types_literal = '["Creature", "Phyrexian", "Artificer"]' if card_id == "sensor_splicer" else '["Creature", "Phyrexian", "Human", "Artificer"]'
    note = {
        "wing_splicer": "// CR 603.6a (enters trigger), 111.1-111.2 (token), 613.1f (layer 6 flying grant).",
        "sensor_splicer": "// CR 603.6a (enters trigger), 111.1-111.2 (token), 613.1f (layer 6 vigilance grant).",
        "master_splicer": "// CR 603.6a (enters trigger), 111.1-111.2 (token), 613.4c (layer 7c +1/+1).",
        "blade_splicer": "// CR 603.6a (enters trigger), 111.1-111.2 (token), 613.1f (layer 6 first strike grant).",
        "vital_splicer": "// CR 603.6a (enters trigger), 111.1-111.2 (token), 701.19 (regenerate).",
    }[card_id] + " No pinned ruling changes the printed behavior."
    if ability is None:
        write(card_id, header(oid, note) + f'''(
  id: "{card_id}",
  name: "{name}",
  face_id: "{card_id.replace(chr(45), chr(95))}",
  mana_cost: "{mana}",
  types: {types_literal},
  power: 1,
  toughness: 1,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [CreateTokens(token: "golem_c_3_3", count: 1)],
  )],
  activated_abilities: [(
    ability_id: "activated_01",
    presentation: OracleLines([2]),
    costs: [Mana("{{1}}")],
    effect: [Regenerate(subject: Chosen((kind: Creature, controller: You, required_subtypes: ["Golem"])))],
    targeting: Some((groups: [(
      min: 1,
      max: 1,
      prompt: "Choose target Golem you control",
      effect_indices: [0],
    )])),
  )],
)
''')
    else:
        write(card_id, header(oid, note) + f'''(
  id: "{card_id}",
  name: "{name}",
  face_id: "{card_id.replace(chr(45), chr(95))}",
  mana_cost: "{mana}",
  types: {types_literal},
  power: 1,
  toughness: 1,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [CreateTokens(token: "golem_c_3_3", count: 1)],
  )],
  static_abilities: [(
    ability_id: "static_01",
    presentation: OracleLines([2]),
    definition: {ability},
  )],
)
''')

print("wrote tokens and families 01-02")

# ── 03 artifact value tutors ──────────────────────────────────────────────

TUTOR_NOTE = (
    "// CR 603.6a (enters trigger), 701.23 (search), 701.23b/e (private hidden-zone choice),\n"
    "// 701.24 (shuffle) and 608.2 (resolution). The search is optional and the found card is\n"
    "// revealed. Pinned rulings, where any, are recorded in the family record."
)

for card_id, oid, name, low, high in [
    ("trinket_mage", "ffc95093-24a2-4616-a44a-24788e8df9c8", "Trinket Mage", 0, 1),
    ("tribute_mage", "c2b51295-8820-425f-a517-55231123c7de", "Tribute Mage", 2, 2),
    ("trophy_mage", "08afc0d7-192f-4ab6-b6a0-c4265cf5e225", "Trophy Mage", 3, 3),
    ("transit_mage", "991efbf1-299d-4892-aeab-8ff9827f7168", "Transit Mage", 4, 5),
]:
    bounds = f"min_mana_value: Some({low}), max_mana_value: Some({high})"
    write(card_id, header(oid, TUTOR_NOTE) + f'''(
  id: "{card_id}",
  name: "{name}",
  face_id: "{card_id.replace(chr(45), chr(95))}",
  mana_cost: "{{2}}{{U}}",
  types: ["Creature", "Human", "Wizard"],
  power: 2,
  toughness: 2,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [SearchLibrary(
      filter: Some((card_type: Some(Artifact), {bounds})),
      optional: true,
      reveal: true,
    )],
  )],
)
''')

# ── 04 subtype harbingers ────────────────────────────────────────────────

HARBINGER_NOTE = (
    "// CR 603.6a (enters trigger), 701.23a-b (search and failing to find),\n"
    "// 701.20a/701.23e (reveal), 701.24b (shuffle then put the found card on top) and\n"
    "// 608.2 (resolution). No pinned ruling changes the printed behavior."
)

for card_id, oid, name, mana, body, subtypes, extra in [
    ("boggart_harbinger", "56508ae7-6013-4149-8935-96b8d5091baa", "Boggart Harbinger", "{2}{B}", (2, 1), ["Goblin"], None),
    ("kithkin_harbinger", "a3d68848-e8cc-497c-951b-e7111f6d5142", "Kithkin Harbinger", "{2}{W}", (1, 3), ["Kithkin"], None),
    ("giant_harbinger", "2445e58b-87ed-4ab2-8209-a5e1f566fba7", "Giant Harbinger", "{4}{R}", (3, 4), ["Giant"], None),
    ("flamekin_harbinger", "d6585e30-4ca0-4701-b274-b24f3508dd97", "Flamekin Harbinger", "{R}", (1, 1), ["Elemental"], None),
    ("elvish_harbinger", "28ddc23c-5b0b-4747-9a33-6845acf27bec", "Elvish Harbinger", "{2}{G}", (1, 2), ["Elf"], "mana"),
    ("treefolk_harbinger", "98c387bd-c9ba-464a-8380-839a5604a37f", "Treefolk Harbinger", "{G}", (0, 3), ["Treefolk", "Forest"], None),
]:
    if len(subtypes) == 1:
        filt = f'required_subtypes: ["{subtypes[0]}"]'
    else:
        filt = "any_of: Some([" + ", ".join(f'(required_subtypes: ["{s}"])' for s in subtypes) + "])"
    abilities = ''
    if extra == "mana":
        abilities = '''
  activated_abilities: [(
    ability_id: "activated_01",
    presentation: OracleLines([2]),
    costs: [Tap],
    effect: [ProduceMana(options: [(w: 1), (u: 1), (b: 1), (r: 1), (g: 1)])],
  )],
'''
    write(card_id, header(oid, HARBINGER_NOTE) + f'''(
  id: "{card_id}",
  name: "{name}",
  face_id: "{card_id.replace(chr(45), chr(95))}",
  mana_cost: "{mana}",
  types: ["Creature", "{subtypes[0]}", "{"Druid" if card_id == "elvish_harbinger" or card_id == "treefolk_harbinger" else ("Shaman" if card_id in ("boggart_harbinger", "giant_harbinger", "flamekin_harbinger") else "Wizard")}"] ,
  power: {body[0]},
  toughness: {body[1]},
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [SearchLibrary(
      filter: Some(({filt})),
      optional: true,
      reveal: true,
      destination: TopOfLibrary,
    )],
  )],
{abilities})
''')

# ── 05 aura restrictions ─────────────────────────────────────────────────

AURA_NOTE = (
    "// CR 303.4 (Aura enchant), 509.1b (blocking restrictions), 613.1f/613.4c (layer 6\n"
    "// keyword and P/T modifiers), 602.5 (prohibit activated abilities) and 502.3\n"
    "// (doesn't untap). No pinned ruling changes the printed behavior."
)

for card_id, oid, name, mana, statics in [
    ("cloak_of_mists", "13358b1d-0a35-48ed-a7fb-70359fa3998f", "Cloak of Mists", "{1}{U}",
     ['definition: AttachedModifier(restriction: (cant_be_blocked: true))']),
    ("aether_tunnel", "0a90d1fb-7a76-4b62-b001-4a539a8e5df1", "Aether Tunnel", "{1}{U}",
     ['definition: AttachedModifier(delta_power: 1, restriction: (cant_be_blocked: true))']),
    ("protective_bubble", "1fba543c-bd22-4f7b-9ff4-0631af8ee814", "Protective Bubble", "{3}{U}",
     ['definition: AttachedModifier(keywords: [Shroud], restriction: (cant_be_blocked: true))']),
    ("demotion", "c333eb79-300e-4dc0-9126-8dca1bac6ef2", "Demotion", "{W}",
     ['definition: AttachedModifier(restriction: (cant_block: true))',
      'definition: ProhibitActivatedAbilitiesOfAttachedPermanent']),
    ("arrest", "81728b98-8cf9-4734-a318-69184bb4d15c", "Arrest", "{2}{W}",
     ['definition: AttachedModifier(restriction: (cant_attack: true, cant_block: true))',
      'definition: ProhibitActivatedAbilitiesOfAttachedPermanent']),
    ("paralyzing_grasp", "c1459d75-2bc2-4dae-8c7e-4e759a4dffc9", "Paralyzing Grasp", "{2}{U}",
     ['definition: AttachedModifier(doesnt_untap_during_untap_step: true)']),
    ("invisibility", "de26b0c6-dfb7-45a8-9d7f-f8d45522d675", "Invisibility", "{U}{U}",
     ['definition: AttachedModifier(restriction: (cant_be_blocked_by: [(kind: Creature, excluded_subtypes: ["Wall"])]))']),
]:
    nodes = ",\n".join(
        f'    (ability_id: "static_{index:02d}", presentation: OracleLines([2]), {body})'
        for index, body in enumerate(statics, 1)
    )
    write(card_id, header(oid, AURA_NOTE) + f'''(
  id: "{card_id}",
  name: "{name}",
  face_id: "{card_id.replace(chr(45), chr(95))}",
  mana_cost: "{mana}",
  types: ["Enchantment", "Aura"],
  spell_effect: [AuraAttach()],
  static_abilities: [
{nodes},
  ],
)
''')

# ── 06 enters each player discards ───────────────────────────────────────

DISCARD_NOTE = (
    "// CR 603.6a (enters trigger), 701.9 (discard) and 101.4/APNAP (each player's hidden\n"
    "// choice); Earsplitting Rats also uses 701.19 (regenerate). No pinned ruling changes\n"
    "// the printed behavior."
)

for card_id, oid, name, mana, body, extra in [
    ("miasmic_mummy", "b5b049d4-242a-48ec-85d1-1ef66c8cfa4c", "Miasmic Mummy", "{1}{B}", (2, 2), None),
    ("sibsig_icebreakers", "4880c976-b417-48e4-98a2-9e5a8da0d758", "Sibsig Icebreakers", "{2}{B}", (2, 3), None),
    ("earsplitting_rats", "6c14ecbf-0097-4fd8-97a3-a32b532c21e0", "Earsplitting Rats", "{3}{B}", (2, 1), "regen"),
]:
    extra_text = ''
    if extra == "regen":
        extra_text = f'''
  activated_abilities: [(
    ability_id: "activated_01",
    presentation: OracleLines([2]),
    costs: [Discard],
    effect: [Regenerate(subject: Source)],
  )],
'''
    subtype = {"miasmic_mummy": "Zombie Jackal", "sibsig_icebreakers": "Zombie", "earsplitting_rats": "Rat"}[card_id]
    types = ", ".join(f'"{part}"' for part in ["Creature", *subtype.split()])
    write(card_id, header(oid, DISCARD_NOTE) + f'''(
  id: "{card_id}",
  name: "{name}",
  face_id: "{card_id.replace(chr(45), chr(95))}",
  mana_cost: "{mana}",
  types: [{types}],
  power: {body[0]},
  toughness: {body[1]},
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WhenSelfEntersBattlefield,
    effect: [Discard(who: EachPlayer, quantity: Exact(1))],
  )],
{extra_text})
''')

# ── 08 attack team pumps ─────────────────────────────────────────────────

ATTACK_NOTE = (
    "// CR 508.1m/603.2c (attack trigger), 611.2a/514.2 (until end of turn), 611.2c\n"
    "// (affected set locked at resolution), 613.4c (layer 7c pump) and 613.1f (layer 6\n"
    "// keyword grant). No pinned ruling changes the printed behavior."
)

for card_id, oid, name, mana, types, power, toughness, effect in [
    ("goblin_general", "5fab0d9c-ccf7-4c6a-94b4-959be139dad8", "Goblin General", "{1}{R}{R}", ["Creature", "Goblin", "Warrior"], 1, 1,
     'PumpAll(filter: (controller: YouControl, subtype: "Goblin"), power: 1, toughness: 1)'),
    ("riot_ringleader", "420d09b5-aced-4f71-914c-90d19f7476dc", "Riot Ringleader", "{2}{R}", ["Creature", "Human", "Warrior"], 2, 2,
     'PumpAll(filter: (controller: YouControl, subtype: "Human"), power: 1, toughness: 0)'),
    ("blood-chin_rager", "a7036d08-6ee4-49b1-bbbf-c96a937afa15", "Blood-Chin Rager", "{1}{B}", ["Creature", "Human", "Warrior"], 2, 2,
     'GrantKeywordsAll(filter: (controller: YouControl, subtype: "Warrior"), keywords: [Menace])'),
    ("inspiring_unicorn", "1f9797e2-7ebb-41f9-9199-e9ee575eefd3", "Inspiring Unicorn", "{2}{W}{W}", ["Creature", "Unicorn"], 2, 2,
     'PumpAll(filter: (controller: YouControl), power: 1, toughness: 1)'),
    ("chieftain_en-dal", "8aae3a28-dcf3-460e-89f9-4b0c96d6f55e", "Chieftain en-Dal", "{1}{W}{W}", ["Creature", "Human", "Knight"], 2, 2,
     'GrantKeywordsAll(filter: (attacking: true), keywords: [FirstStrike])'),
    ("fangren_pathcutter", "36d1a3f4-7903-41aa-9fb7-401d8789b00c", "Fangren Pathcutter", "{4}{G}{G}", ["Creature", "Beast"], 4, 6,
     'GrantKeywordsAll(filter: (attacking: true), keywords: [Trample])'),
]:
    type_line = ", ".join(f'"{part}"' for part in types)
    write(card_id, header(oid, ATTACK_NOTE) + f'''(
  id: "{card_id}",
  name: "{name}",
  face_id: "{card_id.replace(chr(45), chr(95))}",
  mana_cost: "{mana}",
  types: [{type_line}],
  power: {power},
  toughness: {toughness},
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: WheneverSelfAttacks(minimum_other_attackers: 0),
    effect: [{effect}],
  )],
)
''')

# ── 09 Spirit or Arcane cast payoffs ─────────────────────────────────────

SPIRIT_NOTE = (
    "// CR 601.2i/603.2 (cast trigger), 603.3 (the trigger remains on the stack independently\n"
    "// of the spell), 611.2a/514.2 (until end of turn), 701.19 (regenerate), 122.1a\n"
    "// (+1/+1 counter), 613 (continuous effects) and 509.1b (can't block). Pinned rulings\n"
    "// are in the family record."
)

CAST_TRIGGER = '''WheneverPlayerCastsSpell(
      caster: Controller,
      filter: (any_of: Some([
        (required_subtypes: ["Spirit"]),
        (required_subtypes: ["Arcane"]),
      ])),
    )'''

for card_id, oid, name, mana, body, effect, prompt in [
    ("briarknit_kami", "913dcf97-0804-4b21-8daa-f2b481dc0f2f", "Briarknit Kami", "{3}{G}{G}", (3, 3),
     'PutCounters(subject: Chosen((kind: Creature)), counter: PlusOnePlusOne, count: 1)',
     "Choose target creature"),
    ("horizon_seed", "3970e021-209d-41d3-9466-8b3ef9ab1a4c", "Horizon Seed", "{4}{W}", (2, 1),
     'Regenerate(subject: Chosen((kind: Creature)))',
     "Choose target creature"),
    ("guardian_of_solitude", "712ec339-ffa8-43bb-a826-a7b0f0120b1f", "Guardian of Solitude", "{1}{U}", (1, 2),
     'GrantKeywords(subject: Chosen((kind: Creature)), keywords: [Flying])',
     "Choose target creature"),
    ("kami_of_fires_roar", "c247382f-821e-4f3c-9d34-450794ce74c3", "Kami of Fire's Roar", "{3}{R}", (2, 3),
     'ApplyCombatRestriction(scope: Chosen((kind: Creature)), restriction: (cant_block: true))',
     "Choose target creature"),
    ("soul_of_magma", "1566af29-bd7b-41bb-a589-499348fbd32c", "Soul of Magma", "{3}{R}{R}", (2, 2),
     'DamageTarget(amount: 1, target: (kind: Creature))',
     "Choose target creature"),
]:
    write(card_id, header(oid, SPIRIT_NOTE) + f'''(
  id: "{card_id}",
  name: "{name}",
  face_id: "{card_id.replace(chr(45), chr(95))}",
  mana_cost: "{mana}",
  types: ["Creature", "Spirit"],
  power: {body[0]},
  toughness: {body[1]},
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: OracleLines([1]),
    trigger: {CAST_TRIGGER},
    effect: [{effect}],
    targeting: Some((groups: [(
      min: 1,
      max: 1,
      prompt: "{prompt}",
      effect_indices: [0],
    )])),
  )],
)
''')

print("wrote families 03-09")
