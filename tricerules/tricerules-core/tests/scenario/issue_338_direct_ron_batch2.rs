//! Issue #338 (second cohort) — command-boundary scenarios for the reviewed direct-RON batch.
//!
//! Fanatical Offering, Betrayer's Bargain, Dusk Rose Reliquary, Mudbutton Cursetosser and Lys
//! Alana Dignitary were promoted after complete-definition review against the pinned Scryfall
//! snapshot. The exact records and `rulings_uri` were fetched 2026-09-20. Every expectation is the
//! reviewed printed Oracle behavior. Governing CR concepts: CR 118.8/601.2b/f-h (announced
//! additional costs), 701.4 (behold), 701.9 (sacrifice), 121.1 (draw), 111.10s (Map), 120.3/616.1
//! (damage and the exile-if-would-die replacement; the 2024-09-20 ruling makes it last the turn),
//! 702.21a-b (Ward, paid by the targeting spell's controller), 610.3 (linked exile), 508.1c/509.1b
//! (combat restrictions), 603.6a/603.7 (entry and dies triggers), 106.1/605.1a (mana abilities)
//! and 608.2c (printed instruction order).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cast_cost_group_selection::SelectedObject, CastCostGroupSelection, CostObjectRefs,
    ResolutionChoiceDecision, TargetRefKind,
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

fn permanent_option(
    engine: &GameEngine,
    option_index: u32,
    object_id: u32,
) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        selected_object: Some(SelectedObject::PermanentId(object_id)),
        expected_zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
        ..Default::default()
    }
}

fn option(option_index: u32) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        ..Default::default()
    }
}

fn hand_option(option_index: u32, hand_index: usize) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        selected_object: Some(SelectedObject::HandIndex(hand_index as u32)),
        ..Default::default()
    }
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

fn receipt_option(engine: &GameEngine) -> String {
    engine
        .state
        .stack
        .last()
        .expect("spell on stack")
        .cast_cost_receipts[0]
        .option_id
        .as_ref()
        .expect("committed option id")
        .as_str()
        .to_string()
}

