use crate::helpers::*;

#[test]
fn cast_divination_draws_two_cards() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "divination".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(901, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    for _ in 0..2 {
        let seeded_island_idx = hand_index_for_card(&e, 0, "island");
        let seeded_island = e.state.players[0].hand.remove(seeded_island_idx);
        e.state.players[0].battlefield.push(seeded_island);
        e.state
            .objects
            .get_mut(&seeded_island)
            .expect("seeded island")
            .zone = tricerules_core::Zone::Battlefield;
    }

    let island_to_play_idx = hand_index_for_card(&e, 0, "island");
    e.apply_command(0, &play_land(island_to_play_idx))
        .expect("play third island");

    let hand_before_cast = e.state.players[0].hand.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let div_idx = hand_index_for_card(&e, 0, "divination");
    e.apply_command(0, &cast_spell(div_idx, vec![]))
        .expect("cast divination");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast + 1,
        "cast consumes one card and draws two"
    );
}

#[test]
fn go_for_the_throat_destroys_target_creature() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "go_for_the_throat".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(904, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let p1_bear = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    let seeded_swamp_idx = hand_index_for_card(&e, 0, "swamp");
    let seeded_swamp = e.state.players[0].hand.remove(seeded_swamp_idx);
    e.state.players[0].battlefield.push(seeded_swamp);
    e.state
        .objects
        .get_mut(&seeded_swamp)
        .expect("seeded swamp")
        .zone = tricerules_core::Zone::Battlefield;

    let swamp_to_play_idx = hand_index_for_card(&e, 0, "swamp");
    e.apply_command(0, &play_land(swamp_to_play_idx))
        .expect("play second swamp");

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let gftt_idx = hand_index_for_card(&e, 0, "go_for_the_throat");
    e.apply_command(
        0,
        &cast_spell(gftt_idx, vec![TargetRef { object_id: p1_bear }]),
    )
    .expect("cast go for the throat");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    assert!(e.state.players[1].graveyard.contains(&p1_bear));
    assert_eq!(
        e.state
            .objects
            .get(&p1_bear)
            .expect("target creature object")
            .zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn healing_salve_gains_three_life_for_target_player() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "healing_salve".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2601, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );

    let salve_idx = hand_index_for_card(&e, 0, "healing_salve");
    let p1_life_before = e.state.players[1].life;
    e.apply_command(0, &cast_spell(salve_idx, target_player(1)))
        .expect("cast salve targeting opponent");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };

    assert_eq!(
        e.state.players[1].life,
        p1_life_before + 3,
        "target player (P1) gains 3"
    );
    let life = life_changes_in(&batch);
    assert!(
        life.iter()
            .any(|lc| lc.player_id == 1 && lc.delta == 3 && lc.new_total == p1_life_before + 3),
        "LifeChanged event expected, got {life:?}"
    );
}

#[test]
fn healing_salve_can_target_controller() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "healing_salve".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2602, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let salve_idx = hand_index_for_card(&e, 0, "healing_salve");
    let p0_life_before = e.state.players[0].life;
    e.apply_command(0, &cast_spell(salve_idx, target_player(0)))
        .expect("salve may target controller");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    assert_eq!(e.state.players[0].life, p0_life_before + 3);
}

#[test]
fn angels_mercy_gains_seven_life_for_controller() {
    let mut p0_deck = vec!["angels_mercy".into()];
    for _ in 0..6 {
        p0_deck.push("plains".into());
    }
    let decks = Some(vec![p0_deck, forest_only_deck()]);
    let mut e = GameEngine::new(2603, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            c: 3,
            ..Default::default()
        },
    );
    let mercy_idx = hand_index_for_card(&e, 0, "angels_mercy");
    let life_before = e.state.players[0].life;
    e.apply_command(0, &cast_spell(mercy_idx, vec![]))
        .expect("cast mercy");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    assert_eq!(e.state.players[0].life, life_before + 7, "mercy gains 7");
}

#[test]
fn bump_in_the_night_drains_three_from_target_player() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "bump_in_the_night".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2604, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let bump_idx = hand_index_for_card(&e, 0, "bump_in_the_night");
    let p1_life_before = e.state.players[1].life;
    let p0_life_before = e.state.players[0].life;
    e.apply_command(0, &cast_spell(bump_idx, target_player(1)))
        .expect("cast bump");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(e.state.players[1].life, p1_life_before - 3);
    assert_eq!(
        e.state.players[0].life, p0_life_before,
        "controller unaffected"
    );
    let life = life_changes_in(&batch);
    assert!(
        life.iter().any(|lc| lc.player_id == 1 && lc.delta == -3),
        "LifeChanged(-3) on P1 expected, got {life:?}"
    );
}

