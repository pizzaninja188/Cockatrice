use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn put_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let objects: Vec<_> = card_ids
        .iter()
        .map(|card_id| take_oid_from_library_or_hand(e, player, card_id))
        .collect();
    for &oid in objects.iter().rev() {
        e.state.players[player].library.push_front(oid);
        e.state.objects.get_mut(&oid).expect("object").zone = Zone::Library;
    }
    objects
}

#[test]
fn gorging_vulture_counts_only_creatures_milled_by_its_trigger() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &[
                "gorging_vulture",
                "gorging_vulture",
                "grizzly_bears",
                "rumbling_baloth",
                "forest",
                "island",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(9101, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let cast_oid = relocate_to_hand(&mut e, 0, "gorging_vulture");

    let older_creature = take_oid_from_library_or_hand(&mut e, 0, "grizzly_bears");
    e.state.players[0].graveyard.push(older_creature);
    e.state
        .objects
        .get_mut(&older_creature)
        .expect("object")
        .zone = Zone::Graveyard;
    let milled = put_on_top(
        &mut e,
        0,
        &["rumbling_baloth", "forest", "gorging_vulture", "island"],
    );

    let top_creature = milled[2];
    assert_ne!(cast_oid, top_creature);

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&e, 0, "gorging_vulture");
    let life_before = e.state.players[0].life;
    let library_before = e.state.players[0].library.len();
    let graveyard_before = e.state.players[0].graveyard.len();

    e.apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("cast Gorging Vulture");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.players[0].library.len(), library_before - 4);
    assert_eq!(
        e.state.players[0].graveyard.len(),
        graveyard_before + 4,
        "the ETB trigger mills exactly four cards"
    );
    assert_eq!(
        e.state.players[0].life,
        life_before + 2,
        "only the two creature cards milled this way count; the older graveyard creature does not"
    );
}

#[test]
fn gorging_vulture_mills_as_many_as_possible_and_counts_that_short_cohort() {
    let decks = Some(vec![
        deck_with("swamp", &["gorging_vulture", "rumbling_baloth", "forest"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(9102, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_hand(&mut e, 0, "gorging_vulture");
    put_on_top(&mut e, 0, &["rumbling_baloth", "forest"]);
    while e.state.players[0].library.len() > 2 {
        let oid = e.state.players[0].library.pop_back().expect("library card");
        e.state.players[0].graveyard.push(oid);
        e.state.objects.get_mut(&oid).expect("object").zone = Zone::Graveyard;
    }

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&e, 0, "gorging_vulture");
    let life_before = e.state.players[0].life;
    let graveyard_before = e.state.players[0].graveyard.len();
    e.apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("cast Gorging Vulture");
    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.players[0].library.is_empty());
    assert_eq!(e.state.players[0].graveyard.len(), graveyard_before + 2);
    assert_eq!(e.state.players[0].life, life_before + 1);
}

#[test]
fn milled_card_result_does_not_leak_to_a_later_resolution() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &[
                "gorging_vulture",
                "gorging_vulture",
                "grizzly_bears",
                "rumbling_baloth",
                "forest",
                "island",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(9103, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_hand(&mut e, 0, "gorging_vulture");
    relocate_to_hand(&mut e, 0, "gorging_vulture");
    put_on_top(
        &mut e,
        0,
        &["grizzly_bears", "rumbling_baloth", "forest", "island"],
    );

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let first = hand_index_for_card(&e, 0, "gorging_vulture");
    e.apply_command(0, &cast_spell(first, vec![]))
        .expect("cast first Gorging Vulture");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 22);

    put_on_top(&mut e, 0, &["swamp", "swamp", "swamp", "swamp"]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let second = hand_index_for_card(&e, 0, "gorging_vulture");
    e.apply_command(0, &cast_spell(second, vec![]))
        .expect("cast second Gorging Vulture");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.players[0].life, 22,
        "the second all-land mill must not reuse the first trigger's creature cohort"
    );
}
