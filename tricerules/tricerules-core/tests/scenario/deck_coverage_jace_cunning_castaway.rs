//! Actual-card scenarios for Jace, Cunning Castaway.

use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::Color;
use tricerules_core::{state::CopiableValues, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ruled_event::Ev, DevCommand, DevMoveCard, DevZone,
    RuledCommand, TargetRef,
};

const JACE_ID: &str = "jace,_cunning_castaway";
const ILLUSION_ID: &str = "illusion_u_2_2_spell_target_sacrifice";

fn game(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("create Jace scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn enter_declare_attackers(engine: &mut GameEngine) {
    let active_player = engine.state.active_player_id();
    engine
        .apply_scenario_command(active_player, &primitive_yield())
        .expect("main phase passes to beginning of combat");
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn three_player_game(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(vec![
            deck_with("island", &[]),
            deck_with("island", &[]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("create three-player Jace scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn advance_to_step(engine: &mut GameEngine, wanted: TurnStep) {
    for _ in 0..120 {
        if engine.state.turn_step == wanted {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("game did not reach {wanted:?}");
}

fn jace_trigger_occurrences(engine: &GameEngine, sources: &[u32]) -> Vec<(u32, Option<i32>)> {
    let is_source = |source: u32| sources.contains(&source);
    let mut occurrences = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered)
        .filter_map(|item| {
            item.source_permanent_id
                .filter(|source| is_source(*source))
                .map(|source| (source, item.trigger_context.affected_player))
        })
        .collect::<Vec<_>>();
    if let Some(order) = &engine.state.pending_trigger_order {
        occurrences.extend(order.candidates.iter().filter_map(|trigger| {
            is_source(trigger.source_permanent_id).then_some((
                trigger.source_permanent_id,
                trigger.trigger_context.affected_player,
            ))
        }));
    }
    occurrences.extend(
        engine
            .state
            .staged_trigger_groups
            .iter()
            .flat_map(|group| &group.triggers)
            .filter_map(|trigger| {
                is_source(trigger.source_permanent_id).then_some((
                    trigger.source_permanent_id,
                    trigger.trigger_context.affected_player,
                ))
            }),
    );
    occurrences.sort_unstable();
    occurrences
}

fn resolve_jace_discard(engine: &mut GameEngine, controller: i32) -> u32 {
    pass_priority_round(engine);
    let (prompt_id, discard) = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Jace's delayed trigger asks its controller to discard");
        assert_eq!(pending.deciding_player, controller);
        assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
        assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
        (
            pending.resolution_prompt_id.get(),
            pending.presentation.candidates[0],
        )
    };
    let mut answer = submit_resolution_choice(vec![discard]);
    if let Some(Cmd::SubmitResolutionChoice(choice)) = answer.cmd.as_mut() {
        choice.resolution_prompt_id = prompt_id;
    }
    engine
        .apply_scenario_command(controller, &answer)
        .expect("submit one card from Jace's current discard offer");
    discard
}

fn put_jace(engine: &mut GameEngine, loyalty: u32) -> u32 {
    let jace = inject_permanent_on_battlefield(engine, 0, JACE_ID);
    engine
        .state
        .objects
        .get_mut(&jace)
        .expect("Jace object")
        .set_counter(CounterKind::Loyalty, loyalty);
    engine.initial_response_batch();
    jace
}

fn ability_index(ability_id: &str) -> u32 {
    tricerules_cards::registry::global()
        .get(JACE_ID)
        .expect("Jace is registered")
        .primary_face()
        .activated_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == ability_id)
        .unwrap_or_else(|| panic!("Jace ability {ability_id} is present")) as u32
}

fn move_jace_to_battlefield(engine: &mut GameEngine) {
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Jace, Cunning Castaway".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("return the same physical Jace to the battlefield");
}

fn create_illusion(engine: &mut GameEngine) -> u32 {
    let jace = put_jace(engine, 3);
    apply_ability(engine, 0, jace, ability_index("activated_02"), vec![])
        .expect("activate Jace -2 to create the actual Illusion");
    pass_both_players(engine);
    engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == ILLUSION_ID)
        .expect("Jace created the registered Illusion token")
}

#[test]
fn jace_cunning_castaway_has_exact_registered_identity() {
    let registry = tricerules_cards::registry::global();
    let jace = registry.get(JACE_ID);
    assert!(
        jace.is_some(),
        "exact Jace, Cunning Castaway definition is not admitted"
    );
    let jace = jace.expect("checked above");

    assert!(!registry.is_token(JACE_ID));
    assert_eq!(jace.id, JACE_ID);
    assert_eq!(jace.name, "Jace, Cunning Castaway");
}

#[test]
fn jace_minus_five_at_five_creates_two_fresh_nonlegendary_copies() {
    let mut engine = game(202_610_291);
    let source = put_jace(&mut engine, 5);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );

    apply_ability(
        &mut engine,
        0,
        source,
        ability_index("activated_03"),
        vec![],
    )
    .expect("activate -5 at exactly five loyalty");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        0,
        "the zero-loyalty state-based action happens before ability resolution"
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the -5 ability remains on stack"
    );
    let source_departure_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    assert_ne!(source_generation, source_departure_generation);

    let creation_batch = pass_both_players_with_batch(&mut engine);
    assert!(engine.state.stack.is_empty(), "the -5 ability resolved");

    let mut created_ids = creation_batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::TokenCreated(token)) => {
                assert_eq!(token.card_id, JACE_ID);
                assert_eq!(token.controller_player_id, 0);
                assert_eq!(
                    token.identity.as_ref().expect("public token identity").name,
                    "Jace, Cunning Castaway"
                );
                Some(token.object_id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    created_ids.sort_unstable();

    let copies = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| {
            *object_id != source
                && engine.state.objects[object_id].card_id == JACE_ID
                && engine.state.objects[object_id].token_origin.is_some()
        })
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 2, "-5 creates exactly two copies");
    let mut copy_ids = copies.clone();
    copy_ids.sort_unstable();
    assert_eq!(
        created_ids, copy_ids,
        "each copy has matching TokenCreated metadata"
    );
    assert!(
        engine.state.pending_resolution.is_none(),
        "no legend choice is offered"
    );

    let mut identities = copies
        .iter()
        .map(|object_id| {
            let object = &engine.state.objects[object_id];
            let generation = engine
                .state
                .zone_change_generation
                .get(object_id)
                .copied()
                .unwrap_or(0);
            assert_eq!(object.card_id, JACE_ID);
            assert_eq!(object.owner, 0);
            assert_eq!(object.controller, 0);
            assert_eq!(object.zone, Zone::Battlefield);
            let values = object
                .token_origin
                .as_ref()
                .expect("copy token-origin metadata is present");
            assert_eq!(values.source_card_id, JACE_ID);
            assert_eq!(values.source_face_index, 0);
            assert_eq!(values.display_name, "Jace, Cunning Castaway");
            assert_eq!(values.face.name, "Jace, Cunning Castaway");
            assert_eq!(values.face.loyalty, Some(3));

            let characteristics = engine
                .characteristics(*object_id)
                .expect("copy characteristics");
            assert!(characteristics.has_name("Jace, Cunning Castaway"));
            assert!(characteristics.has_type("Planeswalker"));
            assert!(characteristics.has_type("Jace"));
            assert!(!characteristics.is_legendary());
            assert_eq!(characteristics.colors, [Color::Blue]);
            assert_eq!(characteristics.power, None);
            assert_eq!(characteristics.toughness, None);
            assert_eq!(
                object.counter_count(CounterKind::Loyalty),
                3,
                "each copy enters with the printed loyalty"
            );
            (*object_id, generation)
        })
        .collect::<Vec<_>>();
    identities.sort_unstable();
    assert_ne!(
        identities[0].0, identities[1].0,
        "copies have distinct ObjectIds"
    );
    assert_ne!(
        identities[0], identities[1],
        "copy ObjectId/generation pairs are distinct"
    );
    assert!(identities.iter().all(|identity| identity.0 != source));
    assert!(identities
        .iter()
        .all(|identity| *identity != (source, source_generation)));

    // Each nonlegendary copy has its own current-generation loyalty-use record.
    for copy in copies {
        apply_ability(&mut engine, 0, copy, ability_index("activated_01"), vec![])
            .expect("each distinct Jace copy can activate +1 this turn");
        pass_both_players(&mut engine);
    }
}

#[test]
fn jace_minus_five_uses_original_incarnation_lki_after_same_object_reentry() {
    let mut engine = game(202_610_292);
    let source = put_jace(&mut engine, 5);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);

    apply_ability(
        &mut engine,
        0,
        source,
        ability_index("activated_03"),
        vec![],
    )
    .expect("activate -5 at exactly five loyalty");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the -5 ability remains on stack"
    );
    let departure_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    assert_ne!(source_generation, departure_generation);

    move_jace_to_battlefield(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&source].card_id, JACE_ID);
    let reentered_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    assert_ne!(source_generation, reentered_generation);

    // Make the newer incarnation observably different in copyable values. If the resolving
    // ability looks up the current ObjectId without its recorded generation, its output changes.
    let mut reentered_face = tricerules_cards::registry::global()
        .get(JACE_ID)
        .expect("Jace definition")
        .primary_face()
        .clone();
    reentered_face.name = "Jace, New Incarnation".into();
    reentered_face.loyalty = Some(1);
    let display_name = reentered_face.name.clone();
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: JACE_ID.into(),
        source_face_index: 0,
        display_name,
        face: reentered_face,
        room_faces: None,
    });
    assert!(engine
        .characteristics(source)
        .unwrap()
        .has_name("Jace, New Incarnation"));

    let creation_batch = pass_both_players_with_batch(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "the original -5 ability resolved"
    );
    let created = creation_batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::TokenCreated(token)) => Some(token),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        created.len(),
        2,
        "the old ability creates exactly two tokens"
    );

    let mut created_pairs = created
        .iter()
        .map(|token| {
            assert_eq!(token.card_id, JACE_ID);
            assert_eq!(token.controller_player_id, 0);
            assert_eq!(
                token.identity.as_ref().expect("public token identity").name,
                "Jace, Cunning Castaway"
            );
            assert_ne!(token.object_id, source);
            let object = &engine.state.objects[&token.object_id];
            let origin = object
                .token_origin
                .as_ref()
                .expect("copy token retains frozen source face metadata");
            assert_eq!(origin.source_card_id, JACE_ID);
            assert_eq!(origin.face.name, "Jace, Cunning Castaway");
            assert_eq!(origin.face.loyalty, Some(3));
            assert_eq!(object.owner, 0);
            assert_eq!(object.controller, 0);
            assert_eq!(object.zone, Zone::Battlefield);
            assert_eq!(
                object.counter_count(CounterKind::Loyalty),
                3,
                "copy uses the original generation's loyalty"
            );
            assert!(engine
                .characteristics(token.object_id)
                .unwrap()
                .has_name("Jace, Cunning Castaway"));
            let generation = engine
                .state
                .zone_change_generation
                .get(&token.object_id)
                .copied()
                .unwrap_or(0);
            (token.object_id, generation)
        })
        .collect::<Vec<_>>();
    created_pairs.sort_unstable();
    assert_ne!(created_pairs[0].0, created_pairs[1].0);
    assert_ne!(created_pairs[0], created_pairs[1]);
    assert!(created_pairs.iter().all(|pair| pair.0 != source));
    assert!(created_pairs
        .iter()
        .all(|pair| *pair != (source, source_generation)));
    assert_ne!(
        (source, source_generation),
        (source, reentered_generation),
        "the same physical ObjectId now names a different incarnation"
    );
}

