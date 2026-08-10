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
                filter: AnthemFilter {
                    controller: Some(AnthemController::YouControl),
                    ..AnthemFilter::default()
                },
            }),
            Amount::Count(CountExpression::CreatureDeathsThisTurn),
        ] {
            let encoded = ron::to_string(&amount).unwrap();
            assert_eq!(ron::from_str::<Amount>(&encoded).unwrap(), amount);
        }
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
            max_targets: None,
        };
        assert!(effect.validate(EffectContext::Spell).is_err());
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
            subject: EffectSubject::Source,
        };
        assert!(pump_self.validate(EffectContext::Spell).is_err());
        assert!(pump_self.validate(EffectContext::Ability).is_ok());
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
