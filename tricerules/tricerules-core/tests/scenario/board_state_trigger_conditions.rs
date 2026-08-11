//! Issue #67: intervening-if trigger conditions derived from live battlefield state.

use super::helpers::*;
use tricerules_core::{GameEngine, TurnStep, Zone};

fn remove_from_battlefield(e: &mut GameEngine, player: usize, oid: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != oid);
    e.state.players[player].graveyard.push(oid);
    e.state.objects.get_mut(&oid).expect("object").zone = Zone::Graveyard;
}

fn resolve_stack(e: &mut GameEngine) {
    while !e.state.stack.is_empty() {
        answer_trigger_order_in_engine_order(e);
        let player = e.state.priority_player_id();
        e.apply_command(player, &pass()).expect("priority pass");
    }
}

#[test]
fn issue_67_scholar_does_not_draw_when_the_last_artifact_leaves_before_resolution() {
    let decks = Some(vec![
        deck_with("island", &["scholar_of_stars", "explosive_apparatus"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6701, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let artifact = relocate_to_battlefield(&mut e, 0, "explosive_apparatus", false);
    ensure_in_hand(&mut e, 0, "scholar_of_stars");
    grant_pool(&mut e, 0);
    let scholar_index = hand_index_for_card(&e, 0, "scholar_of_stars");
    e.apply_command(0, &cast_spell(scholar_index, vec![]))
        .expect("cast Scholar of Stars");
    pass_both_players(&mut e);

    assert_eq!(
        e.state.stack.len(),
        1,
        "the true condition creates the trigger"
    );
    let hand_before = e.state.players[0].hand.len();
    remove_from_battlefield(&mut e, 0, artifact);
    pass_both_players(&mut e);

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "CR 603.4 rechecks the condition and suppresses the draw"
    );
}

#[test]
fn issue_67_scholar_never_triggers_without_an_artifact() {
    let decks = Some(vec![
        deck_with("island", &["scholar_of_stars"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6706, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "scholar_of_stars");
    grant_pool(&mut e, 0);
    let scholar_index = hand_index_for_card(&e, 0, "scholar_of_stars");
    e.apply_command(0, &cast_spell(scholar_index, vec![]))
        .expect("cast Scholar of Stars");
    pass_both_players(&mut e);

    assert!(
        e.state.stack.is_empty(),
        "a false intervening-if condition never triggers"
    );
}

#[test]
fn issue_67_scholar_may_use_a_different_artifact_at_resolution() {
    let decks = Some(vec![
        deck_with(
            "island",
            &["scholar_of_stars", "explosive_apparatus", "bonesplitter"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6702, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let original = relocate_to_battlefield(&mut e, 0, "explosive_apparatus", false);
    ensure_in_hand(&mut e, 0, "scholar_of_stars");
    grant_pool(&mut e, 0);
    let scholar_index = hand_index_for_card(&e, 0, "scholar_of_stars");
    e.apply_command(0, &cast_spell(scholar_index, vec![]))
        .expect("cast Scholar of Stars");
    pass_both_players(&mut e);

    let hand_before = e.state.players[0].hand.len();
    remove_from_battlefield(&mut e, 0, original);
    relocate_to_battlefield(&mut e, 0, "bonesplitter", false);
    pass_both_players(&mut e);

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the artifact need not be the same object at both CR 603.4 checks"
    );
}

#[test]
fn issue_67_ornery_dilophosaur_rechecks_derived_power_on_resolution() {
    let decks = Some(vec![
        deck_with("forest", &["ornery_dilophosaur"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(6703, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let dinosaur = relocate_to_battlefield(&mut e, 0, "ornery_dilophosaur", false);
    e.state.objects.get_mut(&dinosaur).expect("dinosaur").power = Some(4);
    e.apply_command(0, &primitive_yield())
        .expect("move to beginning of combat");
    pass_both_players(&mut e);
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
    e.apply_command(0, &declare_attackers(vec![dinosaur]))
        .expect("declare Ornery Dilophosaur");

    assert_eq!(
        e.state.stack.len(),
        1,
        "its own derived power may satisfy the condition"
    );
    e.state.objects.get_mut(&dinosaur).expect("dinosaur").power = Some(2);
    pass_both_players(&mut e);
    assert_eq!(e.effective_power(dinosaur), Some(2));
    assert_eq!(e.effective_toughness(dinosaur), Some(2));
}

#[test]
fn issue_67_ornery_dilophosaur_locks_its_bonus_after_resolution() {
    let decks = Some(vec![
        deck_with("forest", &["ornery_dilophosaur", "rumbling_baloth"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(6707, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let dinosaur = relocate_to_battlefield(&mut e, 0, "ornery_dilophosaur", false);
    let qualifier = relocate_to_battlefield(&mut e, 0, "rumbling_baloth", false);
    e.apply_command(0, &primitive_yield())
        .expect("move to beginning of combat");
    pass_both_players(&mut e);
    e.apply_command(0, &declare_attackers(vec![dinosaur]))
        .expect("declare Ornery Dilophosaur");
    pass_both_players(&mut e);
    assert_eq!(e.effective_power(dinosaur), Some(4));
    assert_eq!(e.effective_toughness(dinosaur), Some(4));

    remove_from_battlefield(&mut e, 0, qualifier);
    assert_eq!(e.effective_power(dinosaur), Some(4));
    assert_eq!(e.effective_toughness(dinosaur), Some(4));
}

#[test]
fn issue_67_faerie_miscreant_requires_another_named_creature_at_resolution() {
    let mut p0 = vec![
        "faerie_miscreant".to_string(),
        "faerie_miscreant".to_string(),
    ];
    while p0.len() < 20 {
        p0.push("island".to_string());
    }
    let decks = Some(vec![p0, deck_with("forest", &[])]);
    let mut e = GameEngine::new(6704, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let first = relocate_to_battlefield(&mut e, 0, "faerie_miscreant", false);
    ensure_in_hand(&mut e, 0, "faerie_miscreant");
    grant_pool(&mut e, 0);
    let second_index = hand_index_for_card(&e, 0, "faerie_miscreant");
    e.apply_command(0, &cast_spell(second_index, vec![]))
        .expect("cast the second Faerie Miscreant");
    pass_both_players(&mut e);

    assert_eq!(e.state.stack.len(), 1);
    let hand_before = e.state.players[0].hand.len();
    remove_from_battlefield(&mut e, 0, first);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before);
}

#[test]
fn issue_67_faerie_miscreant_counts_a_permanent_with_copied_name() {
    let decks = Some(vec![
        deck_with("island", &["clone", "faerie_miscreant"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(6708, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let faerie = relocate_to_battlefield(&mut e, 0, "faerie_miscreant", false);
    ensure_in_hand(&mut e, 0, "clone");
    grant_pool(&mut e, 0);
    let clone_index = hand_index_for_card(&e, 0, "clone");
    e.apply_command(0, &cast_spell(clone_index, vec![]))
        .expect("cast Clone");
    pass_both_players(&mut e);
    e.apply_command(0, &submit_resolution_choice(vec![faerie]))
        .expect("copy Faerie Miscreant");

    assert_eq!(
        e.state.stack.len(),
        1,
        "the copied ETB ability sees the original Faerie"
    );
    let hand_before = e.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
}

#[test]
fn issue_67_turret_ogre_damages_the_opponent_once() {
    let decks = Some(vec![
        deck_with("mountain", &["turret_ogre", "rumbling_baloth"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(6705, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "rumbling_baloth", false);
    ensure_in_hand(&mut e, 0, "turret_ogre");
    grant_pool(&mut e, 0);
    let ogre_index = hand_index_for_card(&e, 0, "turret_ogre");
    e.apply_command(0, &cast_spell(ogre_index, vec![]))
        .expect("cast Turret Ogre");
    resolve_stack(&mut e);

    assert_eq!(e.state.turn_step, TurnStep::Main1);
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 18);
}

#[test]
fn issue_67_turret_ogre_does_not_count_itself() {
    let decks = Some(vec![
        deck_with("mountain", &["turret_ogre"]),
        deck_with("island", &[]),
    ]);
    let mut e = GameEngine::new(6709, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "turret_ogre");
    grant_pool(&mut e, 0);
    let ogre_index = hand_index_for_card(&e, 0, "turret_ogre");
    e.apply_command(0, &cast_spell(ogre_index, vec![]))
        .expect("cast Turret Ogre");
    pass_both_players(&mut e);

    assert!(
        e.state.stack.is_empty(),
        "the 4-power source is excluded by 'another'"
    );
    assert_eq!(e.state.players[1].life, 20);
}
