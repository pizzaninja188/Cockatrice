use super::helpers::*;
use tricerules_core::TurnStep;

fn red_mana(engine: &GameEngine, player: usize) -> (u32, u32) {
    let state = &engine.state.players[player];
    (state.mana_pool.red, state.retained_combat_mana.red)
}

/// Issue #427: the generated Fire Sages firebending reminder resolves as a normal triggered
/// ability and its red mana survives the combat phase's step boundaries (CR 702.189, CR 106.4),
/// then empties at the end-of-combat boundary like the hand-authored Firebending cards.
#[test]
fn issue_427_fire_sages_adds_red_mana_on_attack_and_retains_it_until_end_of_combat() {
    let decks = Some(vec![
        deck_with("mountain", &["fire_sages"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(15_201, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    let sages = relocate_to_battlefield(&mut engine, 0, "fire_sages", false);

    engine
        .apply_command(0, &declare_attackers(vec![sages]))
        .expect("declare Fire Sages");
    assert_eq!(engine.state.stack.len(), 1, "firebending uses the stack");
    assert_eq!(red_mana(&engine, 0), (0, 0), "mana waits for resolution");

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(red_mana(&engine, 0), (1, 1), "one red mana is added");

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
    assert_eq!(
        red_mana(&engine, 0),
        (0, 0),
        "firebending mana empties at the end-of-combat boundary"
    );
}

#[test]
fn issue_427_fire_sages_adds_no_mana_when_it_does_not_attack() {
    let decks = Some(vec![
        deck_with("mountain", &["fire_sages"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(15_202, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    let _sages = relocate_to_battlefield(&mut engine, 0, "fire_sages", false);

    engine
        .apply_command(0, &declare_attackers(vec![]))
        .expect("decline to attack");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(red_mana(&engine, 0), (0, 0));
}

/// Azula, On the Hunt triggers both its generated firebending 2 and its Clue trigger on the same
/// declaration; resolving both loses exactly one life, creates one Clue token, and adds two red.
#[test]
fn issue_427_azula_on_the_hunt_triggers_firebending_two_and_the_clue() {
    let decks = Some(vec![
        deck_with("swamp", &["azula,_on_the_hunt"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(15_203, &[0, 1], 20, decks, true).expect("new game");
    advance_to_declare_attackers(&mut engine);
    let azula = relocate_to_battlefield(&mut engine, 0, "azula,_on_the_hunt", false);
    let life_before = engine.state.players[0].life;

    engine
        .apply_command(0, &declare_attackers(vec![azula]))
        .expect("declare Azula");
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 2, "both attack triggers stack");

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].life,
        life_before - 1,
        "Azula's controller loses exactly one life"
    );
    assert_eq!(
        battlefield_token_oids(&engine, 0, "clue").len(),
        1,
        "one Clue token is created"
    );
    assert_eq!(red_mana(&engine, 0), (2, 2), "firebending 2 adds two red");
}

/// Zhao, Ruthless Admiral observes a sacrifice paid for another spell and pumps the team +1/+0
/// through the generated sacrifice-trigger recipe.
#[test]
fn issue_427_zhao_pumps_the_team_after_a_sacrifice() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &["zhao,_ruthless_admiral", "village_rites", "grizzly_bears"],
        ),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(15_204, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let zhao = relocate_to_battlefield(&mut engine, 0, "zhao,_ruthless_admiral", false);
    let sacrificed = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "village_rites");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );

    let rites = hand_index_for_card(&engine, 0, "village_rites");
    engine
        .apply_command(
            0,
            &cast_spell_with_costs(rites, vec![], vec![permanent_cost_selection(0, sacrificed)]),
        )
        .expect("cast Village Rites sacrificing the bear");
    assert_eq!(
        engine.effective_power(zhao),
        Some(3),
        "the pump waits for the trigger to resolve"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(zhao),
        Some(4),
        "Zhao's team pump adds +1/+0 after the sacrifice"
    );
    assert_eq!(engine.effective_toughness(zhao), Some(4));
}
