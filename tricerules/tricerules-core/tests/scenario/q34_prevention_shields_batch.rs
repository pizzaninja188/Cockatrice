//! Actual-card scenarios for the queue-v34 prevention-shield cohort.
//!
//! Expectations are printed Oracle values. CR 614.1a/615.1 create a finite
//! prevention shield; CR 611.2a/514.2 end it at cleanup. A shield absorbs only
//! damage dealt to the chosen target and only until its amount is consumed.
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
            id: "hold_at_bay",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            amount: 7,
        },
        Case {
            id: "mending_hands",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            amount: 4,
        },
        Case {
            id: "shieldmates_blessing",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            amount: 3,
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
        deck_with("mountain", &["shock", "shock", "shock"]),
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

fn shield(e: &mut GameEngine, case: &Case, target: Vec<tricerules_proto::ruled::v1::TargetRef>) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

fn shock(e: &mut GameEngine, target: Vec<tricerules_proto::ruled::v1::TargetRef>) {
    inject_card_into_hand(e, 0, "shock");
    give_mana(
        e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "shock");
    e.apply_scenario_command(0, &cast_spell(slot, target))
        .expect("cast Shock");
    resolve_entire_stack_two_player(e);
}

#[test]
fn q34_prevention_shields_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(34_020_001);
    shield(&mut e, case, target_player(0));
    shock(&mut e, target_player(0));
    assert_eq!(
        e.state.players[0].life, 20,
        "the 3-point shield absorbs the first 2 damage"
    );
    shock(&mut e, target_player(0));
    assert_eq!(
        e.state.players[0].life, 19,
        "the remaining 1 point absorbs only 1 of the second Shock"
    );
}

#[test]
fn q34_prevention_shields_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(34_020_002);
    shield(&mut e, case, target_player(0));
    shock(&mut e, target_player(0));
    shock(&mut e, target_player(0));
    assert_eq!(
        e.state.players[0].life, 20,
        "a 7-point shield absorbs both Shocks"
    );
}

#[test]
fn q34_prevention_shields_cards_absorb_up_to_their_printed_amount() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_021_000 + index as u64);
        shield(&mut e, case, target_player(0));
        shock(&mut e, target_player(0));
        shock(&mut e, target_player(0));
        let expected_damage = 4u32.saturating_sub(case.amount);
        assert_eq!(
            e.state.players[0].life,
            20 - expected_damage as i32,
            "{}: exactly the printed prevention amount",
            case.id
        );
    }
}

#[test]
fn q34_prevention_shields_cards_protect_only_the_chosen_target() {
    let case = &cases()[1];
    let mut e = engine(34_022_000);
    let protected = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    shield(&mut e, case, target_object(protected));
    shock(&mut e, target_object(protected));
    assert_eq!(
        e.state.objects[&protected].damage, 0,
        "the creature shield absorbs both damage"
    );
    assert_eq!(e.state.players[0].life, 20, "the player was never shielded");

    shock(&mut e, target_player(0));
    assert_eq!(
        e.state.players[0].life, 18,
        "the same shield does not protect the player"
    );
}
