//! Registry conformance for the queue-v50 reviewed cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{
    CardTypeFilter, CounterKind, EffectSubject, GraveyardDestination, Keyword, SpellEffectKind,
    TargetController, TargetKind, TriggerCondition,
};
use tricerules_cards::{registry, CardDefinition, CardFace, Layout};

fn definition(id: &str) -> &'static CardDefinition {
    registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn face_id(name: &str) -> String {
    let folded = name
        .to_lowercase()
        .replace(['\'', ','], "")
        .replace(['é', 'è', 'ê'], "e")
        .replace(['à', 'â'], "a")
        .replace(['ô'], "o");
    let mut text = String::new();
    let mut last_underscore = false;
    for ch in folded.chars() {
        if ch.is_ascii_alphanumeric() {
            text.push(ch);
            last_underscore = false;
        } else if !last_underscore {
            text.push('_');
            last_underscore = true;
        }
    }
    text.trim_matches('_').to_string()
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
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), face_id(name), "{id} face id");
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

// ── 01 put target on top of its owner's library ───────────────────────────

#[test]
fn q50_put_top_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        flashback: Option<&'static str>,
    }
    for case in [
        Case {
            id: "repel",
            name: "Repel",
            mana: "{3}{U}",
            types: &["Instant"],
            flashback: None,
        },
        Case {
            id: "grasp_of_phantoms",
            name: "Grasp of Phantoms",
            mana: "{3}{U}",
            types: &["Sorcery"],
            flashback: Some("{7}{U}"),
        },
        Case {
            id: "run_aground",
            name: "Run Aground",
            mana: "{3}{U}",
            types: &["Instant"],
            flashback: None,
        },
        Case {
            id: "banishment_decree",
            name: "Banishment Decree",
            mana: "{3}{W}{W}",
            types: &["Instant"],
            flashback: None,
        },
        Case {
            id: "temporal_eddy",
            name: "Temporal Eddy",
            mana: "{2}{U}{U}",
            types: &["Sorcery"],
            flashback: None,
        },
        Case {
            id: "fallow_earth",
            name: "Fallow Earth",
            mana: "{2}{G}",
            types: &["Sorcery"],
            flashback: None,
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, None);
        assert!(f.spell_program.is_some(), "{}: placement program", case.id);
        assert!(f.spell_effect.is_empty(), "{}", case.id);
        assert!(f.triggered_abilities.is_empty(), "{}", case.id);
        let targeting = f.targeting.as_ref().expect("one target");
        assert_eq!(targeting.groups.len(), 1, "{}", case.id);
        assert_eq!(targeting.groups[0].min, 1, "{}", case.id);
        assert_eq!(targeting.groups[0].max, 1, "{}", case.id);
        assert_eq!(
            f.flashback_cost.as_ref().map(|cost| cost.to_string()),
            case.flashback.map(str::to_string),
            "{}: flashback",
            case.id
        );
    }
}

// ── 02 filtered graveyard cards to hand ────────────────────────────────────

#[test]
fn q50_graveyard_to_hand_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
    }
    for case in [
        Case {
            id: "boggart_birth_rite",
            name: "Boggart Birth Rite",
            mana: "{B}",
            types: &["Kindred", "Sorcery", "Goblin"],
        },
        Case {
            id: "argivian_find",
            name: "Argivian Find",
            mana: "{W}",
            types: &["Instant"],
        },
        Case {
            id: "natures_spiral",
            name: "Nature's Spiral",
            mana: "{1}{G}",
            types: &["Sorcery"],
        },
        Case {
            id: "deja_vu",
            name: "Déjà Vu",
            mana: "{2}{U}",
            types: &["Sorcery"],
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, None);
        let [SpellEffectKind::MoveGraveyardCards {
            filter,
            destination,
            ..
        }] = f.spell_effect.as_slice()
        else {
            panic!("{}: one graveyard move", case.id)
        };
        assert_eq!(*destination, GraveyardDestination::Hand, "{}", case.id);
        assert_eq!(
            filter.owner,
            Default::default(),
            "{}: your graveyard",
            case.id
        );
        let card = filter.card.as_ref().expect("printed card filter");
        match case.id {
            "boggart_birth_rite" => {
                assert_eq!(card.required_subtypes, ["Goblin"], "{}", case.id);
            }
            "argivian_find" => {
                let branches = card.any_of.as_ref().expect("artifact or enchantment");
                assert_eq!(branches.len(), 2, "{}", case.id);
                assert_eq!(branches[0].card_type, Some(CardTypeFilter::Artifact));
                assert_eq!(branches[1].card_type, Some(CardTypeFilter::Enchantment));
            }
            "natures_spiral" => {
                let branches = card.any_of.as_ref().expect("land or nonland permanent");
                assert_eq!(branches.len(), 2, "{}", case.id);
                assert_eq!(branches[0].card_type, Some(CardTypeFilter::Land));
                assert_eq!(
                    branches[1].card_type,
                    Some(CardTypeFilter::NonlandPermanent)
                );
            }
            "deja_vu" => assert_eq!(card.card_type, Some(CardTypeFilter::Sorcery)),
            _ => unreachable!(),
        }
    }
}

