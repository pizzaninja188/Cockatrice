//! Actual paid targetless graveyard returns, including interrupted simultaneous entry.
use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

fn setup(card: &str) -> GameEngine {
    setup_for(card, &[4, 9, 27])
}

fn setup_for(card: &str, players: &[i32]) -> GameEngine {
    let mut engine = GameEngine::new(
        202_610_501,
        players,
        20,
        Some(vec![deck_with("forest", &[card]); players.len()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, card);
    engine
}

#[test]
fn triumphant_reckoning_abandoned_cohort_restores_only_current_provisional_aura_copies() {
    for (stale_copy, revision_stale) in [(false, false), (false, true), (true, false), (true, true)]
    {
        let mut engine = setup("triumphant_reckoning");
        let copy = inject_graveyard_card(&mut engine, 0, "mirrormade");
        let aura = inject_graveyard_card(&mut engine, 0, "pacifism");
        let artifact = inject_graveyard_card(&mut engine, 0, "mind_stone");
        let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let source = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
        engine.state.objects.get_mut(&source).unwrap().attached_to =
            Some(tricerules_core::AttachmentRecipient::Object(host));
        let spell = cast_and_resolve(&mut engine, "triumphant_reckoning");
        engine
            .apply_command(4, &submit_resolution_choice(vec![source]))
            .unwrap();
        engine
            .apply_command(4, &submit_resolution_choice(vec![host]))
            .unwrap();
        engine
            .apply_command(4, &submit_resolution_choice(vec![host]))
            .unwrap();
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
        assert_eq!(engine.state.objects[&copy].copy_revision, 1);
        let stale = if stale_copy { copy } else { aura };
        if revision_stale {
            engine.state.objects.get_mut(&stale).unwrap().copy_revision += 1;
        } else {
            *engine
                .state
                .zone_change_generation
                .entry(stale)
                .or_default() += 1;
        }
        let changed_values =
            serde_json::to_value(&engine.state.objects[&copy].copiable_values).unwrap();
        let changed_revision = engine.state.objects[&copy].copy_revision;
        let changed_generation = engine
            .state
            .zone_change_generation
            .get(&stale)
            .copied()
            .unwrap_or(0);
        let batch = engine.apply_command(27, &concede()).unwrap();
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&stale)
                .copied()
                .unwrap_or(0),
            changed_generation
        );
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(
                    event.ev,
                    Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackResolved(
                        _
                    ))
                ))
                .count(),
            1
        );
        for oid in [copy, aura, artifact, spell] {
            assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
        }
        if stale_copy {
            assert_eq!(
                serde_json::to_value(&engine.state.objects[&copy].copiable_values).unwrap(),
                changed_values
            );
            assert_eq!(engine.state.objects[&copy].copy_revision, changed_revision);
        } else {
            assert!(
                engine.state.objects[&copy].copiable_values.is_none(),
                "abandoned unchanged Mirrormade must regain its original identity"
            );
            assert_eq!(engine.state.objects[&copy].copy_revision, 0);
            assert_eq!(
                engine
                    .state
                    .zone_change_generation
                    .get(&copy)
                    .copied()
                    .unwrap_or(0),
                0
            );
        }
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn triumphant_reckoning_stale_accepted_aura_entrant_does_not_refuse_concession() {
    use tricerules_proto::ruled::v1::ruled_event::Ev;
    for revision_stale in [false, true] {
        let mut engine = setup("triumphant_reckoning");
        let aura = inject_graveyard_card(&mut engine, 0, "pacifism");
        let artifact = inject_graveyard_card(&mut engine, 0, "mind_stone");
        let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let spell = cast_and_resolve(&mut engine, "triumphant_reckoning");
        engine
            .apply_command(4, &submit_resolution_choice(vec![host]))
            .unwrap();
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
        // Defensive fixture only: gameplay and dev commands cannot move a blocked entrant.
        if revision_stale {
            engine.state.objects.get_mut(&aura).unwrap().copy_revision += 1;
        } else {
            *engine.state.zone_change_generation.entry(aura).or_default() += 1;
        }
        let generation = engine
            .state
            .zone_change_generation
            .get(&aura)
            .copied()
            .unwrap_or(0);
        let revision = engine.state.objects[&aura].copy_revision;
        let batch = engine
            .apply_command(27, &concede())
            .expect("concession remains legal during stale recovery");
        assert!(engine.state.players[2].has_lost);
        for oid in [aura, artifact, spell] {
            assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
        }
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&aura)
                .copied()
                .unwrap_or(0),
            generation
        );
        assert_eq!(engine.state.objects[&aura].copy_revision, revision);
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(event.ev, Some(Ev::StackResolved(_))))
                .count(),
            1
        );
        assert!(!batch
            .events
            .iter()
            .any(|event| matches!(event.ev, Some(Ev::AuraAttached(_)))));
    }
}

