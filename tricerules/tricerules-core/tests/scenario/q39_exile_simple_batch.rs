//! Actual-card scenarios for the queue-v39 simple exile cohort.
//!
//! Expectations are printed Oracle values. CR 701.13 exiles the target; CR 115.1a/601.2c require a
//! legal target at cast; CR 608.2b fizzles a stale target; and the owner remains the owner.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    target: Target,
}

#[derive(Clone, Copy)]
enum Target {
    Permanent,
    Land,
    Enchantment,
    Nonland,
    Artifact,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "scour_from_existence",
            mana: ManaGift {
                c: 7,
                ..Default::default()
            },
            target: Target::Permanent,
        },
        Case {
            id: "caustic_rain",
            mana: ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
            target: Target::Land,
        },
        Case {
            id: "erase",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            target: Target::Enchantment,
        },
        Case {
            id: "utter_end",
            mana: ManaGift {
                w: 1,
                b: 1,
                c: 2,
                ..Default::default()
            },
            target: Target::Nonland,
        },
        Case {
            id: "shattering_blow",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            target: Target::Artifact,
        },
    ]
}

fn gift(case: &Case) -> ManaGift {
    ManaGift {
        w: case.mana.w,
        u: case.mana.u,
        b: case.mana.b,
        r: case.mana.r,
        g: case.mana.g,
        c: case.mana.c,
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
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

fn legal_target(e: &mut GameEngine, target: Target) -> u32 {
    match target {
        Target::Permanent => inject_permanent_on_battlefield(e, 1, "bad_moon"),
        Target::Land => inject_permanent_on_battlefield(e, 1, "forest"),
        Target::Enchantment => inject_permanent_on_battlefield(e, 1, "bad_moon"),
        Target::Nonland => inject_creature_on_battlefield(e, 1, "grizzly_bears"),
        Target::Artifact => inject_permanent_on_battlefield(e, 1, "sol_ring"),
    }
}

fn illegal_target(e: &mut GameEngine, target: Target) -> u32 {
    match target {
        Target::Permanent => inject_card_into_hand(e, 1, "grizzly_bears"),
        Target::Land => inject_creature_on_battlefield(e, 1, "grizzly_bears"),
        Target::Enchantment => inject_creature_on_battlefield(e, 1, "grizzly_bears"),
        Target::Nonland => inject_permanent_on_battlefield(e, 1, "forest"),
        Target::Artifact => inject_creature_on_battlefield(e, 1, "grizzly_bears"),
    }
}

fn cast_case(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    let slot = inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(case));
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(e.state.stack.len(), 1, "{}", case.id);
    slot
}

#[test]
fn q39_exile_simple_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(39_060_001);
    let target = inject_permanent_on_battlefield(&mut e, 1, "bad_moon");
    cast_case(&mut e, case, target);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile);
    assert!(e.state.players[1].exile.contains(&target));
    assert!(!e.state.players[1].graveyard.contains(&target));
}

#[test]
fn q39_exile_simple_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(39_060_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    cast_case(&mut e, case, target);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile);
    assert!(e.state.players[1].exile.contains(&target));
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q39_exile_simple_cards_exile_the_chosen_permanent_to_its_owners_exile() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_061_000 + index as u64);
        let target = legal_target(&mut e, case.target);
        let bystander = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");
        let opponent_hand = e.state.players[1].hand.len();
        cast_case(&mut e, case, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{}", case.id);
        assert!(e.state.players[1].exile.contains(&target), "{}", case.id);
        assert!(
            !e.state.players[1].graveyard.contains(&target),
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&bystander].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[1].hand.len(), opponent_hand, "{}", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn q39_exile_simple_cards_reject_an_illegal_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_062_000 + index as u64);
        let target = illegal_target(&mut e, case.target);
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(case));
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
                .is_err(),
            "{}: illegal target rejected",
            case.id
        );
        assert!(e.state.players[0].hand.contains(&slot), "{}", case.id);
        assert!(!e.state.players[1].exile.contains(&target), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q39_exile_simple_cards_fizzle_when_the_target_left() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_063_000 + index as u64);
        let target = legal_target(&mut e, case.target);
        let slot = cast_case(&mut e, case, target);
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: spell fizzles",
            case.id
        );
        assert!(!e.state.players[1].exile.contains(&target), "{}", case.id);
    }
}
