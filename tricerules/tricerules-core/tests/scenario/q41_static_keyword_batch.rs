//! Actual-card scenarios for the queue-v41 threshold and cascade cohorts.
//!
//! Threshold expectations are printed Oracle values; CR 611.3/613.4c make the bonus live.
//! Cascade is CR 702.85: cast, exile through the first cheaper nonland card, optionally cast it
//! without paying its cost, and randomize the exiled remainder to the bottom.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use std::collections::HashSet;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision, RuledCommand};

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card]),
        deck_with("mountain", &[]),
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

fn move_library_to_graveyard(e: &mut GameEngine, player: usize) -> u32 {
    let oid = e.state.players[player]
        .library
        .pop_back()
        .expect("a library card to move");
    e.state.players[player].graveyard.push(oid);
    e.state.objects.get_mut(&oid).expect("object").zone = Zone::Graveyard;
    oid
}

fn set_library_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect::<Vec<_>>();
    e.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        e.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

#[test]
fn q41_threshold_pump_canary_simplest_turns_on_at_seven_cards() {
    let mut e = engine(41_030_001, "krosan_beast");
    let source = move_ready_to_battlefield(&mut e, 0, "krosan_beast");
    for _ in 0..6 {
        move_library_to_graveyard(&mut e, 0);
    }
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(1), Some(1)),
        "threshold is off at six cards"
    );
    move_library_to_graveyard(&mut e, 0);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(8), Some(8)),
        "the printed +7/+7 turns on at seven"
    );
}

#[test]
fn q41_threshold_pump_canary_hardest_uses_only_the_controllers_graveyard() {
    let mut e = engine(41_030_002, "metamorphic_wurm");
    let source = move_ready_to_battlefield(&mut e, 0, "metamorphic_wurm");
    for _ in 0..6 {
        move_library_to_graveyard(&mut e, 0);
    }
    for _ in 0..10 {
        move_library_to_graveyard(&mut e, 1);
    }
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(3), Some(3)),
        "opponent graveyards do not count"
    );
    move_library_to_graveyard(&mut e, 0);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(7), Some(7)),
        "the printed +4/+4 applies"
    );
}

#[test]
fn q41_threshold_pump_cards_apply_their_printed_bonus() {
    for (index, (card, power, toughness, dp, dt)) in [
        ("krosan_beast", 1, 1, 7, 7),
        ("anurid_barkripper", 2, 2, 2, 2),
        ("springing_tiger", 3, 3, 2, 2),
        ("metamorphic_wurm", 3, 3, 4, 4),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(41_031_000 + index as u64, card);
        let source = move_ready_to_battlefield(&mut e, 0, card);
        for _ in 0..7 {
            move_library_to_graveyard(&mut e, 0);
        }
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(power + dp), Some(toughness + dt)),
            "{card}: printed threshold bonus"
        );
        e.state.players[0].graveyard.pop();
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(power), Some(toughness)),
            "{card}: the bonus turns off below seven"
        );
    }
}

fn cast_creature(e: &mut GameEngine, card: &str, mana: ManaGift) {
    inject_card_into_hand(e, 0, card);
    give_mana(e, 0, mana);
    let hand = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(hand, vec![]))
        .expect("cast the cascade creature");
}

#[test]
fn q41_cascade_canary_simplest_exiles_through_the_first_cheaper_card() {
    let mut e = engine(41_090_001, "shardless_agent");
    let cohort = set_library_top(&mut e, 0, &["forest", "grizzly_bears", "opt"]);
    let library_before = e.state.players[0].library.len();
    cast_creature(
        &mut e,
        "shardless_agent",
        ManaGift {
            g: 1,
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    assert!(
        e.state
            .stack
            .iter()
            .any(|item| item.ability_text.as_deref() == Some("Cascade")),
        "the cast trigger is separate from the creature spell"
    );
    pass_both_players(&mut e);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("Cascade offers the found card");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
    assert_eq!(pending.presentation.candidates, [cohort[1]]);
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline the free cast");
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.players[0].library.len(), library_before);
    let exiled = [cohort[0], cohort[1]];
    let bottom: HashSet<u32> = e.state.players[0]
        .library
        .iter()
        .rev()
        .take(exiled.len())
        .copied()
        .collect();
    assert_eq!(bottom, exiled.iter().copied().collect());
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(cohort[2]),
        "the card after the cascade hit stays on top"
    );
}

