use crate::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_proto::ruled::v1::TargetRef;

fn engine_with_battalion_and_others(other_attackers: usize) -> (GameEngine, u32, Vec<u32>) {
    let decks = Some(vec![
        {
            let mut deck = vec!["unsummon".into()];
            deck.extend(std::iter::repeat_n("plains".into(), 29));
            deck
        },
        vec!["forest".into(); 30],
    ]);
    let mut engine = GameEngine::new(68, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);

    let battalion = inject_creature_on_battlefield(&mut engine, 0, "makeshift_battalion");
    let mut attackers = vec![battalion];
    attackers.extend(
        (0..other_attackers)
            .map(|_| inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears")),
    );

    engine
        .apply_command(0, &primitive_yield())
        .expect("move to beginning of combat");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    (engine, battalion, attackers)
}

fn plus_one_counters(engine: &GameEngine, object_id: u32) -> u32 {
    engine.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne)
}

#[test]
fn makeshift_battalion_does_not_trigger_with_fewer_than_two_other_attackers() {
    for other_attackers in 0..2 {
        let (mut engine, _battalion, attackers) = engine_with_battalion_and_others(other_attackers);
        engine
            .apply_command(0, &declare_attackers(attackers))
            .expect("declare attackers");

        assert!(
            engine.state.stack.is_empty(),
            "Battalion must not trigger with only {other_attackers} other attacker(s)"
        );
    }
}

#[test]
fn makeshift_battalion_triggers_once_at_or_above_the_threshold() {
    for other_attackers in [2, 3] {
        let (mut engine, battalion, attackers) = engine_with_battalion_and_others(other_attackers);
        engine
            .apply_command(0, &declare_attackers(attackers))
            .expect("declare attackers");

        assert_eq!(
            engine.state.stack.len(),
            1,
            "one declaration event creates exactly one Battalion trigger"
        );
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(plus_one_counters(&engine, battalion), 1);
    }
}

#[test]
fn makeshift_battalion_must_itself_be_declared_as_an_attacker() {
    let (mut engine, _battalion, attackers) = engine_with_battalion_and_others(3);
    engine
        .apply_command(0, &declare_attackers(attackers[1..].to_vec()))
        .expect("declare only the other creatures");

    assert!(engine.state.stack.is_empty());
}

#[test]
fn each_battalion_counts_the_other_declared_attackers_relative_to_itself() {
    let (mut engine, first_battalion, mut attackers) = engine_with_battalion_and_others(1);
    let second_battalion = inject_creature_on_battlefield(&mut engine, 0, "makeshift_battalion");
    attackers.insert(1, second_battalion);

    engine
        .apply_command(0, &declare_attackers(attackers))
        .expect("declare two Battalions and one other creature");

    assert_eq!(
        engine
            .state
            .pending_trigger_order
            .as_ref()
            .expect("controller orders simultaneous Battalion triggers")
            .candidates
            .len(),
        2
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(plus_one_counters(&engine, first_battalion), 1);
    assert_eq!(plus_one_counters(&engine, second_battalion), 1);
}

#[test]
fn attack_group_threshold_is_not_rechecked_when_the_trigger_resolves() {
    let (mut engine, battalion, attackers) = engine_with_battalion_and_others(2);
    let departing_attacker = attackers[1];
    engine
        .apply_command(0, &declare_attackers(attackers))
        .expect("declare Battalion and two other attackers");
    assert_eq!(engine.state.stack.len(), 1, "Battalion triggered");

    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(
            0,
            &cast_spell(
                unsummon,
                vec![TargetRef {
                    object_id: departing_attacker,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Unsummon above the Battalion trigger");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.stack.len(),
        1,
        "the already-created Battalion trigger remains"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(plus_one_counters(&engine, battalion), 1);
}

#[test]
fn ordinary_self_attack_triggers_use_a_zero_other_attacker_threshold() {
    let (mut engine, _battalion, _attackers) = engine_with_battalion_and_others(0);
    let thief = inject_creature_on_battlefield(&mut engine, 0, "audacious_thief");
    engine
        .apply_command(0, &declare_attackers(vec![thief]))
        .expect("attack with Audacious Thief alone");

    assert_eq!(engine.state.stack.len(), 1);
}