#[test]
fn blood_tithe_drains_each_opponent_and_gains_controller_equal_life() {
    let mut p0_deck = vec!["blood_tithe".into()];
    for _ in 0..6 {
        p0_deck.push("swamp".into());
    }
    let decks = Some(vec![p0_deck, forest_only_deck()]);
    let mut e = GameEngine::new(2606, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let tithe_idx = hand_index_for_card(&e, 0, "blood_tithe");
    let p1_life_before = e.state.players[1].life;
    let p0_life_before = e.state.players[0].life;
    e.apply_command(0, &cast_spell(tithe_idx, vec![]))
        .expect("cast tithe");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(e.state.players[1].life, p1_life_before - 3);
    assert_eq!(
        e.state.players[0].life,
        p0_life_before + 3,
        "controller gains 3 (life lost from one opponent)"
    );
    let life = life_changes_in(&batch);
    assert!(
        life.iter().any(|lc| lc.player_id == 0 && lc.delta == 3),
        "expected +3 LifeChanged on controller, got {life:?}"
    );
}

#[test]
fn eyeblights_ending_destroys_target_creature() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "eyeblights_ending".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2607, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "eyeblights_ending");
    e.apply_command(0, &cast_spell(idx, vec![TargetRef { object_id: bear }]))
        .expect("cast eyeblight");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(
        e.state.objects.get(&bear).expect("bear").zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(e.state.players[1].graveyard.contains(&bear));
    assert!(!e.state.players[1].battlefield.contains(&bear));
    let moves = permanents_moved_in(&batch);
    assert!(
        moves.iter().any(|m| m.object_id == bear
            && m.destination
                == tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32),
        "expected PermanentMoved(Graveyard) for bear, got {moves:?}"
    );
}

#[test]
fn swords_to_plowshares_exiles_and_gains_life_equal_to_power() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "swords_to_plowshares".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2608, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let bear_power = e.state.objects.get(&bear).unwrap().power.unwrap();
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "swords_to_plowshares");
    let p1_life_before = e.state.players[1].life;
    e.apply_command(0, &cast_spell(idx, vec![TargetRef { object_id: bear }]))
        .expect("cast swords");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(
        e.state.objects.get(&bear).expect("bear").zone,
        tricerules_core::Zone::Exile
    );
    // Lifegain accrues to the creature's controller (P1), per Swords' Oracle text.
    assert_eq!(
        e.state.players[1].life,
        p1_life_before + bear_power as i32,
        "controller of exiled creature gains life equal to its power"
    );
}

#[test]
fn unsummon_returns_target_creature_to_owner_hand() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "unsummon".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2610, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "unsummon");
    let p1_hand_before = e.state.players[1].hand.len();
    e.apply_command(0, &cast_spell(idx, vec![TargetRef { object_id: bear }]))
        .expect("cast unsummon");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(
        e.state.objects.get(&bear).expect("bear").zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(e.state.players[1].hand.len(), p1_hand_before + 1);
    assert!(!e.state.players[1].battlefield.contains(&bear));
    let moves = permanents_moved_in(&batch);
    assert!(
        moves.iter().any(|m| m.object_id == bear
            && m.destination
                == tricerules_proto::ruled::v1::permanent_moved::Destination::Hand as i32),
        "expected PermanentMoved(Hand) for bear, got {moves:?}"
    );
}

#[test]
fn boomerang_returns_target_land_to_owner_hand() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "boomerang".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(2612, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let island_idx = hand_index_for_card(&e, 0, "island");
    e.apply_command(0, &play_land(island_idx))
        .expect("play island");
    let island_oid = battlefield_object_for_card(&e, 0, "island");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "boomerang");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: island_oid,
            }],
        ),
    )
    .expect("cast boomerang on own island");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    assert_eq!(
        e.state.objects.get(&island_oid).expect("island").zone,
        tricerules_core::Zone::Hand
    );
    assert!(!e.state.players[0].battlefield.contains(&island_oid));
}

