//! Exact-card artifact threshold, search, and untargeted entry-copy coverage.
use super::helpers::*;
use tricerules_cards::{CardRegistry, CounterKind};
use tricerules_core::Zone;

fn game(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![island_only_deck(), island_only_deck()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn remove_artifact(engine: &mut GameEngine, object: u32) {
    let controller = engine.state.objects[&object].controller as usize;
    engine.state.players[controller]
        .battlefield
        .retain(|id| *id != object);
    engine.state.objects.get_mut(&object).unwrap().zone = Zone::Graveyard;
    engine.state.players[controller].graveyard.push(object);
}

#[test]
fn fair_exact_land_and_mana_ability() {
    let card = CardRegistry::global()
        .get("inventors_fair")
        .expect("Fair registered");
    assert_eq!(card.name, "Inventors' Fair");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "");
    assert_eq!(face.types, ["Land"]);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(face.activated_abilities.len(), 2);
    let mut engine = game(202609324);
    let fair = inject_permanent_on_battlefield(&mut engine, 0, "inventors_fair");
    apply_ability(&mut engine, 0, fair, 0, vec![]).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&fair].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn fair_upkeep_checks_three_artifacts_at_trigger_and_resolution() {
    for (artifacts, remove_after_trigger, gain) in [(2, false, 0), (3, false, 1), (3, true, 0)] {
        let mut engine = game(202609325);
        inject_permanent_on_battlefield(&mut engine, 1, "inventors_fair");
        let mut objects = vec![];
        for _ in 0..artifacts {
            objects.push(inject_permanent_on_battlefield(&mut engine, 1, "sol_ring"));
        }
        // Opponent artifacts never supply the missing third controller artifact.
        inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        end_active_turn(&mut engine, 0);
        assert_eq!(engine.state.stack.len(), if artifacts == 3 { 1 } else { 0 });
        if remove_after_trigger {
            remove_artifact(&mut engine, objects[0]);
        }
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.players[1].life, 20 + gain);
        assert_eq!(engine.state.players[0].life, 20);
    }
}

#[test]
fn fair_activation_checks_before_costs_then_reveals_artifact_to_hand_and_shuffles() {
    let mut engine = game(202609326);
    let fair = inject_permanent_on_battlefield(&mut engine, 0, "inventors_fair");
    for _ in 0..2 {
        inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    }
    engine.state.players[0].mana_pool.colorless = 4;
    apply_ability(&mut engine, 0, fair, 1, vec![]).expect_err("only two artifacts");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 4);
    assert_eq!(engine.state.objects[&fair].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&fair].tapped);
    let third = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let artifact = inject_library_card(&mut engine, 0, "bottle_gnomes");
    let island = inject_library_card(&mut engine, 0, "island");
    let foreign = inject_library_card(&mut engine, 1, "sol_ring");
    apply_ability(&mut engine, 0, fair, 1, vec![]).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&fair].zone, Zone::Graveyard);
    // Activation restriction is not rechecked at resolution.
    remove_artifact(&mut engine, third);
    pass_both_players(&mut engine);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibrarySearch);
    assert_eq!((pending.presentation.min, pending.presentation.max), (0, 1));
    assert!(pending.presentation.candidates.contains(&artifact));
    assert!(!pending.presentation.candidates.contains(&island));
    assert!(!pending.presentation.candidates.contains(&foreign));
    let before = format!("{:?}", engine.state.pending_resolution);
    for (actor, choice) in [(1, artifact), (0, island), (0, foreign)] {
        engine
            .apply_command(actor, &submit_resolution_choice(vec![choice]))
            .expect_err("invalid chooser/card");
        assert_eq!(format!("{:?}", engine.state.pending_resolution), before);
        assert_eq!(engine.state.objects[&artifact].zone, Zone::Library);
    }
    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![artifact]))
        .unwrap();
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&artifact));
    assert!(completion
        .events
        .iter()
        .any(|e| matches!(&e.ev, Some(Ev::CardsRevealed(_)))));
    assert_eq!(
        completion
            .events
            .iter()
            .filter(
                |e| matches!(&e.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
            )
            .count(),
        1
    );
    assert!(engine.state.pending_resolution.is_none());
}

