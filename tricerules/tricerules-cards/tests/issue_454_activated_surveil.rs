//! Issue #454 registry and presentation conformance for the activated mana-and-tap Surveil 1
//! family cohort.
//!
//! All seven identities print exactly `<mana cost>, {T}: Surveil 1.` in the pinned Scryfall
//! snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`; exact-name records and `rulings_uri`
//! responses were fetched 2026-09-19 and none returned rulings. The expectations below are the
//! reviewed printed Oracle behavior and the shipped typed vocabulary, not a copy of generator
//! output. CR 701.25 (surveil), CR 602.2b (activated-ability costs), CR 614.1c (enters tapped),
//! and CR 605 (mana abilities) govern the asserted shapes.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    EntersTappedAffected, LibraryPartitionKind, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, ActivatedAbilityDef, ActivationTiming,
    CardFace, CardRegistry, ManaAmount, ManaCost,
};

type ManaOption = (u32, u32, u32, u32, u32, u32);

struct GuildLand {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    surveil_cost: &'static str,
    mana_options: [ManaOption; 2],
}

const GUILD_LANDS: [GuildLand; 5] = [
    GuildLand {
        id: "titans_grave",
        name: "Titan's Grave",
        face_id: "titan_s_grave",
        surveil_cost: "{2}{B}{G}",
        mana_options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0)],
    },
    GuildLand {
        id: "paradox_gardens",
        name: "Paradox Gardens",
        face_id: "paradox_gardens",
        surveil_cost: "{2}{G}{U}",
        mana_options: [(0, 0, 0, 0, 1, 0), (0, 1, 0, 0, 0, 0)],
    },
    GuildLand {
        id: "fields_of_strife",
        name: "Fields of Strife",
        face_id: "fields_of_strife",
        surveil_cost: "{2}{R}{W}",
        mana_options: [(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)],
    },
    GuildLand {
        id: "spectacle_summit",
        name: "Spectacle Summit",
        face_id: "spectacle_summit",
        surveil_cost: "{2}{U}{R}",
        mana_options: [(0, 1, 0, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
    },
    GuildLand {
        id: "forum_of_amity",
        name: "Forum of Amity",
        face_id: "forum_of_amity",
        surveil_cost: "{2}{W}{B}",
        mana_options: [(1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
    },
];

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing reviewed card {id}"))
        .primary_face()
}

fn mana_options(ability: &ActivatedAbilityDef) -> Vec<ManaOption> {
    ability
        .mana_options()
        .expect("generated mana ability publishes typed options")
        .iter()
        .map(|option: &ManaAmount| (option.w, option.u, option.b, option.r, option.g, option.c))
        .collect()
}

fn surveil_activated(face: &CardFace) -> &ActivatedAbilityDef {
    face.activated_abilities
        .iter()
        .find(|ability| {
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::LibraryPartition {
                    kind: LibraryPartitionKind::Surveil,
                    ..
                }]
            )
        })
        .expect("face prints the reviewed activated surveil ability")
}

fn assert_surveil_ability(ability: &ActivatedAbilityDef, ability_id: &str, line: u16, cost: &str) {
    assert_eq!(ability.ability_id.as_str(), ability_id);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![line])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.costs,
        [
            AbilityCost::Mana(ManaCost::parse(cost).expect("reviewed mana cost")),
            AbilityCost::Tap,
        ]
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::LibraryPartition {
            count: 1,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );
    assert!(ability.cost_modifiers.is_empty());
    assert!(ability.targeting.is_none());
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert!(ability.conditions.is_empty());
    assert!(ability.activation_limit.is_none());
}

#[test]
fn issue_454_registers_exactly_the_reviewed_seven() {
    let registry = CardRegistry::global();
    for land in &GUILD_LANDS {
        assert_eq!(
            registry.id_for_name(land.name),
            Some(land.id),
            "{}",
            land.id
        );
        FaceExpectation {
            id: land.id,
            name: land.name,
            face_id: land.face_id,
            mana_cost: "",
            types: &["Land"],
            keywords: &[],
            power_toughness: None,
        }
        .check();
    }

    assert_eq!(
        registry.id_for_name("Tocasia's Dig Site"),
        Some("tocasias_dig_site")
    );
    FaceExpectation {
        id: "tocasias_dig_site",
        name: "Tocasia's Dig Site",
        face_id: "tocasia_s_dig_site",
        mana_cost: "",
        types: &["Land"],
        keywords: &[],
        power_toughness: None,
    }
    .check();

    assert_eq!(registry.id_for_name("Wretched Doll"), Some("wretched_doll"));
    FaceExpectation {
        id: "wretched_doll",
        name: "Wretched Doll",
        face_id: "wretched_doll",
        mana_cost: "{1}{B}",
        types: &["Artifact", "Creature", "Toy"],
        keywords: &[],
        power_toughness: Some((3, 1)),
    }
    .check();
}