#[test]
fn tome_scour_mills_five_cards_from_target_player() {
    let mut p1_deck = vec!["forest".into(); 30];
    // Sentinel cards at the top so we can assert ordering.
    p1_deck[0] = "grizzly_bears".into();
    p1_deck[1] = "savannah_lions".into();
    p1_deck[2] = "coral_merfolk".into();
    p1_deck[3] = "walking_corpse".into();
    p1_deck[4] = "balduvian_barbarians".into();
    let decks = Some(vec![island_only_deck(), p1_deck]);
    let mut e = GameEngine::new(2613, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    // Place tome_scour in P0 hand directly to avoid deck ordering churn.
    take_card_from_library_to_hand(&mut e, 0, "island");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    // Inject the spell into hand from the registry.
    let scour_id = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
            },
        );
        e.state.players[0].hand.push(id);
        id
    };
    let scour_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == scour_id)
        .expect("scour in hand");
    let lib_before = e.state.players[1].library.len();
    let grave_before = e.state.players[1].graveyard.len();
    e.apply_command(0, &cast_spell(scour_idx, target_player(1)))
        .expect("cast tome scour");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(e.state.players[1].library.len(), lib_before - 5);
    assert_eq!(e.state.players[1].graveyard.len(), grave_before + 5);
    let moves = permanents_moved_in(&batch);
    let to_grave: Vec<_> = moves
        .iter()
        .filter(|m| {
            m.owner_player_id == 1
                && m.destination
                    == tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32
        })
        .collect();
    assert_eq!(to_grave.len(), 5, "five PermanentMoved->Graveyard events");
    assert!(
        to_grave.iter().all(|m| !m.card_id.is_empty()),
        "milled PermanentMoved events must carry card_id so servers can resolve library cards"
    );
}

#[test]
fn tome_scour_caps_at_library_size() {
    let mut p1_deck = vec!["forest".into(); 8];
    let decks = Some(vec![island_only_deck(), p1_deck.split_off(0)]);
    let mut e = GameEngine::new(2614, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    // Manually drain P1 library to 2 cards.
    while e.state.players[1].library.len() > 2 {
        let oid = e.state.players[1].library.pop_back().unwrap();
        e.state.players[1].graveyard.push(oid);
        if let Some(o) = e.state.objects.get_mut(&oid) {
            o.zone = tricerules_core::Zone::Graveyard;
        }
    }
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let scour_id = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
            },
        );
        e.state.players[0].hand.push(id);
        id
    };
    let scour_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == scour_id)
        .expect("scour in hand");
    e.apply_command(0, &cast_spell(scour_idx, target_player(1)))
        .expect("cast scour");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    // Library should be empty (only had 2 to mill), graveyard should hold both — engine must not panic.
    assert_eq!(e.state.players[1].library.len(), 0);
}

#[test]
fn tome_scour_can_target_controller() {
    // Tome Scour is Oracle "target player": milling yourself is legal.
    let decks = Some(vec![island_only_deck(), forest_only_deck()]);
    let mut e = GameEngine::new(2615, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let scour_id = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
            },
        );
        e.state.players[0].hand.push(id);
        id
    };
    let scour_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == scour_id)
        .expect("scour in hand");
    let lib_before = e.state.players[0].library.len();
    e.apply_command(0, &cast_spell(scour_idx, target_player(0)))
        .expect("tome scour targeting its controller is legal");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    // Five cards milled from the controller's own library (the resolved sorcery also lands
    // in the controller's graveyard, so assert the library side for an unambiguous count).
    assert_eq!(e.state.players[0].library.len(), lib_before - 5);
}

#[test]
fn wrath_of_god_destroys_all_creatures_except_indestructible() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["grizzly_bears", "darksteel_myr", "wrath_of_god"],
        ),
        deck_with("plains", &["savannah_lions"]),
    ]);
    let mut e = GameEngine::new(7200, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let myr = relocate_to_battlefield(&mut e, 0, "darksteel_myr", false);
    let lions = relocate_to_battlefield(&mut e, 1, "savannah_lions", false);
    relocate_to_hand(&mut e, 0, "wrath_of_god");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 4,
            ..Default::default()
        },
    );
    let wrath_idx = hand_index_for_card(&e, 0, "wrath_of_god");
    e.apply_command(0, &cast_spell(wrath_idx, vec![]))
        .expect("cast wrath of god");
    resolve_entire_stack_two_player(&mut e);

    assert!(
        e.state.players[0].graveyard.contains(&bears),
        "grizzly bears destroyed"
    );
    assert!(
        e.state.players[1].graveyard.contains(&lions),
        "savannah lions destroyed"
    );
    // CR 702.12b: an indestructible creature survives "destroy all creatures".
    assert!(
        e.state.players[0].battlefield.contains(&myr),
        "indestructible Darksteel Myr survives"
    );
}