#[test]
fn jace_plus_one_groups_combatants_and_resolves_a_private_draw_discard() {
    let mut engine = game(202_610_297);
    let jace = put_jace(&mut engine, 3);
    apply_ability(&mut engine, 0, jace, ability_index("activated_01"), vec![])
        .expect("activate Jace's actual +1");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        4
    );

    inject_card_into_hand(&mut engine, 1, "heros_downfall");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &pass())
        .expect("yield priority so player 1 can answer");
    let downfall = hand_index_for_card(&engine, 1, "heros_downfall");
    engine
        .apply_scenario_command(1, &cast_spell(downfall, target_object(jace)))
        .expect("cast Hero's Downfall at Jace");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&jace].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.active_event_observers.len(),
        1,
        "the +1 observer is independent of the Jace that created it"
    );

    let first_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    enter_declare_attackers(&mut engine);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![first_attacker, second_attacker]))
        .expect("declare both creatures against player 1");
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 16,
        "both bears deal combat damage"
    );

    let jace_triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.is_triggered && item.source_permanent_id == Some(jace))
        .count();
    assert_eq!(
        jace_triggers, 1,
        "one committed combat-damage batch with two creatures hitting one player creates one occurrence"
    );

    let library_before = engine.state.players[0].library.len();
    let hand_before = engine.state.players[0].hand.clone();
    let choice_batch = pass_both_players_with_batch(&mut engine);
    let choice = find_resolution_choice(&choice_batch)
        .expect("the Jace trigger publishes the controller's discard offer");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the trigger draws, then parks the controller's discard choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
    assert!(pending.resolution_prompt_id.get() != 0);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(choice.public_reveal.is_none());
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert_eq!(engine.state.players[0].hand.len(), hand_before.len() + 1);
    let drawn = engine.state.players[0]
        .hand
        .iter()
        .find(|object_id| !hand_before.contains(object_id))
        .copied()
        .expect("the actual trigger's draw");
    assert!(choice.candidate_object_ids.contains(&drawn));

    let prompt_id = pending.resolution_prompt_id.get();
    let discard = choice.candidate_object_ids[0];
    let mut answer = submit_resolution_choice(vec![discard]);
    if let Some(Cmd::SubmitResolutionChoice(choice)) = answer.cmd.as_mut() {
        choice.resolution_prompt_id = prompt_id;
    }
    engine
        .apply_scenario_command(0, &answer)
        .expect("submit one card from the current private offer");
    assert!(engine.state.players[0].graveyard.contains(&discard));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.active_event_observers.len(), 1);

    for _ in 0..40 {
        if engine.state.active_player_id() != 0 {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert_ne!(engine.state.active_player_id(), 0);
    assert!(
        engine.state.active_event_observers.is_empty(),
        "the actual +1 observer expires at cleanup"
    );
}

#[test]
fn jace_plus_one_repeats_per_damaged_player_across_first_and_regular_damage_steps() {
    let mut engine = three_player_game(202_610_298);
    let jace = put_jace(&mut engine, 3);
    apply_ability(&mut engine, 0, jace, ability_index("activated_01"), vec![])
        .expect("activate Jace's actual +1");
    pass_priority_round(&mut engine);

    let first_striker = inject_creature_on_battlefield(&mut engine, 0, "black_knight");
    let first_player_bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_player_bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let third_player_bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    enter_declare_attackers(&mut engine);

    let legal = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .clone();
    let assignments = [
        (first_striker, 1),
        (first_player_bear, 1),
        (second_player_bear, 1),
        (third_player_bear, 2),
    ]
    .into_iter()
    .map(|(attacker, defender)| {
        legal
            .iter()
            .find(|row| row.attacker_object_id == attacker && row.defending_player_id == defender)
            .cloned()
            .expect("current legal assignment for this attacker and player")
    })
    .collect();
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(
                    tricerules_proto::ruled::v1::DeclareAttackers { assignments },
                )),
            },
        )
        .expect("split the attack between the two nonactive players");

    advance_to_step(&mut engine, TurnStep::FirstStrikeDamage);
    assert_eq!(
        jace_trigger_occurrences(&engine, &[jace]),
        vec![(jace, None)],
        "first-strike damage is one complete slice; Jace's body needs no damaged-player input"
    );
    let library_before = engine.state.players[0].library.len();
    resolve_jace_discard(&mut engine, 0);
    assert_eq!(engine.state.players[0].library.len(), library_before - 1);
    assert_eq!(engine.state.active_event_observers.len(), 1);

    advance_to_step(&mut engine, TurnStep::CombatDamage);
    assert_eq!(
        jace_trigger_occurrences(&engine, &[jace]),
        vec![(jace, None), (jace, None)],
        "three regular attackers hit both players, so this slice creates one occurrence per player"
    );
    resolve_jace_discard(&mut engine, 0);
    resolve_jace_discard(&mut engine, 0);
    assert_eq!(engine.state.players[0].library.len(), library_before - 3);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.active_event_observers.len(), 1);
}

