//! Exact Audacious Reshapers identity and rules behavior.

use super::helpers::*;
use tricerules_cards::ManaCost;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ResolutionChoiceDecision, SubmitResolutionChoice};

const AUDACIOUS_RESHAPERS: &str = "audacious_reshapers";

fn game(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn activated_player_uses_their_library_and_takes_the_damage_in_a_three_player_game() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        0x0A0D_5108,
        &[0, 1, 2],
        20,
        None,
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_permanent_on_battlefield(&mut engine, 1, AUDACIOUS_RESHAPERS);
    let payment_artifact = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
    let revealed = set_library(&mut engine, 1, &["forest", "sol_ring", "island"]);
    engine
        .apply_command(0, &pass())
        .expect("pass priority to the activating player");
    engine
        .apply_command(1, &activate(&engine, source, payment_artifact))
        .expect("player one activates reshapers");
    resolve_one(&mut engine);

    assert_eq!(engine.state.objects[&revealed[1]].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[1].life, 18);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[2].life, 20);
    assert_eq!(engine.state.players[1].library.front(), Some(&revealed[2]));
    assert_eq!(engine.state.players[1].library.back(), Some(&revealed[0]));
}

fn set_library(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    for object_id in std::mem::take(&mut engine.state.players[player].library) {
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
        engine.state.players[player].graveyard.push(object_id);
        *engine
            .state
            .zone_change_generation
            .entry(object_id)
            .or_insert(0) += 1;
    }
    card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect()
}

fn resolve_one(engine: &mut GameEngine) -> RuledEventBatch {
    let mut result = RuledEventBatch::default();
    let count = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        - engine.state.passes_since_stack_change as usize;
    for _ in 0..count {
        let actor = engine.state.priority_player_id();
        result = engine.apply_command(actor, &pass()).expect("pass priority");
    }
    result
}

fn activate(engine: &GameEngine, source: u32, artifact: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation
        .cost_selections
        .push(permanent_cost_selection(1, artifact));
    command
}

fn choose_entry_opponent(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn reveal_cards(batch: &RuledEventBatch) -> Vec<tricerules_proto::ruled::v1::RevealedCard> {
    batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal)) => {
                Some(reveal.cards.clone())
            }
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn audacious_reshapers_has_a_complete_card_definition() {
    let card = tricerules_cards::registry::global()
        .get(AUDACIOUS_RESHAPERS)
        .expect("Audacious Reshapers needs a complete definition");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost, ManaCost::parse("{2}{R}").unwrap());
    assert_eq!(face.types, ["Creature", "Human", "Artificer"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(face.activated_abilities.len(), 1);
}

#[test]
fn reveal_until_artifact_puts_first_match_into_play_and_deals_revealed_count_damage() {
    let mut engine = game(0x0A0D_5101);
    let source = inject_permanent_on_battlefield(&mut engine, 0, AUDACIOUS_RESHAPERS);
    let payment_artifact = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let revealed = set_library(&mut engine, 0, &["forest", "sol_ring", "island"]);

    let command = activate(&engine, source, payment_artifact);
    engine
        .apply_command(0, &command)
        .expect("activate reshapers");
    let resolved = resolve_one(&mut engine);

    assert_eq!(engine.state.objects[&revealed[1]].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[0].library.front(), Some(&revealed[2]));
    assert_eq!(engine.state.players[0].library.back(), Some(&revealed[0]));
    assert_eq!(engine.state.zone_change_generation[&revealed[0]], 1);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&revealed[2])
            .copied()
            .unwrap_or(0),
        0
    );
    assert_eq!(
        reveal_cards(&resolved)
            .iter()
            .map(|card| card.object_id)
            .collect::<Vec<_>>(),
        revealed[..2]
    );
}

fn resolve_without_artifact(seed: u64) -> (i32, Vec<u32>, Vec<u64>, Vec<u64>) {
    let mut engine = game(seed);
    let source = inject_permanent_on_battlefield(&mut engine, 0, AUDACIOUS_RESHAPERS);
    let payment_artifact = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let revealed = set_library(&mut engine, 0, &["forest", "island"]);
    engine
        .apply_command(0, &activate(&engine, source, payment_artifact))
        .expect("activate reshapers");
    let result = resolve_one(&mut engine);
    let old_generations = reveal_cards(&result)
        .iter()
        .map(|card| card.zone_change_generation)
        .collect();
    let current_generations = revealed
        .iter()
        .map(|object_id| {
            engine
                .state
                .zone_change_generation
                .get(object_id)
                .copied()
                .unwrap_or(0)
        })
        .collect();
    (
        engine.state.players[0].life,
        engine.state.players[0].library.iter().copied().collect(),
        old_generations,
        current_generations,
    )
}

