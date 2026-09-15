//! Issue #300 — the reviewed land-sacrifice draw cohort.
//!
//! CR 602.1/602.2 require the ordered mana and sacrifice costs to be paid before
//! an activated ability resolves. CR 701.21a restricts sacrifice to a permanent
//! controlled by the player paying the cost; the generated recipe's Land filter
//! is therefore exercised through the engine's published cost choices below.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, ChooseTriggerTarget, RuledCommand,
    TargetRefKind,
};

fn setup_ripchain(seed: u64) -> (GameEngine, u32, u32, u32) {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "ripchain_razorkin");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    let drawn = inject_library_card(&mut engine, 0, "forest");
    // The engine draws from the front of its library deque.  Put the fixture
    // card on top so the assertion below names the physical card drawn.
    let position = engine.state.players[0]
        .library
        .iter()
        .position(|oid| *oid == drawn)
        .expect("fixture card in library");
    let card = engine.state.players[0]
        .library
        .remove(position)
        .expect("fixture card position");
    engine.state.players[0].library.push_front(card);
    engine.state.players[0].mana_pool.colorless = 2;
    engine.state.players[0].mana_pool.red = 1;
    (engine, source, land, drawn)
}

fn three_player_main1(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != tricerules_core::TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance three-player turn");
    }
    engine
}

fn hand_ability(engine: &GameEngine, source: u32, ability_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            ..Default::default()
        })),
    }
}

fn choose_stack_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

#[test]
fn issue_300_ripchain_sacrifices_one_controlled_land_then_draws_one() {
    let (mut engine, source, land, drawn) = setup_ripchain(300_001);
    let hand_before = engine.state.players[0].hand.len();

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![permanent_cost_selection(1, land)],
            ),
        )
        .expect("pay mana then sacrifice the controlled land");

    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1, "ability waits on the stack");
    assert_eq!(
        engine.state.priority_player_id(),
        0,
        "active player receives the normal response window"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
}

#[test]
fn issue_300_no_controlled_land_publishes_no_payable_choice_and_fails_atomically() {
    let mut engine = GameEngine::new(300_002, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "ripchain_razorkin");
    engine.state.players[0].mana_pool.colorless = 2;
    engine.state.players[0].mana_pool.red = 1;

    let key = u64::from(source) << 32;
    let choices =
        &engine.initial_response_batch().legal_by_player[&0].cost_choices_by_ability[&key];
    let sacrifice = choices
        .choices
        .iter()
        .find(|choice| choice.cost_index == 1)
        .expect("sacrifice cost choice is published");
    assert!(sacrifice.candidate_ids.is_empty());
    assert!(!choices.non_mana_costs_payable);

    let mana_before = engine.state.players[0].mana_pool;
    engine
        .apply_command(0, &activate_ability_with_costs(source, 0, vec![], vec![]))
        .expect_err("the missing land makes activation illegal");
    assert_eq!(engine.state.players[0].mana_pool, mana_before);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_300_nonland_and_opponent_land_are_rejected_without_partial_payment() {
    let (mut engine, source, own_land, _) = setup_ripchain(300_003);
    let nonland = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let mana_before = engine.state.players[0].mana_pool;

    for invalid in [nonland, opponent_land] {
        engine
            .apply_command(
                0,
                &activate_ability_with_costs(
                    source,
                    0,
                    vec![],
                    vec![permanent_cost_selection(1, invalid)],
                ),
            )
            .expect_err("only one controlled land may pay this cost");
        assert_eq!(engine.state.players[0].mana_pool, mana_before);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&own_land].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&invalid].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn issue_300_cost_stays_paid_while_opponent_responds_before_draw_resolution() {
    let (mut engine, source, land, drawn) = setup_ripchain(300_004);
    let hand_before = engine.state.players[0].hand.len();

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![permanent_cost_selection(1, land)],
            ),
        )
        .expect("activate draw ability");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);

    engine
        .apply_command(0, &pass())
        .expect("controller passes response window");
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);

    engine
        .apply_command(1, &pass())
        .expect("opponent passes to resolve");
    assert_eq!(engine.state.stack.len(), 0);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
}

#[test]
fn issue_300_cost_stays_paid_when_source_leaves_before_resolution() {
    let (mut engine, source, land, drawn) = setup_ripchain(300_006);
    let hand_before = engine.state.players[0].hand.len();
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![permanent_cost_selection(1, land)],
            ),
        )
        .expect("pay the draw ability before responding");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(source)))
        .expect("respond by removing the ability source");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
}