#[test]
fn pyroclasm_deals_two_damage_to_each_creature() {
    let decks = Some(vec![
        deck_with("mountain", &["grizzly_bears", "giant_spider", "pyroclasm"]),
        deck_with("mountain", &["savannah_lions"]),
    ]);
    let mut e = GameEngine::new(7300, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false); // 2/2 -> dies
    let spider = relocate_to_battlefield(&mut e, 0, "giant_spider", false); // 2/4 -> survives
    let lions = relocate_to_battlefield(&mut e, 1, "savannah_lions", false); // 2/1 -> dies
    relocate_to_hand(&mut e, 0, "pyroclasm");

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "pyroclasm");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast pyroclasm");
    resolve_entire_stack_two_player(&mut e);

    // State-based actions destroy creatures with lethal damage (CR 704.5g).
    assert!(
        e.state.players[0].graveyard.contains(&bears),
        "2-toughness creature dies"
    );
    assert!(
        e.state.players[1].graveyard.contains(&lions),
        "1-toughness creature dies"
    );
    // Giant Spider (toughness 4) survives, marked with 2 damage until cleanup.
    assert!(
        e.state.players[0].battlefield.contains(&spider),
        "4-toughness creature survives"
    );
    assert_eq!(e.state.objects.get(&spider).expect("spider").damage, 2);
}

// ── Lightning Strike ──────────────────────────────────────────────────────────

#[test]
fn lightning_strike_deals_three_damage_to_creature() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_strike"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(91025, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    relocate_to_hand(&mut e, 0, "lightning_strike");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "lightning_strike");
    e.apply_command(0, &cast_spell(idx, vec![TargetRef { object_id: bear }]))
        .expect("cast lightning strike targeting creature");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    // Grizzly Bears is 2/2; 3 damage is lethal — SBA destroys it.
    assert!(e.state.players[1].graveyard.contains(&bear));
    assert_eq!(
        e.state.objects.get(&bear).expect("bear").zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn lightning_strike_deals_three_damage_to_player() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_strike"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(91026, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    relocate_to_hand(&mut e, 0, "lightning_strike");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let p1_life_before = e.state.players[1].life;
    let idx = hand_index_for_card(&e, 0, "lightning_strike");
    e.apply_command(0, &cast_spell(idx, target_player(1)))
        .expect("cast lightning strike targeting opponent");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(e.state.players[1].life, p1_life_before - 3);
    let life = life_changes_in(&batch);
    assert!(
        life.iter().any(|lc| lc.player_id == 1 && lc.delta == -3),
        "LifeChanged(-3) on P1 expected, got {life:?}"
    );
}

#[test]
fn lightning_strike_rejects_land_target() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_strike"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(91027, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    relocate_to_hand(&mut e, 0, "lightning_strike");
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let land_oid = battlefield_object_for_card(&e, 0, "mountain");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "lightning_strike");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: land_oid,
            }],
        ),
    )
    .expect_err("lightning strike cannot target a land");
}

/// Regression: a Draw spell that empties the library must NOT error out of resolution (the old
/// `draw_card(...)?` aborted mid-resolution and left the stack half-mutated). CR 120.3 / 104.3c:
/// draw as many as possible, then the player loses as a state-based action.
#[test]
fn draw_spell_decking_out_loses_without_erroring() {
    let decks = Some(vec![
        vec![
            "divination".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec!["island".into(); 7],
    ]);
    let mut e = GameEngine::new(2025, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Seven-card deck => the whole deck is in hand and the library is empty. Move one card back
    // so Divination (draw 2) draws exactly one, then runs the library dry on the second draw.
    let island_idx = hand_index_for_card(&e, 0, "island");
    let island_oid = e.state.players[0].hand.remove(island_idx);
    e.state.players[0].library.push_back(island_oid);
    e.state
        .objects
        .get_mut(&island_oid)
        .expect("seeded island")
        .zone = tricerules_core::Zone::Library;
    assert_eq!(e.state.players[0].library.len(), 1);

    // Pay {2}{U} and cast Divination.
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let div_idx = hand_index_for_card(&e, 0, "divination");
    e.apply_command(0, &cast_spell(div_idx, vec![]))
        .expect("p0 cast divination");

    // Resolving must succeed even though the library runs dry partway through the draw.
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass resolves divination (must not error)");

    assert!(e.state.players[0].library.is_empty(), "library drawn dry");
    assert!(
        e.state.players[0].has_lost,
        "P0 attempted to draw from an empty library and loses (CR 104.3c)"
    );
    assert_eq!(e.state.winner, Some(1), "P1 wins once P0 decks out");
}
