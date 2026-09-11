use tricerules_cards::primitives::{
    BattlefieldAggregate, CardTypeFilter, GameCondition, PlayerLifeAggregate, StaticAbilityDef,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, CardRegistry, LibraryPartitionKind, ManaCost,
    RelativePlayerSet, SpellEffectKind,
};

fn generated_face(id: &str) -> &'static tricerules_cards::CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing generated card {id}"))
        .primary_face()
}

#[test]
fn issue_258_registers_the_exact_sixteen_land_cohort() {
    let surveil = [
        "savage_mansion",
        "ominous_asylum",
        "suburban_sanctuary",
        "sinister_hideout",
        "university_campus",
    ];
    let fast = [
        "concealed_courtyard",
        "inspiring_vantage",
        "blooming_marsh",
        "botanical_sanctum",
    ];
    let life = [
        "raucous_carnival",
        "etched_cornfield",
        "peculiar_lighthouse",
    ];
    let slow = [
        "sundown_pass",
        "shattered_sanctum",
        "dreamroot_cascade",
        "deathcap_glade",
    ];

    for id in surveil.into_iter().chain(fast).chain(life).chain(slow) {
        let face = generated_face(id);
        assert_eq!(face.types, ["Land"], "{id}");
        assert_eq!(face.mana_cost, ManaCost::default(), "{id}");
        assert_eq!(face.face_id.as_str(), id, "{id}");
    }
}

#[test]
fn issue_258_surveil_lands_preserve_cost_effect_and_presentation_identity() {
    for id in [
        "savage_mansion",
        "ominous_asylum",
        "suburban_sanctuary",
        "sinister_hideout",
        "university_campus",
    ] {
        let face = generated_face(id);
        let ability = face
            .activated_abilities
            .iter()
            .find(|ability| ability.ability_id.as_str() == "activated_02")
            .unwrap_or_else(|| panic!("{id} missing activated_02"));
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{4}").unwrap()),
                AbilityCost::Tap,
            ],
            "{id}"
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            }],
            "{id}"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn issue_258_conditional_lands_preserve_exact_public_predicates() {
    for (ids, min, max) in [
        (
            &[
                "concealed_courtyard",
                "inspiring_vantage",
                "blooming_marsh",
                "botanical_sanctum",
            ][..],
            Some(3),
            None,
        ),
        (
            &[
                "sundown_pass",
                "shattered_sanctum",
                "dreamroot_cascade",
                "deathcap_glade",
            ][..],
            None,
            Some(1),
        ),
    ] {
        for id in ids {
            let face = generated_face(id);
            let [ability] = face.static_abilities.as_slice() else {
                panic!("{id} must have exactly one static ability");
            };
            assert_eq!(ability.ability_id.as_str(), "static_01", "{id}");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1]),
                "{id}"
            );
            let StaticAbilityDef::EntersTapped {
                condition:
                    Some(GameCondition::BattlefieldAggregate {
                        filter,
                        aggregate,
                        min: actual_min,
                        max: actual_max,
                    }),
                unless_cost: None,
                ..
            } = &ability.definition
            else {
                panic!("{id} must use a conditional intrinsic tapped-entry ability");
            };
            assert_eq!(*aggregate, BattlefieldAggregate::Count, "{id}");
            assert_eq!(filter.controllers, RelativePlayerSet::Controller, "{id}");
            assert_eq!(filter.card_type, Some(CardTypeFilter::Land), "{id}");
            assert!(filter.exclude_source, "{id}");
            assert_eq!((*actual_min, *actual_max), (min, max), "{id}");
        }
    }

    for id in [
        "raucous_carnival",
        "etched_cornfield",
        "peculiar_lighthouse",
    ] {
        let face = generated_face(id);
        let [ability] = face.static_abilities.as_slice() else {
            panic!("{id} must have exactly one static ability");
        };
        assert_eq!(ability.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(matches!(
            ability.definition,
            StaticAbilityDef::EntersTapped {
                condition: Some(GameCondition::PlayerLifeAggregate {
                    players: RelativePlayerSet::All,
                    aggregate: PlayerLifeAggregate::Minimum,
                    min: Some(14),
                    max: None,
                }),
                unless_cost: None,
                ..
            }
        ));
    }
}
