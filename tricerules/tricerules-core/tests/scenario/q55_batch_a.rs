//! Actual-card scenarios for the queue-v55 batch A cohorts.
//!
//! Expectations are printed Oracle values. The relevant rules are CR 115.1/601.2c
//! (targets are chosen as the spell is cast), CR 120.3 (damage), CR 119.3 (life gain/loss),
//! CR 121.1 (draw), CR 400.3 (a card returns to its owner's hand), CR 701.22 (scry),
//! CR 701.25 (surveil), CR 701.34 (proliferate), CR 702.29a (cycling), CR 110.2 (reanimated
//! permanents enter under the effect controller's control), and CR 608.2b (a spell with an
//! illegal sole/first target fails to resolve, so no rider happens).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChoiceKind, SubmitResolutionChoice};

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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn choice(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            chosen_object_ids: objects,
            chosen_player_ids: players,
            ..Default::default()
        })),
    }
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn hand_ability(e: &GameEngine, source: u32, ability_index: u32) -> RuledCommand {
    use tricerules_proto::ruled::v1::{AbilitySourceZone, ActivateAbility};
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
            ability_index,
            ..Default::default()
        })),
    }
}

// ---------------------------------------------------------------------------
// 01-damage-creature-planeswalker

struct BurnCase {
    id: &'static str,
    mana: ManaGift,
    amount: u32,
}

fn burn_cases() -> [BurnCase; 4] {
    [
        BurnCase {
            id: "strangle",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            amount: 3,
        },
        BurnCase {
            id: "thundering_rebuke",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            amount: 4,
        },
        BurnCase {
            id: "flame-blessed_bolt",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            amount: 2,
        },
        BurnCase {
            id: "jayas_firenado",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            amount: 5,
        },
    ]
}

#[test]
fn q55_a_canary_simplest_strangle_damages_a_creature_exactly() {
    let mut e = engine(55_100_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let spell = inject_card_into_hand(&mut e, 0, "strangle");
    let slot = hand_index_for_card(&e, 0, "strangle");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Strangle");
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact {{R}} cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].damage, 3);
    assert_eq!(e.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(e.state.players[1].life, 20, "a creature is not a player");
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn q55_a_creature_or_planeswalker_burn_deals_exact_damage() {
    for (index, case) in burn_cases().into_iter().enumerate() {
        let mut e = engine(55_101_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].damage, case.amount,
            "{}: exact printed damage",
            case.id
        );
    }

    let mut walker_engine = engine(55_101_100);
    let walker = inject_permanent_on_battlefield(&mut walker_engine, 1, "jace_beleren");
    walker_engine
        .state
        .objects
        .get_mut(&walker)
        .expect("planeswalker")
        .set_counter(CounterKind::Loyalty, 10);
    give_mana(
        &mut walker_engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut walker_engine, 0, "thundering_rebuke");
    let slot = hand_index_for_card(&walker_engine, 0, "thundering_rebuke");
    walker_engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(walker)))
        .expect("cast Thundering Rebuke at a planeswalker");
    resolve_entire_stack_two_player(&mut walker_engine);
    assert_eq!(
        walker_engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        6,
        "four damage removes four loyalty counters"
    );
}

#[test]
fn q55_a_flame_blessed_bolt_exiles_a_creature_that_would_die() {
    let mut e = engine(55_101_200);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "flame-blessed_bolt");
    let slot = hand_index_for_card(&e, 0, "flame-blessed_bolt");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Flame-Blessed Bolt");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Exile,
        "the lethal-damaged creature is exiled instead of dying"
    );
    assert!(!e.state.players[1].graveyard.contains(&target));
}

#[test]
fn q55_a_flame_blessed_bolt_exiles_a_later_death_that_turn() {
    let mut e = engine(55_101_250);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "flame-blessed_bolt");
    let slot = hand_index_for_card(&e, 0, "flame-blessed_bolt");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Flame-Blessed Bolt");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&target].damage, 2, "the 4/4 survives");

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    let bolt = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_scenario_command(0, &cast_spell(bolt, target_object(target)))
        .expect("cast Lightning Bolt later in the turn");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Exile,
        "the pinned ruling applies the exile replacement to a later death this turn"
    );
}

#[test]
fn q55_a_jayas_firenado_stale_target_does_not_scry() {
    let mut e = engine(55_101_350);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "jayas_firenado");
    let slot = hand_index_for_card(&e, 0, "jayas_firenado");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Jaya's Firenado");
    move_to_graveyard(&mut e, 1, target);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].damage, 0,
        "no damage after fizzling"
    );
    assert!(
        e.state.pending_resolution.is_none(),
        "the pinned ruling denies the scry when the target is illegal"
    );
}

