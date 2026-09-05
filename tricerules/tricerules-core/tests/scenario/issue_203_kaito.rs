use super::helpers::*;
use tricerules_cards::{
    primitives::CounterKind, CardRegistry, ContinuousEffectKind, EffectDuration,
    PermanentTypeFilter, TypeLineReplacement,
};
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine};
use tricerules_proto::ruled::v1::{
    cost_selection, AbilitySourceZone, ActivateAbility, ChoiceKind, CostObjectRef, CostObjectRefs,
    CostSelection, PreviewPayment, RuledCommand,
};

fn setup_unblocked_kaito(seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("island", &["kaito,_bane_of_nightmares", "grizzly_bears"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "kaito,_bane_of_nightmares");
    ensure_card_in_hand(&mut engine, 0, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    engine.apply_command(0, &pass()).expect("attacker passes");
    engine.apply_command(1, &pass()).expect("defender passes");
    let kaito =
        engine.state.players[0].hand[hand_index_for_card(&engine, 0, "kaito,_bane_of_nightmares")];
    (engine, attacker, kaito)
}

fn ninjutsu_command(engine: &GameEngine, kaito: u32, attacker: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: kaito,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&kaito)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            cost_selections: vec![CostSelection {
                cost_index: 1,
                selection: Some(cost_selection::Selection::BattlefieldObjects(
                    CostObjectRefs {
                        objects: vec![CostObjectRef {
                            object_id: attacker,
                            zone_change_generation: engine
                                .state
                                .zone_change_generation
                                .get(&attacker)
                                .copied()
                                .unwrap_or(0),
                        }],
                    },
                )),
            }],
            ..Default::default()
        })),
    }
}

#[test]
fn issue_203_kaito_is_supported() {
    assert!(
        CardRegistry::global()
            .get("kaito,_bane_of_nightmares")
            .is_some(),
        "Kaito must be present in the ruled registry"
    );
}

