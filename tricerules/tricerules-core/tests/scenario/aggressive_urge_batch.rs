//! Actual-card scenarios for the reviewed Aggressive Urge pump-and-draw family.
//!
//! Expectations are printed Oracle values. CR 611.2a/514.2 end the pump at cleanup;
//! CR 121.1 draws one card after the pump; CR 608.2b fizzles the whole spell when the
//! targeted creature is illegal, so the draw does not happen. The saved Aggressive Urge
//! (2018-01-19) and Defiant Strike (2020-06-23) rulings assert that exact fizzle result.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: i32,
    toughness: i32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "aggressive_urge",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "defiant_strike",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
        },
        Case {
            id: "sudden_strength",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn cast_pump(e: &mut GameEngine, case: &Case, target: u32) {
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast the pump spell");
}

#[test]
fn aggressive_urge_batch_cards_pump_then_draw_one_and_expire() {
    for (index, case) in cases().into_iter().enumerate() {
        for (label, target_player) in [("own", 0usize), ("opponent", 1usize)] {
            let mut e = engine(880_000 + index as u64 * 10 + target_player as u64, case.id);
            let target = inject_creature_on_battlefield(&mut e, target_player, "grizzly_bears");
            inject_card_into_hand(&mut e, 0, case.id);
            let hand_before = e.state.players[0].hand.len();
            cast_pump(&mut e, &case, target);
            pass_both_players(&mut e);
            assert_eq!(
                e.effective_power(target),
                Some(2 + case.power as u32),
                "{} {label}",
                case.id
            );
            assert_eq!(
                e.effective_toughness(target),
                Some(2 + case.toughness as u32),
                "{} {label}",
                case.id
            );
            assert_eq!(
                e.state.players[0].hand.len(),
                hand_before,
                "{} {label}: cast loses one, draw gains one",
                case.id
            );
            end_active_turn(&mut e, 0);
            assert_eq!(
                e.effective_power(target),
                Some(2),
                "{} {label}: the pump expires",
                case.id
            );
            assert_eq!(
                e.effective_toughness(target),
                Some(2),
                "{} {label}: the pump expires",
                case.id
            );
        }
    }
}

#[test]
fn aggressive_urge_batch_cards_fizzle_without_drawing_when_the_target_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(881_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        cast_pump(&mut e, &case, creature);
        e.apply_scenario_command(0, &pass())
            .expect("pump controller passes");
        inject_card_into_hand(&mut e, 1, "unsummon");
        give_mana(
            &mut e,
            1,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let unsummon_slot = hand_index_for_card(&e, 1, "unsummon");
        e.apply_scenario_command(1, &cast_spell(unsummon_slot, target_object(creature)))
            .expect("opponent casts Unsummon in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&creature].zone, Zone::Hand, "{}", case.id);
        assert!(
            e.state.players[0].hand.contains(&creature),
            "{}: the creature returns to hand",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: the pump fizzles; the cast loss is offset by the returned creature and no draw",
            case.id
        );
        assert_eq!(
            count_card_id_in_graveyard(&e, 0, case.id),
            1,
            "{}: the fizzled pump is in its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn aggressive_urge_batch_cards_require_a_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(882_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        give_mana(&mut e, 0, gift(&case.mana));
        let pool_before = {
            let p = &e.state.players[0].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        };
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: the creature target is required",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        let p = &e.state.players[0].mana_pool;
        assert_eq!(
            (p.white, p.blue, p.black, p.red, p.green, p.colorless),
            pool_before,
            "{}: the rejected cast spends no mana",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
