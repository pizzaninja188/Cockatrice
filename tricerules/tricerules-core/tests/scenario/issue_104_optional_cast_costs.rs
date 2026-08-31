use crate::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::state::CastCostObjectReceipt;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cast_cost_group_selection::SelectedObject, ruled_event::Ev, CastCostGroupSelection,
    CastCostOptionKind, ChoiceKind, HandActionKind, PresentationPathKind, ResolutionChoiceDecision,
};

fn mana_option(group_index: u32, option_index: u32) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index,
        option_index,
        selected_object: None,
        expected_zone_change_generation: 0,
    }
}

#[test]
fn grow_from_the_ashes_publishes_and_records_kicker_as_part_of_total_cost() {
    let decks = Some(vec![
        deck_with("forest", &["grow_from_the_ashes"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(104_001, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "grow_from_the_ashes");
    let forest = relocate_to_battlefield(&mut e, 0, "forest", false);
    let legal_batch = e
        .apply_command(0, &activate_ability(forest, 0, vec![]))
        .expect("mana ability");
    let action = legal_batch.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| {
            action.kind == HandActionKind::HandActionCastSpell as i32
                && action.card_name == "Grow from the Ashes"
        })
        .expect("Grow legal action");
    let group = &action.cost_choices.as_ref().unwrap().cast_cost_groups[0];
    assert_eq!((group.min, group.max), (0, 1));
    assert_eq!(group.options[0].kind, CastCostOptionKind::Mana as i32);
    assert_eq!(group.options[0].additional_mana_cost, "{2}");
    let group_presentation = group
        .presentation
        .as_ref()
        .expect("stable kicker group identity");
    assert_eq!(group_presentation.card_id, "grow_from_the_ashes");
    assert_eq!(group_presentation.face_id, "grow_from_the_ashes");
    assert_eq!(group_presentation.path.last().unwrap().id, "cast_cost_01");
    let option_presentation = group.options[0]
        .presentation
        .as_ref()
        .expect("stable kicker option identity");
    assert_eq!(option_presentation.oracle_line_indices, [1]);
    assert_eq!(option_presentation.path.last().unwrap().id, "option_01");

    e.state.players[0].mana_pool.colorless = 4;
    let slot = hand_index_for_card(&e, 0, "grow_from_the_ashes");
    let batch = e
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(slot, vec![], vec![mana_option(0, 0)]),
        )
        .expect("kicked cast pays the combined {4}{G} total");
    assert_eq!(
        (
            e.state.players[0].mana_pool.white,
            e.state.players[0].mana_pool.blue,
            e.state.players[0].mana_pool.black,
            e.state.players[0].mana_pool.red,
            e.state.players[0].mana_pool.green,
            e.state.players[0].mana_pool.colorless,
        ),
        (0, 0, 0, 0, 0, 0)
    );
    let receipt = &e.state.stack.last().unwrap().cast_cost_receipts[0];
    assert_eq!(receipt.label, "Kicker {2}");
    assert!(receipt.object.is_none());
    let pushed = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(pushed)) => Some(pushed),
            _ => None,
        })
        .expect("kicked stack publication");
    assert_eq!(pushed.chosen_cast_cost_labels, ["Kicker {2}"]);
    assert_eq!(pushed.chosen_cast_cost_presentations.len(), 1);
    assert_eq!(
        pushed.chosen_cast_cost_presentations[0]
            .path
            .last()
            .unwrap()
            .kind,
        PresentationPathKind::CastCostOption as i32
    );

    let first = e.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    e.apply_command(first, &pass()).expect("first pass");
    let resolving = e
        .apply_command(second, &pass())
        .expect("resolve kicked Grow");
    let choice = find_resolution_choice(&resolving).expect("kicked search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 2));
    let chosen = choice
        .candidate_object_ids
        .iter()
        .copied()
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(chosen.len(), 2);
    e.apply_command(0, &submit_resolution_choice(chosen.clone()))
        .expect("choose two basic lands");
    assert!(chosen
        .iter()
        .all(|oid| e.state.objects[oid].zone == Zone::Battlefield));
}

