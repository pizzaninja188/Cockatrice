use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, CounterKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};

fn issue_174_effect(engine: &mut GameEngine, oid: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn issue_174_sprite_uses_animated_types_and_counters_and_loses_static_restrictions() {
    let decks = Some(vec![
        deck_with("forest", &["argothian_sprite"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(174_009, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let sprite = move_ready_to_battlefield(&mut engine, 0, "argothian_sprite");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let animated = inject_permanent_on_battlefield(&mut engine, 1, "explosive_apparatus");
    issue_174_effect(
        &mut engine,
        animated,
        ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
            card_types: vec![tricerules_cards::PermanentTypeFilter::Creature],
            ..Default::default()
        }),
    );
    issue_174_effect(
        &mut engine,
        animated,
        ContinuousEffectKind::Layer7bSetPt {
            power: 3,
            toughness: 3,
        },
    );
    let characteristics = engine.characteristics(animated).expect("animated artifact");
    assert!(characteristics.is_creature() && characteristics.is_artifact());
    let ordinary = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, sprite, 0, vec![]).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.characteristics(sprite).unwrap().power, Some(4));
    issue_174_effect(
        &mut engine,
        blocker,
        ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
            card_types: vec![tricerules_cards::PermanentTypeFilter::Artifact],
            ..Default::default()
        }),
    );
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![sprite, ordinary]))
        .unwrap();
    pass_to_declare_blockers(&mut engine);
    assert!(!engine.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .iter()
        .any(|pair| pair.attacker_id == sprite));
    issue_174_effect(
        &mut engine,
        sprite,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    assert!(engine.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .iter()
        .any(|pair| pair.attacker_id == sprite && pair.blocker_id == animated));
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: sprite,
                blocker_id: animated,
            }]),
        )
        .unwrap();
}

