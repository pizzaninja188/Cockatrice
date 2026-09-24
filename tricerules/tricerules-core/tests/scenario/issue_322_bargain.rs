//! Issue #322 — Bargain is an optional cast-time sacrifice whose receipt drives spell text.
//!
//! Oracle text is pinned in the matching card definitions. CR 601.2b/f and 118.8 govern the
//! optional additional cost, CR 607.2i links that choice to the resolution instructions, CR
//! 702.166 defines Bargain, and CR 707.2 preserves announced spell-copy choices.

use super::helpers::*;
use tricerules_cards::{Keyword, ObjectCastCostKind};
use tricerules_core::state::CastCostObjectReceipt;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, CostObjectRef, CostObjectRefs, RuledCommand,
};

fn bargain_selection(engine: &GameEngine, permanent: u32) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        battlefield_objects: Some(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id: permanent,
                zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&permanent)
                    .copied()
                    .unwrap_or(0),
            }],
        }),
        ..Default::default()
    }
}

fn choose_trigger_target(target: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(target),
        })),
    }
}

#[test]
fn archons_glory_applies_its_conditional_keywords_only_after_bargain() {
    for bargained in [false, true] {
        let mut engine = GameEngine::new(322_001, &[0, 1], 20, None, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let treasure = inject_permanent_on_battlefield(&mut engine, 0, "treasure");
        let spell = inject_card_into_hand(&mut engine, 0, "archons_glory");
        engine.state.players[0].mana_pool.white = 1;

        let costs = if bargained {
            vec![bargain_selection(&engine, treasure)]
        } else {
            vec![]
        };
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    hand_index_for_card(&engine, 0, "archons_glory"),
                    target_object(target),
                    costs,
                ),
            )
            .expect("Archon's Glory casts with or without Bargain");
        let receipt = engine
            .state
            .stack
            .last()
            .unwrap()
            .cast_cost_receipts
            .first();
        assert_eq!(receipt.is_some(), bargained);
        if let Some(receipt) = receipt {
            assert_eq!(receipt.object_cost_kind, Some(ObjectCastCostKind::Bargain));
            assert!(matches!(
                receipt.objects.as_slice(),
                [CastCostObjectReceipt::ChosenPermanent { object_id, .. }] if *object_id == treasure
            ));
        }
        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(engine.effective_power(target), Some(4));
        assert_eq!(
            engine.effective_has_keyword(target, Keyword::Flying),
            bargained
        );
        assert_eq!(
            engine.effective_has_keyword(target, Keyword::Lifelink),
            bargained
        );
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    }
}

#[test]
fn bargain_cost_rejects_ineligible_opponent_and_stale_objects_atomically() {
    let mut engine = GameEngine::new(322_008, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let own_treasure = inject_permanent_on_battlefield(&mut engine, 0, "treasure");
    let opponent_treasure = inject_permanent_on_battlefield(&mut engine, 1, "treasure");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let spell = inject_card_into_hand(&mut engine, 0, "archons_glory");
    engine.state.players[0].mana_pool.white = 1;
    let hand_before = engine.state.players[0].hand.clone();
    let mana_before = (
        engine.state.players[0].mana_pool.white,
        engine.state.players[0].mana_pool.blue,
        engine.state.players[0].mana_pool.black,
        engine.state.players[0].mana_pool.red,
        engine.state.players[0].mana_pool.green,
        engine.state.players[0].mana_pool.colorless,
    );
    let mut stale = bargain_selection(&engine, own_treasure);
    stale.battlefield_objects.as_mut().unwrap().objects[0].zone_change_generation += 1;
    let invalid_costs = [
        bargain_selection(&engine, own_creature),
        bargain_selection(&engine, opponent_treasure),
        stale,
    ];

    for selection in invalid_costs {
        assert!(engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    hand_index_for_card(&engine, 0, "archons_glory"),
                    target_object(target),
                    vec![selection],
                ),
            )
            .is_err());
        assert_eq!(engine.state.players[0].hand, hand_before);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&own_creature].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&own_treasure].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&opponent_treasure].zone,
            Zone::Battlefield
        );
        assert_eq!(
            (
                engine.state.players[0].mana_pool.white,
                engine.state.players[0].mana_pool.blue,
                engine.state.players[0].mana_pool.black,
                engine.state.players[0].mana_pool.red,
                engine.state.players[0].mana_pool.green,
                engine.state.players[0].mana_pool.colorless,
            ),
            mana_before
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn candy_grapple_uses_three_or_five_minus_five_from_the_committed_choice() {
    for (bargained, expected_toughness) in [(false, 3), (true, 1)] {
        let decks = Some(vec![
            deck_with("swamp", &[]),
            deck_with("forest", &["colossal_dreadmaw"]),
        ]);
        let mut engine = GameEngine::new(322_002, &[0, 1], 20, decks, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let target = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
        let treasure = inject_permanent_on_battlefield(&mut engine, 0, "treasure");
        inject_card_into_hand(&mut engine, 0, "candy_grapple");
        engine.state.players[0].mana_pool.black = 1;
        engine.state.players[0].mana_pool.colorless = 1;

        let costs = if bargained {
            vec![bargain_selection(&engine, treasure)]
        } else {
            vec![]
        };
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    hand_index_for_card(&engine, 0, "candy_grapple"),
                    target_object(target),
                    costs,
                ),
            )
            .expect("Candy Grapple casts with or without Bargain");
        assert_eq!(
            engine
                .state
                .stack
                .last()
                .unwrap()
                .cast_cost_receipts
                .is_empty(),
            !bargained
        );
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.effective_toughness(target), Some(expected_toughness));
    }
}