fn cast_steel(engine: &mut GameEngine) -> u32 {
    let steel = inject_card_into_hand(engine, 0, "sculpting_steel");
    give_mana(
        engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(engine, 0, "sculpting_steel");
    let cast = engine.apply_command(0, &cast_spell(index, vec![])).unwrap();
    assert!(find_resolution_choice(&cast).is_none());
    pass_both_players(engine);
    steel
}

#[test]
fn steel_filters_artifacts_untargeted_and_copies_values_not_status_or_identity() {
    let mut engine = game(202609327);
    let gnomes = inject_creature_on_battlefield(&mut engine, 1, "bottle_gnomes");
    engine.state.objects.get_mut(&gnomes).unwrap().tapped = true;
    engine
        .state
        .objects
        .get_mut(&gnomes)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 3);
    let ring = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let steel = cast_steel(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("entry copy choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert_eq!((pending.presentation.min, pending.presentation.max), (0, 1));
    assert!(pending.presentation.candidates.contains(&gnomes));
    assert!(pending.presentation.candidates.contains(&ring));
    assert!(!pending.presentation.candidates.contains(&bear));
    let before = format!("{:?}", engine.state.pending_resolution);
    for (actor, choice) in [(1, gnomes), (0, bear)] {
        engine
            .apply_command(actor, &submit_resolution_choice(vec![choice]))
            .expect_err("invalid entry choice");
        assert_eq!(format!("{:?}", engine.state.pending_resolution), before);
    }
    engine
        .apply_command(0, &submit_resolution_choice(vec![gnomes]))
        .unwrap();
    assert_eq!(engine.state.objects[&steel].card_id, "sculpting_steel");
    assert!(!engine.state.objects[&steel].is_token());
    assert_eq!(engine.state.objects[&steel].controller, 0);
    assert!(!engine.state.objects[&steel].tapped);
    assert_eq!(
        engine.state.objects[&steel].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(engine.effective_power(steel), Some(1));
    assert_eq!(engine.effective_toughness(steel), Some(3));
    assert_eq!(
        engine.state.objects[&steel]
            .copiable_values
            .as_ref()
            .unwrap()
            .face
            .types,
        ["Artifact", "Creature", "Gnome"]
    );
    apply_ability(&mut engine, 0, steel, 0, vec![]).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 23);
    assert_eq!(engine.state.objects[&steel].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&gnomes].zone, Zone::Battlefield);
}

#[test]
fn steel_copies_noncreature_artifact_etb_and_may_decline() {
    for choose in [false, true] {
        let mut engine = game(202609328);
        let wellspring = inject_permanent_on_battlefield(&mut engine, 1, "ichor_wellspring");
        let before = engine.state.players[0].hand.len();
        let steel = cast_steel(&mut engine);
        engine
            .apply_command(
                0,
                &submit_resolution_choice(if choose { vec![wellspring] } else { vec![] }),
            )
            .unwrap();
        assert_eq!(engine.state.objects[&steel].card_id, "sculpting_steel");
        assert_eq!(engine.state.objects[&steel].zone, Zone::Battlefield);
        if choose {
            assert_eq!(
                engine.state.stack.len(),
                1,
                "copied ETB triggers exactly once"
            );
            resolve_entire_stack_two_player(&mut engine);
            assert_eq!(engine.state.players[0].hand.len(), before + 1);
        } else {
            assert!(engine.state.stack.is_empty());
            assert!(engine.state.objects[&steel].copiable_values.is_none());
            assert_eq!(engine.state.players[0].hand.len(), before);
        }
    }
}

#[test]
fn steel_copies_artifact_token_and_copy_of_copy_without_becoming_token() {
    let card = CardRegistry::global().get("sculpting_steel").unwrap();
    assert_eq!(card.name, "Sculpting Steel");
    assert_eq!(card.face_count(), 1);
    assert_eq!(card.primary_face().mana_cost.to_string(), "{3}");
    assert_eq!(card.primary_face().types, ["Artifact"]);
    assert_eq!(card.primary_face().static_abilities.len(), 1);
    let mut engine = game(202609329);
    let token = inject_permanent_on_battlefield(&mut engine, 1, "treasure");
    assert!(engine.state.objects[&token].is_token());
    let first = cast_steel(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![token]))
        .unwrap();
    let second = cast_steel(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![first]))
        .unwrap();
    for steel in [first, second] {
        assert_eq!(engine.state.objects[&steel].card_id, "sculpting_steel");
        assert!(!engine.state.objects[&steel].is_token());
        let values = engine.state.objects[&steel]
            .copiable_values
            .as_ref()
            .unwrap();
        assert_eq!(values.face.name, "Treasure");
        assert_eq!(values.face.types, ["Artifact", "Treasure"]);
    }
}
