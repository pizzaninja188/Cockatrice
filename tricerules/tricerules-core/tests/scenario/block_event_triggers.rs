use crate::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::BlockPair;

fn setup_block(attacker_card: &str, blocker_card: &str, seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("swamp", &[attacker_card]),
        deck_with("forest", &[blocker_card]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut engine);
    let attacker = relocate_to_battlefield(&mut engine, 0, attacker_card, false);
    let blocker = relocate_to_battlefield(&mut engine, 1, blocker_card, false);

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare blocker");

    (engine, attacker, blocker)
}

#[test]
fn snarespinner_triggers_only_when_it_blocks_a_flying_creature() {
    let (mut flying, _, snarespinner) = setup_block("aven_wind_mage", "snarespinner", 6201);
    assert_eq!(
        flying.state.stack.len(),
        1,
        "flying attacker creates trigger"
    );
    resolve_entire_stack_two_player(&mut flying);
    assert_eq!(
        flying
            .characteristics(snarespinner)
            .and_then(|value| value.power),
        Some(3),
        "Snarespinner receives +2/+0 before combat damage"
    );

    let (nonflying, _, _) = setup_block("grizzly_bears", "snarespinner", 6202);
    assert!(
        nonflying.state.stack.is_empty(),
        "a nonflying attacker does not create the trigger"
    );
}

#[test]
fn gloom_sower_drains_the_blocking_creatures_controller() {
    let (mut engine, _, blocker) = setup_block("gloom_sower", "grizzly_bears", 6203);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one blocker creates one trigger"
    );

    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != blocker);
    engine.state.players[1].graveyard.push(blocker);
    engine
        .state
        .objects
        .get_mut(&blocker)
        .expect("blocker")
        .zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(blocker)
        .or_default() += 1;

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 22);
    assert_eq!(
        engine.state.players[1].life, 18,
        "the trigger uses the departed blocker's controller at the event as LKI"
    );
}

#[test]
fn gloom_sower_triggers_once_for_each_blocking_creature() {
    let decks = Some(vec![
        deck_with("swamp", &["gloom_sower"]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(6204, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut engine);
    let attacker = relocate_to_battlefield(&mut engine, 0, "gloom_sower", false);
    let first_blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let second_blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: first_blocker,
                },
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: second_blocker,
                },
            ]),
        )
        .expect("declare blockers");

    let pending = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("two simultaneous block triggers require ordering");
    assert_eq!(pending.candidates.len(), 2);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 2);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 24);
    assert_eq!(engine.state.players[1].life, 16);
}
