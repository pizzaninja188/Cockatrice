//! Actual registered Chaos Warp, plus explicitly private authoring-only tail compositions.
use crate::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::{state::CopiableValues, EngineDeck, Zone};

// Private composition only; this fixture is not Chaos Warp's Oracle definition or runtime data.
#[cfg(feature = "authoring")]
const CHAOS_TAIL_FIXTURE: &str = r#"(
  id: "chaos_warp_tail_fixture", name: "Chaos Warp Tail Fixture", face_id: "chaos_warp_tail_fixture",
  mana_cost: "{2}{R}", types: ["Instant"],
  spell_effect: [Draw(count: 1, who: Controller), ChaosWarp, Draw(count: 2, who: Controller)],
  targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose target permanent", effect_indices: [1])])),
)"#;

fn engine(seed: u64) -> GameEngine {
    engine_with_decks(
        seed,
        vec![
            EngineDeck {
                mainboard: vec!["mountain".into(); 12],
                commanders: vec![]
            };
            3
        ],
    )
}

fn engine_with_decks(seed: u64, decks: Vec<EngineDeck>) -> GameEngine {
    let mut engine =
        GameEngine::new_with_commander_decks(seed, &[0, 4, 9], 20, Some(decks), true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[cfg(feature = "authoring")]
fn engine_with_draft(seed: u64, decks: Vec<EngineDeck>, draft: &str) -> GameEngine {
    let registry = Box::leak(Box::new(CardRegistry::from_chunks_and_tokens(&[
        draft,
        include_str!("../../../tricerules-cards/data/mountain.ron"),
        include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
        include_str!("../../../tricerules-cards/data/divination.ron"),
        include_str!("../../../tricerules-cards/data/island.ron"),
        include_str!("../../../tricerules-cards/data/kami_of_the_crescent_moon.ron"),
        include_str!("../../../tricerules-cards/data/pacifism.ron"),
        include_str!("../../../tricerules-cards/data/clever_impersonator.ron"),
        include_str!("../../../tricerules-cards/data/orb_of_dreams.ron"),
        include_str!("../../../tricerules-cards/data/invasion_of_ulgrotha_grandmother_ravi_sengir.ron"),
    ], &[]).unwrap()));
    let mut engine =
        GameEngine::new_for_authoring(seed, &[0, 4, 9], 20, Some(decks), true, registry).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn declared_commander_engine(seed: u64) -> (GameEngine, u32) {
    prepare_stolen_commander(engine_with_decks(seed, commander_decks()))
}

#[cfg(feature = "authoring")]
fn declared_commander_engine_with_draft(seed: u64, draft: &str) -> (GameEngine, u32) {
    prepare_stolen_commander(engine_with_draft(seed, commander_decks(), draft))
}

fn commander_decks() -> Vec<EngineDeck> {
    let mut decks = vec![
        EngineDeck {
            mainboard: vec!["mountain".into(); 12],
            commanders: vec![]
        };
        3
    ];
    decks[1] = EngineDeck {
        mainboard: vec!["island".into(); 12],
        commanders: vec!["kami_of_the_crescent_moon".into()],
    };
    decks
}

fn prepare_stolen_commander(mut engine: GameEngine) -> (GameEngine, u32) {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    engine.enable_dev_commands();
    let target = engine.state.players[1].command_zone[0];
    engine
        .apply_command(
            4,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 4,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Kami of the Crescent Moon".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != target);
    engine.state.players[2].battlefield.push(target);
    engine.state.objects.get_mut(&target).unwrap().controller = 9;
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .base_controller = 9;
    engine.state.players[1].library.clear();
    (engine, target)
}

fn token_with_owed_return(engine: &mut GameEngine, returning_card: &str) -> (u32, u32) {
    use tricerules_core::state::{
        ActiveEventObserver, EventObserverMatcher, EventObserverPayload, TriggerObjectRef,
    };
    let source = inject_permanent_on_battlefield(engine, 1, "grizzly_bears");
    let definition = CardRegistry::global().get("grizzly_bears").unwrap();
    engine.state.objects.get_mut(&source).unwrap().token_origin = Some(CopiableValues {
        source_card_id: definition.id.clone(),
        source_face_index: 0,
        face: definition.primary_face().clone(),
        room_faces: None,
        display_name: definition.name.clone(),
    });
    let returning = inject_library_card(engine, 0, returning_card);
    engine.state.players[0]
        .library
        .retain(|oid| *oid != returning);
    engine.state.players[0].exile.push(returning);
    engine.state.objects.get_mut(&returning).unwrap().zone = Zone::Exile;
    // Narrow private CR 610.3 fixture: the accepted paid Chaos command exercises its departure.
    engine
        .state
        .active_event_observers
        .push(ActiveEventObserver {
            watched: Some(TriggerObjectRef {
                object_id: source,
                zone_change_generation: 0,
                controller_at_event: 4,
            }),
            matcher: EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
            payload: EventObserverPayload::ReturnExiledObject {
                exiled: TriggerObjectRef {
                    object_id: returning,
                    zone_change_generation: 0,
                    controller_at_event: 0,
                },
            },
        });
    engine.state.players[1].library.clear();
    (source, returning)
}

fn owner_token(engine: &mut GameEngine) -> u32 {
    let target = inject_permanent_on_battlefield(engine, 1, "grizzly_bears");
    let definition = CardRegistry::global().get("grizzly_bears").unwrap();
    engine.state.objects.get_mut(&target).unwrap().token_origin = Some(CopiableValues {
        source_card_id: definition.id.clone(),
        source_face_index: 0,
        face: definition.primary_face().clone(),
        room_faces: None,
        display_name: definition.name.clone(),
    });
    engine.state.players[1].library.clear();
    target
}

fn aura_choice_engine(seed: u64) -> (GameEngine, u32, u32, u32, u32, Vec<RuledEventBatch>) {
    let mut engine = engine(seed);
    let host = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    let target = owner_token(&mut engine);
    let aura = inject_library_card(&mut engine, 1, "pacifism");
    let (spell, batches) = cast_and_resolve(&mut engine, target);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        4
    );
    (engine, target, aura, host, spell, batches)
}

fn assert_atomic_rejection(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn paid_chaos_warp_copied_aura_without_surviving_recipient_restores_original_library_card_values() {
    let mut engine = engine(800_450);
    let host = inject_permanent_on_battlefield(&mut engine, 2, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::state::AttachmentRecipient::Object(host));
    let target = owner_token(&mut engine);
    let copier = inject_library_card(&mut engine, 1, "clever_impersonator");
    let (spell, _) = cast_and_resolve(&mut engine, target);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        tricerules_core::custom::ChoiceKind::CopySource
    );
    engine
        .apply_command(4, &submit_resolution_choice(vec![aura]))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        [host]
    );
    assert_eq!(engine.state.objects[&copier].copy_revision, 1);
    engine.apply_command(9, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&copier].zone, Zone::Library);
    assert_eq!(engine.state.players[1].library.front(), Some(&copier));
    assert_eq!(engine.state.objects[&copier].copy_revision, 0);
    assert!(engine.state.objects[&copier].copiable_values.is_none());
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&copier)
            .copied()
            .unwrap_or(0),
        0
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn paid_chaos_warp_copied_aura_owner_departure_is_accepted_without_resurrection() {
    let mut engine = engine(800_451);
    let host = inject_permanent_on_battlefield(&mut engine, 2, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::state::AttachmentRecipient::Object(host));
    let target = owner_token(&mut engine);
    let copier = inject_library_card(&mut engine, 1, "clever_impersonator");
    let (spell, _) = cast_and_resolve(&mut engine, target);
    engine
        .apply_command(4, &submit_resolution_choice(vec![aura]))
        .unwrap();
    assert_eq!(engine.state.objects[&copier].copy_revision, 1);
    let completed = engine.apply_command(4, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.objects.contains_key(&copier));
    assert!(!engine.state.objects.contains_key(&target));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        completed
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == spell))
            .count(),
        1
    );
}

