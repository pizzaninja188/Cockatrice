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
