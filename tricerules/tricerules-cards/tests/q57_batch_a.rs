//! Registry conformance for the queue-v57 batch A cohorts.
//!
//! Every included card is a normal-layout single-face card whose typed definition is the
//! reviewed flash Equipment, basic-land search or static anthem shape. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, CreatureScopeController, CreatureScopeFilter, StaticAbilityDef,
};
use tricerules_cards::{registry, AbilityPresentation, CardFace, Keyword, Layout};

fn identity(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    let face_id = if id == "quick-draw_dagger" {
        "quick_draw_dagger"
    } else {
        id
    };
    assert_eq!(f.face_id.as_str(), face_id, "{id} face id");
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

#[test]
fn q57_flash_equipment_map_definitions() {
    for (id, name, mana, equip, power, toughness, static_keywords, grant, untap) in [
        (
            "galadhrim_bow",
            "Galadhrim Bow",
            "{2}{G}",
            "{2}",
            1,
            2,
            &["Reach"][..],
            false,
            true,
        ),
        (
            "hidden_footblade",
            "Hidden Footblade",
            "{R}",
            "{2}",
            1,
            0,
            &["Haste"][..],
            true,
            false,
        ),
        (
            "javelin_of_lightning",
            "Javelin of Lightning",
            "{1}{R}",
            "{4}",
            2,
            0,
            &["FirstStrike"][..],
            false,
            false,
        ),
        (
            "quick-draw_dagger",
            "Quick-Draw Dagger",
            "{3}",
            "{1}",
            1,
            1,
            &[][..],
            true,
            false,
        ),
    ] {
        let f = identity(id, name, mana, &["Artifact", "Equipment"]);
        assert_eq!(f.keywords, [Keyword::Flash], "{id} flash");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.supertypes.is_empty(), "{id}");
        assert_eq!((f.power, f.toughness), (None, None), "{id} not a creature");

        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one entry trigger");
        };
        assert_eq!(triggered.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            triggered.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} trigger presentation"
        );
        assert_eq!(
            triggered.trigger,
            tricerules_cards::primitives::TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        let effects = format!("{:?}", triggered.effect);
        assert!(effects.contains("AttachSource"), "{id}: {effects}");
        assert_eq!(effects.contains("GrantKeywords"), grant, "{id}: {effects}");
        assert_eq!(effects.contains("Untap"), untap, "{id}: {effects}");
        let targeting = triggered.targeting.as_ref().expect("{id} target group");
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
        assert_eq!(
            targeting.groups[0].effect_indices,
            if grant || untap { vec![0, 1] } else { vec![0] },
            "{id}"
        );

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one attached modifier");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id} static presentation"
        );
        let definition = format!("{:?}", static_ability.definition);
        assert!(
            definition.contains("AttachedModifier"),
            "{id}: {definition}"
        );
        assert!(
            definition.contains(&format!("delta_power: {power}")),
            "{id}: {definition}"
        );
        assert!(
            definition.contains(&format!("delta_toughness: {toughness}")),
            "{id}: {definition}"
        );
        for keyword in static_keywords {
            assert!(definition.contains(keyword), "{id}: {definition}");
        }
        assert_eq!(
            definition.contains("ActivePlayer"),
            id == "javelin_of_lightning",
            "{id}: only the Javelin is turn-conditioned"
        );

        let [activated] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(
            activated.presentation,
            AbilityPresentation::OracleLines(vec![4]),
            "{id} equip presentation"
        );
        let [AbilityCost::Mana(cost)] = activated.costs.as_slice() else {
            panic!("{id}: one mana cost");
        };
        assert_eq!(cost.to_string(), equip, "{id} equip cost");
        let equip_effect = format!("{:?}", activated.effect);
        assert!(equip_effect.contains("Equip"), "{id}: {equip_effect}");
    }
}

