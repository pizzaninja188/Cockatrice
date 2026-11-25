//! Actual-card scenarios for the queue-v75 02-proliferate-riders cohort.
//!
//! Expectations are printed Oracle values. CR 701.34 governs proliferate (choose any number of
//! permanents and/or players, each gets one counter of each kind it already has), CR 121 draws,
//! CR 701.8 destroys, CR 115.1d/400.7 move a graveyard card, and CR 608.2b fizzles a spell whose
//! only chosen target is illegal. Printed order follows CR 608.2c.
use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    ChoiceKind, RuledCommand, SubmitResolutionChoice, TargetRefKind,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "contentious_plan",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "steady_progress",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "atomize",
            mana: ManaGift {
                b: 1,
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "unnatural_restoration",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "vivisurgeons_insight",
            mana: ManaGift {
                u: 2,
                c: 3,
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
    let decks = Some(vec![deck_with("island", &[]), deck_with("swamp", &[])]);
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

fn graveyard_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }
}

fn choose_proliferate(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: objects,
            chosen_player_ids: players,
            ..Default::default()
        })),
    }
}

fn pend_proliferate(e: &GameEngine) -> &tricerules_core::state::PendingResolution {
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("a Proliferate choice is parked");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::Proliferate,
        "the parked choice is Proliferate"
    );
    pending
}

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
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
fn q75_proliferate_canary_simplest_contentious_plan_proliferates_before_drawing() {
    let case = &cases()[0];
    let mut e = engine(75_200_001);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.state
        .objects
        .get_mut(&bear)
        .expect("bear")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let drawn = put_on_top(&mut e, 0, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let hand_after_cast = e.state.players[0].hand.len() - 1;
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Contentious Plan");
    pass_both_players(&mut e);
    let pending = pend_proliferate(&e);
    assert!(pending.presentation.candidates.contains(&bear));
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_after_cast,
        "the draw has not happened while Proliferate is being chosen"
    );
    e.apply_scenario_command(0, &choose_proliferate(vec![bear], vec![]))
        .expect("proliferate the bear");
    assert_eq!(
        e.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "Proliferate adds one more of each kind already there"
    );
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "then the draw");
    assert_eq!(e.state.players[0].hand.len(), hand_after_cast + 1);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed cost");
    assert!(e.state.stack.is_empty());
}