#[test]
fn issue_174_synthetic_defenders_have_separate_blocking_graphs() {
    let mut engine = GameEngine::new(174_010, &[10, 20], 20, None, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.apply_command(10, &primitive_yield()).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(10, &declare_attackers(vec![first, second]))
        .unwrap();
    pass_both_players(&mut engine);
    // The constructor still rejects multiplayer; exercise only the seat-generic snapshot seam.
    let mut third = engine.state.players[1].clone();
    third.id = 30;
    third.battlefield.clear();
    third.hand.clear();
    third.library.clear();
    engine.state.players.push(third);
    let third_blocker = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let assignment = engine
        .state
        .combat
        .as_mut()
        .unwrap()
        .attack_assignments
        .get_mut(&second)
        .unwrap();
    assignment.defending_player = 30;
    assignment.defender = tricerules_core::state::CombatDefenderTarget::Player(30);
    let batch = engine.initial_response_batch();
    assert_eq!(
        batch.legal_by_player[&20].legal_block_pairs,
        [BlockPair {
            attacker_id: first,
            blocker_id: blocker
        }]
    );
    assert_eq!(
        batch.legal_by_player[&30].legal_block_pairs,
        [BlockPair {
            attacker_id: second,
            blocker_id: third_blocker
        }]
    );
}

fn issue_174_graveyard_card(engine: &mut GameEngine, player: usize) -> u32 {
    let oid = engine.state.players[player].library.pop_back().unwrap();
    engine.state.players[player].graveyard.push(oid);
    engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Graveyard;
    oid
}

#[test]
fn issue_174_accepted_commands_replay_after_an_illegal_declaration() {
    use tricerules_proto::ruled::v1::{
        dev_command::Dev, DevAddMana, DevCommand, DevPutCardInZone, DevZone,
    };
    fn fresh() -> GameEngine {
        let mut engine = GameEngine::new(
            174_011,
            &[0, 1],
            20,
            Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]),
            true,
        )
        .unwrap();
        engine.enable_dev_commands();
        engine
    }
    fn record(
        engine: &mut GameEngine,
        log: &mut Vec<(i32, RuledCommand, RuledEventBatch)>,
        player: i32,
        command: RuledCommand,
    ) {
        let batch = engine.apply_command(player, &command).unwrap();
        log.push((player, command, batch));
    }
    let dev = |target, payload| RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(payload),
        })),
    };
    let put = |target, name: &str| {
        dev(
            target,
            Dev::PutCardInZone(DevPutCardInZone {
                card_name: name.into(),
                zone: DevZone::Battlefield as i32,
                ready: true,
            }),
        )
    };
    let mut engine = fresh();
    let mut log = Vec::new();
    for player in [0, 1, 0, 1] {
        record(&mut engine, &mut log, player, pass());
    }
    record(&mut engine, &mut log, 0, put(0, "Rampaging Ceratops"));
    let ceratops = *engine.state.players[0].battlefield.last().unwrap();
    record(&mut engine, &mut log, 0, put(0, "Verdant Outrider"));
    let outrider = *engine.state.players[0].battlefield.last().unwrap();
    let mut blockers = Vec::new();
    for _ in 0..3 {
        record(&mut engine, &mut log, 0, put(1, "Grizzly Bears"));
        blockers.push(*engine.state.players[1].battlefield.last().unwrap());
    }
    record(
        &mut engine,
        &mut log,
        0,
        dev(
            0,
            Dev::AddMana(DevAddMana {
                g: 1,
                c: 1,
                ..Default::default()
            }),
        ),
    );
    let activation = activate_ability_for(&engine, outrider, 0, vec![]);
    record(&mut engine, &mut log, 0, activation);
    for player in [0, 1] {
        record(&mut engine, &mut log, player, pass());
    }
    record(&mut engine, &mut log, 0, primitive_yield());
    for player in [0, 1] {
        record(&mut engine, &mut log, player, pass());
    }
    record(
        &mut engine,
        &mut log,
        0,
        declare_attackers(vec![ceratops, outrider]),
    );
    for player in [0, 1] {
        record(&mut engine, &mut log, player, pass());
    }
    let before = engine.initial_response_batch();
    let index = engine.state.command_index;
    assert!(engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: ceratops,
                blocker_id: blockers[0]
            }])
        )
        .is_err());
    assert_eq!(engine.state.command_index, index);
    assert_eq!(engine.initial_response_batch(), before);
    record(
        &mut engine,
        &mut log,
        1,
        declare_blockers(
            blockers
                .into_iter()
                .map(|blocker_id| BlockPair {
                    attacker_id: ceratops,
                    blocker_id,
                })
                .collect(),
        ),
    );
    let mut replay = fresh();
    for (player, command, expected) in log {
        assert_eq!(replay.apply_command(player, &command).unwrap(), expected);
    }
    assert_eq!(
        replay.initial_response_batch(),
        engine.initial_response_batch()
    );
    assert_eq!(replay.state.command_index, engine.state.command_index);
}

#[test]
fn issue_174_outrider_checks_live_power_but_does_not_undo_blocks() {
    let decks = Some(vec![
        deck_with("forest", &["verdant_outrider"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(174_004, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let outrider = move_ready_to_battlefield(&mut engine, 0, "verdant_outrider");
    let ordinary = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, outrider, 0, vec![]).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![outrider, ordinary]))
        .unwrap();
    pass_to_declare_blockers(&mut engine);
    let blocks = declare_blockers(vec![BlockPair {
        attacker_id: outrider,
        blocker_id: blocker,
    }]);
    let command_index = engine.state.command_index;
    assert!(engine.apply_command(1, &blocks).is_err());
    assert_eq!(engine.state.command_index, command_index);
    engine
        .state
        .objects
        .get_mut(&blocker)
        .unwrap()
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
    assert!(engine.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .iter()
        .any(|p| p.attacker_id == outrider));
    engine.apply_command(1, &blocks).unwrap();
    engine
        .state
        .objects
        .get_mut(&blocker)
        .unwrap()
        .counters
        .clear();
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, outrider, 0, vec![]).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.combat.as_ref().unwrap().blockers[&outrider],
        [blocker]
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 18,
        "only the ordinary unblocked attacker deals player damage"
    );
}

