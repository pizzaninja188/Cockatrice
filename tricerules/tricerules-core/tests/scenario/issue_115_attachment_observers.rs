//! Issue #115: attachment damage observers and conditional attached modifiers.
//!
//! Oracle and rulings verified 2026-08-20 for Goldvein Pick, Cracked Skull, and
//! Quick-Draw Katana. Governing rules: CR 111.10a, 113.7a, 113.8, 120.4,
//! 301.5, 303.4, 510.2-3a, 603.2-3, 611.3, 613.1f-g, and 702.6.

use crate::helpers::*;
use tricerules_cards::Keyword;

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

#[test]
fn issue_115_goldvein_pick_observes_equipped_creature_combat_damage() {
    let decks = Some(vec![
        deck_with("mountain", &["goldvein_pick"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11501, &[0, 1], 20, decks, true).expect("engine");
    advance_to_declare_attackers(&mut engine);

    let attacker = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "grizzly_bears")
        .expect("eligible attacker");
    let pick = relocate_to_battlefield(&mut engine, 0, "goldvein_pick", false);
    engine
        .state
        .objects
        .get_mut(&pick)
        .expect("Goldvein Pick")
        .attached_to = Some(AttachmentRecipient::Object(attacker));

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare equipped attacker");
    engine.apply_command(0, &pass()).expect("attacker pass");
    engine.apply_command(1, &pass()).expect("defender pass");
    engine
        .apply_command(0, &pass())
        .expect("attacker blockers pass");
    engine
        .apply_command(1, &pass())
        .expect("defender blockers pass and combat damage");

    assert_eq!(
        engine.state.stack.len(),
        1,
        "Pick trigger reaches the stack"
    );
    resolve_entire_stack_two_player(&mut engine);

    let treasures: Vec<_> = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| engine.state.objects[object_id].card_id == "treasure")
        .collect();
    assert_eq!(
        treasures.len(),
        1,
        "one combat-damage event creates one Treasure"
    );

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id: treasures[0],
                    expected_zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(&treasures[0])
                        .copied()
                        .unwrap_or(0),
                    ability_index: 0,
                    mana_option_index: 2,
                    ..Default::default()
                })),
            },
        )
        .expect("tap and sacrifice Treasure for black mana");
    assert_eq!(engine.state.players[0].mana_pool.black, 1);
    assert!(!engine.state.objects.contains_key(&treasures[0]));
}

