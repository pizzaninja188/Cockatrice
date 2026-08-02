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
            c: 3,
            ..Default::default()
        },
    );
    let gftt_idx = hand_index_for_card(&e, 0, "go_for_the_throat");
    e.apply_command(
        0,
        &cast_spell(
            gftt_idx,
            vec![TargetRef {
                object_id: p1_bear,
                damage_amount: 0,
            }],
        ),
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

/// A Healing Salve (3-shield) on a player fully absorbs a Lightning Bolt (3 damage);
/// the player's life is unchanged and the shield is exhausted.
#[test]
fn healing_salve_shield_fully_consumed_by_bolt() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "healing_salve".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2601, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cast Healing Salve on P1 → 3-damage shield.
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let salve_idx = hand_index_for_card(&e, 0, "healing_salve");
    e.apply_command(0, &cast_modal_spell(salve_idx, vec![(1, target_player(1))]))
        .expect("cast salve on P1");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass resolves salve");
    assert_eq!(
        e.state
            .damage_prevention_shields
            .get(&1u32)
            .copied()
            .unwrap_or(0),
        3,
        "3-point shield placed on P1"
    );
    let p1_life = e.state.players[1].life;

    // Lightning Bolt deals 3 to P1 → shield absorbs all 3, P1 life unchanged.
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt on P1");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass resolves bolt");
    assert_eq!(
        e.state.players[1].life, p1_life,
        "shield absorbs all 3 — P1 life unchanged"
    );
    assert_eq!(
        e.state
            .damage_prevention_shields
            .get(&1u32)
            .copied()
            .unwrap_or(0),
        0,
        "shield exhausted after absorbing 3 damage"
    );
}

/// Two Healing Salves (6-shield total) absorb a Lightning Bolt (3 damage); 3 shield points
/// remain — demonstrating partial shield consumption.
#[test]
fn healing_salve_double_shield_partially_consumed_by_bolt() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "healing_salve".into(),
            "healing_salve".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2602, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cast two Healing Salves on P1 → 6-point shield.
    for _ in 0..2 {
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let salve_idx = hand_index_for_card(&e, 0, "healing_salve");
        e.apply_command(0, &cast_modal_spell(salve_idx, vec![(1, target_player(1))]))
            .expect("cast salve");
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass resolves salve");
    }
    assert_eq!(
        e.state
            .damage_prevention_shields
            .get(&1u32)
            .copied()
            .unwrap_or(0),
        6,
        "6-point shield placed on P1 (two salves)"
    );
    let p1_life = e.state.players[1].life;

    // Lightning Bolt (3 damage) is absorbed; 3 shield remain.
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt on P1");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass resolves bolt");
    assert_eq!(
        e.state.players[1].life, p1_life,
        "P1 life unchanged — shield absorbed bolt"
    );
    assert_eq!(
        e.state
            .damage_prevention_shields
            .get(&1u32)
            .copied()
            .unwrap_or(0),
        3,
        "3 shield points remaining after absorbing 3 of 6"
    );
}

