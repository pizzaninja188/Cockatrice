use super::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, ControllerReference, EffectDuration, PermanentTypeFilter,
    TypeLineReplacement,
};
use tricerules_cards::CounterKind;
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::TurnStep;
use tricerules_proto::ruled::v1::{
    dev_command, BlockPair, DevCommand, DevMoveCard, DevPutCardInZone, DevZone, RuledCommand,
};

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    let active_player = engine.state.active_player_id();
    let defending_player = engine
        .state
        .sole_defending_player_id()
        .expect("sole defending player");
    engine
        .apply_command(active_player, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_command(active_player, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_command(defending_player, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) -> RuledEventBatch {
    let active_player = engine.state.active_player_id();
    let defending_player = engine
        .state
        .sole_defending_player_id()
        .expect("sole defending player");
    engine
        .apply_command(active_player, &pass())
        .expect("active player passes after declaring attackers");
    engine
        .apply_command(defending_player, &pass())
        .expect("defender passes after attackers are declared")
}

fn move_card(target: i32, zone: DevZone, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                card_name: card_name.to_string(),
                zone: zone as i32,
                ready: true,
            })),
        })),
    }
}

fn put_ready_on_battlefield(target: i32, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(dev_command::Dev::PutCardInZone(DevPutCardInZone {
                card_name: card_name.to_string(),
                zone: DevZone::Battlefield as i32,
                ready: true,
            })),
        })),
    }
}

fn set_counter(engine: &mut GameEngine, oid: u32, kind: CounterKind, count: u32) {
    engine
        .state
        .objects
        .get_mut(&oid)
        .expect("counter recipient")
        .set_counter(kind, count);
}

#[test]
fn any_and_named_counter_scopes_recompute_live_membership() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "michelangelo,_mutant_bff",
                "herald_of_secret_streams",
                "grizzly_bears",
                "grizzly_bears",
                "grizzly_bears",
            ],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(194_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "michelangelo,_mutant_bff");
    resolve_entire_stack_two_player(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "herald_of_secret_streams");
    let plus_one = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let stun = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let counterless = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    set_counter(&mut engine, plus_one, CounterKind::PlusOnePlusOne, 1);
    set_counter(&mut engine, stun, CounterKind::Stun, 1);
    set_counter(&mut engine, opponent, CounterKind::PlusOnePlusOne, 1);

    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, plus_one),
        vec![
            "Can't be blocked",
            "Can't be blocked by more than 1 creature"
        ]
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, stun),
        vec!["Can't be blocked by more than 1 creature"]
    );
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, counterless).is_empty());
    assert!(zone_view_rules_annotation_labels(&mut engine, 1, opponent).is_empty());

    let affected_control_timestamp = engine.state.command_index + 1;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(stun),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: affected_control_timestamp,
    });
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, stun).is_empty());
    engine
        .state
        .continuous_effects
        .retain(|effect| effect.timestamp != affected_control_timestamp);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, stun),
        vec!["Can't be blocked by more than 1 creature"]
    );

    set_counter(&mut engine, plus_one, CounterKind::PlusOnePlusOne, 0);
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, plus_one).is_empty());

    let late = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    set_counter(&mut engine, late, CounterKind::PlusOnePlusOne, 1);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, late),
        vec![
            "Can't be blocked",
            "Can't be blocked by more than 1 creature"
        ]
    );

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(late),
        kind: ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: Vec::new(),
        }),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, late).is_empty());
}

#[test]
fn source_control_departure_and_return_retarget_the_scope() {
    let decks = Some(vec![
        deck_with("island", &["herald_of_secret_streams", "grizzly_bears"]),
        deck_with("island", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(194_002, &[10, 20], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let herald = move_ready_to_battlefield(&mut engine, 0, "herald_of_secret_streams");
    let original_bear = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_bear = move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    set_counter(&mut engine, original_bear, CounterKind::PlusOnePlusOne, 1);
    set_counter(&mut engine, opponent_bear, CounterKind::PlusOnePlusOne, 1);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, original_bear),
        vec!["Can't be blocked"]
    );
    assert!(zone_view_rules_annotation_labels(&mut engine, 1, opponent_bear).is_empty());

    let blank_timestamp = engine.state.command_index + 1;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(herald),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: blank_timestamp,
    });
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, original_bear).is_empty());
    engine
        .state
        .continuous_effects
        .retain(|effect| effect.timestamp != blank_timestamp);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, original_bear),
        vec!["Can't be blocked"]
    );

    let control_timestamp = engine.state.command_index + 2;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(herald),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(20),
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: control_timestamp,
    });
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, original_bear).is_empty());
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, opponent_bear),
        vec!["Can't be blocked"]
    );

    engine
        .state
        .continuous_effects
        .retain(|effect| effect.timestamp != control_timestamp);
    let generation = engine.state.zone_change_generation[&herald];
    engine
        .apply_command(
            10,
            &move_card(10, DevZone::Hand, "Herald of Secret Streams"),
        )
        .expect("move Herald to hand");
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, original_bear).is_empty());
    engine
        .apply_command(
            10,
            &move_card(10, DevZone::Battlefield, "Herald of Secret Streams"),
        )
        .expect("return Herald to battlefield");
    assert_eq!(
        battlefield_object_for_card(&engine, 0, "herald_of_secret_streams"),
        herald
    );
    assert!(engine.state.zone_change_generation[&herald] > generation);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, original_bear),
        vec!["Can't be blocked"]
    );
}

