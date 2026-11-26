//! Actual-card scenarios for Bounty Hunter (#535).
//!
//! Expectations come from the printed Oracle text. CR 602.2/118.3 govern activation and the tap
//! cost, CR 115.1 governs each single target, CR 122.1 makes a bounty counter an ordinary named
//! counter with no rules of its own, CR 608.2b re-checks the target on resolution, and CR 701.8
//! governs destroy.
use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{GameEngine, Zone};

const PUT_BOUNTY: u32 = 0;
const DESTROY_BOUNTIED: u32 = 1;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["bounty_hunter"]),
        deck_with("forest", &["grizzly_bears", "vampire_bats"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn untap(engine: &mut GameEngine, object: u32) {
    engine.state.objects.get_mut(&object).unwrap().tapped = false;
}

#[test]
fn issue_535_puts_exactly_one_bounty_counter_with_no_other_effect() {
    let mut engine = engine(535_001);
    let hunter = inject_creature_on_battlefield(&mut engine, 0, "bounty_hunter");
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    apply_ability(&mut engine, 0, hunter, PUT_BOUNTY, target_object(bears))
        .expect("put a bounty counter on a nonblack creature");
    assert!(
        engine.state.objects[&hunter].tapped,
        "the tap cost was paid"
    );
    resolve_entire_stack_two_player(&mut engine);

    let target = &engine.state.objects[&bears];
    assert_eq!(target.counter_count(CounterKind::Bounty), 1);
    assert_eq!(
        target.counters.len(),
        1,
        "only the bounty counter was added"
    );
    assert_eq!(target.zone, Zone::Battlefield);
    assert_eq!(
        (
            engine.effective_power(bears),
            engine.effective_toughness(bears)
        ),
        (Some(2), Some(2)),
        "a bounty counter doesn't change power or toughness"
    );
}

#[test]
fn issue_535_bounty_counter_cannot_target_a_black_creature() {
    let mut engine = engine(535_002);
    let hunter = inject_creature_on_battlefield(&mut engine, 0, "bounty_hunter");
    let bats = inject_creature_on_battlefield(&mut engine, 1, "vampire_bats");

    assert!(
        apply_ability(&mut engine, 0, hunter, PUT_BOUNTY, target_object(bats)).is_err(),
        "a black creature isn't a legal target"
    );
    assert!(
        !engine.state.objects[&hunter].tapped,
        "rejected activation pays no cost"
    );
    assert_eq!(
        engine.state.objects[&bats].counter_count(CounterKind::Bounty),
        0
    );
}

#[test]
fn issue_535_destroy_requires_a_bounty_counter() {
    let mut engine = engine(535_003);
    let hunter = inject_creature_on_battlefield(&mut engine, 0, "bounty_hunter");
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    assert!(
        apply_ability(
            &mut engine,
            0,
            hunter,
            DESTROY_BOUNTIED,
            target_object(bears)
        )
        .is_err(),
        "a creature without a bounty counter isn't a legal target"
    );
    assert!(
        !engine.state.objects[&hunter].tapped,
        "rejected activation pays no cost"
    );
    assert_eq!(engine.state.objects[&bears].zone, Zone::Battlefield);
}

#[test]
fn issue_535_destroys_a_creature_with_a_bounty_counter() {
    let mut engine = engine(535_004);
    let hunter = inject_creature_on_battlefield(&mut engine, 0, "bounty_hunter");
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    apply_ability(&mut engine, 0, hunter, PUT_BOUNTY, target_object(bears)).expect("bounty");
    resolve_entire_stack_two_player(&mut engine);
    untap(&mut engine, hunter);

    apply_ability(
        &mut engine,
        0,
        hunter,
        DESTROY_BOUNTIED,
        target_object(bears),
    )
    .expect("destroy the bountied creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&bears].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&hunter].zone, Zone::Battlefield);
}

#[test]
fn issue_535_destroy_fizzles_when_the_bounty_counter_is_removed() {
    let mut engine = engine(535_005);
    let hunter = inject_creature_on_battlefield(&mut engine, 0, "bounty_hunter");
    let bears = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&bears)
        .unwrap()
        .set_counter(CounterKind::Bounty, 1);

    apply_ability(
        &mut engine,
        0,
        hunter,
        DESTROY_BOUNTIED,
        target_object(bears),
    )
    .expect("destroy is legal while the counter is present");
    engine
        .state
        .objects
        .get_mut(&bears)
        .unwrap()
        .set_counter(CounterKind::Bounty, 0);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&bears].zone,
        Zone::Battlefield,
        "the now-illegal target isn't destroyed"
    );
}