#[test]
fn issue_454_guild_lands_enter_tapped_and_produce_both_colors() {
    for land in &GUILD_LANDS {
        let face = face(land.id);
        let [entry] = face.static_abilities.as_slice() else {
            panic!("{} must have one entry replacement", land.id);
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{}", land.id);
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            land.id
        );
        assert_eq!(
            entry.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{}",
            land.id
        );

        let [mana, surveil] = face.activated_abilities.as_slice() else {
            panic!("{} must have a mana ability and a surveil ability", land.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{}", land.id);
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            land.id
        );
        assert_eq!(mana.costs, [AbilityCost::Tap], "{}", land.id);
        assert_eq!(
            mana_options(mana),
            land.mana_options.to_vec(),
            "{} printed option order",
            land.id
        );

        assert_surveil_ability(surveil, "activated_02", 3, land.surveil_cost);
        assert!(
            face.spell_effect.is_empty(),
            "{} is a land with no spell effect",
            land.id
        );
    }
}

#[test]
fn issue_454_tocasia_dig_site_produces_colorless_and_surveils_for_three() {
    let face = face("tocasias_dig_site");
    assert!(
        face.static_abilities.is_empty(),
        "Tocasia's Dig Site does not enter tapped"
    );
    let [mana, surveil] = face.activated_abilities.as_slice() else {
        panic!("Tocasia's Dig Site must have a mana ability and a surveil ability");
    };
    assert_eq!(mana.ability_id.as_str(), "activated_01");
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(mana.costs, [AbilityCost::Tap]);
    assert_eq!(mana_options(mana), [(0, 0, 0, 0, 0, 1)]);

    assert_surveil_ability(surveil, "activated_02", 2, "{3}");
}

#[test]
fn issue_454_wretched_doll_is_a_black_toy_with_one_surveil_activation() {
    let face = face("wretched_doll");
    assert_eq!(face.power, Some(3));
    assert_eq!(face.toughness, Some(1));
    assert!(face.static_abilities.is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.triggered_abilities.is_empty());
    let [surveil] = face.activated_abilities.as_slice() else {
        panic!("Wretched Doll must have exactly one activated ability");
    };
    assert_surveil_ability(surveil, "activated_01", 1, "{B}");
}

#[test]
fn issue_454_excluded_two_mana_identities_stay_unregistered() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("lunatic_pandora", "Lunatic Pandora"),
        ("coastal_bulwark", "Coastal Bulwark"),
        ("laser_screwdriver", "Laser Screwdriver"),
    ] {
        assert!(
            registry.get(id).is_none(),
            "{id} prints other unsupported clauses and must stay unregistered"
        );
        assert!(
            registry.id_for_name(name).is_none(),
            "{id} must not resolve by name"
        );
    }
}

#[test]
fn issue_454_shipped_surveil_identities_stay_unchanged() {
    let registry = CardRegistry::global();

    for id in [
        "ominous_asylum",
        "savage_mansion",
        "university_campus",
        "sinister_hideout",
        "suburban_sanctuary",
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing shipped four-mana surveil land {id}"));
        let ability = surveil_activated(definition.primary_face());
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{4}").expect("reviewed cost")),
                AbilityCost::Tap,
            ],
            "{id} must keep its four-mana surveil cost"
        );
    }

    let wall = registry
        .get("rune-sealed_wall")
        .expect("Rune-Sealed Wall must stay registered");
    let ability = surveil_activated(wall.primary_face());
    assert_eq!(
        ability.costs,
        [AbilityCost::Tap],
        "Rune-Sealed Wall must keep its no-mana surveil cost"
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::LibraryPartition {
            count: 1,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }]
    );
}
