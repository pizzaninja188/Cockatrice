//! Actual-card scenarios for the queue-v75 01-treasure-riders cohort.
//!
//! Expectations are printed Oracle values. CR 115.1/601.2c chooses targets at cast, CR 608.2b
//! fizzles a spell whose only chosen target is illegal, CR 701.8 destroys, CR 701.6 counters and
//! CR 120.3 deals damage; the Treasure token follows CR 111.10a/701.7 and is created by the
//! spell's controller. Full order follows CR 608.2c.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "deadly_derision",
            mana: ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "grim_bounty",
            mana: ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "improvised_weaponry",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "pirates_prize",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "hornswoggle",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        },
    ]
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["lightning_bolt", "grizzly_bears"]),
        deck_with("island", &["grizzly_bears"]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast(e: &mut GameEngine, player: usize, card: &str, mana: &ManaGift, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, player, card);
    give_mana(e, player as i32, gift(mana));
    let slot = hand_index_for_card(e, player, card);
    e.apply_scenario_command(player as i32, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("{card}: cast: {error:?}"));
    assert_eq!(
        pool(e, player),
        (0, 0, 0, 0, 0, 0),
        "{card}: exact printed cost"
    );
}

fn assert_rejected(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    mana: &ManaGift,
    targets: Vec<TargetRef>,
) {
    let object = inject_card_into_hand(e, player, card);
    give_mana(e, player as i32, gift(mana));
    let before = pool(e, player);
    let hand_before = e.state.players[player].hand.clone();
    let stack_before = e.state.stack.len();
    let slot = hand_index_for_card(e, player, card);
    let error = e
        .apply_scenario_command(player as i32, &cast_spell(slot, targets))
        .expect_err(&format!("{card}: cast must be rejected"));
    assert!(
        matches!(error, tricerules_core::EngineError::Illegal(_)),
        "{card}: expected an illegal-input error, got {error:?}"
    );
    assert_eq!(
        pool(e, player),
        before,
        "{card}: rejected cast spends nothing"
    );
    assert_eq!(
        e.state.players[player].hand, hand_before,
        "{card}: rejected cast keeps the card in hand"
    );
    assert_eq!(e.state.objects[&object].zone, Zone::Hand, "{card}");
    assert_eq!(
        e.state.stack.len(),
        stack_before,
        "{card}: rejected cast pushes nothing"
    );
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn q75_treasure_canary_simplest_improvised_weaponry_damages_and_makes_a_treasure() {
    let case = &cases()[2];
    let mut e = engine(75_100_001);
    let life = e.state.players[1].life;
    let opponent_hand = e.state.players[1].hand.clone();
    cast(&mut e, 0, case.id, &case.mana, target_player(1));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, life - 2, "exactly two damage");
    assert_eq!(e.state.players[0].life, 20, "the caster is not damaged");
    assert_eq!(
        battlefield_token_oids(&e, 0, "treasure").len(),
        1,
        "the caster creates exactly one Treasure"
    );
    assert!(
        battlefield_token_oids(&e, 1, "treasure").is_empty(),
        "the opponent gets no Treasure"
    );
    assert_eq!(e.state.players[1].hand, opponent_hand, "no hidden change");
    assert_eq!(e.state.stack.len(), 0);
}

#[test]
fn q75_treasure_canary_hardest_hornswoggle_counters_a_creature_spell() {
    let case = &cases()[4];
    let mut e = engine(75_100_002);
    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the creature spell");
    e.apply_scenario_command(0, &pass())
        .expect("pass to the opponent");
    let target = e.state.stack.last().expect("creature spell").id;
    cast(&mut e, 1, case.id, &case.mana, target_object(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Graveyard,
        "the countered creature spell is put into its owner's graveyard"
    );
    assert!(
        e.state.players[0].battlefield.is_empty(),
        "no creature enters the battlefield"
    );
    assert_eq!(
        battlefield_token_oids(&e, 1, "treasure").len(),
        1,
        "Hornswoggle's controller creates the Treasure"
    );
    assert!(battlefield_token_oids(&e, 0, "treasure").is_empty());
}

#[test]
fn q75_treasure_riders_apply_their_printed_bodies_and_exact_costs() {
    // Deadly Derision destroys a creature and creates a Treasure.
    let case = &cases()[0];
    let mut e = engine(75_101_001);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast(&mut e, 0, case.id, &case.mana, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert!(battlefield_token_oids(&e, 1, "treasure").is_empty());

    // Grim Bounty destroys a planeswalker and creates a Treasure.
    let case = &cases()[1];
    let mut e = engine(75_101_002);
    let walker = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
    e.state
        .objects
        .get_mut(&walker)
        .expect("walker")
        .set_counter(tricerules_cards::primitives::CounterKind::Loyalty, 3);
    cast(&mut e, 0, case.id, &case.mana, target_object(walker));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&walker].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert!(battlefield_token_oids(&e, 1, "treasure").is_empty());

    // Improvised Weaponry kills a two-toughness creature and creates a Treasure.
    let case = &cases()[2];
    let mut e = engine(75_101_003);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast(&mut e, 0, case.id, &case.mana, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);

    // Pirate's Prize draws exactly two cards and creates a Treasure.
    let case = &cases()[3];
    let mut e = engine(75_101_004);
    let first = inject_library_card(&mut e, 0, "grizzly_bears");
    let second = inject_library_card(&mut e, 0, "storm_crow");
    e.state.players[0]
        .library
        .retain(|oid| *oid != first && *oid != second);
    e.state.players[0].library.push_front(second);
    e.state.players[0].library.push_front(first);
    let hand_before = e.state.players[0].hand.len();
    let opponent_hand = e.state.players[1].hand.clone();
    cast(&mut e, 0, case.id, &case.mana, vec![]);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 2);
    assert!(e.state.players[0].hand.contains(&first));
    assert!(e.state.players[0].hand.contains(&second));
    assert_eq!(
        e.state.players[1].hand, opponent_hand,
        "opponent draws nothing"
    );
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
}

