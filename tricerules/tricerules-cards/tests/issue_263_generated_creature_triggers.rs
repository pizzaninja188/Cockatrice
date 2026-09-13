use tricerules_cards::primitives::{
    CardTypeFilter, EffectSubject, PlayerRecipient, SpellCastFilter, SpellEffectKind,
    TargetController, TargetFilter, TargetKind, TargetObjectExclusion,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, CastTriggerPlayer, CounterKind, TriggerCondition,
};

#[test]
fn issue_263_registers_all_fourteen_generated_cards_with_exact_typed_triggers() {
    let registry = CardRegistry::global();

    for id in ["exosuit_savior", "mischievous_pup", "stickytongue_sentinel"] {
        let ability = ability(registry, id);
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(!ability.may);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    controller: TargetController::You,
                    excluded_objects: vec![TargetObjectExclusion::Source],
                    ..TargetFilter::default()
                })),
            }]
        );
        let group = &ability.targeting.as_ref().expect("targeting").groups[0];
        assert_eq!((group.min, group.max), (0, 1));
        assert_eq!(group.effect_indices, [0]);
    }

    for id in ["daggerfang_duo", "deathcap_marionette", "mineshaft_spider"] {
        let ability = ability(registry, id);
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(ability.may);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Mill {
                count: Amount::Fixed(2),
                who: PlayerRecipient::Controller,
            }]
        );
    }

    for id in ["boar-q-pine", "tempest_angler"] {
        let ability = ability(registry, id);
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter: SpellCastFilter {
                    card_type: Some(CardTypeFilter::Noncreature),
                    ..SpellCastFilter::default()
                },
                ordinal: None,
                ordinal_scope: Default::default(),
            }
        );
        assert_source_counter(ability);
    }

    for id in ["atlantean_cavalry", "lakeshore_apothecary"] {
        let ability = ability(registry, id);
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPlayerDrawsNthCard {
                drawer: CastTriggerPlayer::Controller,
                ordinal: 2,
            }
        );
        assert_source_counter(ability);
    }

    for id in ["herald_of_faith", "shopkeepers_bane"] {
        let ability = ability(registry, id);
        assert_self_attacks(ability);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),
            }]
        );
    }

    for id in ["mysterios_phantasm", "screaming_phantom"] {
        let ability = ability(registry, id);
        assert_self_attacks(ability);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Mill {
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
            }]
        );
    }
}

fn ability(
    registry: &'static CardRegistry,
    id: &str,
) -> &'static tricerules_cards::TriggeredAbilityDef {
    let ability = &registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
        .triggered_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    let expected_line = if matches!(id, "boar-q-pine" | "tempest_angler") {
        1
    } else {
        2
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![expected_line])
    );
    ability
}

fn assert_source_counter(ability: &tricerules_cards::TriggeredAbilityDef) {
    assert!(!ability.may);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source,
        }]
    );
}

fn assert_self_attacks(ability: &tricerules_cards::TriggeredAbilityDef) {
    assert!(!ability.may);
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
}