#[test]
fn no_match_randomizes_all_revealed_cards_and_renews_their_generations() {
    let first = resolve_without_artifact(0x0A0D_5102);
    let replay = resolve_without_artifact(0x0A0D_5102);
    assert_eq!(first, replay, "the bottom order is replay-deterministic");
    assert_eq!(first.0, 18, "both revealed cards deal damage");
    assert_eq!(
        first.2,
        [0, 0],
        "the reveal event keeps original identities"
    );
    assert_eq!(first.3, [1, 1], "bottoming creates new library objects");
}

#[test]
fn empty_library_reveals_no_cards_and_deals_no_damage() {
    let mut engine = game(0x0A0D_5103);
    let source = inject_permanent_on_battlefield(&mut engine, 0, AUDACIOUS_RESHAPERS);
    let payment_artifact = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    set_library(&mut engine, 0, &[]);
    engine
        .apply_command(0, &activate(&engine, source, payment_artifact))
        .expect("activate with an empty library");
    let result = resolve_one(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
    assert!(reveal_cards(&result).is_empty());
}

#[test]
fn revealed_count_survives_a_parked_battlefield_entry_choice() {
    let mut engine = game(0x0A0D_5104);
    let source = inject_permanent_on_battlefield(&mut engine, 0, AUDACIOUS_RESHAPERS);
    let payment_artifact = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let revealed = set_library(&mut engine, 0, &["forest", "black_vise", "island"]);
    engine
        .apply_command(0, &activate(&engine, source, payment_artifact))
        .expect("activate reshapers");
    let result = resolve_one(&mut engine);

    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(
        reveal_cards(&result)
            .iter()
            .map(|card| card.object_id)
            .collect::<Vec<_>>(),
        revealed[..2]
    );
    engine
        .apply_command(0, &choose_entry_opponent(0))
        .expect("finish Black Vise's entry choice");
    assert_eq!(engine.state.objects[&revealed[1]].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[0].library.front(), Some(&revealed[2]));
    assert_eq!(engine.state.players[0].library.back(), Some(&revealed[0]));
    assert_eq!(engine.state.zone_change_generation[&revealed[0]], 1);
}

#[test]
fn source_departure_after_activation_uses_last_known_damage_source() {
    let mut engine = game(0x0A0D_5105);
    let source = inject_permanent_on_battlefield(&mut engine, 0, AUDACIOUS_RESHAPERS);
    let payment_artifact = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let revealed = set_library(&mut engine, 0, &["sol_ring"]);
    let bolt = inject_card_into_hand(&mut engine, 1, "lightning_bolt");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate(&engine, source, payment_artifact))
        .expect("activate reshapers");
    if engine.state.priority_player_id() != 1 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .expect("pass priority to the opponent");
    }
    engine
        .apply_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "lightning_bolt"),
                target_object(source),
            ),
        )
        .expect("destroy the ability source above its ability");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&bolt].zone, Zone::Graveyard);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 19);
    assert_eq!(engine.state.objects[&revealed[0]].zone, Zone::Battlefield);
}

#[test]
fn damage_prevention_applies_to_the_revealed_count_damage() {
    let mut engine = game(0x0A0D_5106);
    let source = inject_permanent_on_battlefield(&mut engine, 0, AUDACIOUS_RESHAPERS);
    let payment_artifact = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    set_library(&mut engine, 0, &["forest", "sol_ring"]);
    let player_id = engine.state.players[0].id;
    engine
        .state
        .add_damage_prevention_shield(player_id as u32, 1);
    engine
        .apply_command(0, &activate(&engine, source, payment_artifact))
        .expect("activate reshapers");
    resolve_one(&mut engine);

    assert_eq!(engine.state.players[0].life, 19);
    assert_eq!(
        engine.state.remaining_damage_prevention(player_id as u32),
        0
    );
}

#[test]
fn artifact_cost_rejects_nonartifact_and_opponent_permanents_atomically() {
    let mut engine = game(0x0A0D_5107);
    let source = inject_permanent_on_battlefield(&mut engine, 0, AUDACIOUS_RESHAPERS);
    let nonartifact = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let opponent_artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    for selection in [nonartifact, opponent_artifact] {
        let before = format!("{:?}", engine.state);
        assert!(engine
            .apply_command(0, &activate(&engine, source, selection))
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
    }
}