#[test]
fn q75_treasure_riders_fizzle_without_a_treasure_when_the_target_is_stale() {
    for (index, case) in [&cases()[0], &cases()[1], &cases()[2]]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(75_102_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast at the creature");
        move_to_graveyard(&mut e, 1, target);
        resolve_entire_stack_two_player(&mut e);
        for player in [0, 1] {
            assert!(
                battlefield_token_oids(&e, player, "treasure").is_empty(),
                "{}: an illegal target means no Treasure for either player",
                case.id
            );
        }
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the fizzled spell resolves to its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn q75_treasure_hornswoggle_still_creates_a_treasure_on_an_uncounterable_spell() {
    let case = &cases()[4];
    let mut e = engine(75_103_001);
    inject_card_into_hand(&mut e, 0, "blurred_mongoose");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "blurred_mongoose");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Blurred Mongoose");
    e.apply_scenario_command(0, &pass())
        .expect("pass to the opponent");
    let target = e.state.stack.last().expect("creature spell").id;
    cast(&mut e, 1, case.id, &case.mana, target_object(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Battlefield,
        "the uncounterable creature spell resolves"
    );
    assert_eq!(
        battlefield_token_oids(&e, 1, "treasure").len(),
        1,
        "the Treasure is still created when the counter fails"
    );
}

#[test]
fn q75_treasure_hornswoggle_fizzles_without_a_treasure_when_the_spell_leaves_the_stack() {
    let case = &cases()[4];
    let mut e = engine(75_103_002);
    // Player 0 casts a creature spell, then counters it with their own Counterspell while
    // Hornswoggle is on the stack, so Hornswoggle's only target is gone at resolution.
    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "counterspell");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let bears = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(bears, vec![]))
        .expect("cast the creature spell");
    e.apply_scenario_command(0, &pass())
        .expect("pass to the opponent");
    let creature_spell = e.state.stack.last().expect("creature spell").id;
    cast(
        &mut e,
        1,
        case.id,
        &case.mana,
        target_object(creature_spell),
    );
    let hornswoggle = e.state.stack.last().expect("Hornswoggle").id;
    e.apply_scenario_command(1, &pass())
        .expect("pass back to the creature's controller");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let counterspell = hand_index_for_card(&e, 0, "counterspell");
    e.apply_scenario_command(0, &cast_spell(counterspell, target_object(creature_spell)))
        .expect("counter the creature spell in response");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&creature_spell].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&hornswoggle].zone,
        Zone::Graveyard,
        "the fizzled Hornswoggle resolves to its owner's graveyard"
    );
    for player in [0, 1] {
        assert!(
            battlefield_token_oids(&e, player, "treasure").is_empty(),
            "a stale Hornswoggle target means no Treasure"
        );
    }
    assert!(e.state.stack.is_empty());
}

#[test]
fn q75_treasure_riders_reject_missing_and_wrong_targets() {
    // Deadly Derision needs a creature or planeswalker, not a land or player.
    let mut e = engine(75_104_001);
    assert_rejected(&mut e, 0, "deadly_derision", &cases()[0].mana, vec![]);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    assert_rejected(
        &mut e,
        0,
        "deadly_derision",
        &cases()[0].mana,
        target_object(land),
    );
    assert_rejected(
        &mut e,
        0,
        "deadly_derision",
        &cases()[0].mana,
        target_player(1),
    );

    // Grim Bounty has the same filter.
    let mut e = engine(75_104_002);
    assert_rejected(&mut e, 0, "grim_bounty", &cases()[1].mana, target_player(1));

    // Improvised Weaponry and Pirate's Prize differ: the first needs a target, the second none.
    let mut e = engine(75_104_003);
    assert_rejected(&mut e, 0, "improvised_weaponry", &cases()[2].mana, vec![]);
    let mut e = engine(75_104_004);
    cast(&mut e, 0, "pirates_prize", &cases()[3].mana, vec![]);

    // Hornswoggle can only target a creature spell.
    let mut e = engine(75_104_005);
    assert_rejected(&mut e, 0, "hornswoggle", &cases()[4].mana, vec![]);
    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Lightning Bolt");
    let bolt = e.state.stack.last().expect("bolt").id;
    assert_rejected(
        &mut e,
        0,
        "hornswoggle",
        &cases()[4].mana,
        target_object(bolt),
    );
}