#[test]
#[cfg(feature = "authoring")]
fn chaos_warp_private_composition_copied_aura_owner_departure_resumes_original_tail_once() {
    let decks = vec![
        EngineDeck {
            mainboard: vec!["mountain".into(); 12],
            commanders: vec![]
        };
        3
    ];
    let mut engine = engine_with_draft(800_452, decks, CHAOS_TAIL_FIXTURE);
    let host = inject_permanent_on_battlefield(&mut engine, 2, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(tricerules_core::state::AttachmentRecipient::Object(host));
    let target = owner_token(&mut engine);
    let copier = inject_library_card(&mut engine, 1, "clever_impersonator");
    let hand_before = engine.state.players[0].hand.len();
    let (spell, _) = cast_named_and_resolve(&mut engine, "chaos_warp_tail_fixture", target);
    engine
        .apply_command(4, &submit_resolution_choice(vec![aura]))
        .unwrap();
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    engine.apply_command(4, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.objects.contains_key(&copier));
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn paid_chaos_warp_permanent_foreign_control_departure_keeps_exiled_commander_and_finishes_owner_instructions(
) {
    let (mut engine, target) = declared_commander_engine(800_440);
    let top = inject_library_card(&mut engine, 1, "divination");
    let generation = engine.state.zone_change_generation[&target];
    let (spell, mut batches) = cast_and_resolve(&mut engine, target);
    assert_eq!(engine.state.objects[&target].base_controller, 9);
    batches.push(engine.apply_command(9, &concede()).unwrap());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.zone_change_generation[&target], generation + 1);
    assert_eq!(engine.state.players[1].library.front(), Some(&top));
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(batches.iter().flat_map(|batch| &batch.events).filter(|event| matches!(&event.ev,
        Some(Ev::CardsRevealed(reveal)) if reveal.cards.len() == 1 && reveal.cards[0].object_id == top)).count(), 1);
    assert_eq!(
        batches
            .iter()
            .flat_map(|batch| &batch.events)
            .filter(|event| matches!(&event.ev,
        Some(Ev::Log(log)) if log.text == "P4 shuffles their library."))
            .count(),
        1
    );
}

#[test]
#[cfg(feature = "authoring")]
fn chaos_warp_private_composition_resumes_tail_once_after_foreign_controlled_commander_is_exiled_on_departure(
) {
    let (mut engine, target) = declared_commander_engine_with_draft(800_441, CHAOS_TAIL_FIXTURE);
    inject_library_card(&mut engine, 1, "divination");
    let hand_before = engine.state.players[0].hand.len();
    let (spell, _) = cast_named_and_resolve(&mut engine, "chaos_warp_tail_fixture", target);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    let completed = engine.apply_command(9, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        completed
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == spell))
            .count(),
        1
    );
}