#[test]
fn issue_338_fanatical_offering_sacrifices_and_makes_a_map() {
    // Artifact branch.
    let decks = Some(vec![
        deck_with("swamp", &["fanatical_offering", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_701, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "swiftfoot_boots");
    ensure_card_in_hand(&mut engine, 0, "fanatical_offering");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "fanatical_offering");
    assert!(
        engine.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "the sacrifice additional cost is mandatory"
    );
    let slot = hand_index_for_card(&engine, 0, "fanatical_offering");
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![object_cost(&engine, 0, &[artifact])],
            ),
        )
        .expect("sacrificing an artifact pays the additional cost");
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(receipt_option(&engine), "sacrifice_artifact_or_creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before - 1 + 2);
    let map = battlefield_object_for_card(&engine, 0, "map");
    assert_eq!(engine.state.objects[&map].card_id, "map");

    // Creature branch.
    let decks = Some(vec![
        deck_with("swamp", &["fanatical_offering", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut creature_engine = GameEngine::new(338_702, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut creature_engine);
    let fodder = inject_creature_on_battlefield(&mut creature_engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut creature_engine, 0, "fanatical_offering");
    give_mana(
        &mut creature_engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&creature_engine, 0, "fanatical_offering");
    creature_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![object_cost(&creature_engine, 0, &[fodder])],
            ),
        )
        .expect("sacrificing a creature pays the additional cost");
    assert_eq!(creature_engine.state.objects[&fodder].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut creature_engine);
    let _ = battlefield_object_for_card(&creature_engine, 0, "map");
}

#[test]
fn issue_338_betrayers_bargain_pays_and_exiles_any_fatal_creature() {
    // Sacrifice branch and immediate lethal damage.
    let decks = Some(vec![
        deck_with("mountain", &["betrayers_bargain", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(338_801, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "betrayers_bargain");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "betrayers_bargain");
    assert!(
        engine
            .apply_command(0, &cast_spell(slot, target_object(target)))
            .is_err(),
        "one announced additional cost is mandatory"
    );
    let slot = hand_index_for_card(&engine, 0, "betrayers_bargain");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![object_cost(&engine, 0, &[fodder])],
            ),
        )
        .expect("sacrificing a creature pays the additional cost");
    assert_eq!(receipt_option(&engine), "sacrifice_creature_or_enchantment");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);

    // {2} branch.
    let decks = Some(vec![
        deck_with("mountain", &["betrayers_bargain", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut mana_engine = GameEngine::new(338_802, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut mana_engine);
    let target = inject_creature_on_battlefield(&mut mana_engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut mana_engine, 0, "betrayers_bargain");
    give_mana(
        &mut mana_engine,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&mana_engine, 0, "betrayers_bargain");
    mana_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, target_object(target), vec![option(1)]),
        )
        .expect("paying {2} pays the additional cost");
    assert_eq!(receipt_option(&mana_engine), "pay_mana");
    resolve_entire_stack_two_player(&mut mana_engine);
    assert_eq!(mana_engine.state.objects[&target].zone, Zone::Exile);

    // The replacement lasts the rest of the turn (2024-09-20 ruling): a 6-toughness creature
    // survives this spell's 5 damage, then dies to a later Lightning Bolt and is exiled instead.
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["betrayers_bargain", "lightning_bolt", "grizzly_bears"],
        ),
        deck_with("forest", &["havenwood_wurm"]),
    ]);
    let mut later_engine = GameEngine::new(338_803, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut later_engine);
    let fodder = inject_creature_on_battlefield(&mut later_engine, 0, "grizzly_bears");
    let wurm = inject_creature_with_stats(&mut later_engine, 1, "havenwood_wurm", 5, 6);
    ensure_card_in_hand(&mut later_engine, 0, "betrayers_bargain");
    ensure_card_in_hand(&mut later_engine, 0, "lightning_bolt");
    give_mana(
        &mut later_engine,
        0,
        ManaGift {
            r: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&later_engine, 0, "betrayers_bargain");
    later_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(wurm),
                vec![object_cost(&later_engine, 0, &[fodder])],
            ),
        )
        .expect("first spell pays the additional cost");
    resolve_entire_stack_two_player(&mut later_engine);
    assert_eq!(later_engine.state.objects[&wurm].zone, Zone::Battlefield);
    assert_eq!(later_engine.state.objects[&wurm].damage, 5);
    let bolt_slot = hand_index_for_card(&later_engine, 0, "lightning_bolt");
    later_engine
        .apply_command(0, &cast_spell(bolt_slot, target_object(wurm)))
        .expect("Lightning Bolt finishes the Wurm");
    resolve_entire_stack_two_player(&mut later_engine);
    assert_eq!(
        later_engine.state.objects[&wurm].zone,
        Zone::Exile,
        "the turn-long exile replacement applies to the later lethal damage too"
    );

    // The replacement is bound to the targeted object: an unrelated creature that dies later
    // this turn still goes to its owner's graveyard.
    let bystander = inject_creature_with_stats(&mut later_engine, 1, "grizzly_bears", 2, 2);
    later_engine
        .state
        .objects
        .get_mut(&bystander)
        .expect("bystander")
        .damage = 3;
    later_engine
        .apply_command(0, &pass())
        .expect("state-based actions destroy the unrelated creature");
    assert_eq!(later_engine.state.objects[&bystander].zone, Zone::Graveyard);

    // The enchantment alternative also pays the additional cost.
    let decks = Some(vec![
        deck_with("mountain", &["betrayers_bargain", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut enchant_engine = GameEngine::new(338_804, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut enchant_engine);
    let enchantment = inject_permanent_on_battlefield(&mut enchant_engine, 0, "crusade");
    let target = inject_creature_on_battlefield(&mut enchant_engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut enchant_engine, 0, "betrayers_bargain");
    give_mana(
        &mut enchant_engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&enchant_engine, 0, "betrayers_bargain");
    enchant_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![object_cost(&enchant_engine, 0, &[enchantment])],
            ),
        )
        .expect("sacrificing an enchantment pays the additional cost");
    assert_eq!(
        enchant_engine.state.objects[&enchantment].zone,
        Zone::Graveyard
    );
    resolve_entire_stack_two_player(&mut enchant_engine);
    assert_eq!(enchant_engine.state.objects[&target].zone, Zone::Exile);
}

#[test]
fn issue_338_dusk_rose_reliquary_wards_and_exiles_until_it_leaves() {
    let decks = Some(vec![
        deck_with("plains", &["dusk_rose_reliquary", "grizzly_bears"]),
        deck_with("mountain", &["abrade"]),
    ]);
    let mut engine = GameEngine::new(338_901, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let fodder = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let victim = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "dusk_rose_reliquary");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "dusk_rose_reliquary");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![object_cost(&engine, 0, &[fodder])],
            ),
        )
        .expect("sacrificing a creature pays the additional cost");
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "entry trigger waits"
    );
    let reliquary = battlefield_object_for_card(&engine, 0, "dusk_rose_reliquary");
    engine
        .apply_command(0, &choose_trigger_target(victim))
        .expect("the entry trigger exiles an opposing creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&victim].zone, Zone::Exile);

    // The opponent destroys the Reliquary with Abrade; Ward {2} makes them pay to keep it.
    engine
        .apply_command(0, &pass())
        .expect("active player passes priority to the opponent");
    ensure_card_in_hand(&mut engine, 1, "abrade");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let abrade_slot = hand_index_for_card(&engine, 1, "abrade");
    engine
        .apply_command(
            1,
            &cast_modal_spell(abrade_slot, vec![(1, target_object(reliquary))]),
        )
        .expect("Abrade's destroy-artifact mode targets the Reliquary");
    assert_eq!(engine.state.stack.len(), 2, "Ward above Abrade");
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward mana payment");
    assert_eq!(pending.deciding_player, 1, "the opponent pays Ward");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::ManaPayment);
    submit_mana_resolution_decision(&mut engine, 1, ResolutionChoiceDecision::PayMana)
        .expect("pay Ward {2}");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&reliquary].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&victim].zone,
        Zone::Battlefield,
        "the linked exile returns the creature when the Reliquary leaves"
    );

    // Declining Ward counters the targeting spell and leaves the exiled creature exiled.
    let decks = Some(vec![
        deck_with("plains", &["dusk_rose_reliquary", "grizzly_bears"]),
        deck_with("mountain", &["abrade"]),
    ]);
    let mut decline = GameEngine::new(338_902, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut decline);
    let fodder = inject_creature_on_battlefield(&mut decline, 0, "grizzly_bears");
    let victim = inject_creature_on_battlefield(&mut decline, 1, "grizzly_bears");
    ensure_card_in_hand(&mut decline, 0, "dusk_rose_reliquary");
    give_mana(
        &mut decline,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&decline, 0, "dusk_rose_reliquary");
    decline
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![object_cost(&decline, 0, &[fodder])],
            ),
        )
        .expect("sacrificing a creature pays the additional cost");
    pass_both_players(&mut decline);
    let reliquary = battlefield_object_for_card(&decline, 0, "dusk_rose_reliquary");
    decline
        .apply_command(0, &choose_trigger_target(victim))
        .expect("exile the opposing creature");
    resolve_entire_stack_two_player(&mut decline);
    decline
        .apply_command(0, &pass())
        .expect("active player passes priority to the opponent");
    ensure_card_in_hand(&mut decline, 1, "abrade");
    give_mana(
        &mut decline,
        1,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let abrade_slot = hand_index_for_card(&decline, 1, "abrade");
    let abrade_id = decline.state.players[1].hand[abrade_slot];
    decline
        .apply_command(
            1,
            &cast_modal_spell(abrade_slot, vec![(1, target_object(reliquary))]),
        )
        .expect("Abrade targets the Reliquary");
    pass_both_players(&mut decline);
    submit_mana_resolution_decision(&mut decline, 1, ResolutionChoiceDecision::Decline)
        .expect("decline Ward");
    resolve_entire_stack_two_player(&mut decline);
    assert_eq!(decline.state.objects[&reliquary].zone, Zone::Battlefield);
    assert_eq!(decline.state.objects[&abrade_id].zone, Zone::Graveyard);
    assert_eq!(decline.state.objects[&victim].zone, Zone::Exile);

    // The artifact alternative also pays the additional cost.
    let decks = Some(vec![
        deck_with("plains", &["dusk_rose_reliquary"]),
        deck_with("mountain", &[]),
    ]);
    let mut artifact_engine = GameEngine::new(338_903, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut artifact_engine);
    let artifact = inject_permanent_on_battlefield(&mut artifact_engine, 0, "swiftfoot_boots");
    ensure_card_in_hand(&mut artifact_engine, 0, "dusk_rose_reliquary");
    give_mana(
        &mut artifact_engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&artifact_engine, 0, "dusk_rose_reliquary");
    artifact_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![object_cost(&artifact_engine, 0, &[artifact])],
            ),
        )
        .expect("sacrificing an artifact pays the additional cost");
    assert_eq!(
        artifact_engine.state.objects[&artifact].zone,
        Zone::Graveyard
    );
    resolve_entire_stack_two_player(&mut artifact_engine);
    let _ = battlefield_object_for_card(&artifact_engine, 0, "dusk_rose_reliquary");
}

