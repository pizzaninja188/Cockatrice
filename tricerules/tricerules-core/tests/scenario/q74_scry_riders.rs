//! Actual-card scenarios for the queue-v74 03-scry-riders cohort.
//!
//! Expectations are printed Oracle values. Targets are chosen at cast (CR 115.1, 601.2c); a
//! spell whose only target is illegal at resolution does not resolve, so its scry does not
//! happen (CR 608.2b); damage follows CR 120.3; scry follows CR 701.22a-b; and the pump and haste
//! grant end at cleanup (CR 611.2a/514.2). Anchor's put-on-top follows CR 401.4.
use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("mountain", &[])]);
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

fn combat_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("mountain", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_declare_attackers(&mut e);
    e
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect::<Vec<_>>();
    e.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        e.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn resolve_top(e: &mut GameEngine) -> RuledEventBatch {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    e.apply_scenario_command(first, &pass())
        .expect("first pass");
    e.apply_scenario_command(second, &pass())
        .expect("second pass")
}

#[test]
fn q74_scry_canary_simplest_serum_visions_draws_then_scries_two() {
    let mut e = engine(74_300_001);
    let top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant", "forest"]);
    inject_card_into_hand(&mut e, 0, "serum_visions");
    let hand_before = e.state.players[0].hand.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let spell = e.state.players[0].hand[hand_index_for_card(&e, 0, "serum_visions")];
    let slot = hand_index_for_card(&e, 0, "serum_visions");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Serum Visions");
    let parked = resolve_top(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
    assert_eq!(
        e.state.objects[&top[0]].zone,
        Zone::Hand,
        "the draw happens before the scry"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "draw one, cast one"
    );
    let choice = find_resolution_choice(&parked).expect("Serum Visions parks its scry");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids, vec![top[1], top[2]]);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[2]]))
        .expect("bottom the second scried card");
    assert_eq!(
        *e.state.players[0].library.back().expect("library"),
        top[2],
        "the chosen card goes to the bottom"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q74_scry_canary_hardest_anchor_puts_the_target_on_top_then_scries_one() {
    let mut e = engine(74_300_002);
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let own_top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant"]);
    let spell = inject_card_into_hand(&mut e, 0, "anchor_to_the_aether");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "anchor_to_the_aether");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Anchor to the Aether");
    let parked = resolve_top(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
    assert_eq!(
        e.state.players[1].library.front().copied(),
        Some(creature),
        "the target is put on top of its owner's library"
    );
    let choice = find_resolution_choice(&parked).expect("Anchor parks its scry");
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(
        choice.candidate_object_ids,
        vec![own_top[0]],
        "scry looks at the caster's own library, not the target owner's library"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![own_top[0]]))
        .expect("bottom the caster's top card");
    assert_eq!(
        *e.state.players[0].library.back().expect("library"),
        own_top[0],
        "the scried card goes to the bottom"
    );
    assert_eq!(
        e.state.players[1].library.front().copied(),
        Some(creature),
        "the bounced creature stays on top of its owner's library"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn q74_scry_riders_apply_their_printed_bodies_and_scry_counts() {
    // Anchor to the Aether: any creature, then scry 1.
    {
        let mut e = engine(74_301_001);
        let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        inject_card_into_hand(&mut e, 0, "anchor_to_the_aether");
        give_mana(
            &mut e,
            0,
            gift(&ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            }),
        );
        let spell = e.state.players[0].hand[hand_index_for_card(&e, 0, "anchor_to_the_aether")];
        let slot = hand_index_for_card(&e, 0, "anchor_to_the_aether");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .expect("cast Anchor");
        let parked = resolve_top(&mut e);
        let choice = find_resolution_choice(&parked).expect("Anchor scry");
        assert_eq!((choice.min, choice.max), (0, 1));
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("keep the card");
        assert_eq!(e.state.players[1].library.front().copied(), Some(creature));
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    }

    // Jaya's Greeting: 3 damage to a creature, then scry 1.
    {
        let mut e = engine(74_301_002);
        let victim = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        inject_card_into_hand(&mut e, 0, "jayas_greeting");
        give_mana(
            &mut e,
            0,
            gift(&ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            }),
        );
        let spell = e.state.players[0].hand[hand_index_for_card(&e, 0, "jayas_greeting")];
        let slot = hand_index_for_card(&e, 0, "jayas_greeting");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(victim)))
            .expect("cast Jaya's Greeting");
        let parked = resolve_top(&mut e);
        let choice = find_resolution_choice(&parked).expect("Jaya's Greeting scry");
        assert_eq!((choice.min, choice.max), (0, 1));
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(
            e.state.objects[&victim].damage, 3,
            "Jaya's Greeting marks exactly three damage before the scry"
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("keep the card");
        assert_eq!(
            e.state.objects[&victim].zone,
            Zone::Graveyard,
            "state-based actions kill the 2/2 after the spell finishes"
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
        assert_eq!(e.state.players[1].life, 20, "creature damage only");
    }

    // Piercing Light: 2 damage to an attacking creature, then scry 1.
    {
        let mut e = combat_engine(74_301_003);
        let attacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        inject_card_into_hand(&mut e, 0, "piercing_light");
        give_mana(
            &mut e,
            0,
            gift(&ManaGift {
                w: 1,
                ..Default::default()
            }),
        );
        let spell = e.state.players[0].hand[hand_index_for_card(&e, 0, "piercing_light")];
        let slot = hand_index_for_card(&e, 0, "piercing_light");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(attacker)))
            .expect("cast Piercing Light");
        let parked = resolve_top(&mut e);
        let choice = find_resolution_choice(&parked).expect("Piercing Light scry");
        assert_eq!((choice.min, choice.max), (0, 1));
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(
            e.state.objects[&attacker].damage, 2,
            "Piercing Light marks exactly two damage before the scry"
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("keep the card");
        assert_eq!(
            e.state.objects[&attacker].zone,
            Zone::Graveyard,
            "state-based actions kill the 2/2 after the spell finishes"
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    }

    // Samut's Sprint: +2/+1 and haste, then scry 1; both end at cleanup.
    {
        let mut e = engine(74_301_004);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        inject_card_into_hand(&mut e, 0, "samuts_sprint");
        give_mana(
            &mut e,
            0,
            gift(&ManaGift {
                r: 1,
                ..Default::default()
            }),
        );
        let spell = e.state.players[0].hand[hand_index_for_card(&e, 0, "samuts_sprint")];
        let slot = hand_index_for_card(&e, 0, "samuts_sprint");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast Samut's Sprint");
        let parked = resolve_top(&mut e);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(4), Some(3))
        );
        assert!(e.effective_has_keyword(target, Keyword::Haste));
        let choice = find_resolution_choice(&parked).expect("Samut's Sprint scry");
        assert_eq!((choice.min, choice.max), (0, 1));
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("keep the card");
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2), Some(2)),
            "the pump expires at cleanup"
        );
        assert!(!e.effective_has_keyword(target, Keyword::Haste));
    }

    // Serum Visions: draw one, then scry 2.
    {
        let mut e = engine(74_301_005);
        let top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant", "forest"]);
        inject_card_into_hand(&mut e, 0, "serum_visions");
        give_mana(
            &mut e,
            0,
            gift(&ManaGift {
                u: 1,
                ..Default::default()
            }),
        );
        let spell = e.state.players[0].hand[hand_index_for_card(&e, 0, "serum_visions")];
        let slot = hand_index_for_card(&e, 0, "serum_visions");
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast Serum Visions");
        let parked = resolve_top(&mut e);
        assert_eq!(e.state.objects[&top[0]].zone, Zone::Hand);
        let choice = find_resolution_choice(&parked).expect("Serum Visions scry");
        assert_eq!((choice.min, choice.max), (0, 2));
        e.apply_scenario_command(0, &submit_resolution_choice(vec![top[2]]))
            .expect("bottom one");
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    }
}

