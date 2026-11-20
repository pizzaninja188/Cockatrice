//! Registry conformance for the queue-v45 reuse-only cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::mana::ManaCost;
use tricerules_cards::primitives::{
    AbilityCost, CastTriggerPlayer, Color, EffectSubject, GameCondition, InstructionKind,
    PlayerRecipient, ResolutionCost, SpellCastFilter, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardDefinition, CardFace, Layout};

fn definition(id: &str) -> &'static CardDefinition {
    tricerules_cards::registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn identity(
    id: &str,
    name: &str,
    mana: &str,
    types: &[&str],
    body: Option<(u32, u32)>,
) -> &'static CardFace {
    let definition = definition(id);
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(
        tricerules_cards::registry::global().id_for_name(name),
        Some(id),
        "{id}"
    );
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), id, "{id} face id");
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    assert_eq!(
        (f.power, f.toughness),
        body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
        "{id} body"
    );
    f
}

fn spell_cast_filter(
    ability: &tricerules_cards::primitives::TriggeredAbilityDef,
) -> (CastTriggerPlayer, Color) {
    match ability.trigger {
        TriggerCondition::WheneverPlayerCastsSpell {
            caster,
            filter:
                SpellCastFilter {
                    is_color: Some(color),
                    ..
                },
            ..
        } => (caster, color),
        ref other => panic!("expected a cast trigger, got {other:?}"),
    }
}

fn single_choose(
    ability: &tricerules_cards::primitives::TriggeredAbilityDef,
) -> (&InstructionKind, bool) {
    let program = ability.program.as_ref().expect("typed program");
    let [step] = program.body.steps.as_slice() else {
        panic!("one Choose step");
    };
    (&step.kind, true)
}

// ── 01 spell-life artifacts with an optional {1} ─────────────────────────────

#[test]
fn q45_spell_life_pay_maps_definitions() {
    for (id, name, color, mana, types, body) in [
        (
            "wooden_sphere",
            "Wooden Sphere",
            Color::Green,
            "{1}",
            &["Artifact"][..],
            None,
        ),
        (
            "ivory_cup",
            "Ivory Cup",
            Color::White,
            "{1}",
            &["Artifact"][..],
            None,
        ),
        (
            "crystal_rod",
            "Crystal Rod",
            Color::Blue,
            "{1}",
            &["Artifact"][..],
            None,
        ),
        (
            "iron_star",
            "Iron Star",
            Color::Red,
            "{1}",
            &["Artifact"][..],
            None,
        ),
        (
            "throne_of_bone",
            "Throne of Bone",
            Color::Black,
            "{1}",
            &["Artifact"][..],
            None,
        ),
        (
            "apothecary_initiate",
            "Apothecary Initiate",
            Color::White,
            "{W}",
            &["Creature", "Kithkin", "Cleric"][..],
            Some((1, 1)),
        ),
    ] {
        let f = identity(id, name, mana, types, body);
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one triggered ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            spell_cast_filter(ability),
            (CastTriggerPlayer::AnyPlayer, color),
            "{id}"
        );
        let (kind, _) = single_choose(ability);
        let InstructionKind::Choose {
            optional: true,
            branches,
            otherwise,
            ..
        } = kind
        else {
            panic!("{id}: optional Choose");
        };
        assert!(otherwise.steps.is_empty(), "{id}");
        let [branch] = branches.as_slice() else {
            panic!("{id}: one paid branch");
        };
        assert_eq!(
            branch.cost,
            ResolutionCost::Mana(ManaCost::parse("{1}").unwrap()),
            "{id}"
        );
        assert!(
            matches!(
                branch.body.steps.as_slice(),
                [step] if matches!(
                    &step.kind,
                    InstructionKind::Perform(effect)
                        if matches!(effect.as_ref(), SpellEffectKind::GainLife { amount: tricerules_cards::Amount::Fixed(1), .. })
                )
            ),
            "{id}: gain one life"
        );
    }
}

// ── 02 spell-life artifacts without a payment ────────────────────────────────