#[test]
fn issue_174_outrider_generation_expiry_and_ability_removal() {
    let decks = Some(vec![
        deck_with("forest", &["verdant_outrider", "unsummon"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(174_005, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let outrider = move_ready_to_battlefield(&mut engine, 0, "verdant_outrider");
    let command_index = engine.state.command_index;
    assert!(apply_ability(&mut engine, 0, outrider, 0, vec![]).is_err());
    assert_eq!(engine.state.command_index, command_index);
    grant_pool(&mut engine, 0);
    let stale_activation = activate_ability_for(&engine, outrider, 0, vec![]);
    engine.apply_command(0, &stale_activation).unwrap();
    ensure_in_hand(&mut engine, 0, "unsummon");
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(outrider)))
        .unwrap();
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&outrider].zone, Zone::Hand);
    assert_eq!(
        move_ready_to_battlefield(&mut engine, 0, "verdant_outrider"),
        outrider
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, outrider).is_empty(),
        "old activation must not bind to the new generation"
    );
    assert!(engine.apply_command(0, &stale_activation).is_err());
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, outrider, 0, vec![]).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    issue_174_effect(
        &mut engine,
        outrider,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, outrider)
            .iter()
            .any(|label| label.contains("power 2 or less")),
        "resolved rules effects survive ability removal"
    );
    end_active_turn(&mut engine, 0);
    assert!(!zone_view_rules_annotation_labels(&mut engine, 0, outrider)
        .iter()
        .any(|label| label.contains("power 2 or less")));
}

#[test]
fn issue_174_hermit_threshold_is_live_and_does_not_change_established_blocks() {
    let decks = Some(vec![
        deck_with("island", &["nightwhorl_hermit"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(174_006, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let hermit = move_ready_to_battlefield(&mut engine, 0, "nightwhorl_hermit");
    let ordinary = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    for _ in 0..6 {
        issue_174_graveyard_card(&mut engine, 0);
    }
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![hermit, ordinary]))
        .unwrap();
    assert!(!engine.state.objects[&hermit].tapped, "vigilance");
    pass_to_declare_blockers(&mut engine);
    assert_eq!(engine.characteristics(hermit).unwrap().power, Some(1));
    let seventh = issue_174_graveyard_card(&mut engine, 0);
    assert_eq!(engine.characteristics(hermit).unwrap().power, Some(2));
    assert!(!engine.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .iter()
        .any(|p| p.attacker_id == hermit));
    engine.state.players[0]
        .graveyard
        .retain(|oid| *oid != seventh);
    engine.state.players[0].hand.push(seventh);
    engine.state.objects.get_mut(&seventh).unwrap().zone = Zone::Hand;
    assert!(engine.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .iter()
        .any(|p| p.attacker_id == hermit));
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: hermit,
                blocker_id: blocker,
            }]),
        )
        .unwrap();
    issue_174_graveyard_card(&mut engine, 0);
    assert_eq!(
        engine.state.combat.as_ref().unwrap().blockers[&hermit],
        [blocker]
    );
    issue_174_effect(
        &mut engine,
        hermit,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    assert_eq!(engine.characteristics(hermit).unwrap().power, Some(1));
    assert!(!zone_view_rules_annotation_labels(&mut engine, 0, hermit)
        .contains(&"Can't be blocked".into()));
}

