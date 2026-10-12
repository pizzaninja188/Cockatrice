//! Skitterbeam Battalion's Prototype cast choice and cast-gated token copies.
use super::helpers::*;
use tricerules_cards::{CardRegistry, Color};
use tricerules_core::{state::CopiableValues, GameEngine, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    dev_command, DevCommand, DevMoveCard, DevZone, TargetRef, TargetRefKind,
};

const SKITTERBEAM: &str = "skitterbeam_battalion";

fn game(seed: u64) -> GameEngine {
    let deck = deck_with("mountain", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_and_resolve(prototyped: bool, seed: u64) -> (GameEngine, u32) {
    let mut engine = game(seed);
    let source = inject_card_into_hand(&mut engine, 0, SKITTERBEAM);
    give_mana(
        &mut engine,
        0,
        if prototyped {
            ManaGift {
                r: 2,
                c: 3,
                ..Default::default()
            }
        } else {
            ManaGift {
                c: 9,
                ..Default::default()
            }
        },
    );

    let slot = hand_index_for_card(&engine, 0, SKITTERBEAM);
    let response = engine.initial_response_batch();
    let cast_actions = response.legal_by_player[&0]
        .hand_actions
        .iter()
        .filter(|action| {
            action.hand_index == slot as u32
                && action.kind
                    == tricerules_proto::ruled::v1::HandActionKind::HandActionCastSpell as i32
        })
        .collect::<Vec<_>>();
    assert!(cast_actions.iter().any(|action| !action.prototyped));
    assert!(cast_actions.iter().any(|action| action.prototyped));
    assert_eq!(
        cast_actions
            .iter()
            .find(|action| action.prototyped)
            .unwrap()
            .cost,
        "{3}{R}{R}"
    );

    let mut command = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!("cast helper creates a CastSpell command")
    };
    cast.prototyped = prototyped;
    engine
        .apply_command(0, &command)
        .expect("cast the selected Skitterbeam mode");

    pass_priority_round(&mut engine); // resolve the creature and put its ETB trigger on the stack
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    pass_priority_round(&mut engine); // resolve the cast-gated ETB trigger
    assert!(engine.state.stack.is_empty());
    (engine, source)
}

#[test]
fn skitterbeam_prototype_keeps_selected_characteristics_on_both_token_copies() {
    for (prototyped, seed, expected_mana_value, expected_pt, expected_colors, expected_cost) in [
        (false, 31_120_001, 9, (4, 4), vec![], "{9}"),
        (true, 31_120_002, 5, (2, 2), vec![Color::Red], "{3}{R}{R}"),
    ] {
        let (engine, source) = cast_and_resolve(prototyped, seed);
        let objects = engine.state.players[0]
            .battlefield
            .iter()
            .copied()
            .filter(|object_id| engine.state.objects[object_id].card_id == SKITTERBEAM)
            .collect::<Vec<_>>();
        assert_eq!(objects.len(), 3, "the cast source plus two copies");

        for object_id in objects {
            let object = &engine.state.objects[&object_id];
            let characteristics = engine.characteristics(object_id).unwrap();
            assert_eq!(characteristics.mana_value, expected_mana_value);
            assert_eq!(
                (characteristics.power, characteristics.toughness),
                (Some(expected_pt.0), Some(expected_pt.1))
            );
            assert_eq!(characteristics.colors, expected_colors);
            assert_eq!(object.prototyped, prototyped);
            if object_id == source {
                assert!(!object.is_token());
            } else {
                assert!(object.is_token());
                let copied_values = object.token_origin.as_ref().unwrap();
                assert_eq!(copied_values.prototyped, prototyped);
                assert_eq!(copied_values.face.mana_cost.to_string(), expected_cost);
            }
        }
    }
}

#[test]
fn skitterbeam_prototype_copy_views_follow_effective_copiable_marker() {
    let (mut engine, source) = cast_and_resolve(true, 31_120_005);
    let copied_token = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|&object_id| object_id != source && engine.state.objects[&object_id].is_token())
        .expect("Prototype token copy");
    let copied_values = engine.state.objects[&copied_token]
        .token_origin
        .clone()
        .expect("token copy retains copiable Prototype values");
    let source_definition = tricerules_cards::registry::global()
        .get(SKITTERBEAM)
        .unwrap();
    let normal_values = CopiableValues {
        source_card_id: SKITTERBEAM.into(),
        source_face_index: 0,
        display_name: source_definition.name.clone(),
        face: source_definition.primary_face().clone(),
        prototyped: false,
        room_faces: None,
    };

    engine
        .state
        .objects
        .get_mut(&copied_token)
        .unwrap()
        .prototyped = false;
    engine
        .state
        .objects
        .get_mut(&copied_token)
        .unwrap()
        .copiable_values = Some(copied_values);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(normal_values);

    let batch = engine.initial_response_batch();
    let zone_view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("initial battlefield replacement");
    let view_for = |object_id| {
        zone_view
            .per_player
            .iter()
            .flat_map(|player| &player.battlefield_objects)
            .find(|object| object.object_id == object_id)
            .unwrap_or_else(|| panic!("battlefield object {object_id} is published"))
    };
    assert!(
        view_for(copied_token).prototyped,
        "a copy layer with Prototype copiable values keeps the client marker"
    );
    assert!(
        !view_for(source).prototyped,
        "a normal copy layer masks the physical card's stale Prototype marker"
    );
}