#[test]
fn q74_scry_riders_fizzle_stale_targets_without_scrying() {
    // Samut's Sprint: the only target leaves; no pump, no haste and no scry.
    let mut e = engine(74_303_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let spell = inject_card_into_hand(&mut e, 0, "samuts_sprint");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            r: 1,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "samuts_sprint");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Samut's Sprint");
    move_to_graveyard(&mut e, 0, target);
    let batch = resolve_top(&mut e);
    assert!(
        find_resolution_choice(&batch).is_none(),
        "a fizzled Samut's Sprint does not scry"
    );
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    // Anchor to the Aether: the only target leaves; it is not put on a library and no scry
    // choice appears.
    let mut e = engine(74_303_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let own_top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant"]);
    let opponent_library = e.state.players[1].library.clone();
    let spell = inject_card_into_hand(&mut e, 0, "anchor_to_the_aether");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "anchor_to_the_aether");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Anchor to the Aether");
    move_to_graveyard(&mut e, 1, target);
    let batch = resolve_top(&mut e);
    assert!(
        find_resolution_choice(&batch).is_none(),
        "a fizzled Anchor to the Aether does not scry"
    );
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[1].library, opponent_library,
        "a fizzled Anchor does not move the stale target to a library"
    );
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(own_top[0]),
        "the caster's library is untouched"
    );
}

