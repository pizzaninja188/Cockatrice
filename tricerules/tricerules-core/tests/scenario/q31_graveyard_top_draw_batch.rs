//! Actual-card scenarios for the queue-v31 graveyard-to-top draw cohort.
//!
//! Expectations are printed Oracle values. CR 115.1/601.2c choose any number of
//! graveyard-card targets; CR 608.2b skips a stale target; CR 401.4 puts the
//! chosen cards on top in the chosen order; CR 121 draws one card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::TargetRef;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "footbottom_feast",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "forever_young",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "gravepurge",
            mana: ManaGift {
                b: 1,
                c: 2,
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &["footbottom_feast", "forever_young", "gravepurge"],
        ),
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

fn target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        ..Default::default()
    }
}

fn cast(e: &mut GameEngine, case: &Case, targets: Vec<TargetRef>) -> usize {
    ensure_card_in_hand(e, 0, case.id);
    let hand_before = e.state.players[0].hand.len();
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    hand_before
}

#[test]
fn q31_graveyard_top_draw_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(31_090_001);
    let a = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let b = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let hand_before = cast(&mut e, case, vec![target(a), target(b)]);
    resolve_entire_stack_two_player(&mut e);
    assert_ne!(e.state.objects[&a].zone, Zone::Graveyard);
    assert_ne!(e.state.objects[&b].zone, Zone::Graveyard);
    let zones = [e.state.objects[&a].zone, e.state.objects[&b].zone];
    assert!(zones.contains(&Zone::Library), "one card remains on top");
    assert!(zones.contains(&Zone::Hand), "the drawn card is in hand");
    assert_eq!(e.state.players[0].hand.len(), hand_before);
}

#[test]
fn q31_graveyard_top_draw_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(31_090_002);
    let a = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    cast(&mut e, case, vec![target(a)]);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&a].zone, Zone::Hand);
}

#[test]
fn q31_graveyard_top_draw_cards_choose_any_number_of_creature_cards() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_091_000 + index as u64);
        let a = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        let b = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        let other = inject_graveyard_card(&mut e, 0, "lightning_bolt");
        let hand_before = cast(&mut e, case, vec![target(a), target(b)]);
        resolve_entire_stack_two_player(&mut e);
        assert_ne!(e.state.objects[&a].zone, Zone::Graveyard, "{}", case.id);
        assert_ne!(e.state.objects[&b].zone, Zone::Graveyard, "{}", case.id);
        let zones = [e.state.objects[&a].zone, e.state.objects[&b].zone];
        assert!(
            zones.contains(&Zone::Library),
            "{}: one card remains on top",
            case.id
        );
        assert!(
            zones.contains(&Zone::Hand),
            "{}: the drawn card is in hand",
            case.id
        );
        assert_eq!(
            e.state.objects[&other].zone,
            Zone::Graveyard,
            "{}: noncreature card untouched",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
    }
}

#[test]
fn q31_graveyard_top_draw_cards_may_choose_zero() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_092_000 + index as u64);
        let hand_before = cast(&mut e, case, vec![]);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: zero targets still draws",
            case.id
        );
    }
}

#[test]
fn q31_graveyard_top_draw_cards_reject_an_opponents_graveyard() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_093_000 + index as u64);
        let theirs = inject_graveyard_card(&mut e, 1, "grizzly_bears");
        ensure_card_in_hand(&mut e, 0, case.id);
        assert!(
            e.apply_scenario_command(
                0,
                &cast_spell(hand_index_for_card(&e, 0, case.id), vec![target(theirs)],)
            )
            .is_err(),
            "{}: an opponent's graveyard card is not a legal target",
            case.id
        );
    }
}

#[test]
fn q31_graveyard_top_draw_cards_skip_a_stale_target_and_still_draw() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(31_094_000 + index as u64);
        let stale = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        let live = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        let hand_before = cast(&mut e, case, vec![target(stale), target(live)]);
        // One of two targets leaves the graveyard before the spell resolves; the other
        // remains legal, so the spell still resolves and draws.
        e.state.players[0].graveyard.retain(|oid| *oid != stale);
        e.state.objects.get_mut(&stale).expect("target").zone = Zone::Exile;
        e.state.players[0].exile.push(stale);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&stale].zone, Zone::Exile, "{}", case.id);
        assert_ne!(e.state.objects[&live].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: still draws",
            case.id
        );
    }
}