#[test]
fn issue_174_hermit_threshold_uses_controller_not_owner() {
    let decks = Some(vec![
        deck_with("island", &["nightwhorl_hermit"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(174_007, &[10, 20], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let hermit = move_ready_to_battlefield(&mut engine, 0, "nightwhorl_hermit");
    for _ in 0..7 {
        issue_174_graveyard_card(&mut engine, 0);
    }
    assert_eq!(engine.characteristics(hermit).unwrap().power, Some(2));
    issue_174_effect(
        &mut engine,
        hermit,
        ContinuousEffectKind::Layer2Control {
            controller: tricerules_cards::ControllerReference::Fixed(20),
        },
    );
    assert_eq!(engine.characteristics(hermit).unwrap().controller, 20);
    assert_eq!(engine.characteristics(hermit).unwrap().power, Some(1));
    for _ in 0..7 {
        issue_174_graveyard_card(&mut engine, 1);
    }
    assert_eq!(engine.characteristics(hermit).unwrap().power, Some(2));
    assert_eq!(engine.state.objects[&hermit].owner, 10);
}

#[test]
fn issue_174_outrider_publishes_its_resolved_restriction() {
    let decks = Some(vec![
        deck_with("forest", &["verdant_outrider"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(174_003, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let outrider = move_ready_to_battlefield(&mut engine, 0, "verdant_outrider");
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, outrider, 0, vec![]).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, outrider),
        ["Can't be blocked by creatures with power 2 or less"]
    );
}

#[test]
fn issue_174_ceratops_requires_three_and_sprite_excludes_artifacts() {
    let decks = Some(vec![
        deck_with("forest", &["rampaging_ceratops", "argothian_sprite"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(174_002, &[10, 20], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let ceratops = move_ready_to_battlefield(&mut engine, 0, "rampaging_ceratops");
    let sprite = move_ready_to_battlefield(&mut engine, 0, "argothian_sprite");
    let blockers: Vec<_> = (0..3)
        .map(|_| inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears"))
        .collect();
    let artifact = inject_creature_on_battlefield(&mut engine, 1, "ornithopter");
    engine.apply_command(10, &primitive_yield()).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(10, &declare_attackers(vec![ceratops, sprite]))
        .unwrap();
    pass_both_players(&mut engine);
    let legal = &engine.initial_response_batch().legal_by_player[&20];
    assert!(!legal
        .legal_block_pairs
        .iter()
        .any(|p| p.attacker_id == sprite && p.blocker_id == artifact));
    for count in [1, 2] {
        let command_index = engine.state.command_index;
        assert!(engine
            .apply_command(
                20,
                &declare_blockers(
                    blockers[..count]
                        .iter()
                        .map(|b| BlockPair {
                            attacker_id: ceratops,
                            blocker_id: *b
                        })
                        .collect()
                )
            )
            .is_err());
        assert_eq!(engine.state.command_index, command_index);
    }
    engine
        .apply_command(
            20,
            &declare_blockers(
                blockers
                    .iter()
                    .map(|b| BlockPair {
                        attacker_id: ceratops,
                        blocker_id: *b,
                    })
                    .collect(),
            ),
        )
        .expect("three blockers are legal, Sprite may remain unblocked");
}

#[test]
fn issue_174_competing_must_block_creatures_allow_a_maximal_declaration() {
    let decks = Some(vec![
        deck_with("forest", &["safewright_cavalry"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(174_001, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let attacker = move_ready_to_battlefield(&mut engine, 0, "safewright_cavalry");
    let first = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    for blocker in [first, second] {
        engine
            .state
            .objects
            .get_mut(&blocker)
            .unwrap()
            .must_block_if_able = true;
    }
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .unwrap();
    pass_to_declare_blockers(&mut engine);
    let legal = &engine.initial_response_batch().legal_by_player[&1];
    assert!(
        legal.required_blocker_ids.is_empty(),
        "either blocker is a legal choice; neither is individually mandatory"
    );
    assert_eq!(legal.legal_block_pairs.len(), 2);
    let command_index = engine.state.command_index;
    assert!(engine.apply_command(1, &declare_blockers(vec![])).is_err());
    assert_eq!(engine.state.command_index, command_index);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: first,
            }]),
        )
        .expect("satisfy the maximum one requirement");
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_command(0, &pass())
        .expect("active player passes after declaring attackers");
    engine
        .apply_command(1, &pass())
        .expect("defender passes after attackers are declared")
}

#[test]
fn frilled_sea_serpent_rejects_blocks_and_drives_automatic_empty_blocks() {
    let mut engine = GameEngine::new(77_001, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let serpent = inject_creature_on_battlefield(&mut engine, 0, "frilled_sea_serpent");
    let ordinary_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 7,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(serpent, 0, vec![]))
        .expect("activate Frilled Sea Serpent");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, serpent),
        vec!["Can't be blocked"],
        "the active unblockable effect is visible in the battlefield feed"
    );
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![serpent, ordinary_attacker]))
        .expect("declare attackers");
    pass_to_declare_blockers(&mut engine);

    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: serpent,
                blocker_id: blocker,
            }]),
        )
        .expect_err("the Serpent cannot be blocked this turn");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: ordinary_attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("the same creature can block the ordinary attacker");

    let mut auto = GameEngine::new(77_002, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut auto);
    let serpent = inject_creature_on_battlefield(&mut auto, 0, "frilled_sea_serpent");
    inject_creature_on_battlefield(&mut auto, 1, "grizzly_bears");
    give_mana(
        &mut auto,
        0,
        ManaGift {
            u: 7,
            ..Default::default()
        },
    );
    auto.apply_command(0, &activate_ability(serpent, 0, vec![]))
        .expect("activate Frilled Sea Serpent");
    resolve_entire_stack_two_player(&mut auto);
    advance_main1_to_declare_attackers(&mut auto);
    auto.apply_command(0, &declare_attackers(vec![serpent]))
        .expect("declare Serpent");
    let batch = pass_to_declare_blockers(&mut auto);
    assert!(
        auto.state
            .combat
            .as_ref()
            .expect("combat")
            .blockers_declared
    );
    assert_eq!(blockers_declared_in(&batch)[0].block_pairs, vec![]);
}

#[test]
fn frilled_sea_serpent_does_not_undo_a_block_declared_before_activation() {
    let mut engine = GameEngine::new(77_003, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let serpent = inject_creature_on_battlefield(&mut engine, 0, "frilled_sea_serpent");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![serpent]))
        .expect("declare Serpent");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: serpent,
                blocker_id: blocker,
            }]),
        )
        .expect("declare block before the Serpent ability resolves");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 7,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability(serpent, 0, vec![]))
        .expect("activate after blockers");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.combat.as_ref().expect("combat").blockers[&serpent],
        vec![blocker]
    );
}