#[test]
fn issue_115_goldvein_pick_ignores_equipped_creature_noncombat_damage() {
    let decks = Some(vec![
        deck_with("island", &["goldvein_pick", "prodigal_sorcerer"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11506, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let pick = relocate_to_battlefield(&mut engine, 0, "goldvein_pick", false);
    let sorcerer = relocate_to_battlefield(&mut engine, 0, "prodigal_sorcerer", false);
    engine
        .state
        .objects
        .get_mut(&pick)
        .expect("Goldvein Pick")
        .attached_to = Some(AttachmentRecipient::Object(sorcerer));
    engine
        .apply_command(0, &activate_ability(sorcerer, 0, target_player(1)))
        .expect("activate Prodigal Sorcerer");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[1].life, 19);
    assert!(
        engine.state.players[0]
            .battlefield
            .iter()
            .all(|object_id| engine.state.objects[object_id].card_id != "treasure"),
        "noncombat damage does not satisfy Goldvein Pick"
    );
}

#[test]
fn issue_115_cracked_skull_destroys_the_damaged_same_generation_creature() {
    let decks = Some(vec![
        deck_with("mountain", &["cracked_skull", "lightning_bolt"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11502, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let skull = relocate_to_battlefield(&mut engine, 0, "cracked_skull", false);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "colossal_dreadmaw");
    engine
        .state
        .objects
        .get_mut(&skull)
        .expect("Cracked Skull")
        .attached_to = Some(AttachmentRecipient::Object(creature));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    ensure_card_in_hand(&mut engine, 0, "lightning_bolt");
    let shock_index = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(shock_index, target_object(creature)))
        .expect("cast Shock");
    engine.apply_command(0, &pass()).expect("caster pass");
    engine
        .apply_command(1, &pass())
        .expect("opponent pass and Shock resolves");

    assert_eq!(
        engine.state.stack.len(),
        1,
        "Skull trigger reaches the stack"
    );
    engine
        .state
        .objects
        .get_mut(&skull)
        .expect("Cracked Skull")
        .attached_to = None;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&creature].zone,
        tricerules_core::Zone::Graveyard,
        "the independent trigger destroys the creature observed at damage time"
    );
}

#[test]
fn issue_115_cracked_skull_does_not_destroy_a_returned_new_object() {
    let decks = Some(vec![
        deck_with("mountain", &["cracked_skull", "lightning_bolt"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11507, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let skull = relocate_to_battlefield(&mut engine, 0, "cracked_skull", false);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "colossal_dreadmaw");
    engine
        .state
        .objects
        .get_mut(&skull)
        .expect("Cracked Skull")
        .attached_to = Some(AttachmentRecipient::Object(creature));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    ensure_card_in_hand(&mut engine, 0, "lightning_bolt");
    let bolt_index = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(0, &cast_spell(bolt_index, target_object(creature)))
        .expect("cast Lightning Bolt");
    engine.apply_command(0, &pass()).expect("caster pass");
    engine.apply_command(1, &pass()).expect("Bolt resolves");
    assert_eq!(engine.state.stack.len(), 1);

    engine
        .state
        .objects
        .get_mut(&skull)
        .expect("Cracked Skull")
        .attached_to = None;
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != creature);
    engine.state.players[0].graveyard.push(creature);
    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("creature")
        .zone = tricerules_core::Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(creature)
        .or_default() += 1;
    engine.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != creature);
    engine.state.players[0].battlefield.push(creature);
    let returned = engine
        .state
        .objects
        .get_mut(&creature)
        .expect("returned creature");
    returned.zone = tricerules_core::Zone::Battlefield;
    returned.damage = 0;
    *engine
        .state
        .zone_change_generation
        .entry(creature)
        .or_default() += 1;

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&creature].zone,
        tricerules_core::Zone::Battlefield,
        "the returned permanent is not the trigger's observed object"
    );
}

#[test]
fn issue_115_quick_draw_katana_tracks_its_controllers_turn() {
    let decks = Some(vec![
        deck_with("plains", &["quick-draw_katana"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11503, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    ensure_card_in_hand(&mut engine, 0, "quick-draw_katana");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let katana_index = hand_index_for_card(&engine, 0, "quick-draw_katana");
    engine
        .apply_command(0, &cast_spell(katana_index, vec![]))
        .expect("cast Quick-Draw Katana");
    resolve_entire_stack_two_player(&mut engine);

    let katana = battlefield_object_for_card(&engine, 0, "quick-draw_katana");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&katana)
        .expect("Katana")
        .attached_to = Some(AttachmentRecipient::Object(creature));

    assert_eq!(engine.effective_power(creature), Some(4));
    assert!(engine.effective_has_keyword(creature, Keyword::FirstStrike));

    engine.state.active_player_idx = 1;
    assert_eq!(engine.effective_power(creature), Some(2));
    assert!(!engine.effective_has_keyword(creature, Keyword::FirstStrike));
}

#[test]
fn issue_115_nonland_discard_publishes_full_hand_but_validates_eligible_subset() {
    let decks = Some(vec![
        deck_with("swamp", &["coercion"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11504, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let cleared: Vec<_> = engine.state.players[1].hand.drain(..).collect();
    engine.state.players[1].library.extend(cleared);
    let land = inject_card_into_hand(&mut engine, 1, "forest");
    let creature = inject_card_into_hand(&mut engine, 1, "grizzly_bears");

    relocate_to_hand(&mut engine, 0, "coercion");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let coercion_index = hand_index_for_card(&engine, 0, "coercion");
    engine
        .apply_command(0, &cast_spell(coercion_index, target_player(1)))
        .expect("cast Coercion");
    engine.apply_command(0, &pass()).expect("caster pass");
    let batch = engine
        .apply_command(1, &pass())
        .expect("opponent pass and Coercion parks");

    let choice = find_resolution_choice(&batch).expect("opponent-hand choice");
    assert_eq!(choice.candidate_object_ids, [land, creature]);
    assert_eq!(choice.candidate_selectable, [false, true]);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("pending discard")
            .presentation
            .candidates,
        [creature],
        "only nonlands are accepted by authoritative submission validation"
    );

    assert!(
        engine
            .apply_command(0, &submit_resolution_choice(vec![land]))
            .is_err(),
        "a displayed land cannot be selected"
    );
    assert!(engine.state.players[1].hand.contains(&land));
    engine
        .apply_command(0, &submit_resolution_choice(vec![creature]))
        .expect("choose nonland");
    assert_eq!(
        engine.state.objects[&creature].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn issue_115_cracked_skull_may_decline_after_privately_looking_at_the_hand() {
    let decks = Some(vec![
        deck_with("swamp", &["cracked_skull"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(11505, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);

    let cleared: Vec<_> = engine.state.players[1].hand.drain(..).collect();
    engine.state.players[1].library.extend(cleared);
    let land = inject_card_into_hand(&mut engine, 1, "forest");
    let nonland = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    let enchanted = inject_creature_on_battlefield(&mut engine, 1, "colossal_dreadmaw");

    ensure_card_in_hand(&mut engine, 0, "cracked_skull");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let skull_index = hand_index_for_card(&engine, 0, "cracked_skull");
    engine
        .apply_command(0, &cast_spell(skull_index, target_object(enchanted)))
        .expect("cast Cracked Skull");
    engine.apply_command(0, &pass()).expect("caster pass");
    engine.apply_command(1, &pass()).expect("Aura resolves");
    engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("target opponent with the ETB trigger");
    engine
        .apply_command(0, &pass())
        .expect("trigger controller pass");
    let batch = engine
        .apply_command(1, &pass())
        .expect("trigger resolves and parks for the optional choice");

    let choice = find_resolution_choice(&batch).expect("private hand choice");
    assert_eq!(choice.candidate_object_ids, [land, nonland]);
    assert_eq!(choice.candidate_selectable, [false, true]);
    assert_eq!((choice.min, choice.max), (0, 1));
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("decline after looking");
    assert!(engine.state.players[1].hand.contains(&land));
    assert!(engine.state.players[1].hand.contains(&nonland));
}
