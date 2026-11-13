//! Actual-card scenarios for the reviewed "This creature enters tapped." family.
//!
//! Expectations are printed Oracle values. CR 614.1d and 614.12 make the
//! clause a replacement effect that modifies how the permanent enters; the
//! permanent does not use the stack and untaps normally in its controller's
//! next untap step (CR 502.3).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 8] {
    [
        Case {
            id: "shambling_ghoul",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 3),
        },
        Case {
            id: "wolf_cove_villager",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "rotting_legion",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            body: (4, 5),
        },
        Case {
            id: "scarwood_treefolk",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            body: (3, 5),
        },
        Case {
            id: "unhallowed_phalanx",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            body: (1, 13),
        },
        Case {
            id: "forgotten_sentinel",
            mana: ManaGift {
                c: 4,
                ..Default::default()
            },
            body: (4, 3),
        },
        Case {
            id: "crooked_custodian",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            body: (3, 2),
        },
        Case {
            id: "rusted_sentinel",
            mana: ManaGift {
                c: 4,
                ..Default::default()
            },
            body: (3, 4),
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

fn cast_case(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the creature");
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn enters_tapped_creatures_batch_cards_enter_tapped_from_hand() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(870_000 + index as u64, case.id);
        let creature = cast_case(&mut e, &case);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(
            e.state.objects[&creature].tapped,
            "{}: enters tapped",
            case.id
        );
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.body.1),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn enters_tapped_creatures_batch_cards_untap_on_their_controllers_next_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(871_000 + index as u64, case.id);
        let creature = cast_case(&mut e, &case);
        assert!(e.state.objects[&creature].tapped, "{}", case.id);
        end_active_turn(&mut e, 0);
        for _ in 0..12 {
            if e.state.active_player_id() == 0 {
                break;
            }
            e.apply_scenario_command(1, &primitive_yield())
                .expect("opponent yields");
            resolve_cleanup_discards_if_any(&mut e);
        }
        assert_eq!(e.state.active_player_id(), 0, "{}", case.id);
        assert!(
            !e.state.objects[&creature].tapped,
            "{}: untaps in its controller's next untap step",
            case.id
        );
    }
}