#[test]
fn skitterbeam_entering_without_being_cast_does_not_create_tokens() {
    let mut engine = game(31_120_003);
    let source = inject_card_into_hand(&mut engine, 0, SKITTERBEAM);
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Skitterbeam Battalion".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the card to the battlefield without casting it");

    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].battlefield.len(), 1);
}

#[test]
fn copied_prototype_creature_spell_resolves_as_a_prototyped_token_without_cast_triggers() {
    const COPY_ANY_SPELL: &str = r#"(
        id: "prototype_copy_any_spell_fixture",
        name: "Prototype Copy Any Spell Fixture",
        face_id: "prototype_copy_any_spell_fixture",
        mana_cost: "{1}{U}",
        types: ["Instant"],
        spell_effect: [CopyTargetSpell(count: 1, spell_filter: (card_type: None))],
    )"#;
    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/mountain.ron"),
                include_str!("../../../tricerules-cards/data/skitterbeam_battalion.ron"),
                COPY_ANY_SPELL,
            ],
            &[],
        )
        .expect("isolated Prototype spell-copy registry"),
    ));
    let deck = vec!["mountain".into(); 40];
    let mut engine = GameEngine::new(
        registry,
        31_120_004,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new isolated-registry game");
    advance_to_main1_from_game_start(&mut engine);

    let source = inject_card_into_hand(&mut engine, 0, SKITTERBEAM);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            c: 3,
            ..Default::default()
        },
    );
    let source_slot = hand_index_for_card(&engine, 0, SKITTERBEAM);
    let mut cast_source = cast_spell(source_slot, vec![]);
    let Some(Cmd::CastSpell(cast)) = cast_source.cmd.as_mut() else {
        unreachable!()
    };
    cast.prototyped = true;
    let cast_batch = engine
        .apply_command(0, &cast_source)
        .expect("cast Skitterbeam as a prototyped spell");
    let source_push = cast_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackPushed(pushed))
                if pushed.object_id == source =>
            {
                Some(pushed)
            }
            _ => None,
        })
        .expect("public stack event for the Prototype spell");
    assert!(source_push.prototyped);

    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    let copier = inject_card_into_hand(&mut engine, 1, "prototype_copy_any_spell_fixture");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let copier_slot = hand_index_for_card(&engine, 1, "prototype_copy_any_spell_fixture");
    engine
        .apply_command(
            1,
            &cast_spell(
                copier_slot,
                vec![TargetRef {
                    object_id: source,
                    kind: TargetRefKind::Stack as i32,
                    ..Default::default()
                }],
            ),
        )
        .expect("cast the fixture spell targeting the Prototype spell");

    let mut copy_batch = Default::default();
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        copy_batch = engine
            .apply_command(actor, &pass())
            .expect("pass to resolve the fixture copy spell");
    }
    let copy_push = copy_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::StackPushed(pushed))
                if pushed.is_copy =>
            {
                Some(pushed)
            }
            _ => None,
        })
        .expect("copy of the prototyped spell is pushed");
    assert!(copy_push.prototyped);

    pass_priority_round(&mut engine); // the spell copy becomes its own token
    assert_eq!(
        engine.state.stack.len(),
        1,
        "only the original spell remains"
    );
    assert_eq!(
        engine.state.players[1]
            .battlefield
            .iter()
            .filter(|object_id| engine.state.objects[object_id].is_token())
            .count(),
        1,
        "a copied spell was not cast, so its copied ETB ability creates no extra tokens"
    );

    pass_priority_round(&mut engine); // the original spell enters and triggers
    pass_priority_round(&mut engine); // the original's cast-gated trigger creates two copies
    let tokens = engine
        .state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter(|object_id| engine.state.objects[object_id].is_token())
        .collect::<Vec<_>>();
    assert_eq!(tokens.len(), 3, "one spell copy and two ETB copies");
    for object_id in tokens {
        let object = &engine.state.objects[&object_id];
        let characteristics = engine.characteristics(object_id).unwrap();
        assert!(object.prototyped);
        assert_eq!(characteristics.mana_value, 5);
        assert_eq!(characteristics.colors, vec![Color::Red]);
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(2), Some(2))
        );
    }
    assert_eq!(engine.state.objects[&copier].zone, Zone::Graveyard);
}
