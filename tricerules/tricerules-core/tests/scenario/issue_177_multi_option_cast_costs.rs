use crate::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, CastCostGroupSelection, CastMethod, CastSpell, ChoiceKind,
    SelectedSpellMode,
};

fn mana_option(option_index: u32) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        selected_object: None,
        expected_zone_change_generation: 0,
    }
}

#[test]
fn phantom_interference_casts_both_spree_modes_with_one_atomic_total() {
    let decks = Some(vec![
        deck_with("island", &["phantom_interference"]),
        deck_with("mountain", &["lightning_bolt"]),
    ]);
    let mut e = GameEngine::new(177_001, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "phantom_interference");
    ensure_card_in_hand(&mut e, 1, "lightning_bolt");

    e.state.priority_idx = 1;
    e.state.players[1].mana_pool.red = 1;
    let bolt_slot = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(bolt_slot, target_player(0)))
        .expect("opponent spell supplies the Spree target");
    let bolt = e.state.stack.last().unwrap().id;

    e.state.players[0].mana_pool.blue = 1;
    e.state.players[0].mana_pool.colorless = 4;
    let phantom_slot = hand_index_for_card(&e, 0, "phantom_interference");
    let command = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(phantom_slot)),
            selected_modes: vec![
                SelectedSpellMode {
                    mode_index: 0,
                    targets: vec![],
                },
                SelectedSpellMode {
                    mode_index: 1,
                    targets: target_object(bolt),
                },
            ],
            cast_cost_group_selections: vec![mana_option(0), mana_option(1)],
            ..Default::default()
        })),
    };
    e.apply_command(0, &command)
        .expect("both distinct options in one group are legal");

    let phantom = e.state.stack.last().unwrap();
    assert_eq!(phantom.chosen_modes.len(), 2);
    assert_eq!(phantom.cast_cost_receipts.len(), 2);
    assert_eq!(e.state.players[0].mana_pool.blue, 0);
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn phantom_interference_rejects_a_cost_linked_to_an_unchosen_mode_atomically() {
    let decks = Some(vec![
        deck_with("island", &["phantom_interference"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(177_002, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "phantom_interference");
    e.state.players[0].mana_pool.blue = 1;
    e.state.players[0].mana_pool.colorless = 1;
    let before_mana = e.state.players[0].mana_pool;
    let before_hand = e.state.players[0].hand.clone();
    let before_stack_len = e.state.stack.len();
    let before_command_index = e.state.command_index;
    let slot = hand_index_for_card(&e, 0, "phantom_interference");
    let command = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(slot)),
            selected_modes: vec![SelectedSpellMode {
                mode_index: 0,
                targets: vec![],
            }],
            // This is the second mode's cost, not the chosen first mode's cost.
            cast_cost_group_selections: vec![mana_option(1)],
            ..Default::default()
        })),
    };
    assert!(e.apply_command(0, &command).is_err());
    assert_eq!(e.state.players[0].mana_pool, before_mana);
    assert_eq!(e.state.players[0].hand, before_hand);
    assert_eq!(e.state.stack.len(), before_stack_len);
    assert_eq!(e.state.command_index, before_command_index);
}

#[test]
fn final_showdown_chooses_during_resolution_and_applies_modes_in_printed_order() {
    let decks = Some(vec![
        deck_with("plains", &["final_showdown", "wind_drake", "grizzly_bears"]),
        deck_with("mountain", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(177_003, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "final_showdown");
    let survivor = relocate_to_battlefield(&mut e, 0, "wind_drake", false);
    let own_other = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let opposing = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    e.state.players[0].mana_pool.white = 3;
    e.state.players[0].mana_pool.colorless = 5;

    let slot = hand_index_for_card(&e, 0, "final_showdown");
    let command = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(slot)),
            selected_modes: (0..3)
                .map(|mode_index| SelectedSpellMode {
                    mode_index,
                    targets: vec![],
                })
                .collect(),
            cast_cost_group_selections: (0..3).map(mana_option).collect(),
            ..Default::default()
        })),
    };
    e.apply_command(0, &command).expect("cast all three modes");
    let first = e.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    e.apply_command(first, &pass()).expect("first pass");
    let resolving = e.apply_command(second, &pass()).expect("second pass");
    let choice = find_resolution_choice(&resolving).expect("resolution-time permanent choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::PermanentObjects);
    assert_eq!(choice.prompt_text, "Choose a creature you control.");
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(choice.candidate_object_ids.contains(&survivor));
    assert!(choice.candidate_object_ids.contains(&own_other));
    assert!(!choice.candidate_object_ids.contains(&opposing));

    e.apply_command(0, &submit_resolution_choice(vec![survivor]))
        .expect("choose the creature that gains indestructible");
    assert_eq!(e.state.objects[&survivor].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&own_other].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard);
    let characteristics = e.characteristics(survivor).expect("surviving creature");
    assert!(!characteristics.has_keyword(Keyword::Flying));
    assert!(characteristics.has_keyword(Keyword::Indestructible));
}