#[test]
fn q45_spell_life_free_maps_definitions() {
    for (id, name, color) in [
        ("krakens_eye", "Kraken's Eye", Color::Blue),
        ("angels_feather", "Angel's Feather", Color::White),
        ("wurms_tooth", "Wurm's Tooth", Color::Green),
        ("dragons_claw", "Dragon's Claw", Color::Red),
        ("demons_horn", "Demon's Horn", Color::Black),
    ] {
        let f = identity(id, name, "{2}", &["Artifact"], None);
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one triggered ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            spell_cast_filter(ability),
            (CastTriggerPlayer::AnyPlayer, color),
            "{id}"
        );
        let (kind, _) = single_choose(ability);
        let InstructionKind::Choose {
            optional: true,
            branches,
            otherwise,
            ..
        } = kind
        else {
            panic!("{id}: optional free Choose");
        };
        assert!(otherwise.steps.is_empty(), "{id}");
        let [branch] = branches.as_slice() else {
            panic!("{id}: one free branch");
        };
        assert_eq!(branch.cost, ResolutionCost::None, "{id}");
        assert!(
            matches!(
                branch.body.steps.as_slice(),
                [step] if matches!(
                    &step.kind,
                    InstructionKind::Perform(effect)
                        if matches!(effect.as_ref(), SpellEffectKind::GainLife { amount: tricerules_cards::Amount::Fixed(1), .. })
                )
            ),
            "{id}: gain one life"
        );
    }
}

// ── 03 upkeep sacrifice unless the printed mana is paid ──────────────────────

#[test]
fn q45_upkeep_sacrifice_maps_definitions() {
    for (id, name, mana, cost, body, line) in [
        ("darba", "Darba", "{3}{G}", "{G}{G}", Some((5, 4)), 1u16),
        (
            "hungry_mist",
            "Hungry Mist",
            "{2}{G}{G}",
            "{G}{G}",
            Some((6, 2)),
            1,
        ),
        (
            "school_of_piranha",
            "School of Piranha",
            "{1}{U}",
            "{1}{U}",
            Some((3, 3)),
            1,
        ),
        (
            "phantasmal_forces",
            "Phantasmal Forces",
            "{3}{U}",
            "{U}",
            Some((4, 1)),
            2,
        ),
        (
            "spindrift_drake",
            "Spindrift Drake",
            "{U}",
            "{U}",
            Some((2, 1)),
            2,
        ),
        (
            "molting_harpy",
            "Molting Harpy",
            "{B}",
            "{2}",
            Some((2, 1)),
            2,
        ),
    ] {
        let types: &[&str] = if id == "phantasmal_forces" {
            &["Creature", "Illusion"]
        } else if id == "spindrift_drake" {
            &["Creature", "Drake"]
        } else if id == "molting_harpy" {
            &["Creature", "Harpy", "Mercenary"]
        } else if id == "school_of_piranha" {
            &["Creature", "Fish"]
        } else if id == "hungry_mist" {
            &["Creature", "Elemental"]
        } else {
            &["Creature", "Bird", "Beast"]
        };
        let f = identity(id, name, mana, types, body);
        assert_eq!(
            ability_copy(f).presentation,
            AbilityPresentation::OracleLines(vec![line]),
            "{id}"
        );
        let ability = &f.triggered_abilities[0];
        assert!(
            matches!(
                ability.trigger,
                TriggerCondition::AtBeginningOfUpkeep {
                    player: CastTriggerPlayer::Controller
                }
            ),
            "{id}: your upkeep"
        );
        let (kind, _) = single_choose(ability);
        let InstructionKind::Choose {
            optional: true,
            branches,
            otherwise,
            ..
        } = kind
        else {
            panic!("{id}: optional Choose");
        };
        let [branch] = branches.as_slice() else {
            panic!("{id}: one paid branch");
        };
        assert_eq!(
            branch.cost,
            ResolutionCost::Mana(ManaCost::parse(cost).unwrap()),
            "{id}"
        );
        assert!(
            matches!(
                otherwise.steps.as_slice(),
                [step] if matches!(
                    &step.kind,
                    InstructionKind::Perform(effect)
                        if matches!(
                            effect.as_ref(),
                            SpellEffectKind::Sacrifice { subject: EffectSubject::Source }
                        )
                )
            ),
            "{id}: otherwise sacrifice the source"
        );
    }
}

fn ability_copy(
    face: &'static CardFace,
) -> &'static tricerules_cards::primitives::TriggeredAbilityDef {
    &face.triggered_abilities[0]
}

// ── 04 opponent-upkeep hand-size damage ─────────────────────────────────────

