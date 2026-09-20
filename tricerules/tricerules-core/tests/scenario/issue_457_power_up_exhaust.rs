//! Issue #457 — the shared Power-up and Exhaust two-counter activation family through the
//! command boundary.
//!
//! Every expectation is the reviewed printed Oracle behavior, not a copy of generator output.
//! Exact Scryfall records and `rulings_uri` responses were fetched 2026-09-20 against pinned
//! snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`; the Exhaust rulings confirm normal activation
//! timing and that a permanent which leaves and returns is a new object (CR 400.7) whose Exhaust
//! ability can be activated again. Governing CR concepts (verified against the current official
//! 2026-09-25 Comprehensive Rules): CR 602.2b (activated-ability costs), CR 702.193 (Power-up and
//! its entry-turn cost reduction), CR 702.177 (Exhaust and its once-per-object limit), CR 122.1
//! (+1/+1 counters), and CR 400.7 (new object on zone change).

use super::helpers::*;
use tricerules_cards::{CardRegistry, CounterKind};
use tricerules_core::{GameEngine, Zone};

fn mana_pool(engine: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

/// The engine-published mana cost of one battlefield object's activated ability.
fn published_ability_mana_cost(
    engine: &mut GameEngine,
    player: usize,
    object_id: u32,
    ability_index: usize,
) -> String {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.get(player))
        .into_iter()
        .flat_map(|view| view.battlefield_objects.iter())
        .find(|object| object.object_id == object_id)
        .and_then(|object| object.activated_abilities.get(ability_index))
        .map(|ability| ability.mana_cost.clone())
        .unwrap_or_else(|| panic!("missing published ability {ability_index} for {object_id}"))
}

/// Cast `card` from an injected hand card, resolve it, and return its battlefield object id.
fn cast_creature(engine: &mut GameEngine, card: &str) -> u32 {
    let source = inject_card_into_hand(engine, 0, card);
    let slot = hand_index_for_card(engine, 0, card);
    grant_pool(engine, 0);
    semantic::accepted(engine, 0, &cast_spell(slot, vec![]));
    semantic::complete(engine, 24, |_| None).require_exercised();
    semantic::assert_main_priority(engine, 0);
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Battlefield,
        "{card} must resolve onto the battlefield"
    );
    source
}

/// Relocate an owned permanent off the battlefield through the dev command boundary.
fn move_permanent_to_graveyard(engine: &mut GameEngine, player: usize, card_id: &str) {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    let player_id = engine.state.players[player].id;
    let name = CardRegistry::global()
        .get(card_id)
        .expect("registered card")
        .name
        .clone();
    engine.enable_dev_commands();
    engine
        .apply_command(
            player_id,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player_id,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: name,
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the permanent to the graveyard");
}

#[test]
fn issue_457_power_up_adds_two_counters_and_stays_once_per_object() {
    let mut engine = semantic::main_phase(457_100);
    let source = cast_creature(&mut engine, "serpent_specialist");
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, source, 0),
        "{3}",
        "Serpent Specialist's {{G}} reduces the printed {{3}}{{G}} ability while it entered this turn"
    );

    let before = mana_pool(&engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let activation = activate_ability_for(&engine, source, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);
    semantic::complete(&mut engine, 16, |_| None).require_exercised();
    assert_eq!(
        mana_pool(&engine),
        before,
        "the entry-turn activation paid exactly the reduced {{3}}"
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "the ability adds exactly two +1/+1 counters"
    );
    assert!(
        !engine.state.objects[&source].tapped,
        "the ability does not tap its source"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let before = mana_pool(&engine);
    let uses_before = engine.state.activation_uses_per_object.clone();
    apply_ability(&mut engine, 0, source, 0, vec![])
        .expect_err("a Power-up ability can be activated only once per object");
    assert_eq!(
        mana_pool(&engine),
        before,
        "a rejected second activation pays nothing"
    );
    assert_eq!(engine.state.activation_uses_per_object, uses_before);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
}

#[test]
fn issue_457_power_up_is_full_price_after_the_entry_turn() {
    let mut engine = semantic::main_phase(457_200);
    let source = cast_creature(&mut engine, "brave_brawler");
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, source, 0),
        "{3}",
        "Brave Brawler's {{1}}{{W}} reduces the printed {{4}}{{W}} ability on its entry turn"
    );

    end_active_turn(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, source, 0),
        "{4}{W}",
        "the previous turn's entry no longer reduces the ability"
    );

    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine);
    semantic::assert_main_priority(&engine, 0);
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, source, 0),
        "{4}{W}"
    );

    let before = mana_pool(&engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            w: 1,
            ..Default::default()
        },
    );
    let activation = activate_ability_for(&engine, source, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);
    semantic::complete(&mut engine, 16, |_| None).require_exercised();
    assert_eq!(
        mana_pool(&engine),
        before,
        "the later-turn activation paid the full {{4}}{{W}}"
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
}

#[test]
fn issue_457_exhaust_is_full_price_on_the_entry_turn() {
    let mut engine = semantic::main_phase(457_300);
    let source = cast_creature(&mut engine, "prowcatcher_specialist");
    assert_eq!(
        published_ability_mana_cost(&mut engine, 0, source, 0),
        "{3}{R}",
        "Exhaust has no entry-turn reduction"
    );

    let before = mana_pool(&engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            r: 1,
            ..Default::default()
        },
    );
    let activation = activate_ability_for(&engine, source, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);
    semantic::complete(&mut engine, 16, |_| None).require_exercised();
    assert_eq!(
        mana_pool(&engine),
        before,
        "the entry-turn activation paid exactly the full {{3}}{{R}}"
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert!(
        !engine.state.objects[&source].tapped,
        "the ability does not tap its source"
    );
}

#[test]
fn issue_457_exhaust_can_be_activated_again_after_a_zone_change() {
    let mut engine = semantic::main_phase(457_400);
    let source = cast_creature(&mut engine, "skystreak_engineer");
    let first_generation = semantic::generation(&engine, source);

    let before = mana_pool(&engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            u: 1,
            ..Default::default()
        },
    );
    let activation = activate_ability_for(&engine, source, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);
    semantic::complete(&mut engine, 16, |_| None).require_exercised();
    assert_eq!(
        mana_pool(&engine),
        before,
        "the first activation paid the full {{4}}{{U}}"
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [false],
        "the first object's Exhaust ability is spent"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            u: 1,
            ..Default::default()
        },
    );
    let before = mana_pool(&engine);
    apply_ability(&mut engine, 0, source, 0, vec![])
        .expect_err("the first object's Exhaust allowance stays spent");
    assert_eq!(mana_pool(&engine), before);

    // CR 400.7: leaving and returning creates a new object with a fresh allowance.
    move_permanent_to_graveyard(&mut engine, 0, "skystreak_engineer");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    let returned = move_ready_to_battlefield(&mut engine, 0, "skystreak_engineer");
    assert_eq!(
        returned, source,
        "the relocated card keeps its object id with a new generation"
    );
    assert!(
        semantic::generation(&engine, source) > first_generation,
        "the zone change must create a new object"
    );
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [true],
        "the new object can activate its Exhaust ability again"
    );

    let before = mana_pool(&engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            u: 1,
            ..Default::default()
        },
    );
    let activation = activate_ability_for(&engine, source, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);
    semantic::complete(&mut engine, 16, |_| None).require_exercised();
    assert_eq!(
        mana_pool(&engine),
        before,
        "the new object paid the full {{4}}{{U}} again"
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
}
