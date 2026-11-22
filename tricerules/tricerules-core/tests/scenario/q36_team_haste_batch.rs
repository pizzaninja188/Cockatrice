//! Actual-card scenarios for the queue-v36 team-haste cohort.
//!
//! Expectations are printed Oracle values. CR 604.1/611.3a keep the static ability
//! active while the source is on the battlefield; CR 613.1f grants haste in layer 6,
//! and CR 109.5 makes "you" the source's controller.
use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    body: bool,
    wall: bool,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "cyclops_of_eternal_fury",
            body: true,
            wall: false,
        },
        Case {
            id: "fervor",
            body: false,
            wall: false,
        },
        Case {
            id: "madrush_cyclops",
            body: true,
            wall: false,
        },
        Case {
            id: "tuktuk_rubblefort",
            body: true,
            wall: true,
        },
    ]
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = tricerules_core::Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
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

#[test]
fn q36_team_haste_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(36_050_001, case.id);
    let _source = move_ready_to_battlefield(&mut e, 0, case.id);
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let other = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
    assert!(
        e.effective_has_keyword(own, Keyword::Haste),
        "Fervor grants haste to a creature you control"
    );
    assert!(
        !e.effective_has_keyword(other, Keyword::Haste),
        "Fervor does not grant haste to an opponent's creature"
    );
    assert_eq!(
        (e.effective_power(own), e.effective_toughness(own)),
        (Some(2), Some(2)),
        "Fervor does not modify power or toughness"
    );
}

#[test]
fn q36_team_haste_canary_hardest() {
    let case = &cases()[3];
    let mut e = engine(36_050_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let other = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
    for creature in [source, own] {
        assert!(
            e.effective_has_keyword(creature, Keyword::Haste),
            "Tuktuk Rubblefort grants haste to each creature you control"
        );
    }
    assert!(
        e.effective_has_keyword(source, Keyword::Defender),
        "Tuktuk Rubblefort keeps defender"
    );
    assert!(
        e.effective_has_keyword(source, Keyword::Reach),
        "Tuktuk Rubblefort keeps reach"
    );
    assert!(
        !e.effective_has_keyword(other, Keyword::Haste),
        "Tuktuk Rubblefort does not grant haste to an opponent's creature"
    );
}

#[test]
fn q36_team_haste_cards_grant_haste_to_each_creature_you_control() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_051_000 + index as u64, case.id);
        let source = if case.body {
            Some(move_ready_to_battlefield(&mut e, 0, case.id))
        } else {
            move_ready_to_battlefield(&mut e, 0, case.id);
            None
        };
        let own_a = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let own_b = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        let opponent = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        for creature in [own_a, own_b] {
            assert!(
                e.effective_has_keyword(creature, Keyword::Haste),
                "{}: controlled creature has haste",
                case.id
            );
        }
        if let Some(source) = source {
            assert!(
                e.effective_has_keyword(source, Keyword::Haste),
                "{}: the source includes itself",
                case.id
            );
        }
        assert!(
            !e.effective_has_keyword(opponent, Keyword::Haste),
            "{}: an opponent's creature does not have haste",
            case.id
        );
        if case.wall {
            assert!(
                e.effective_has_keyword(source.expect("body"), Keyword::Defender),
                "{}: defender is retained",
                case.id
            );
            assert!(
                e.effective_has_keyword(source.expect("body"), Keyword::Reach),
                "{}: reach is retained",
                case.id
            );
        }
    }
}

#[test]
fn q36_team_haste_cards_stop_granting_when_the_source_leaves() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_052_000 + index as u64, case.id);
        let source = if case.body {
            Some(move_ready_to_battlefield(&mut e, 0, case.id))
        } else {
            move_ready_to_battlefield(&mut e, 0, case.id);
            None
        };
        let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        assert!(
            e.effective_has_keyword(own, Keyword::Haste),
            "{}: haste is active",
            case.id
        );
        let source = source.unwrap_or_else(|| battlefield_object_for_card(&e, 0, case.id));
        move_to_graveyard(&mut e, 0, source);
        assert!(
            !e.effective_has_keyword(own, Keyword::Haste),
            "{}: haste ends when the source leaves",
            case.id
        );
    }
}