// ── 03 tap plus stun counters ──────────────────────────────────────────────

#[test]
fn q50_tap_stun_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        counters: u32,
        scry: u32,
        opponent_only: bool,
        draw: bool,
    }
    for case in [
        Case {
            id: "cryogenic_stasis",
            name: "Cryogenic Stasis",
            mana: "{1}{U}",
            types: &["Instant"],
            counters: 1,
            scry: 0,
            opponent_only: false,
            draw: true,
        },
        Case {
            id: "impede_momentum",
            name: "Impede Momentum",
            mana: "{1}{U}",
            types: &["Sorcery"],
            counters: 3,
            scry: 1,
            opponent_only: false,
            draw: false,
        },
        Case {
            id: "tranquilize",
            name: "Tranquilize",
            mana: "{1}{U}",
            types: &["Sorcery"],
            counters: 3,
            scry: 0,
            opponent_only: true,
            draw: false,
        },
        Case {
            id: "freeze_in_place",
            name: "Freeze in Place",
            mana: "{1}{U}",
            types: &["Sorcery"],
            counters: 3,
            scry: 2,
            opponent_only: true,
            draw: false,
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, None);
        assert!(
            matches!(f.spell_effect[0], SpellEffectKind::Tap { .. }),
            "{}",
            case.id
        );
        let SpellEffectKind::PutCounters {
            counter,
            count,
            subject,
        } = &f.spell_effect[1]
        else {
            panic!("{}: stun placement", case.id)
        };
        assert_eq!(*counter, CounterKind::Stun, "{}", case.id);
        let tricerules_cards::primitives::Amount::Fixed(count) = count else {
            panic!("{}: fixed stun count", case.id)
        };
        assert_eq!(*count, case.counters, "{}", case.id);
        let EffectSubject::Chosen(filter) = subject else {
            panic!("{}: chosen target", case.id)
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{}", case.id);
        assert_eq!(
            filter.controller,
            if case.opponent_only {
                TargetController::Opponent
            } else {
                TargetController::Any
            },
            "{}: controller scope",
            case.id
        );
        let has_scry = f
            .spell_effect
            .iter()
            .any(|effect| matches!(effect, SpellEffectKind::Scry { .. }));
        assert_eq!(has_scry, case.scry > 0, "{}", case.id);
        let has_draw = f
            .spell_effect
            .iter()
            .any(|effect| matches!(effect, SpellEffectKind::Draw { .. }));
        assert_eq!(has_draw, case.draw, "{}", case.id);
    }
}

// ── 04-06 enters-triggered targeted modifications ─────────────────────────

fn only_trigger(f: &CardFace) -> &tricerules_cards::TriggeredAbilityDef {
    assert_eq!(f.triggered_abilities.len(), 1, "{}", f.name);
    &f.triggered_abilities[0]
}

fn chosen_filter(subject: &EffectSubject) -> &tricerules_cards::primitives::ObjectFilter {
    match subject {
        EffectSubject::Chosen(filter) => filter,
        other => panic!("expected chosen subject, got {other:?}"),
    }
}