#[test]
fn jace_plus_one_on_two_created_copies_keeps_activation_observers_independent() {
    let mut engine = game(202_610_299);
    let source = put_jace(&mut engine, 5);
    apply_ability(
        &mut engine,
        0,
        source,
        ability_index("activated_03"),
        vec![],
    )
    .expect("activate Jace's actual -5");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);

    let mut copies = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object_id| {
            *object_id != source
                && engine.state.objects[object_id].card_id == JACE_ID
                && engine.state.objects[object_id].token_origin.is_some()
        })
        .collect::<Vec<_>>();
    copies.sort_unstable();
    assert_eq!(copies.len(), 2);

    for &copy in &copies {
        apply_ability(&mut engine, 0, copy, ability_index("activated_01"), vec![])
            .expect("each new Jace copy has its own current +1 ability");
        pass_both_players(&mut engine);
        assert_eq!(
            engine.state.objects[&copy].counter_count(CounterKind::Loyalty),
            4
        );
    }
    assert_eq!(
        engine.state.active_event_observers.len(),
        2,
        "separate +1 activations install separate observers"
    );

    let first_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    enter_declare_attackers(&mut engine);
    engine
        .apply_scenario_command(0, &declare_attackers(vec![first_attacker, second_attacker]))
        .expect("declare both creatures against player 1");
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[1].life, 16);

    let expected = copies.iter().map(|copy| (*copy, None)).collect::<Vec<_>>();
    assert_eq!(
        jace_trigger_occurrences(&engine, &copies),
        expected,
        "one same-player damage slice produces one trigger for each independent activation"
    );
    let library_before = engine.state.players[0].library.len();
    resolve_jace_discard(&mut engine, 0);
    resolve_jace_discard(&mut engine, 0);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn jace_illusion_spell_target_sacrifices_it_and_sole_target_spell_has_no_effect() {
    let mut engine = game(202_610_293);
    let illusion = create_illusion(&mut engine);
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(illusion)))
        .expect("cast an ordinary spell at the Illusion");
    let spell_object = engine
        .state
        .stack
        .iter()
        .find(|item| !item.is_triggered)
        .expect("Unsummon on stack below its trigger")
        .id;
    assert_eq!(engine.state.stack.len(), 2);
    assert!(
        engine.state.stack.last().is_some_and(|item| {
            item.is_triggered && item.source_permanent_id == Some(illusion)
        }),
        "spell targeting puts the Illusion's trigger above the spell"
    );

    pass_both_players(&mut engine);
    assert!(!engine.state.objects.contains_key(&illusion));
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack.last().unwrap().id, spell_object);

    // The token was Unsummon's only target. It has ceased to exist after leaving the battlefield;
    // Unsummon has no effect because its only target is gone.
    pass_both_players(&mut engine);
    assert!(!engine.state.objects.contains_key(&illusion));
    assert_eq!(engine.state.objects[&spell_object].zone, Zone::Graveyard);
    assert!(!engine.state.players[0].hand.contains(&illusion));
}

