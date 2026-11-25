//! Actual-card scenarios for the queue-v74 01-cycling-control cohort.
//!
//! Expectations are printed Oracle values. Cycling is a hand-zone activated ability (CR 702.29a)
//! with its printed cost (CR 118.3); the spell body resolves first and each body follows its
//! printed target, count, destination and player scope. Targets are chosen at cast (CR 115.1,
//! 601.2c), a spell with only illegal targets does not resolve (CR 608.2b), damage follows
//! CR 120.3, and discard follows CR 701.9.
use super::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_cards::CounterKind;
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{AbilitySourceZone, ActivateAbility, ChoiceKind, RuledCommand};

struct CycleCase {
    id: &'static str,
    mana: ManaGift,
    cycle: ManaGift,
}

fn cycle_cases() -> [CycleCase; 5] {
    [
        CycleCase {
            id: "flame_jet",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
        CycleCase {
            id: "rescind",
            mana: ManaGift {
                u: 2,
                c: 1,
                ..Default::default()
            },
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
        CycleCase {
            id: "thornado",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            cycle: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        },
        CycleCase {
            id: "unburden",
            mana: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
        CycleCase {
            id: "wander_in_death",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn remove_all_abilities(e: &mut GameEngine, object: u32) {
    e.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: e.state.command_index,
    });
}

fn assert_rejected(e: &mut GameEngine, card: &str, mana: &ManaGift, targets: Vec<TargetRef>) {
    let object = inject_card_into_hand(e, 0, card);
    give_mana(e, 0, gift(mana));
    let before = pool(e, 0);
    let hand_before = e.state.players[0].hand.clone();
    let slot = hand_index_for_card(e, 0, card);
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, targets))
        .expect_err(&format!("{card}: cast must be rejected"));
    assert!(
        matches!(error, tricerules_core::EngineError::Illegal(_)),
        "{card}: expected an illegal-input error, got {error:?}"
    );
    assert_eq!(pool(e, 0), before, "{card}: rejected cast spends nothing");
    assert_eq!(
        e.state.players[0].hand, hand_before,
        "{card}: rejected cast keeps the card in hand"
    );
    assert_eq!(e.state.objects[&object].zone, Zone::Hand, "{card}");
    assert!(e.state.stack.is_empty(), "{card}: no stack object");
}

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

fn hand_ability(e: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

fn cycle_from_hand(e: &mut GameEngine, id: &str, cost: &ManaGift) {
    let card = inject_card_into_hand(e, 0, id);
    let drawn = put_on_top(e, 0, "grizzly_bears");
    let hand_before = e.state.players[0].hand.len();
    let opponent_hand = e.state.players[1].hand.clone();
    give_mana(e, 0, gift(cost));
    e.apply_scenario_command(0, &hand_ability(e, card))
        .unwrap_or_else(|error| panic!("{id}: cycle: {error:?}"));
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Graveyard,
        "{id}: cycling discards itself"
    );
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{id}: exact cycling cost");
    resolve_entire_stack_two_player(e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{id}: draws one");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "{id}: one-for-one"
    );
    assert_eq!(
        e.state.players[1].hand, opponent_hand,
        "{id}: opponent hand unchanged"
    );
}

#[test]
fn q74_cycling_canary_simplest_flame_jet_damages_the_chosen_player_and_cycles() {
    let case = &cycle_cases()[0];
    let mut e = engine(74_100_001);
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let life = e.state.players[1].life;
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Flame Jet at the opponent");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed cost");
    assert_eq!(e.state.players[1].life, life - 3, "exactly three damage");
    assert_eq!(e.state.players[0].life, 20, "the caster is not damaged");
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    cycle_from_hand(&mut e, case.id, &case.cycle);
}

#[test]
fn q74_cycling_canary_hardest_wander_in_death_returns_two_and_cycles() {
    let case = &cycle_cases()[4];
    let mut e = engine(74_100_002);
    let first = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let second = inject_graveyard_card(&mut e, 0, "storm_crow");
    let stranger = inject_graveyard_card(&mut e, 0, "hill_giant");
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(
        0,
        &cast_spell(slot, [target_object(first), target_object(second)].concat()),
    )
    .expect("cast Wander in Death with two targets");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed cost");
    for object in [first, second] {
        assert_eq!(e.state.objects[&object].zone, Zone::Hand);
        assert!(e.state.players[0].hand.contains(&object));
        assert!(!e.state.players[0].graveyard.contains(&object));
    }
    assert_eq!(
        e.state.objects[&stranger].zone,
        Zone::Graveyard,
        "only the chosen cards return"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    cycle_from_hand(&mut e, case.id, &case.cycle);
}

#[test]
fn q74_cycling_control_cards_apply_their_printed_bodies_and_cycle() {
    for (index, case) in cycle_cases().into_iter().enumerate() {
        let mut e = engine(74_101_000 + index as u64);
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        match case.id {
            "flame_jet" => {
                let walker = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
                e.state
                    .objects
                    .get_mut(&walker)
                    .expect("walker")
                    .set_counter(CounterKind::Loyalty, 20);
                e.apply_scenario_command(0, &cast_spell(slot, target_object(walker)))
                    .expect("cast at a planeswalker");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(
                    e.state.objects[&walker].counter_count(CounterKind::Loyalty),
                    17,
                    "Flame Jet removes three loyalty counters"
                );
                assert_eq!(e.state.players[1].life, 20, "the player is not damaged");
            }
            "rescind" => {
                let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
                e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                    .expect("cast Rescind");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(e.state.objects[&creature].zone, Zone::Hand);
                assert!(
                    e.state.players[1].hand.contains(&creature),
                    "Rescind returns the permanent to its owner's hand"
                );
            }
            "thornado" => {
                let flier = inject_creature_with_stats(&mut e, 1, "ravenous_skirge", 2, 2);
                let walker = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
                e.state
                    .objects
                    .get_mut(&walker)
                    .expect("walker")
                    .set_counter(CounterKind::Loyalty, 20);
                e.apply_scenario_command(0, &cast_spell(slot, target_object(flier)))
                    .expect("cast Thornado");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(e.state.objects[&flier].zone, Zone::Graveyard);
                assert_eq!(e.state.players[1].life, 20, "no player damage");
            }
            "unburden" => {
                let first = inject_card_into_hand(&mut e, 1, "grizzly_bears");
                let second = inject_card_into_hand(&mut e, 1, "storm_crow");
                let kept = inject_card_into_hand(&mut e, 1, "hill_giant");
                e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                    .expect("cast Unburden at the opponent");
                e.apply_scenario_command(0, &pass()).expect("caster pass");
                let batch = e
                    .apply_scenario_command(1, &pass())
                    .expect("Unburden resolves");
                let choice = find_resolution_choice(&batch).expect("discard choice");
                assert_eq!(choice.deciding_player_id, 1, "the targeted player chooses");
                assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
                assert_eq!((choice.min, choice.max), (2, 2), "exactly two cards");
                e.apply_scenario_command(1, &submit_resolution_choice(vec![first, second]))
                    .expect("the opponent discards two");
                assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
                assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
                assert_eq!(e.state.objects[&kept].zone, Zone::Hand);
            }
            "wander_in_death" => {
                let first = inject_graveyard_card(&mut e, 0, "grizzly_bears");
                let second = inject_graveyard_card(&mut e, 0, "storm_crow");
                e.apply_scenario_command(
                    0,
                    &cast_spell(slot, [target_object(first), target_object(second)].concat()),
                )
                .expect("cast Wander in Death");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(e.state.objects[&first].zone, Zone::Hand);
                assert_eq!(e.state.objects[&second].zone, Zone::Hand);
            }
            other => panic!("unhandled {other}"),
        }
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed cost",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: spell resolves to the graveyard",
            case.id
        );
        cycle_from_hand(&mut e, case.id, &case.cycle);
    }
}

#[test]
fn q74_cycling_control_rejects_missing_wrong_targets_and_unfunded_cycling() {
    // Flame Jet needs a player or planeswalker target, not a creature.
    let mut e = engine(74_102_001);
    assert_rejected(&mut e, "flame_jet", &cycle_cases()[0].mana, vec![]);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    assert_rejected(
        &mut e,
        "flame_jet",
        &cycle_cases()[0].mana,
        target_object(bear),
    );

    // Thornado cannot destroy a nonflying creature or a planeswalker.
    let mut e = engine(74_102_002);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let walker = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
    assert_rejected(
        &mut e,
        "thornado",
        &cycle_cases()[2].mana,
        target_object(bear),
    );
    assert_rejected(
        &mut e,
        "thornado",
        &cycle_cases()[2].mana,
        target_object(walker),
    );

    // Wander in Death cannot take an opponent's creature card or a land card.
    let mut e = engine(74_102_003);
    let theirs = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    let land = inject_graveyard_card(&mut e, 0, "forest");
    assert_rejected(
        &mut e,
        "wander_in_death",
        &cycle_cases()[4].mana,
        target_object(theirs),
    );
    assert_rejected(
        &mut e,
        "wander_in_death",
        &cycle_cases()[4].mana,
        target_object(land),
    );

    // Rescind accepts any permanent but not a player.
    let mut e = engine(74_102_004);
    assert_rejected(&mut e, "rescind", &cycle_cases()[1].mana, target_player(1));

    // Unburden can only target a player.
    let mut e = engine(74_102_005);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    assert_rejected(
        &mut e,
        "unburden",
        &cycle_cases()[3].mana,
        target_object(bear),
    );

    for case in cycle_cases() {
        let mut e = engine(74_102_100 + case.mana.c as u64);
        let card = inject_card_into_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.clone();
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: case.cycle.c.saturating_sub(1),
                w: case.cycle.w.saturating_sub(1),
                u: case.cycle.u.saturating_sub(1),
                b: case.cycle.b.saturating_sub(1),
                r: case.cycle.r.saturating_sub(1),
                g: case.cycle.g.saturating_sub(1),
            },
        );
        let before = pool(&e, 0);
        let error = e
            .apply_scenario_command(0, &hand_ability(&e, card))
            .expect_err(&format!("{}: cycling needs its full printed mana", case.id));
        assert!(
            matches!(error, tricerules_core::EngineError::Illegal(_)),
            "{}: expected an affordability error, got {error:?}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}: no stack object", case.id);
        assert_eq!(
            pool(&e, 0),
            before,
            "{}: rejected cycling spends nothing",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand, hand_before,
            "{}: rejected cycling keeps the card in hand",
            case.id
        );
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);
    }
}

