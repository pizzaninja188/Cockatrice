//! Actual Myr Battlesphere: four tokens and a resolution-time optional Myr cohort.
use super::helpers::*;
use tricerules_cards::{
    ContinuousEffectKind, ControllerReference, EffectDuration, Keyword, PermanentTypeFilter,
    TypeLineReplacement,
};
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

fn cast_myr() -> (GameEngine, u32, Vec<u32>) {
    cast_myr_players(&[0, 1])
}

fn cast_myr_players(players: &[i32]) -> (GameEngine, u32, Vec<u32>) {
    let mut decks = vec![deck_with("forest", &["myr_battlesphere"])];
    decks.extend(players.iter().skip(1).map(|_| deck_with("forest", &[])));
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        513_001,
        players,
        20,
        Some(decks),
        true,
    )
    .expect("complete Myr Battlesphere registration");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "myr_battlesphere");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "myr_battlesphere");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "real ETB trigger");
    pass_priority_round(&mut engine);
    let source = battlefield_object_for_card(&engine, 0, "myr_battlesphere");
    let tokens = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|oid| engine.state.objects[oid].card_id == "myr_token_c_1_1")
        .collect();
    (engine, source, tokens)
}

fn pending_attack() -> (GameEngine, u32, Vec<u32>) {
    let (mut engine, source, tokens) = cast_myr();
    declare_myr_attack(&mut engine, source, TargetRefKind::Player, 1);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    (engine, source, tokens)
}

fn declare_myr_attack(engine: &mut GameEngine, source: u32, kind: TargetRefKind, defender: u32) {
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;
    engine.apply_command(0, &primitive_yield()).unwrap();
    pass_priority_round(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    let assignment = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == source
                && assignment.defender.as_ref().is_some_and(|target| {
                    target.kind == kind as i32 && target.object_id == defender
                })
        })
        .cloned()
        .expect("engine-published actual Myr attack");
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(rv1::DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .unwrap();
}