#[test]
fn issue_300_countered_ability_keeps_paid_cost_and_does_not_draw() {
    let decks = Some(vec![
        deck_with("island", &["tishanas_tidebinder"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(300_007, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "ripchain_razorkin");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    let drawn = inject_library_card(&mut engine, 0, "forest");
    let position = engine.state.players[0]
        .library
        .iter()
        .position(|oid| *oid == drawn)
        .expect("fixture card in library");
    let card = engine.state.players[0]
        .library
        .remove(position)
        .expect("fixture card position");
    engine.state.players[0].library.push_front(card);
    ensure_in_hand(&mut engine, 0, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            r: 1,
            c: 3,
            ..Default::default()
        },
    );

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![permanent_cost_selection(1, land)],
            ),
        )
        .expect("pay the draw ability before countering it");
    let ability_id = engine.state.stack.last().expect("ability on stack").id;
    let tidebinder = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    engine
        .apply_command(0, &cast_spell(tidebinder, vec![]))
        .expect("cast Tishana's Tidebinder in response");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_stack_target(ability_id))
        .expect("target the exact activated ability");
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine.apply_command(first, &pass()).expect("first pass");
    let resolved = engine
        .apply_command(second, &pass())
        .expect("counter resolves");

    assert!(resolved.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability_id
    )));
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Library);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_300_seismic_mountaincycling_discards_from_hand_and_searches_revealed_mountain() {
    let decks = Some(vec![
        deck_with("forest", &["seismic_monstrosaur"]),
        deck_with("plains", &[]),
    ]);
    let mut engine = GameEngine::new(300_008, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "seismic_monstrosaur");
    let source =
        engine.state.players[0].hand[hand_index_for_card(&engine, 0, "seismic_monstrosaur")];
    let mountain = inject_library_card(&mut engine, 0, "mountain");
    let forest = inject_library_card(&mut engine, 0, "forest");
    engine.state.players[0]
        .library
        .retain(|oid| *oid != mountain);
    engine.state.players[0].library.push_front(mountain);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = hand_ability(&engine, source, 1);
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &command)
        .expect("activate Mountaincycling from hand");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].hand.len(), hand_before - 1);

    engine.apply_command(0, &pass()).expect("controller passes");
    let search = engine
        .apply_command(1, &pass())
        .expect("Mountaincycling resolves after the response window");
    let choice = find_resolution_choice(&search).expect("private revealed library search");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [mountain]);
    assert!(!choice.candidate_object_ids.contains(&forest));

    let completed = engine
        .apply_command(0, &submit_resolution_choice(vec![mountain]))
        .expect("choose the Mountain");
    assert_eq!(engine.state.objects[&mountain].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(completed.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::CardsRevealed(reveal))
            if reveal.cards.len() == 1 && reveal.cards[0].object_id == mountain
    )));
    assert!(completed.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
    )));
}

#[test]
fn issue_300_three_player_controller_pays_and_draws_for_foreign_owned_source() {
    let mut engine = three_player_main1(300_005);
    // P0 owns the creature while P2 controls it.  Both payment and the draw
    // must follow the activating controller (P2), not the card owner.
    let source = inject_creature_under_foreign_control(&mut engine, 0, 2, "ripchain_razorkin");
    let land = inject_permanent_on_battlefield(&mut engine, 2, "mountain");
    let drawn = inject_library_card(&mut engine, 2, "forest");
    let owner_hand_before = engine.state.players[0].hand.len();
    let position = engine.state.players[2]
        .library
        .iter()
        .position(|oid| *oid == drawn)
        .expect("fixture card in library");
    let card = engine.state.players[2]
        .library
        .remove(position)
        .expect("fixture card position");
    engine.state.players[2].library.push_front(card);
    engine.state.players[2].mana_pool.colorless = 2;
    engine.state.players[2].mana_pool.red = 1;

    engine
        .apply_command(0, &pass())
        .expect("P0 passes priority");
    engine
        .apply_command(1, &pass())
        .expect("P1 passes priority");
    engine
        .apply_command(
            2,
            &activate_ability_with_costs(
                source,
                0,
                vec![],
                vec![permanent_cost_selection(1, land)],
            ),
        )
        .expect("P2 controls and activates the source");
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[2].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[2].mana_pool.red, 0);

    while !engine.state.stack.is_empty() {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("three-player response pass");
    }
    assert_eq!(engine.state.players[2].hand.len(), 1);
    assert_eq!(engine.state.objects[&drawn].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].hand.len(), owner_hand_before);
}