#[test]
fn q75_proliferate_canary_hardest_atomize_destroys_then_proliferates() {
    let case = &cases()[2];
    let mut e = engine(75_200_002);
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    e.state
        .objects
        .get_mut(&creature)
        .expect("creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    let opponent_counter = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    e.state
        .objects
        .get_mut(&opponent_counter)
        .expect("counter bearer")
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Atomize at the creature");
    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Graveyard,
        "the nonland permanent is destroyed first"
    );
    let candidates = pend_proliferate(&e).presentation.candidates.clone();
    assert!(
        !candidates.contains(&creature),
        "the destroyed permanent is no longer a Proliferate candidate"
    );
    assert!(candidates.contains(&opponent_counter));
    e.apply_scenario_command(0, &choose_proliferate(vec![opponent_counter], vec![]))
        .expect("proliferate an opponent's counter-bearing permanent");
    assert_eq!(
        e.state.objects[&opponent_counter].counter_count(CounterKind::PlusOnePlusOne),
        3
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q75_proliferate_riders_apply_their_printed_bodies_in_order() {
    // Steady Progress is the no-target instant twin of Contentious Plan.
    let case = &cases()[1];
    let mut e = engine(75_201_001);
    let drawn = put_on_top(&mut e, 0, "grizzly_bears");
    let counter_bear = inject_creature_on_battlefield(&mut e, 0, "hill_giant");
    e.state
        .objects
        .get_mut(&counter_bear)
        .expect("bear")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Steady Progress");
    pass_both_players(&mut e);
    assert!(
        pend_proliferate(&e)
            .presentation
            .candidates
            .contains(&counter_bear),
        "Steady Progress parks Proliferate before drawing"
    );
    assert_eq!(
        e.state.objects[&drawn].zone,
        Zone::Library,
        "the draw is still pending while Proliferate is chosen"
    );
    e.apply_scenario_command(0, &choose_proliferate(vec![], vec![]))
        .expect("choose no permanents or players");
    assert_eq!(
        e.state.objects[&drawn].zone,
        Zone::Hand,
        "then the draw happens"
    );
    assert_eq!(
        e.state.objects[&counter_bear].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "choosing no recipients performs no proliferation"
    );
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));

    // Vivisurgeon's Insight draws three first, then proliferates.
    let case = &cases()[4];
    let mut e = engine(75_201_002);
    let cards: Vec<u32> = (0..3)
        .map(|_| put_on_top(&mut e, 0, "grizzly_bears"))
        .collect();
    let counter_bear = inject_creature_on_battlefield(&mut e, 0, "hill_giant");
    e.state
        .objects
        .get_mut(&counter_bear)
        .expect("bear")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Vivisurgeon's Insight");
    pass_both_players(&mut e);
    for card in &cards {
        assert_eq!(
            e.state.objects[card].zone,
            Zone::Hand,
            "all three cards are drawn before the Proliferate choice"
        );
    }
    assert_eq!(
        e.state.players[0].hand.len(),
        before - 1 + 3,
        "the three-card draw is complete before the Proliferate choice"
    );
    assert!(
        pend_proliferate(&e)
            .presentation
            .candidates
            .contains(&counter_bear),
        "Vivisurgeon's Insight parks Proliferate after the draw"
    );
    e.apply_scenario_command(0, &choose_proliferate(vec![], vec![]))
        .expect("choose no permanents or players");
    assert_eq!(
        e.state.objects[&counter_bear].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "choosing no recipients performs no proliferation"
    );

    // Unnatural Restoration returns one permanent card, then proliferates.
    let case = &cases()[3];
    let mut e = engine(75_201_003);
    let returned = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let land = inject_graveyard_card(&mut e, 0, "forest");
    let nonpermanent = inject_graveyard_card(&mut e, 0, "lightning_bolt");
    let theirs = inject_graveyard_card(&mut e, 1, "storm_crow");
    let bear = inject_creature_on_battlefield(&mut e, 0, "hill_giant");
    e.state
        .objects
        .get_mut(&bear)
        .expect("bear")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(returned)]))
        .expect("cast Unnatural Restoration");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&returned].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&returned));
    for untouched in [land, nonpermanent, theirs] {
        assert_eq!(
            e.state.objects[&untouched].zone,
            Zone::Graveyard,
            "only the chosen permanent card returns"
        );
    }
    pend_proliferate(&e);
    e.apply_scenario_command(0, &choose_proliferate(vec![bear], vec![]))
        .expect("proliferate a counter-bearing permanent");
    assert_eq!(
        e.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
}

