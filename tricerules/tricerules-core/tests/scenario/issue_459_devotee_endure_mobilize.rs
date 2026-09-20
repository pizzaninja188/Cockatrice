//! Issue #459 - the Devotee tri-color mana, fixed Endure, and fixed Mobilize families through
//! the command boundary.
//!
//! Every expectation is the reviewed printed Oracle behavior, not a copy of generator output.
//! Exact Scryfall records and `rulings_uri` responses were fetched 2026-09-20 against pinned
//! snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`. The Fortress Kin-Guard ruling confirms the
//! resolution-time counter-or-Spirit choice and that a creature which cannot receive counters
//! (for example one no longer on the battlefield) just creates the Spirit; the Dalkovan
//! Packbeasts ruling confirms the Warrior tokens enter attacking without having been declared as
//! attackers, so abilities that trigger whenever a creature attacks do not trigger for them.
//! Governing CR concepts: CR 602.5b (once-each-turn activation restrictions), CR 701.63
//! (Endure), CR 702.181 (Mobilize), CR 508.4 (attacking without being declared as attacking),
//! CR 111.1 (tokens), CR 122.1 (+1/+1 counters), and CR 603.7 / 514.2 (the delayed
//! next-end-step sacrifice).

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ResolutionChoiceDecision, RuledCommand,
};

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            cast_spell: None,
            spell_cast_announcement: None,
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: vec![],
        })),
    }
}

fn abzan_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["abzan_devotee"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn pool(engine: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

#[test]
fn issue_459_abzan_devotee_produces_each_color_once_per_turn() {
    for (index, option_index) in [0u32, 1, 2].into_iter().enumerate() {
        let mut engine = abzan_engine(459_100 + index as u64);
        let source = move_ready_to_battlefield(&mut engine, 0, "abzan_devotee");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let mut command = activate_ability_for(&engine, source, 0, vec![]);
        let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
            unreachable!("activate_ability_for builds an activation");
        };
        activation.mana_option_index = option_index;
        semantic::accepted(&mut engine, 0, &command);
        let expected = match option_index {
            0 => (1, 0, 0, 0, 0, 0),
            1 => (0, 0, 1, 0, 0, 0),
            2 => (0, 0, 0, 0, 1, 0),
            _ => unreachable!(),
        };
        assert_eq!(pool(&engine), expected, "option {option_index}");

        // CR 602.5b: the printed once-each-turn restriction rejects a second activation even
        // when a fresh {1} is available, and the rejected attempt consumes nothing.
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let before = pool(&engine);
        let stale = activate_ability_for(&engine, source, 0, vec![]);
        engine
            .apply_command(0, &stale)
            .expect_err("the second activation in the same turn is illegal");
        assert_eq!(pool(&engine), before, "rejected activation");
    }
}

#[test]
fn issue_459_abzan_devotee_returns_itself_from_the_graveyard() {
    let mut engine = abzan_engine(459_120);
    let source = inject_graveyard_card(&mut engine, 0, "abzan_devotee");
    let generation = semantic::generation(&engine, source);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let command = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Graveyard as i32,
            expected_zone_change_generation: generation,
            ability_index: 1,
            targets: Vec::new(),
            ..Default::default()
        })),
    };
    semantic::accepted(&mut engine, 0, &command);
    resolve_entire_stack_two_player(&mut engine);
    semantic::assert_object(
        &engine,
        source,
        "abzan_devotee",
        0,
        0,
        Zone::Hand,
        generation + 1,
    );
}

/// Casts Fortress Kin-Guard and runs the entry trigger to its parked branch choice.
fn cast_fortress_to_choice(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "fortress_kin-guard");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "fortress_kin-guard");
    semantic::accepted(engine, 0, &cast_spell(slot, vec![]));
    pass_both_players(engine); // the creature enters and the endure trigger waits
    pass_both_players(engine); // the trigger resolves and parks the branch choice
    source
}

