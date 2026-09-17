//! Issue #318 — the five reviewed single-clause completion cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 400.3/608.2d govern the owner-choice library placement and its logged resolution-time choice;
//! CR 601.2f governs the target-matching cost reduction; CR 111.1/701.6 and 509.1b govern the 1/1
//! black Rat token and its can't-block restriction; CR 611.2a/514.2 govern the until-end-of-turn
//! flying grant and its expiry; CR 603.2/120.3 govern the self-damage draw trigger.

use super::helpers::*;
use tricerules_cards::{Color, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, BlockPair, ChoiceKind, ChooseTriggerTarget, ResolutionChoiceDecision,
    RuledCommand, SubmitResolutionChoice,
};

fn deck_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn has_log(batch: &RuledEventBatch, text: &str) -> bool {
    batch
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == text))
}

fn pool_total(engine: &GameEngine, player: usize) -> u32 {
    let pool = &engine.state.players[player].mana_pool;
    pool.white + pool.blue + pool.black + pool.red + pool.green + pool.colorless
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
fn issue_318_misleading_motes_owner_chooses_top_or_bottom() {
    for (seed, branch, expected_front) in [(318_001, 0u32, true), (318_002, 1, false)] {
        let mut engine = deck_engine(seed, &["misleading_motes"], &["grizzly_bears"]);
        let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        ensure_in_hand(&mut engine, 0, "misleading_motes");
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, "misleading_motes");

        engine
            .apply_command(0, &cast_spell(slot, target_object(bear)))
            .expect("cast Misleading Motes at the opposing bear");
        engine
            .apply_command(0, &pass())
            .expect("caster passes priority");
        let parked = engine
            .apply_command(1, &pass())
            .expect("opponent passes priority into resolution");

        let choice = find_resolution_choice(&parked).expect("owner placement choice");
        assert_eq!(
            choice.deciding_player_id, 1,
            "the targeted creature's owner makes the placement choice"
        );
        assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
        assert_eq!((choice.min, choice.max), (1, 1));
        assert!(
            choice.prompt_text.contains("Grizzly Bears")
                && choice.prompt_text.contains("top")
                && choice.prompt_text.contains("bottom"),
            "the placement choice is publicly logged: {}",
            choice.prompt_text
        );
        assert!(
            has_log(&parked, &choice.prompt_text),
            "the owner placement prompt is logged"
        );
        assert_eq!(choice.resolution_branches.len(), 2);
        assert_eq!(choice.resolution_branches[0].label, "Top");
        assert_eq!(choice.resolution_branches[1].label, "Bottom");

        assert!(
            engine.apply_command(0, &select_branch(branch)).is_err(),
            "only the owner may choose the placement"
        );
        let completion = engine
            .apply_command(1, &select_branch(branch))
            .expect("the owner chooses top or bottom");

        assert_eq!(engine.state.objects[&bear].zone, Zone::Library);
        let library: Vec<u32> = engine.state.players[1].library.iter().copied().collect();
        if expected_front {
            assert_eq!(library.first().copied(), Some(bear));
        } else {
            assert_eq!(library.last().copied(), Some(bear));
        }
        assert!(
            permanents_moved_in(&completion)
                .iter()
                .any(|moved| moved.object_id == bear),
            "the permanent move is published"
        );
    }
}

#[test]
fn issue_318_run_behind_reduces_only_for_an_attacking_target() {
    let decks = Some(vec![
        deck_with("island", &["run_behind", "run_behind"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(318_010, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "run_behind");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let token_attacker = inject_creature_on_battlefield(&mut engine, 0, "soldier_w_1_1");
    let tapped = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&tapped).expect("bear").tapped = true;

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker, token_attacker]))
        .expect("attack with a creature and an attacking token");

    let slot = hand_index_for_card(&engine, 0, "run_behind");
    let batch = engine.initial_response_batch();
    let published = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    let application = published
        .targeted_cost_reduction_applications
        .first()
        .expect("attacking-target reduction application");
    assert_eq!(application.generic_mana, 1);
    assert!(application
        .qualifying_targets
        .iter()
        .any(|candidate| candidate.object_id == attacker));
    assert!(
        application
            .qualifying_targets
            .iter()
            .any(|candidate| candidate.object_id == token_attacker),
        "Run Behind reduces for any attacking creature, including a token"
    );
    assert!(
        !application
            .qualifying_targets
            .iter()
            .any(|candidate| candidate.object_id == tapped),
        "a tapped nonattacker does not qualify"
    );

    // The reduced cost is exactly {2}{U}: {1}{U} is not enough, and the two-colorless total is.
    let command = cast_spell(slot, target_object(attacker));
    let index = engine.state.command_index;
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    assert!(engine.apply_command(0, &command).is_err());
    assert_eq!(engine.state.command_index, index);
    engine.state.players[0].mana_pool.colorless = 2;
    engine
        .apply_command(0, &command)
        .expect("the attacking target reduces the cost to {2}{U}");
    assert_eq!(pool_total(&engine, 0), 0);
    engine
        .apply_command(0, &pass())
        .expect("caster passes priority");
    let parked = engine
        .apply_command(1, &pass())
        .expect("opponent passes priority into resolution");
    let choice = find_resolution_choice(&parked).expect("owner placement choice");
    assert_eq!(choice.deciding_player_id, 0);
    engine
        .apply_command(0, &select_branch(0))
        .expect("the owner chooses top");
    assert_eq!(
        engine.state.players[0].library.front().copied(),
        Some(attacker)
    );

    // The same spell at the tapped nonattacker is not reduced and needs the full {3}{U}.
    ensure_in_hand(&mut engine, 0, "run_behind");
    let slot = hand_index_for_card(&engine, 0, "run_behind");
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let command = cast_spell(slot, target_object(tapped));
    let index = engine.state.command_index;
    assert!(
        engine.apply_command(0, &command).is_err(),
        "an unreduced Run Behind cannot be cast for {{2}}{{U}}"
    );
    assert_eq!(engine.state.command_index, index);
    engine.state.players[0].mana_pool.colorless = 3;
    engine
        .apply_command(0, &command)
        .expect("the full {3}{U} pays for a nonattacking target");
    assert_eq!(pool_total(&engine, 0), 0);
}