#[test]
fn q50_etb_shrink_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        body: (u32, u32),
        keywords: &'static [Keyword],
        power: i32,
        toughness: i32,
    }
    for case in [
        Case {
            id: "eyeblight_assassin",
            name: "Eyeblight Assassin",
            mana: "{2}{B}",
            types: &["Creature", "Elf", "Assassin"],
            body: (2, 2),
            keywords: &[],
            power: -1,
            toughness: -1,
        },
        Case {
            id: "black_dragon",
            name: "Black Dragon",
            mana: "{5}{B}{B}",
            types: &["Creature", "Dragon"],
            body: (4, 4),
            keywords: &[Keyword::Flying],
            power: -3,
            toughness: -3,
        },
        Case {
            id: "faerie_duelist",
            name: "Faerie Duelist",
            mana: "{1}{U}",
            types: &["Creature", "Faerie", "Rogue"],
            body: (1, 2),
            keywords: &[Keyword::Flash, Keyword::Flying],
            power: -2,
            toughness: 0,
        },
        Case {
            id: "brinebarrow_intruder",
            name: "Brinebarrow Intruder",
            mana: "{U}",
            types: &["Creature", "Human", "Rogue"],
            body: (1, 2),
            keywords: &[Keyword::Flash],
            power: -2,
            toughness: 0,
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, Some(case.body));
        assert_eq!(f.keywords, case.keywords, "{}", case.id);
        let trigger = only_trigger(f);
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            case.id
        );
        let SpellEffectKind::PumpTarget {
            power,
            toughness,
            subject,
            ..
        } = &trigger.effect[0]
        else {
            panic!("{}: pump", case.id)
        };
        assert_eq!(
            (*power, *toughness),
            (case.power, case.toughness),
            "{}",
            case.id
        );
        let filter = chosen_filter(subject);
        assert_eq!(filter.controller, TargetController::Opponent, "{}", case.id);
    }
}

#[test]
fn q50_etb_pump_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        body: (u32, u32),
        power: i32,
        toughness: i32,
        indestructible: bool,
    }
    for case in [
        Case {
            id: "haazda_officer",
            name: "Haazda Officer",
            mana: "{2}{W}",
            types: &["Creature", "Human", "Soldier"],
            body: (3, 2),
            power: 1,
            toughness: 1,
            indestructible: false,
        },
        Case {
            id: "herald_of_the_fair",
            name: "Herald of the Fair",
            mana: "{2}{W}",
            types: &["Creature", "Human"],
            body: (3, 2),
            power: 1,
            toughness: 1,
            indestructible: false,
        },
        Case {
            id: "rage-scarred_berserker",
            name: "Rage-Scarred Berserker",
            mana: "{4}{B}",
            types: &["Creature", "Minotaur", "Berserker"],
            body: (5, 4),
            power: 1,
            toughness: 0,
            indestructible: true,
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, Some(case.body));
        let trigger = only_trigger(f);
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            case.id
        );
        let SpellEffectKind::PumpTarget {
            power,
            toughness,
            subject,
            ..
        } = &trigger.effect[0]
        else {
            panic!("{}: pump", case.id)
        };
        assert_eq!(
            (*power, *toughness),
            (case.power, case.toughness),
            "{}",
            case.id
        );
        assert_eq!(
            chosen_filter(subject).controller,
            TargetController::You,
            "{}",
            case.id
        );
        let grants_indestructible = trigger.effect.iter().any(|effect| {
            matches!(
                effect,
                SpellEffectKind::GrantKeywords { keywords, .. }
                    if keywords == &[Keyword::Indestructible]
            )
        });
        assert_eq!(grants_indestructible, case.indestructible, "{}", case.id);
    }
}