#[test]
fn copied_bargained_kellans_lightblades_keeps_the_choice_and_destroys_its_attacker() {
    let mut engine = GameEngine::new(322_003, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let treasure = inject_permanent_on_battlefield(&mut engine, 0, "treasure");
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("defending player passes");
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("bear attacks");

    inject_card_into_hand(&mut engine, 0, "kellans_lightblades");
    inject_card_into_hand(&mut engine, 1, "twincast");
    engine.state.players[0].mana_pool.white = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    engine.state.players[1].mana_pool.blue = 2;
    let spell_slot = hand_index_for_card(&engine, 0, "kellans_lightblades");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                spell_slot,
                target_object(attacker),
                vec![bargain_selection(&engine, treasure)],
            ),
        )
        .expect("bargained Kellan's Lightblades targets the attacker");
    let original = engine.state.stack.last().unwrap().clone();
    assert_eq!(
        original.cast_cost_receipts[0].object_cost_kind,
        Some(ObjectCastCostKind::Bargain)
    );

    engine.apply_command(0, &pass()).expect("pass to Twincast");
    engine
        .apply_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "twincast"),
                target_object(original.id),
            ),
        )
        .expect("Twincast copies the Bargained spell");
    pass_both_players(&mut engine);
    engine
        .apply_command(1, &submit_resolution_choice(vec![attacker]))
        .expect("retarget the spell copy to the same attacker");

    let copy = engine.state.stack.iter().find(|item| item.is_copy).unwrap();
    assert_eq!(copy.cast_cost_receipts, original.cast_cost_receipts);
    assert_eq!(
        copy.cast_cost_receipts[0].object_cost_kind,
        Some(ObjectCastCostKind::Bargain)
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&attacker].damage, 0);
}

#[test]
fn nonbargained_kellans_lightblades_deals_three_damage() {
    let decks = Some(vec![
        deck_with("forest", &["colossal_dreadmaw"]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(322_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let attacker = relocate_to_battlefield(&mut engine, 0, "colossal_dreadmaw", false);
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    engine
        .apply_command(1, &pass())
        .expect("defending player passes");
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("Dreadmaw attacks");

    inject_card_into_hand(&mut engine, 1, "kellans_lightblades");
    engine.state.players[1].mana_pool.white = 1;
    engine.state.players[1].mana_pool.colorless = 1;
    engine
        .apply_command(0, &pass())
        .expect("pass priority to the defender");
    engine
        .apply_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "kellans_lightblades"),
                target_object(attacker),
            ),
        )
        .expect("nonbargained Kellan's Lightblades casts");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&attacker].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&attacker].damage, 3);
}

#[test]
fn bargained_ouphe_exiles_a_targeted_opponent_artifact() {
    let mut engine = GameEngine::new(322_005, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let payment = inject_permanent_on_battlefield(&mut engine, 0, "treasure");
    let target = inject_permanent_on_battlefield(&mut engine, 1, "treasure");
    inject_card_into_hand(&mut engine, 0, "troublemaker_ouphe");
    engine.state.players[0].mana_pool.green = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                hand_index_for_card(&engine, 0, "troublemaker_ouphe"),
                Vec::new(),
                vec![bargain_selection(&engine, payment)],
            ),
        )
        .expect("cast Troublemaker Ouphe with Bargain");
    assert_eq!(
        engine.state.stack.last().unwrap().cast_cost_receipts[0].object_cost_kind,
        Some(ObjectCastCostKind::Bargain)
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_target(target))
        .expect("choose an opponent's artifact");
    pass_both_players(&mut engine);
    assert!(!engine.state.players[1].battlefield.contains(&target));
    assert!(engine.state.pending_triggers.is_empty());
}