#[test]
fn issue_338_mudbutton_cursetosser_beholds_or_pays_and_punishes_on_death() {
    let decks = Some(vec![
        deck_with("swamp", &["mudbutton_cursetosser", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_951, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let goblin = inject_creature_on_battlefield(&mut engine, 0, "crazed_goblin");
    let non_goblin = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "mudbutton_cursetosser");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "mudbutton_cursetosser");
    assert!(
        engine.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "an announced additional cost is mandatory"
    );
    let slot = hand_index_for_card(&engine, 0, "mudbutton_cursetosser");
    assert!(
        engine
            .apply_command(
                0,
                &cast_spell_with_cast_cost_groups(
                    slot,
                    vec![],
                    vec![permanent_option(&engine, 0, non_goblin)],
                ),
            )
            .is_err(),
        "only a Goblin can be beheld"
    );
    let slot = hand_index_for_card(&engine, 0, "mudbutton_cursetosser");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![permanent_option(&engine, 0, goblin)],
            ),
        )
        .expect("beholding a Goblin pays the additional cost");
    assert_eq!(receipt_option(&engine), "behold");
    assert_eq!(engine.state.objects[&goblin].zone, Zone::Battlefield);
    resolve_entire_stack_two_player(&mut engine);
    let cursetosser = battlefield_object_for_card(&engine, 0, "mudbutton_cursetosser");

    // The printed can't-block restriction is published on the public battlefield view.
    let labels = zone_view_rules_annotation_labels(&mut engine, 0, cursetosser);
    assert!(
        labels.iter().any(|label| label == "Can't block"),
        "public view reports the restriction, got {labels:?}"
    );

    // The {2} alternative is published for the same group.
    let decks = Some(vec![
        deck_with("swamp", &["mudbutton_cursetosser"]),
        deck_with("forest", &[]),
    ]);
    let mut mana_engine = GameEngine::new(338_952, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut mana_engine);
    ensure_card_in_hand(&mut mana_engine, 0, "mudbutton_cursetosser");
    give_mana(
        &mut mana_engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&mana_engine, 0, "mudbutton_cursetosser");
    mana_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, vec![], vec![option(1)]),
        )
        .expect("paying {2} pays the additional cost");
    assert_eq!(receipt_option(&mana_engine), "pay_mana");

    // A Goblin card in hand is also a legal behold choice; beholding reveals it and leaves it
    // in the hand.
    let decks = Some(vec![
        deck_with("swamp", &["mudbutton_cursetosser"]),
        deck_with("forest", &[]),
    ]);
    let mut hand_engine = GameEngine::new(338_953, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut hand_engine);
    let goblin_card = inject_card_into_hand(&mut hand_engine, 0, "crazed_goblin");
    ensure_card_in_hand(&mut hand_engine, 0, "mudbutton_cursetosser");
    give_mana(
        &mut hand_engine,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&hand_engine, 0, "mudbutton_cursetosser");
    let goblin_slot = hand_index_for_card(&hand_engine, 0, "crazed_goblin");
    hand_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, vec![], vec![hand_option(0, goblin_slot)]),
        )
        .expect("revealing a Goblin card pays the additional cost");
    assert_eq!(receipt_option(&hand_engine), "behold");
    assert_eq!(
        hand_engine.state.objects[&goblin_card].zone,
        Zone::Hand,
        "beholding a hand card leaves it in hand"
    );

    // The dies trigger only targets a creature an opponent controls with power 2 or less.
    let power_two = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let power_three = inject_creature_with_stats(&mut engine, 1, "hill_giant", 3, 3);
    engine
        .state
        .objects
        .get_mut(&cursetosser)
        .expect("source")
        .damage = 99;
    engine
        .apply_command(0, &pass())
        .expect("state-based actions destroy the Cursetosser and queue its dies trigger");
    assert_eq!(engine.state.objects[&cursetosser].zone, Zone::Graveyard);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(
        engine
            .apply_command(0, &choose_trigger_target(power_three))
            .is_err(),
        "a power-3 creature is outside the filter"
    );
    engine
        .apply_command(0, &choose_trigger_target(power_two))
        .expect("choose an opposing creature with power 2 or less");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&power_two].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&power_three].zone, Zone::Battlefield);
}