#[test]
fn q74_cycling_control_thornado_rechecks_flying_at_resolution() {
    let mut e = engine(74_102_201);
    let flier = inject_creature_with_stats(&mut e, 1, "ravenous_skirge", 2, 2);
    let spell = inject_card_into_hand(&mut e, 0, "thornado");
    give_mana(&mut e, 0, gift(&cycle_cases()[2].mana));
    let slot = hand_index_for_card(&e, 0, "thornado");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(flier)))
        .expect("cast Thornado at the flier");
    assert!(e.effective_has_keyword(flier, Keyword::Flying));
    remove_all_abilities(&mut e, flier);
    assert!(
        !e.effective_has_keyword(flier, Keyword::Flying),
        "the target is no longer a flying creature"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&flier].zone,
        Zone::Battlefield,
        "Thornado rechecks the flying characteristic at resolution"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q74_cycling_control_wander_in_death_handles_zero_one_and_stale_targets() {
    // Zero targets is legal for "up to two".
    let mut e = engine(74_103_101);
    inject_card_into_hand(&mut e, 0, "wander_in_death");
    give_mana(&mut e, 0, gift(&cycle_cases()[4].mana));
    let slot = hand_index_for_card(&e, 0, "wander_in_death");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast with zero targets");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));

    // One target returns exactly that card.
    let mut e = engine(74_103_102);
    let one = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let other = inject_graveyard_card(&mut e, 0, "storm_crow");
    inject_card_into_hand(&mut e, 0, "wander_in_death");
    give_mana(&mut e, 0, gift(&cycle_cases()[4].mana));
    let slot = hand_index_for_card(&e, 0, "wander_in_death");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(one)))
        .expect("cast with one target");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&one].zone, Zone::Hand);
    assert_eq!(e.state.objects[&other].zone, Zone::Graveyard);

    // Three targets is not legal.
    let mut e = engine(74_103_103);
    let three: Vec<u32> = ["grizzly_bears", "storm_crow", "hill_giant"]
        .into_iter()
        .map(|card| inject_graveyard_card(&mut e, 0, card))
        .collect();
    let spell = inject_card_into_hand(&mut e, 0, "wander_in_death");
    give_mana(&mut e, 0, gift(&cycle_cases()[4].mana));
    let before = pool(&e, 0);
    let hand_before = e.state.players[0].hand.clone();
    let slot = hand_index_for_card(&e, 0, "wander_in_death");
    let targets = three
        .iter()
        .flat_map(|object| target_object(*object))
        .collect();
    let error = e
        .apply_scenario_command(0, &cast_spell(slot, targets))
        .expect_err("three graveyard targets exceed the printed up-to-two");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(pool(&e, 0), before, "rejected cast spends no mana");
    assert_eq!(e.state.players[0].hand, hand_before);
    assert_eq!(e.state.objects[&spell].zone, Zone::Hand);
    assert!(e.state.stack.is_empty());

    // A target moved out of the graveyard before resolution is not returned.
    let mut e = engine(74_103_104);
    let stale = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let spell = inject_card_into_hand(&mut e, 0, "wander_in_death");
    give_mana(&mut e, 0, gift(&cycle_cases()[4].mana));
    let slot = hand_index_for_card(&e, 0, "wander_in_death");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(stale)))
        .expect("cast at the graveyard card");
    e.state.players[0]
        .graveyard
        .retain(|object| *object != stale);
    e.state.objects.get_mut(&stale).expect("card").zone = Zone::Exile;
    e.state.players[0].exile.push(stale);
    *e.state.zone_change_generation.entry(stale).or_default() += 1;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&stale].zone, Zone::Exile);
    assert!(!e.state.players[0].hand.contains(&stale));
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q74_cycling_control_fizzles_a_stale_bounce_target() {
    let mut e = engine(74_103_001);
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let spell = inject_card_into_hand(&mut e, 0, "rescind");
    give_mana(&mut e, 0, gift(&cycle_cases()[1].mana));
    let slot = hand_index_for_card(&e, 0, "rescind");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast at the creature");
    move_to_graveyard(&mut e, 1, creature);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Graveyard,
        "the departed permanent is not returned to hand"
    );
    assert!(
        !e.state.players[1].hand.contains(&creature),
        "a fizzled bounce does not move the card"
    );
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Graveyard,
        "the fizzled Rescind resolves to the graveyard"
    );
}