#[test]
fn q55_a_jayas_firenado_damages_then_scries() {
    let mut e = engine(55_101_300);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 20, 20);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "jayas_firenado");
    let slot = hand_index_for_card(&e, 0, "jayas_firenado");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Jaya's Firenado");
    let batch = {
        e.apply_scenario_command(0, &pass()).expect("pass");
        e.apply_scenario_command(1, &pass()).expect("resolve")
    };
    assert_eq!(e.state.objects[&target].damage, 5);
    let request = find_resolution_choice(&batch).expect("scry parks");
    assert_eq!(request.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((request.min, request.max), (0, 1));
}

#[test]
fn q55_a_creature_or_planeswalker_burn_rejects_players_and_fizzles_stale_targets() {
    let mut e = engine(55_102_001);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "strangle");
    let slot = hand_index_for_card(&e, 0, "strangle");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .is_err(),
        "a player is not a creature or planeswalker"
    );
    assert_eq!(pool(&e, 0), (0, 0, 0, 1, 0, 0), "rejected cast keeps {{R}}");
    assert!(e.state.stack.is_empty());

    let mut stale = engine(55_102_002);
    let target = inject_creature_with_stats(&mut stale, 1, "grizzly_bears", 20, 20);
    give_mana(
        &mut stale,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut stale, 0, "thundering_rebuke");
    let slot = hand_index_for_card(&stale, 0, "thundering_rebuke");
    stale
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Thundering Rebuke");
    move_to_graveyard(&mut stale, 1, target);
    let spell = stale.state.stack.last().map(|item| item.id);
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.players[1].life, 20, "stale target fizzles");
    assert_eq!(
        stale.state.objects[&target].damage, 0,
        "no damage is marked"
    );
    if let Some(spell) = spell {
        assert_eq!(
            stale.state.objects[&spell].zone,
            Zone::Graveyard,
            "the fizzled spell still goes to its owner's graveyard"
        );
    }
}

// ---------------------------------------------------------------------------
// 02-damage-any-target-riders

#[test]
fn q55_a_canary_simplest_zap_damages_a_player_and_draws() {
    let mut e = engine(55_110_001);
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "zap");
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "zap");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Zap");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 19, "one damage");
    assert_eq!(e.state.players[0].library.len(), library_before - 1);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the spell left hand and the draw replaced it"
    );
}

#[test]
fn q55_a_damage_any_target_riders_apply_their_printed_effects() {
    for (index, (id, mana, amount, gain, draw)) in [
        (
            "ember_shot",
            ManaGift {
                r: 1,
                c: 6,
                ..Default::default()
            },
            3,
            0,
            true,
        ),
        (
            "zap",
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            1,
            0,
            true,
        ),
        (
            "dark_nourishment",
            ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            3,
            3,
            false,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(55_111_000 + index as u64);
        let p0_life = e.state.players[0].life;
        let library_before = e.state.players[0].library.len();
        give_mana(&mut e, 0, mana);
        inject_card_into_hand(&mut e, 0, id);
        let slot = hand_index_for_card(&e, 0, id);
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .unwrap_or_else(|error| panic!("{id}: cast: {error:?}"));
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{id}: exact cost");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.players[1].life, 20 - amount, "{id}: damage");
        assert_eq!(e.state.players[0].life, p0_life + gain, "{id}: life gain");
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - i32::from(draw) as usize,
            "{id}: draw rider"
        );
    }
}

#[test]
fn q55_a_volt_charge_damages_then_proliferates_chosen_counters() {
    let mut e = engine(55_112_001);
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.state
        .objects
        .get_mut(&creature)
        .expect("creature")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    e.state.players[0].counters.insert(CounterKind::Poison, 1);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "volt_charge");
    let slot = hand_index_for_card(&e, 0, "volt_charge");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Volt Charge");
    let batch = {
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        e.apply_scenario_command(1, &pass()).expect("resolve")
    };
    let request = find_resolution_choice(&batch).expect("proliferate parks for choices");
    assert_eq!(request.choice_kind(), ChoiceKind::Proliferate);
    assert_eq!(e.state.players[1].life, 17, "three damage");
    e.apply_scenario_command(0, &choice(vec![creature], vec![0]))
        .expect("proliferate the creature and player");
    assert_eq!(
        e.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        e.state.players[0]
            .counters
            .get(&CounterKind::Poison)
            .copied(),
        Some(2)
    );
}