#[test]
fn q41_cascade_cards_exile_only_lower_mana_value_nonlands() {
    for (index, (card, mana, not_cheaper)) in [
        (
            "shardless_agent",
            ManaGift {
                g: 1,
                u: 1,
                c: 1,
                ..Default::default()
            },
            "saltfield_recluse",
        ),
        (
            "maelstrom_colossus",
            ManaGift {
                c: 8,
                ..Default::default()
            },
            "maelstrom_colossus",
        ),
        (
            "enlisted_wurm",
            ManaGift {
                g: 1,
                w: 1,
                c: 4,
                ..Default::default()
            },
            "maelstrom_colossus",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(41_091_000 + index as u64, card);
        let cohort = set_library_top(&mut e, 0, &["forest", not_cheaper, "grizzly_bears", "opt"]);
        cast_creature(&mut e, card, mana);
        let batch = pass_both_players_with_batch(&mut e);
        let revealed = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal)) => {
                    Some(reveal)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{card}: cascade publicly reveals the exiled prefix"));
        assert_eq!(revealed.zone_owner_player_id, 0, "{card}");
        assert_eq!(
            revealed
                .cards
                .iter()
                .map(|card| card.object_id)
                .collect::<Vec<_>>(),
            cohort[..3],
            "{card}: cascade reveals through the first cheaper nonland"
        );
        let pending = e
            .state
            .pending_resolution
            .as_ref()
            .unwrap_or_else(|| panic!("{card}: Cascade offers a card"));
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
        let found = pending.presentation.candidates[0];
        let found_card = &e.state.objects[&found].card_id;
        let found_face = tricerules_cards::registry::global()
            .get(found_card)
            .expect("registered cascade hit")
            .primary_face();
        assert!(
            found_face.mana_cost.mana_value()
                < tricerules_cards::registry::global()
                    .get(card)
                    .unwrap()
                    .primary_face()
                    .mana_cost
                    .mana_value(),
            "{card}: the found spell costs less"
        );
        assert!(
            !found_face.types.iter().any(|t| t == "Land"),
            "{card}: the found card is not a land"
        );
        assert_eq!(found, cohort[2], "{card}: the first cheaper nonland");
    }
}

#[test]
fn q41_cascade_canary_hardest_finds_a_cheaper_spell_at_eight_mana() {
    let mut e = engine(41_093_001, "maelstrom_colossus");
    let cohort = set_library_top(&mut e, 0, &["forest", "grizzly_bears"]);
    cast_creature(
        &mut e,
        "maelstrom_colossus",
        ManaGift {
            c: 8,
            ..Default::default()
        },
    );
    pass_both_players(&mut e);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the eight-mana cascade offers a hit");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
    assert_eq!(pending.presentation.candidates, [cohort[1]]);
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline the free cast");
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q41_cascade_canary_casts_the_found_card_without_paying() {
    let mut e = engine(41_092_001, "shardless_agent");
    set_library_top(&mut e, 0, &["forest", "grizzly_bears", "opt"]);
    let library_before = e.state.players[0].library.len();
    cast_creature(
        &mut e,
        "shardless_agent",
        ManaGift {
            g: 1,
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    pass_both_players(&mut e);
    let pending = e.state.pending_resolution.as_ref().expect("cascade offer");
    let prompt = pending.resolution_prompt_id.get();
    let found = pending.presentation.candidates[0];
    let generation = e.state.zone_change_generation[&found];
    let action = e.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == found)
        .cloned()
        .expect("Cascade publishes the qualifying free cast");
    let announcement = rv1::CastSpell {
        source: Some(exile_cast_source(found, generation)),
        face_index: action.face_index,
        cast_method: action.cast_method,
        casting_permission_id: action.casting_permission_id,
        ..Default::default()
    };
    let command = RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
            resolution_prompt_id: prompt,
            decision: ResolutionChoiceDecision::CastSpell as i32,
            cast_spell: Some(announcement),
            ..Default::default()
        })),
    };
    e.apply_scenario_command(0, &command)
        .expect("cast the found card for free");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&found].zone, Zone::Battlefield);
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 1,
        "the found card left the library for the battlefield"
    );
    let mana = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            mana.white,
            mana.blue,
            mana.black,
            mana.red,
            mana.green,
            mana.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "the cascade cast paid no mana"
    );
}
