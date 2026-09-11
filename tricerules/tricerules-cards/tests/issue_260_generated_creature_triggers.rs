use tricerules_cards::primitives::{
    EffectSubject, PlayerRecipient, SpellEffectKind, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, LibraryPartitionKind, TriggerCondition,
};

#[test]
fn issue_260_registers_all_ten_generated_cards_with_exact_typed_triggers() {
    let registry = CardRegistry::global();

    for (id, oracle_line) in [("boulderborn_dragon", 2), ("il_mheg_pixie", 2)] {
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
            ability.trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0,
            }
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

    for (id, oracle_line) in [("cartographers_companion", 1), ("waterwind_scout", 2)] {
        assert_token_trigger(
            registry,
            id,
            oracle_line,
            TriggerCondition::WhenSelfEntersBattlefield,
            "map",
        );
    }

    for (id, oracle_line) in [("venomized_cat", 2), ("scarblade_scout", 2)] {
        let ability = &registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
            .triggered_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Mill {
                count: Amount::Fixed(2),
                who: PlayerRecipient::Controller,
            }]
        );
    }

    for id in ["bigfin_bouncer", "exclusion_mage"] {
        let ability = &registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
            .triggered_abilities[0];
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::Opponent,
                    ..TargetFilter::default()
                })),
            }]
        );
        let group = &ability.targeting.as_ref().expect("targeting").groups[0];
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.effect_indices, [0]);
    }

    for (id, oracle_line) in [("eager_trufflesnout", 2), ("scream_puff", 2)] {
        assert_token_trigger(
            registry,
            id,
            oracle_line,
            TriggerCondition::WheneverSelfDealsCombatDamageToPlayer,
            "food",
        );
    }
}

fn assert_token_trigger(
    registry: &CardRegistry,
    id: &str,
    oracle_line: u16,
    trigger: TriggerCondition,
    token: &str,
) {
    let ability = &registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
        .triggered_abilities[0];
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![oracle_line])
    );
    assert_eq!(ability.trigger, trigger);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::CreateTokens {
            token: token.into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
}
