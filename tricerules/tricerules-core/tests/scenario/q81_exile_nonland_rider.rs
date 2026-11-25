//! Actual-card scenarios for the queue-v81 02-exile-nonland-rider cohort.
//!
//! Expectations are printed Oracle values. CR 701.13 exiles the target, CR 115.1a/601.2c require
//! the nonland permanent target, and CR 608.2b removes the whole spell when that sole target is
//! illegal. The 2024-04-12 and 2020-06-23 rulings confirm the riders do not happen on a stale
//! target. Treasure creation follows CR 111.10a/701.7; the Soldier token follows CR 111.3.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    rider: Rider,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Rider {
    LoseThree,
    Treasure,
    Soldier,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "anguished_unmaking",
            mana: ManaGift {
                w: 1,
                b: 1,
                c: 1,
                ..Default::default()
            },
            rider: Rider::LoseThree,
        },
        Case {
            id: "buy_your_silence",
            mana: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            rider: Rider::Treasure,
        },
        Case {
            id: "secure_the_scene",
            mana: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            rider: Rider::Soldier,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &["grizzly_bears", "forest"]),
        deck_with("island", &["grizzly_bears", "forest"]),
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

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

fn token_card_ids(e: &GameEngine, player: usize) -> Vec<String> {
    e.state.players[player]
        .battlefield
        .iter()
        .filter_map(|id| e.state.objects.get(id))
        .filter(|object| object.token_origin.is_some() || object.token_faces.is_some())
        .map(|object| object.card_id.clone())
        .collect()
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("target").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn q81_exile_rider_canary_simplest_anguished_unmaking_exiles_and_loses_three() {
    let case = &cases()[0];
    let mut e = engine(81_200_001);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    cast_at(&mut e, case, target);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile, "exiled");
    assert_eq!(e.state.players[0].life, 17, "the caster loses 3 life");
    assert_eq!(
        e.state.players[1].life, 20,
        "the target's controller is untouched"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed cost is spent");
    assert!(token_card_ids(&e, 1).is_empty(), "no token rider");
}

#[test]
fn q81_exile_rider_canary_hardest_secure_the_scene_gives_controller_a_soldier() {
    let case = &cases()[2];
    let mut e = engine(81_200_002);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    cast_at(&mut e, case, target);
    assert_eq!(e.state.objects[&target].zone, Zone::Exile, "exiled");
    assert_eq!(
        token_card_ids(&e, 1),
        vec!["soldier_w_1_1".to_string()],
        "the exiled permanent's controller creates the Soldier"
    );
    assert!(
        token_card_ids(&e, 0).is_empty(),
        "the caster does not create the token"
    );
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn q81_exile_rider_cards_exile_and_apply_their_riders() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(81_201_000 + index as u64);
        let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        cast_at(&mut e, &case, target);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: target is exiled",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        match case.rider {
            Rider::LoseThree => {
                assert_eq!(e.state.players[0].life, 17, "{}", case.id);
                assert!(token_card_ids(&e, 1).is_empty(), "{}", case.id);
            }
            Rider::Treasure => {
                assert_eq!(
                    token_card_ids(&e, 1),
                    vec!["treasure".to_string()],
                    "{}: the controller creates a Treasure",
                    case.id
                );
                assert_eq!(e.state.players[0].life, 20, "{}", case.id);
                assert_eq!(e.state.players[1].life, 20, "{}", case.id);
            }
            Rider::Soldier => {
                assert_eq!(
                    token_card_ids(&e, 1),
                    vec!["soldier_w_1_1".to_string()],
                    "{}: the controller creates a Soldier",
                    case.id
                );
                assert_eq!(e.state.players[0].life, 20, "{}", case.id);
                assert_eq!(e.state.players[1].life, 20, "{}", case.id);
            }
        }
    }
}

#[test]
fn q81_exile_rider_rejects_land_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(81_202_000 + index as u64);
        let forest = move_ready_to_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let result = e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(forest)));
        assert!(
            result.is_err(),
            "{}: a land is not a legal nonland-permanent target",
            case.id
        );
        assert_eq!(
            e.state.objects[&forest].zone,
            Zone::Battlefield,
            "{}: the land is untouched",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}: nothing was cast", case.id);
    }
}

#[test]
fn q81_exile_rider_stale_target_prevents_riders() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(81_203_000 + index as u64);
        let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        move_to_graveyard(&mut e, 1, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: the caster does not lose life on a stale target",
            case.id
        );
        assert!(
            token_card_ids(&e, 1).is_empty(),
            "{}: no token on a stale target",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