#[test]
fn paid_chaos_warp_attachment_recipient_departure_refreshes_survivors_or_skips_without_moving_aura()
{
    for surviving_host in [false, true] {
        let mut engine = engine(800_430 + u64::from(surviving_host));
        let host = inject_permanent_on_battlefield(&mut engine, 2, "grizzly_bears");
        let survivor = surviving_host
            .then(|| inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears"));
        let target = owner_token(&mut engine);
        let aura = inject_library_card(&mut engine, 1, "pacifism");
        let (spell, mut batches) = cast_and_resolve(&mut engine, target);
        assert!(engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates
            .contains(&host));
        batches.push(engine.apply_command(9, &concede()).unwrap());
        assert!(!engine.state.objects.contains_key(&host));
        if let Some(survivor) = survivor {
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(pending.deciding_player, 4);
            assert_eq!(pending.presentation.candidates, [survivor]);
            assert_atomic_rejection(&mut engine, 4, &submit_resolution_choice(vec![host]));
            batches.push(
                engine
                    .apply_command(4, &submit_resolution_choice(vec![survivor]))
                    .unwrap(),
            );
            assert_eq!(
                engine.state.objects[&aura].attached_to,
                Some(tricerules_core::state::AttachmentRecipient::Object(
                    survivor
                ))
            );
        } else {
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
            assert_eq!(engine.state.players[1].library.front(), Some(&aura));
            assert_eq!(
                engine
                    .state
                    .zone_change_generation
                    .get(&aura)
                    .copied()
                    .unwrap_or(0),
                0
            );
        }
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(!engine.state.objects.contains_key(&target));
        assert_eq!(
            batches
                .iter()
                .flat_map(|batch| &batch.events)
                .filter(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == spell))
                .count(),
            1
        );
    }
}

#[test]
#[cfg(feature = "authoring")]
fn chaos_warp_private_composition_resumes_tail_once_after_sole_attachment_recipient_departure() {
    let decks = vec![
        EngineDeck {
            mainboard: vec!["mountain".into(); 12],
            commanders: vec![]
        };
        3
    ];
    let mut engine = engine_with_draft(800_432, decks, CHAOS_TAIL_FIXTURE);
    inject_permanent_on_battlefield(&mut engine, 2, "grizzly_bears");
    let target = owner_token(&mut engine);
    let aura = inject_library_card(&mut engine, 1, "pacifism");
    let hand_before = engine.state.players[0].hand.len();
    let (spell, _) = cast_named_and_resolve(&mut engine, "chaos_warp_tail_fixture", target);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    let completed = engine.apply_command(9, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(
        completed
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == spell))
            .count(),
        1
    );
}

