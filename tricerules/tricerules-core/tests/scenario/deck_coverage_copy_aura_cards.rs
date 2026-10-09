use crate::helpers::*;
use tricerules_core::{state::AttachmentRecipient, GameEngine, Zone};
use tricerules_proto::ruled::v1::{ruled_event::Ev, ChoiceKind};

fn copy_card_engine(seed: u64, copy_card: &str) -> GameEngine {
    let decks = Some(vec![
        vec![copy_card.to_string(); 20],
        vec!["island".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_hand(&mut engine, 0, copy_card);
    engine
}

fn reanimate_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        vec!["reanimate".to_string(); 20],
        vec!["island".to_string(); 20],
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_hand(&mut engine, 0, "reanimate");
    engine.state.players[0].mana_pool.black = 1;
    engine
}

fn source_aura(engine: &mut GameEngine) -> (u32, u32) {
    let host = inject_creature_on_battlefield(engine, 1, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(engine, 1, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(host));
    (aura, host)
}

fn cast_and_open_copy_choice(engine: &mut GameEngine, card: &str, blue: u32, generic: u32) -> u32 {
    engine.state.players[0].mana_pool.blue = blue;
    engine.state.players[0].mana_pool.colorless = generic;
    let copy = engine.state.players[0].hand[hand_index_for_card(engine, 0, card)];
    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(engine, 0, card), Vec::new()),
        )
        .expect("cast entry-copy permanent");
    pass_both_players(engine);
    copy
}

#[test]
fn clever_impersonator_copies_nonland_permanents_and_attaches_when_copying_an_aura() {
    let mut engine = copy_card_engine(303_501, "clever_impersonator");
    let (aura, source_host) = source_aura(&mut engine);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "island");
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let copy = cast_and_open_copy_choice(&mut engine, "clever_impersonator", 2, 2);

    let copy_sources = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copy-source choice");
    assert_eq!(
        copy_sources.presentation.candidates,
        vec![recipient, source_host, aura, artifact],
        "source_host={source_host} aura={aura} artifact={artifact} recipient={recipient}"
    );
    assert!(copy_sources
        .presentation
        .prompt
        .contains("nonland permanent"));
    assert!(!copy_sources.presentation.candidates.contains(&land));
    engine
        .apply_command(0, &submit_resolution_choice(vec![aura]))
        .expect("choose Aura as copy source");
    let aura_recipients = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura recipient choice");
    assert_eq!(
        aura_recipients.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );
    assert_eq!(
        aura_recipients.presentation.candidates,
        vec![source_host, recipient]
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![recipient]))
        .expect("choose legal enchanted permanent");

    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&copy].attached_to,
        Some(AttachmentRecipient::Object(recipient))
    );
    assert_eq!(engine.characteristics(copy).unwrap().names, ["Pacifism"]);
}

#[test]
fn mirrormade_limits_copy_sources_to_artifacts_or_enchantments() {
    let mut engine = copy_card_engine(303_502, "mirrormade");
    let (aura, source_host) = source_aura(&mut engine);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "island");
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let copy = cast_and_open_copy_choice(&mut engine, "mirrormade", 2, 1);

    let copy_sources = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copy-source choice");
    assert_eq!(
        copy_sources.presentation.candidates,
        vec![aura, artifact],
        "source_host={source_host} aura={aura} artifact={artifact} creature={creature} land={land} recipient={recipient}"
    );
    assert!(copy_sources
        .presentation
        .prompt
        .contains("artifact or enchantment"));
    assert!(!copy_sources.presentation.candidates.contains(&creature));
    assert!(!copy_sources.presentation.candidates.contains(&land));
    engine
        .apply_command(0, &submit_resolution_choice(vec![aura]))
        .expect("choose Aura as copy source");
    let aura_recipients = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura recipient choice");
    assert_eq!(
        aura_recipients.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );
    assert_eq!(
        aura_recipients.presentation.candidates,
        vec![source_host, creature, recipient]
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![recipient]))
        .expect("choose legal enchanted permanent");

    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&copy].attached_to,
        Some(AttachmentRecipient::Object(recipient))
    );
    assert_eq!(engine.characteristics(copy).unwrap().names, ["Pacifism"]);
}

