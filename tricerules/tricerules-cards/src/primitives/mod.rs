//! Generic data-driven vocabulary used by card definitions.
//!
//! The submodules group effects, targeting, costs, abilities, and keywords while the
//! re-exports below preserve the established `primitives::X` API and RON serde shapes.

mod abilities;
mod costs;
mod effects;
mod keywords;
mod targeting;

pub use abilities::*;
pub use costs::*;
pub use effects::*;
pub use keywords::*;
pub use targeting::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_counter_payment_must_be_nonzero() {
        let invalid = SpellEffectKind::CounterTargetSpell {
            spell_filter: None,
            unless_controller_pays: Some(0),
        };
        assert!(invalid.validate(EffectContext::Spell).is_err());
        let mana_leak_shape = SpellEffectKind::CounterTargetSpell {
            spell_filter: None,
            unless_controller_pays: Some(3),
        };
        assert!(mana_leak_shape.validate(EffectContext::Spell).is_ok());
    }

    #[test]
    fn optional_draw_discard_is_only_single_card_discard_then_draw() {
        let effect = |who, discard_count, order| SpellEffectKind::DrawDiscard {
            who,
            draw_count: 1,
            discard_count,
            order,
            optional: true,
        };
        assert!(effect(
            PlayerRecipient::Controller,
            1,
            DrawDiscardOrder::DiscardThenDraw
        )
        .validate(EffectContext::Ability)
        .is_ok());

        for invalid in [
            effect(
                PlayerRecipient::Controller,
                1,
                DrawDiscardOrder::DrawThenDiscard,
            ),
            effect(
                PlayerRecipient::Controller,
                2,
                DrawDiscardOrder::DiscardThenDraw,
            ),
            effect(
                PlayerRecipient::EachPlayer,
                1,
                DrawDiscardOrder::DiscardThenDraw,
            ),
        ] {
            assert!(invalid.validate(EffectContext::Ability).is_err());
        }
    }

    #[test]
    fn amount_serde_preserves_literals_x_and_named_conditionals() {
        assert_eq!(ron::from_str::<Amount>("4").unwrap(), Amount::Fixed(4));
        assert_eq!(ron::from_str::<Amount>(r#""X""#).unwrap(), Amount::X);

        let amount = Amount::Conditional {
            condition: GameCondition::CreatureDeathsThisTurn {
                min: Some(1),
                max: None,
            },
            when_true: 8,
            otherwise: 4,
        };
        let encoded = ron::to_string(&amount).unwrap();
        assert_eq!(ron::from_str::<Amount>(&encoded).unwrap(), amount);

        for amount in [
            Amount::Count(CountExpression::BattlefieldCreatures {
                filter: BattlefieldCreatureCountFilter {
                    controllers: RelativePlayerSet::Controller,
                    subtype: None,
                    required_keywords: vec![],
                    exclude_source: false,
                },
            }),
            Amount::Count(CountExpression::GraveyardCardsNamed {
                owners: RelativePlayerSet::Controller,
                name: "Growth Cycle".into(),
            }),
            Amount::Count(CountExpression::CreatureDeathsThisTurn),
            Amount::Count(CountExpression::CardsMilledThisWay {
                filter: CardTypeFilter::Creature,
            }),
        ] {
            let encoded = ron::to_string(&amount).unwrap();
            assert_eq!(ron::from_str::<Amount>(&encoded).unwrap(), amount);
        }
    }

    #[test]
    fn counted_amounts_reject_empty_authored_filters() {
        assert!(Amount::Count(CountExpression::BattlefieldCreatures {
            filter: BattlefieldCreatureCountFilter {
                controllers: RelativePlayerSet::Controller,
                subtype: Some(" ".into()),
                required_keywords: vec![],
                exclude_source: false,
            },
        })
        .validate()
        .is_err());
        assert!(Amount::Count(CountExpression::GraveyardCardsNamed {
            owners: RelativePlayerSet::Controller,
            name: String::new(),
        })
        .validate()
        .is_err());
    }

    #[test]
    fn lose_life_serde_supports_explicit_and_default_player_recipients() {
        let explicit: SpellEffectKind =
            ron::from_str("LoseLife(amount: Fixed(2), who: EachOpponent)")
                .expect("each-opponent life loss should deserialize");
        assert!(matches!(
            explicit,
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                ..
            }
        ));
        assert!(ron::to_string(&explicit)
            .expect("each-opponent life loss should serialize")
            .contains("who:EachOpponent"));

        let legacy: SpellEffectKind = ron::from_str("LoseLife(amount: Fixed(1))")
            .expect("legacy controller life loss should deserialize");
        assert!(ron::to_string(&legacy)
            .expect("legacy controller life loss should serialize")
            .contains("who:Controller"));
    }

    #[test]
    fn turn_history_condition_requires_a_valid_bound() {
        assert!(GameCondition::CreatureDeathsThisTurn {
            min: None,
            max: None,
        }
        .validate()
        .is_err());
        assert!(GameCondition::CreatureDeathsThisTurn {
            min: Some(2),
            max: Some(1),
        }
        .validate()
        .is_err());
        assert!(GameCondition::CreatureDeathsThisTurn {
            min: Some(1),
            max: None,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn graveyard_aggregate_condition_requires_valid_bounds_and_round_trips() {
        let condition = |min, max| GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::DistinctCardTypes,
            min,
            max,
        };
        assert!(condition(None, None).validate().is_err());
        assert!(condition(Some(4), Some(3)).validate().is_err());

        let delirium = condition(Some(4), None);
        delirium.validate().expect("valid delirium condition");
        let serialized = ron::to_string(&delirium).expect("serialize delirium condition");
        let decoded: GameCondition =
            ron::from_str(&serialized).expect("deserialize delirium condition");
        assert_eq!(decoded, delirium);
    }

    #[test]
    fn activation_conditions_require_valid_count_bounds() {
        let condition = |min, max| ActivationCondition::BattlefieldCreatureCount {
            filter: BattlefieldCreatureCountFilter {
                controllers: RelativePlayerSet::Controller,
                subtype: None,
                required_keywords: vec![Keyword::Flying],
                exclude_source: false,
            },
            min,
            max,
        };
        assert!(condition(None, None).validate().is_err());
        assert!(condition(Some(2), Some(1)).validate().is_err());
        assert!(condition(Some(1), None).validate().is_ok());
    }

    #[test]
    fn spell_cost_conditions_require_valid_counts_and_nonzero_reductions() {
        let condition = |min, max| GameCondition::BattlefieldCreatureCount {
            filter: BattlefieldCreatureCountFilter {
                controllers: RelativePlayerSet::Controller,
                subtype: None,
                required_keywords: vec![Keyword::Flying],
                exclude_source: false,
            },
            min,
            max,
        };
        assert!(condition(None, None).validate().is_err());
        assert!(condition(Some(2), Some(1)).validate().is_err());

        let zero = SpellCostModifier::ConditionalGenericReduction {
            amount: 0,
            condition: condition(Some(1), None),
        };
        assert!(zero.validate().is_err());
        let winged_words = SpellCostModifier::ConditionalGenericReduction {
            amount: 1,
            condition: condition(Some(1), None),
        };
        assert!(winged_words.validate().is_ok());
    }

    #[test]
    fn explicit_and_intrinsic_sorcery_speed_share_one_query() {
        let explicit = ActivatedAbilityDef {
            costs: vec![],
            effect: vec![],
            targeting: None,
            timing: ActivationTiming::SorcerySpeed,
            conditions: vec![],
            activation_limit: None,
            text: String::new(),
        };
        assert!(explicit.requires_sorcery_speed());

        let equip = ActivatedAbilityDef {
            costs: vec![],
            effect: vec![SpellEffectKind::Equip {
                target: TargetFilter::default(),
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: vec![],
            activation_limit: None,
            text: String::new(),
        };
        assert!(equip.requires_sorcery_speed());
    }

    #[test]
    fn activation_limit_rejects_zero_maximum() {
        let ability = ActivatedAbilityDef {
            costs: vec![],
            effect: vec![SpellEffectKind::ProduceMana {
                options: vec![ManaAmount {
                    g: 1,
                    ..Default::default()
                }],
                restriction: None,
                conditional: None,
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: vec![],
            activation_limit: Some(ActivationLimit::PerTurn { max_activations: 0 }),
            text: "Add {G}.".into(),
        };
        assert!(ability.validate_shape().is_err());
    }

    #[test]
    fn damage_targets_rejects_conditional_amounts() {
        let effect = SpellEffectKind::DamageTargets {
            amount: Amount::Conditional {
                condition: GameCondition::CreatureDeathsThisTurn {
                    min: Some(1),
                    max: None,
                },
                when_true: 5,
                otherwise: 3,
            },
            target: TargetFilter::default(),
            division: DamageDivision::ChooseAtCast,
            extra_mana_per_target: 0,
        };
        assert!(effect.validate(EffectContext::Spell).is_err());
    }

    #[test]
    fn milled_result_amount_requires_an_immediately_preceding_mill() {
        let counted_gain = SpellEffectKind::GainLife {
            amount: Amount::Count(CountExpression::CardsMilledThisWay {
                filter: CardTypeFilter::Creature,
            }),
        };
        let mill = SpellEffectKind::Mill {
            count: Amount::Fixed(4),
            who: PlayerRecipient::Controller,
        };

        assert!(SpellEffectKind::validate_list(&[mill.clone(), counted_gain.clone()]).is_ok());
        assert!(SpellEffectKind::validate_list(std::slice::from_ref(&counted_gain)).is_err());
        assert!(SpellEffectKind::validate_list(&[
            mill,
            SpellEffectKind::Draw {
                count: Amount::Fixed(1),
            },
            counted_gain,
        ])
        .is_err());
    }

    #[test]
    fn player_effect_accepts_player_spec() {
        assert!(SpellEffectKind::TargetPlayerLosesLife {
            amount: 3,
            target: TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..Default::default()
            },
        }
        .validate(EffectContext::Spell)
        .is_ok());
    }

    #[test]
    fn player_effect_rejects_nonplayer_spec() {
        assert!(SpellEffectKind::TargetPlayerGainsLife {
            amount: 3,
            target: TargetFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
        }
        .validate(EffectContext::Spell)
        .is_err());
    }

    #[test]
    fn damage_accepts_any_kind() {
        for kind in [
            TargetKind::AnyTarget,
            TargetKind::Creature,
            TargetKind::AnyPlayer,
            TargetKind::OpponentPlayer,
        ] {
            assert!(SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target: TargetFilter {
                    kind,
                    ..Default::default()
                },
            }
            .validate(EffectContext::Spell)
            .is_ok());
        }
    }

    /// Issue #39: untargeted mass selection has no activating player, so a controller relation can
    /// never be honored there — the effect's own `players` scope is the correct knob. Reject the
    /// filter at load rather than silently ignoring the field.
    #[test]
    fn mass_effect_rejects_controller_relative_filter() {
        let scoped = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..Default::default()
        };
        assert!(SpellEffectKind::DestroyAll {
            kind: scoped.clone(),
            prevent_regeneration: false,
        }
        .validate(EffectContext::Spell)
        .is_err());
        assert!(SpellEffectKind::DamageAll {
            amount: 2,
            kind: scoped,
        }
        .validate(EffectContext::Spell)
        .is_err());

        // The same effect without the controller restriction still loads.
        assert!(SpellEffectKind::DestroyAll {
            kind: TargetFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
            prevent_regeneration: true,
        }
        .validate(EffectContext::Spell)
        .is_ok());
    }

    #[test]
    fn source_subject_rejected_in_spell_context_allowed_in_ability() {
        let pump_self = SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 1,
            scale: None,
            subject: EffectSubject::Source,
        };
        assert!(pump_self.validate(EffectContext::Spell).is_err());
        assert!(pump_self.validate(EffectContext::Ability).is_ok());

        let fight_from_source = SpellEffectKind::Fight {
            first: EffectSubject::Source,
            second: EffectSubject::Chosen(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::NotYou,
                ..Default::default()
            }),
        };
        assert_eq!(fight_from_source.target_filters().len(), 1);
        assert!(fight_from_source.validate(EffectContext::Spell).is_err());
        assert!(fight_from_source.validate(EffectContext::Ability).is_ok());

        let invalid_fight = SpellEffectKind::Fight {
            first: EffectSubject::Source,
            second: EffectSubject::Chosen(TargetFilter {
                kind: TargetKind::AnyPermanent,
                ..Default::default()
            }),
        };
        assert!(invalid_fight.validate(EffectContext::Ability).is_err());
    }

    #[test]
    fn combat_restriction_scopes_validate_by_context_and_subject_kind() {
        let cant_block = CombatRestriction {
            cant_block: true,
            ..Default::default()
        };
        let source = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Source,
            restriction: cant_block,
        };
        assert!(source.validate(EffectContext::Spell).is_err());
        assert!(source.validate(EffectContext::Ability).is_ok());

        let chosen_player = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..Default::default()
            }),
            restriction: cant_block,
        };
        assert!(chosen_player.validate(EffectContext::Spell).is_err());

        let matching_creatures = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Matching(TargetFilter {
                kind: TargetKind::Creature,
                excluded_keywords: vec![Keyword::Flying],
                ..Default::default()
            }),
            restriction: cant_block,
        };
        assert!(matching_creatures.validate(EffectContext::Spell).is_ok());

        let matching_permanents = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Matching(TargetFilter {
                kind: TargetKind::AnyPermanent,
                ..Default::default()
            }),
            restriction: cant_block,
        };
        assert!(matching_permanents.validate(EffectContext::Spell).is_err());

        let empty = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(TargetFilter::default_creature()),
            restriction: CombatRestriction::default(),
        };
        assert!(empty.validate(EffectContext::Spell).is_err());
    }

    #[test]
    fn chosen_subject_defaults_to_creature_target() {
        assert_eq!(
            EffectSubject::default(),
            EffectSubject::Chosen(TargetFilter::default_creature())
        );
    }

    #[test]
    fn target_controller_relation_is_a_composable_filter_dimension() {
        let filter = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::Opponent,
            tapped: Some(true),
            not_artifact: true,
            ..TargetFilter::default()
        };

        assert_eq!(filter.controller, TargetController::Opponent);
        assert_eq!(filter.tapped, Some(true));
        assert!(filter.not_artifact);
        assert_eq!(TargetFilter::default().controller, TargetController::Any);
    }

    #[test]
    fn controller_relative_target_filter_requires_a_permanent_kind() {
        for kind in [
            TargetKind::AnyTarget,
            TargetKind::AnyPlayer,
            TargetKind::OpponentPlayer,
        ] {
            let effect = SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(1),
                target: TargetFilter {
                    kind,
                    controller: TargetController::Opponent,
                    ..TargetFilter::default()
                },
            };
            assert!(effect.validate(EffectContext::Spell).is_err());
        }
    }

    #[test]
    fn source_exclusion_requires_an_object_capable_target_kind() {
        for kind in [TargetKind::AnyPlayer, TargetKind::OpponentPlayer] {
            let effect = SpellEffectKind::TargetPlayerGainsLife {
                amount: 1,
                target: TargetFilter {
                    kind,
                    exclude_source: true,
                    ..TargetFilter::default()
                },
            };
            assert!(effect.validate(EffectContext::Spell).is_err());
        }

        let object_capable = SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(1),
            target: TargetFilter {
                kind: TargetKind::AnyTarget,
                exclude_source: true,
                ..TargetFilter::default()
            },
        };
        assert!(object_capable.validate(EffectContext::Ability).is_ok());
    }

    #[test]
    fn untargeted_filters_reject_source_exclusion() {
        let filter = TargetFilter {
            kind: TargetKind::Creature,
            exclude_source: true,
            ..TargetFilter::default()
        };
        assert!(SpellEffectKind::DestroyAll {
            kind: filter.clone(),
            prevent_regeneration: false,
        }
        .validate(EffectContext::Spell)
        .is_err());
        assert!(SpellEffectKind::GrantKeywordsAllPermanents {
            filter,
            keywords: vec![Keyword::Flying],
        }
        .validate(EffectContext::Ability)
        .is_err());
    }

    #[test]
    fn untargeted_filter_preserves_you_scope_but_defers_opponent_scope() {
        let keywords = vec![Keyword::Indestructible];
        let filter = |controller| TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller,
            ..TargetFilter::default()
        };

        assert!(SpellEffectKind::GrantKeywordsAllPermanents {
            filter: filter(TargetController::You),
            keywords: keywords.clone(),
        }
        .validate(EffectContext::Spell)
        .is_ok());
        assert!(SpellEffectKind::GrantKeywordsAllPermanents {
            filter: filter(TargetController::Opponent),
            keywords,
        }
        .validate(EffectContext::Spell)
        .is_err());
        assert!(SpellEffectKind::TargetPlayerSacrifices {
            target: TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..TargetFilter::default()
            },
            filter: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }
        .validate(EffectContext::Spell)
        .is_err());
    }
}