#[test]
fn paid_chaos_warp_requires_red_mana_and_exactly_one_battlefield_target() {
    let mut engine = engine(601_210);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    let spell = inject_card_into_hand(&mut engine, 0, "chaos_warp");
    let slot = hand_index_for_card(&engine, 0, "chaos_warp");
    engine.state.players[0].mana_pool.colorless = 3;
    assert_atomic_rejection(&mut engine, 0, &cast_spell(slot, target_object(target)));
    engine.state.players[0].mana_pool.colorless = 2;
    engine.state.players[0].mana_pool.red = 1;
    assert_atomic_rejection(&mut engine, 0, &cast_spell(slot, vec![]));
    assert_atomic_rejection(&mut engine, 0, &cast_spell(slot, target_object(spell)));
    let mut duplicate = target_object(target);
    duplicate.extend(target_object(target));
    assert_atomic_rejection(&mut engine, 0, &cast_spell(slot, duplicate));
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn paid_chaos_warp_twincast_copy_can_complete_commander_choice_without_a_physical_spell_card() {
    let (mut engine, target) = declared_commander_engine(707_110);
    let original = inject_card_into_hand(&mut engine, 0, "chaos_warp");
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = hand_index_for_card(&engine, 0, "chaos_warp");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    engine.apply_command(0, &pass()).unwrap();
    let twin = inject_card_into_hand(&mut engine, 1, "twincast");
    engine.state.players[1].mana_pool.blue = 2;
    let slot = hand_index_for_card(&engine, 1, "twincast");
    engine
        .apply_command(4, &cast_spell(slot, target_object(original)))
        .unwrap();
    for _ in 0..3 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(4, &submit_resolution_choice(vec![target]))
        .unwrap();
    let copy = engine.state.stack.last().unwrap().id;
    assert!(engine.state.stack.last().unwrap().is_copy);
    assert!(!engine.state.objects.contains_key(&copy));
    for _ in 0..3 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        4
    );
    engine.apply_command(4, &branch(0)).unwrap();
    assert_eq!(engine.state.objects[&target].zone, Zone::Command);
    assert_eq!(engine.state.objects[&twin].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&original].zone, Zone::Stack);
    assert!(!engine.state.stack.iter().any(|item| item.id == copy));
    for _ in 0..3 {
        engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    assert_eq!(engine.state.objects[&original].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn paid_chaos_warp_replacement_order_then_aura_attachment_preserves_one_public_receipt() {
    let mut engine = engine(616_110);
    let host = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
    for _ in 0..2 {
        inject_permanent_on_battlefield(&mut engine, 2, "orb_of_dreams");
    }
    let target = owner_token(&mut engine);
    let aura = inject_library_card(&mut engine, 1, "pacifism");
    let (spell, batches) = cast_and_resolve(&mut engine, target);
    let reveal = batches
        .iter()
        .flat_map(|batch| &batch.events)
        .find_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal.clone()),
            _ => None,
        })
        .unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 4);
    assert_eq!(
        pending.presentation.choice_kind,
        tricerules_core::custom::ChoiceKind::ReplacementEffect
    );
    let application = pending.presentation.candidates[0];
    assert_atomic_rejection(&mut engine, 0, &submit_resolution_choice(vec![application]));
    let generation = engine
        .state
        .zone_change_generation
        .get(&aura)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(aura, generation + 1);
    assert_atomic_rejection(&mut engine, 4, &submit_resolution_choice(vec![application]));
    engine.state.zone_change_generation.insert(aura, generation);
    engine
        .apply_command(4, &submit_resolution_choice(vec![application]))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        tricerules_core::custom::ChoiceKind::AuraPermanent
    );
    assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
    assert!(engine.state.objects.contains_key(&target));
    let snapshot = engine.initial_response_batch();
    let active = snapshot
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ActivePublicRevealSnapshot(active)) => Some(active),
            _ => None,
        })
        .unwrap();
    assert_eq!(active.reveals, [reveal]);
    engine
        .apply_command(4, &submit_resolution_choice(vec![host]))
        .unwrap();
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert!(engine.state.objects[&aura].tapped);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(!engine.state.objects.contains_key(&target));
}

#[test]
fn paid_chaos_warp_aura_attachment_is_not_targeting_but_obeys_protection() {
    use tricerules_cards::primitives::{Color, Keyword, ProtectionQuality};
    for keyword in [Keyword::Shroud, Keyword::Hexproof] {
        let mut engine = engine(303_410);
        let host = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
        let protected = inject_permanent_on_battlefield(&mut engine, 2, "grizzly_bears");
        let definition = CardRegistry::global().get("grizzly_bears").unwrap();
        let values = || CopiableValues {
            source_card_id: definition.id.clone(),
            source_face_index: 0,
            face: definition.primary_face().clone(),
            room_faces: None,
            display_name: definition.name.clone(),
        };
        let mut host_values = values();
        host_values.face.keywords.push(keyword);
        engine.state.objects.get_mut(&host).unwrap().copiable_values = Some(host_values);
        let mut protected_values = values();
        protected_values
            .face
            .protections
            .push(ProtectionQuality::Color(Color::White));
        engine
            .state
            .objects
            .get_mut(&protected)
            .unwrap()
            .copiable_values = Some(protected_values);
        let target = owner_token(&mut engine);
        let aura = inject_library_card(&mut engine, 1, "pacifism");
        cast_and_resolve(&mut engine, target);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            [host]
        );
        assert_atomic_rejection(&mut engine, 4, &submit_resolution_choice(vec![protected]));
        engine
            .apply_command(4, &submit_resolution_choice(vec![host]))
            .unwrap();
        assert_eq!(
            engine.state.objects[&aura].attached_to,
            Some(tricerules_core::state::AttachmentRecipient::Object(host))
        );
    }
}

#[test]
fn paid_chaos_warp_owner_choice_protobuf_commands_replay_identical_batches_and_identity() {
    use prost::Message;
    // Both runs start from the same explicit stolen-commander fixture. Only accepted commands
    // below are replayed; this does not claim that private setup is a native session journal.
    let create = || {
        let (mut engine, target) = declared_commander_engine(903_930);
        let spell = inject_card_into_hand(&mut engine, 0, "chaos_warp");
        engine.state.players[0].mana_pool.red = 1;
        engine.state.players[0].mana_pool.colorless = 2;
        (engine, target, spell)
    };
    for destination in 0..=1 {
        let (mut original, target, spell) = create();
        let slot = hand_index_for_card(&original, 0, "chaos_warp");
        let mut commands = vec![(0, cast_spell(slot, target_object(target)))];
        let mut batches = vec![original
            .apply_command(commands[0].0, &commands[0].1)
            .unwrap()];
        for _ in 0..3 {
            let actor = original.state.priority_player_id();
            let command = pass();
            batches.push(original.apply_command(actor, &command).unwrap());
            commands.push((actor, command));
        }
        let command = branch(destination);
        batches.push(original.apply_command(4, &command).unwrap());
        commands.push((4, command));
        let (mut replay, replay_target, replay_spell) = create();
        assert_eq!((target, spell), (replay_target, replay_spell));
        for ((actor, command), expected) in commands.iter().zip(batches) {
            let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
            assert_eq!(replay.apply_command(*actor, &decoded).unwrap(), expected);
        }
        assert_eq!(
            replay.diagnostic_snapshot().unwrap(),
            original.diagnostic_snapshot().unwrap()
        );
    }
}