#[test]
fn q57_basic_land_search_map_definitions() {
    for (id, name, mana, types, power, toughness, kind) in [
        (
            "giant_ladybug",
            "Giant Ladybug",
            "{2}{G}",
            &["Creature", "Insect"][..],
            Some(4),
            Some(1),
            "etb_top_reach",
        ),
        (
            "kodamas_reach",
            "Kodama's Reach",
            "{2}{G}",
            &["Sorcery", "Arcane"][..],
            None,
            None,
            "split",
        ),
        (
            "lay_of_the_land",
            "Lay of the Land",
            "{G}",
            &["Sorcery"][..],
            None,
            None,
            "hand",
        ),
        (
            "loam_larva",
            "Loam Larva",
            "{1}{G}",
            &["Creature", "Insect"][..],
            Some(1),
            Some(3),
            "etb_top",
        ),
        (
            "natural_connection",
            "Natural Connection",
            "{2}{G}",
            &["Instant"][..],
            None,
            None,
            "battlefield_tapped",
        ),
        (
            "quirion_trailblazer",
            "Quirion Trailblazer",
            "{3}{G}",
            &["Creature", "Elf", "Scout"][..],
            Some(1),
            Some(2),
            "etb_battlefield",
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!((f.power, f.toughness), (power, toughness), "{id} body");
        if id == "giant_ladybug" {
            assert_eq!(f.keywords, [Keyword::Reach], "{id}");
        } else {
            assert!(f.keywords.is_empty(), "{id}");
        }
        if matches!(kind, "hand" | "battlefield_tapped" | "split") {
            assert!(f.triggered_abilities.is_empty(), "{id}");
            let effects = format!("{:?}", f.spell_effect);
            assert!(effects.contains("SearchLibrary"), "{id}: {effects}");
            assert!(effects.contains("BasicLand"), "{id}: {effects}");
            match kind {
                "hand" => {
                    assert!(effects.contains("destination: Hand"), "{id}: {effects}");
                    assert!(effects.contains("reveal: true"), "{id}: {effects}");
                }
                "battlefield_tapped" => {
                    assert!(
                        effects.contains("Battlefield { tapped: true }"),
                        "{id}: {effects}"
                    );
                    assert!(effects.contains("reveal: false"), "{id}: {effects}");
                }
                "split" => {
                    assert!(
                        effects.contains("BattlefieldTappedThenHand"),
                        "{id}: {effects}"
                    );
                    assert!(effects.contains("count: 2"), "{id}: {effects}");
                    assert!(effects.contains("reveal: true"), "{id}: {effects}");
                }
                _ => unreachable!(),
            }
        } else {
            assert!(f.spell_effect.is_empty(), "{id}");
            let [triggered] = f.triggered_abilities.as_slice() else {
                panic!("{id}: exactly one entry trigger");
            };
            assert_eq!(
                triggered.presentation,
                AbilityPresentation::OracleLines(vec![if id == "giant_ladybug" { 2 } else { 1 }]),
                "{id} presentation"
            );
            let program = format!("{:?}", triggered.program.as_ref().expect("typed program"));
            assert!(program.contains("Choose"), "{id}: {program}");
            assert!(program.contains("optional: true"), "{id}: {program}");
            assert!(program.contains("SearchLibrary"), "{id}: {program}");
            assert!(program.contains("BasicLand"), "{id}: {program}");
            match kind {
                "etb_battlefield" => assert!(
                    program.contains("Battlefield { tapped: true }"),
                    "{id}: {program}"
                ),
                "etb_top" | "etb_top_reach" => {
                    assert!(program.contains("TopOfLibrary"), "{id}: {program}")
                }
                _ => unreachable!(),
            }
        }
    }
}

fn controlled_filter() -> CreatureScopeFilter {
    CreatureScopeFilter {
        controller: Some(CreatureScopeController::YouControl),
        ..Default::default()
    }
}

#[test]
fn q57_static_anthems_map_definitions() {
    for (id, name, mana, types, power, toughness, shape) in [
        (
            "blade_historian",
            "Blade Historian",
            "{R/W}{R/W}{R/W}{R/W}",
            &["Creature", "Human", "Cleric"][..],
            Some(2),
            Some(3),
            "double_strike_attacking",
        ),
        (
            "favorable_winds",
            "Favorable Winds",
            "{1}{U}",
            &["Enchantment"][..],
            None,
            None,
            "flying_pt",
        ),
        (
            "orcish_oriflamme",
            "Orcish Oriflamme",
            "{3}{R}",
            &["Enchantment"][..],
            None,
            None,
            "attacking_pt",
        ),
        (
            "primal_rage",
            "Primal Rage",
            "{1}{G}",
            &["Enchantment"][..],
            None,
            None,
            "trample",
        ),
        (
            "war_horn",
            "War Horn",
            "{3}",
            &["Artifact"][..],
            None,
            None,
            "attacking_pt",
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!((f.power, f.toughness), (power, toughness), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        match (shape, &static_ability.definition) {
            (
                "double_strike_attacking",
                StaticAbilityDef::AnthemKeyword {
                    filter,
                    condition,
                    keyword,
                },
            ) => {
                assert_eq!(
                    filter,
                    &CreatureScopeFilter {
                        matching: Box::new(tricerules_cards::primitives::ObjectFilter {
                            kind: tricerules_cards::primitives::TargetKind::Creature,
                            combat_role: Some(tricerules_cards::primitives::CombatRole::Attacking),
                            ..Default::default()
                        }),
                        ..controlled_filter()
                    },
                    "{id} filter"
                );
                assert_eq!(condition, &None, "{id} condition");
                assert_eq!(keyword, &Keyword::DoubleStrike, "{id} keyword");
            }
            (
                "flying_pt",
                StaticAbilityDef::AnthemPt {
                    filter,
                    condition,
                    delta_power,
                    delta_toughness,
                },
            ) => {
                assert_eq!(
                    filter,
                    &CreatureScopeFilter {
                        matching: Box::new(tricerules_cards::primitives::ObjectFilter {
                            kind: tricerules_cards::primitives::TargetKind::Creature,
                            required_keywords: vec![Keyword::Flying],
                            ..Default::default()
                        }),
                        ..controlled_filter()
                    },
                    "{id} filter"
                );
                assert_eq!(condition, &None, "{id} condition");
                assert_eq!((*delta_power, *delta_toughness), (1, 1), "{id}");
            }
            (
                "attacking_pt",
                StaticAbilityDef::AnthemPt {
                    filter,
                    condition,
                    delta_power,
                    delta_toughness,
                },
            ) => {
                assert_eq!(
                    filter,
                    &CreatureScopeFilter {
                        matching: Box::new(tricerules_cards::primitives::ObjectFilter {
                            kind: tricerules_cards::primitives::TargetKind::Creature,
                            combat_role: Some(tricerules_cards::primitives::CombatRole::Attacking),
                            ..Default::default()
                        }),
                        ..controlled_filter()
                    },
                    "{id} filter"
                );
                assert_eq!(condition, &None, "{id} condition");
                assert_eq!((*delta_power, *delta_toughness), (1, 0), "{id}");
            }
            (
                "trample",
                StaticAbilityDef::AnthemKeyword {
                    filter,
                    condition,
                    keyword,
                },
            ) => {
                assert_eq!(filter, &controlled_filter(), "{id} filter");
                assert_eq!(condition, &None, "{id} condition");
                assert_eq!(keyword, &Keyword::Trample, "{id} keyword");
            }
            _ => panic!("{id}: unexpected static anthem shape"),
        }
    }
}

#[test]
fn q57_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "galadhrim_bow",
            "Galadhrim Bow",
            "860cb09eec0b63476a698624336f6e857195f09078d2970fdf03c9f5b6a79a79",
        ),
        (
            "hidden_footblade",
            "Hidden Footblade",
            "8037b1211d37a6b9c1469fbe5f889ef4b1c5193df8fb6fa8600cc730ee0e306c",
        ),
        (
            "javelin_of_lightning",
            "Javelin of Lightning",
            "76a5cd6b3064cc4084d71e0a4496f77d07ce51e6c75c5c90a1ddeee66bbdfe4f",
        ),
        (
            "quick-draw_dagger",
            "Quick-Draw Dagger",
            "ef4ced53392509952631f0c0c22c8d80f91944024797d28c53341b10d6b7b173",
        ),
        (
            "giant_ladybug",
            "Giant Ladybug",
            "f82e59b17b68e675f21cc25304895eeac9b8bbd1776c9ffabbc94ce58a00a9be",
        ),
        (
            "kodamas_reach",
            "Kodama's Reach",
            "f8952bf1d76fc6e53e288385c7844b5bfbe2ee3919b661ecb284ec286849729c",
        ),
        (
            "lay_of_the_land",
            "Lay of the Land",
            "87ccc4dd397b1bb8f325a9c72c6113aea4f8cb8b8addf735dd7739244d634028",
        ),
        (
            "loam_larva",
            "Loam Larva",
            "8567e78bd1eed7e594f22544f7217deaa65310e980acc9fccfe43c4bbda1065b",
        ),
        (
            "natural_connection",
            "Natural Connection",
            "af7e6d11de38817e1097a328f0258bfe3a93869a40245fdce63c8a2f90a67566",
        ),
        (
            "quirion_trailblazer",
            "Quirion Trailblazer",
            "d52ffdc16b3f744490df9962ca675e962f6e955c87129fefe81c3f43c4a6343c",
        ),
        (
            "blade_historian",
            "Blade Historian",
            "78272f1a0878c0e1d5f45ba8b8c574a5e491eb99a9f6ccbf10bba1508e667742",
        ),
        (
            "favorable_winds",
            "Favorable Winds",
            "cab986a7ea897797cade518bb014e71331efd59e0c5fedf712d93d02ccd94281",
        ),
        (
            "orcish_oriflamme",
            "Orcish Oriflamme",
            "d7ee6171e14cb6e585d92941a46684b009521e07425a8ee3ff1f23d70bf635c7",
        ),
        (
            "primal_rage",
            "Primal Rage",
            "e69790f68d227a904f121fcfbce9968440dd3373aefe5615620d6c229fe59bc3",
        ),
        (
            "war_horn",
            "War Horn",
            "d7ee6171e14cb6e585d92941a46684b009521e07425a8ee3ff1f23d70bf635c7",
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
        let face_id = if id == "quick-draw_dagger" {
            "quick_draw_dagger"
        } else {
            id
        };
        assert_eq!(fields[2], face_id, "face id drift for {id}");
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
    }
}