/// Fog prevents all combat damage that would be dealt this turn.
#[test]
fn fog_prevents_all_combat_damage() {
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        vec![
            "forest".into(),
            "fog".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(2603, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);

    // P0 attacks with the grizzly bears injected by advance_to_declare_attackers.
    let attacker = *e.state.players[0]
        .battlefield
        .last()
        .expect("bears on battlefield");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    // P0 passes — P1 has priority in DeclareAttackers.
    e.apply_command(0, &pass())
        .expect("p0 pass DeclareAttackers");
    // P1 casts Fog at instant speed before passing.
    give_mana(
        &mut e,
        1,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let fog_idx = hand_index_for_card(&e, 1, "fog");
    e.apply_command(1, &cast_spell(fog_idx, vec![]))
        .expect("P1 casts Fog");
    e.apply_command(1, &pass()).expect("p1 pass on stack");
    e.apply_command(0, &pass()).expect("p0 pass → Fog resolves");
    // Active player (P0) has priority in DeclareAttackers with empty stack.
    e.apply_command(0, &pass())
        .expect("p0 pass DeclareAttackers after Fog");
    // Both passed → no eligible P1 blockers → auto-declare empty blockers → DeclareBlockers, P0 has priority.
    e.apply_command(1, &pass())
        .expect("p1 pass → DeclareBlockers auto-advance");
    // Both pass in DeclareBlockers to resolve combat damage.
    e.apply_command(0, &pass())
        .expect("p0 pass DeclareBlockers");
    let b = e
        .apply_command(1, &pass())
        .expect("p1 pass → combat damage resolves");
    // Fog prevents all combat damage; P1 life must be 20.
    let life = life_changes_in(&b);
    assert!(
        life.is_empty() || life.iter().all(|lc| lc.delta >= 0),
        "Fog prevented all combat damage; no negative LifeChanged expected, got {life:?}"
    );
    assert_eq!(
        e.state.players[1].life, 20,
        "P1 took no damage thanks to Fog"
    );
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
            c: 3,
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
            c: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "eyeblights_ending");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
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
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
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
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
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
                damage_amount: 0,
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
                controller: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
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
                controller: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
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
                controller: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
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
fn tome_scour_self_mill_puts_the_spell_under_the_milled_cards() {
    // CR 608.2m: a sorcery is put into its owner's graveyard as the *final* part of its
    // resolution — after its own effects. So a self-targeted Tome Scour comes to rest beneath
    // the five cards it milled, not on top of them. Graveyard order is load-bearing for any
    // card that cares about it, so pin it here rather than only asserting membership.
    let decks = Some(vec![island_only_deck(), forest_only_deck()]);
    let mut e = GameEngine::new(2616, &[0, 1], 20, decks, true).expect("new");
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
                controller: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
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
    // Mill takes from the front of the library, so these are the five that will move, in order.
    let top_five: Vec<_> = e.state.players[0].library.iter().take(5).copied().collect();
    let grave_before = e.state.players[0].graveyard.len();

    e.apply_command(0, &cast_spell(scour_idx, target_player(0)))
        .expect("cast tome scour at self");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    let graveyard = &e.state.players[0].graveyard;
    assert_eq!(
        graveyard.len(),
        grave_before + 6,
        "five milled cards plus the spell itself"
    );
    assert_eq!(
        &graveyard[grave_before..grave_before + 5],
        top_five.as_slice(),
        "milled cards enter the graveyard in library order, ahead of the spell"
    );
    assert_eq!(
        graveyard.last(),
        Some(&scour_id),
        "CR 608.2m: the spell is placed as the final part of its own resolution"
    );
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

/// Disentomb (or Raise Dead) returns a creature card from the controller's graveyard to hand.
#[test]
fn disentomb_returns_creature_from_graveyard_to_hand() {
    let decks = Some(vec![vec!["swamp".into(); 10], vec!["forest".into(); 10]]);
    let mut e = GameEngine::new(1401, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Disentomb into P0's hand and Grizzly Bears into P0's graveyard.
    let bears_oid = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                controller: 0,
                card_id: "grizzly_bears".into(),
                zone: tricerules_core::Zone::Graveyard,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
            },
        );
        e.state.players[0].graveyard.push(id);
        id
    };
    let disentomb_oid = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                controller: 0,
                card_id: "disentomb".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
            },
        );
        e.state.players[0].hand.push(id);
        id
    };

    assert!(
        e.state.players[0].graveyard.contains(&bears_oid),
        "bears in graveyard"
    );

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let disentomb_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == disentomb_oid)
        .expect("disentomb in hand");
    let hand_before = e.state.players[0].hand.len();
    e.apply_command(
        0,
        &cast_spell(
            disentomb_idx,
            vec![TargetRef {
                object_id: bears_oid,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast disentomb");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass resolves disentomb");

    // Grizzly Bears should now be in P0's hand, not the graveyard.
    assert!(
        !e.state.players[0].graveyard.contains(&bears_oid),
        "bears must have left graveyard"
    );
    assert!(
        e.state.players[0].hand.contains(&bears_oid),
        "bears must be in P0's hand"
    );
    // Hand size: disentomb left (-1), bears returned (+1) → net unchanged.
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "net hand size unchanged"
    );
    assert_eq!(
        e.state.objects.get(&bears_oid).expect("obj").zone,
        tricerules_core::Zone::Hand
    );
}

/// ReturnFromGraveyard fizzles cleanly when the graveyard target is no longer legal at resolution
/// (e.g., the target was the only creature and the graveyard is now empty of valid targets).
#[test]
fn return_from_graveyard_fizzles_when_target_removed_before_resolution() {
    let decks = Some(vec![vec!["swamp".into(); 10], vec!["forest".into(); 10]]);
    let mut e = GameEngine::new(1402, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Disentomb directly into P0's hand.
    let disentomb_oid = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                controller: 0,
                card_id: "disentomb".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
            },
        );
        e.state.players[0].hand.push(id);
        id
    };

    // Inject a creature into P0's graveyard.
    let dummy_oid = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                controller: 0,
                card_id: "grizzly_bears".into(),
                zone: tricerules_core::Zone::Graveyard,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
            },
        );
        e.state.players[0].graveyard.push(id);
        id
    };

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let disentomb_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == disentomb_oid)
        .expect("disentomb in hand");
    e.apply_command(
        0,
        &cast_spell(
            disentomb_idx,
            vec![TargetRef {
                object_id: dummy_oid,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast disentomb");

    // Before resolution, move dummy OID off the graveyard (simulate exile). Exile, not the
    // battlefield: the battlefield lists are the control index, so parking a `Zone::Battlefield`
    // object outside every list builds a board the engine cannot represent (and the CR 704 SBA
    // invariant check rightly rejects). Either zone proves the same thing — the target is no
    // longer a legal graveyard target at resolution.
    let pos = e.state.players[0]
        .graveyard
        .iter()
        .position(|&oid| oid == dummy_oid)
        .unwrap();
    e.state.players[0].graveyard.remove(pos);
    e.state.players[0].exile.push(dummy_oid);
    if let Some(o) = e.state.objects.get_mut(&dummy_oid) {
        o.zone = tricerules_core::Zone::Exile;
    }

    // Resolution should fizzle gracefully (no panic, no error).
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass — fizzle");

    // The dummy OID must NOT be in hand (fizzle = no move).
    assert!(
        !e.state.players[0].hand.contains(&dummy_oid),
        "fizzled return must not move the card to hand"
    );
}

/// Gravedigger ETB trigger: entering the battlefield causes a targeted trigger that returns a
/// creature card from the controller's graveyard to hand.
#[test]
fn gravedigger_etb_trigger_returns_creature_from_graveyard() {
    let decks = Some(vec![vec!["swamp".into(); 10], vec!["forest".into(); 10]]);
    let mut e = GameEngine::new(1403, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Grizzly Bears into P0's graveyard and Gravedigger into P0's hand.
    let bears_oid = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                controller: 0,
                card_id: "grizzly_bears".into(),
                zone: tricerules_core::Zone::Graveyard,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
            },
        );
        e.state.players[0].graveyard.push(id);
        id
    };
    let gd_oid = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                controller: 0,
                card_id: "gravedigger".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
                attached_to: None,
                regeneration_shields: 0,
                must_attack_if_able: false,
                must_block_if_able: false,
                face_up_index: 0,
            },
        );
        e.state.players[0].hand.push(id);
        id
    };

    // Pay {3}{B} and cast Gravedigger.
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let gd_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == gd_oid)
        .expect("gravedigger in hand");
    e.apply_command(0, &cast_spell(gd_idx, vec![]))
        .expect("cast gravedigger");

    // Both players pass; Gravedigger resolves and ETB trigger fires.
    e.apply_command(0, &pass()).expect("p0 pass");
    let batch = e
        .apply_command(1, &pass())
        .expect("p1 pass resolves gravedigger");

    // Gravedigger ETB trigger requires a target from the graveyard.
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "Gravedigger ETB trigger must be queued"
    );

    // CR 603.3d: while the trigger is parked, its legal targets must be published under the same
    // (source_oid << 32 | ability_index) key an activated ability uses — that is what lets the
    // client highlight them and auto-open the graveyard they live in.
    {
        let pt = e
            .state
            .pending_triggers
            .front()
            .expect("pending trigger queued");
        let key = (pt.source_permanent_id as u64) << 32 | pt.ability_index as u64;
        let p0 = batch.legal_by_player.get(&0).expect("p0 legal actions");
        let targets = p0
            .valid_targets_by_ability
            .get(&key)
            .expect("a parked trigger must publish its valid targets");
        assert_eq!(
            targets.valid_graveyard_ids,
            vec![bears_oid],
            "the graveyard creature is the trigger's only legal target"
        );
        // Only the controller may answer the trigger, so nobody else is told what it can hit.
        let p1 = batch.legal_by_player.get(&1).expect("p1 legal actions");
        assert!(
            !p1.valid_targets_by_ability.contains_key(&key),
            "the opponent must not receive the trigger's target set"
        );
    }

    // P0 chooses Grizzly Bears as the graveyard target.
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: bears_oid,
            })),
        },
    )
    .expect("choose trigger target — bears from graveyard");

    // Trigger resolves: bears should move from graveyard to hand.
    e.apply_command(0, &pass())
        .expect("p0 pass trigger on stack");
    e.apply_command(1, &pass())
        .expect("p1 pass resolves trigger");

    assert!(
        !e.state.players[0].graveyard.contains(&bears_oid),
        "bears must have left graveyard after trigger"
    );
    assert!(
        e.state.players[0].hand.contains(&bears_oid),
        "bears must be in P0's hand after trigger"
    );
    assert_eq!(
        e.state.objects.get(&bears_oid).expect("obj").zone,
        tricerules_core::Zone::Hand
    );
    // Gravedigger itself should be on the battlefield.
    let gd_on_bf = e.state.players[0]
        .battlefield
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("gravedigger"));
    assert!(gd_on_bf, "Gravedigger must be on P0's battlefield");
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