#[test]
fn sculpting_steel_copies_only_artifacts() {
    let mut engine = copy_card_engine(303_503, "sculpting_steel");
    let (aura, _) = source_aura(&mut engine);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "island");
    let copy = cast_and_open_copy_choice(&mut engine, "sculpting_steel", 0, 3);

    let copy_sources = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copy-source choice");
    assert_eq!(copy_sources.presentation.candidates, vec![artifact]);
    assert!(copy_sources.presentation.prompt.contains("an artifact"));
    assert!(!copy_sources.presentation.candidates.contains(&aura));
    assert!(!copy_sources.presentation.candidates.contains(&creature));
    assert!(!copy_sources.presentation.candidates.contains(&land));
    engine
        .apply_command(0, &submit_resolution_choice(vec![artifact]))
        .expect("choose artifact as copy source");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert_eq!(engine.characteristics(copy).unwrap().names, ["Sol Ring"]);
}

#[test]
fn sculpting_steel_can_copy_an_aura_that_is_an_artifact() {
    let mut engine = copy_card_engine(303_504, "sculpting_steel");
    let (aura, source_host) = source_aura(&mut engine);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let artifact_type = tricerules_cards::primitives::PermanentTypeFilter::Artifact;
    engine
        .state
        .continuous_effects
        .push(tricerules_core::state::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::state::AffectedScope::PermanentsMatching {
                reference_player: 0,
                filter: Box::new(tricerules_cards::primitives::TargetFilter {
                    kind: tricerules_cards::primitives::TargetKind::AnyPermanent,
                    ..Default::default()
                }),
                exclude: None,
            },
            kind: tricerules_cards::primitives::ContinuousEffectKind::Layer4AddTypes(
                tricerules_cards::primitives::TypeLineAddition {
                    land_types: Vec::new(),
                    card_types: vec![artifact_type],
                    creature_types: Vec::new(),
                },
            ),
            condition: None,
            duration: tricerules_cards::primitives::EffectDuration::Indefinite,
            timestamp: 1,
        });
    let copy = cast_and_open_copy_choice(&mut engine, "sculpting_steel", 0, 3);

    let copy_sources = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copy-source choice");
    assert!(
        copy_sources.presentation.candidates.contains(&aura),
        "source_host={source_host} aura={aura} artifact={artifact} creature={creature} recipient={recipient} candidates={:?}",
        copy_sources.presentation.candidates
    );
    assert!(copy_sources.presentation.candidates.contains(&artifact));
    assert!(copy_sources.presentation.candidates.contains(&creature));
    engine
        .apply_command(0, &submit_resolution_choice(vec![aura]))
        .expect("choose artifact Aura as copy source");
    let aura_recipients = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura recipient choice");
    assert_eq!(
        aura_recipients.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );
    assert_eq!(
        aura_recipients.presentation.candidates,
        vec![source_host, creature, recipient]
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![recipient]))
        .expect("choose legal enchanted permanent");

    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&copy].attached_to,
        Some(AttachmentRecipient::Object(recipient))
    );
    assert_eq!(engine.characteristics(copy).unwrap().names, ["Pacifism"]);
}

#[test]
fn reanimated_clever_impersonator_stays_in_its_graveyard_when_its_copied_aura_has_no_recipient() {
    let mut engine = reanimate_engine(303_505);
    let clever = inject_graveyard_card(&mut engine, 0, "clever_impersonator");
    let source_host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "cartouche_of_knowledge");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(source_host));
    let life = engine.state.players[0].life;
    let cast = cast_spell(
        hand_index_for_card(&engine, 0, "reanimate"),
        vec![tricerules_proto::ruled::v1::TargetRef {
            expected_zone_change_generation: None,
            object_id: clever,
            damage_amount: 0,
            group_index: 0,
            kind: 0,
        }],
    );
    engine.apply_command(0, &cast).expect("cast Reanimate");
    pass_both_players(&mut engine);

    let copy_sources = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copy-source choice");
    assert!(copy_sources.presentation.candidates.contains(&aura));
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![aura]))
        .expect("choose the enchant-you-control Aura");

    assert_eq!(engine.state.objects[&clever].zone, Zone::Graveyard);
    assert!(engine.state.players[0].graveyard.contains(&clever));
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .all(|id| *id != clever));
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].life, life - 4);
    assert!(engine.state.objects[&clever].copiable_values.is_none());
    assert_eq!(engine.state.objects[&clever].copy_revision, 0);
    assert!(
        engine.state.stack.is_empty(),
        "the Aura ETB draw must not trigger"
    );
    assert!(!batch.events.iter().any(|event| matches!(
        event.ev,
        Some(Ev::PermanentMoved(ref moved))
            if moved.object_id == clever
                && moved.destination == tricerules_proto::ruled::v1::permanent_moved::Destination::Battlefield as i32
    )));
}
