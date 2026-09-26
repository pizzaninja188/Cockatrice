//! Actual-card coverage for Manifold Key's artifact untap and unblockable abilities.
//!
//! Oracle text and its Scryfall ruling were checked 2026-09-26. CR 602.2a-b and 115.1c cover
//! activation and target choice; CR 509.1b preserves an already-declared legal block, and CR 608.2b
//! rechecks targets on resolution.

use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep};
use tricerules_proto::ruled::v1::BlockPair;

const MANIFOLD_KEY: &str = "manifold_key";

fn main1_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("advance to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("defending player passes");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

#[test]
fn manifold_key_untaps_another_artifact_and_rejects_illegal_targets() {
    let mut engine = main1_engine(202_609_320);
    let key = inject_permanent_on_battlefield(&mut engine, 0, MANIFOLD_KEY);
    let opponent_key = inject_permanent_on_battlefield(&mut engine, 1, "voltaic_key");
    let nonartifact = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&opponent_key).unwrap().tapped = true;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    for illegal_target in [key, nonartifact] {
        engine
            .apply_command(
                0,
                &activate_ability_for(&engine, key, 0, target_object(illegal_target)),
            )
            .expect_err("the target must be another artifact");
        assert!(!engine.state.objects[&key].tapped);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    }

    apply_ability(&mut engine, 0, key, 0, target_object(opponent_key))
        .expect("activate targeting an opponent's artifact");
    assert!(
        engine.state.objects[&key].tapped,
        "the tap cost is paid now"
    );
    assert!(
        engine.state.objects[&opponent_key].tapped,
        "untapping waits for resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects[&opponent_key].tapped);
}

#[test]
fn manifold_key_makes_a_target_unblockable_and_cannot_unblock_a_blocked_creature() {
    let mut before_blocks = main1_engine(202_609_321);
    let key = inject_permanent_on_battlefield(&mut before_blocks, 0, MANIFOLD_KEY);
    let attacker = inject_creature_on_battlefield(&mut before_blocks, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut before_blocks, 1, "grizzly_bears");
    give_mana(
        &mut before_blocks,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_ability(&mut before_blocks, 0, key, 1, target_object(attacker))
        .expect("activate the creature evasion ability");
    resolve_entire_stack_two_player(&mut before_blocks);

    advance_main1_to_declare_attackers(&mut before_blocks);
    before_blocks
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("attack with the target creature");
    before_blocks
        .apply_command(0, &pass())
        .expect("active player passes after attackers");
    before_blocks
        .apply_command(1, &pass())
        .expect("defending player passes to declare blockers");
    assert_eq!(before_blocks.state.turn_step, TurnStep::DeclareBlockers);
    let legal_pairs = before_blocks.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .clone();
    assert!(legal_pairs.iter().all(|pair| pair.attacker_id != attacker));
    before_blocks
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect_err("an unblockable attacker cannot be blocked");

    let mut after_blocks = main1_engine(202_609_322);
    let key = inject_permanent_on_battlefield(&mut after_blocks, 0, MANIFOLD_KEY);
    let attacker = inject_creature_on_battlefield(&mut after_blocks, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut after_blocks, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut after_blocks);
    after_blocks
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("attack before activating the evasion ability");
    after_blocks
        .apply_command(0, &pass())
        .expect("active player passes after attackers");
    after_blocks
        .apply_command(1, &pass())
        .expect("defending player passes to declare blockers");
    assert_eq!(after_blocks.state.turn_step, TurnStep::DeclareBlockers);
    after_blocks
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare a legal block before the ability resolves");
    assert_eq!(
        after_blocks.state.combat.as_ref().unwrap().blockers[&attacker],
        [blocker]
    );

    give_mana(
        &mut after_blocks,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_ability(&mut after_blocks, 0, key, 1, target_object(attacker))
        .expect("activate after blockers were declared");
    resolve_entire_stack_two_player(&mut after_blocks);
    assert_eq!(
        after_blocks.state.combat.as_ref().unwrap().blockers[&attacker],
        [blocker],
        "the ruling leaves an already-blocked attacker blocked"
    );
}
