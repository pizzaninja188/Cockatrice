//! Actual-card scenarios for the reviewed `{T}: Add {C}{C}` family.
//!
//! Expectations are printed Oracle values. CR 605.1a and 605.3b make the
//! targetless mana ability resolve immediately without using the stack. A
//! creature source still obeys the CR 302.6 summoning-sickness rule for its
//! `{T}` cost; Ur-Golem's Eye is an artifact and does not.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    creature: bool,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "sunastian_falconer",
            mana: ManaGift {
                r: 1,
                g: 1,
                c: 3,
                ..Default::default()
            },
            creature: true,
        },
        Case {
            id: "weaver_of_currents",
            mana: ManaGift {
                g: 1,
                u: 1,
                c: 1,
                ..Default::default()
            },
            creature: true,
        },
        Case {
            id: "ur-golems_eye",
            mana: ManaGift {
                c: 4,
                ..Default::default()
            },
            creature: false,
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn colorless_double_mana_batch_cards_activate_for_two_colorless_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(910_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let before = pool(&e);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect("activate the colorless mana ability");
        assert_eq!(pool(&e).5, before.5 + 2, "{}: two colorless", case.id);
        assert_eq!(pool(&e).0, before.0, "{}: no colored mana", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}: mana ability", case.id);
    }
}

#[test]
fn colorless_double_mana_batch_cards_apply_summoning_sickness() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(911_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the mana source");
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        let before = pool(&e);
        if case.creature {
            assert!(
                e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                    .is_err(),
                "{}: a summoning-sick creature cannot pay {{T}}",
                case.id
            );
            assert_eq!(
                pool(&e),
                before,
                "{}: rejected activation adds no mana",
                case.id
            );
            assert!(!e.state.objects[&source].tapped, "{}", case.id);
        } else {
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .expect("artifact mana ability");
            assert_eq!(pool(&e).5, before.5 + 2, "{}", case.id);
        }
    }
}
