use crate::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    CastCostGroupSelection, CastMethod, CastSpell, CostObjectRefs, RuledCommand, SelectedSpellMode,
};

fn object_ref(engine: &GameEngine, object_id: u32) -> tricerules_proto::ruled::v1::CostObjectRef {
    tricerules_proto::ruled::v1::CostObjectRef {
        object_id,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
    }
}

fn object_cost(engine: &GameEngine, option_index: u32, objects: &[u32]) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        battlefield_objects: Some(CostObjectRefs {
            objects: objects.iter().map(|oid| object_ref(engine, *oid)).collect(),
        }),
        ..Default::default()
    }
}

#[test]
fn issue_182_cruel_alliance_requires_teamwork_for_the_expanded_target_and_gains_life() {
    let decks = Some(vec![
        deck_with("swamp", &["cruel_alliance", "grizzly_bears"]),
        deck_with("forest", &["colossal_dreadmaw"]),
    ]);
    let mut engine = GameEngine::new(182_101, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "cruel_alliance");
    let teammate = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "cruel_alliance");

    assert!(engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .is_err());
    assert!(!engine.state.objects[&teammate].tapped);

    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![object_cost(&engine, 0, &[teammate])],
            ),
        )
        .expect("Teamwork expands Cruel Alliance's target filter");
    assert!(engine.state.objects[&teammate].tapped);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 23);
}

#[test]
fn issue_182_murdocks_crusade_links_teamwork_to_choosing_both_modes() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["murdocks_crusade", "grizzly_bears", "grizzly_bears"],
        ),
        deck_with(
            "forest",
            &["colossal_dreadmaw", "burn,_burn,_tree_and_fern"],
        ),
    ]);
    let mut engine = GameEngine::new(182_102, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "murdocks_crusade");
    let creature = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    let enchantment = relocate_to_battlefield(&mut engine, 1, "burn,_burn,_tree_and_fern", false);
    let slot = hand_index_for_card(&engine, 0, "murdocks_crusade");
    let without_candidates = engine.initial_response_batch();
    let published = without_candidates.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == slot as u32)
        .expect("Murdock's Crusade remains castable for one mode");
    assert_eq!(published.max_modes, 1);
    assert!(published.all_modes_cast_cost.is_none());
    let first = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let second = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    grant_pool(&mut engine, 0);
    let with_candidates = engine.initial_response_batch();
    let published = with_candidates.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == slot as u32)
        .expect("Murdock's Crusade is castable with Teamwork");
    assert_eq!(published.max_modes, 2);
    assert!(published.all_modes_cast_cost.is_some());
    let modes = vec![
        SelectedSpellMode {
            mode_index: 0,
            targets: target_object(creature),
        },
        SelectedSpellMode {
            mode_index: 1,
            targets: target_object(enchantment),
        },
    ];
    let without_teamwork = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(hand_cast_source(slot)),
            cast_method: CastMethod::Normal as i32,
            selected_modes: modes.clone(),
            ..Default::default()
        })),
    };
    assert!(engine.apply_command(0, &without_teamwork).is_err());
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    source: Some(hand_cast_source(slot)),
                    cast_method: CastMethod::Normal as i32,
                    selected_modes: modes,
                    cast_cost_group_selections: vec![object_cost(&engine, 0, &[first, second])],
                    ..Default::default()
                })),
            },
        )
        .expect("Teamwork permits choosing both modes");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&enchantment].zone, Zone::Exile);
}

#[test]
fn issue_182_maria_triggers_only_from_the_teamwork_payment_action() {
    let decks = Some(vec![
        deck_with("swamp", &["cruel_alliance", "agent_maria_hill"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(182_103, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "cruel_alliance");
    let maria = relocate_to_battlefield(&mut engine, 0, "agent_maria_hill", false);
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let hand_before = engine.state.players[0].hand.len();
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "cruel_alliance");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![object_cost(&engine, 0, &[maria])],
            ),
        )
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&maria].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
}

#[test]
fn issue_182_object_kicker_and_required_sacrifice_or_mana_group_are_atomic() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &["stomped_by_the_foot", "stir_up_trouble", "grizzly_bears"],
        ),
        deck_with("forest", &["hill_giant", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(182_104, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "stomped_by_the_foot");
    ensure_card_in_hand(&mut engine, 0, "stir_up_trouble");
    let sacrifice = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let giant = relocate_to_battlefield(&mut engine, 1, "hill_giant", false);
    grant_pool(&mut engine, 0);
    let stomped = hand_index_for_card(&engine, 0, "stomped_by_the_foot");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                stomped,
                target_object(giant),
                vec![object_cost(&engine, 0, &[sacrifice])],
            ),
        )
        .unwrap();
    assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&giant].zone, Zone::Graveyard);

    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let stir = hand_index_for_card(&engine, 0, "stir_up_trouble");
    assert!(engine
        .apply_command(0, &cast_spell(stir, target_object(target)))
        .is_err());
    engine.state.priority_idx = 0;
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                stir,
                target_object(target),
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 1,
                    ..Default::default()
                }],
            ),
        )
        .expect("required group accepts the pay-four option");
}