#[test]
fn paid_chaos_warp_aura_choice_retains_public_reveal_and_guards_actual_top_generation_owner_and_recipient(
) {
    let (mut engine, target, aura, host, spell, batches) = aura_choice_engine(701_241);
    let reveal = batches
        .iter()
        .flat_map(|batch| &batch.events)
        .find_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal.clone()),
            _ => None,
        })
        .unwrap();
    assert!(!reveal.reveal_id.is_empty());
    assert_eq!(reveal.cards[0].object_id, aura);
    assert!(
        engine.state.objects.contains_key(&target),
        "token waits while resolution is parked"
    );
    // Raw front may be a deferred token. The physical deck and the completion guard use cards.
    engine.state.players[1].library.retain(|oid| *oid != target);
    engine.state.players[1].library.push_front(target);
    let snapshot = engine.initial_response_batch();
    let active = snapshot
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ActivePublicRevealSnapshot(active)) => Some(active),
            _ => None,
        })
        .unwrap();
    assert_eq!(active.reveals, [reveal]);
    let command = submit_resolution_choice(vec![host]);
    assert_atomic_rejection(&mut engine, 0, &command);
    assert_atomic_rejection(&mut engine, 4, &submit_resolution_choice(vec![aura]));
    let generation = engine
        .state
        .zone_change_generation
        .get(&aura)
        .copied()
        .unwrap_or(0);
    engine
        .state
        .zone_change_generation
        .insert(aura, generation + 1);
    assert_atomic_rejection(&mut engine, 4, &command);
    engine.state.zone_change_generation.insert(aura, generation);
    let original_library = engine.state.players[1].library.clone();
    let other = inject_library_card(&mut engine, 1, "divination");
    engine.state.players[1].library.retain(|oid| *oid != other);
    engine.state.players[1].library.push_front(other);
    assert_atomic_rejection(&mut engine, 4, &command);
    engine.state.players[1].library = original_library;
    engine.state.objects.get_mut(&aura).unwrap().owner = 9;
    assert_atomic_rejection(&mut engine, 4, &command);
    engine.state.objects.get_mut(&aura).unwrap().owner = 4;
    let completed = engine.apply_command(4, &command).unwrap();
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::state::AttachmentRecipient::Object(host))
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(!engine.state.objects.contains_key(&target));
    assert_eq!(
        completed
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == spell))
            .count(),
        1
    );
}

#[test]
fn paid_chaos_warp_no_recipient_aura_stays_top_and_token_only_library_reveals_nothing() {
    for aura_present in [true, false] {
        let mut engine = engine(701_242 + u64::from(aura_present));
        let target = owner_token(&mut engine);
        let aura = aura_present.then(|| inject_library_card(&mut engine, 1, "pacifism"));
        let (spell, batches) = cast_and_resolve(&mut engine, target);
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(!engine.state.objects.contains_key(&target));
        assert_eq!(
            batches
                .iter()
                .flat_map(|batch| &batch.events)
                .filter(|event| matches!(event.ev, Some(Ev::CardsRevealed(_))))
                .count(),
            usize::from(aura_present)
        );
        if let Some(aura) = aura {
            assert_eq!(
                engine.state.players[1]
                    .library
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                [aura]
            );
            assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
            assert_eq!(engine.state.objects[&aura].attached_to, None);
            assert!(!batches
                .iter()
                .flat_map(|batch| &batch.events)
                .any(|event| matches!(&event.ev,
                Some(Ev::PermanentMoved(moved)) if moved.object_id == aura)));
        } else {
            assert!(engine.state.players[1].library.is_empty());
        }
    }
}

#[test]
fn paid_chaos_warp_entry_owner_caster_and_unrelated_concessions_settle_without_resurrection() {
    for departed in [4, 0, 9] {
        let (mut engine, target, aura, host, spell, _) =
            aura_choice_engine(800_410 + departed as u64);
        engine.apply_command(departed, &concede()).unwrap();
        if departed == 4 {
            assert!(engine.state.pending_resolution.is_none());
            assert!(!engine.state.objects.contains_key(&aura));
            assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        } else if departed == 0 {
            assert!(engine.state.pending_resolution.is_none());
            assert!(!engine.state.objects.contains_key(&spell));
            assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
        } else {
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .deciding_player,
                4
            );
            engine
                .apply_command(4, &submit_resolution_choice(vec![host]))
                .unwrap();
            assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
            assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        }
        assert!(!engine.state.objects.contains_key(&target));
    }
}