#[test]
fn q50_etb_counter_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        body: (u32, u32),
        subtype: &'static str,
    }
    for case in [
        Case {
            id: "stromkirk_mentor",
            name: "Stromkirk Mentor",
            mana: "{3}{B}",
            types: &["Creature", "Vampire", "Soldier"],
            body: (4, 2),
            subtype: "Vampire",
        },
        Case {
            id: "sanguine_glorifier",
            name: "Sanguine Glorifier",
            mana: "{3}{W}",
            types: &["Creature", "Vampire", "Cleric"],
            body: (3, 3),
            subtype: "Vampire",
        },
        Case {
            id: "jade_bearer",
            name: "Jade Bearer",
            mana: "{G}",
            types: &["Creature", "Merfolk", "Shaman"],
            body: (1, 1),
            subtype: "Merfolk",
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, Some(case.body));
        let trigger = only_trigger(f);
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            case.id
        );
        let SpellEffectKind::PutCounters {
            counter,
            count,
            subject,
        } = &trigger.effect[0]
        else {
            panic!("{}: counter", case.id)
        };
        assert_eq!(*counter, CounterKind::PlusOnePlusOne, "{}", case.id);
        assert!(
            matches!(count, tricerules_cards::primitives::Amount::Fixed(1)),
            "{}",
            case.id
        );
        let filter = chosen_filter(subject);
        assert_eq!(filter.required_subtypes, [case.subtype], "{}", case.id);
        assert_eq!(
            filter.excluded_objects,
            [tricerules_cards::primitives::TargetObjectExclusion::Source],
            "{}: another",
            case.id
        );
    }
}

// ── 07 optional basic-land or subtype search into hand ────────────────────

#[test]
fn q50_basic_land_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        body: (u32, u32),
        keywords: &'static [Keyword],
    }
    for case in [
        Case {
            id: "gatecreeper_vine",
            name: "Gatecreeper Vine",
            mana: "{1}{G}",
            types: &["Creature", "Plant"],
            body: (0, 2),
            keywords: &[Keyword::Defender],
        },
        Case {
            id: "district_guide",
            name: "District Guide",
            mana: "{2}{G}",
            types: &["Creature", "Elf", "Scout"],
            body: (2, 2),
            keywords: &[],
        },
        Case {
            id: "farfinder",
            name: "Farfinder",
            mana: "{3}",
            types: &["Creature", "Fox"],
            body: (1, 1),
            keywords: &[Keyword::Vigilance],
        },
        Case {
            id: "pilgrims_eye",
            name: "Pilgrim's Eye",
            mana: "{3}",
            types: &["Artifact", "Creature", "Thopter"],
            body: (1, 1),
            keywords: &[Keyword::Flying],
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, Some(case.body));
        assert_eq!(f.keywords, case.keywords, "{}", case.id);
        let trigger = only_trigger(f);
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            case.id
        );
        assert!(
            trigger.program.is_some(),
            "{}: optional search program",
            case.id
        );
    }
}

// ── 08 Treasure on death or entry ─────────────────────────────────────────

#[test]
fn q50_treasure_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        body: (u32, u32),
        keywords: &'static [Keyword],
        trigger: TriggerCondition,
    }
    for case in [
        Case {
            id: "dire_fleet_hoarder",
            name: "Dire Fleet Hoarder",
            mana: "{1}{B}",
            types: &["Creature", "Human", "Pirate"],
            body: (2, 1),
            keywords: &[],
            trigger: TriggerCondition::WhenSelfDies,
        },
        Case {
            id: "jewel-eyed_cobra",
            name: "Jewel-Eyed Cobra",
            mana: "{2}{G}",
            types: &["Creature", "Snake"],
            body: (3, 1),
            keywords: &[Keyword::Deathtouch],
            trigger: TriggerCondition::WhenSelfDies,
        },
        Case {
            id: "patron_of_the_arts",
            name: "Patron of the Arts",
            mana: "{2}{R}",
            types: &["Creature", "Dragon", "Noble"],
            body: (3, 1),
            keywords: &[],
            trigger: TriggerCondition::WhenSelfEntersOrIsPutIntoGraveyardFromBattlefield,
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, Some(case.body));
        assert_eq!(f.keywords, case.keywords, "{}", case.id);
        let trigger = only_trigger(f);
        assert_eq!(trigger.trigger, case.trigger, "{}", case.id);
        let SpellEffectKind::CreateTokens { token, count, .. } = &trigger.effect[0] else {
            panic!("{}: Treasure", case.id)
        };
        assert_eq!(token.as_str(), "treasure", "{}", case.id);
        assert_eq!(
            *count,
            tricerules_cards::primitives::Amount::Fixed(1),
            "{}",
            case.id
        );
    }
}

