//! #444 exact Command semantics from the pinned Oracle IDs. Targets are selected through real
//! cast commands; mass effects snapshot current controllers only as their instructions resolve.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, Zone};

fn game(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["coral_merfolk", "elvish_mystic", "grizzly_bears"],
        ),
        deck_with(
            "mountain",
            &["grizzly_bears", "coral_merfolk", "elvish_mystic"],
        ),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn prepare(engine: &mut GameEngine, id: &str) -> usize {
    inject_card_into_hand(engine, 0, id);
    grant_pool(engine, 0);
    hand_index_for_card(engine, 0, id)
}

#[test]
fn issue_444_sygg_grants_lifelink_only_to_chosen_players_current_creatures() {
    let mut engine = game(444_001);
    let ours = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let slot = prepare(&mut engine, "syggs_command");
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_player(1)), (2, target_player(0))]),
        )
        .expect("target two players in distinct printed modes");
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.effective_has_keyword(ours, Keyword::Lifelink));
    assert!(engine.effective_has_keyword(theirs, Keyword::Lifelink));
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before,
        "cast one, draw one"
    );
    assert!(engine.state.stack.is_empty() && engine.state.pending_resolution.is_none());
}

#[test]
fn issue_444_trystan_pumps_and_untaps_only_target_players_creatures() {
    let mut engine = game(444_002);
    let ours = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let destroyed = inject_creature_on_battlefield(&mut engine, 1, "coral_merfolk");
    engine.state.objects.get_mut(&ours).unwrap().tapped = true;
    engine.state.objects.get_mut(&theirs).unwrap().tapped = true;
    let slot = prepare(&mut engine, "trystans_command");
    engine
        .apply_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(2, target_object(destroyed)), (3, target_player(1))],
            ),
        )
        .expect("destroy and targeted team mode");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&destroyed].zone, Zone::Graveyard);
    assert_eq!(
        (
            engine.effective_power(theirs),
            engine.effective_toughness(theirs)
        ),
        (Some(5), Some(5))
    );
    assert!(!engine.state.objects[&theirs].tapped);
    assert_eq!(
        (
            engine.effective_power(ours),
            engine.effective_toughness(ours)
        ),
        (Some(2), Some(2))
    );
    assert!(engine.state.objects[&ours].tapped);
}

#[test]
fn issue_444_player_target_rejects_absent_player_and_wrong_mode_count() {
    let mut engine = game(444_003);
    let slot = prepare(&mut engine, "syggs_command");
    assert!(engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_player(999)), (2, target_player(0))])
        )
        .is_err());
    assert!(engine
        .apply_command(0, &cast_modal_spell(slot, vec![(1, target_player(0))]))
        .is_err());
}

#[test]
fn issue_444_sygg_copy_and_stun_modes_preserve_distinct_targets() {
    let mut engine = game(444_004);
    let merfolk = inject_creature_on_battlefield(&mut engine, 0, "coral_merfolk");
    let victim = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let slot = prepare(&mut engine, "syggs_command");
    engine
        .apply_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(0, target_object(merfolk)), (3, target_object(victim))],
            ),
        )
        .expect("copy own Merfolk and stun another creature");
    resolve_entire_stack_two_player(&mut engine);
    let copies = engine.state.players[0]
        .battlefield
        .iter()
        .filter(|&&id| id != merfolk && engine.state.objects[&id].is_token())
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 1);
    assert_eq!(
        engine.effective_power(copies[0]),
        engine.effective_power(merfolk)
    );
    assert!(engine.state.objects[&victim].tapped);
    assert_eq!(
        engine.state.objects[&victim].counter_count(tricerules_cards::CounterKind::Stun),
        1
    );
}

#[test]
fn issue_444_trystan_copy_and_bounded_return_move_each_selected_permanent_card() {
    let mut engine = game(444_005);
    let elf = inject_creature_on_battlefield(&mut engine, 0, "elvish_mystic");
    let land = inject_graveyard_card(&mut engine, 0, "forest");
    let creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let instant = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    let mut cards = target_object(land);
    cards.extend(target_object(creature));
    let slot = prepare(&mut engine, "trystans_command");
    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(0, target_object(elf)), (1, cards)]),
        )
        .expect("copy Elf and return two permanent cards");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&land].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&instant].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[0]
            .battlefield
            .iter()
            .filter(|&&id| id != elf && engine.state.objects[&id].is_token())
            .count(),
        1
    );
}