#[test]
fn q75_proliferate_riders_fizzle_without_proliferating_on_a_stale_target() {
    // Atomize: the destroyed permanent leaves before resolution.
    let mut e = engine(75_202_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "atomize");
    give_mana(&mut e, 0, gift(&cases()[2].mana));
    let slot = hand_index_for_card(&e, 0, "atomize");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Atomize");
    move_to_graveyard(&mut e, 1, target);
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.pending_resolution.is_none(),
        "an illegal sole target means no proliferate"
    );
    assert!(e.state.stack.is_empty());

    // Unnatural Restoration: the chosen graveyard card is exiled before resolution.
    let mut e = engine(75_202_002);
    let target = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let bear = inject_creature_on_battlefield(&mut e, 0, "hill_giant");
    e.state
        .objects
        .get_mut(&bear)
        .expect("bear")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    inject_card_into_hand(&mut e, 0, "unnatural_restoration");
    give_mana(&mut e, 0, gift(&cases()[3].mana));
    let slot = hand_index_for_card(&e, 0, "unnatural_restoration");
    e.apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(target)]))
        .expect("cast Unnatural Restoration");
    e.state.players[0].graveyard.retain(|oid| *oid != target);
    e.state.objects.get_mut(&target).expect("card").zone = Zone::Exile;
    e.state.players[0].exile.push(target);
    *e.state.zone_change_generation.entry(target).or_default() += 1;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile);
    assert!(
        e.state.pending_resolution.is_none(),
        "an illegal sole target means no proliferate"
    );
    assert_eq!(
        e.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn q75_proliferate_riders_reject_illegal_targets_and_underfunded_costs() {
    // Atomize cannot destroy a land or target a player.
    let mut e = engine(75_203_001);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    inject_card_into_hand(&mut e, 0, "atomize");
    give_mana(&mut e, 0, gift(&cases()[2].mana));
    let slot = hand_index_for_card(&e, 0, "atomize");
    for targets in [target_object(land), target_player(1)] {
        let error = e
            .apply_scenario_command(0, &cast_spell(slot, targets))
            .expect_err("Atomize only targets a nonland permanent");
        assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    }
    assert_eq!(
        pool(&e, 0),
        (0, 0, 1, 0, 1, 2),
        "rejected casts spend nothing"
    );

    // Unnatural Restoration rejects a nonpermanent card and an opponent's card.
    let mut e = engine(75_203_002);
    let instant = inject_graveyard_card(&mut e, 0, "lightning_bolt");
    let theirs = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "unnatural_restoration");
    give_mana(&mut e, 0, gift(&cases()[3].mana));
    let slot = hand_index_for_card(&e, 0, "unnatural_restoration");
    for target in [instant, theirs] {
        let error = e
            .apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(target)]))
            .expect_err("only a controller's permanent card is legal");
        assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    }

    // Atomize with a legal nonland-permanent target and one mana short is an affordability error.
    let mut e = engine(75_203_010);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let object = inject_card_into_hand(&mut e, 0, "atomize");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "atomize");
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect_err("Atomize's printed cost must be paid in full");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert!(
        !format!("{error:?}").to_lowercase().contains("target"),
        "the rejection is not a target error: {error:?}"
    );
    assert_eq!(
        pool(&e, 0),
        (0, 0, 1, 0, 1, 1),
        "the rejected cast spends nothing"
    );
    assert_eq!(e.state.objects[&object].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // Unnatural Restoration with a legal permanent card and one mana short.
    let mut e = engine(75_203_011);
    let target = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let object = inject_card_into_hand(&mut e, 0, "unnatural_restoration");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "unnatural_restoration");
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(target)]))
        .expect_err("Unnatural Restoration's printed cost must be paid in full");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert!(!format!("{error:?}").to_lowercase().contains("target"));
    assert_eq!(
        pool(&e, 0),
        (0, 0, 0, 0, 1, 0),
        "the rejected cast spends nothing"
    );
    assert_eq!(e.state.objects[&object].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // The untargeted riders reject an underfunded cast and leave the card in hand.
    for (index, case) in [&cases()[0], &cases()[1], &cases()[4]]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(75_204_000 + index as u64);
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
        assert!(
            matches!(error, tricerules_core::EngineError::Illegal(_)),
            "{}: expected an affordability error, got {error:?}",
            case.id
        );
        assert_eq!(pool(&e, 0), before, "{}: spends nothing", case.id);
        assert_eq!(e.state.players[0].hand, hand_before, "{}", case.id);
        assert_eq!(e.state.objects[&object].zone, Zone::Hand, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q75_proliferate_choice_is_optional_and_reaches_opponents() {
    let case = &cases()[0];
    let mut e = engine(75_205_001);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    for object in [own, opposing] {
        e.state
            .objects
            .get_mut(&object)
            .expect("bear")
            .set_counter(CounterKind::PlusOnePlusOne, 1);
    }
    let opponent = e.state.player_idx(1).expect("opponent");
    e.state.players[opponent]
        .counters
        .insert(CounterKind::Poison, 1);
    inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Contentious Plan");
    pass_both_players(&mut e);
    pend_proliferate(&e);
    e.apply_scenario_command(0, &choose_proliferate(vec![opposing], vec![1]))
        .expect("choose the opponent's permanent and the opponent");
    assert_eq!(
        e.state.objects[&opposing].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        e.state.objects[&own].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "an un chosen permanent is not proliferated"
    );
    assert_eq!(
        e.state.players[opponent].counters[&CounterKind::Poison],
        2,
        "the opponent's poison counter grows"
    );
}