#[test]
fn graveyard_return_empty_cohort_finishes_and_extra_targets_reject_atomically() {
    for card in ["primevals_glorious_rebirth", "triumphant_reckoning"] {
        let mut engine = setup(card);
        let qualifier = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
        engine
            .state
            .objects
            .get_mut(&qualifier)
            .unwrap()
            .counters
            .insert(tricerules_cards::CounterKind::Loyalty, 3);
        let excluded = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
        engine.state.players[0].mana_pool.colorless = 6;
        engine.state.players[0].mana_pool.white = 3;
        engine.state.players[0].mana_pool.black = 1;
        let slot = hand_index_for_card(&engine, 0, card);
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(4, &cast_spell(slot, target_object(excluded)))
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        let spell = cast_and_resolve(&mut engine, card);
        assert_eq!(engine.state.objects[&excluded].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn triumphant_reckoning_aura_entry_is_untargeted_but_respects_enchantment_protection() {
    use tricerules_cards::primitives::ProtectionQuality;
    use tricerules_cards::{Color, ContinuousEffectKind, EffectDuration, Keyword};
    for kind in [
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Hexproof),
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Shroud),
        ContinuousEffectKind::Layer6AddProtection(ProtectionQuality::Color(Color::White)),
    ] {
        let is_protection = matches!(kind, ContinuousEffectKind::Layer6AddProtection(_));
        let mut engine = setup("triumphant_reckoning");
        let aura = inject_graveyard_card(&mut engine, 0, "pacifism");
        let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        engine
            .state
            .continuous_effects
            .push(tricerules_core::ContinuousEffect {
                source_id: None,
                trigger_grant_origin: None,
                affected: tricerules_core::AffectedScope::Single(host),
                kind,
                condition: None,
                duration: EffectDuration::Indefinite,
                timestamp: engine.state.command_index,
            });
        cast_and_resolve(&mut engine, "triumphant_reckoning");
        if is_protection {
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
            assert_eq!(engine.state.objects[&aura].attached_to, None);
        } else {
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(pending.presentation.choice_kind, ChoiceKind::AuraPermanent);
            assert_eq!(pending.presentation.candidates, vec![host]);
            engine
                .apply_command(4, &submit_resolution_choice(vec![host]))
                .unwrap();
            finish_order(&mut engine);
            assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
            assert_eq!(
                engine.state.objects[&aura].attached_to,
                Some(tricerules_core::AttachmentRecipient::Object(host))
            );
        }
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn graveyard_return_entrants_observe_the_whole_simultaneous_cohort() {
    for (card, entrants) in [
        (
            "primevals_glorious_rebirth",
            ["atraxa,_praetors_voice", "ghalta,_primal_hunger"],
        ),
        (
            "triumphant_reckoning",
            ["nyxborn_courser", "nyxborn_courser"],
        ),
    ] {
        let mut engine = setup(card);
        if card == "primevals_glorious_rebirth" {
            let qualifier = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
            engine
                .state
                .objects
                .get_mut(&qualifier)
                .unwrap()
                .counters
                .insert(tricerules_cards::CounterKind::Loyalty, 3);
        }
        let observer = tricerules_cards::CardRegistry::global()
            .get("soul_warden")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .clone();
        // Existing granted observer fixture: both entering creatures see the other's entry.
        engine
            .state
            .add_triggered_ability_grant(tricerules_core::ContinuousEffect {
                source_id: None,
                trigger_grant_origin: None,
                affected: tricerules_core::AffectedScope::AllCreatures,
                kind: tricerules_cards::ContinuousEffectKind::GrantTriggeredAbility(Box::new(
                    observer,
                )),
                condition: None,
                duration: tricerules_cards::EffectDuration::Indefinite,
                timestamp: 1,
            });
        let cohort: Vec<_> = entrants
            .iter()
            .map(|entry| inject_graveyard_card(&mut engine, 0, entry))
            .collect();
        cast_and_resolve(&mut engine, card);
        assert!(cohort
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Graveyard));
        finish_order(&mut engine);
        answer_trigger_order_in_engine_order(&mut engine);
        assert_eq!(
            engine.state.stack.len(),
            2,
            "two simultaneous observers, not one sequential observer"
        );
        for _ in 0..4 {
            if engine.state.stack.is_empty() {
                break;
            }
            pass_priority_round(&mut engine);
        }
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.players[0].life, 22);
        assert!(cohort
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Battlefield));
    }
}

