//! Actual-card scenarios for the reviewed team-pump-on-enter cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a makes each entering creature
//! trigger the ability; CR 603.6d and the printed word "another" exclude the source's
//! own entry; CR 613.4c applies the source-only pump and CR 514.2 ends it.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    pump: (u32, u32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "bronzebeak_moa",
            mana: ManaGift {
                g: 1,
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            pump: (3, 3),
        },
        Case {
            id: "herd_gnarr",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
            pump: (2, 2),
        },
        Case {
            id: "llanowar_stalker",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            body: (1, 1),
            pump: (1, 0),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "grizzly_bears", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let _slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn team_pump_on_enter_batch_cards_pump_only_their_own_source_when_another_creature_enters() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_700_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        let bystander = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0 + case.pump.0),
            "{}: another creature pumps the source",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1 + case.pump.1),
            "{}: another creature pumps the source",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(3), Some(3)),
            "{}: source only",
            case.id
        );
    }
}

#[test]
fn team_pump_on_enter_batch_cards_exclude_their_own_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_701_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: the source's own entry does not trigger",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}

#[test]
fn team_pump_on_enter_batch_cards_ignore_opponents_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_702_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: an opponent's creature does not trigger",
            case.id
        );
    }
}

#[test]
fn team_pump_on_enter_batch_cards_stack_one_trigger_per_creature_and_expire() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_703_000 + index as u64, case.id);
        let source = cast_source(&mut e, &case);
        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        resolve_entire_stack_two_player(&mut e);
        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0 + 2 * case.pump.0),
            "{}: one trigger per entering creature",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1 + 2 * case.pump.1),
            "{}: one trigger per entering creature",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}