#[test]
fn behold_reveals_only_the_selected_dragon_until_the_spell_leaves_the_stack() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &["caustic_exhale", "adult_gold_dragon", "grizzly_bears"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(104_002, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "caustic_exhale");
    ensure_card_in_hand(&mut e, 0, "adult_gold_dragon");
    ensure_card_in_hand(&mut e, 0, "grizzly_bears");
    let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    e.state.players[0].mana_pool.black = 1;
    let spell_slot = hand_index_for_card(&e, 0, "caustic_exhale");
    let dragon_slot = hand_index_for_card(&e, 0, "adult_gold_dragon");

    let batch = e
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                spell_slot,
                target_object(target),
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    selected_object: Some(SelectedObject::HandIndex(dragon_slot as u32)),
                    expected_zone_change_generation: 0,
                }],
            ),
        )
        .expect("behold Dragon from hand");
    assert!(matches!(
        e.state.stack.last().unwrap().cast_cost_receipts[0].object,
        Some(CastCostObjectReceipt::RevealedHand { ref card_id, .. }) if card_id == "adult_gold_dragon"
    ));
    let snapshot = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ActivePublicRevealSnapshot(snapshot)) => Some(snapshot),
            _ => None,
        })
        .expect("active reveal snapshot");
    assert_eq!(snapshot.reveals.len(), 1);
    assert_eq!(snapshot.reveals[0].card_id, "adult_gold_dragon");
    assert!(!snapshot
        .reveals
        .iter()
        .any(|reveal| reveal.card_id == "grizzly_bears"));

    resolve_entire_stack_two_player(&mut e);
    let cleared = e
        .apply_command(0, &pass())
        .expect("next authoritative batch");
    assert!(cleared.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::ActivePublicRevealSnapshot(snapshot)) if snapshot.reveals.is_empty()
        )
    }));
}

