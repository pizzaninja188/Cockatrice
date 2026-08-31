use crate::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_event::Ev, HandActionKind};

fn retain_only_hand_object(e: &mut GameEngine, player: usize, keep: u32) {
    let removed: Vec<u32> = e.state.players[player]
        .hand
        .iter()
        .copied()
        .filter(|oid| *oid != keep)
        .collect();
    e.state.players[player].hand.retain(|oid| *oid == keep);
    for oid in removed {
        e.state.players[player].library.push_back(oid);
        e.state.objects.get_mut(&oid).unwrap().zone = Zone::Library;
    }
}

#[test]
fn discard_spell_is_not_legal_when_it_is_the_only_hand_card() {
    let decks = Some(vec![
        deck_with("mountain", &["thrill_of_possibility"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(5301, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    let thrill = relocate_to_hand(&mut e, 0, "thrill_of_possibility");
    let mountain = relocate_to_battlefield(&mut e, 0, "mountain", false);
    retain_only_hand_object(&mut e, 0, thrill);

    let batch = e
        .apply_command(0, &activate_ability(mountain, 0, vec![]))
        .unwrap();
    let legal = batch.legal_by_player.get(&0).unwrap();
    assert!(!legal.hand_actions.iter().any(|action| {
        action.kind == HandActionKind::HandActionCastSpell as i32
            && action.card_name == "Thrill of Possibility"
    }));
}

#[test]
fn thrill_discards_the_selected_physical_card_then_draws_two() {
    let decks = Some(vec![
        deck_with("mountain", &["thrill_of_possibility", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(5302, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "thrill_of_possibility");
    ensure_card_in_hand(&mut e, 0, "grizzly_bears");
    let spell_slot = hand_index_for_card(&e, 0, "thrill_of_possibility");
    let discard_slot = hand_index_for_card(&e, 0, "grizzly_bears");
    let discarded = e.state.players[0].hand[discard_slot];
    e.state.players[0].mana_pool.red = 1;
    e.state.players[0].mana_pool.colorless = 1;

    let batch = e
        .apply_command(
            0,
            &cast_spell_with_costs(
                spell_slot,
                vec![],
                vec![hand_cost_selection(0, discard_slot as u32)],
            ),
        )
        .expect("discard and cast atomically");
    assert_eq!(e.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(e.state.stack.len(), 1);
    assert!(batch
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::PermanentMoved(_)))));
    assert!(batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log))
                if log.text == "P0 casts Thrill of Possibility discarding Grizzly Bears"
        )
    }));

    let hand_after_payment = e.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_after_payment + 2);
}

#[test]
fn bone_splinters_can_sacrifice_its_own_target_and_then_fizzle() {
    let decks = Some(vec![
        deck_with("swamp", &["bone_splinters", "grizzly_bears"]),
        deck_with("mountain", &["hill_giant"]),
    ]);
    let mut e = GameEngine::new(5303, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "bone_splinters");
    let victim = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    e.state.players[0].mana_pool.black = 1;
    let spell_slot = hand_index_for_card(&e, 0, "bone_splinters");

    e.apply_command(
        0,
        &cast_spell_with_costs(
            spell_slot,
            target_object(victim),
            vec![permanent_cost_selection(0, victim)],
        ),
    )
    .expect("target legality is checked before the same creature is sacrificed");
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.stack.is_empty());
}