#[test]
fn q45_upkeep_hand_damage_maps_definitions() {
    for (id, name, mana, types, body, damage) in [
        (
            "lavaborn_muse",
            "Lavaborn Muse",
            "{3}{R}",
            &["Creature", "Spirit"][..],
            Some((3, 3)),
            3u32,
        ),
        (
            "paupers_cage",
            "Paupers' Cage",
            "{3}",
            &["Artifact"][..],
            None,
            2,
        ),
        (
            "hellfire_mongrel",
            "Hellfire Mongrel",
            "{2}{R}",
            &["Creature", "Elemental", "Dog"][..],
            Some((2, 2)),
            2,
        ),
    ] {
        let f = identity(id, name, mana, types, body);
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one trigger");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(
            matches!(
                ability.trigger,
                TriggerCondition::AtBeginningOfUpkeep {
                    player: CastTriggerPlayer::Opponent
                }
            ),
            "{id}: opponent upkeep"
        );
        assert!(
            matches!(
                ability.intervening_if,
                Some(GameCondition::CardsInHand { max: Some(2), .. })
            ),
            "{id}: two-or-fewer intervening if"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::DamagePlayer { amount: tricerules_cards::Amount::Fixed(value), who: PlayerRecipient::AffectedPlayer }] if *value == damage
            ),
            "{id}: printed damage to that player"
        );
    }
}

// ── 05 dies into Insect tokens ──────────────────────────────────────────────

#[test]
fn q45_dies_insect_tokens_maps_definitions() {
    for (id, name, mana, types, body, count) in [
        (
            "symbiotic_beast",
            "Symbiotic Beast",
            "{4}{G}{G}",
            &["Creature", "Insect", "Beast"][..],
            Some((4, 4)),
            4u32,
        ),
        (
            "symbiotic_wurm",
            "Symbiotic Wurm",
            "{5}{G}{G}{G}",
            &["Creature", "Wurm"][..],
            Some((7, 7)),
            7,
        ),
        (
            "symbiotic_elf",
            "Symbiotic Elf",
            "{3}{G}",
            &["Creature", "Elf"][..],
            Some((2, 2)),
            2,
        ),
    ] {
        let f = identity(id, name, mana, types, body);
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one trigger");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfDies, "{id}");
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::CreateTokens { token, count: amount, .. }]
                    if token == "insect_g_1_1" && *amount == tricerules_cards::Amount::Fixed(count)
            ),
            "{id}: printed Insect count"
        );
    }
}

// ── 06 enters with Eldrazi Spawn ────────────────────────────────────────────

#[test]
fn q45_etb_eldrazi_spawn_maps_definitions() {
    for (id, name, mana, body, count) in [
        (
            "emrakuls_hatcher",
            "Emrakul's Hatcher",
            "{4}{R}",
            Some((3, 3)),
            3u32,
        ),
        (
            "kozileks_predator",
            "Kozilek's Predator",
            "{3}{G}",
            Some((3, 3)),
            2,
        ),
        ("dread_drone", "Dread Drone", "{4}{B}", Some((4, 1)), 2),
    ] {
        let f = identity(id, name, mana, &["Creature", "Eldrazi", "Drone"], body);
        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one trigger");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::CreateTokens { token, count: amount, .. }]
                    if token == "eldrazi_spawn_c_0_1" && *amount == tricerules_cards::Amount::Fixed(count)
            ),
            "{id}: printed Spawn count"
        );
    }
}

// ── 07 sacrifice for a pump ─────────────────────────────────────────────────

#[test]
fn q45_sacrifice_pump_maps_definitions() {
    for (id, name, mana, types, pump) in [
        (
            "shield_mate",
            "Shield Mate",
            "{W}",
            &["Creature", "Human", "Soldier"][..],
            (0i32, 4i32),
        ),
        (
            "seal_of_strength",
            "Seal of Strength",
            "{G}",
            &["Enchantment"][..],
            (3, 3),
        ),
        (
            "child_of_thorns",
            "Child of Thorns",
            "{G}",
            &["Creature", "Spirit"][..],
            (1, 1),
        ),
    ] {
        let body = if id == "seal_of_strength" {
            None
        } else {
            Some((1, 1))
        };
        let f = identity(id, name, mana, types, body);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(ability.costs, vec![AbilityCost::SacrificeSelf], "{id}");
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Chosen(filter),
                    ..
                }] if (*power, *toughness) == pump && filter.kind == tricerules_cards::primitives::TargetKind::Creature
            ),
            "{id}: printed pump on a target creature"
        );
    }
}

// ── 08 return up to N target creatures ──────────────────────────────────────