#[test]
fn stale_behold_permanent_rejects_the_atomic_cast_without_spending_mana() {
    let decks = Some(vec![
        deck_with("swamp", &["caustic_exhale", "adult_gold_dragon"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(104_003, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "caustic_exhale");
    let dragon = relocate_to_battlefield(&mut e, 0, "adult_gold_dragon", false);
    let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    e.state.players[0].mana_pool.black = 1;
    let slot = hand_index_for_card(&e, 0, "caustic_exhale");
    let published_generation = e
        .state
        .zone_change_generation
        .get(&dragon)
        .copied()
        .unwrap_or(0);
    e.state
        .zone_change_generation
        .insert(dragon, published_generation + 1);
    let before_stack = e.state.stack.len();
    let before_hand = e.state.players[0].hand.clone();

    let err = e
        .apply_command(
            0,
            &cast_spell_with_cast_cost_groups(
                slot,
                target_object(target),
                vec![CastCostGroupSelection {
                    group_index: 0,
                    option_index: 0,
                    selected_object: Some(SelectedObject::PermanentId(dragon)),
                    expected_zone_change_generation: published_generation,
                }],
            ),
        )
        .expect_err("stale behold identity must reject");
    assert!(format!("{err:?}").contains("stale"));
    assert_eq!(e.state.players[0].mana_pool.black, 1);
    assert_eq!(e.state.players[0].hand, before_hand);
    assert_eq!(e.state.stack.len(), before_stack);
}

#[test]
fn kicked_gnarlid_colony_enters_with_counters_and_grants_trample() {
    let decks = Some(vec![
        deck_with("forest", &["gnarlid_colony"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(104_004, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "gnarlid_colony");
    e.state.players[0].mana_pool.green = 2;
    e.state.players[0].mana_pool.colorless = 3;
    let slot = hand_index_for_card(&e, 0, "gnarlid_colony");
    let oid = e.state.players[0].hand[slot];
    e.apply_command(
        0,
        &cast_spell_with_cast_cost_groups(slot, vec![], vec![mana_option(0, 0)]),
    )
    .expect("cast kicked Gnarlid Colony");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
    assert_eq!(
        e.state.objects[&oid].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert!(e.effective_has_keyword(oid, Keyword::Trample));
}

#[test]
fn osseous_exhale_uses_the_behold_receipt_after_the_revealed_card_is_unrelated_to_resolution() {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("plains", &["osseous_exhale", "adult_gold_dragon"]),
    ]);
    let mut e = GameEngine::new(104_005, &[0, 1], 20, decks, true).unwrap();
    advance_to_declare_attackers(&mut e);
    let attacker = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    ensure_card_in_hand(&mut e, 1, "osseous_exhale");
    ensure_card_in_hand(&mut e, 1, "adult_gold_dragon");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("pass to defender");
    e.state.players[1].mana_pool.white = 1;
    e.state.players[1].mana_pool.colorless = 1;
    let spell = hand_index_for_card(&e, 1, "osseous_exhale");
    let dragon = hand_index_for_card(&e, 1, "adult_gold_dragon");
    e.apply_command(
        1,
        &cast_spell_with_cast_cost_groups(
            spell,
            target_object(attacker),
            vec![CastCostGroupSelection {
                group_index: 0,
                option_index: 0,
                selected_object: Some(SelectedObject::HandIndex(dragon as u32)),
                expected_zone_change_generation: 0,
            }],
        ),
    )
    .expect("cast Osseous Exhale with behold");
    e.apply_command(1, &pass()).expect("defender pass");
    e.apply_command(0, &pass()).expect("resolve Osseous Exhale");
    assert_eq!(e.state.players[1].life, 22);
    assert_eq!(e.state.objects[&attacker].zone, Zone::Graveyard);
}

fn dispelling_exhale_payment_cost(behold: bool, seed: u64) -> u32 {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("island", &["dispelling_exhale", "adult_gold_dragon"]),
    ]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "grizzly_bears");
    ensure_card_in_hand(&mut e, 1, "dispelling_exhale");
    ensure_card_in_hand(&mut e, 1, "adult_gold_dragon");
    e.state.players[0].mana_pool.green = 1;
    e.state.players[0].mana_pool.colorless = 1;
    let creature = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(creature, vec![]))
        .expect("cast creature spell");
    let target_spell = e.state.stack.last().unwrap().id;
    e.apply_command(0, &pass())
        .expect("pass to counterspell caster");
    e.state.players[1].mana_pool.blue = 1;
    e.state.players[1].mana_pool.colorless = 1;
    let exhale = hand_index_for_card(&e, 1, "dispelling_exhale");
    let selections = if behold {
        let dragon = hand_index_for_card(&e, 1, "adult_gold_dragon");
        vec![CastCostGroupSelection {
            group_index: 0,
            option_index: 0,
            selected_object: Some(SelectedObject::HandIndex(dragon as u32)),
            expected_zone_change_generation: 0,
        }]
    } else {
        vec![]
    };
    e.apply_command(
        1,
        &cast_spell_with_cast_cost_groups(exhale, target_object(target_spell), selections),
    )
    .expect("cast Dispelling Exhale");
    e.apply_command(1, &pass())
        .expect("counterspell caster pass");
    let parked = e
        .apply_command(0, &pass())
        .expect("resolve Dispelling Exhale");
    let choice = find_resolution_choice(&parked).expect("soft-counter payment");
    assert_eq!(choice.choice_kind(), ChoiceKind::ManaPayment);
    e.apply_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline payment");
    choice.generic_mana_cost
}

#[test]
fn dispelling_exhale_links_behold_to_the_soft_counter_amount() {
    assert_eq!(dispelling_exhale_payment_cost(false, 104_006), 2);
    assert_eq!(dispelling_exhale_payment_cost(true, 104_007), 4);
}