fn grant(engine: &mut GameEngine, oid: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn move_source(engine: &mut GameEngine, zone: rv1::DevZone) {
    engine.enable_dev_commands();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                    target_player_id: 0,
                    dev: Some(rv1::dev_command::Dev::MoveCard(rv1::DevMoveCard {
                        card_name: "Myr Battlesphere".into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap();
}

#[test]
fn myr_battlesphere_actual_cast_creates_four_exact_rules_named_tokens() {
    let (engine, source, tokens) = cast_myr();
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(tokens.len(), 4);
    for oid in tokens {
        let object = &engine.state.objects[&oid];
        assert!(object.token_origin.is_some() && object.summoning_sick && !object.tapped);
        let values = engine.characteristics(oid).unwrap();
        assert_eq!(values.names, ["Myr Token"]);
        assert_eq!(values.types, ["Artifact", "Creature", "Myr"]);
        assert_eq!((values.power, values.toughness), (Some(1), Some(1)));
        assert!(values.colors.is_empty() && values.keywords.is_empty());
        assert_eq!(values.controller, 0);
    }
}

#[test]
fn myr_battlesphere_partial_selection_taps_only_chosen_and_counts_once() {
    let (mut engine, source, tokens) = pending_attack();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!((pending.presentation.min, pending.presentation.max), (0, 4));
    assert_eq!(pending.presentation.candidates, tokens);
    engine
        .apply_command(0, &submit_resolution_choice(tokens[..2].to_vec()))
        .unwrap();
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
    assert_eq!(engine.characteristics(source).unwrap().power, Some(6));
    assert_eq!(engine.state.players[1].life, 18);
    assert_eq!(engine.state.players[0].life, 20);
    for (index, token) in tokens.iter().enumerate() {
        assert_eq!(engine.state.objects[token].tapped, index < 2);
    }
}

#[test]
fn myr_battlesphere_zero_selection_declines_tap_bonus_and_damage() {
    let (mut engine, source, tokens) = pending_attack();
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert_eq!(engine.characteristics(source).unwrap().power, Some(4));
    assert_eq!(engine.state.players[1].life, 20);
    assert!(tokens.iter().all(|id| !engine.state.objects[id].tapped));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn myr_battlesphere_prevention_parks_after_one_tap_and_bonus() {
    let (mut engine, source, tokens) = pending_attack();
    engine.state.add_damage_prevention_shield(1, 1);
    engine.state.add_damage_prevention_shield(1, 1);
    engine
        .apply_command(0, &submit_resolution_choice(tokens.clone()))
        .unwrap();
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.deciding_player, 1);
    assert_eq!(engine.characteristics(source).unwrap().power, Some(8));
    assert_eq!(engine.state.players[1].life, 20, "damage has not committed");
    let choice = pending.presentation.candidates[0];
    engine
        .apply_command(1, &submit_resolution_choice(vec![choice]))
        .unwrap();
    assert_eq!(
        engine.characteristics(source).unwrap().power,
        Some(8),
        "bonus never doubled"
    );
    assert_eq!(
        engine.state.players[1].life, 18,
        "fixed X=4 minus two shields"
    );
    assert!(tokens.iter().all(|id| engine.state.objects[id].tapped));
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}

#[test]
fn myr_battlesphere_invalid_and_stale_cohorts_reject_without_mutation() {
    for case in 0..8 {
        let (mut engine, source, tokens) = pending_attack();
        let chosen = match case {
            0 => vec![tokens[0]], // wrong player
            1 => vec![tokens[0], tokens[0]],
            2 => vec![u32::MAX],
            3 => vec![source], // source is tapped and was not offered
            _ => vec![tokens[0], tokens[1]],
        };
        match case {
            4 => engine.state.objects.get_mut(&tokens[1]).unwrap().tapped = true,
            5 => {
                *engine
                    .state
                    .zone_change_generation
                    .entry(tokens[1])
                    .or_default() += 1
            }
            6 => grant(
                &mut engine,
                tokens[1],
                ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(1),
                },
            ),
            7 => grant(
                &mut engine,
                tokens[1],
                ContinuousEffectKind::Layer4SetCreatureTypes(vec!["Golem".into()]),
            ),
            _ => {}
        }
        let before = engine.diagnostic_snapshot().unwrap();
        let index = engine.state.command_index;
        assert!(
            engine
                .apply_command(
                    if case == 0 { 1 } else { 0 },
                    &submit_resolution_choice(chosen)
                )
                .is_err(),
            "case {case}"
        );
        assert_eq!(
            engine.diagnostic_snapshot().unwrap(),
            before,
            "atomic rejection case {case}"
        );
        assert_eq!(engine.state.command_index, index);
        assert!(
            !engine.state.objects[&tokens[0]].tapped,
            "valid first member never partially pays"
        );
        assert!(engine.state.pending_resolution.is_some());
    }
}

#[test]
fn myr_battlesphere_eligible_scope_includes_nontoken_sick_and_noncreature_myr() {
    let (mut engine, source, tokens) = cast_myr();
    let non_token = inject_creature_on_battlefield(&mut engine, 0, "darksteel_myr");
    engine
        .state
        .objects
        .get_mut(&non_token)
        .unwrap()
        .summoning_sick = true;
    let noncreature = tokens[0];
    // Preserve a Myr subtype while changing card types through a copiable snapshot fixture.
    // A type-setting effect would erase creature subtypes; this models an already legal
    // Kindred Artifact Myr occurrence rather than inventing a forbidden type effect.
    let mut copy = engine.state.objects[&noncreature]
        .token_origin
        .clone()
        .unwrap();
    copy.face.types.retain(|kind| kind != "Creature");
    copy.face.types.insert(0, "Kindred".into());
    engine
        .state
        .objects
        .get_mut(&noncreature)
        .unwrap()
        .copiable_values = Some(copy);
    let tapped = tokens[1];
    engine.state.objects.get_mut(&tapped).unwrap().tapped = true;
    let enemy = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    let other = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    declare_myr_attack(&mut engine, source, TargetRefKind::Player, 1);
    pass_priority_round(&mut engine);
    let candidates = &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates;
    assert_eq!(candidates.len(), 4);
    assert!(candidates.contains(&non_token) && candidates.contains(&noncreature));
    assert!(
        !candidates.contains(&tapped)
            && !candidates.contains(&enemy)
            && !candidates.contains(&other)
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![non_token, noncreature]))
        .unwrap();
    assert!(engine.state.objects[&non_token].tapped && engine.state.objects[&noncreature].tapped);
    assert_eq!(engine.characteristics(source).unwrap().power, Some(6));
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn myr_battlesphere_no_untapped_myr_skips_choice_and_vigilant_source_is_eligible() {
    for vigilant in [false, true] {
        let (mut engine, source, tokens) = cast_myr();
        for token in &tokens {
            engine.state.objects.get_mut(token).unwrap().tapped = true;
        }
        if vigilant {
            grant(
                &mut engine,
                source,
                ContinuousEffectKind::Layer6AddKeyword(Keyword::Vigilance),
            );
        }
        declare_myr_attack(&mut engine, source, TargetRefKind::Player, 1);
        pass_priority_round(&mut engine);
        if vigilant {
            let pending = engine.state.pending_resolution.as_ref().unwrap();
            assert_eq!(pending.presentation.candidates, vec![source]);
            assert_eq!(pending.presentation.max, 1);
            engine
                .apply_command(0, &submit_resolution_choice(vec![source]))
                .unwrap();
            assert!(engine.state.objects[&source].tapped);
            assert_eq!(engine.characteristics(source).unwrap().power, Some(5));
            assert_eq!(engine.state.players[1].life, 19);
        } else {
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(engine.characteristics(source).unwrap().power, Some(4));
            assert_eq!(engine.state.players[1].life, 20);
        }
    }
}

#[test]
fn myr_battlesphere_absent_or_returned_source_still_taps_and_damages_without_bonus() {
    for returned in [false, true] {
        let (mut engine, source, tokens) = cast_myr();
        declare_myr_attack(&mut engine, source, TargetRefKind::Player, 1);
        move_source(&mut engine, rv1::DevZone::Exile);
        if returned {
            move_source(&mut engine, rv1::DevZone::Battlefield);
            pass_priority_round(&mut engine); // returned source's new ETB trigger
        }
        pass_priority_round(&mut engine);
        engine
            .apply_command(0, &submit_resolution_choice(tokens[..2].to_vec()))
            .unwrap();
        assert_eq!(engine.state.players[1].life, 18);
        assert!(tokens[..2].iter().all(|id| engine.state.objects[id].tapped));
        if returned {
            assert_eq!(engine.characteristics(source).unwrap().power, Some(4));
        } else {
            assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
        }
    }
}

#[test]
fn myr_battlesphere_noncreature_source_retains_bonus_for_same_incarnation_until_cleanup() {
    let (mut engine, source, tokens) = cast_myr();
    declare_myr_attack(&mut engine, source, TargetRefKind::Player, 1);
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
    );
    pass_priority_round(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(tokens[..2].to_vec()))
        .unwrap();
    assert!(!engine.characteristics(source).unwrap().is_creature());
    engine
        .state
        .continuous_effects
        .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
    assert_eq!(engine.characteristics(source).unwrap().power, Some(6));
    assert_eq!(engine.state.players[1].life, 18);
    engine.state.turn_step = TurnStep::EndStep;
    pass_priority_round(&mut engine);
    resolve_cleanup_discards_if_any(&mut engine);
    assert_eq!(engine.characteristics(source).unwrap().power, Some(4));
}

#[test]
fn myr_battlesphere_frozen_chooser_and_current_damage_source_controller_are_distinct() {
    let (mut engine, source, tokens) = cast_myr_players(&[0, 1, 2]);
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
    );
    declare_myr_attack(&mut engine, source, TargetRefKind::Player, 1);
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(2),
        },
    );
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        0
    );
    engine
        .apply_command(0, &submit_resolution_choice(tokens[..2].to_vec()))
        .unwrap();
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 18);
    assert_eq!(engine.state.players[2].life, 22);
    assert_eq!(engine.characteristics(source).unwrap().power, Some(6));
}

