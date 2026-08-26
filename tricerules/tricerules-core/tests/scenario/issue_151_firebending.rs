use super::helpers::*;
use tricerules_cards::primitives::{ActivationLimit, ContinuousEffectKind, EffectDuration};
use tricerules_cards::CardRegistry;
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep};

fn red_mana(engine: &GameEngine, player: usize) -> (u32, u32) {
    let state = &engine.state.players[player];
    (state.mana_pool.red, state.retained_combat_mana.red)
}

fn enter_declare_attackers_from_main1(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase yields to beginning of combat");
    pass_both_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

#[test]
fn firebending_uses_the_stack_captures_controller_and_expires_after_end_of_combat() {
    let decks = Some(vec![
        deck_with("mountain", &["vindictive_warden"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(15_101, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    let warden = relocate_to_battlefield(&mut engine, 0, "vindictive_warden", false);

    engine
        .apply_command(0, &declare_attackers(vec![warden]))
        .expect("declare the firebender");
    assert_eq!(engine.state.stack.len(), 1, "firebending uses the stack");
    assert_eq!(red_mana(&engine, 0), (0, 0), "mana waits for resolution");

    engine
        .state
        .objects
        .get_mut(&warden)
        .expect("warden")
        .controller = 1;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        red_mana(&engine, 0),
        (1, 1),
        "the captured trigger controller gets the mana"
    );
    assert_eq!(red_mana(&engine, 1), (0, 0));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    assert_eq!(red_mana(&engine, 0), (2, 1));
    while engine.state.turn_step != TurnStep::EndCombat {
        pass_both_players(&mut engine);
        assert_eq!(
            red_mana(&engine, 0),
            (1, 1),
            "ordinary mana empties while firebending mana crosses combat steps"
        );
    }
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main2);
    assert_eq!(red_mana(&engine, 0), (0, 0));
}

#[test]
fn fire_nation_cadets_has_firebending_only_when_a_lesson_is_in_its_graveyard() {
    fn attack_with_lesson(seed: u64, has_lesson: bool) -> usize {
        let mut engine = anthem_engine(seed, "fire_nation_cadets");
        if has_lesson {
            inject_graveyard_card(&mut engine, 0, "abandon_attachments");
        }
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let hand_index = hand_index_for_card(&engine, 0, "fire_nation_cadets");
        engine
            .apply_command(0, &cast_spell(hand_index, vec![]))
            .expect("cast Cadets");
        resolve_entire_stack_two_player(&mut engine);
        let cadets = battlefield_object_for_card(&engine, 0, "fire_nation_cadets");
        engine
            .state
            .objects
            .get_mut(&cadets)
            .expect("Cadets")
            .summoning_sick = false;
        enter_declare_attackers_from_main1(&mut engine);
        engine
            .apply_command(0, &declare_attackers(vec![cadets]))
            .expect("declare Cadets");
        engine.state.stack.len()
    }

    assert_eq!(attack_with_lesson(15_102, false), 0);
    assert_eq!(attack_with_lesson(15_103, true), 1);
}

#[test]
fn separately_granted_firebending_instances_trigger_and_resolve_independently() {
    let mut engine = GameEngine::new(15_104, &[0, 1], 20, None, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    let warden = inject_creature_on_battlefield(&mut engine, 0, "vindictive_warden");
    let firebending = CardRegistry::global()
        .get("vindictive_warden")
        .expect("Warden definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(warden),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(firebending)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    engine
        .apply_command(0, &declare_attackers(vec![warden]))
        .expect("declare Warden");
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 2);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(red_mana(&engine, 0), (2, 2));
}

#[test]
fn a_creature_put_onto_the_battlefield_attacking_does_not_fire_its_granted_firebending() {
    let decks = Some(vec![
        deck_with("plains", &["dragonback_lancer"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(15_105, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    let lancer = relocate_to_battlefield(&mut engine, 0, "dragonback_lancer", false);
    engine
        .apply_command(0, &declare_attackers(vec![lancer]))
        .expect("declare the mobilize attacker");

    let firebending = CardRegistry::global()
        .get("vindictive_warden")
        .expect("Warden definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::AllCreatures,
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(firebending)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(battlefield_token_oids(&engine, 0, "warrior_r_1_1").len(), 1);
    assert_eq!(red_mana(&engine, 0), (0, 0));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn same_seed_and_commands_replay_the_same_firebending_state() {
    fn replay() -> (u32, u32, usize, u64) {
        let decks = Some(vec![
            deck_with("mountain", &["vindictive_warden"]),
            deck_with("island", &[]),
        ]);
        let mut engine = GameEngine::new(15_106, &[0, 1], 20, decks, true).expect("new game");
        advance_to_declare_attackers(&mut engine);
        let warden = relocate_to_battlefield(&mut engine, 0, "vindictive_warden", false);
        engine
            .apply_command(0, &declare_attackers(vec![warden]))
            .expect("declare Warden");
        resolve_entire_stack_two_player(&mut engine);
        let mana = red_mana(&engine, 0);
        (
            mana.0,
            mana.1,
            engine.state.stack.len(),
            engine.state.command_index,
        )
    }

    assert_eq!(replay(), replay());
}

#[test]
fn rough_rhino_cavalry_authors_firebending_and_a_per_object_exhaust_limit() {
    let face = CardRegistry::global()
        .get("rough_rhino_cavalry")
        .expect("Rough Rhino Cavalry definition")
        .primary_face();
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(
        face.activated_abilities[0].activation_limit,
        Some(ActivationLimit::PerObject { max_activations: 1 })
    );
    assert_eq!(face.activated_abilities[0].effect.len(), 2);
}