#[test]
fn paid_chaos_warp_commander_owner_caster_and_unrelated_concessions_clear_only_relevant_work() {
    for departed in [4, 0, 9] {
        let (mut engine, target) = declared_commander_engine(800_420 + departed as u64);
        // Model actual stolen control: the base controller is its owner, and concession ends
        // the continuous control effect, returning control before the pending owner answer.
        engine
            .state
            .objects
            .get_mut(&target)
            .unwrap()
            .base_controller = 4;
        engine
            .state
            .continuous_effects
            .push(tricerules_core::ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: tricerules_core::AffectedScope::Single(target),
                kind: tricerules_cards::primitives::ContinuousEffectKind::Layer2Control {
                    controller: tricerules_cards::primitives::ControllerReference::Fixed(9),
                },
                condition: None,
                duration: tricerules_cards::primitives::EffectDuration::UntilEndOfTurn,
                timestamp: engine.state.command_index,
            });
        let (spell, _) = cast_and_resolve(&mut engine, target);
        engine.apply_command(departed, &concede()).unwrap();
        if departed == 4 {
            assert!(engine.state.pending_resolution.is_none());
            assert!(!engine.state.objects.contains_key(&target));
            assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        } else if departed == 0 {
            assert!(engine.state.pending_resolution.is_none());
            assert!(!engine.state.objects.contains_key(&spell));
            assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        } else {
            assert_eq!(
                engine
                    .state
                    .pending_resolution
                    .as_ref()
                    .unwrap()
                    .deciding_player,
                4
            );
            engine.apply_command(4, &branch(0)).unwrap();
            assert_eq!(engine.state.objects[&target].zone, Zone::Command);
            assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        }
    }
}

#[test]
fn paid_chaos_warp_battle_is_a_permanent_and_enters_front_face_with_owner_protector_choice() {
    let mut engine = engine(110_410);
    let target = owner_token(&mut engine);
    let battle = inject_library_card(
        &mut engine,
        1,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
    );
    let (spell, _) = cast_and_resolve(&mut engine, target);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        4
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        tricerules_core::custom::ChoiceKind::BattleProtector
    );
    engine
        .apply_command(4, &submit_resolution_choice(vec![9]))
        .unwrap();
    assert_eq!(engine.state.objects[&battle].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&battle].controller, 4);
    assert_eq!(engine.state.objects[&battle].face_up_index, 0);
    assert_eq!(engine.state.battle_protectors.get(&battle), Some(&9));
    assert_eq!(
        engine.state.objects[&battle]
            .counters
            .get(&tricerules_cards::primitives::CounterKind::Defense),
        Some(&5)
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn paid_chaos_warp_finishes_owed_creature_return_before_choosing_revealed_aura_recipient() {
    let mut engine = engine(610_310);
    let (target, returning) = token_with_owed_return(&mut engine, "grizzly_bears");
    let aura = inject_library_card(&mut engine, 1, "pacifism");
    let (spell, _) = cast_and_resolve(&mut engine, target);
    assert_eq!(engine.state.objects[&returning].zone, Zone::Battlefield);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("returned creature is an Aura recipient");
    assert_eq!(pending.deciding_player, 4);
    assert_eq!(pending.presentation.candidates, [returning]);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Library);
    engine
        .apply_command(4, &submit_resolution_choice(vec![returning]))
        .unwrap();
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::state::AttachmentRecipient::Object(
            returning
        ))
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(!engine.state.objects.contains_key(&target));
}