#[test]
fn myr_battlesphere_departed_damage_recipient_drains_parked_batch_without_repeating_bonus() {
    let (mut engine, source, tokens) = cast_myr_players(&[0, 1, 2]);
    declare_myr_attack(&mut engine, source, TargetRefKind::Player, 1);
    pass_priority_round(&mut engine);
    engine.state.add_damage_prevention_shield(1, 1);
    engine.state.add_damage_prevention_shield(1, 1);
    engine
        .apply_command(0, &submit_resolution_choice(tokens.clone()))
        .unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    assert_eq!(engine.characteristics(source).unwrap().power, Some(8));
    engine.apply_command(1, &concede()).unwrap();
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
    assert_eq!(engine.characteristics(source).unwrap().power, Some(8));
    assert!(tokens.iter().all(|oid| engine.state.objects[oid].tapped));
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn myr_battlesphere_accepted_choice_and_damage_resumption_replay_identically() {
    let (mut engine, _, tokens) = pending_attack();
    let (mut replay, _, _) = pending_attack();
    for participant in [&mut engine, &mut replay] {
        participant.state.add_damage_prevention_shield(1, 1);
        participant.state.add_damage_prevention_shield(1, 1);
    }
    let command = submit_resolution_choice(tokens);
    assert_eq!(
        engine.apply_command(0, &command).unwrap(),
        replay.apply_command(0, &command).unwrap()
    );
    let chosen = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    let command = submit_resolution_choice(vec![chosen]);
    assert_eq!(
        engine.apply_command(1, &command).unwrap(),
        replay.apply_command(1, &command).unwrap()
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn myr_battlesphere_original_planeswalker_generation_and_battle_recipient_are_preserved() {
    for case in 0..4 {
        let (mut engine, source, tokens) = cast_myr();
        let card = if case == 3 {
            "invasion_of_ulgrotha_grandmother_ravi_sengir"
        } else {
            "jace_beleren"
        };
        let defender = inject_permanent_on_battlefield(&mut engine, 1, card);
        let counter = if case == 3 {
            tricerules_cards::CounterKind::Defense
        } else {
            tricerules_cards::CounterKind::Loyalty
        };
        engine
            .state
            .objects
            .get_mut(&defender)
            .unwrap()
            .set_counter(counter, 5);
        if case == 3 {
            engine.state.battle_protectors.insert(defender, 1);
        }
        declare_myr_attack(&mut engine, source, TargetRefKind::Permanent, defender);
        if case == 1 {
            *engine
                .state
                .zone_change_generation
                .entry(defender)
                .or_default() += 2;
        } else if case == 2 {
            engine.state.players[1]
                .battlefield
                .retain(|&oid| oid != defender);
            engine.state.players[1].exile.push(defender);
            engine.state.objects.get_mut(&defender).unwrap().zone = Zone::Exile;
            *engine
                .state
                .zone_change_generation
                .entry(defender)
                .or_default() += 1;
        }
        pass_priority_round(&mut engine);
        engine
            .apply_command(0, &submit_resolution_choice(tokens[..2].to_vec()))
            .unwrap();
        assert_eq!(engine.characteristics(source).unwrap().power, Some(6));
        assert_eq!(
            engine.state.players[1].life, 20,
            "never redirects to defending player"
        );
        assert_eq!(
            engine.state.objects[&defender].counter_count(counter),
            if case == 0 { 3 } else { 5 }
        );
    }
}

#[test]
fn myr_battlesphere_surviving_foreign_trigger_retains_damage_after_source_owner_departure() {
    let (mut engine, _, tokens) = cast_myr_players(&[0, 1, 2]);
    let source = inject_creature_under_foreign_control(&mut engine, 2, 0, "myr_battlesphere");
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
    );
    declare_myr_attack(&mut engine, source, TargetRefKind::Player, 1);
    engine.apply_command(2, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&source));
    pass_priority_round(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(tokens[..2].to_vec()))
        .unwrap();
    assert_eq!(engine.state.players[0].life, 22);
    assert_eq!(engine.state.players[1].life, 18);
    assert!(tokens[..2]
        .iter()
        .all(|oid| engine.state.objects[oid].tapped));
}

#[test]
fn myr_battlesphere_parked_damage_retains_departed_source_and_current_prevention_choice() {
    let (mut engine, _, tokens) = cast_myr_players(&[0, 1, 2]);
    let source = inject_creature_under_foreign_control(&mut engine, 2, 0, "myr_battlesphere");
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
    );
    declare_myr_attack(&mut engine, source, TargetRefKind::Player, 1);
    pass_priority_round(&mut engine);
    engine.state.add_damage_prevention_shield(1, 1);
    engine.state.add_damage_prevention_shield(1, 1);
    engine
        .apply_command(0, &submit_resolution_choice(tokens.clone()))
        .unwrap();
    let choices = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .clone();
    engine.apply_command(2, &concede()).unwrap();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        choices,
        "unchanged prevention choice preserves published opaque identity"
    );
    engine
        .apply_command(1, &submit_resolution_choice(vec![choices[0]]))
        .unwrap();
    assert_eq!(engine.state.players[1].life, 18);
    assert_eq!(engine.state.players[0].life, 22);
    assert!(tokens.iter().all(|oid| engine.state.objects[oid].tapped));
    assert!(engine.state.pending_resolution.is_none() && engine.state.stack.is_empty());
}
