//! Actual-card scenarios for the reviewed graveyard-to-hand sorcery family.
//!
//! Expectations are printed Oracle values. CR 115.1d defines the optional
//! "up to two" targets; CR 601.2c requires them at cast; CR 608.2b leaves a
//! stale target in its zone; CR 400.7 makes the returned card a new object.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "morbid_plunder",
            mana: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "march_of_the_returned",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "dutiful_return",
            mana: ManaGift {
                b: 1,
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

fn engine(seed: u64, case: &Case) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[case.id]),
        deck_with("forest", &[]),
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

fn cast_at(e: &mut GameEngine, case: &Case, targets: Vec<u32>) -> Result<(), String> {
    ensure_in_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(
        0,
        &cast_spell(slot, targets.into_iter().flat_map(target_object).collect()),
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn graveyard_two_to_hand_batch_cards_return_up_to_two_creature_cards() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(860_000 + index as u64, &case);
        let first = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        let second = inject_graveyard_card(&mut e, 0, "storm_crow");
        cast_at(&mut e, &case, vec![first, second]).expect("cast with two targets");
        pass_both_players(&mut e);
        for card in [first, second] {
            assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{}", case.id);
            assert!(
                e.state.players[0].hand.contains(&card),
                "{}: returned card is in hand",
                case.id
            );
            assert!(
                !e.state.players[0].graveyard.contains(&card),
                "{}: returned card left the graveyard",
                case.id
            );
            assert_eq!(e.state.objects[&card].controller, 0, "{}", case.id);
            assert_eq!(e.state.objects[&card].owner, 0, "{}", case.id);
        }
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn graveyard_two_to_hand_batch_cards_reject_illegal_targets_and_three_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(861_000 + index as u64, &case);
        let spell = inject_graveyard_card(&mut e, 0, "divination");
        let theirs = inject_graveyard_card(&mut e, 1, "grizzly_bears");
        let three = [
            inject_graveyard_card(&mut e, 0, "grizzly_bears"),
            inject_graveyard_card(&mut e, 0, "storm_crow"),
            inject_graveyard_card(&mut e, 0, "hill_giant"),
        ];
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);

        for targets in [vec![spell], vec![theirs], three.to_vec()] {
            let slot = hand_index_for_card(&e, 0, case.id);
            assert!(
                e.apply_scenario_command(
                    0,
                    &cast_spell(slot, targets.into_iter().flat_map(target_object).collect()),
                )
                .is_err(),
                "{}: only up to two creature cards in your graveyard are legal",
                case.id
            );
            assert_eq!(
                pool(&e),
                before,
                "{}: the rejected cast spends no mana",
                case.id
            );
            assert!(e.state.stack.is_empty(), "{}", case.id);
        }
        assert!(e.state.players[0]
            .hand
            .iter()
            .any(|oid| e.state.objects[oid].card_id == case.id));
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(
            e.state.objects[&theirs].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
    }
}

#[test]
fn graveyard_two_to_hand_batch_cards_fizzle_stale_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(862_000 + index as u64, &case);
        let creature = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        cast_at(&mut e, &case, vec![creature]).expect("cast with one target");
        e.state.players[0].graveyard.retain(|oid| *oid != creature);
        e.state.objects.get_mut(&creature).expect("creature").zone = Zone::Exile;
        e.state.players[0].exile.push(creature);
        *e.state.zone_change_generation.entry(creature).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&creature].zone, Zone::Exile, "{}", case.id);
        assert!(
            !e.state.players[0].hand.contains(&creature),
            "{}: a stale target is not returned",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
