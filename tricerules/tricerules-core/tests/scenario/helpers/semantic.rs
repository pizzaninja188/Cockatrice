//! Strict, bounded semantic evidence. Expectations belong to callers, not production emitters.
use super::*;
use tricerules_core::{TurnStep, Zone};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Evidence {
    Exercised,
    NotApplicable(&'static str),
    FixtureBlocked(String),
}

impl Evidence {
    pub fn require_exercised(self) {
        assert_eq!(self, Self::Exercised, "semantic evidence was not exercised");
    }
}

pub(crate) fn complete(
    engine: &mut GameEngine,
    bound: usize,
    mut answer: impl FnMut(&GameEngine) -> Option<(i32, RuledCommand)>,
) -> Evidence {
    for used in 0..=bound {
        if engine.state.stack.is_empty()
            && engine.state.blocking_choice().is_none()
            && engine.state.pending_spell_cast.is_none()
        {
            return Evidence::Exercised;
        }
        if used == bound {
            return Evidence::FixtureBlocked(format!("exhausted command bound {bound}"));
        }
        let (player, command) = if engine.state.blocking_choice().is_some()
            || engine.state.pending_spell_cast.is_some()
        {
            let Some(answer) = answer(engine) else {
                return Evidence::FixtureBlocked("unanswered pending choice".into());
            };
            answer
        } else {
            (engine.state.priority_player_id(), pass())
        };
        accepted(engine, player, &command);
    }
    unreachable!()
}

/// Casts, activations, passes and explicit choice answers all use the real command boundary.
/// Rejection is a test failure, never fixture-blocked or semantic coverage.
pub(crate) fn accepted(
    engine: &mut GameEngine,
    player: i32,
    command: &RuledCommand,
) -> RuledEventBatch {
    engine
        .apply_command(player, command)
        .unwrap_or_else(|error| panic!("expected accepted command {command:?}: {error:?}"))
}

pub(crate) fn main_phase(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    assert_main_priority(&engine, 0);
    engine
}

pub(crate) fn assert_main_priority(engine: &GameEngine, player: i32) {
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert_eq!(engine.state.active_player_id(), player);
    assert_eq!(engine.state.priority_player_id(), player);
}

pub(crate) fn generation(engine: &GameEngine, object: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object)
        .copied()
        .unwrap_or(0)
}

pub(crate) fn assert_object(
    engine: &GameEngine,
    object: u32,
    card: &str,
    owner: i32,
    controller: i32,
    zone: Zone,
    expected_generation: u64,
) {
    let actual = &engine.state.objects[&object];
    assert_eq!(actual.card_id, card);
    assert_eq!((actual.owner, actual.controller), (owner, controller));
    assert_eq!(actual.zone, zone);
    assert_eq!(actual.face_up_index, 0);
    assert_eq!(generation(engine, object), expected_generation);
}