#[test]
fn goblin_smuggler_uses_derived_power_and_revalidates_its_target() {
    let mut engine = GameEngine::new(77_004, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let small = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let large = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 3, 3);
    let key = (smuggler as u64) << 32;
    let targets = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
        [&key]
        .groups[0]
        .valid_permanent_ids;
    assert_eq!(targets, &[small]);

    let err = engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(large)))
        .expect_err("power 3 is not a legal target");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert!(!engine.state.objects[&smuggler].tapped);

    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(small)))
        .expect("activate targeting a power-2 creature");
    engine
        .state
        .objects
        .get_mut(&small)
        .expect("small creature")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
    resolve_entire_stack_two_player(&mut engine);

    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![small]))
        .expect("declare the now-power-3 creature");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: small,
                blocker_id: blocker,
            }]),
        )
        .expect("the stale target made the ability fail to resolve");
}

#[test]
fn goblin_smuggler_effect_persists_if_power_increases_after_resolution() {
    let mut engine = GameEngine::new(77_005, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let ordinary_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(target)))
        .expect("activate Goblin Smuggler");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, target),
        vec!["Can't be blocked"],
        "the chosen creature reports the resolved combat restriction"
    );
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![target, ordinary_attacker]))
        .expect("declare attackers");
    pass_to_declare_blockers(&mut engine);
    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: target,
                blocker_id: blocker,
            }]),
        )
        .expect_err("the resolved restriction does not recheck power");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
}

#[test]
fn destructive_tampering_tracks_current_flying_status_and_later_creatures() {
    let decks = Some(vec![
        deck_with("mountain", &["destructive_tampering"]),
        deck_with("mountain", &[]),
    ]);
    let mut engine = GameEngine::new(77_006, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "destructive_tampering");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "destructive_tampering");
    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, vec![])]))
        .expect("cast the blocking-restriction mode");
    resolve_entire_stack_two_player(&mut engine);

    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let ground = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let bird_grabber = inject_creature_on_battlefield(&mut engine, 1, "goblin_bird-grabber");
    let flyer = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");

    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, ground),
        vec!["Can't block"]
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, bird_grabber),
        vec!["Can't block"]
    );
    assert!(zone_view_rules_annotation_labels(&mut engine, 1, flyer).is_empty());

    engine.apply_command(0, &pass()).expect("pass priority");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(1, &activate_ability(bird_grabber, 0, vec![]))
        .expect("grant Flying after Destructive Tampering resolves");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(bird_grabber, Keyword::Flying));
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, bird_grabber),
        vec!["Flying"],
        "gaining Flying removes the dynamic restriction label but retains the granted keyword"
    );

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_to_declare_blockers(&mut engine);

    let legal = &engine.initial_response_batch().legal_by_player[&1];
    let legal_pairs: Vec<_> = legal
        .legal_block_pairs
        .iter()
        .map(|pair| (pair.blocker_id, pair.attacker_id))
        .collect();
    assert_eq!(legal_pairs, [(bird_grabber, attacker), (flyer, attacker)]);
    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: ground,
            }]),
        )
        .expect_err("a later-entering nonflyer cannot block");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: bird_grabber,
            }]),
        )
        .expect("a creature that gained Flying can block");
}

