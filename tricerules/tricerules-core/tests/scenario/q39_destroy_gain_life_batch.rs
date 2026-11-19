//! Actual-card scenarios for the queue-v39 destroy-and-gain-life cohort.
//!
//! Expectations are printed Oracle values. CR 701.8 destroys the target; CR 119.3 gains life;
//! CR 115.1a and 601.2c require a legal target at cast; CR 608.2b fizzles a stale target; and
//! CR 608.2c applies the printed order.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    life: i32,
    target: Target,
}

#[derive(Clone, Copy)]
enum Target {
    Creature,
    Nonblack,
    Enchantment,
    Flying,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "lichs_caress",
            mana: ManaGift {
                b: 2,
                c: 3,
                ..Default::default()
            },
            life: 3,
            target: Target::Creature,
        },
        Case {
            id: "dark_offering",
            mana: ManaGift {
                b: 2,
                c: 4,
                ..Default::default()
            },
            life: 3,
            target: Target::Nonblack,
        },
        Case {
            id: "poison_arrow",
            mana: ManaGift {
                b: 2,
                c: 4,
                ..Default::default()
            },
            life: 3,
            target: Target::Nonblack,
        },
        Case {
            id: "ray_of_dissolution",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            life: 3,
            target: Target::Enchantment,
        },
        Case {
            id: "cursebreak",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            life: 2,
            target: Target::Enchantment,
        },
        Case {
            id: "aerial_predation",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            life: 2,
            target: Target::Flying,
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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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
        Target::Creature => inject_creature_on_battlefield(e, 1, "grizzly_bears"),
        Target::Nonblack => inject_creature_on_battlefield(e, 1, "grizzly_bears"),
        Target::Enchantment => inject_permanent_on_battlefield(e, 1, "bad_moon"),
        Target::Flying => inject_creature_on_battlefield(e, 1, "serra_angel"),
    }
}

fn illegal_target(e: &mut GameEngine, target: Target) -> u32 {
    match target {
        Target::Creature => inject_permanent_on_battlefield(e, 1, "forest"),
        Target::Nonblack => inject_creature_on_battlefield(e, 1, "black_knight"),
        Target::Enchantment => inject_creature_on_battlefield(e, 1, "grizzly_bears"),
        Target::Flying => inject_creature_on_battlefield(e, 1, "grizzly_bears"),
    }
}

fn cast_case(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    let slot = inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(case));
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(e.state.stack.len(), 1, "{}: spell is on the stack", case.id);
    slot
}

#[test]
fn q39_destroy_gain_life_canary_simplest() {
    let case = &cases()[4];
    let mut e = engine(39_050_001);
    let target = inject_permanent_on_battlefield(&mut e, 1, "bad_moon");
    cast_case(&mut e, case, target);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, 22);
    assert_eq!(e.state.players[1].life, 20);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q39_destroy_gain_life_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(39_050_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    cast_case(&mut e, case, target);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, 23);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q39_destroy_gain_life_cards_destroy_the_target_and_gain_the_printed_life() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_051_000 + index as u64);
        let target = legal_target(&mut e, case.target);
        let bystander = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");
        cast_case(&mut e, case, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: target destroyed",
            case.id
        );
        assert_eq!(
            e.state.objects[&bystander].zone,
            Zone::Battlefield,
            "{}: bystander survives",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            20 + case.life,
            "{}: life gain",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: opponent life unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            7,
            "{}: hidden hand unchanged",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q39_destroy_gain_life_cards_reject_an_illegal_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_052_000 + index as u64);
        let target = illegal_target(&mut e, case.target);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(case));
        let slot = hand_index_for_card(&e, 0, case.id);
        let before = e.state.players[0].hand.len();
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
                .is_err(),
            "{}: illegal target is rejected",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            before,
            "{}: no card leaves hand",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}: no life gained", case.id);
    }
}

#[test]
fn q39_destroy_gain_life_cards_fizzle_when_the_target_left() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_053_000 + index as u64);
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
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: no life gain on fizzle",
            case.id
        );
        assert!(e.state.players[0].graveyard.contains(&slot), "{}", case.id);
    }
}