#[test]
fn jace_illusion_ability_target_does_not_sacrifice_it() {
    let mut engine = game(202_610_294);
    let illusion = create_illusion(&mut engine);
    let sorcerer = inject_creature_on_battlefield(&mut engine, 0, "prodigal_sorcerer");
    let sorcerer_ability = tricerules_cards::registry::global()
        .get("prodigal_sorcerer")
        .expect("Prodigal Sorcerer is registered")
        .primary_face()
        .activated_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == "activated_01")
        .expect("Prodigal Sorcerer targeting ability is present") as u32;
    apply_ability(
        &mut engine,
        0,
        sorcerer,
        sorcerer_ability,
        target_object(illusion),
    )
    .expect("target the Illusion with an activated ability");
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_trigger_order.is_none());

    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&illusion].zone, Zone::Battlefield);
}

#[test]
fn jace_illusion_spell_copy_target_sacrifices_token_before_copy_fails_to_resolve() {
    let mut engine = game(202_610_295);
    let illusion = create_illusion(&mut engine);
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_scenario_command(0, &cast_spell(bolt, target_player(1)))
        .expect("cast the original Bolt at player 1");
    let bolt_object = engine.state.stack.last().expect("Bolt on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("yield to the Twincast player");

    inject_card_into_hand(&mut engine, 1, "twincast");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast = hand_index_for_card(&engine, 1, "twincast");
    engine
        .apply_scenario_command(
            1,
            &cast_spell(
                twincast,
                vec![TargetRef {
                    object_id: bolt_object,
                    expected_zone_change_generation: None,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Twincast at Bolt");
    engine
        .apply_scenario_command(1, &pass())
        .expect("Twincast player passes");
    engine
        .apply_scenario_command(0, &pass())
        .expect("resolve Twincast to a target choice");
    assert!(engine.state.pending_resolution.is_some());

    let copy_target_batch = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![illusion]))
        .expect("retarget the copied Bolt to the actual Illusion");
    assert!(copy_target_batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackPushed(stack_object)) if stack_object.is_copy
    )));
    assert!(
        engine.state.stack.last().is_some_and(|item| {
            item.is_triggered && item.source_permanent_id == Some(illusion)
        }),
        "SpellCopy target selection puts the Illusion's trigger above the copy"
    );

    pass_both_players(&mut engine);
    assert!(!engine.state.objects.contains_key(&illusion));
    assert_eq!(engine.state.stack.len(), 2, "copy and original Bolt remain");
    assert!(engine.state.stack.last().unwrap().is_copy);
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 20,
        "the copy has no legal target"
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 17,
        "only the original Bolt resolves"
    );
}

#[test]
fn jace_illusion_multi_target_spell_keeps_its_other_legal_target() {
    let mut engine = game(202_610_296);
    let illusion = create_illusion(&mut engine);
    let bears = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "frost_breath");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            u: 1,
            ..Default::default()
        },
    );
    let frost_breath = hand_index_for_card(&engine, 0, "frost_breath");
    let mut targets = target_object(illusion);
    targets.extend(target_object(bears));
    engine
        .apply_scenario_command(0, &cast_spell(frost_breath, targets))
        .expect("cast Frost Breath at Illusion and Grizzly Bears");
    pass_both_players(&mut engine);
    assert!(!engine.state.objects.contains_key(&illusion));
    pass_both_players(&mut engine);
    assert!(engine.state.objects[&bears].tapped);
    assert!(!engine.state.objects.contains_key(&illusion));
}
