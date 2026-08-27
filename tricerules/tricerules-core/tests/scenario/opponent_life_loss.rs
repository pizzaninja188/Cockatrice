use super::helpers::*;

#[test]
fn infectious_horror_attack_trigger_is_untargeted_life_loss_not_damage() {
    let mut engine = GameEngine::new(87_001, &[0, 1], 20, None, true).expect("new engine");
    advance_to_declare_attackers(&mut engine);
    let horror = inject_creature_on_battlefield(&mut engine, 0, "infectious_horror");
    engine.state.add_damage_prevention_shield(1, 2);

    engine
        .apply_command(0, &declare_attackers(vec![horror]))
        .expect("declare Infectious Horror as an attacker");

    assert_eq!(
        engine.state.stack.len(),
        1,
        "attack trigger reaches the stack"
    );
    assert!(
        engine.state.stack[0].targets.is_empty(),
        "each opponent is an untargeted recipient"
    );
    assert!(
        engine.state.pending_triggers.is_empty(),
        "the trigger must not request a target"
    );
    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (20, 20),
        "the trigger has not resolved yet"
    );

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (20, 18),
        "only the opponent loses 2 life"
    );
    assert_eq!(
        engine.state.remaining_damage_prevention(1),
        2,
        "direct life loss neither uses nor consumes damage prevention"
    );
    assert_eq!(engine.state.turn_history.current.player(1).life_lost, 2);
    assert_eq!(engine.state.turn_history.current.player(0).life_lost, 0);
}