#[test]
fn q74_scry_riders_reject_wrong_targets_and_fizzle_stale_targets_without_scrying() {
    // Jaya's Greeting rejects a player target.
    let mut e = engine(74_302_001);
    let spell = inject_card_into_hand(&mut e, 0, "jayas_greeting");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        }),
    );
    let before = pool(&e);
    let hand_before = e.state.players[0].hand.clone();
    let slot = hand_index_for_card(&e, 0, "jayas_greeting");
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect_err("Jaya's Greeting requires a creature");
    assert!(format!("{error:?}").contains("Illegal"));
    assert_eq!(pool(&e), before, "rejected cast spends no mana");
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // Piercing Light rejects an idle creature.
    let mut e = engine(74_302_002);
    let idle = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let spell = inject_card_into_hand(&mut e, 0, "piercing_light");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            w: 1,
            ..Default::default()
        }),
    );
    let before = pool(&e);
    let hand_before = e.state.players[0].hand.clone();
    let slot = hand_index_for_card(&e, 0, "piercing_light");
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, target_object(idle)))
        .expect_err("an idle creature is not attacking or blocking");
    assert!(format!("{error:?}").contains("Illegal"));
    assert_eq!(pool(&e), before, "rejected cast spends no mana");
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // Samut's Sprint requires a creature.
    let mut e = engine(74_302_003);
    let land = inject_permanent_on_battlefield(&mut e, 0, "island");
    let spell = inject_card_into_hand(&mut e, 0, "samuts_sprint");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            r: 1,
            ..Default::default()
        }),
    );
    let before = pool(&e);
    let hand_before = e.state.players[0].hand.clone();
    let slot = hand_index_for_card(&e, 0, "samuts_sprint");
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, target_object(land)))
        .expect_err("Samut's Sprint requires a creature");
    assert!(format!("{error:?}").contains("Illegal"));
    assert_eq!(pool(&e), before, "rejected cast spends no mana");
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // Anchor to the Aether requires a creature.
    let mut e = engine(74_302_004);
    let land = inject_permanent_on_battlefield(&mut e, 0, "island");
    let spell = inject_card_into_hand(&mut e, 0, "anchor_to_the_aether");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        }),
    );
    let before = pool(&e);
    let hand_before = e.state.players[0].hand.clone();
    let slot = hand_index_for_card(&e, 0, "anchor_to_the_aether");
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, target_object(land)))
        .expect_err("Anchor to the Aether requires a creature");
    assert!(format!("{error:?}").contains("Illegal"));
    assert_eq!(pool(&e), before, "rejected cast spends no mana");
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // Jaya's Greeting's only target leaving means no damage and no scry.
    let mut e = engine(74_302_005);
    let victim = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant"]);
    inject_card_into_hand(&mut e, 0, "jayas_greeting");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "jayas_greeting");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(victim)))
        .expect("cast at the creature");
    move_to_graveyard(&mut e, 1, victim);
    let batch = resolve_top(&mut e);
    assert!(
        find_resolution_choice(&batch).is_none(),
        "a fizzled Jaya's Greeting does not scry"
    );
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        top.iter()
            .filter(|object| e.state.players[0].library.contains(object))
            .count(),
        2,
        "the top two cards are untouched"
    );
}