#[test]
fn issue_318_edgewall_pack_creates_a_blockless_rat_token() {
    let mut engine = deck_engine(318_020, &[], &["edgewall_pack"]);
    move_ready_to_battlefield(&mut engine, 1, "edgewall_pack");
    pass_both_players(&mut engine);

    let rats = battlefield_token_oids(&engine, 1, "rat_b_1_1_cant_block");
    assert_eq!(rats.len(), 1, "the ETB creates exactly one Rat");
    let rat = rats[0];
    let characteristics = engine.characteristics(rat).expect("Rat characteristics");
    assert!(characteristics.is_creature());
    assert!(characteristics.has_type("Rat"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(1), Some(1))
    );
    assert_eq!(characteristics.colors, vec![Color::Black]);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, rat),
        vec!["Can't block"]
    );

    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("attack with the bear");
    pass_to_declare_blockers(&mut engine);
    let legal = &engine.initial_response_batch().legal_by_player[&1];
    assert!(
        !legal
            .legal_block_pairs
            .iter()
            .any(|pair| pair.blocker_id == rat),
        "the Rat token is never a legal blocker"
    );
    assert!(
        engine
            .apply_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: rat,
                }]),
            )
            .is_err(),
        "declaring the Rat as a blocker is rejected"
    );
}

#[test]
fn issue_318_stratosoarer_grants_flying_until_end_of_turn() {
    let mut engine = deck_engine(318_030, &["stratosoarer"], &["grizzly_bears"]);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    assert!(!engine.effective_has_keyword(target, Keyword::Flying));

    move_ready_to_battlefield(&mut engine, 0, "stratosoarer");
    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("mandatory ETB target choice");
    assert!(!pending.may, "Stratosoarer's ETB is mandatory");
    let key = (pending.source_permanent_id as u64) << 32 | pending.ability_index as u64;
    let batch = engine.initial_response_batch();
    assert!(
        batch.legal_by_player[&0].valid_targets_by_ability[&key].groups[0]
            .valid_permanent_ids
            .contains(&target),
        "the chosen creature is a legal target"
    );
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(target),
                })),
            },
        )
        .expect("choose the flying target");
    pass_both_players(&mut engine);
    assert!(engine.effective_has_keyword(target, Keyword::Flying));

    end_active_turn(&mut engine, 0);
    assert!(
        !engine.effective_has_keyword(target, Keyword::Flying),
        "the grant expires at cleanup"
    );
}

#[test]
fn issue_318_thieving_otter_draws_for_combat_and_own_noncombat_damage() {
    // Combat damage from the Otter itself draws exactly one card.
    let mut engine = deck_engine(318_040, &["thieving_otter"], &[]);
    let otter = move_ready_to_battlefield(&mut engine, 0, "thieving_otter");
    let before = engine.state.players[0].hand.len();
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![otter]))
        .expect("attack with the Otter");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(0, &pass())
        .expect("active player passes in declare blockers");
    engine
        .apply_command(1, &pass())
        .expect("defender passes into combat damage");
    assert_eq!(engine.state.players[1].life, 18, "the Otter deals two");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        before + 1,
        "combat damage to an opponent draws exactly once"
    );

    // Noncombat damage: another creature's ping does not trigger the Otter, but the same physical
    // source dealing the damage as the Otter does. Model the source acquiring the Otter's printed
    // ability while the ping is on the stack (the issue #85 Thieving Magpie pattern).
    let mut engine = deck_engine(318_041, &["thieving_otter"], &[]);
    move_ready_to_battlefield(&mut engine, 0, "thieving_otter");
    let pinger = inject_creature_on_battlefield(&mut engine, 0, "prodigal_sorcerer");
    let before = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &activate_ability(pinger, 0, target_player(1)))
        .expect("ping the opponent with another creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        before,
        "damage from a different source must not draw"
    );

    engine
        .state
        .objects
        .get_mut(&pinger)
        .expect("pinger")
        .tapped = false;
    engine
        .apply_command(0, &activate_ability(pinger, 0, target_player(1)))
        .expect("ping the opponent");
    engine
        .state
        .objects
        .get_mut(&pinger)
        .expect("pinger")
        .card_id = "thieving_otter".into();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        before + 1,
        "the Otter's own noncombat damage to an opponent draws once"
    );

    // Damage to a non-opponent player does not trigger the ability.
    let pinger_object = engine.state.objects.get_mut(&pinger).expect("pinger");
    pinger_object.tapped = false;
    pinger_object.card_id = "prodigal_sorcerer".into();
    engine
        .apply_command(0, &activate_ability(pinger, 0, target_player(0)))
        .expect("ping its own controller");
    engine
        .state
        .objects
        .get_mut(&pinger)
        .expect("pinger")
        .card_id = "thieving_otter".into();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        before + 1,
        "damage to a non-opponent player must not draw"
    );
}