#[test]
fn q55_a_volt_charge_and_dark_nourishment_fizzle_stale_targets_without_riders() {
    let mut volt = engine(55_113_001);
    let target = inject_creature_with_stats(&mut volt, 1, "grizzly_bears", 20, 20);
    volt.state
        .objects
        .get_mut(&target)
        .expect("target")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    give_mana(
        &mut volt,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut volt, 0, "volt_charge");
    let slot = hand_index_for_card(&volt, 0, "volt_charge");
    volt.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Volt Charge");
    move_to_graveyard(&mut volt, 1, target);
    resolve_entire_stack_two_player(&mut volt);
    assert!(
        volt.state.pending_resolution.is_none(),
        "no proliferation after a fizzled spell"
    );

    let mut dark = engine(55_113_002);
    let target = inject_creature_with_stats(&mut dark, 1, "grizzly_bears", 20, 20);
    give_mana(
        &mut dark,
        0,
        ManaGift {
            b: 1,
            c: 4,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut dark, 0, "dark_nourishment");
    let slot = hand_index_for_card(&dark, 0, "dark_nourishment");
    dark.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Dark Nourishment");
    move_to_graveyard(&mut dark, 1, target);
    resolve_entire_stack_two_player(&mut dark);
    assert_eq!(
        dark.state.players[0].life, 20,
        "no life after a fizzled spell"
    );
}

#[test]
fn q55_a_spark_spray_deals_one_and_cycles_for_red() {
    let mut e = engine(55_114_001);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "spark_spray");
    let slot = hand_index_for_card(&e, 0, "spark_spray");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Spark Spray");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 19);

    let mut cyc = engine(55_114_002);
    let card = inject_card_into_hand(&mut cyc, 0, "spark_spray");
    let drawn = inject_library_card(&mut cyc, 0, "forest");
    cyc.state.players[0].library.retain(|oid| *oid != drawn);
    cyc.state.players[0].library.push_front(drawn);
    give_mana(
        &mut cyc,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    cyc.apply_scenario_command(0, &hand_ability(&cyc, card, 0))
        .expect("cycle Spark Spray");
    resolve_entire_stack_two_player(&mut cyc);
    assert_eq!(cyc.state.objects[&card].zone, Zone::Graveyard);
    assert_eq!(cyc.state.objects[&drawn].zone, Zone::Hand);
    assert_eq!(pool(&cyc, 0), (0, 0, 0, 0, 0, 0), "exact cycling cost");
}

// ---------------------------------------------------------------------------
// 03-bounce-riders

#[test]
fn q55_a_canary_simplest_vapor_snag_returns_a_creature_and_charges_its_controller() {
    let mut e = engine(55_120_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "vapor_snag");
    let slot = hand_index_for_card(&e, 0, "vapor_snag");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Vapor Snag");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Hand);
    assert!(e.state.players[1].hand.contains(&target), "owner's hand");
    assert_eq!(
        e.state.players[1].life, 19,
        "the creature's controller loses 1"
    );
    assert_eq!(e.state.players[0].life, 20, "the caster does not lose life");
}

#[test]
fn q55_a_bounce_riders_resolve_in_printed_order() {
    // Voyage's End: scry 1 after the bounce.
    let mut voyage = engine(55_121_001);
    let target = inject_creature_on_battlefield(&mut voyage, 1, "grizzly_bears");
    give_mana(
        &mut voyage,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut voyage, 0, "voyages_end");
    let slot = hand_index_for_card(&voyage, 0, "voyages_end");
    voyage
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Voyage's End");
    let batch = {
        voyage.apply_scenario_command(0, &pass()).expect("pass");
        voyage.apply_scenario_command(1, &pass()).expect("resolve")
    };
    let request = find_resolution_choice(&batch).expect("scry parks");
    assert_eq!(request.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((request.min, request.max), (0, 1));
    assert_eq!(voyage.state.objects[&target].zone, Zone::Hand);
    assert!(voyage.state.players[1].hand.contains(&target));

    // Unexplained Disappearance: surveil 1 after the bounce.
    let mut unexplained = engine(55_121_002);
    let target = inject_creature_on_battlefield(&mut unexplained, 1, "grizzly_bears");
    give_mana(
        &mut unexplained,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut unexplained, 0, "unexplained_disappearance");
    let slot = hand_index_for_card(&unexplained, 0, "unexplained_disappearance");
    unexplained
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Unexplained Disappearance");
    let batch = {
        unexplained
            .apply_scenario_command(0, &pass())
            .expect("pass");
        unexplained
            .apply_scenario_command(1, &pass())
            .expect("resolve")
    };
    let request = find_resolution_choice(&batch).expect("surveil parks");
    assert_eq!(request.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(unexplained.state.objects[&target].zone, Zone::Hand);
    assert!(unexplained.state.players[1].hand.contains(&target));

    // Dramatic Rescue: gain 2 life after the bounce.
    let mut rescue = engine(55_121_003);
    let target = inject_creature_on_battlefield(&mut rescue, 1, "grizzly_bears");
    give_mana(
        &mut rescue,
        0,
        ManaGift {
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut rescue, 0, "dramatic_rescue");
    let slot = hand_index_for_card(&rescue, 0, "dramatic_rescue");
    rescue
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Dramatic Rescue");
    resolve_entire_stack_two_player(&mut rescue);
    assert_eq!(rescue.state.objects[&target].zone, Zone::Hand);
    assert!(rescue.state.players[1].hand.contains(&target));
    assert_eq!(rescue.state.players[0].life, 22);

    // Depths of Desire: create a Treasure under the caster's control.
    let mut depths = engine(55_121_004);
    let target = inject_creature_on_battlefield(&mut depths, 1, "grizzly_bears");
    give_mana(
        &mut depths,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut depths, 0, "depths_of_desire");
    let slot = hand_index_for_card(&depths, 0, "depths_of_desire");
    depths
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Depths of Desire");
    resolve_entire_stack_two_player(&mut depths);
    assert_eq!(depths.state.objects[&target].zone, Zone::Hand);
    assert!(depths.state.players[1].hand.contains(&target));
    assert_eq!(
        battlefield_token_oids(&depths, 0, "treasure").len(),
        1,
        "the caster controls the Treasure"
    );
    assert!(battlefield_token_oids(&depths, 1, "treasure").is_empty());
}

#[test]
fn q55_a_bounce_riders_fizzle_with_the_stale_target() {
    for (index, id) in [
        "vapor_snag",
        "voyages_end",
        "unexplained_disappearance",
        "dramatic_rescue",
        "depths_of_desire",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(55_122_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let mana = match id {
            "vapor_snag" => ManaGift {
                u: 1,
                ..Default::default()
            },
            "voyages_end" | "unexplained_disappearance" => ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            "dramatic_rescue" => ManaGift {
                w: 1,
                u: 1,
                ..Default::default()
            },
            _ => ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        };
        give_mana(&mut e, 0, mana);
        inject_card_into_hand(&mut e, 0, id);
        let slot = hand_index_for_card(&e, 0, id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .unwrap_or_else(|error| panic!("{id}: cast: {error:?}"));
        if id == "vapor_snag" {
            // A real removal spell records last-known information; a buggy Vapor Snag would
            // then observe the departed creature's controller and lose 1 life.
            e.apply_scenario_command(0, &pass())
                .expect("pass priority to the opponent");
            give_mana(
                &mut e,
                1,
                ManaGift {
                    b: 2,
                    c: 1,
                    ..Default::default()
                },
            );
            inject_card_into_hand(&mut e, 1, "murder");
            let murder = hand_index_for_card(&e, 1, "murder");
            e.apply_scenario_command(1, &cast_spell(murder, target_object(target)))
                .expect("cast Murder in response");
        } else {
            move_to_graveyard(&mut e, 1, target);
        }
        let first = e.state.priority_player_id();
        let second = 1 - first;
        let batch = if id == "vapor_snag" {
            resolve_entire_stack_two_player(&mut e);
            None
        } else {
            Some({
                e.apply_scenario_command(first, &pass())
                    .unwrap_or_else(|error| panic!("{id}: first pass: {error:?}"));
                e.apply_scenario_command(second, &pass())
                    .unwrap_or_else(|error| panic!("{id}: second pass: {error:?}"))
            })
        };
        if let Some(batch) = &batch {
            assert!(
                life_changes_in(batch).is_empty(),
                "{id}: a fizzled spell emits no life change"
            );
        }
        assert_eq!(e.state.players[1].life, 20, "{id}: no life rider");
        assert_eq!(e.state.players[0].life, 20, "{id}: no caster life rider");
        assert!(
            e.state.pending_resolution.is_none(),
            "{id}: no scry/surveil"
        );
        assert!(
            battlefield_token_oids(&e, 0, "treasure").is_empty(),
            "{id}: no Treasure rider"
        );
    }
}