#[test]
fn kaito_battlefield_view_preserves_loyalty_indices_and_presentations() {
    let decks = Some(vec![
        deck_with("island", &["kaito,_bane_of_nightmares"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(203_012, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let kaito = move_ready_to_battlefield(&mut engine, 0, "kaito,_bane_of_nightmares");

    let batch = engine.initial_response_batch();
    let view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("zone view");
    let object = view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == kaito)
        .expect("Kaito battlefield object");

    assert_eq!(
        object
            .activated_abilities
            .iter()
            .map(|ability| ability.ability_index)
            .collect::<Vec<_>>(),
        vec![1, 2, 3],
        "the hand-only Ninjutsu slot must remain absent without shifting loyalty command indices"
    );
    assert_eq!(
        object
            .activated_abilities
            .iter()
            .map(|ability| {
                ability
                    .presentation
                    .as_ref()
                    .expect("ability presentation")
                    .oracle_line_indices
                    .clone()
            })
            .collect::<Vec<_>>(),
        vec![vec![3], vec![4], vec![5]]
    );
}

#[test]
fn animated_planeswalker_can_activate_loyalty_abilities() {
    let decks = Some(vec![
        deck_with("island", &["jace_beleren"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(203_001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let jace = deploy_to_battlefield(&mut engine, 0, "jace_beleren", false);
    engine
        .state
        .objects
        .get_mut(&jace)
        .expect("Jace")
        .set_counter(CounterKind::Loyalty, 4);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(jace),
        kind: ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec!["Ninja".into()],
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });

    assert!(
        zone_view_ability_flags(&mut engine, 0, jace)
            .into_iter()
            .any(|activatable| activatable),
        "losing the planeswalker type must not disable printed loyalty abilities"
    );
}

#[test]
fn kaito_ninjutsu_returns_an_exact_unblocked_attacker_and_reveals_the_source() {
    let (mut engine, attacker, kaito) = setup_unblocked_kaito(203_002);
    let generation = engine
        .state
        .zone_change_generation
        .get(&attacker)
        .copied()
        .unwrap_or(0);
    let legal = engine.initial_response_batch();
    let action = legal.legal_by_player[&0]
        .zone_ability_actions
        .iter()
        .find(|action| action.object_id == kaito)
        .expect("Kaito Ninjutsu action");
    assert!(action
        .ability
        .as_ref()
        .is_some_and(|ability| ability.activatable));
    let key = (u64::from(kaito) << 32) | u64::from(action.ability_index);
    let choice = &legal.legal_by_player[&0].cost_choices_by_ability[&key].choices[0];
    assert_eq!(
        choice.candidate_objects[0]
            .object
            .as_ref()
            .unwrap()
            .object_id,
        attacker
    );
    assert_eq!(
        choice.candidate_objects[0]
            .object
            .as_ref()
            .unwrap()
            .zone_change_generation,
        generation
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let batch = engine
        .apply_command(0, &ninjutsu_command(&engine, kaito, attacker))
        .expect("activate Ninjutsu");
    assert_eq!(
        engine.state.objects[&attacker].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(
        engine.state.objects[&kaito].zone,
        tricerules_core::Zone::Hand
    );
    assert!(batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::ActivePublicRevealSnapshot(snapshot))
                if snapshot.reveals.iter().any(|reveal| reveal.card_id == "kaito,_bane_of_nightmares")
        )
    }));
}

#[test]
fn kaito_ninjutsu_payment_preview_accepts_its_generation_bound_hand_source() {
    let (mut engine, attacker, kaito) = setup_unblocked_kaito(203_011);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let Some(Cmd::ActivateAbility(mut activation)) = ninjutsu_command(&engine, kaito, attacker).cmd
    else {
        panic!("Ninjutsu command must be an activation");
    };

    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id: 203,
            revision: 1,
            activate_ability: Some(activation.clone()),
            ..Default::default()
        },
    );

    assert!(preview.valid, "{}", preview.error);
    assert_eq!(preview.total_cost, "{1}{U}{B}");
    activation.payment = preview.selection;
    activation.payment.as_mut().unwrap().mana = Some(tricerules_proto::ruled::v1::PaymentMana {
        u: 1,
        b: 1,
        c: 1,
        ..Default::default()
    });
    let paid = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id: 203,
            revision: 2,
            activate_ability: Some(activation),
            ..Default::default()
        },
    );
    assert!(paid.valid, "{}", paid.error);
    assert!(
        paid.complete,
        "total {}, remaining {}, selection {:?}",
        paid.total_cost, paid.remaining_cost, paid.selection
    );
}

#[test]
fn kaito_ninjutsu_enters_tapped_attacking_the_same_defender() {
    let (mut engine, attacker, kaito) = setup_unblocked_kaito(203_003);
    let defending_assignment = engine.state.combat.as_ref().unwrap().attack_assignments[&attacker];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &ninjutsu_command(&engine, kaito, attacker))
        .expect("activate Ninjutsu");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&kaito].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(engine.state.objects[&kaito].tapped);
    assert_eq!(
        engine.state.combat.as_ref().unwrap().attack_assignments[&kaito].defender,
        defending_assignment.defender
    );
    assert_eq!(
        engine.state.objects[&kaito].counter_count(CounterKind::Loyalty),
        4
    );
}

#[test]
fn kaito_ninjutsu_remains_available_through_end_of_combat() {
    for (seed, step) in [
        (203_005, tricerules_core::TurnStep::FirstStrikeDamage),
        (203_006, tricerules_core::TurnStep::CombatDamage),
        (203_007, tricerules_core::TurnStep::EndCombat),
    ] {
        let (mut engine, _, kaito) = setup_unblocked_kaito(seed);
        engine.state.turn_step = step;
        let action = engine.initial_response_batch().legal_by_player[&0]
            .zone_ability_actions
            .iter()
            .find(|action| action.object_id == kaito)
            .cloned()
            .expect("Kaito zone ability");
        assert!(
            action.ability.is_some_and(|ability| ability.activatable),
            "Ninjutsu must remain available during {step:?}"
        );
    }
}