// ── DiscardCards tests ─────────────────────────────────────────────────────────

fn inject_card_into_hand(e: &mut GameEngine, player: usize, player_id: i32, card_id: &str) -> u32 {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    e.state.objects.insert(
        id,
        tricerules_core::state::GameObject {
            id,
            owner: player_id,
            controller: player_id,
            card_id: card_id.to_string(),
            zone: tricerules_core::Zone::Hand,
            tapped: false,
            summoning_sick: false,
            power: None,
            toughness: None,
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
        },
    );
    e.state.players[player].hand.push(id);
    id
}

#[test]
fn hymn_to_tourach_discards_two_random_cards() {
    let decks = Some(vec![
        deck_with("swamp", &["hymn_to_tourach"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(3000, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject 3 specific cards into P1's hand so we have a controlled count.
    inject_card_into_hand(&mut e, 1, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 1, 1, "storm_crow");
    inject_card_into_hand(&mut e, 1, 1, "grizzly_bears");
    let hand_before = e.state.players[1].hand.len();
    let grave_before = e.state.players[1].graveyard.len();

    relocate_to_hand(&mut e, 0, "hymn_to_tourach");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    let hymn_idx = hand_index_for_card(&e, 0, "hymn_to_tourach");
    e.apply_command(0, &cast_spell(hymn_idx, target_player(1)))
        .expect("cast hymn");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass — hymn resolves");

    assert_eq!(
        e.state.players[1].hand.len(),
        hand_before - 2,
        "P1 discarded 2 cards"
    );
    assert_eq!(
        e.state.players[1].graveyard.len(),
        grave_before + 2,
        "2 cards in graveyard"
    );
}

#[test]
fn hymn_to_tourach_discards_all_when_hand_smaller_than_count() {
    let decks = Some(vec![
        deck_with("swamp", &["hymn_to_tourach"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(3001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Clear P1's hand and inject exactly 1 card so count(2) > hand_size(1).
    let cleared: Vec<_> = e.state.players[1].hand.drain(..).collect();
    e.state.players[1].library.extend(cleared);
    inject_card_into_hand(&mut e, 1, 1, "grizzly_bears");
    assert_eq!(e.state.players[1].hand.len(), 1);

    relocate_to_hand(&mut e, 0, "hymn_to_tourach");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    let hymn_idx = hand_index_for_card(&e, 0, "hymn_to_tourach");
    e.apply_command(0, &cast_spell(hymn_idx, target_player(1)))
        .expect("cast hymn");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass — hymn resolves");

    // CR 701.7a: if the player has fewer cards than count, they discard all.
    assert_eq!(
        e.state.players[1].hand.len(),
        0,
        "P1 discarded their only card"
    );
}

#[test]
fn hymn_to_tourach_empty_hand_is_no_op() {
    let decks = Some(vec![
        deck_with("swamp", &["hymn_to_tourach"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(3002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Empty P1's hand entirely.
    let cleared: Vec<_> = e.state.players[1].hand.drain(..).collect();
    e.state.players[1].library.extend(cleared);
    assert_eq!(e.state.players[1].hand.len(), 0);

    relocate_to_hand(&mut e, 0, "hymn_to_tourach");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    let hymn_idx = hand_index_for_card(&e, 0, "hymn_to_tourach");
    e.apply_command(0, &cast_spell(hymn_idx, target_player(1)))
        .expect("cast hymn");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass — hymn resolves (no-op)");

    assert_eq!(
        e.state.players[1].hand.len(),
        0,
        "P1 still has empty hand after no-op"
    );
}

#[test]
fn coercion_caster_chooses_which_card_to_discard() {
    let decks = Some(vec![
        deck_with("swamp", &["coercion"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(3003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Clear P1's hand and inject exactly 2 cards so the test is unambiguous.
    let cleared: Vec<_> = e.state.players[1].hand.drain(..).collect();
    e.state.players[1].library.extend(cleared);
    let bear_oid = inject_card_into_hand(&mut e, 1, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 1, 1, "storm_crow");
    assert_eq!(e.state.players[1].hand.len(), 2);

    relocate_to_hand(&mut e, 0, "coercion");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let coercion_idx = hand_index_for_card(&e, 0, "coercion");
    e.apply_command(0, &cast_spell(coercion_idx, target_player(1)))
        .expect("cast coercion");

    e.apply_command(0, &pass()).expect("p0 pass");
    let resolve_batch = e
        .apply_command(1, &pass())
        .expect("p1 pass — coercion parks for choice");

    // Engine should be waiting for P0 (caster) to choose.
    let choice_req =
        find_resolution_choice(&resolve_batch).expect("ResolutionChoiceRequired must be emitted");
    assert_eq!(choice_req.deciding_player_id, 0, "P0 (caster) decides");
    // ChoiceKind::OpponentHand: the target's hand is shown only to the caster (CR 701.7
    // "look at target player's hand"), so the relay redacts the candidates from everyone else.
    assert_eq!(
        choice_req.choice_kind(),
        ChoiceKind::OpponentHand,
        "discard-a-chosen-card reveals the hand privately to the caster, not publicly"
    );
    assert_eq!(choice_req.min, 1);
    assert_eq!(choice_req.max, 1);
    assert!(
        choice_req.candidate_object_ids.contains(&bear_oid),
        "bear is a candidate"
    );

    // P0 picks the grizzly_bears.
    e.apply_command(0, &submit_resolution_choice(vec![bear_oid]))
        .expect("P0 submits choice");

    assert_eq!(
        e.state.players[1].hand.len(),
        1,
        "P1 has 1 card left (storm_crow)"
    );
    assert!(
        e.state.players[1].graveyard.contains(&bear_oid),
        "grizzly_bears is in P1 graveyard"
    );
}
// ---------------------------------------------------------------------------
// ReturnFromGraveyard -> Battlefield (Zombify)

/// Zombify returns a creature card from its controller's own graveyard onto the battlefield.
/// Owner == controller here, so this exercises the reanimation path without any control change:
/// the creature enters, fires its ETB trigger (CR 603.6), and is summoning sick (CR 302.6).
#[test]
fn zombify_returns_creature_from_graveyard_to_battlefield() {
    let decks = Some(vec![
        deck_with("swamp", &["zombify"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(3020, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Elvish Visionary's ETB draws a card, so the trigger is observable as a hand-size change.
    let visionary = inject_graveyard_card(&mut e, 0, "elvish_visionary");
    relocate_to_hand(&mut e, 0, "zombify");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "zombify");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: visionary,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast zombify");
    let hand_before_resolution = e.state.players[0].hand.len();
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass — zombify resolves");

    assert!(
        !e.state.players[0].graveyard.contains(&visionary),
        "the creature left the graveyard"
    );
    assert!(
        e.state.players[0].battlefield.contains(&visionary),
        "the creature is on its controller's battlefield"
    );
    assert_eq!(
        e.state.objects.get(&visionary).expect("obj").zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(
        e.state.objects.get(&visionary).expect("obj").summoning_sick,
        "CR 302.6: a reanimated creature is summoning sick"
    );

    // The ETB trigger uses the stack, so resolve it before checking the draw.
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_resolution + 1,
        "the ETB trigger drew a card"
    );
}

/// A noncreature card in the graveyard is not a legal Zombify target
/// (`GraveyardCardType::Creature`), and the engine rejects the cast rather than fizzling later.
#[test]
fn zombify_cannot_target_a_noncreature_graveyard_card() {
    let decks = Some(vec![
        deck_with("swamp", &["zombify"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(3021, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let sorcery = inject_graveyard_card(&mut e, 0, "divination");
    relocate_to_hand(&mut e, 0, "zombify");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "zombify");
    let err = e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: sorcery,
                damage_amount: 0,
            }],
        ),
    );
    assert!(
        err.is_err(),
        "a noncreature graveyard card is an illegal target, got {err:?}"
    );
}

/// A creature card in the *opponent's* graveyard is not a legal target for
/// `GraveyardOwner::Controller` — Zombify reads "your graveyard".
#[test]
fn zombify_cannot_target_an_opponents_graveyard() {
    let decks = Some(vec![
        deck_with("swamp", &["zombify"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(3022, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let their_bear = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    relocate_to_hand(&mut e, 0, "zombify");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "zombify");
    let err = e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: their_bear,
                damage_amount: 0,
            }],
        ),
    );
    assert!(
        err.is_err(),
        "another player's graveyard is out of range for owner: Controller,
 controller: Controller, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// LoseLife (Thoughtseize)

/// Thoughtseize costs its caster 2 life on resolution (CR 118) in its printed order: *after* the
/// discard, which suspends resolution for the caster's choice.
///
/// This is the regression test for `docs/issues.md` #36 — a suspending effect used to end the
/// resolution outright, silently dropping every effect after it in the list. Thoughtseize's RON
/// was reordered to dodge that; it is back in Oracle order, so the `LoseLife` here only happens if
/// `complete_parked_resolution` really does resume the tail.
#[test]
fn thoughtseize_caster_loses_two_life() {
    let decks = Some(vec![
        deck_with("swamp", &["thoughtseize"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(3010, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Two cards in P1's hand so the choice is a real one and the resolution parks.
    let cleared: Vec<_> = e.state.players[1].hand.drain(..).collect();
    e.state.players[1].library.extend(cleared);
    let bear_oid = inject_card_into_hand(&mut e, 1, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 1, 1, "storm_crow");

    relocate_to_hand(&mut e, 0, "thoughtseize");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let life_before = e.state.players[0].life;
    let opponent_life_before = e.state.players[1].life;

    let idx = hand_index_for_card(&e, 0, "thoughtseize");
    e.apply_command(0, &cast_spell(idx, target_player(1)))
        .expect("cast thoughtseize");
    e.apply_command(0, &pass()).expect("p0 pass");
    let resolve_batch = e
        .apply_command(1, &pass())
        .expect("p1 pass — thoughtseize parks for the discard choice");

    // The life loss comes after the suspending discard, so it has *not* happened yet while the
    // engine waits for the choice.
    assert!(
        e.state.pending_resolution.is_some(),
        "resolution parks for the discard choice"
    );
    assert_eq!(
        e.state.players[0].life, life_before,
        "no life is lost before the choice is answered"
    );
    assert!(
        life_changes_in(&resolve_batch).is_empty(),
        "no LifeChanged event yet"
    );
    assert_eq!(
        e.state.players[1].life, opponent_life_before,
        "the target player's life is untouched"
    );

    let resume_batch = e
        .apply_command(0, &submit_resolution_choice(vec![bear_oid]))
        .expect("P0 submits choice");

    assert!(
        e.state.players[1].graveyard.contains(&bear_oid),
        "the chosen card is discarded"
    );
    // #36: the effect after the suspending one runs on resume.
    assert_eq!(
        e.state.players[0].life,
        life_before - 2,
        "caster loses 2 life once the parked resolution resumes"
    );
    assert!(
        life_changes_in(&resume_batch)
            .iter()
            .any(|lc| lc.player_id == 0 && lc.delta == -2),
        "a LifeChanged(-2) event reaches the clients on the resume batch"
    );
    assert_eq!(
        e.state.players[1].life, opponent_life_before,
        "the target player's life is still untouched"
    );
}

/// #36, the straight-through half: a caster-chooses `DiscardCards` against an *empty* hand does
/// not park at all, so the life loss must still happen on the same batch. The two paths were
/// order-of-magnitude inconsistent for the same card before the fix — this one kept its second
/// effect while the parking path above silently dropped it.
#[test]
fn thoughtseize_loses_life_even_when_the_discard_does_not_park() {
    let decks = Some(vec![
        deck_with("swamp", &["thoughtseize"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(3011, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Empty P1 hand: nothing to choose from, so the discard resolves without suspending.
    let cleared: Vec<_> = e.state.players[1].hand.drain(..).collect();
    e.state.players[1].library.extend(cleared);

    relocate_to_hand(&mut e, 0, "thoughtseize");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let life_before = e.state.players[0].life;

    let idx = hand_index_for_card(&e, 0, "thoughtseize");
    e.apply_command(0, &cast_spell(idx, target_player(1)))
        .expect("cast thoughtseize");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass — resolves");

    assert!(
        e.state.pending_resolution.is_none(),
        "an empty hand offers no choice, so nothing parks"
    );
    assert_eq!(
        e.state.players[0].life,
        life_before - 2,
        "caster loses 2 life"
    );
}

// ---------------------------------------------------------------------------
// TargetPlayerSacrifices (Diabolic Edict)
// ---------------------------------------------------------------------------

#[test]
fn diabolic_edict_forces_target_player_to_sacrifice_a_creature() {
    let decks = Some(vec![
        {
            let mut d = vec!["diabolic_edict".to_string(), "swamp".to_string()];
            d.extend(std::iter::repeat_n("swamp".to_string(), 28));
            d
        },
        {
            let mut d = vec!["grizzly_bears".to_string()];
            d.extend(std::iter::repeat_n("forest".to_string(), 29));
            d
        },
    ]);
    let mut e = GameEngine::new(9001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Give P0 mana to cast Diabolic Edict ({1}{B}) and put a bear on P1's board.
    let bear_id = deploy_to_battlefield(&mut e, 1, "grizzly_bears", false);
    ensure_in_hand(&mut e, 0, "diabolic_edict");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    let edict_idx = hand_index_for_card(&e, 0, "diabolic_edict");
    e.apply_command(0, &cast_spell(edict_idx, target_player(1)))
        .expect("P0 casts Diabolic Edict targeting P1");

    // Both players pass priority to let it resolve.
    e.apply_command(0, &pass()).expect("P0 pass");
    let batch = e.apply_command(1, &pass()).expect("P1 pass resolves edict");

    // Engine should emit a ResolutionChoiceRequired for P1 to pick a creature.
    let req = find_resolution_choice(&batch).expect("resolution choice required");
    assert_eq!(
        req.deciding_player_id, 1,
        "P1 must choose what to sacrifice"
    );
    assert_eq!(req.min, 1);
    assert_eq!(req.max, 1);
    assert!(
        req.candidate_object_ids.contains(&bear_id),
        "Grizzly Bears is a legal sacrifice"
    );
    assert!(e.state.pending_resolution.is_some());

    // P1 sacrifices the bear.
    let result = e
        .apply_command(1, &submit_resolution_choice(vec![bear_id]))
        .expect("P1 submits sacrifice choice");

    assert!(
        e.state.pending_resolution.is_none(),
        "resolution complete after choice"
    );
    assert!(
        e.state.players[1].battlefield.is_empty(),
        "bear was sacrificed off the battlefield"
    );
    assert_eq!(
        e.state.players[1].graveyard.len(),
        1,
        "bear moved to P1 graveyard"
    );
    // PermanentMoved event should have been emitted.
    let moved = result.events.iter().any(|ev| {
        matches!(&ev.ev, Some(tricerules_proto::ruled::v1::ruled_event::Ev::PermanentMoved(m))
            if m.object_id == bear_id
            && m.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32)
    });
    assert!(
        moved,
        "PermanentMoved(Graveyard) event emitted for the bear"
    );
}

#[test]
fn diabolic_edict_fizzles_when_target_has_no_creatures() {
    let decks = Some(vec![
        {
            let mut d = vec!["diabolic_edict".to_string()];
            d.extend(std::iter::repeat_n("swamp".to_string(), 29));
            d
        },
        std::iter::repeat_n("forest".to_string(), 30).collect(),
    ]);
    let mut e = GameEngine::new(9002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // P1 has no creatures on the battlefield.
    assert!(e.state.players[1].battlefield.is_empty());

    ensure_in_hand(&mut e, 0, "diabolic_edict");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    let edict_idx = hand_index_for_card(&e, 0, "diabolic_edict");
    e.apply_command(0, &cast_spell(edict_idx, target_player(1)))
        .expect("P0 casts Diabolic Edict");

    e.apply_command(0, &pass()).expect("P0 pass");
    let batch = e
        .apply_command(1, &pass())
        .expect("P1 pass resolves edict (fizzle)");

    // No ResolutionChoiceRequired — effect fizzled with no valid sacrifice.
    assert!(
        find_resolution_choice(&batch).is_none(),
        "no choice required: P1 has nothing to sacrifice"
    );
    assert!(
        e.state.pending_resolution.is_none(),
        "resolution finished immediately (fizzle)"
    );
    // P1's battlefield and graveyard are both empty.
    assert!(e.state.players[1].battlefield.is_empty());
    assert!(e.state.players[1].graveyard.is_empty());
}

#[test]
fn diabolic_edict_rejects_invalid_sacrifice_choice() {
    let decks = Some(vec![
        {
            let mut d = vec!["diabolic_edict".to_string()];
            d.extend(std::iter::repeat_n("swamp".to_string(), 29));
            d
        },
        {
            let mut d = vec!["grizzly_bears".to_string()];
            d.extend(std::iter::repeat_n("forest".to_string(), 29));
            d
        },
    ]);
    let mut e = GameEngine::new(9003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear_id = deploy_to_battlefield(&mut e, 1, "grizzly_bears", false);
    ensure_in_hand(&mut e, 0, "diabolic_edict");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    let edict_idx = hand_index_for_card(&e, 0, "diabolic_edict");
    e.apply_command(0, &cast_spell(edict_idx, target_player(1)))
        .expect("P0 casts Diabolic Edict");

    e.apply_command(0, &pass()).expect("P0 pass");
    let batch = e.apply_command(1, &pass()).expect("P1 pass resolves edict");
    assert!(find_resolution_choice(&batch).is_some());

    // P1 tries to submit an object_id that is not in the candidates (e.g. a forest land).
    let forest_id = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let bad_result = e.apply_command(1, &submit_resolution_choice(vec![forest_id]));
    assert!(
        bad_result.is_err(),
        "submitting a non-candidate object is rejected"
    );

    // Resolution is still pending; bear is still alive.
    assert!(
        e.state.pending_resolution.is_some(),
        "still pending after invalid choice"
    );
    assert!(e.state.players[1].battlefield.contains(&bear_id));

    // Now submit the correct choice — bear gets sacrificed.
    e.apply_command(1, &submit_resolution_choice(vec![bear_id]))
        .expect("valid sacrifice choice accepted");

    assert!(e.state.pending_resolution.is_none());
    assert!(!e.state.players[1].battlefield.contains(&bear_id));
}

#[test]
fn diabolic_edict_targeting_self_is_legal() {
    // Diabolic Edict says "target player" (not opponent), so casting it on yourself is legal.
    let decks = Some(vec![
        {
            let mut d = vec!["diabolic_edict".to_string(), "grizzly_bears".to_string()];
            d.extend(std::iter::repeat_n("swamp".to_string(), 28));
            d
        },
        std::iter::repeat_n("forest".to_string(), 30).collect(),
    ]);
    let mut e = GameEngine::new(9004, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear_id = deploy_to_battlefield(&mut e, 0, "grizzly_bears", false);
    ensure_in_hand(&mut e, 0, "diabolic_edict");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    let edict_idx = hand_index_for_card(&e, 0, "diabolic_edict");
    // Target P0 (self) — should be legal for AnyPlayer kind.
    e.apply_command(0, &cast_spell(edict_idx, target_player(0)))
        .expect("P0 targets self with Diabolic Edict");

    e.apply_command(0, &pass()).expect("P0 pass");
    let batch = e.apply_command(1, &pass()).expect("P1 pass resolves edict");

    let req = find_resolution_choice(&batch).expect("choice required");
    assert_eq!(
        req.deciding_player_id, 0,
        "P0 is the deciding player (self-target)"
    );

    e.apply_command(0, &submit_resolution_choice(vec![bear_id]))
        .expect("P0 sacrifices own bear");

    assert!(!e.state.players[0].battlefield.contains(&bear_id));
}

/// CR 615: a prevention shield applies to *any* damage from any source, not only to single-target
/// spell damage. Pyroclasm's `DamageAll` used to write straight into `o.damage` and walk past the
/// shield entirely.
#[test]
fn healing_salve_shield_absorbs_mass_damage() {
    let decks = Some(vec![
        deck_with("plains", &["healing_salve", "pyroclasm"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(2661, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Two 2/2s; only the shielded one survives Pyroclasm's 2 damage.
    let shielded = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let unshielded = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    ensure_in_hand(&mut e, 0, "healing_salve");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let salve_idx = hand_index_for_card(&e, 0, "healing_salve");
    e.apply_command(
        0,
        &cast_modal_spell(
            salve_idx,
            vec![(
                1,
                vec![TargetRef {
                    object_id: shielded,
                    damage_amount: 0,
                }],
            )],
        ),
    )
    .expect("cast salve on our own creature");
    pass_both_players(&mut e);

    ensure_in_hand(&mut e, 0, "pyroclasm");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let pyro_idx = hand_index_for_card(&e, 0, "pyroclasm");
    e.apply_command(0, &cast_spell(pyro_idx, vec![]))
        .expect("cast Pyroclasm");
    pass_both_players(&mut e);

    assert_eq!(
        e.state.objects.get(&shielded).expect("shielded").zone,
        tricerules_core::Zone::Battlefield,
        "the 3-point shield absorbs all 2 of Pyroclasm's damage"
    );
    assert_eq!(
        e.state.objects.get(&unshielded).expect("unshielded").zone,
        tricerules_core::Zone::Graveyard,
        "the unshielded 2/2 still dies"
    );
}

/// The same shield must apply to multi-target damage (Fire's two-way split), which took the same
/// unprotected write path as mass damage.
#[test]
fn healing_salve_shield_absorbs_multi_target_damage() {
    let decks = Some(vec![
        deck_with("plains", &["healing_salve", "fire_ice"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(2662, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    ensure_in_hand(&mut e, 0, "healing_salve");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let salve_idx = hand_index_for_card(&e, 0, "healing_salve");
    e.apply_command(0, &cast_modal_spell(salve_idx, vec![(1, target_player(1))]))
        .expect("shield P1");
    pass_both_players(&mut e);
    let p1_life = e.state.players[1].life;
    let p0_life = e.state.players[0].life;

    // Fire: 1 damage to P1 (shielded) and 1 to P0 (not shielded).
    ensure_in_hand(&mut e, 0, "fire_ice");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let fire_idx = hand_index_for_card(&e, 0, "fire_ice");
    e.apply_command(
        0,
        &cast_spell_face(fire_idx, targets_with_damage(vec![(1, 1), (0, 1)]), 0),
    )
    .expect("cast Fire split 1/1");
    pass_both_players(&mut e);

    assert_eq!(
        e.state.players[1].life, p1_life,
        "P1's shield absorbs its share of Fire's damage"
    );
    assert_eq!(
        e.state.players[0].life,
        p0_life - 1,
        "the unshielded half still lands"
    );
}
