use super::helpers::*;
use tricerules_core::state::Zone;

fn game(card: &str) -> GameEngine {
    let mut e = GameEngine::new(
        20260908,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &[card, "grizzly_bears"]),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .expect("deck supported");
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, card);
    e.state.players[0].mana_pool.red = 10;
    e.state.players[0].mana_pool.blue = 10;
    e
}

#[test]
fn spellementals_damage_spells_reject_lands_and_damage_creatures() {
    for card in [
        "sear",
        "broadside_barrage",
        "traumatic_critique",
        "impractical_joke",
        "burst_lightning",
    ] {
        let mut e = game(card);
        let land = move_ready_to_battlefield(&mut e, 1, "forest");
        let creature = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        let slot = hand_index_for_card(&e, 0, card);
        assert!(
            e.apply_command(0, &cast_spell_x(slot, target_object(land), 2))
                .is_err(),
            "{card}"
        );
        let x = if card == "traumatic_critique" { 2 } else { 0 };
        e.apply_command(0, &cast_spell_x(slot, target_object(creature), x))
            .expect("legal creature");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.pending_resolution.is_some(),
            matches!(card, "broadside_barrage" | "traumatic_critique")
        );
        if e.state.pending_resolution.is_some() {
            let discard = e.state.players[0].hand[0];
            e.apply_command(0, &submit_resolution_choice(vec![discard]))
                .expect("discard");
        }
        assert_eq!(
            count_card_id_in_graveyard(&e, 1, "grizzly_bears"),
            1,
            "{card}"
        );
    }
}

#[test]
fn spellementals_riverpyre_restriction_and_steam_vents_entry_payment() {
    let mut e = game("riverpyre_verge");
    let verge = move_ready_to_battlefield(&mut e, 0, "riverpyre_verge");
    assert!(e
        .apply_command(0, &activate_ability_for(&e, verge, 1, vec![]))
        .is_err());
    move_ready_to_battlefield(&mut e, 0, "island");
    let blue = e.state.players[0].mana_pool.blue;
    e.apply_command(0, &activate_ability_for(&e, verge, 1, vec![]))
        .expect("Island enables blue");
    assert_eq!(e.state.players[0].mana_pool.blue, blue + 1);
    for pay in [false, true] {
        let mut e = game("steam_vents");
        let slot = hand_index_for_card(&e, 0, "steam_vents");
        let object = e.state.players[0].hand[slot];
        e.apply_command(0, &play_land(slot)).expect("entry choice");
        use tricerules_proto::ruled::v1::{ResolutionChoiceDecision, SubmitResolutionChoice};
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: if pay {
                        ResolutionChoiceDecision::SelectBranch
                    } else {
                        ResolutionChoiceDecision::Decline
                    } as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("entry decision");
        assert_eq!(e.state.players[0].life, if pay { 18 } else { 20 });
        assert_eq!(e.state.objects[&object].tapped, !pay);
    }
}

#[test]
fn spellementals_impractical_joke_can_resolve_without_a_target() {
    let mut e = game("impractical_joke");
    let slot = hand_index_for_card(&e, 0, "impractical_joke");
    e.apply_command(0, &cast_spell(slot, vec![]))
        .expect("zero targets");
    pass_both_players(&mut e);
    assert!(!e.state.damage_prevention_prohibitions.is_empty());
}

#[test]
fn spellementals_crab_reduces_generic_cost_and_taps_two_creatures() {
    let mut e = game("eddymurk_crab");
    for _ in 0..5 {
        inject_graveyard_card(&mut e, 0, "opt");
    }
    e.state.players[0].mana_pool.red = 0;
    e.state.players[0].mana_pool.blue = 1;
    let slot = hand_index_for_card(&e, 0, "eddymurk_crab");
    assert!(
        e.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "reduction cannot pay blue pips"
    );
    e.state.players[0].mana_pool.blue = 2;
    let first = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    let second = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(slot, vec![]))
        .expect("five-card reduction");
    pass_both_players(&mut e);
    let crab = battlefield_object_for_card(&e, 0, "eddymurk_crab");
    assert!(!e.state.objects[&crab].tapped);
    let choose = |targets| RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets,
            ..Default::default()
        })),
    };
    let mut targets = target_object(first);
    targets.extend(target_object(second));
    e.apply_command(0, &choose(targets)).expect("two targets");
    pass_both_players(&mut e);
    assert!(e.state.objects[&first].tapped);
    assert!(e.state.objects[&second].tapped);
}

#[test]
fn spellementals_crab_flashes_in_tapped_on_an_opponents_turn() {
    let mut e = game("eddymurk_crab");
    e.state.active_player_idx = 1;
    e.state.priority_idx = 0;
    let slot = hand_index_for_card(&e, 0, "eddymurk_crab");
    e.apply_command(0, &cast_spell(slot, vec![]))
        .expect("flash on opponent's turn");
    pass_both_players(&mut e);
    let crab = battlefield_object_for_card(&e, 0, "eddymurk_crab");
    assert!(e.state.objects[&crab].tapped);
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget::default())),
        },
    )
    .expect("choose zero targets");
    pass_both_players(&mut e);
    assert!(e.state.pending_triggers.is_empty());
}

#[test]
fn spellementals_burst_lightning_kicker_changes_damage() {
    for kicked in [false, true] {
        let mut e = game("burst_lightning");
        let slot = hand_index_for_card(&e, 0, "burst_lightning");
        let selections = if kicked {
            vec![tricerules_proto::ruled::v1::CastCostGroupSelection {
                group_index: 0,
                option_index: 0,
                ..Default::default()
            }]
        } else {
            vec![]
        };
        e.apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, target_player(1), selections),
        )
        .expect("cast");
        pass_both_players(&mut e);
        assert_eq!(e.state.players[1].life, if kicked { 16 } else { 18 });
    }
}

#[test]
fn spellementals_critique_illegal_target_prevents_draw_and_discard() {
    let mut e = game("traumatic_critique");
    let creature = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    let slot = hand_index_for_card(&e, 0, "traumatic_critique");
    e.apply_command(0, &cast_spell_x(slot, target_object(creature), 2))
        .expect("cast");
    let hand = e.state.players[0].hand.clone();
    // Simulate removal while the spell is on the stack; the original target is no longer legal.
    e.state.players[1].battlefield.retain(|id| *id != creature);
    e.state.players[1].graveyard.push(creature);
    e.state.objects.get_mut(&creature).unwrap().zone = Zone::Graveyard;
    pass_both_players(&mut e);
    assert_eq!(e.state.players[0].hand, hand);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn spellementals_fast_and_slow_lands_check_other_lands() {
    for (card, count, tapped) in [
        ("spirebluff_canal", 2, false),
        ("spirebluff_canal", 3, true),
        ("stormcarved_coast", 1, true),
        ("stormcarved_coast", 2, false),
    ] {
        let mut e = game(card);
        for _ in 0..count {
            move_ready_to_battlefield(&mut e, 0, "island");
        }
        let slot = hand_index_for_card(&e, 0, card);
        e.apply_command(0, &play_land(slot)).expect("land play");
        let object = e
            .state
            .objects
            .values()
            .find(|o| o.card_id == card && o.zone == Zone::Battlefield)
            .expect("land entered");
        assert_eq!(object.tapped, tapped, "{card} with {count} other lands");
    }
}