// ── 09 destroy artifact or enchantment ────────────────────────────────────

#[test]
fn q50_destroy_maps_definitions() {
    struct Case {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        branches: usize,
    }
    for case in [
        Case {
            id: "naturalize",
            name: "Naturalize",
            mana: "{1}{G}",
            types: &["Instant"],
            branches: 2,
        },
        Case {
            id: "natures_chant",
            name: "Nature's Chant",
            mana: "{1}{G/W}",
            types: &["Instant"],
            branches: 2,
        },
        Case {
            id: "return_to_the_earth",
            name: "Return to the Earth",
            mana: "{3}{G}",
            types: &["Instant"],
            branches: 3,
        },
    ] {
        let f = identity(case.id, case.name, case.mana, case.types, None);
        let [SpellEffectKind::Destroy { subject }] = f.spell_effect.as_slice() else {
            panic!("{}: destroy", case.id)
        };
        let filter = chosen_filter(subject);
        let branches = filter.any_of.as_ref().expect("printed type disjunction");
        assert_eq!(branches.len(), case.branches, "{}", case.id);
    }
}

// ── fingerprints ──────────────────────────────────────────────────────────

#[test]
fn q50_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "repel",
            "Repel",
            "31532dd2a4b05bf10a021a1244094c9aa3e0221d3976cd031d525026cebb136e",
        ),
        (
            "grasp_of_phantoms",
            "Grasp of Phantoms",
            "ebeafbbab807b60723095378dfba96a77611dc35bbe1a6bec496e81c07887bd2",
        ),
        (
            "run_aground",
            "Run Aground",
            "2cdb0ae2ea5a6f5bda9f2ace8b1018b7164f24c8be526389c6998ae72bf58d04",
        ),
        (
            "banishment_decree",
            "Banishment Decree",
            "28aac405629dd49841d0cc386b6667ba98e8ccaf513442c5a480f341f1cf9326",
        ),
        (
            "temporal_eddy",
            "Temporal Eddy",
            "42df473d2b1051af6fb0a6f1d378bcdebfb462d5365779a039942293a51b2753",
        ),
        (
            "fallow_earth",
            "Fallow Earth",
            "cfc2e057d6aebca96097be6ae7fcc4981aa898cd22ec7a840409ff7fc489a4aa",
        ),
        (
            "boggart_birth_rite",
            "Boggart Birth Rite",
            "a28da7e4183e32ea3d0395ae8bf1f85c7c9819ef18306bde6c8a602cd843ab0d",
        ),
        (
            "argivian_find",
            "Argivian Find",
            "10e4d15734b6afcc82e06fef1ac7f3484bd3f7157a6afeca2cec877d06ed6dd8",
        ),
        (
            "natures_spiral",
            "Nature's Spiral",
            "16b9fd9c84f6e2332d7f895ac2a0b92b7eb5a326a2c0c9526b619eea4a771a61",
        ),
        (
            "deja_vu",
            "Déjà Vu",
            "88f985fa2908cacfd4a140f396377f7b1c0e4be56681d3b5c2d71a7bd272e88a",
        ),
        (
            "cryogenic_stasis",
            "Cryogenic Stasis",
            "5fa62eec186f56a0efa0411ae82c972b21ca62fc3cba2d25987fbe50bfa6e6a6",
        ),
        (
            "impede_momentum",
            "Impede Momentum",
            "7830233526752deb764236a0175bf055a9f6fe2489774785f244965eecb2590f",
        ),
        (
            "tranquilize",
            "Tranquilize",
            "5b7cba9034ea07f86b2baf5a7338b98e9c6f5bb669d99f1e674a74b77ff9a9af",
        ),
        (
            "freeze_in_place",
            "Freeze in Place",
            "3c28ac60d5c75ecb77ba8a1f40e1501c836255a5ac4e324fd4b754f83652a623",
        ),
        (
            "eyeblight_assassin",
            "Eyeblight Assassin",
            "7d76e8b1ba8699d11c87b96b5475b259b3cfed0c5dec0a53c842269f999a6d50",
        ),
        (
            "black_dragon",
            "Black Dragon",
            "5f47b800a684f87acc8b5b301f61a3e38dca6bc0475470ed250afe58b4f4d01c",
        ),
        (
            "faerie_duelist",
            "Faerie Duelist",
            "fda001f9c0e4a7eee0fa5a9316407b79e03a3adf85b9e5f7e3e72faf4deaa594",
        ),
        (
            "brinebarrow_intruder",
            "Brinebarrow Intruder",
            "154b0e3d20c8eb57d1b7c5e02009ca6af1372f5d5b738ced6ed4c65429074c21",
        ),
        (
            "haazda_officer",
            "Haazda Officer",
            "643e41e44cd89cffab5d7b65460e8e2217e00aaeb77504cf07f3194d111225b8",
        ),
        (
            "herald_of_the_fair",
            "Herald of the Fair",
            "643e41e44cd89cffab5d7b65460e8e2217e00aaeb77504cf07f3194d111225b8",
        ),
        (
            "rage-scarred_berserker",
            "Rage-Scarred Berserker",
            "0d13c51d295960dd002349f8b7c425a0efd99c3bb2000caed38660c1b91b005f",
        ),
        (
            "stromkirk_mentor",
            "Stromkirk Mentor",
            "188f32d4be4ac044ddada9a40c27e6e6210aaeb6c4fad29652bdf63a7d51f898",
        ),
        (
            "sanguine_glorifier",
            "Sanguine Glorifier",
            "188f32d4be4ac044ddada9a40c27e6e6210aaeb6c4fad29652bdf63a7d51f898",
        ),
        (
            "jade_bearer",
            "Jade Bearer",
            "dbef90d18f1d5367a601d8f78c40b0ee00a725ffc0a9fe670eebd885dc5c9c19",
        ),
        (
            "gatecreeper_vine",
            "Gatecreeper Vine",
            "deb5a688c1e171e98614545ea170816095b87cc47a01a985bf90e97fcb44e491",
        ),
        (
            "district_guide",
            "District Guide",
            "57391b0d274a73fdccc36d1ffe68878f84fda56cb862999eb7492a5a3d7e8d6a",
        ),
        (
            "farfinder",
            "Farfinder",
            "18bd129cdefb893b2c7bd596d355112d9da24939b53166b1dd5dc48032b1e9c3",
        ),
        (
            "pilgrims_eye",
            "Pilgrim's Eye",
            "06075c8cee2b109ff43accc82bd4b6a559fb41483be799bcc0d8833818e103fb",
        ),
        (
            "dire_fleet_hoarder",
            "Dire Fleet Hoarder",
            "e5d54b972c569e5a975f10e673d90d472b6558d12cf7d98053c6af4f00a38318",
        ),
        (
            "jewel-eyed_cobra",
            "Jewel-Eyed Cobra",
            "9c71443f4ee6d1788c5aa11475d5ac373fc54690b8119d86389361cbfa6594eb",
        ),
        (
            "patron_of_the_arts",
            "Patron of the Arts",
            "eba6e5668ccc37ee396ca95b838cb7ace6a890c2fdc34413f7461ba8ac0c5d33",
        ),
        (
            "naturalize",
            "Naturalize",
            "1c50bfdd31a28155b4cab0e9b74980c1307c66df9ec65f7c88afa3f9f87a8317",
        ),
        (
            "natures_chant",
            "Nature's Chant",
            "1c50bfdd31a28155b4cab0e9b74980c1307c66df9ec65f7c88afa3f9f87a8317",
        ),
        (
            "return_to_the_earth",
            "Return to the Earth",
            "bdd75ebd1cab2fadcf69dfebe2203fb82e516f75437358259dac6e4de9569f62",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id(name), "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
        assert_eq!(definition(id).primary_face().name, name, "{id}");
    }
}
