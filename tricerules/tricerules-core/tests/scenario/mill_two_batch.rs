//! Actual-card scenarios for the queue-v26 activated mill-two cohort.
//!
//! Expectations are printed Oracle values. CR 701.17a-b mills exactly two when
//! available and as many as possible otherwise; CR 115.1/608.2b target a player;
//! CR 118.3 requires the tap cost; CR 121.1 keeps mill distinct from draw.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    activation: [u32; 6],
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "millstone",
            activation: [0, 0, 0, 0, 0, 2],
        },
        Case {
            id: "vedalken_entrancer",
            activation: [0, 1, 0, 0, 0, 0],
        },
        Case {
            id: "merfolk_mesmerist",
            activation: [0, 1, 0, 0, 0, 0],
        },
    ]
}

fn gift(mask: [u32; 6]) -> ManaGift {
    ManaGift {
        w: mask[0],
        u: mask[1],
        b: mask[2],
        r: mask[3],
        g: mask[4],
        c: mask[5],
    }
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn activate(
    e: &mut GameEngine,
    case: &Case,
    source: u32,
    target: Vec<tricerules_proto::ruled::v1::TargetRef>,
) {
    give_mana(e, 0, gift(case.activation));
    e.apply_scenario_command(0, &activate_ability_for(e, source, 0, target))
        .unwrap_or_else(|error| panic!("{}: activate: {error}", case.id));
}

#[test]
fn mill_two_batch_cards_mill_the_target_player_for_exactly_two() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(26_090_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let own_library = e.state.players[0].library.len();
        let target_library = e.state.players[1].library.len();
        let target_graveyard = e.state.players[1].graveyard.len();
        let top_two: Vec<u32> = e.state.players[1].library.iter().take(2).copied().collect();
        assert_eq!(top_two.len(), 2, "{}: a two-card library prefix", case.id);
        activate(&mut e, case, source, target_player(1));
        assert!(
            !e.state.stack.is_empty(),
            "{}: the activated ability is on the stack",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            target_library,
            "{}: activating does not mill before resolution",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.players[1].library.len(),
            target_library - 2,
            "{}: exactly two cards leave the target library",
            case.id
        );
        assert_eq!(
            e.state.players[1].graveyard.len(),
            target_graveyard + 2,
            "{}: exactly two cards reach the target graveyard",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            own_library,
            "{}: the controller's library is unchanged",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        for card in top_two {
            assert!(
                e.state.players[1].graveyard.contains(&card),
                "{}: each top card reached the target graveyard",
                case.id
            );
        }
    }
}

#[test]
fn mill_two_batch_cards_can_target_their_controller() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(26_091_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let own_library = e.state.players[0].library.len();
        let own_graveyard = e.state.players[0].graveyard.len();
        let opponent_library = e.state.players[1].library.len();
        activate(&mut e, case, source, target_player(0));
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.players[0].library.len(),
            own_library - 2,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[0].graveyard.len(),
            own_graveyard + 2,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: the opponent is untouched",
            case.id
        );
    }
}

#[test]
fn mill_two_batch_cards_reject_missing_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(26_092_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(case.activation));
        let opponent_library = e.state.players[1].library.len();
        let before = e.state.command_index;
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect_err("the target player is mandatory");
        assert_eq!(e.state.command_index, before, "{}", case.id);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn mill_two_batch_cards_mill_as_much_as_possible_without_causing_a_loss() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(26_093_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        while e.state.players[1].library.len() > 1 {
            let oid = e.state.players[1].library.pop_back().expect("library card");
            e.state.players[1].graveyard.push(oid);
            e.state.objects.get_mut(&oid).expect("object").zone = Zone::Graveyard;
        }
        let target_life = e.state.players[1].life;
        let target_graveyard = e.state.players[1].graveyard.len();
        activate(&mut e, case, source, target_player(1));
        resolve_entire_stack_two_player(&mut e);

        assert!(e.state.players[1].library.is_empty(), "{}", case.id);
        assert_eq!(
            e.state.players[1].graveyard.len(),
            target_graveyard + 1,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, target_life,
            "{}: mill is not a loss",
            case.id
        );
        assert!(
            !e.state.players[1].has_lost,
            "{}: milling does not lose",
            case.id
        );
    }
}

#[test]
fn mill_two_batch_cards_reject_a_second_activation_while_tapped() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(26_094_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target_library = e.state.players[1].library.len();
        activate(&mut e, case, source, target_player(1));
        resolve_entire_stack_two_player(&mut e);
        let before = e.state.command_index;
        give_mana(&mut e, 0, gift(case.activation));
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, target_player(1)))
            .expect_err("a tapped permanent cannot pay the tap cost again");
        assert_eq!(e.state.command_index, before, "{}", case.id);
        assert_eq!(
            e.state.players[1].library.len(),
            target_library - 2,
            "{}",
            case.id
        );
    }
}
