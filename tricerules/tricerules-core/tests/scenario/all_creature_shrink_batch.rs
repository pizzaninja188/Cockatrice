//! Actual-card scenarios for the queue-v21 all-creature shrink cohort.
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

fn cases() -> [Case; 6] {
    [
        Case {
            id: "hell_swarm",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            shrink: (-1, 0),
        },
        Case {
            id: "infest",
            mana: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            shrink: (-2, -2),
        },
        Case {
            id: "languish",
            mana: ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
            shrink: (-4, -4),
        },
        Case {
            id: "marsh_gas",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            shrink: (-2, 0),
        },
        Case {
            id: "nausea",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            shrink: (-1, -1),
        },
        Case {
            id: "shrivel",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            shrink: (-1, -1),
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
fn all_creature_shrink_batch_canary_cards_shrink_every_creature() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "hell_swarm" | "languish"))
    {
        let mut e = engine(9_979_000);
        let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        cast_spell_at_nothing(&mut e, &case);
        for creature in [mine, theirs] {
            assert_eq!(
                (e.effective_power(creature), e.effective_toughness(creature)),
                (
                    Some((5 + case.shrink.0) as u32),
                    Some((5 + case.shrink.1) as u32)
                ),
                "{}: every creature is affected",
                case.id
            );
        }
    }
}

#[test]
fn all_creature_shrink_batch_cards_shrink_every_creature_by_the_printed_amount() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_980_000 + index as u64);
        let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        let spell = cast_spell_at_nothing(&mut e, &case);
        assert_eq!(
            (e.effective_power(mine), e.effective_toughness(mine)),
            (
                Some((5 + case.shrink.0) as u32),
                Some((5 + case.shrink.1) as u32)
            ),
            "{}: the caster's creature is affected too",
            case.id
        );
        assert_eq!(
            (e.effective_power(theirs), e.effective_toughness(theirs)),
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
fn all_creature_shrink_batch_cards_kill_every_creature_whose_toughness_reaches_zero() {
    for (index, case) in cases().into_iter().enumerate() {
        if case.shrink.1 >= 0 {
            continue;
        }
        let mut e = engine(9_981_000 + index as u64);
        let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 1, 1);
        let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 1, 1);
        cast_spell_at_nothing(&mut e, &case);
        assert_eq!(
            e.state.objects[&mine].zone,
            Zone::Graveyard,
            "{}: the caster's 1/1 dies to the printed toughness reduction",
            case.id
        );
        assert_eq!(
            e.state.objects[&theirs].zone,
            Zone::Graveyard,
            "{}: an opposing 1/1 dies to the printed toughness reduction",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&mine),
            "{}: the caster's dead creature is in its owner's graveyard",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&theirs),
            "{}: the opposing dead creature is in its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn all_creature_shrink_batch_cards_do_not_affect_creatures_that_enter_after_resolution() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_982_000 + index as u64);
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
fn all_creature_shrink_batch_cards_keep_affecting_a_creature_that_changes_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_983_000 + index as u64);
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
fn all_creature_shrink_batch_cards_expire_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_984_000 + index as u64);
        let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 5, 5);
        let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        cast_spell_at_nothing(&mut e, &case);
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(mine), e.effective_toughness(mine)),
            (Some(5), Some(5)),
            "{}: CR 514.2 restores the caster's printed body",
            case.id
        );
        assert_eq!(
            (e.effective_power(theirs), e.effective_toughness(theirs)),
            (Some(5), Some(5)),
            "{}: CR 514.2 restores the opponent's printed body",
            case.id
        );
    }
}

#[test]
fn all_creature_shrink_batch_cards_reject_a_cast_the_mana_cannot_pay() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_985_000 + index as u64);
        let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
                .is_err(),
            "{}: the printed cost is required",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: no mana is spent",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Hand,
            "{}: the spell stays in hand",
            case.id
        );
        assert_eq!(
            (e.effective_power(theirs), e.effective_toughness(theirs)),
            (Some(5), Some(5)),
            "{}: no creature is modified",
            case.id
        );
    }
}

#[test]
fn all_creature_shrink_batch_cards_leave_hidden_zones_and_life_unchanged() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_986_000 + index as u64);
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
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}