#[derive(Clone, Copy)]
pub(crate) enum DrawSurface {
    Spell,
    Etb,
    Mode { index: u32, id: &'static str },
}

/// Independently reviewed values; never derive these from CardRegistry effects or a recipe.
pub(crate) struct DrawCase {
    pub card: &'static str,
    pub surface: DrawSurface,
    pub mana: ManaGift,
    pub recipient: usize,
    pub count: usize,
    pub food: usize,
}

pub(crate) fn divination() -> DrawCase {
    DrawCase {
        card: "divination",
        surface: DrawSurface::Spell,
        mana: ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        recipient: 0,
        count: 2,
        food: 0,
    }
}

pub(crate) fn visionary() -> DrawCase {
    DrawCase {
        card: "elvish_visionary",
        surface: DrawSurface::Etb,
        mana: ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        recipient: 0,
        count: 1,
        food: 0,
    }
}

pub(crate) fn pawpatch() -> DrawCase {
    DrawCase {
        card: "pawpatch_formation",
        surface: DrawSurface::Mode {
            index: 2,
            id: "mode_03",
        },
        mana: ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        recipient: 0,
        count: 1,
        food: 1,
    }
}

/// Pilot for untargeted controller draw. ETB and mode composition remain explicit contracts.
/// Returns exercised only after checking both completion and the caller's semantic expectations.
pub(crate) fn exercise_draw(case: DrawCase) -> Evidence {
    let mut engine = main_phase(448_001);
    let source = inject_card_into_hand(&mut engine, 0, case.card);
    assert_object(&engine, source, case.card, 0, 0, Zone::Hand, 0);
    give_mana(&mut engine, 0, case.mana);
    let slot = hand_index_for_card(&engine, 0, case.card);
    let hands: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|p| p.hand.clone())
        .collect();
    let libraries: Vec<Vec<_>> = engine
        .state
        .players
        .iter()
        .map(|p| p.library.iter().copied().collect())
        .collect();
    let generations: Vec<_> = libraries[case.recipient]
        .iter()
        .map(|id| generation(&engine, *id))
        .collect();
    let drawn_cards: Vec<_> = libraries[case.recipient]
        .iter()
        .map(|id| engine.state.objects[id].card_id.clone())
        .collect();
    assert!(
        case.count <= libraries[case.recipient].len(),
        "fixture library too small"
    );
    let command = match case.surface {
        DrawSurface::Mode { index, .. } => cast_modal_spell(slot, vec![(index, vec![])]),
        _ => cast_spell(slot, vec![]),
    };
    accepted(&mut engine, 0, &command);
    assert_object(&engine, source, case.card, 0, 0, Zone::Stack, 1);
    let stack = engine.state.stack.last().expect("cast reached stack");
    assert_eq!(stack.card_id, case.card);
    assert_eq!(stack.controller, 0);
    assert_eq!(stack.face_index, 0);
    match case.surface {
        DrawSurface::Mode { id, .. } => {
            assert_eq!(stack.chosen_modes.len(), 1);
            assert_eq!(
                stack.chosen_modes[0].mode_id.as_str(),
                id,
                "selected mode identity"
            );
        }
        _ => assert!(stack.chosen_modes.is_empty()),
    }
    if matches!(case.surface, DrawSurface::Etb) {
        // Cast resolution creates the real ETB; injecting a permanent would not prove this.
        for _ in 0..engine.state.players.len() {
            let player = engine.state.priority_player_id();
            accepted(&mut engine, player, &pass());
        }
        assert_object(&engine, source, case.card, 0, 0, Zone::Battlefield, 2);
        assert_eq!(
            engine.state.players[0].hand.len(),
            hands[0].len() - 1,
            "ETB has not drawn yet"
        );
        assert_eq!(engine.state.stack.len(), 1);
        assert!(engine.state.stack[0].is_triggered);
        assert_eq!(engine.state.stack[0].card_id, case.card);
    }
    let evidence = complete(&mut engine, 16, |_| None);
    if evidence != Evidence::Exercised {
        return evidence;
    }
    assert_main_priority(&engine, 0);
    let destination = if matches!(case.surface, DrawSurface::Etb) {
        Zone::Battlefield
    } else {
        Zone::Graveyard
    };
    assert_object(&engine, source, case.card, 0, 0, destination, 2);
    for (seat, player) in engine.state.players.iter().enumerate() {
        let count = if seat == case.recipient {
            case.count
        } else {
            0
        };
        let mut hand = hands[seat].clone();
        hand.retain(|id| *id != source);
        hand.extend_from_slice(&libraries[seat][..count]);
        assert_eq!(
            player.hand, hand,
            "{} recipient {seat} exact hand",
            case.card
        );
        assert_eq!(
            player.library.iter().copied().collect::<Vec<_>>(),
            libraries[seat][count..],
            "exact library suffix"
        );
        assert_eq!(player.life, 20);
        assert_eq!(
            battlefield_token_oids(&engine, seat, "food").len(),
            if seat == 0 { case.food } else { 0 }
        );
    }
    for (index, object) in libraries[case.recipient][..case.count].iter().enumerate() {
        let player = engine.state.players[case.recipient].id;
        assert_object(
            &engine,
            *object,
            &drawn_cards[index],
            player,
            player,
            Zone::Hand,
            generations[index] + 1,
        );
    }
    Evidence::Exercised
}