#[test]
fn paid_chaos_warp_preserves_owner_instruction_through_owed_aura_return_choice_without_early_reveal(
) {
    let mut engine = engine(610_311);
    let host = inject_permanent_on_battlefield(&mut engine, 2, "grizzly_bears");
    let (target, returning) = token_with_owed_return(&mut engine, "pacifism");
    let top_aura = inject_library_card(&mut engine, 1, "pacifism");
    let (spell, batches) = cast_and_resolve(&mut engine, target);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("owed Aura returns first");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.source_object_id, returning);
    assert!(!batches
        .iter()
        .flat_map(|batch| &batch.events)
        .any(|event| matches!(event.ev, Some(Ev::CardsRevealed(_)))));
    let resumed = engine
        .apply_command(0, &submit_resolution_choice(vec![host]))
        .unwrap();
    assert_eq!(engine.state.objects[&returning].zone, Zone::Battlefield);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Chaos owner instruction survives observer choice");
    assert_eq!(pending.deciding_player, 4);
    assert!(pending.presentation.candidates.contains(&host));
    assert_eq!(engine.state.objects[&top_aura].zone, Zone::Library);
    let reveal = resumed
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .unwrap();
    assert_eq!(reveal.cards[0].object_id, top_aura);
    assert!(!reveal.reveal_id.is_empty());
    engine
        .apply_command(4, &submit_resolution_choice(vec![host]))
        .unwrap();
    assert_eq!(engine.state.objects[&top_aura].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn paid_chaos_warp_declared_stolen_commander_owner_chooses_both_destinations_and_rejects_invalid_answers_atomically(
) {
    for destination in 0..=1 {
        let (mut engine, target) = declared_commander_engine(903_910 + destination as u64);
        let top = (destination == 0).then(|| inject_library_card(&mut engine, 1, "divination"));
        let generation = engine.state.zone_change_generation[&target];
        let (spell, mut batches) = cast_and_resolve(&mut engine, target);
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("owner commander choice");
        assert_eq!(pending.deciding_player, 4);
        assert_eq!(
            pending.presentation.choice_kind,
            tricerules_core::custom::ChoiceKind::ResolutionBranch
        );
        assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
        for (actor, answer) in [
            (9, branch(destination)),
            (0, branch(destination)),
            (4, branch(2)),
        ] {
            let before = serde_json::to_string(&engine.state).unwrap();
            assert!(engine.apply_command(actor, &answer).is_err());
            assert_eq!(serde_json::to_string(&engine.state).unwrap(), before);
        }
        let stack_generation = engine.state.zone_change_generation[&spell];
        engine
            .state
            .zone_change_generation
            .insert(spell, stack_generation + 1);
        assert_atomic_rejection(&mut engine, 4, &branch(destination));
        engine
            .state
            .zone_change_generation
            .insert(spell, stack_generation);
        let mut malformed = branch(destination);
        if let Some(Cmd::SubmitResolutionChoice(answer)) = &mut malformed.cmd {
            answer.chosen_object_ids.push(target);
        }
        assert_atomic_rejection(&mut engine, 4, &malformed);
        engine
            .state
            .zone_change_generation
            .insert(target, generation + 1);
        let before = serde_json::to_string(&engine.state).unwrap();
        assert!(engine.apply_command(4, &branch(destination)).is_err());
        assert_eq!(serde_json::to_string(&engine.state).unwrap(), before);
        engine
            .state
            .zone_change_generation
            .insert(target, generation);
        batches.push(engine.apply_command(4, &branch(destination)).unwrap());
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.players[1].declared_commander_object_ids,
            [target]
        );
        if destination == 0 {
            assert_eq!(engine.state.players[1].command_zone, [target]);
            assert_eq!(engine.state.objects[&target].zone, Zone::Command);
            assert_eq!(engine.state.zone_change_generation[&target], generation + 1);
            assert_eq!(
                engine.state.players[1]
                    .library
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                [top.unwrap()]
            );
            assert!(batches.iter().flat_map(|batch| &batch.events).any(|event| matches!(&event.ev,
                Some(Ev::PermanentMoved(moved)) if moved.object_id == target && moved.destination() == tricerules_proto::ruled::v1::permanent_moved::Destination::Command)));
        } else {
            assert!(engine.state.players[1].command_zone.is_empty());
            assert!(engine.state.players[1].library.is_empty());
            assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
            assert_eq!(engine.state.objects[&target].controller, 4);
            assert_eq!(engine.state.zone_change_generation[&target], generation + 2);
        }
        let reveals: Vec<_> = batches
            .iter()
            .flat_map(|batch| &batch.events)
            .filter_map(|event| match &event.ev {
                Some(Ev::CardsRevealed(reveal)) => Some(reveal),
                _ => None,
            })
            .collect();
        assert_eq!(reveals.len(), 1);
        assert_eq!(reveals[0].zone_owner_player_id, 4);
        assert_eq!(reveals[0].cards[0].object_id, top.unwrap_or(target));
        assert!(!reveals[0].reveal_id.is_empty());
    }
}

#[test]
fn paid_chaos_warp_ordinary_same_name_card_does_not_inherit_commander_designation() {
    let (mut engine, commander) = declared_commander_engine(903_920);
    let ordinary = inject_permanent_on_battlefield(&mut engine, 1, "kami_of_the_crescent_moon");
    let generation = engine
        .state
        .zone_change_generation
        .get(&ordinary)
        .copied()
        .unwrap_or(0);
    let (spell, _) = cast_and_resolve(&mut engine, ordinary);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&ordinary].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&ordinary],
        generation + 2
    );
    assert_eq!(engine.state.objects[&commander].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.players[1].declared_commander_object_ids,
        [commander]
    );
}

fn cast_and_resolve(engine: &mut GameEngine, target: u32) -> (u32, Vec<RuledEventBatch>) {
    cast_named_and_resolve(engine, "chaos_warp", target)
}

fn cast_named_and_resolve(
    engine: &mut GameEngine,
    card: &str,
    target: u32,
) -> (u32, Vec<RuledEventBatch>) {
    let spell = inject_card_into_hand(engine, 0, card);
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = hand_index_for_card(engine, 0, card);
    let mut batches = vec![engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap()];
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        batches.push(engine.apply_command(actor, &pass()).unwrap());
    }
    (spell, batches)
}

