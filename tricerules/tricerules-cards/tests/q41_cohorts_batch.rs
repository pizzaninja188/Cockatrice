//! Registry conformance for the queue-v41 reuse-only cohorts.
use tricerules_cards::primitives::{
    AbilityCost, Amount, CardTypeFilter, EffectSubject, GameCondition, GraveyardAggregate, Keyword,
    LifeAmount, PermanentTypeFilter, PlayerRecipient, PowerComparison, SearchDestination,
    SpellEffectKind, SpellKeyword, StaticAbilityDef, TargetKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardDefinition, Layout};

fn definition(id: &str) -> &'static CardDefinition {
    tricerules_cards::registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn mana_cost(cost: &AbilityCost) -> String {
    match cost {
        AbilityCost::Mana(cost) => cost.to_string(),
        other => panic!("expected a mana cost, got {other:?}"),
    }
}

#[test]
fn q41_subtype_tutor_maps_definitions() {
    struct Card {
        id: &'static str,
        name: &'static str,
        mana: &'static str,
        types: &'static [&'static str],
        power: u32,
        toughness: u32,
        ability_cost: &'static str,
        subtype: &'static str,
        max_mana_value: u32,
    }
    let cards = [
        Card {
            id: "ramosian_sergeant",
            name: "Ramosian Sergeant",
            mana: "{W}",
            types: &["Creature", "Human", "Rebel"],
            power: 1,
            toughness: 1,
            ability_cost: "{3}",
            subtype: "Rebel",
            max_mana_value: 2,
        },
        Card {
            id: "ramosian_lieutenant",
            name: "Ramosian Lieutenant",
            mana: "{1}{W}",
            types: &["Creature", "Human", "Rebel"],
            power: 1,
            toughness: 2,
            ability_cost: "{4}",
            subtype: "Rebel",
            max_mana_value: 3,
        },
        Card {
            id: "amrou_scout",
            name: "Amrou Scout",
            mana: "{1}{W}",
            types: &["Creature", "Kithkin", "Rebel", "Scout"],
            power: 2,
            toughness: 1,
            ability_cost: "{4}",
            subtype: "Rebel",
            max_mana_value: 3,
        },
        Card {
            id: "ramosian_commander",
            name: "Ramosian Commander",
            mana: "{2}{W}{W}",
            types: &["Creature", "Human", "Rebel"],
            power: 2,
            toughness: 4,
            ability_cost: "{6}",
            subtype: "Rebel",
            max_mana_value: 5,
        },
        Card {
            id: "cateran_persuader",
            name: "Cateran Persuader",
            mana: "{B}{B}",
            types: &["Creature", "Human", "Mercenary"],
            power: 2,
            toughness: 1,
            ability_cost: "{1}",
            subtype: "Mercenary",
            max_mana_value: 1,
        },
        Card {
            id: "cateran_brute",
            name: "Cateran Brute",
            mana: "{2}{B}",
            types: &["Creature", "Horror", "Mercenary"],
            power: 2,
            toughness: 2,
            ability_cost: "{2}",
            subtype: "Mercenary",
            max_mana_value: 2,
        },
        Card {
            id: "cateran_kidnappers",
            name: "Cateran Kidnappers",
            mana: "{2}{B}{B}",
            types: &["Creature", "Human", "Mercenary"],
            power: 4,
            toughness: 2,
            ability_cost: "{3}",
            subtype: "Mercenary",
            max_mana_value: 3,
        },
    ];
    let registry = tricerules_cards::registry::global();
    for card in cards {
        let definition = definition(card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness))
        );
        assert!(f.keywords.is_empty());
        assert_eq!(f.activated_abilities.len(), 1, "{}", card.id);
        let ability = &f.activated_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.costs.len(), 2, "{}", card.id);
        assert_eq!(
            mana_cost(&ability.costs[0]),
            card.ability_cost,
            "{}",
            card.id
        );
        assert_eq!(ability.costs[1], AbilityCost::Tap, "{}", card.id);
        match ability.effect.as_slice() {
            [SpellEffectKind::SearchLibrary {
                filter: Some(filter),
                destination: SearchDestination::Battlefield { tapped: false },
                ..
            }] => {
                assert_eq!(filter.card_type, Some(CardTypeFilter::NonlandPermanent));
                assert_eq!(
                    filter
                        .required_subtypes
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    vec![card.subtype],
                    "{}",
                    card.id
                );
                assert_eq!(
                    filter.max_mana_value,
                    Some(card.max_mana_value),
                    "{}",
                    card.id
                );
            }
            other => panic!("{}: expected a battlefield search, got {other:?}", card.id),
        }
    }
}

