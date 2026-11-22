//! Actual-card scenarios for the queue-v18 opponent-wide shrink cohort.
//!
//! Expectations are printed Oracle values. CR 611.2c fixes the affected set when the
//! effect begins; CR 613.4c applies the modification; CR 514.2 ends it at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    shrink: (i32, i32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "cower_in_fear",
            mana: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            shrink: (-1, -1),
        },
        Case {
            id: "hysterical_blindness",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            shrink: (-4, 0),
        },
        Case {
            id: "turn_the_tide",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            shrink: (-2, 0),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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

fn cast_spell_at_nothing(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let spell = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    resolve_entire_stack_two_player(e);
    spell
}

#[test]
fn mass_shrink_batch_cards_shrink_only_opponents_creatures_by_the_printed_amount() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_860_000 + index as u64);
        let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        let bystander = inject_creature_with_stats(&mut e, 1, "hill_giant", 5, 5);
        let spell = cast_spell_at_nothing(&mut e, &case);
        assert_eq!(
            (e.effective_power(mine), e.effective_toughness(mine)),
            (Some(2), Some(2)),
            "{}: your creature is unchanged",
            case.id
        );
        assert_eq!(
            (e.effective_power(theirs), e.effective_toughness(theirs)),
            (
                Some((5 + case.shrink.0) as u32),
                Some((5 + case.shrink.1) as u32)
            ),
            "{}: exact printed shrink on each opponent creature",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (
                Some((5 + case.shrink.0) as u32),
                Some((5 + case.shrink.1) as u32)
            ),
            "{}: every opponent creature is affected",
            case.id
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn mass_shrink_batch_cards_kill_only_opposing_creatures_whose_toughness_reaches_zero() {
    for (index, case) in cases().into_iter().enumerate() {
        if case.shrink.1 >= 0 {
            continue;
        }
        let mut e = engine(9_861_000 + index as u64);
        let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 1, 1);
        let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 1, 1);
        cast_spell_at_nothing(&mut e, &case);
        assert_eq!(
            e.state.objects[&mine].zone,
            Zone::Battlefield,
            "{}: your 1/1 survives",
            case.id
        );
        assert_eq!(
            e.state.objects[&theirs].zone,
            Zone::Graveyard,
            "{}: an opposing 1/1 dies to the printed toughness reduction",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&theirs),
            "{}: the dead creature is in its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn mass_shrink_batch_cards_do_not_affect_creatures_that_enter_after_resolution() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_862_000 + index as u64);
        cast_spell_at_nothing(&mut e, &case);
        let late = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        assert_eq!(
            (e.effective_power(late), e.effective_toughness(late)),
            (Some(2), Some(2)),
            "{}: a creature entering after resolution is unaffected",
            case.id
        );
    }
}

#[test]
fn mass_shrink_batch_cards_keep_affecting_a_creature_that_changes_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_863_000 + index as u64);
        let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        cast_spell_at_nothing(&mut e, &case);
        e.state.players[1]
            .battlefield
            .retain(|object| *object != creature);
        e.state.players[0].battlefield.push(creature);
        let changed = e
            .state
            .objects
            .get_mut(&creature)
            .expect("affected creature");
        changed.base_controller = 0;
        changed.controller = 0;
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some((5 + case.shrink.0) as u32),
                Some((5 + case.shrink.1) as u32)
            ),
            "{}: the affected object keeps the modification after a control change",
            case.id
        );
    }
}

#[test]
fn mass_shrink_batch_cards_do_not_affect_creatures_that_come_under_opponent_control_later() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_863_500 + index as u64);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        cast_spell_at_nothing(&mut e, &case);
        e.state.players[0]
            .battlefield
            .retain(|object| *object != creature);
        e.state.players[1].battlefield.push(creature);
        let changed = e
            .state
            .objects
            .get_mut(&creature)
            .expect("unaffected creature");
        changed.base_controller = 1;
        changed.controller = 1;
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(5), Some(5)),
            "{}: a creature controlled by the caster at resolution is not added when it later changes control",
            case.id
        );
    }
}

#[test]
fn mass_shrink_batch_cards_expire_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_864_000 + index as u64);
        let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        cast_spell_at_nothing(&mut e, &case);
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(theirs), e.effective_toughness(theirs)),
            (Some(5), Some(5)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}

#[test]
fn mass_shrink_batch_cards_leave_hidden_zones_and_controllers_life_unchanged() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_865_000 + index as u64);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        cast_spell_at_nothing(&mut e, &case);
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: the opponent's hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: the opponent's library is unchanged",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}