#[test]
fn issue_459_endure_one_offers_both_branches_and_applies_the_choice() {
    let mut counters_engine = semantic::main_phase(459_200);
    let source = cast_fortress_to_choice(&mut counters_engine);
    let pending = counters_engine
        .state
        .pending_resolution
        .as_ref()
        .expect("endure branch choice");
    assert_eq!(pending.deciding_player, 0);
    semantic::accepted(&mut counters_engine, 0, &select_branch(0));
    assert_eq!(
        counters_engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the counter branch adds exactly one +1/+1 counter"
    );
    assert!(battlefield_token_oids(&counters_engine, 0, "spirit_w_1_1").is_empty());

    let mut token_engine = semantic::main_phase(459_201);
    cast_fortress_to_choice(&mut token_engine);
    semantic::accepted(&mut token_engine, 0, &select_branch(1));
    let tokens = battlefield_token_oids(&token_engine, 0, "spirit_w_1_1");
    assert_eq!(
        tokens.len(),
        1,
        "the Spirit branch creates exactly one token"
    );
    let characteristics = token_engine
        .characteristics(tokens[0])
        .expect("token characteristics");
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(1), Some(1))
    );
}

#[test]
fn issue_459_endure_counter_branch_falls_back_to_the_spirit_when_the_source_leaves() {
    let mut engine = semantic::main_phase(459_210);
    let source = inject_card_into_hand(&mut engine, 0, "fortress_kin-guard");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "fortress_kin-guard");
    semantic::accepted(&mut engine, 0, &cast_spell(slot, vec![]));
    pass_both_players(&mut engine); // the creature enters; the endure trigger waits
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    let bolt_slot = hand_index_for_card(&engine, 0, "lightning_bolt");
    semantic::accepted(
        &mut engine,
        0,
        &cast_spell(bolt_slot, target_object(source)),
    );
    pass_both_players(&mut engine); // the Bolt resolves and the source dies
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    pass_both_players(&mut engine); // endure resolves with no live counter branch
    assert!(
        engine.state.pending_resolution.is_none(),
        "a dead source forces the Spirit branch without a prompt"
    );
    assert_eq!(battlefield_token_oids(&engine, 0, "spirit_w_1_1").len(), 1);
}

#[test]
fn issue_459_mobilize_three_creates_attacking_warriors_and_no_attack_watcher_fires() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["dalkovan_packbeasts", "misty_mountains_raider"],
        ),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(459_300, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    let packbeasts = relocate_to_battlefield(&mut engine, 0, "dalkovan_packbeasts", false);
    relocate_to_battlefield(&mut engine, 0, "misty_mountains_raider", false);

    // Exactly one declared attacker: the Packbeasts. Both the controller-attacks watcher and the
    // Mobilize trigger wait. The opponent player is the only legal defending recipient, so the
    // engine auto-selects each Warrior's defender (CR 508.4); issue_106's scenario covers the
    // multi-option prompt shape.
    semantic::accepted(&mut engine, 0, &declare_attackers(vec![packbeasts]));
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());

    let tokens = battlefield_token_oids(&engine, 0, "warrior_r_1_1");
    assert_eq!(tokens.len(), 3);
    let combat = engine.state.combat.as_ref().expect("combat state");
    for token in &tokens {
        let characteristics = engine
            .characteristics(*token)
            .expect("token characteristics");
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(1), Some(1))
        );
        assert!(engine.state.objects[token].tapped, "tokens enter tapped");
        assert!(
            combat.attacking.contains(token),
            "tokens enter attacking without a declaration"
        );
    }

    // CR 508.4: the Warrior tokens were never declared as attackers, so the controller-attacks
    // watcher triggered only for the Packbeasts declaration and its Army carries exactly the two
    // counters from that one resolution.
    let army = battlefield_token_oids(&engine, 0, "goblin_army_b_0_0");
    assert_eq!(army.len(), 1, "the watcher fired exactly once");
    assert_eq!(
        engine.state.objects[&army[0]].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "the attacking tokens must not trigger the attack watcher"
    );

    for _ in 0..12 {
        if engine.state.turn_step == TurnStep::EndStep {
            break;
        }
        semantic::accepted(&mut engine, 0, &primitive_yield());
    }
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        battlefield_token_oids(&engine, 0, "warrior_r_1_1").is_empty(),
        "the delayed next-end-step trigger sacrifices the exact Warrior cohort"
    );
}