#[test]
fn q41_threshold_pump_maps_definitions() {
    let cases: [(&str, &str, &str, u32, u32, i32, i32); 4] = [
        ("krosan_beast", "Krosan Beast", "{3}{G}", 1, 1, 7, 7),
        (
            "anurid_barkripper",
            "Anurid Barkripper",
            "{1}{G}{G}",
            2,
            2,
            2,
            2,
        ),
        ("springing_tiger", "Springing Tiger", "{3}{G}", 3, 3, 2, 2),
        (
            "metamorphic_wurm",
            "Metamorphic Wurm",
            "{3}{G}{G}",
            3,
            3,
            4,
            4,
        ),
    ];
    for (id, name, mana, power, toughness, dp, dt) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert!(f.keywords.is_empty());
        assert_eq!(f.static_abilities.len(), 1, "{id}");
        let ability = &f.static_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        match &ability.definition {
            StaticAbilityDef::ConditionalSelfModifier {
                condition:
                    GameCondition::GraveyardAggregate {
                        aggregate: GraveyardAggregate::CardCount,
                        min: Some(7),
                        max: None,
                        ..
                    },
                delta_power,
                delta_toughness,
                base_power: None,
                base_toughness: None,
                ..
            } => {
                assert_eq!((*delta_power, *delta_toughness), (dp, dt), "{id}");
            }
            other => panic!("{id}: expected threshold self pump, got {other:?}"),
        }
    }
}

#[test]
fn q41_etb_each_loses_life_maps_definitions() {
    let cases: [(&str, &str, &str, u32, u32, u32); 3] = [
        ("howling_banshee", "Howling Banshee", "{2}{B}{B}", 3, 3, 3),
        ("blood-toll_harpy", "Blood-Toll Harpy", "{2}{B}", 2, 1, 1),
        ("rabid_bloodsucker", "Rabid Bloodsucker", "{4}{B}", 3, 2, 2),
    ];
    for (id, name, mana, power, toughness, amount) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.keywords, vec![Keyword::Flying]);
        assert_eq!(f.triggered_abilities.len(), 1, "{id}");
        let trigger = &f.triggered_abilities[0];
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(value),
                who: PlayerRecipient::EachPlayer,
            }] if *value == amount
        ));
    }
}