#[test]
fn issue_338_lys_alana_dignitary_mana_requires_an_elf_in_graveyard() {
    let decks = Some(vec![
        deck_with("forest", &["lys_alana_dignitary", "llanowar_elves"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(338_971, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let elf = inject_creature_on_battlefield(&mut engine, 0, "llanowar_elves");
    ensure_card_in_hand(&mut engine, 0, "lys_alana_dignitary");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "lys_alana_dignitary");
    engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                vec![],
                vec![permanent_option(&engine, 0, elf)],
            ),
        )
        .expect("beholding an Elf pays the additional cost");
    assert_eq!(receipt_option(&engine), "behold");
    resolve_entire_stack_two_player(&mut engine);
    let dignitary = battlefield_object_for_card(&engine, 0, "lys_alana_dignitary");
    engine
        .state
        .objects
        .get_mut(&dignitary)
        .expect("source")
        .summoning_sick = false;
    engine
        .state
        .objects
        .get_mut(&dignitary)
        .expect("source")
        .tapped = false;

    // Without an Elf card in the graveyard the activation condition is not satisfied.
    engine.state.players[0].mana_pool.green = 0;
    assert!(
        engine
            .apply_command(0, &activate_ability_for(&engine, dignitary, 0, vec![]))
            .is_err(),
        "the mana ability requires an Elf card in the graveyard"
    );

    // With one, {T} adds {G}{G} and taps the source.
    inject_graveyard_card(&mut engine, 0, "llanowar_elves");
    engine
        .apply_command(0, &activate_ability_for(&engine, dignitary, 0, vec![]))
        .expect("an Elf card in the graveyard enables the mana ability");
    assert_eq!(engine.state.players[0].mana_pool.green, 2);
    assert!(engine.state.objects[&dignitary].tapped);

    // The {2} alternative is published for the same group.
    let decks = Some(vec![
        deck_with("forest", &["lys_alana_dignitary"]),
        deck_with("forest", &[]),
    ]);
    let mut mana_cast_engine = GameEngine::new(338_972, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut mana_cast_engine);
    ensure_card_in_hand(&mut mana_cast_engine, 0, "lys_alana_dignitary");
    give_mana(
        &mut mana_cast_engine,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&mana_cast_engine, 0, "lys_alana_dignitary");
    mana_cast_engine
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, vec![], vec![option(1)]),
        )
        .expect("paying {2} pays the additional cost");
    assert_eq!(receipt_option(&mana_cast_engine), "pay_mana");
}