#[test]
fn clone_copies_the_scoped_restriction_for_its_controller() {
    let decks = Some(vec![
        deck_with("island", &["clone", "grizzly_bears"]),
        deck_with("island", &["herald_of_secret_streams"]),
    ]);
    let mut engine = GameEngine::new(194_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let herald = move_ready_to_battlefield(&mut engine, 1, "herald_of_secret_streams");
    engine
        .apply_command(0, &put_ready_on_battlefield(0, "Clone"))
        .expect("put Clone and request a copy source");
    engine
        .apply_command(0, &submit_resolution_choice(vec![herald]))
        .expect("copy Herald");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    let bear = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    set_counter(&mut engine, bear, CounterKind::PlusOnePlusOne, 1);

    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, bear),
        vec!["Can't be blocked"]
    );
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, clone).is_empty());
}

#[test]
fn maximum_one_publishes_pairs_and_rejects_a_forged_double_block_atomically() {
    let decks = Some(vec![
        deck_with("forest", &["michelangelo,_mutant_bff", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(194_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "michelangelo,_mutant_bff");
    resolve_entire_stack_two_player(&mut engine);
    let attacker = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let first = move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    let second = move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    set_counter(&mut engine, attacker, CounterKind::Stun, 1);

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    let blockers = pass_to_declare_blockers(&mut engine);
    let legal = &blockers.legal_by_player[&1].legal_block_pairs;
    assert_eq!(legal.len(), 2, "each individual block remains selectable");

    let command_before = engine.state.command_index;
    engine
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: first,
                },
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: second,
                },
            ]),
        )
        .expect_err("maximum one rejects two blockers");
    assert_eq!(engine.state.command_index, command_before);
    assert!(engine
        .state
        .combat
        .as_ref()
        .expect("combat")
        .blockers
        .is_empty());
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: first,
            }]),
        )
        .expect("one blocker remains legal");
}

#[test]
fn cumulative_prohibition_removes_pairs_and_forged_blocks_are_rejected() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "michelangelo,_mutant_bff",
                "herald_of_secret_streams",
                "grizzly_bears",
            ],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(194_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "michelangelo,_mutant_bff");
    resolve_entire_stack_two_player(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "herald_of_secret_streams");
    let attacker = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    set_counter(&mut engine, attacker, CounterKind::PlusOnePlusOne, 1);

    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, attacker),
        vec![
            "Can't be blocked",
            "Can't be blocked by more than 1 creature"
        ]
    );
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    let blockers = pass_to_declare_blockers(&mut engine);
    assert!(blockers.legal_by_player[&1].legal_block_pairs.is_empty());
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect_err("unblockable attacker rejects forged blocker");
}

#[test]
fn gaining_a_counter_after_blockers_does_not_make_the_attacker_unblocked() {
    let decks = Some(vec![
        deck_with("island", &["herald_of_secret_streams", "grizzly_bears"]),
        deck_with("island", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(194_006, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    move_ready_to_battlefield(&mut engine, 0, "herald_of_secret_streams");
    let attacker = move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker = move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("block before the counter exists");

    set_counter(&mut engine, attacker, CounterKind::PlusOnePlusOne, 1);
    assert_eq!(
        engine.state.combat.as_ref().expect("combat").blockers[&attacker],
        vec![blocker]
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, attacker),
        vec!["Can't be blocked"]
    );
}

#[test]
fn michelangelo_creates_mutagen_on_entry_and_attack() {
    let decks = Some(vec![
        deck_with("forest", &["michelangelo,_mutant_bff"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(194_007, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let michelangelo = move_ready_to_battlefield(&mut engine, 0, "michelangelo,_mutant_bff");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "mutagen").len(), 1);

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![michelangelo]))
        .expect("Michelangelo attacks");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "mutagen").len(), 2);
}