#[test]
fn paid_chaos_warp_illegal_sole_target_fizzles_without_shuffle_or_reveal() {
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};
    let mut engine = engine(608_210);
    engine.enable_dev_commands();
    let target = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    let spell = inject_card_into_hand(&mut engine, 0, "chaos_warp");
    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let slot = hand_index_for_card(&engine, 0, "chaos_warp");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .unwrap();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 4,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Grizzly Bears".into(),
                        zone: DevZone::Exile as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .unwrap();
    let library = engine.state.players[1].library.clone();
    let mut batches = vec![];
    for _ in 0..3 {
        batches.push(
            engine
                .apply_command(engine.state.priority_player_id(), &pass())
                .unwrap(),
        );
    }
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[1].library, library);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!batches
        .iter()
        .flat_map(|batch| &batch.events)
        .any(|event| matches!(event.ev, Some(Ev::CardsRevealed(_)))));
    assert!(!batches
        .iter()
        .flat_map(|batch| &batch.events)
        .any(|event| matches!(&event.ev,
        Some(Ev::Log(log)) if log.text.contains("shuffles their library"))));
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
#[cfg(feature = "authoring")]
fn chaos_warp_private_composition_resumes_original_tail_once_after_commander_answer_or_owner_departure(
) {
    for owner_leaves in [false, true] {
        let (mut engine, target) = declared_commander_engine_with_draft(
            608_220 + u64::from(owner_leaves),
            CHAOS_TAIL_FIXTURE,
        );
        let hand_before = engine.state.players[0].hand.len();
        let (spell, mut batches) =
            cast_named_and_resolve(&mut engine, "chaos_warp_tail_fixture", target);
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before + 1,
            "first instruction executes before owner choice"
        );
        let command = if owner_leaves { concede() } else { branch(0) };
        batches.push(engine.apply_command(4, &command).unwrap());
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before + 3,
            "saved tail executes exactly once"
        );
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert_eq!(
            batches
                .iter()
                .flat_map(|batch| &batch.events)
                .filter(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == spell))
                .count(),
            1
        );
    }
}

#[test]
#[cfg(feature = "authoring")]
fn chaos_warp_private_composition_resumes_tail_once_after_entry_choice_or_owner_departure() {
    for owner_leaves in [false, true] {
        let decks = vec![
            EngineDeck {
                mainboard: vec!["mountain".into(); 12],
                commanders: vec![]
            };
            3
        ];
        let mut engine =
            engine_with_draft(608_230 + u64::from(owner_leaves), decks, CHAOS_TAIL_FIXTURE);
        let host = inject_permanent_on_battlefield(&mut engine, 0, "grizzly_bears");
        let target = owner_token(&mut engine);
        let aura = inject_library_card(&mut engine, 1, "pacifism");
        let hand_before = engine.state.players[0].hand.len();
        let (spell, mut batches) =
            cast_named_and_resolve(&mut engine, "chaos_warp_tail_fixture", target);
        assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
        let command = if owner_leaves {
            concede()
        } else {
            submit_resolution_choice(vec![host])
        };
        batches.push(engine.apply_command(4, &command).unwrap());
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        if owner_leaves {
            assert!(!engine.state.objects.contains_key(&aura));
        } else {
            assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
        }
        assert_eq!(
            batches
                .iter()
                .flat_map(|batch| &batch.events)
                .filter(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == spell))
                .count(),
            1
        );
    }
}

#[test]
fn paid_chaos_warp_token_target_uses_owner_library_and_reveals_nonpermanent_without_moving_it() {
    let mut engine = engine(701_240);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    let definition = CardRegistry::global().get("grizzly_bears").unwrap();
    engine.state.objects.get_mut(&target).unwrap().token_origin = Some(CopiableValues {
        source_card_id: definition.id.clone(),
        source_face_index: 0,
        face: definition.primary_face().clone(),
        room_faces: None,
        display_name: definition.name.clone(),
    });
    // Stolen-control setup is private; the actual paid spell and priority passes are commands.
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != target);
    engine.state.players[2].battlefield.push(target);
    engine.state.objects.get_mut(&target).unwrap().controller = 9;
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .base_controller = 9;
    engine.state.players[1].library.clear();
    let top = inject_library_card(&mut engine, 1, "divination");
    let caster_library = engine.state.players[0].library.clone();
    let controller_library = engine.state.players[2].library.clone();
    let (spell, batches) = cast_and_resolve(&mut engine, target);
    assert!(
        !engine.state.objects.contains_key(&target),
        "token ceases only at completed resolution"
    );
    assert_eq!(
        engine.state.players[1]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        [top]
    );
    assert_eq!(engine.state.objects[&top].zone, Zone::Library);
    assert_eq!(engine.state.players[0].library, caster_library);
    assert_eq!(engine.state.players[2].library, controller_library);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    let reveals: Vec<_> = batches
        .iter()
        .flat_map(|batch| &batch.events)
        .filter_map(|event| match &event.ev {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .collect();
    assert_eq!(reveals.len(), 1);
    assert_eq!(reveals[0].zone_owner_player_id, 4);
    assert_eq!(reveals[0].cards.len(), 1);
    assert_eq!(reveals[0].cards[0].object_id, top);
    assert!(!reveals[0].reveal_id.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
