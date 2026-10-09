//! Complete-command rollback for typed life and power/toughness numeric failures.
use super::*;

pub(super) struct NumericCheckpoint {
    state: GameState,
    pending_spell: Option<casting::PendingSpellCastInternal>,
    pending_ability: Option<activation::PendingAbilityActivationInternal>,
    private_zones: HashMap<PlayerId, PrivateZoneSnapshot>,
    battlefield: Option<BattlefieldViewSnapshot>,
    first_strike: bool,
}

impl GameEngine {
    pub(super) fn numeric_checkpoint(&self) -> NumericCheckpoint {
        NumericCheckpoint {
            state: self.state.clone(),
            pending_spell: self.pending_spell_cast_internal.clone(),
            pending_ability: self.pending_ability_activation_internal.clone(),
            private_zones: self.private_zone_cache.clone(),
            battlefield: self.battlefield_view_cache.clone(),
            first_strike: self.first_strike_step_pending_cache,
        }
    }

    pub(super) fn restore_numeric_checkpoint(&mut self, checkpoint: NumericCheckpoint) {
        self.state = checkpoint.state;
        self.pending_spell_cast_internal = checkpoint.pending_spell;
        self.pending_ability_activation_internal = checkpoint.pending_ability;
        self.private_zone_cache = checkpoint.private_zones;
        self.battlefield_view_cache = checkpoint.battlefield;
        self.first_strike_step_pending_cache = checkpoint.first_strike;
    }
}