#[test]
fn q41_destroy_power_scry_maps_definitions() {
    let cases: [(&str, &str, &str, u32, u32); 3] = [
        ("vanquish_the_foul", "Vanquish the Foul", "{5}{W}", 4, 1),
        ("guiding_bolt", "Guiding Bolt", "{2}{W}", 4, 2),
        ("skywhalers_shot", "Skywhaler's Shot", "{2}{W}", 3, 1),
    ];
    for (id, name, mana, power, scry) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!((f.power, f.toughness), (None, None));
        match f.spell_effect.as_slice() {
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(filter),
            }, SpellEffectKind::Scry {
                count: Amount::Fixed(value),
            }] => {
                assert_eq!(filter.kind, TargetKind::Creature, "{id}");
                assert_eq!(filter.power, Some(PowerComparison::AtLeast(power)), "{id}");
                assert_eq!(*value, scry, "{id}");
            }
            other => panic!("{id}: expected destroy then scry, got {other:?}"),
        }
        assert_eq!(f.targeting.as_ref().map(|t| t.groups.len()), Some(1));
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn q41_activated_pump_maps_definitions() {
    let cases: [(&str, &str, &str, &[&str], i32, i32); 5] = [
        ("wyluli_wolf", "Wyluli Wolf", "{1}{G}", &[], 1, 1),
        ("ghost_warden", "Ghost Warden", "{1}{W}", &[], 1, 1),
        (
            "nantuko_disciple",
            "Nantuko Disciple",
            "{3}{G}",
            &["{G}"],
            2,
            2,
        ),
        (
            "thriss,_nantuko_primus",
            "Thriss, Nantuko Primus",
            "{5}{G}{G}",
            &["{G}"],
            5,
            5,
        ),
        (
            "tower_of_champions",
            "Tower of Champions",
            "{4}",
            &["{8}"],
            6,
            6,
        ),
    ];
    for (id, name, mana, ability_mana, dp, dt) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.activated_abilities.len(), 1, "{id}");
        let ability = &f.activated_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.costs.len(), ability_mana.len() + 1, "{id}");
        for (index, expected) in ability_mana.iter().enumerate() {
            assert_eq!(mana_cost(&ability.costs[index]), *expected, "{id}");
        }
        assert_eq!(*ability.costs.last().unwrap(), AbilityCost::Tap, "{id}");
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power,
                toughness,
                subject: EffectSubject::Chosen(filter),
                ..
            }] if *power == dp
                && *toughness == dt
                && filter.kind == TargetKind::Creature
        ));
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn q41_activated_shrink_maps_definitions() {
    let cases: [(&str, &str, &str, &[&str], bool, i32, i32); 6] = [
        (
            "ghosts_of_the_damned",
            "Ghosts of the Damned",
            "{1}{B}{B}",
            &[],
            false,
            -1,
            0,
        ),
        (
            "saltfield_recluse",
            "Saltfield Recluse",
            "{2}{W}",
            &[],
            false,
            -2,
            0,
        ),
        (
            "phyrexian_denouncer",
            "Phyrexian Denouncer",
            "{1}{B}",
            &[],
            true,
            -1,
            -1,
        ),
        (
            "phyrexian_defiler",
            "Phyrexian Defiler",
            "{2}{B}{B}",
            &[],
            true,
            -3,
            -3,
        ),
        (
            "staff_of_zegon",
            "Staff of Zegon",
            "{4}",
            &["{3}"],
            false,
            -2,
            0,
        ),
        (
            "bloodtallow_candle",
            "Bloodtallow Candle",
            "{1}",
            &["{6}"],
            true,
            -5,
            -5,
        ),
    ];
    for (id, name, mana, ability_mana, sacrifices, dp, dt) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.activated_abilities.len(), 1, "{id}");
        let ability = &f.activated_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let expected_len = ability_mana.len() + 1 + usize::from(sacrifices);
        assert_eq!(ability.costs.len(), expected_len, "{id}");
        for (index, expected) in ability_mana.iter().enumerate() {
            assert_eq!(mana_cost(&ability.costs[index]), *expected, "{id}");
        }
        assert_eq!(ability.costs[ability_mana.len()], AbilityCost::Tap, "{id}");
        if sacrifices {
            assert_eq!(
                ability.costs[ability_mana.len() + 1],
                AbilityCost::SacrificeSelf,
                "{id}"
            );
        }
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power,
                toughness,
                subject: EffectSubject::Chosen(filter),
                ..
            }] if *power == dp
                && *toughness == dt
                && filter.kind == TargetKind::Creature
        ));
    }
}

#[test]
fn q41_sacrifice_destroy_enchantment_maps_definitions() {
    let cases: [(&str, &str, &str, &str); 3] = [
        ("druid_lyrist", "Druid Lyrist", "{G}", "{G}"),
        ("elvish_lyrist", "Elvish Lyrist", "{G}", "{G}"),
        ("elvish_hexhunter", "Elvish Hexhunter", "{G/W}", "{G/W}"),
    ];
    for (id, name, mana, ability_mana) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.activated_abilities.len(), 1, "{id}");
        let ability = &f.activated_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.costs.len(), 3, "{id}");
        assert_eq!(mana_cost(&ability.costs[0]), ability_mana, "{id}");
        assert_eq!(ability.costs[1], AbilityCost::Tap, "{id}");
        assert_eq!(ability.costs[2], AbilityCost::SacrificeSelf, "{id}");
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(filter),
            }] if filter.kind == TargetKind::AnyPermanent
                && filter.permanent_types == [PermanentTypeFilter::Enchantment]
        ));
    }
}

#[test]
#[allow(clippy::type_complexity)]
fn q41_cascade_creatures_map_definitions() {
    let cases: [(&str, &str, &str, &[&str], u32, u32); 3] = [
        (
            "maelstrom_colossus",
            "Maelstrom Colossus",
            "{8}",
            &["Artifact", "Creature", "Golem"],
            7,
            7,
        ),
        (
            "shardless_agent",
            "Shardless Agent",
            "{1}{G}{U}",
            &["Artifact", "Creature", "Human", "Rogue"],
            2,
            2,
        ),
        (
            "enlisted_wurm",
            "Enlisted Wurm",
            "{4}{G}{W}",
            &["Creature", "Wurm"],
            5,
            5,
        ),
    ];
    for (id, name, mana, types, power, toughness) in cases {
        let f = definition(id).primary_face();
        assert_eq!(f.name, name);
        assert_eq!(f.mana_cost.to_string(), mana);
        assert_eq!(f.types, types);
        assert_eq!((f.power, f.toughness), (Some(power), Some(toughness)));
        assert_eq!(f.spell_keywords, vec![SpellKeyword::Cascade]);
        assert!(f.keywords.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.activated_abilities.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.static_abilities.is_empty());
    }
}