#[test]
fn q45_bounce_up_to_maps_definitions() {
    for (id, name, mana, types, max) in [
        (
            "into_the_void",
            "Into the Void",
            "{3}{U}",
            &["Sorcery"][..],
            2u32,
        ),
        ("waterwhirl", "Waterwhirl", "{4}{U}{U}", &["Instant"][..], 2),
        (
            "captivating_gyre",
            "Captivating Gyre",
            "{4}{U}{U}",
            &["Sorcery"][..],
            3,
        ),
    ] {
        let f = identity(id, name, mana, types, None);
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::ReturnToOwnersHand { subject: EffectSubject::Chosen(filter) }]
                    if filter.kind == tricerules_cards::primitives::TargetKind::Creature
            ),
            "{id}: return the chosen creatures"
        );
        let group = &f.targeting.as_ref().expect("targeting").groups[0];
        assert_eq!(
            (group.min, group.max),
            (0, max),
            "{id}: up to printed maximum"
        );
    }
}

// ── 09 tap up to N target creatures ─────────────────────────────────────────

#[test]
fn q45_tap_up_to_maps_definitions() {
    for (id, name, mana, types, max) in [
        (
            "lead_astray",
            "Lead Astray",
            "{1}{W}",
            &["Instant"][..],
            2u32,
        ),
        ("downpour", "Downpour", "{1}{U}", &["Instant"][..], 3),
        (
            "terashis_cry",
            "Terashi's Cry",
            "{3}{W}",
            &["Sorcery", "Arcane"][..],
            3,
        ),
    ] {
        let f = identity(id, name, mana, types, None);
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::Tap { subject: EffectSubject::Chosen(filter) }]
                    if filter.kind == tricerules_cards::primitives::TargetKind::Creature
            ),
            "{id}: tap the chosen creatures"
        );
        let group = &f.targeting.as_ref().expect("targeting").groups[0];
        assert_eq!(
            (group.min, group.max),
            (0, max),
            "{id}: up to printed maximum"
        );
    }
}

// ── fingerprints ────────────────────────────────────────────────────────────

#[test]
fn q45_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let cards: [(&str, &str); 35] = [
        ("wooden_sphere", "Wooden Sphere"),
        ("ivory_cup", "Ivory Cup"),
        ("crystal_rod", "Crystal Rod"),
        ("iron_star", "Iron Star"),
        ("throne_of_bone", "Throne of Bone"),
        ("apothecary_initiate", "Apothecary Initiate"),
        ("krakens_eye", "Kraken's Eye"),
        ("angels_feather", "Angel's Feather"),
        ("wurms_tooth", "Wurm's Tooth"),
        ("dragons_claw", "Dragon's Claw"),
        ("demons_horn", "Demon's Horn"),
        ("darba", "Darba"),
        ("hungry_mist", "Hungry Mist"),
        ("school_of_piranha", "School of Piranha"),
        ("phantasmal_forces", "Phantasmal Forces"),
        ("spindrift_drake", "Spindrift Drake"),
        ("molting_harpy", "Molting Harpy"),
        ("lavaborn_muse", "Lavaborn Muse"),
        ("paupers_cage", "Paupers' Cage"),
        ("hellfire_mongrel", "Hellfire Mongrel"),
        ("symbiotic_beast", "Symbiotic Beast"),
        ("symbiotic_wurm", "Symbiotic Wurm"),
        ("symbiotic_elf", "Symbiotic Elf"),
        ("emrakuls_hatcher", "Emrakul's Hatcher"),
        ("kozileks_predator", "Kozilek's Predator"),
        ("dread_drone", "Dread Drone"),
        ("shield_mate", "Shield Mate"),
        ("seal_of_strength", "Seal of Strength"),
        ("child_of_thorns", "Child of Thorns"),
        ("into_the_void", "Into the Void"),
        ("waterwhirl", "Waterwhirl"),
        ("captivating_gyre", "Captivating Gyre"),
        ("lead_astray", "Lead Astray"),
        ("downpour", "Downpour"),
        ("terashis_cry", "Terashi's Cry"),
    ];
    for (id, name) in cards {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id, "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4].len(), 64, "fingerprint length for {id}");
        assert!(
            fields[4].chars().all(|ch| ch.is_ascii_hexdigit()),
            "fingerprint digest for {id}"
        );
        assert_eq!(definition(id).primary_face().name, name, "{id}");
    }
}
