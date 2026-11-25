//! Actual-card scenarios for the queue-v78 02-attack-removal-rider cohort.
//!
//! Expectations are printed Oracle values. CR 115.1a and 601.2c require one attacking-creature
//! target as the spell is cast; CR 608.2b revalidates it and a stale sole target fizzles the whole
//! spell, so the pinned Bright Reprisal / Second Thoughts rulings deny the draw. CR 701.8 destroys,
//! CR 701.13 exiles and CR 119.3 gains life.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Removal {
    Destroy,
    Exile,
}

#[derive(Clone, Copy)]
enum Rider {
    Draw,
    GainLife(u32),
}

struct Case {
    id: &'static str,
    mana: ManaGift,
    removal: Removal,
    rider: Rider,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "bright_reprisal",
            mana: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            removal: Removal::Destroy,
            rider: Rider::Draw,
        },
        Case {
            id: "second_thoughts",
            mana: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            removal: Removal::Exile,
            rider: Rider::Draw,
        },
        Case {
            id: "silverstrike",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            removal: Removal::Destroy,
            rider: Rider::GainLife(3),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
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

fn declare_an_attacker(e: &mut GameEngine) -> u32 {
    let attacker = inject_creature_on_battlefield(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("active pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass begin combat");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    attacker
}

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) -> Result<u32, String> {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .map(|_| e.state.stack.last().expect("spell").id)
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn q78_attack_removal_rider_canary_simplest_silverstrike_destroys_and_gains_three() {
    let case = &cases()[2];
    let mut e = engine(78_200_001);
    let attacker = declare_an_attacker(&mut e);
    let spell = cast_at(&mut e, case, attacker).expect("cast Silverstrike");
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "the printed {{3}}{{W}} is spent"
    );
    assert_eq!(
        e.state.objects[&attacker].zone,
        Zone::Battlefield,
        "the creature is destroyed only on resolution"
    );
    assert_eq!(e.state.players[0].life, 20);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&attacker].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, 23, "Silverstrike gains 3 life");
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn q78_attack_removal_rider_canary_hardest_bright_reprisal_draws_after_a_legal_destroy() {
    let case = &cases()[0];
    let mut e = engine(78_200_002);
    let attacker = declare_an_attacker(&mut e);
    let hand_before = e.state.players[0].hand.len();
    let library_before = e.state.players[0].library.len();
    cast_at(&mut e, case, attacker).expect("cast Bright Reprisal");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the injected spell leaves the hand after being cast"
    );
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "the printed {{4}}{{W}} is spent"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&attacker].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the controller draws one card"
    );
    assert_eq!(e.state.players[0].library.len(), library_before - 1);
}

#[test]
fn q78_attack_removal_rider_cards_apply_their_printed_removal_and_rider() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(78_201_000 + index as u64);
        let attacker = declare_an_attacker(&mut e);
        let hand_before = e.state.players[0].hand.len();
        cast_at(&mut e, &case, attacker).unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        resolve_entire_stack_two_player(&mut e);
        match case.removal {
            Removal::Destroy => assert_eq!(
                e.state.objects[&attacker].zone,
                Zone::Graveyard,
                "{}: destroyed",
                case.id
            ),
            Removal::Exile => assert_eq!(
                e.state.objects[&attacker].zone,
                Zone::Exile,
                "{}: exiled",
                case.id
            ),
        }
        match case.rider {
            Rider::Draw => assert_eq!(
                e.state.players[0].hand.len(),
                hand_before + 1,
                "{}: drew one",
                case.id
            ),
            Rider::GainLife(amount) => assert_eq!(
                e.state.players[0].life,
                20 + amount as i32,
                "{}: gained the printed life",
                case.id
            ),
        }
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q78_attack_removal_rider_cards_reject_a_nonattacking_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(78_202_000 + index as u64);
        let bystander = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(bystander)))
                .is_err(),
            "{}: a creature that is not attacking is not a legal target",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&bystander].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q78_attack_removal_rider_cards_fizzle_without_the_rider_on_a_stale_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(78_203_000 + index as u64);
        let attacker = declare_an_attacker(&mut e);
        let hand_before = e.state.players[0].hand.len();
        cast_at(&mut e, &case, attacker).expect("cast at the attacker");
        e.state.players[0]
            .battlefield
            .retain(|oid| *oid != attacker);
        e.state.objects.get_mut(&attacker).expect("attacker").zone = Zone::Exile;
        e.state.players[0].exile.push(attacker);
        *e.state.zone_change_generation.entry(attacker).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&attacker].zone,
            Zone::Exile,
            "{}: the stale attacker stays exiled",
            case.id
        );
        match case.rider {
            Rider::Draw => assert_eq!(
                e.state.players[0].hand.len(),
                hand_before,
                "{}: the pinned ruling denies the draw",
                case.id
            ),
            Rider::GainLife(_) => assert_eq!(
                e.state.players[0].life, 20,
                "{}: no life is gained",
                case.id
            ),
        }
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q78_attack_removal_rider_legal_indestructible_target_still_draws() {
    let case = &cases()[0];
    let mut e = engine(78_204_000);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let indestructible = inject_creature_on_battlefield(&mut e, 0, "darksteel_myr");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("active pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass begin combat");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker, indestructible]))
        .expect("declare both attackers");
    let hand_before = e.state.players[0].hand.len();
    cast_at(&mut e, case, indestructible).expect("cast at the indestructible attacker");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&indestructible].zone,
        Zone::Battlefield,
        "indestructible survives the destroy instruction"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the 2017-09-29 ruling still draws for a legal indestructible target"
    );
}

#[test]
fn q78_attack_removal_rider_cards_leave_a_creature_that_stopped_attacking() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(78_205_000 + index as u64);
        let attacker = declare_an_attacker(&mut e);
        let hand_before = e.state.players[0].hand.len();
        cast_at(&mut e, &case, attacker).expect("cast at the attacker");
        let combat = e.state.combat.as_mut().expect("combat");
        combat.attacking.retain(|oid| *oid != attacker);
        combat.attack_assignments.remove(&attacker);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&attacker].zone,
            Zone::Battlefield,
            "{}: a creature that stopped attacking is not removed",
            case.id
        );
        match case.rider {
            Rider::Draw => assert_eq!(
                e.state.players[0].hand.len(),
                hand_before,
                "{}: the spell fizzles and does not draw",
                case.id
            ),
            Rider::GainLife(_) => assert_eq!(
                e.state.players[0].life, 20,
                "{}: the spell fizzles and gains no life",
                case.id
            ),
        }
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q78_attack_removal_rider_silverstrike_still_gains_three_against_an_indestructible_attacker() {
    let case = &cases()[2];
    let mut e = engine(78_206_000);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let indestructible = inject_creature_on_battlefield(&mut e, 0, "darksteel_myr");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("active pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass begin combat");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker, indestructible]))
        .expect("declare both attackers");
    cast_at(&mut e, case, indestructible).expect("cast at the indestructible attacker");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&indestructible].zone,
        Zone::Battlefield,
        "indestructible survives the destroy instruction"
    );
    assert_eq!(
        e.state.players[0].life, 23,
        "Silverstrike still gains 3 for a legal target"
    );
}