#[test]
fn primevals_glorious_rebirth_requires_a_controlled_legendary_creature_or_planeswalker() {
    for qualifier in [
        None,
        Some((0, "alhammarrets_archive")),
        Some((1, "ghalta,_primal_hunger")),
    ] {
        let mut engine = setup("primevals_glorious_rebirth");
        if let Some((seat, card)) = qualifier {
            inject_permanent_on_battlefield(&mut engine, seat, card);
        }
        engine.state.players[0].mana_pool.colorless = 5;
        engine.state.players[0].mana_pool.white = 1;
        engine.state.players[0].mana_pool.black = 1;
        let slot = hand_index_for_card(&engine, 0, "primevals_glorious_rebirth");
        let before = format!("{:?}", engine.state);
        assert!(engine.apply_command(4, &cast_spell(slot, vec![])).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        assert!(!engine.initial_response_batch().legal_by_player[&4]
            .hand_actions
            .iter()
            .any(|action| action.hand_index as usize == slot));
    }
}

#[test]
fn primevals_glorious_rebirth_returns_legendary_permanent_cards_including_land() {
    for qualifier in ["ghalta,_primal_hunger", "jace_beleren"] {
        let mut engine = setup("primevals_glorious_rebirth");
        let qualifying_object = inject_permanent_on_battlefield(&mut engine, 0, qualifier);
        if qualifier == "jace_beleren" {
            engine
                .state
                .objects
                .get_mut(&qualifying_object)
                .unwrap()
                .counters
                .insert(tricerules_cards::CounterKind::Loyalty, 3);
        }
        let eligible: Vec<_> = [
            "boseiju,_who_endures",
            "alhammarrets_archive",
            "atraxa,_praetors_voice",
        ]
        .iter()
        .map(|card| inject_graveyard_card(&mut engine, 0, card))
        .collect();
        let excluded: Vec<_> = [
            "forest",
            "mind_stone",
            "grizzly_bears",
            "primevals_glorious_rebirth",
        ]
        .iter()
        .map(|card| inject_graveyard_card(&mut engine, 0, card))
        .collect();
        let opponent = inject_graveyard_card(&mut engine, 1, "boseiju,_who_endures");
        let spell = cast_and_resolve(&mut engine, "primevals_glorious_rebirth");
        assert!(eligible
            .iter()
            .all(|oid| engine.state.objects[oid].zone == Zone::Graveyard));
        finish_order(&mut engine);
        for oid in eligible {
            assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
            assert_eq!(engine.state.zone_change_generation[&oid], 1);
        }
        for oid in excluded.into_iter().chain([opponent, spell]) {
            assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
        }
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn triumphant_reckoning_cannot_attach_aura_to_another_simultaneous_entrant() {
    let mut engine = setup("triumphant_reckoning");
    let aura = inject_graveyard_card(&mut engine, 0, "pacifism");
    let creature = inject_graveyard_card(&mut engine, 0, "nyxborn_courser");
    let artifact = inject_graveyard_card(&mut engine, 0, "mind_stone");
    cast_and_resolve(&mut engine, "triumphant_reckoning");
    finish_order(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&aura)
            .copied()
            .unwrap_or(0),
        0
    );
    for oid in [creature, artifact] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    }
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn triumphant_reckoning_refreshes_unanswered_aura_while_skipping_prior_accepted_choice() {
    for players in [&[4, 9, 27][..], &[4, 9, 27, 42][..]] {
        let mut engine = setup_for("triumphant_reckoning", players);
        let first_aura = inject_graveyard_card(&mut engine, 0, "pacifism");
        let second_aura = inject_graveyard_card(&mut engine, 0, "pacifism");
        let artifact = inject_graveyard_card(&mut engine, 0, "mind_stone");
        let departing_host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let surviving_host = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
        cast_and_resolve(&mut engine, "triumphant_reckoning");
        engine
            .apply_command(4, &submit_resolution_choice(vec![departing_host]))
            .unwrap();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            ChoiceKind::AuraPermanent
        );
        let before = format!("{:?}", engine.state);
        assert!(engine
            .apply_command(27, &submit_resolution_choice(vec![surviving_host]))
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        assert!(engine
            .apply_command(4, &submit_resolution_choice(vec![artifact]))
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
        engine.apply_command(9, &concede()).unwrap();
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("second choice remains owed");
        assert!(!pending.presentation.candidates.contains(&departing_host));
        assert!(pending.presentation.candidates.contains(&surviving_host));
        engine
            .apply_command(4, &submit_resolution_choice(vec![surviving_host]))
            .unwrap();
        finish_order(&mut engine);
        assert_eq!(engine.state.objects[&first_aura].zone, Zone::Graveyard);
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&first_aura)
                .copied()
                .unwrap_or(0),
            0
        );
        assert_eq!(engine.state.objects[&second_aura].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.objects[&second_aura].attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(surviving_host))
        );
        assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn triumphant_reckoning_restores_skipped_aura_copy_but_keeps_accepted_values_if_copy_source_leaves()
{
    for recipient_leaves in [true, false] {
        let mut engine = setup_for("triumphant_reckoning", &[4, 9, 27, 42]);
        let copy = inject_graveyard_card(&mut engine, 0, "mirrormade");
        let artifact = inject_graveyard_card(&mut engine, 0, "mind_stone");
        let recipient = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let source_host = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
        let source = inject_permanent_on_battlefield(&mut engine, 2, "pacifism");
        engine.state.objects.get_mut(&source).unwrap().attached_to =
            Some(tricerules_core::AttachmentRecipient::Object(source_host));
        let spell = cast_and_resolve(&mut engine, "triumphant_reckoning");
        engine
            .apply_command(4, &submit_resolution_choice(vec![source]))
            .expect("Mirrormade copies Pacifism");
        engine
            .apply_command(4, &submit_resolution_choice(vec![recipient]))
            .expect("copy accepts existing recipient");
        assert_eq!(engine.state.objects[&copy].copy_revision, 1);
        engine
            .apply_command(if recipient_leaves { 9 } else { 27 }, &concede())
            .unwrap();
        finish_order(&mut engine);
        if recipient_leaves {
            assert_eq!(engine.state.objects[&copy].zone, Zone::Graveyard);
            assert_eq!(engine.state.objects[&copy].copy_revision, 0);
            assert!(engine.state.objects[&copy].copiable_values.is_none());
            assert_eq!(
                engine
                    .state
                    .zone_change_generation
                    .get(&copy)
                    .copied()
                    .unwrap_or(0),
                0
            );
            assert_eq!(engine.state.objects[&copy].attached_to, None);
        } else {
            assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
            assert!(engine.characteristics(copy).unwrap().is_aura());
            assert_eq!(
                engine.state.objects[&copy].attached_to,
                Some(tricerules_core::AttachmentRecipient::Object(recipient))
            );
            assert_eq!(engine.state.zone_change_generation[&copy], 1);
        }
        assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn triumphant_reckoning_does_not_rebind_accepted_recipient_to_a_new_incarnation_fixture() {
    let mut engine = setup("triumphant_reckoning");
    let aura = inject_graveyard_card(&mut engine, 0, "pacifism");
    let artifact = inject_graveyard_card(&mut engine, 0, "mind_stone");
    let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_and_resolve(&mut engine, "triumphant_reckoning");
    engine
        .apply_command(4, &submit_resolution_choice(vec![host]))
        .unwrap();
    // Internal fixture models the recipient leaving and returning under the same object ID.
    *engine.state.zone_change_generation.entry(host).or_default() += 2;
    finish_order(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&aura)
            .copied()
            .unwrap_or(0),
        0
    );
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
}

#[test]
fn triumphant_reckoning_copied_planeswalker_gets_printed_loyalty_once() {
    let mut engine = setup("triumphant_reckoning");
    let returned = inject_graveyard_card(&mut engine, 0, "jace_beleren");
    let copy = inject_graveyard_card(&mut engine, 0, "mirrormade");
    let source = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .counters
        .insert(tricerules_cards::CounterKind::Loyalty, 7);
    // Existing type-changing effect makes the source legal for Mirrormade's copy choice.
    // Those added types and its seven counters are not copiable values.
    engine
        .state
        .continuous_effects
        .push(tricerules_core::ContinuousEffect {
            source_id: None,
            trigger_grant_origin: None,
            affected: tricerules_core::AffectedScope::Single(source),
            kind: tricerules_cards::ContinuousEffectKind::Layer4AddTypes(
                tricerules_cards::primitives::TypeLineAddition {
                    card_types: vec![tricerules_cards::PermanentTypeFilter::Artifact],
                    ..Default::default()
                },
            ),
            condition: None,
            duration: tricerules_cards::EffectDuration::Indefinite,
            timestamp: engine.state.command_index,
        });
    cast_and_resolve(&mut engine, "triumphant_reckoning");
    engine
        .apply_command(4, &submit_resolution_choice(vec![source]))
        .unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::SimultaneousEntryOrder
    );
    let order = pending.presentation.candidates.clone();
    engine
        .apply_command(4, &submit_resolution_choice(order))
        .unwrap();
    // Legend rule: same-controller Jaces enter together. The copy uses Jace's printed name.
    if let Some(pending) = engine.state.pending_resolution.as_ref() {
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::LegendKeep);
        let actor = pending.deciding_player;
        engine
            .apply_command(actor, &submit_resolution_choice(vec![copy]))
            .unwrap();
    }
    assert_eq!(
        engine.state.objects[&copy].counter_count(tricerules_cards::CounterKind::Loyalty),
        3
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(tricerules_cards::CounterKind::Loyalty),
        7
    );
    assert_eq!(engine.state.objects[&returned].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&copy].zone, Zone::Battlefield);
    assert!(!engine.characteristics(copy).unwrap().is_artifact());
    assert!(engine.state.stack.is_empty());
}