#[test]
fn insufficient_mana_leaves_every_nonmana_payment_uncommitted() {
    let decks = Some(vec![
        deck_with("swamp", &["village_rites", "grizzly_bears"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(5304, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "village_rites");
    let creature = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let spell_slot = hand_index_for_card(&e, 0, "village_rites");
    let hand_before = e.state.players[0].hand.clone();
    let stack_before = e.state.stack.len();
    let command_index_before = e.state.command_index;

    e.apply_command(
        0,
        &cast_spell_with_costs(
            spell_slot,
            vec![],
            vec![permanent_cost_selection(0, creature)],
        ),
    )
    .expect_err("mana is validated before any sacrifice is committed");
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(e.state.stack.len(), stack_before);
    assert_eq!(e.state.command_index, command_index_before);
}

#[test]
fn tormenting_voice_and_village_rites_pay_their_authored_costs() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &[
                "tormenting_voice",
                "village_rites",
                "grizzly_bears",
                "mountain",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(5305, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);

    ensure_card_in_hand(&mut e, 0, "tormenting_voice");
    ensure_card_in_hand(&mut e, 0, "mountain");
    let voice_slot = hand_index_for_card(&e, 0, "tormenting_voice");
    let discarded_slot = hand_index_for_card(&e, 0, "mountain");
    let discarded = e.state.players[0].hand[discarded_slot];
    e.state.players[0].mana_pool.red = 1;
    e.state.players[0].mana_pool.colorless = 1;
    e.apply_command(
        0,
        &cast_spell_with_costs(
            voice_slot,
            vec![],
            vec![hand_cost_selection(0, discarded_slot as u32)],
        ),
    )
    .unwrap();
    assert_eq!(e.state.objects[&discarded].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);

    ensure_card_in_hand(&mut e, 0, "village_rites");
    let creature = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let rites_slot = hand_index_for_card(&e, 0, "village_rites");
    e.state.players[0].mana_pool.black = 1;
    e.apply_command(
        0,
        &cast_spell_with_costs(
            rites_slot,
            vec![],
            vec![permanent_cost_selection(0, creature)],
        ),
    )
    .unwrap();
    assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
}

#[test]
fn missing_wrong_zone_opponent_and_stale_selections_are_rejected() {
    let make_engine = |seed| {
        let decks = Some(vec![
            deck_with("swamp", &["village_rites", "grizzly_bears"]),
            deck_with("mountain", &["hill_giant"]),
        ]);
        let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
        advance_to_main1_from_game_start(&mut e);
        ensure_card_in_hand(&mut e, 0, "village_rites");
        let own = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
        let opposing = relocate_to_battlefield(&mut e, 1, "hill_giant", false);
        e.state.players[0].mana_pool.black = 1;
        let slot = hand_index_for_card(&e, 0, "village_rites");
        (e, slot, own, opposing)
    };

    let (mut missing, slot, _, _) = make_engine(5306);
    missing
        .apply_command(0, &cast_spell_with_costs(slot, vec![], vec![]))
        .expect_err("selection is mandatory");

    let (mut duplicate, slot, own, _) = make_engine(5311);
    duplicate
        .apply_command(
            0,
            &cast_spell_with_costs(
                slot,
                vec![],
                vec![
                    permanent_cost_selection(0, own),
                    permanent_cost_selection(0, own),
                ],
            ),
        )
        .expect_err("duplicate component assignments are rejected");

    let (mut wrong_zone, slot, _, _) = make_engine(5307);
    wrong_zone
        .apply_command(
            0,
            &cast_spell_with_costs(slot, vec![], vec![hand_cost_selection(0, 0)]),
        )
        .expect_err("sacrifice selection must name a permanent");

    let (mut opponent, slot, _, opposing) = make_engine(5308);
    opponent
        .apply_command(
            0,
            &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, opposing)]),
        )
        .expect_err("opponent-controlled permanents cannot pay the cost");

    let (mut stale, slot, own, _) = make_engine(5309);
    stale.state.players[0].battlefield.retain(|oid| *oid != own);
    stale.state.players[0].graveyard.push(own);
    stale.state.objects.get_mut(&own).unwrap().zone = Zone::Graveyard;
    stale
        .apply_command(
            0,
            &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, own)]),
        )
        .expect_err("stale battlefield identities are rejected");
}

#[test]
fn command_path_rejects_discarding_the_spell_itself() {
    let decks = Some(vec![
        deck_with("mountain", &["thrill_of_possibility"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(5312, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "thrill_of_possibility");
    let slot = hand_index_for_card(&e, 0, "thrill_of_possibility");
    e.state.players[0].mana_pool.red = 1;
    e.state.players[0].mana_pool.colorless = 1;
    e.apply_command(
        0,
        &cast_spell_with_costs(slot, vec![], vec![hand_cost_selection(0, slot as u32)]),
    )
    .expect_err("the source object is excluded independently of legal-action publication");
}

#[test]
fn sacrifice_spell_is_not_published_without_a_matching_permanent() {
    let decks = Some(vec![
        deck_with("swamp", &["village_rites"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(5313, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "village_rites");
    let swamp = relocate_to_battlefield(&mut e, 0, "swamp", false);
    let batch = e
        .apply_command(0, &activate_ability(swamp, 0, vec![]))
        .unwrap();
    assert!(
        !batch.legal_by_player[&0].hand_actions.iter().any(|action| {
            action.kind == HandActionKind::HandActionCastSpell as i32
                && action.card_name == "Village Rites"
        })
    );
}

#[test]
fn sacrifice_dies_trigger_is_stacked_above_the_new_spell_with_lki() {
    let decks = Some(vec![
        deck_with("swamp", &["village_rites", "highland_game"]),
        deck_with("mountain", &[]),
    ]);
    let mut e = GameEngine::new(5310, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "village_rites");
    let elk = relocate_to_battlefield(&mut e, 0, "highland_game", false);
    let rites_slot = hand_index_for_card(&e, 0, "village_rites");
    e.state.players[0].mana_pool.black = 1;

    let batch = e
        .apply_command(
            0,
            &cast_spell_with_costs(rites_slot, vec![], vec![permanent_cost_selection(0, elk)]),
        )
        .unwrap();
    assert!(batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log))
                if log.text == "P0 casts Village Rites sacrificing Highland Game"
        )
    }));
    assert_eq!(e.state.stack.len(), 2);
    let top = e.state.stack.last().unwrap();
    assert_eq!(top.source_permanent_id, Some(elk));
    assert_eq!(
        top.ability_text.as_deref(),
        Some("Highland Game — triggered ability (triggered_01)")
    );
}
