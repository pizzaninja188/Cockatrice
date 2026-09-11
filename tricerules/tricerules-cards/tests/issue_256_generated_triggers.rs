use tricerules_cards::primitives::{PlayerRecipient, SpellEffectKind};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, LibraryPartitionKind, TriggerCondition,
};

#[test]
fn issue_256_registers_all_eleven_generated_cards_with_typed_triggers() {
    let registry = CardRegistry::global();

    for (id, ability_index, oracle_line) in [
        ("meticulous_artisan", 1, 2),
        ("plundering_pirate", 0, 1),
        ("flamekin_gildweaver", 0, 2),
        ("redcap_thief", 0, 1),
    ] {
        let ability = &registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
            .triggered_abilities[ability_index];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::CreateTokens {
                token: "treasure".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }]
        );
    }

    for id in ["gleaming_barrier", "piggy_bank", "common_crook"] {
        let ability = &registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
            .triggered_abilities[0];
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfDies);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::CreateTokens {
                token: "treasure".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }]
        );
    }

    let canyon = registry
        .get("canyon_crawler")
        .expect("missing Canyon Crawler")
        .primary_face();
    assert_eq!(
        canyon.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        canyon.triggered_abilities[0].effect,
        [SpellEffectKind::CreateTokens {
            token: "food".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );

    let flock = registry
        .get("wakandan_drone_flock")
        .expect("missing Wakandan Drone Flock")
        .primary_face();
    assert_eq!(
        flock.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        flock.triggered_abilities[0].effect,
        [SpellEffectKind::Scry {
            count: Amount::Fixed(2),
        }]
    );

    for (id, oracle_line) in [("shore_lurker", 2), ("sanitation_automaton", 1)] {
        let ability = &registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
            .triggered_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
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
}
