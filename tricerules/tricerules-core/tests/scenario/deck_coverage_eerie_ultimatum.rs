//! Eerie Ultimatum's exact card identity and graveyard return behavior.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

fn eerie_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[4, 9],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_eerie_ultimatum(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "eerie_ultimatum");
    give_mana(
        engine,
        4,
        ManaGift {
            w: 2,
            b: 3,
            g: 2,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(engine, 0, "eerie_ultimatum");
    semantic::accepted(engine, 4, &cast_spell(hand_index, Vec::new()));
    source
}

fn resolve_eerie_to_choice(engine: &mut GameEngine) {
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        engine
            .apply_command(actor, &pass())
            .expect("pass priority for Eerie Ultimatum");
    }
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn eerie_returns_different_named_permanents_and_keeps_simultaneous_aura_in_graveyard() {
    let mut engine = eerie_engine(20_261_012);
    let bear = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let duplicate_bear = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let planeswalker = inject_graveyard_card(&mut engine, 0, "jace_beleren");
    let land = inject_graveyard_card(&mut engine, 0, "forest");
    let artifact = inject_graveyard_card(&mut engine, 0, "sol_ring");
    let aura = inject_graveyard_card(&mut engine, 0, "pacifism");
    let nonpermanent = inject_graveyard_card(&mut engine, 0, "opt");
    let opponent_card = inject_graveyard_card(&mut engine, 1, "hill_giant");
    let source = cast_eerie_ultimatum(&mut engine);
    resolve_eerie_to_choice(&mut engine);

    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 4);
    assert_eq!(pending.presentation.min, 0);
    assert_eq!(pending.presentation.max, 6);
    assert!(pending.presentation.unique_names);
    for candidate in [bear, duplicate_bear, planeswalker, land, artifact, aura] {
        assert!(pending.presentation.candidates.contains(&candidate));
    }
    assert!(!pending.presentation.candidates.contains(&nonpermanent));
    assert!(!pending.presentation.candidates.contains(&opponent_card));

    let pending_before = serde_json::to_value(&engine.state).unwrap();
    let duplicate_result =
        engine.apply_command(4, &submit_resolution_choice(vec![bear, duplicate_bear]));
    assert!(
        duplicate_result.is_err(),
        "duplicate card names are illegal"
    );
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), pending_before);

    engine
        .apply_command(
            4,
            &submit_resolution_choice(vec![bear, planeswalker, land, artifact, aura]),
        )
        .expect("choose distinct names from the frozen graveyard cohort");

    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::SimultaneousEntryOrder
    );
    assert!(
        !answer_simultaneous_entry_order_in_engine_order(&mut engine).is_empty(),
        "the whole return cohort uses the shared simultaneous-entry path"
    );
    assert!(
        engine.state.pending_resolution.is_none(),
        "entry cohort must finish without a follow-up choice: {:#?}",
        engine.state.pending_resolution
    );
    assert_eq!(engine.state.objects[&bear].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&planeswalker].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&duplicate_bear].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonpermanent].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opponent_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn eerie_can_choose_zero_and_skips_the_choice_when_no_permanent_card_matches() {
    let mut with_candidates = eerie_engine(20_261_013);
    let bear = inject_graveyard_card(&mut with_candidates, 0, "grizzly_bears");
    let source = cast_eerie_ultimatum(&mut with_candidates);
    resolve_eerie_to_choice(&mut with_candidates);
    with_candidates
        .apply_command(4, &submit_resolution_choice(Vec::new()))
        .expect("zero is a legal number of cards");
    assert_eq!(with_candidates.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(with_candidates.state.objects[&source].zone, Zone::Graveyard);
    assert!(with_candidates.state.stack.is_empty());

    let mut without_candidates = eerie_engine(20_261_014);
    let instant = inject_graveyard_card(&mut without_candidates, 0, "opt");
    let source = cast_eerie_ultimatum(&mut without_candidates);
    for _ in 0..2 {
        let actor = without_candidates.state.priority_player_id();
        without_candidates
            .apply_command(actor, &pass())
            .expect("resolve Eerie Ultimatum with no permanent candidates");
    }
    assert!(without_candidates.state.pending_resolution.is_none());
    assert_eq!(
        without_candidates.state.objects[&instant].zone,
        Zone::Graveyard
    );
    assert_eq!(
        without_candidates.state.objects[&source].zone,
        Zone::Graveyard
    );
    assert!(without_candidates.state.stack.is_empty());
}

#[test]
fn eerie_rejects_a_graveyard_card_whose_zone_generation_changed_after_the_choice() {
    let mut engine = eerie_engine(20_261_015);
    let bear = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    cast_eerie_ultimatum(&mut engine);
    resolve_eerie_to_choice(&mut engine);
    *engine.state.zone_change_generation.entry(bear).or_default() += 1;
    let pending_before = serde_json::to_value(&engine.state).unwrap();

    let result = engine.apply_command(4, &submit_resolution_choice(vec![bear]));

    assert!(
        result.is_err(),
        "the choice names an earlier card incarnation"
    );
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), pending_before);
}

#[test]
fn eerie_serialized_resolution_choice_replays_identically() {
    use prost::Message;

    fn apply_and_compare(
        original: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: tricerules_proto::ruled::v1::RuledCommand,
    ) {
        let decoded =
            tricerules_proto::ruled::v1::RuledCommand::decode(command.encode_to_vec().as_slice())
                .expect("serialized ruled command");
        let observed = original
            .apply_command(actor, &command)
            .expect("original command");
        let reproduced = replay
            .apply_command(actor, &decoded)
            .expect("replayed command");
        assert_eq!(observed, reproduced);
        assert_eq!(
            original.diagnostic_snapshot().unwrap(),
            replay.diagnostic_snapshot().unwrap()
        );
    }

    fn replay_fixture() -> (GameEngine, u32) {
        let mut engine = eerie_engine(20_261_016);
        let bear = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
        inject_card_into_hand(&mut engine, 0, "eerie_ultimatum");
        give_mana(
            &mut engine,
            4,
            ManaGift {
                w: 2,
                b: 3,
                g: 2,
                ..Default::default()
            },
        );
        (engine, bear)
    }

    let (mut original, bear) = replay_fixture();
    let (mut replay, _) = replay_fixture();
    let hand_index = hand_index_for_card(&original, 0, "eerie_ultimatum");
    apply_and_compare(
        &mut original,
        &mut replay,
        4,
        cast_spell(hand_index, Vec::new()),
    );
    for _ in 0..2 {
        let actor = original.state.priority_player_id();
        apply_and_compare(&mut original, &mut replay, actor, pass());
    }
    let choice_actor = original
        .state
        .pending_resolution
        .as_ref()
        .expect("Eerie Ultimatum is waiting for its resolution choice")
        .deciding_player;
    apply_and_compare(
        &mut original,
        &mut replay,
        choice_actor,
        submit_resolution_choice(vec![bear]),
    );

    assert_eq!(original.state.objects[&bear].zone, Zone::Battlefield);
    assert!(original.state.pending_resolution.is_none());
    assert!(original.state.stack.is_empty());
}