#[test]
fn unbargained_ouphe_and_a_permanent_copy_have_no_bargain_trigger() {
    let mut unbargained = GameEngine::new(322_006, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut unbargained);
    let target = inject_permanent_on_battlefield(&mut unbargained, 1, "treasure");
    inject_card_into_hand(&mut unbargained, 0, "troublemaker_ouphe");
    unbargained.state.players[0].mana_pool.green = 1;
    unbargained.state.players[0].mana_pool.colorless = 1;
    unbargained
        .apply_command(
            0,
            &cast_spell(
                hand_index_for_card(&unbargained, 0, "troublemaker_ouphe"),
                Vec::new(),
            ),
        )
        .expect("cast Troublemaker Ouphe without Bargain");
    pass_both_players(&mut unbargained);
    assert!(unbargained.state.pending_triggers.is_empty());
    assert!(unbargained.state.players[1].battlefield.contains(&target));

    let mut copy_game = GameEngine::new(322_007, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut copy_game);
    let payment = inject_permanent_on_battlefield(&mut copy_game, 0, "treasure");
    inject_card_into_hand(&mut copy_game, 0, "troublemaker_ouphe");
    copy_game.state.players[0].mana_pool.green = 1;
    copy_game.state.players[0].mana_pool.colorless = 1;
    copy_game
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                hand_index_for_card(&copy_game, 0, "troublemaker_ouphe"),
                Vec::new(),
                vec![bargain_selection(&copy_game, payment)],
            ),
        )
        .expect("cast a Bargained source to copy");
    pass_both_players(&mut copy_game);
    assert!(copy_game.state.pending_triggers.is_empty());

    let opponent_artifact = inject_permanent_on_battlefield(&mut copy_game, 1, "treasure");
    inject_card_into_hand(&mut copy_game, 0, "clone");
    copy_game.state.players[0].mana_pool.blue = 1;
    copy_game.state.players[0].mana_pool.colorless = 3;
    copy_game
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&copy_game, 0, "clone"), Vec::new()),
        )
        .expect("cast Clone without a Bargain cost");
    pass_both_players(&mut copy_game);
    let source = battlefield_object_for_card(&copy_game, 0, "troublemaker_ouphe");
    copy_game
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("Clone enters as a copy of the Bargained Ouphe");
    assert_eq!(copy_game.state.players[0].battlefield.len(), 2);
    assert!(copy_game.state.pending_triggers.is_empty());
    assert!(copy_game.state.players[1]
        .battlefield
        .contains(&opponent_artifact));
}

#[test]
fn copied_bargained_ouphe_spell_keeps_bargain_for_its_battlefield_entry() {
    let mut engine = GameEngine::new(322_009, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let payment = inject_permanent_on_battlefield(&mut engine, 0, "treasure");
    let opponent_artifact = inject_permanent_on_battlefield(&mut engine, 1, "treasure");
    inject_card_into_hand(&mut engine, 0, "troublemaker_ouphe");
    engine.state.players[0].mana_pool.green = 1;
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                hand_index_for_card(&engine, 0, "troublemaker_ouphe"),
                Vec::new(),
                vec![bargain_selection(&engine, payment)],
            ),
        )
        .expect("cast a Bargained Troublemaker Ouphe");

    // Represent the CR 707 spell copy exactly at its downstream entry boundary: it is not cast,
    // so caster identity is absent, while the original cost choices and receipt are copied.
    let mut copy = engine
        .state
        .stack
        .last()
        .expect("Ouphe spell on stack")
        .clone();
    copy.id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    copy.is_copy = true;
    copy.cast_by = None;
    copy.cast_occurrence = None;
    engine.state.stack.push(copy);

    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_target(opponent_artifact))
        .expect("copied Bargain choice enables the Ouphe trigger");
    pass_both_players(&mut engine);
    assert!(!engine.state.players[1]
        .battlefield
        .contains(&opponent_artifact));
}
