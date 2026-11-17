//! Actual-card scenarios for the queue-v28 "target player draws two cards and loses
//! 2 life" cohort.
//!
//! Expectations are printed Oracle values. CR 115.1 chooses one player target;
//! CR 121.1 makes that player draw two; CR 119.3 makes the same player lose 2 life.
//! The spell itself leaves its controller's hand before the draw, so self-targeting is a
//! net +1 hand card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "painful_lesson",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "sign_in_blood",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
        },
        Case {
            id: "blood_pact",
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[card, "grizzly_bears"]),
        deck_with("swamp", &["grizzly_bears"]),
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

fn cast_draw(e: &mut GameEngine, case: &Case, player: i32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_player(player)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn draw_two_lose_two_batch_cards_apply_to_the_targeted_opponent() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_060_000 + index as u64, case.id);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_life = e.state.players[1].life;
        let caster_life = e.state.players[0].life;
        cast_draw(&mut e, &case, 1);
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand + 2,
            "{}: the targeted opponent draws two",
            case.id
        );
        assert_eq!(
            e.state.players[1].life,
            opponent_life - 2,
            "{}: the targeted opponent loses 2 life",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, caster_life,
            "{}: the caster's life is unchanged",
            case.id
        );
        assert_eq!(
            count_card_id_in_graveyard(&e, 0, case.id),
            1,
            "{}: the spell is in its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn draw_two_lose_two_batch_cards_can_target_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_061_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let life_before = e.state.players[0].life;
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_player(0)))
            .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: -1 for the cast and +2 for the draw",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            life_before - 2,
            "{}: the caster loses 2 life",
            case.id
        );
    }
}