fn cast_and_resolve(engine: &mut GameEngine, card: &str) -> u32 {
    let spell = paid_cast(engine, card);
    for _ in 0..engine.state.players.len() {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    spell
}

#[test]
fn triumphant_reckoning_logged_aura_choice_and_concession_replay_identically() {
    use prost::Message;
    fn fixture() -> (GameEngine, u32, u32, u32) {
        let mut engine = setup("triumphant_reckoning");
        let aura = inject_graveyard_card(&mut engine, 0, "pacifism");
        let artifact = inject_graveyard_card(&mut engine, 0, "mind_stone");
        let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        engine.state.players[0].mana_pool.colorless = 6;
        engine.state.players[0].mana_pool.white = 3;
        (engine, aura, artifact, host)
    }
    let (mut original, aura, artifact, host) = fixture();
    let (mut replay, _, _, _) = fixture();
    let slot = hand_index_for_card(&original, 0, "triumphant_reckoning");
    let commands = [
        (4, cast_spell(slot, vec![])),
        (4, pass()),
        (9, pass()),
        (27, pass()),
        (4, submit_resolution_choice(vec![host])),
        (9, concede()),
    ];
    for (actor, command) in commands {
        let decoded =
            tricerules_proto::ruled::v1::RuledCommand::decode(command.encode_to_vec().as_slice())
                .unwrap();
        let observed = original.apply_command(actor, &command).unwrap();
        let reproduced = replay.apply_command(actor, &decoded).unwrap();
        assert_eq!(observed, reproduced);
        assert_eq!(
            original.diagnostic_snapshot().unwrap(),
            replay.diagnostic_snapshot().unwrap()
        );
    }
    assert_eq!(original.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(
        original
            .state
            .zone_change_generation
            .get(&aura)
            .copied()
            .unwrap_or(0),
        0
    );
    assert_eq!(original.state.objects[&artifact].zone, Zone::Battlefield);
    assert!(original.state.pending_resolution.is_none());
    assert!(original.state.stack.is_empty());
}

fn paid_cast(engine: &mut GameEngine, card: &str) -> u32 {
    engine.state.players[0].mana_pool.colorless = if card == "primevals_glorious_rebirth" {
        5
    } else {
        6
    };
    engine.state.players[0].mana_pool.white = if card == "primevals_glorious_rebirth" {
        1
    } else {
        3
    };
    engine.state.players[0].mana_pool.black = if card == "primevals_glorious_rebirth" {
        1
    } else {
        0
    };
    let slot = hand_index_for_card(engine, 0, card);
    let spell = engine.state.players[0].hand[slot];
    engine
        .apply_command(4, &cast_spell(slot, vec![]))
        .expect("paid actual spell");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert_eq!(engine.state.players[0].mana_pool.black, 0);
    spell
}

#[test]
fn graveyard_return_twincast_uses_copy_controller_and_original_selects_fresh_cohort() {
    for card in ["primevals_glorious_rebirth", "triumphant_reckoning"] {
        let mut engine = setup(card);
        if card == "primevals_glorious_rebirth" {
            inject_permanent_on_battlefield(&mut engine, 0, "ghalta,_primal_hunger");
        }
        let original_return = inject_graveyard_card(&mut engine, 0, "alhammarrets_archive");
        let copy_return = inject_graveyard_card(&mut engine, 1, "alhammarrets_archive");
        let original = paid_cast(&mut engine, card);
        engine.apply_command(4, &pass()).unwrap();
        inject_card_into_hand(&mut engine, 1, "twincast");
        engine.state.players[1].mana_pool.blue = 2;
        let slot = hand_index_for_card(&engine, 1, "twincast");
        engine
            .apply_command(9, &cast_spell(slot, target_object(original)))
            .unwrap();
        pass_priority_round(&mut engine);
        let copy = engine.state.stack.last().unwrap();
        assert!(copy.is_copy && copy.controller == 9);
        // Copying a legendary spell is not casting it: P9 has no qualifying permanent.
        pass_priority_round(&mut engine);
        finish_order(&mut engine);
        assert_eq!(engine.state.objects[&copy_return].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&copy_return].controller, 9);
        assert_eq!(engine.state.objects[&original_return].zone, Zone::Graveyard);
        let fresh = inject_graveyard_card(&mut engine, 0, "boseiju,_who_endures");
        pass_priority_round(&mut engine);
        finish_order(&mut engine);
        assert_eq!(
            engine.state.objects[&original_return].zone,
            Zone::Battlefield
        );
        assert_eq!(
            engine.state.objects[&fresh].zone,
            if card == "primevals_glorious_rebirth" {
                Zone::Battlefield
            } else {
                Zone::Graveyard
            }
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn triumphant_reckoning_returned_kaito_applies_intrinsic_loyalty_before_animation() {
    let mut engine = setup("triumphant_reckoning");
    let kaito = inject_graveyard_card(&mut engine, 0, "kaito,_bane_of_nightmares");
    cast_and_resolve(&mut engine, "triumphant_reckoning");
    finish_order(&mut engine);
    assert_eq!(engine.state.objects[&kaito].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&kaito].counter_count(tricerules_cards::CounterKind::Loyalty),
        4
    );
    let characteristics = engine.characteristics(kaito).unwrap();
    assert!(characteristics.is_creature());
    assert!(!characteristics.has_type("Planeswalker"));
    assert!(characteristics.has_keyword(tricerules_cards::Keyword::Hexproof));
}

fn finish_order(engine: &mut GameEngine) {
    for _ in 0..12 {
        let Some(pending) = engine.state.pending_resolution.as_ref() else {
            return;
        };
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::SimultaneousEntryOrder
        );
        let actor = pending.deciding_player;
        let ids = pending.presentation.candidates.clone();
        engine
            .apply_command(actor, &submit_resolution_choice(ids))
            .unwrap();
    }
    panic!("entry ordering did not finish");
}

#[test]
fn triumphant_reckoning_returns_all_three_types_once_and_only_own_current_graveyard() {
    let mut engine = setup("triumphant_reckoning");
    let eligible: Vec<_> = ["mind_stone", "pacifism", "jace_beleren", "nyxborn_courser"]
        .iter()
        .map(|card| inject_graveyard_card(&mut engine, 0, card))
        .collect();
    let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let excluded: Vec<_> = ["forest", "grizzly_bears", "divination"]
        .iter()
        .map(|card| inject_graveyard_card(&mut engine, 0, card))
        .collect();
    let opposing = inject_graveyard_card(&mut engine, 1, "mind_stone");
    let spell = cast_and_resolve(&mut engine, "triumphant_reckoning");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura choice");
    assert!(pending.presentation.candidates.contains(&host));
    assert!(eligible
        .iter()
        .all(|oid| engine.state.objects[oid].zone == Zone::Graveyard));
    engine
        .apply_command(4, &submit_resolution_choice(vec![host]))
        .unwrap();
    finish_order(&mut engine);
    for oid in eligible {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
        assert_eq!(engine.state.zone_change_generation[&oid], 1);
        assert!(!engine.state.objects[&oid].tapped);
        if engine.state.objects[&oid].card_id == "jace_beleren" {
            assert_eq!(
                engine.state.objects[&oid].counter_count(tricerules_cards::CounterKind::Loyalty),
                3
            );
        }
    }
    for oid in excluded.into_iter().chain([opposing, spell]) {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    }
    assert!(engine.state.stack.is_empty());
}

#[test]
fn triumphant_reckoning_skips_accepted_aura_if_recipient_owner_concedes_before_entry() {
    let mut engine = setup("triumphant_reckoning");
    let aura = inject_graveyard_card(&mut engine, 0, "pacifism");
    let artifact = inject_graveyard_card(&mut engine, 0, "mind_stone");
    let host = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&aura)
        .copied()
        .unwrap_or(0);
    let spell = cast_and_resolve(&mut engine, "triumphant_reckoning");
    engine
        .apply_command(4, &submit_resolution_choice(vec![host]))
        .unwrap();
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
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    engine
        .apply_command(9, &concede())
        .expect("recipient owner can concede during entry ordering");
    finish_order(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&aura)
            .copied()
            .unwrap_or(0),
        original_generation,
        "Aura must skip entry rather than enter illegally and die to SBA"
    );
    assert_eq!(engine.state.objects[&aura].attached_to, None);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.zone_change_generation[&artifact], 1);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
