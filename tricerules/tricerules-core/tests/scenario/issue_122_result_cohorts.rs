use crate::helpers::*;
use tricerules_core::Zone;

fn cast_grab_the_prize(discard_card: &str, seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &["grab_the_prize", discard_card]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "grab_the_prize");
    ensure_card_in_hand(&mut engine, 0, discard_card);
    let spell_slot = hand_index_for_card(&engine, 0, "grab_the_prize");
    let discard_slot = hand_index_for_card(&engine, 0, discard_card);
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 1;

    engine
        .apply_command(
            0,
            &cast_spell_with_costs(
                spell_slot,
                vec![],
                vec![hand_cost_selection(0, discard_slot as u32)],
            ),
        )
        .expect("Grab the Prize should accept its discard payment");
    engine
}

#[test]
fn grab_the_prize_uses_the_paid_cards_type_snapshot() {
    let mut nonland = cast_grab_the_prize("grizzly_bears", 12_201);
    let hand_after_payment = nonland.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut nonland);
    assert_eq!(nonland.state.players[0].hand.len(), hand_after_payment + 2);
    assert_eq!(nonland.state.players[1].life, 18);

    let mut land = cast_grab_the_prize("mountain", 12_202);
    let hand_after_payment = land.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut land);
    assert_eq!(land.state.players[0].hand.len(), hand_after_payment + 2);
    assert_eq!(land.state.players[1].life, 20);
}

#[test]
fn grab_the_prize_keeps_payment_results_for_copies_and_later_zone_changes() {
    let mut moved = cast_grab_the_prize("grizzly_bears", 12_205);
    let paid = moved.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|object_id| moved.state.objects[object_id].card_id == "grizzly_bears")
        .expect("paid card in graveyard");
    moved.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != paid);
    moved.state.players[0].exile.push(paid);
    moved.state.objects.get_mut(&paid).unwrap().zone = Zone::Exile;
    *moved.state.zone_change_generation.entry(paid).or_default() += 1;
    resolve_entire_stack_two_player(&mut moved);
    assert_eq!(moved.state.players[1].life, 18);

    let mut copied = cast_grab_the_prize("grizzly_bears", 12_206);
    inject_card_into_hand(&mut copied, 1, "twincast");
    give_mana(
        &mut copied,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let original = copied.state.stack.last().unwrap().id;
    copied.apply_command(0, &pass()).unwrap();
    let twincast = hand_index_for_card(&copied, 1, "twincast");
    copied
        .apply_command(
            1,
            &cast_spell(
                twincast,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: original,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .unwrap();
    resolve_entire_stack_two_player(&mut copied);
    assert_eq!(copied.state.players[0].life, 18);
    assert_eq!(copied.state.players[1].life, 18);
}

fn cast_gerrards_verdict(cards: &[&str], seed: u64) -> (GameEngine, Vec<u32>) {
    let decks = Some(vec![
        deck_with("plains", &["gerrards_verdict"]),
        vec!["forest".into(); 20],
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let cleared: Vec<_> = engine.state.players[1].hand.drain(..).collect();
    engine.state.players[1].library.extend(cleared);
    let chosen = cards
        .iter()
        .map(|card| inject_card_into_hand(&mut engine, 1, card))
        .collect::<Vec<_>>();
    ensure_card_in_hand(&mut engine, 0, "gerrards_verdict");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            b: 1,
            ..Default::default()
        },
    );
    let spell_slot = hand_index_for_card(&engine, 0, "gerrards_verdict");
    engine
        .apply_command(0, &cast_spell(spell_slot, target_player(1)))
        .expect("cast Gerrard's Verdict");
    engine.apply_command(0, &pass()).unwrap();
    let parked = engine.apply_command(1, &pass()).unwrap();
    let choice = find_resolution_choice(&parked).expect("discard choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!((choice.min, choice.max), (2, 2));
    (engine, chosen)
}

#[test]
fn gerrards_verdict_counts_the_exact_discarded_land_cohort_after_resume() {
    let (mut one_land, chosen) = cast_gerrards_verdict(&["forest", "grizzly_bears"], 12_203);
    one_land
        .apply_command(1, &submit_resolution_choice(chosen))
        .expect("submit one land and one nonland");
    assert_eq!(one_land.state.players[0].life, 23);

    let (mut two_lands, chosen) = cast_gerrards_verdict(&["forest", "mountain"], 12_204);
    two_lands
        .apply_command(1, &submit_resolution_choice(chosen))
        .expect("submit two lands");
    assert_eq!(two_lands.state.players[0].life, 26);
}

#[test]
fn gerrards_verdict_rejects_a_stale_discard_choice_without_losing_the_prompt() {
    let (mut engine, chosen) = cast_gerrards_verdict(&["forest", "mountain"], 12_208);
    *engine
        .state
        .zone_change_generation
        .entry(chosen[0])
        .or_default() += 1;

    let error = engine
        .apply_command(1, &submit_resolution_choice(chosen))
        .expect_err("stale chosen identity must fail closed");
    assert!(error.to_string().contains("stale"));
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.players[0].life, 20);
}
