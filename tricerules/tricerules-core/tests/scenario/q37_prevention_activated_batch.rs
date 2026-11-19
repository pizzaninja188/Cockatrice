//! Actual-card scenarios for the queue-v37 activated prevention cohort.
//!
//! Expectations are printed Oracle values. CR 614.1a/615.1 create a finite prevention
//! shield; CR 602.1/118.3 pay the printed mana and tap costs; CR 611.2a/514.2 end the
//! shield at cleanup. A shield absorbs only damage dealt to the chosen target.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "amulet_of_kroog",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            amount: 1,
        },
        Case {
            id: "split-tail_miko",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            amount: 2,
        },
        Case {
            id: "treefolk_healer",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            amount: 2,
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt", "lightning_bolt"]),
        deck_with("island", &[]),
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

fn activate_shield(e: &mut GameEngine, case: &Case, source: u32, player: i32) {
    give_mana(e, 0, gift(&case.mana));
    apply_ability(e, 0, source, 0, target_player(player)).expect("activate prevention");
    resolve_entire_stack_two_player(e);
}

fn bolt(e: &mut GameEngine, player: usize, target: i32) {
    inject_card_into_hand(e, player, "lightning_bolt");
    give_mana(
        e,
        player as i32,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, player, "lightning_bolt");
    e.apply_scenario_command(player as i32, &cast_spell(slot, target_player(target)))
        .expect("cast Lightning Bolt");
    resolve_entire_stack_two_player(e);
}

#[test]
fn q37_prevention_activated_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(37_030_001);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    activate_shield(&mut e, case, source, 0);
    assert!(e.state.objects[&source].tapped, "the tap cost is paid");
    bolt(&mut e, 0, 0);
    assert_eq!(
        e.state.players[0].life, 19,
        "the 2-point shield absorbs two of the Bolt's three"
    );
    bolt(&mut e, 0, 0);
    assert_eq!(e.state.players[0].life, 16, "the shield is spent");
}

#[test]
fn q37_prevention_activated_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(37_030_002);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    activate_shield(&mut e, case, source, 0);
    bolt(&mut e, 0, 0);
    bolt(&mut e, 0, 0);
    assert_eq!(
        e.state.players[0].life, 16,
        "one 2-point shield absorbs two of six damage"
    );
}

#[test]
fn q37_prevention_activated_cards_absorb_their_printed_amount() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_031_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate_shield(&mut e, case, source, 0);
        bolt(&mut e, 0, 0);
        assert_eq!(
            e.state.players[0].life,
            20 - (3 - case.amount) as i32,
            "{}: exactly the printed prevention amount",
            case.id
        );
    }
}

#[test]
fn q37_prevention_activated_cards_protect_only_the_chosen_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_032_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate_shield(&mut e, case, source, 0);
        bolt(&mut e, 0, 1);
        assert_eq!(
            e.state.players[1].life, 17,
            "{}: only the chosen player is shielded",
            case.id
        );
    }
}

#[test]
fn q37_prevention_activated_cards_require_a_target_and_the_tap_cost() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_033_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, 0, vec![]).expect_err("prevention requires a target");
        assert!(!e.state.objects[&source].tapped, "{}", case.id);

        e.state.objects.get_mut(&source).expect("source").tapped = true;
        apply_ability(&mut e, 0, source, 0, target_player(0))
            .expect_err("a tapped source cannot pay {T}");
    }
}

#[test]
fn q37_prevention_activated_cards_expire_at_cleanup() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_034_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate_shield(&mut e, case, source, 0);
        end_active_turn(&mut e, 0);
        bolt(&mut e, 1, 0);
        assert_eq!(
            e.state.players[0].life, 17,
            "{}: the prevention shield ended with the turn",
            case.id
        );
    }
}