/// Damage/drain producers must validate their signed loss before committing any gain.
pub(super) fn loss_delta(amount: u32) -> Result<i32, EngineError> {
    i32::try_from(-i64::from(amount)).map_err(|_| EngineError::LifeNumericRange("life loss delta"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn archive_signed_loss_domain_includes_the_minimum_public_delta() {
        assert_eq!(loss_delta(i32::MAX as u32 + 1).unwrap(), i32::MIN);
        assert!(matches!(
            loss_delta(i32::MAX as u32 + 2),
            Err(EngineError::LifeNumericRange(_))
        ));
        let mut engine = GameEngine::new(104_821, &[0, 1], 20, None, true).unwrap();
        history::commit_life_change_checked(&mut engine.state, 0, i32::MIN).unwrap();
        assert_eq!(engine.state.players[0].life, i32::MIN + 20);
        assert_eq!(
            engine.state.turn_history.current.player(0).life_lost,
            1_u64 << 31
        );
    }

    #[test]
    fn archive_lifelink_source_sum_overflow_is_typed_before_any_gain() {
        let mut engine = GameEngine::new(104_822, &[0, 1], 20, None, true).unwrap();
        let source = engine.state.players[0].hand[0];
        engine.state.objects.get_mut(&source).unwrap().card_id = "vampire_nighthawk".into();
        resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            None,
        )
        .unwrap();
        let mut completed = Vec::new();
        for _ in 0..2 {
            let target = engine.state.players[1].hand[0];
            engine.state.objects.get_mut(&target).unwrap().card_id = "grizzly_bears".into();
            resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                target,
                Zone::Battlefield,
                None,
            )
            .unwrap();
            completed.push(damage::CompletedDamage {
                spec: damage::DamageSpec {
                    event: damage::DamageEvent::noncombat(
                        source,
                        0,
                        "numeric witness",
                        damage::DamageRecipient::Permanent(target),
                        u32::MAX,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: true,
                },
                result: damage::DamageResult {
                    attempted: u32::MAX,
                    dealt: u32::MAX,
                    prevented: 0,
                },
                prevention_debits: Vec::new(),
            });
        }
        let mut events = Vec::new();
        assert!(matches!(
            engine.commit_completed_damage_batch(&completed, &mut events),
            Err(EngineError::LifeNumericRange("lifelink source sum"))
        ));
        assert_eq!(engine.state.players[0].life, 20);
        assert_eq!(engine.state.turn_history.current.player(0).life_gained, 0);
        assert!(!events
            .iter()
            .any(|event| matches!(event.ev, Some(rv1::ruled_event::Ev::LifeChanged(_)))));
    }

    #[test]
    fn archive_mana_damage_failure_restores_private_pending_payment_transactions() {
        for ability_payment in [false, true] {
            let mut engine = GameEngine::new(104_819, &[0, 1], 20, None, true).unwrap();
            engine.state.opening = None;
            engine.state.turn_step = TurnStep::Main1;
            engine.state.active_player_idx = 0;
            engine.state.priority_idx = 0;
            let mut permanents = Vec::new();
            for card in [
                "alhammarrets_archive",
                "murmuring_bosk",
                "arena",
                "grizzly_bears",
            ] {
                let oid = engine.state.players[0].hand[0];
                engine.state.objects.get_mut(&oid).unwrap().card_id = card.into();
                resolution::move_object_to_zone(
                    &mut engine.state,
                    engine.registry,
                    oid,
                    Zone::Battlefield,
                    None,
                )
                .unwrap();
                permanents.push(oid);
            }
            let bosk = permanents[1];
            // A legal source-quality fixture: lifelink can apply to noncreature damage sources.
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(bosk),
                kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 0,
            });
            let spell = engine.state.players[0].hand[0];
            engine.state.objects.get_mut(&spell).unwrap().card_id = "grizzly_bears".into();
            let opponent = engine.state.players[1].hand[0];
            engine.state.objects.get_mut(&opponent).unwrap().card_id = "grizzly_bears".into();
            resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                opponent,
                Zone::Battlefield,
                None,
            )
            .unwrap();
            engine.state.players[0].mana_pool.colorless = 3;
            engine.initial_response_batch();
            let begin = if ability_payment {
                RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
                        rv1::BeginAbilityActivation {
                            source_object_id: permanents[2],
                            expected_zone_change_generation: engine.state.zone_change_generation
                                [&permanents[2]],
                            ability_index: 0,
                            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                            own_target: Some(rv1::AbilityActivationTarget {
                                object_id: permanents[3],
                                zone_change_generation: engine.state.zone_change_generation
                                    [&permanents[3]],
                                group_index: 0,
                            }),
                            ..Default::default()
                        },
                    )),
                }
            } else {
                RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
                        rv1::BeginSpellCast {
                            announcement: Some(rv1::SpellCastAnnouncement {
                                cast_method: rv1::CastMethod::Normal as i32,
                                source: Some(rv1::CastSource {
                                    expected_zone_change_generation: None,
                                    location: Some(rv1::cast_source::Location::HandIndex(0)),
                                }),
                                ..Default::default()
                            }),
                        },
                    )),
                }
            };
            engine.apply_command(0, &begin).unwrap();
            if ability_payment {
                let pending = engine.state.pending_ability_activation.as_ref().unwrap();
                let choice = RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                        rv1::SubmitAbilityActivationChoice {
                            transaction_id: pending.transaction_id,
                            expected_revision: pending.revision,
                            target: Some(rv1::AbilityActivationTarget {
                                object_id: opponent,
                                zone_change_generation: engine.state.zone_change_generation
                                    [&opponent],
                                group_index: 1,
                            }),
                            ..Default::default()
                        },
                    )),
                };
                engine.apply_command(1, &choice).unwrap();
            }
            assert_eq!(
                engine.pending_ability_activation_internal.is_some(),
                ability_payment
            );
            assert_eq!(
                engine.pending_spell_cast_internal.is_some(),
                !ability_payment
            );
            engine.state.players[0].life = i32::MAX;
            let before = engine.diagnostic_snapshot().unwrap();
            let zones = engine.private_zone_cache.clone();
            let battlefield = engine.battlefield_view_cache.clone();
            let command = RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: bosk,
                        ability_index: 1,
                        expected_zone_change_generation: engine.state.zone_change_generation[&bosk],
                        ..Default::default()
                    },
                )),
            };
            assert!(matches!(
                engine.apply_command(0, &command),
                Err(EngineError::LifeNumericRange(_))
            ));
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
            assert!(engine.private_zone_cache == zones);
            assert!(engine.battlefield_view_cache == battlefield);
            assert_eq!(
                engine.pending_ability_activation_internal.is_some(),
                ability_payment
            );
            assert_eq!(
                engine.pending_spell_cast_internal.is_some(),
                !ability_payment
            );
            assert!(!engine.state.objects[&bosk].tapped);
            // The retained transaction must still function after the rejected mana activation.
            engine.state.players[0].life = 20;
            engine.apply_command(0, &command).unwrap();
            assert_eq!(engine.state.players[0].life, 21);
            let cancel = if ability_payment {
                RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CancelAbilityActivation(
                        rv1::CancelAbilityActivation {
                            transaction_id: engine
                                .state
                                .pending_ability_activation
                                .as_ref()
                                .unwrap()
                                .transaction_id,
                            expected_revision: engine
                                .state
                                .pending_ability_activation
                                .as_ref()
                                .unwrap()
                                .revision,
                        },
                    )),
                }
            } else {
                RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                        rv1::CancelSpellCast {
                            transaction_id: engine
                                .state
                                .pending_spell_cast
                                .as_ref()
                                .unwrap()
                                .transaction_id,
                        },
                    )),
                }
            };
            engine.apply_command(0, &cancel).unwrap();
            assert!(engine.pending_spell_cast_internal.is_none());
            assert!(engine.pending_ability_activation_internal.is_none());
            assert_eq!(engine.state.objects[&spell].zone, Zone::Hand);
            assert_eq!(engine.state.objects[&permanents[2]].zone, Zone::Battlefield);
        }
    }

    /// Bounded local measurement, not a latency gate or a production throughput claim. Fixture
    /// reset is outside each measurement. Report snapshot cost and an actual accepted pass.
    #[test]
    fn archive_checkpoint_cost_across_object_counts() {
        let pass = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
        };
        for objects in [120, 400, 1000] {
            let mut engine = GameEngine::new(104_817, &[0, 1], 20, None, true).unwrap();
            engine.state.opening = None;
            engine.state.turn_step = TurnStep::Main1;
            engine.state.active_player_idx = 0;
            engine.state.priority_idx = 0;
            let template = engine.state.objects[&engine.state.players[0].hand[0]].clone();
            while engine.state.objects.len() < objects {
                let mut object = template.clone();
                object.id = engine.state.next_object_id;
                engine.state.next_object_id += 1;
                object.zone = Zone::Library;
                engine.state.players[0].library.push_back(object.id);
                engine.state.objects.insert(object.id, object);
            }
            // Warm the publication caches; cold views would swamp the checkpoint measurement.
            engine.initial_response_batch();
            let fixture = engine.numeric_checkpoint();
            let mut cloning = Vec::new();
            let mut command = Vec::new();
            for _ in 0..80 {
                let started = Instant::now();
                let checkpoint = std::hint::black_box(engine.numeric_checkpoint());
                cloning.push(started.elapsed().as_nanos());
                drop(checkpoint);
                let started = Instant::now();
                engine.apply_command(0, &pass).unwrap();
                command.push(started.elapsed().as_nanos());
                // A fresh fixture clone/reset is intentionally excluded from both timings.
                engine.state = fixture.state.clone();
                engine.private_zone_cache = fixture.private_zones.clone();
                engine.battlefield_view_cache = fixture.battlefield.clone();
                engine.first_strike_step_pending_cache = fixture.first_strike;
            }
            cloning.sort_unstable();
            command.sort_unstable();
            println!("Archive debug warm-cache objects={objects} samples=80 checkpoint_us p50={:.2} p95={:.2}; accepted_pass_us p50={:.2} p95={:.2}",
                cloning[40] as f64/1000.0, cloning[76] as f64/1000.0,
                command[40] as f64/1000.0, command[76] as f64/1000.0);
            assert_eq!(engine.state.command_index, fixture.state.command_index);
        }
    }
}