#[test]
fn kaito_ninjutsu_does_nothing_if_its_source_leaves_hand() {
    let (mut engine, attacker, kaito) = setup_unblocked_kaito(203_008);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &ninjutsu_command(&engine, kaito, attacker))
        .expect("activate Ninjutsu");
    engine.state.players[0]
        .hand
        .retain(|object| *object != kaito);
    engine.state.players[0].graveyard.push(kaito);
    engine.state.objects.get_mut(&kaito).unwrap().zone = tricerules_core::Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(kaito)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&kaito].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(!engine.state.players[0].battlefield.contains(&kaito));
}

#[test]
fn kaito_animation_and_static_emblem_are_continuous_and_dynamic() {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "kaito,_bane_of_nightmares",
                "ninja_of_the_hand",
                "ninja_of_the_hand",
            ],
        ),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(203_004, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let kaito = move_ready_to_battlefield(&mut engine, 0, "kaito,_bane_of_nightmares");
    let ninja = move_ready_to_battlefield(&mut engine, 0, "ninja_of_the_hand");

    let animated = engine.characteristics(kaito).expect("animated Kaito");
    assert!(animated.is_creature());
    assert!(!animated.has_type("Planeswalker"));
    assert!(animated.has_type("Ninja"));
    assert_eq!((animated.power, animated.toughness), (Some(3), Some(4)));
    assert!(animated.has_keyword(tricerules_cards::Keyword::Hexproof));

    apply_ability(&mut engine, 0, kaito, 1, vec![]).expect("activate +1");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.static_emblems.len(), 1);
    assert_eq!(
        (
            engine.characteristics(ninja).unwrap().power,
            engine.characteristics(ninja).unwrap().toughness
        ),
        (Some(3), Some(3))
    );
    let late_ninja = move_ready_to_battlefield(&mut engine, 0, "ninja_of_the_hand");
    assert_eq!(
        (
            engine.characteristics(late_ninja).unwrap().power,
            engine.characteristics(late_ninja).unwrap().toughness,
        ),
        (Some(3), Some(3)),
        "the emblem affects later entrants dynamically"
    );
}

#[test]
fn kaito_zero_surveils_two_then_draws_for_each_opponent_who_lost_life() {
    let decks = Some(vec![
        deck_with("island", &["kaito,_bane_of_nightmares"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(203_010, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let kaito = move_ready_to_battlefield(&mut engine, 0, "kaito,_bane_of_nightmares");
    engine.state.turn_history.current.player_mut(1).life_lost = 1;
    let hand_before = engine.state.players[0].hand.len();

    apply_ability(&mut engine, 0, kaito, 2, vec![]).expect("activate zero");
    engine.apply_command(0, &pass()).expect("controller passes");
    let resolution = engine.apply_command(1, &pass()).expect("opponent passes");
    let choice = find_resolution_choice(&resolution).expect("Surveil 2 choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.candidate_object_ids.len(), 2);
    engine
        .apply_command(
            0,
            &submit_resolution_choice(choice.candidate_object_ids.clone()),
        )
        .expect("put both surveilled cards into the graveyard");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before + 1,
        "one opponent lost life, so Kaito draws one card after surveilling"
    );
}

#[test]
fn kaito_minus_two_taps_and_places_two_stun_counters_on_one_target() {
    let decks = Some(vec![
        deck_with("island", &["kaito,_bane_of_nightmares"]),
        deck_with("island", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(203_009, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let kaito = move_ready_to_battlefield(&mut engine, 0, "kaito,_bane_of_nightmares");
    let target = move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");

    apply_ability(&mut engine, 0, kaito, 3, target_object(target)).expect("activate -2");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&target].tapped);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Stun),
        2
    );
    assert_eq!(
        engine.state.objects[&kaito].counter_count(CounterKind::Loyalty),
        2
    );
}