#[test]
fn q75_treasure_riders_reject_underfunded_casts() {
    // Deadly Derision and Grim Bounty: legal creature target, one mana short.
    for (index, case) in [&cases()[0], &cases()[1]].into_iter().enumerate() {
        let mut e = engine(75_105_000 + index as u64);
        let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let object = inject_card_into_hand(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: case.mana.b.saturating_sub(1),
                c: case.mana.c,
                ..Default::default()
            },
        );
        let hand_before = e.state.players[0].hand.clone();
        let before = pool(&e, 0);
        let slot = hand_index_for_card(&e, 0, case.id);
        let error = e
            .apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
            .expect_err("the printed cost must be paid in full");
        assert!(
            matches!(error, tricerules_core::EngineError::Illegal(_)),
            "{}: expected an affordability error, got {error:?}",
            case.id
        );
        assert!(
            !format!("{error:?}").to_lowercase().contains("target"),
            "{}: the rejection is not a target error: {error:?}",
            case.id
        );
        assert_eq!(pool(&e, 0), before, "{}: spends nothing", case.id);
        assert_eq!(e.state.players[0].hand, hand_before, "{}", case.id);
        assert_eq!(e.state.objects[&object].zone, Zone::Hand, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }

    // Improvised Weaponry: legal player target, one mana short.
    let case = &cases()[2];
    let mut e = engine(75_105_010);
    let object = inject_card_into_hand(&mut e, 0, case.id);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: case.mana.r,
            c: case.mana.c.saturating_sub(1),
            ..Default::default()
        },
    );
    let hand_before = e.state.players[0].hand.clone();
    let before = pool(&e, 0);
    let slot = hand_index_for_card(&e, 0, case.id);
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect_err("the printed cost must be paid in full");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert!(!format!("{error:?}").to_lowercase().contains("target"));
    assert_eq!(pool(&e, 0), before);
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&object].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // Pirate's Prize: untargeted.
    let case = &cases()[3];
    let mut e = engine(75_105_020);
    let object = inject_card_into_hand(&mut e, 0, case.id);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: case.mana.u,
            c: case.mana.c.saturating_sub(1),
            ..Default::default()
        },
    );
    let hand_before = e.state.players[0].hand.clone();
    let before = pool(&e, 0);
    let slot = hand_index_for_card(&e, 0, case.id);
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect_err("the printed cost must be paid in full");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(pool(&e, 0), before);
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&object].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // Hornswoggle: legal creature-spell target, one mana short.
    let case = &cases()[4];
    let mut e = engine(75_105_030);
    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let bears = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(bears, vec![]))
        .expect("cast the creature spell");
    let target = e.state.stack.last().expect("creature spell").id;
    let object = inject_card_into_hand(&mut e, 0, case.id);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: case.mana.u,
            c: case.mana.c.saturating_sub(1),
            ..Default::default()
        },
    );
    let hand_before = e.state.players[0].hand.clone();
    let before = pool(&e, 0);
    let slot = hand_index_for_card(&e, 0, case.id);
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect_err("the printed cost must be paid in full");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert!(!format!("{error:?}").to_lowercase().contains("target"));
    assert_eq!(pool(&e, 0), before);
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&object].zone, Zone::Hand);
    assert_eq!(
        e.state.stack.len(),
        1,
        "the creature spell is still waiting"
    );
}

#[test]
fn q75_treasure_pirates_prize_draw_is_one_players_private_action() {
    let case = &cases()[3];
    let mut e = engine(75_106_001);
    let first = inject_library_card(&mut e, 0, "grizzly_bears");
    let second = inject_library_card(&mut e, 0, "storm_crow");
    e.state.players[0]
        .library
        .retain(|oid| *oid != first && *oid != second);
    e.state.players[0].library.push_front(second);
    e.state.players[0].library.push_front(first);
    let library_after_seat = e.state.players[0].library.clone();
    let opponent_library = e.state.players[1].library.clone();
    cast(&mut e, 0, case.id, &case.mana, vec![]);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].library, opponent_library,
        "the opponent's library is untouched"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_after_seat.len() - 2,
        "exactly two cards leave the caster's library"
    );
    assert_eq!(e.state.stack.len(), 0);
    assert!(
        e.state.pending_resolution.is_none(),
        "drawing publishes no hidden choice"
    );
    assert_eq!(
        battlefield_token_oids(&e, 0, "treasure").len(),
        1,
        "the Treasure rider resolves with the draw"
    );
    assert!(
        battlefield_token_oids(&e, 1, "treasure").is_empty(),
        "the opponent gets no Treasure"
    );
}
