//! Issue #86: related-player effect recipients.
//!
//! Oracle/rulings verified 2026-08-14 for Chandra's Outrage, Scorch Spitter, Curse of Opulence,
//! and Curse of Disturbance. Governing rules: CR 111.2, 111.10c, 113.7a, 508.3a-b, 605.1a,
//! and 608.2b/h. Scorch Spitter's planeswalker recipient remains deferred to issue #72.

use crate::helpers::*;
use tricerules_core::{AttachmentRecipient, Zone};

#[test]
fn chandras_outrage_damages_the_creature_and_its_controller() {
    let decks = Some(vec![
        deck_with("mountain", &["chandras_outrage"]),
        deck_with("forest", &["colossal_dreadmaw"]),
    ]);
    let mut engine = GameEngine::new(86_100, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    ensure_in_hand(&mut engine, 0, "chandras_outrage");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 4,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "chandras_outrage");
    engine
        .apply_command(0, &cast_spell(spell, target_object(target)))
        .expect("cast Chandra's Outrage");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].damage, 4);
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn chandras_outrage_uses_the_targets_current_controller() {
    let decks = Some(vec![
        deck_with("mountain", &["chandras_outrage"]),
        deck_with("forest", &["colossal_dreadmaw"]),
    ]);
    let mut engine = GameEngine::new(86_105, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    ensure_in_hand(&mut engine, 0, "chandras_outrage");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 4,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "chandras_outrage");
    engine
        .apply_command(0, &cast_spell(spell, target_object(target)))
        .expect("cast Chandra's Outrage");

    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[0].battlefield.push(target);
    let object = engine.state.objects.get_mut(&target).expect("target");
    object.base_controller = 0;
    object.controller = 0;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn chandras_outrage_does_no_player_damage_when_its_target_is_illegal() {
    let decks = Some(vec![
        deck_with("mountain", &["chandras_outrage"]),
        deck_with("forest", &["colossal_dreadmaw"]),
    ]);
    let mut engine = GameEngine::new(86_106, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    ensure_in_hand(&mut engine, 0, "chandras_outrage");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 4,
            ..Default::default()
        },
    );
    let spell = hand_index_for_card(&engine, 0, "chandras_outrage");
    engine
        .apply_command(0, &cast_spell(spell, target_object(target)))
        .expect("cast Chandra's Outrage");

    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[1].hand.push(target);
    engine.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn scorch_spitter_keeps_the_event_time_defender_after_its_source_leaves() {
    let decks = Some(vec![
        deck_with("mountain", &["scorch_spitter"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(86_101, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut engine);
    let spitter = relocate_to_battlefield(&mut engine, 0, "scorch_spitter", false);

    engine
        .apply_command(0, &declare_attackers(vec![spitter]))
        .expect("attack with Scorch Spitter");
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != spitter);
    engine.state.players[0].graveyard.push(spitter);
    engine
        .state
        .objects
        .get_mut(&spitter)
        .expect("Spitter")
        .zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(spitter)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[1].life, 19);
}

fn attach_curse_to_player(engine: &mut GameEngine, card_id: &str, player_id: i32) -> u32 {
    let curse = relocate_to_battlefield(engine, 0, card_id, false);
    engine
        .state
        .objects
        .get_mut(&curse)
        .expect("Curse")
        .attached_to = Some(AttachmentRecipient::Player(player_id));
    curse
}

#[test]
fn curse_of_opulence_triggers_once_for_multiple_attackers_and_gives_both_rewards() {
    let decks = Some(vec![
        deck_with("mountain", &["curse_of_opulence"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(86_102, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut engine);
    attach_curse_to_player(&mut engine, "curse_of_opulence", 1);
    let mut attackers = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .collect::<Vec<_>>();
    attackers.push(inject_creature_on_battlefield(
        &mut engine,
        0,
        "grizzly_bears",
    ));

    engine
        .apply_command(0, &declare_attackers(attackers))
        .expect("attack cursed player with two creatures");
    assert_eq!(engine.state.stack.len(), 1, "one attacked-player trigger");
    resolve_entire_stack_two_player(&mut engine);

    let gold = battlefield_token_oids(&engine, 0, "gold");
    assert_eq!(
        gold.len(),
        2,
        "the Curse controller and the attacking opponent each create one Gold"
    );
    let characteristics = engine
        .characteristics(gold[0])
        .expect("Gold characteristics");
    assert!(characteristics.is_artifact());
    assert!(characteristics.has_type("Gold"));
    assert!(characteristics.colors.is_empty());
    assert_eq!(engine.state.objects[&gold[0]].owner, 0);

    let make_black = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            permanent_id: gold[0],
            ability_index: 0,
            mana_option_index: 2,
            ..Default::default()
        })),
    };
    engine
        .apply_command(0, &make_black)
        .expect("sacrifice Gold for black mana");
    assert_eq!(engine.state.players[0].mana_pool.black, 1);
    assert!(
        !engine.state.objects.contains_key(&gold[0]),
        "the token ceases to exist"
    );
}

#[test]
fn curse_attacking_reward_is_skipped_when_no_attacker_remains() {
    let decks = Some(vec![
        deck_with("mountain", &["curse_of_opulence"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(86_103, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut engine);
    attach_curse_to_player(&mut engine, "curse_of_opulence", 1);
    let attacker = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .expect("attacker");

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("attack cursed player");
    engine
        .state
        .combat
        .as_mut()
        .expect("combat")
        .attacking
        .clear();
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        battlefield_token_oids(&engine, 0, "gold").len(),
        1,
        "only the Curse controller creates a Gold"
    );
}

#[test]
fn curse_of_disturbance_uses_the_same_two_recipient_rewards() {
    let decks = Some(vec![
        deck_with("swamp", &["curse_of_disturbance"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(86_104, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut engine);
    attach_curse_to_player(&mut engine, "curse_of_disturbance", 1);
    let attacker = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .expect("attacker");

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("attack cursed player");
    resolve_entire_stack_two_player(&mut engine);

    let zombies = battlefield_token_oids(&engine, 0, "zombie_b_2_2");
    assert_eq!(zombies.len(), 2);
    let characteristics = engine
        .characteristics(zombies[0])
        .expect("Zombie characteristics");
    assert!(characteristics.is_creature());
    assert_eq!(characteristics.power, Some(2));
    assert_eq!(characteristics.toughness, Some(2));
    assert!(engine.state.objects[&zombies[0]].summoning_sick);
    assert_eq!(engine.state.objects[&zombies[0]].owner, 0);
}
