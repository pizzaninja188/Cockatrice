//! Actual-card scenarios for the reviewed Flying / dies-draw cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b define Flying; CR 603.6c
//! stages the trigger when the source is put into a graveyard from the
//! battlefield (CR 700.4 defines "dies"). The draw belongs to the creature's
//! controller, and exiling the creature is not dying.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "darkslick_drake",
        },
        Case {
            id: "ithilien_kingfisher",
        },
        Case { id: "kingfisher" },
        Case {
            id: "messenger_drake",
        },
        Case {
            id: "palace_familiar",
        },
        Case { id: "runewing" },
    ]
}

fn engine(seed: u64, p0_specials: &[&str], p1_specials: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", p0_specials),
        deck_with("forest", p1_specials),
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

fn kill(e: &mut GameEngine, caster: usize, victim: u32) {
    ensure_in_hand(e, caster, "lightning_blast");
    give_mana(
        e,
        caster as i32,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, caster, "lightning_blast");
    e.apply_scenario_command(caster as i32, &cast_spell(slot, target_object(victim)))
        .expect("four damage is lethal");
}

#[test]
fn flying_dies_draw_batch_cards_have_flying_and_draw_when_they_die() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_170_000 + index as u64, &[case.id], &["lightning_blast"]);
        let victim = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            e.effective_has_keyword(victim, Keyword::Flying),
            "{}: printed Flying is active",
            case.id
        );
        let hand_before = e.state.players[0].hand.len();
        e.apply_scenario_command(0, &pass())
            .expect("pass priority to the opponent");
        kill(&mut e, 1, victim);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&victim].zone,
            Zone::Graveyard,
            "{}: four damage is lethal",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: the dies trigger draws one card",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn flying_dies_draw_batch_cards_draw_only_for_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_171_000 + index as u64, &["lightning_blast"], &[case.id]);
        let victim = move_ready_to_battlefield(&mut e, 1, case.id);
        let controller_hand = e.state.players[1].hand.len();
        ensure_in_hand(&mut e, 0, "lightning_blast");
        let caster_hand = e.state.players[0].hand.len();
        kill(&mut e, 0, victim);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&victim].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            controller_hand + 1,
            "{}: the creature's controller draws",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            caster_hand - 1,
            "{}: the caster does not draw",
            case.id
        );
    }
}

#[test]
fn flying_dies_draw_batch_cards_do_not_draw_when_exiled() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(
            9_172_000 + index as u64,
            &[case.id, "swords_to_plowshares"],
            &[],
        );
        let victim = move_ready_to_battlefield(&mut e, 0, case.id);
        ensure_in_hand(&mut e, 0, "swords_to_plowshares");
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "swords_to_plowshares");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(victim)))
            .expect("exile the creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&victim].zone,
            Zone::Exile,
            "{}: Swords exiles rather than destroys",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before - 1,
            "{}: no dies trigger draws a card",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}: the library is unchanged",
            case.id
        );
    }
}