#[test]
fn cant_be_blocked_coexists_with_menace_and_must_block_requirements() {
    let mut engine = GameEngine::new(77_007, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let menace = inject_creature_on_battlefield(&mut engine, 0, "goblin_trailblazer");
    let ordinary = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&blocker)
        .expect("blocker")
        .must_block_if_able = true;

    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(menace)))
        .expect("activate Goblin Smuggler on a creature with menace");
    resolve_entire_stack_two_player(&mut engine);
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![menace, ordinary]))
        .expect("declare attackers");
    pass_to_declare_blockers(&mut engine);

    let legal = &engine.initial_response_batch().legal_by_player[&1];
    assert_eq!(legal.required_blocker_ids, vec![blocker]);
    let legal_pairs: Vec<_> = legal
        .legal_block_pairs
        .iter()
        .map(|pair| (pair.blocker_id, pair.attacker_id))
        .collect();
    assert_eq!(legal_pairs, [(blocker, ordinary)]);
    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: menace,
                blocker_id: blocker,
            }]),
        )
        .expect_err("cant-be-blocked rejects the assignment before menace can matter");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: ordinary,
                blocker_id: blocker,
            }]),
        )
        .expect("must-block is satisfied by the other legal attacker");
}

#[test]
fn legal_block_pairs_exclude_pair_specific_flying_restrictions() {
    let mut engine = GameEngine::new(77_009, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let ground_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let flying_attacker = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    let ground_blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(
            0,
            &declare_attackers(vec![ground_attacker, flying_attacker]),
        )
        .expect("declare mixed attackers");
    pass_to_declare_blockers(&mut engine);

    let legal = &engine.initial_response_batch().legal_by_player[&1];
    let legal_pairs: Vec<_> = legal
        .legal_block_pairs
        .iter()
        .map(|pair| (pair.blocker_id, pair.attacker_id))
        .collect();
    assert_eq!(legal_pairs, [(ground_blocker, ground_attacker)]);
}

#[test]
fn chosen_combat_restriction_expires_at_cleanup() {
    let mut engine = GameEngine::new(77_008, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(target)))
        .expect("activate Goblin Smuggler");
    resolve_entire_stack_two_player(&mut engine);

    end_active_turn(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine);

    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![target]))
        .expect("declare the formerly restricted creature");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: target,
                blocker_id: blocker,
            }]),
        )
        .expect("the until-end-of-turn restriction expired during cleanup");
}

#[test]
fn chosen_combat_restriction_does_not_follow_a_zone_change() {
    let decks = Some(vec![
        deck_with("island", &["unsummon"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(77_009, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let smuggler = inject_creature_on_battlefield(&mut engine, 0, "goblin_smuggler");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &activate_ability(smuggler, 0, target_object(target)))
        .expect("activate Goblin Smuggler");
    resolve_entire_stack_two_player(&mut engine);

    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Unsummon");
    resolve_entire_stack_two_player(&mut engine);
    let returned = put_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(
        returned, target,
        "the helper deliberately reuses the ObjectId"
    );

    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![returned]))
        .expect("declare the new object represented by the reused id");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: returned,
                blocker_id: blocker,
            }]),
        )
        .expect("the previous object's restriction was cleared on zone change");
}
