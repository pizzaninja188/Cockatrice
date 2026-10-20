use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_proto::ruled::v1::{
    dev_command::Dev, ruled_command::Cmd, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceDecision, RuledCommand, SelectedSpellMode, SubmitResolutionChoice,
};

fn engine() -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("mountain".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        5900,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn three_player_crypt_lurker_branch_choice(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        deck_with("forest", &["crypt_lurker", "grizzly_bears"]),
        island_only_deck(),
        island_only_deck(),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("three-player Crypt Lurker game");
    advance_to_main1_from_game_start(&mut engine);
    inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let discard_candidate = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "crypt_lurker");
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Crypt Lurker");
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Crypt Lurker trigger offers its optional choice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    (engine, discard_candidate)
}

const REFLEXIVE_PROGRAM_DAMAGE_FIXTURE: &str = r#"(
  id: "reflexive_program_damage_fixture",
  name: "Reflexive Program Damage Fixture",
  face_id: "reflexive_program_damage_fixture",
  mana_cost: "{0}",
  types: ["Creature", "Human"],
  power: 1,
  toughness: 2,
  triggered_abilities: [(
    ability_id: "choose_reflexive_damage",
    presentation: Fallback,
    trigger: WhenSelfEntersBattlefield,
    effect: [],
    program: Some((body: (steps: [
      (id: "choose_damage", kind: Choose(
        choice: "damage", chooser: Controller, optional: false,
        branches: [
          (branch_id: "three_any_target", presentation: Fallback,
            cost: None, requirement: Always, body: (steps: [
              (id: "create_any_target_reflexive", kind: Perform(CreateReflexiveTrigger(ability: (
                ability_id: "reflexive_01", presentation: Fallback,
                effect: [DamageTarget(amount: 3, target: (kind: AnyTarget))],
              )))),
            ], yields: [])),
          (branch_id: "four_opposing_creature", presentation: Fallback,
            cost: None, requirement: Always, body: (steps: [
              (id: "create_opposing_creature_reflexive", kind: Perform(CreateReflexiveTrigger(ability: (
                ability_id: "reflexive_02", presentation: Fallback,
                effect: [DamageTarget(amount: 4, target: (kind: Creature, controller: Opponent))],
              )))),
            ], yields: [])),
        ],
        otherwise: (steps: [], yields: []),
      )),
      (id: "parent_tail", kind: Perform(GainLife(amount: 1))),
    ], yields: []), target_bindings: [])),
    modal: None,
    targeting: None,
    may: false,
    intervening_if: None,
    triggers_only_once: false,
    max_triggers_per_turn: None,
  )],
)"#;

const TEST_SIX_TOUGHNESS_CREATURE: &str = r#"(
  id: "test_six_toughness_creature",
  name: "Test Six-Toughness Creature",
  face_id: "test_six_toughness_creature",
  mana_cost: "{0}",
  types: ["Creature", "Wall"],
  power: 0,
  toughness: 6,
)"#;

fn reflexive_program_engine(seed: u64) -> GameEngine {
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                REFLEXIVE_PROGRAM_DAMAGE_FIXTURE,
                TEST_SIX_TOUGHNESS_CREATURE,
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/mountain.ron"),
            ],
            &[],
        )
        .expect("the reflexive Program fixture registry is valid"),
    ));
    let decks = Some(vec![
        tricerules_core::EngineDeck {
            mainboard: [
                vec!["reflexive_program_damage_fixture".into()],
                vec!["forest".into(); 19],
            ]
            .concat(),
            commanders: vec![],
        },
        tricerules_core::EngineDeck {
            mainboard: [
                vec!["test_six_toughness_creature".into()],
                vec!["mountain".into(); 19],
            ]
            .concat(),
            commanders: vec![],
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(registry, seed, &[0, 1], 20, decks, true)
        .expect("two-player reflexive Program engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn open_reflexive_program_choice(engine: &mut GameEngine) {
    inject_card_into_hand(engine, 0, "reflexive_program_damage_fixture");
    let hand_index = hand_index_for_card(engine, 0, "reflexive_program_damage_fixture");
    engine
        .apply_scenario_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast the reflexive Program fixture");
    pass_both_players(engine);
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes for the fixture ETB trigger");
    let offer = engine
        .apply_scenario_command(1, &pass())
        .expect("fixture trigger offers the Program branch choice");
    let choice = find_resolution_choice(&offer).expect("reflexive Program branch choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            cast_spell: None,
            spell_cast_announcement: None,
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: vec![],
            chosen_player_ids: vec![],
        })),
    }
}

fn cast_crypt_lurker_until_branch_choice(engine: &mut GameEngine, crypt_lurker: u32) {
    let index = hand_index_for_card(engine, 0, "crypt_lurker");
    assert_eq!(engine.state.players[0].hand[index], crypt_lurker);
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Crypt Lurker");
    pass_both_players(engine);
    pass_both_players(engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Crypt Lurker trigger offers its optional choice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
}

fn crime_with_bolt(e: &mut GameEngine) {
    inject_card_into_hand(e, 0, "lightning_bolt");
    grant_pool(e, 0);
    e.apply_scenario_command(
        0,
        &cast_spell(
            hand_index_for_card(e, 0, "lightning_bolt"),
            target_player(1),
        ),
    )
    .unwrap();
}

#[test]
fn ability_program_reflexive_damage_uses_a_fresh_target_and_resumes_its_tail_once() {
    let mut any_target = reflexive_program_engine(5_901);
    open_reflexive_program_choice(&mut any_target);
    any_target
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose fixed 3 AnyTarget damage");
    assert_eq!(
        any_target.state.players[0].life, 21,
        "parent tail gains life once"
    );
    assert_eq!(any_target.state.pending_triggers.len(), 1);
    any_target
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_player(1),
                })),
            },
        )
        .expect("choose the reflexive child's fresh player target");
    pass_both_players(&mut any_target);
    assert_eq!(any_target.state.players[1].life, 17);
    assert_eq!(any_target.state.players[0].life, 21);
    assert!(any_target.state.pending_triggers.is_empty());
    assert!(any_target.state.stack.is_empty());

    let mut creature = reflexive_program_engine(5_902);
    let target = inject_creature_with_stats(&mut creature, 1, "test_six_toughness_creature", 0, 6);
    open_reflexive_program_choice(&mut creature);
    creature
        .apply_scenario_command(0, &select_branch(1))
        .expect("choose fixed 4 opposing-Creature damage");
    assert_eq!(
        creature.state.players[0].life, 21,
        "parent tail gains life once"
    );
    creature
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(target),
                })),
            },
        )
        .expect("choose a fresh opposing-Creature target for the child trigger");
    pass_both_players(&mut creature);
    assert_eq!(creature.state.objects[&target].damage, 4);
    assert_eq!(
        creature.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(creature.state.players[0].life, 21);
    assert!(creature.state.pending_triggers.is_empty());
    assert!(creature.state.stack.is_empty());
}

#[test]
fn ability_program_reflexive_creature_child_is_removed_without_a_legal_target() {
    let mut engine = reflexive_program_engine(5_903);
    open_reflexive_program_choice(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("choose the filtered Creature child with no opposing creature available");

    assert_eq!(
        engine.state.players[0].life, 21,
        "the parent tail still resolves once"
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ability_program_reflexive_target_does_not_follow_a_new_zone_generation() {
    let mut engine = reflexive_program_engine(5_904);
    let target = inject_creature_with_stats(&mut engine, 1, "test_six_toughness_creature", 0, 6);
    open_reflexive_program_choice(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("choose the filtered Creature child");
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(target),
                })),
            },
        )
        .expect("choose the fresh Creature target");
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[1].hand.push(target);
    engine.state.objects.get_mut(&target).unwrap().zone = tricerules_core::Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    assert!(engine.state.zone_change_generation[&target] > original_generation);

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(engine.state.objects[&target].damage, 0);
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(
        engine.state.players[0].life, 21,
        "the parent tail resolved once"
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_171_apothecary_targets_first_and_chooses_keyword_at_resolution() {
    for (choice, keyword) in [
        (0, tricerules_cards::Keyword::Menace),
        (1, tricerules_cards::Keyword::Lifelink),
    ] {
        let mut e = engine();
        let apothecary = inject_creature_on_battlefield(&mut e, 0, "rattleback_apothecary");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        crime_with_bolt(&mut e);
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.pending_triggers.front().is_some());
        let choose = |oid| RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                targets: target_object(oid),
                decline: false,
                selected_modes: vec![],
            })),
        };
        assert!(e.apply_scenario_command(0, &choose(opponent)).is_err());
        e.apply_scenario_command(0, &choose(apothecary)).unwrap();
        assert!(
            e.state.pending_resolution.is_none(),
            "keyword is not a modal trigger choice"
        );
        pass_both_players(&mut e);
        assert_eq!(
            e.state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            ChoiceKind::ResolutionBranch
        );
        assert!(e.apply_scenario_command(0, &select_branch(99)).is_err());
        assert!(e.apply_scenario_command(1, &select_branch(choice)).is_err());
        e.apply_scenario_command(0, &select_branch(choice)).unwrap();
        assert!(e
            .characteristics(apothecary)
            .unwrap()
            .keywords
            .contains(&keyword));
        assert_eq!(e.state.turn_history.current.player(0).crimes_committed, 1);
        assert!(
            e.apply_scenario_command(0, &select_branch(choice)).is_err(),
            "no repeated continuation"
        );
        resolve_entire_stack_two_player(&mut e);
        end_active_turn(&mut e, 0);
        assert!(!e
            .characteristics(apothecary)
            .unwrap()
            .keywords
            .contains(&keyword));
    }
}

fn servant_combat_trigger(crime: bool) -> (GameEngine, u32, u32) {
    let mut e = engine();
    let servant = inject_creature_on_battlefield(&mut e, 0, "servant_of_the_stinger");
    let object = e.state.objects.get_mut(&servant).unwrap();
    object.power = Some(1);
    object.toughness = Some(3);
    let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    if crime {
        crime_with_bolt(&mut e);
        resolve_entire_stack_two_player(&mut e);
    }
    e.apply_scenario_command(0, &primitive_yield()).unwrap();
    pass_both_players(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![servant]))
        .unwrap();
    pass_both_players(&mut e);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[1].life, if crime { 16 } else { 19 });
    (e, servant, other)
}

#[test]
fn issue_171_servant_sacrifices_only_itself_then_searches() {
    let (mut e, servant, other) = servant_combat_trigger(true);
    pass_both_players(&mut e);
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .expect("optional sacrifice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    e.apply_scenario_command(0, &select_branch(0)).unwrap();
    let payment = e
        .state
        .pending_resolution
        .as_ref()
        .expect("optional sacrifice payment")
        .presentation
        .clone();
    assert_eq!(payment.choice_kind, ChoiceKind::CostObjects);
    assert_eq!(payment.min, 0);
    assert_eq!(payment.max, 1);
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![servant]
    );
    assert!(e
        .apply_scenario_command(0, &submit_resolution_choice(vec![other]))
        .is_err());
    e.apply_scenario_command(0, &submit_resolution_choice(vec![servant]))
        .unwrap();
    assert!(e.state.players[0].graveyard.contains(&servant));
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("search follows payment");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibrarySearch);
    let searched = pending.presentation.candidates[0];
    e.apply_scenario_command(0, &submit_resolution_choice(vec![searched]))
        .unwrap();
    assert!(e.state.players[0].hand.contains(&searched));
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_171_servant_search_commander_uses_903_9b_hand_or_command_zone() {
    for commander_to_command_zone in [true, false] {
        let (mut e, servant, _) = servant_combat_trigger(true);
        let atraxa = inject_library_card(&mut e, 0, "atraxa,_praetors_voice");
        // This scenario explicitly designates the injected physical Atraxa as P0's commander;
        // it does not claim to validate a Commander deck list.
        e.state.players[0]
            .declared_commander_object_ids
            .push(atraxa);
        let library_before = e.state.players[0].library.len();
        let atraxa_generation_before = e
            .state
            .zone_change_generation
            .get(&atraxa)
            .copied()
            .unwrap_or(0);

        pass_both_players(&mut e);
        e.apply_scenario_command(0, &select_branch(0))
            .expect("choose Servant's optional sacrifice branch");
        let payment_prompt_id = e
            .state
            .pending_resolution
            .as_ref()
            .expect("optional sacrifice payment")
            .resolution_prompt_id
            .get();
        let paid = e
            .apply_scenario_command(0, &submit_resolution_choice(vec![servant]))
            .expect("pay Servant's Program sacrifice cost");
        assert_eq!(
            e.state.objects[&servant].zone,
            tricerules_core::Zone::Graveyard
        );
        let search_prompt_id = e
            .state
            .pending_resolution
            .as_ref()
            .expect("single-card library search")
            .resolution_prompt_id
            .get();
        let search = &e.state.pending_resolution.as_ref().unwrap().presentation;
        assert_eq!(search.choice_kind, ChoiceKind::LibrarySearch);
        assert_eq!((search.min, search.max), (1, 1));
        assert!(search.candidates.contains(&atraxa));

        let searched = e
            .apply_scenario_command(0, &submit_resolution_choice(vec![atraxa]))
            .expect("search Atraxa and receive the CR 903.9b replacement choice");
        let commander_prompt = find_resolution_choice(&searched)
            .expect("the commander replacement is offered before moving Atraxa to hand");
        assert_eq!(commander_prompt.choice_kind(), ChoiceKind::ResolutionBranch);
        assert_eq!(commander_prompt.deciding_player_id, 0);
        assert_eq!((commander_prompt.min, commander_prompt.max), (1, 1));
        assert_eq!(commander_prompt.resolution_branches.len(), 2);
        for (index, label) in ["Command zone", "Hand"].into_iter().enumerate() {
            let branch = &commander_prompt.resolution_branches[index];
            assert_eq!(branch.branch_index, index as u32);
            assert_eq!(branch.label, label);
            assert!(branch.selectable);
        }
        assert_eq!(
            e.state.objects[&atraxa].zone,
            tricerules_core::Zone::Library
        );
        assert_eq!(
            e.state
                .zone_change_generation
                .get(&atraxa)
                .copied()
                .unwrap_or(0),
            atraxa_generation_before,
            "Atraxa remains in the library until its owner chooses a destination"
        );
        let commander_prompt_id = e
            .state
            .pending_resolution
            .as_ref()
            .expect("the commander replacement is pending")
            .resolution_prompt_id
            .get();

        let mut destination = submit_resolution_choice(Vec::new());
        let Some(Cmd::SubmitResolutionChoice(answer)) = destination.cmd.as_mut() else {
            unreachable!()
        };
        answer.resolution_prompt_id = commander_prompt_id;
        answer.decision = ResolutionChoiceDecision::SelectBranch as i32;
        answer.selected_branch_index = if commander_to_command_zone { 0 } else { 1 };
        let moved = e
            .apply_scenario_command(0, &destination)
            .expect("choose Atraxa's CR 903.9b destination");

        let expected_zone = if commander_to_command_zone {
            tricerules_core::Zone::Command
        } else {
            tricerules_core::Zone::Hand
        };
        assert_eq!(e.state.objects[&atraxa].zone, expected_zone);
        assert_eq!(e.state.players[0].library.len(), library_before - 1);
        assert_eq!(
            e.state.zone_change_generation.get(&atraxa).copied(),
            Some(atraxa_generation_before + 1)
        );
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(e.state.stack.len(), 0);

        let batches = [&paid, &searched, &moved];
        assert_eq!(
            batches
                .iter()
                .flat_map(|batch| &batch.events)
                .filter(|event| matches!(
                    &event.ev,
                    Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                        if log.text == "P0 shuffles their library."
                ))
                .count(),
            1,
            "Servant's search and shuffle resolve once"
        );
        assert_eq!(
            batches
                .iter()
                .flat_map(|batch| &batch.events)
                .filter(|event| matches!(
                    &event.ev,
                    Some(tricerules_proto::ruled::v1::ruled_event::Ev::PermanentMoved(moved))
                        if (moved.object_id == servant
                            && moved.destination
                                == tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32)
                            || (moved.object_id == atraxa
                                && moved.destination
                                    == if commander_to_command_zone {
                                        tricerules_proto::ruled::v1::permanent_moved::Destination::Command as i32
                                    } else {
                                        tricerules_proto::ruled::v1::permanent_moved::Destination::Hand as i32
                                    })
                ))
                .count(),
            2,
            "the sacrifice and Atraxa destination each emit one physical move"
        );
        assert_eq!(
            batches
                .iter()
                .flat_map(|batch| &batch.events)
                .filter(|event| matches!(
                    &event.ev,
                    Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                        if log.text.starts_with("P0 sacrifices Servant of the Stinger")
                ))
                .count(),
            1,
            "the sacrifice receipt is emitted once"
        );

        let state_before_replay = format!("{:?}", e.state);
        let mut replay = submit_resolution_choice(vec![atraxa]);
        let Some(Cmd::SubmitResolutionChoice(answer)) = replay.cmd.as_mut() else {
            unreachable!()
        };
        answer.resolution_prompt_id = search_prompt_id;
        assert!(e.apply_scenario_command(0, &replay).is_err());
        assert_eq!(format!("{:?}", e.state), state_before_replay);
        assert!(e.state.players[0]
            .declared_commander_object_ids
            .contains(&atraxa));
        assert_eq!(
            e.state.objects[&atraxa].zone, expected_zone,
            "a stale search answer cannot move the commander a second time"
        );
        assert_ne!(payment_prompt_id, search_prompt_id);
    }
}

#[test]
fn issue_171_servant_can_decline_its_optional_sacrifice_payment() {
    let (mut e, servant, _) = servant_combat_trigger(true);
    let hand_before = e.state.players[0].hand.len();
    pass_both_players(&mut e);
    e.apply_scenario_command(0, &select_branch(0))
        .expect("choose the optional sacrifice branch");
    let payment = &e.state.pending_resolution.as_ref().unwrap().presentation;
    assert_eq!(payment.choice_kind, ChoiceKind::CostObjects);
    assert_eq!((payment.min, payment.max), (0, 1));
    assert_eq!(payment.candidates, vec![servant]);

    e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("decline the optional sacrifice payment");
    assert!(e.state.players[0].battlefield.contains(&servant));
    assert_eq!(e.state.players[0].hand.len(), hand_before);
    assert!(e.state.pending_resolution.is_none());
    assert!(e.state.stack.is_empty());
}

#[test]
fn issue_171_servant_checks_condition_and_never_searches_without_payment() {
    let (e, _, _) = servant_combat_trigger(false);
    assert!(e.state.stack.is_empty(), "no Crime means no combat trigger");
    for failure in ["condition", "decline", "departed", "stolen"] {
        let (mut e, servant, _) = servant_combat_trigger(true);
        let hand_before = e.state.players[0].hand.len();
        match failure {
            // Exercise the resolution-side intervening-if check independently of its monotonic
            // normal turn history, as other condition tests do with controlled fixtures.
            "condition" => e.state.turn_history.current.player_mut(0).crimes_committed = 0,
            "departed" => {
                e.state.players[0].battlefield.retain(|id| *id != servant);
                e.state.players[0].graveyard.push(servant);
                e.state.objects.get_mut(&servant).unwrap().zone = tricerules_core::Zone::Graveyard;
                e.state.zone_change_generation.insert(servant, 1);
            }
            "stolen" => {
                e.state.players[0].battlefield.retain(|id| *id != servant);
                e.state.players[1].battlefield.push(servant);
                let object = e.state.objects.get_mut(&servant).unwrap();
                object.controller = 1;
                object.base_controller = 1;
            }
            _ => {}
        }
        pass_both_players(&mut e);
        if failure == "decline" {
            e.apply_scenario_command(
                0,
                &submit_resolution_decision(ResolutionChoiceDecision::Decline),
            )
            .unwrap();
        }
        assert!(
            e.state.pending_resolution.is_none(),
            "{failure}: no library prompt"
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before);
        assert!(e.state.stack.is_empty());
    }
}

#[test]
fn issue_171_servant_rejects_a_returned_source_during_payment() {
    let (mut e, servant, _) = servant_combat_trigger(true);
    pass_both_players(&mut e);
    e.apply_scenario_command(0, &select_branch(0)).unwrap();
    e.state.zone_change_generation.insert(servant, 1);
    let history = e.state.turn_history.clone();
    assert!(e
        .apply_scenario_command(0, &submit_resolution_choice(vec![servant]))
        .is_err());
    assert!(e.state.players[0].battlefield.contains(&servant));
    assert_eq!(e.state.turn_history, history);
    assert!(e.state.pending_resolution.is_some());
    e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn issue_171_apothecary_fizzles_without_prompt_when_target_blinks() {
    let mut e = engine();
    let source = inject_creature_on_battlefield(&mut e, 0, "rattleback_apothecary");
    crime_with_bolt(&mut e);
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                targets: target_object(source),
                decline: false,
                selected_modes: vec![],
            })),
        },
    )
    .unwrap();
    e.state.zone_change_generation.insert(source, 1);
    pass_both_players(&mut e);
    assert!(e.state.pending_resolution.is_none());
    assert!(!e
        .characteristics(source)
        .unwrap()
        .keywords
        .contains(&tricerules_cards::Keyword::Menace));
    assert_eq!(e.state.stack.len(), 1, "the original Bolt remains");
}

#[test]
fn issue_59_calibration_cards_are_registered() {
    let registry = tricerules_cards::registry::global();

    for card_id in ["trufflesnout", "sparktongue_dragon", "crypt_lurker"] {
        assert!(
            registry.get(card_id).is_some(),
            "issue #59 requires {card_id} to be implemented"
        );
    }
}

#[test]
fn trufflesnout_trigger_mode_is_chosen_before_it_reaches_the_stack() {
    let mut engine = engine();
    let trufflesnout = inject_card_into_hand(&mut engine, 0, "trufflesnout");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "trufflesnout");
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Trufflesnout");
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("modal ETB choice must be pending");
    assert!(pending.ability.modal.is_some());
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: vec![SelectedSpellMode {
                        mode_index: 0,
                        targets: Vec::new(),
                    }],
                    targets: Vec::new(),
                })),
            },
        )
        .expect("choose counter mode");
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .objects
            .get(&trufflesnout)
            .expect("Trufflesnout")
            .counter_count(tricerules_cards::primitives::CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn crypt_lurker_discard_branch_pays_then_draws() {
    let mut engine = engine();
    inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let discarded = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "crypt_lurker");
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Crypt Lurker");
    pass_both_players(&mut engine); // creature resolves, ETB trigger reaches stack
    let branch_offer_batch = pass_both_players_with_batch(&mut engine); // ETB parks for branch choice
    let branch_offer = find_resolution_choice(&branch_offer_batch).expect("branch choice offer");
    let discard_branch = branch_offer
        .resolution_branches
        .iter()
        .find(|branch| branch.branch_index == 1)
        .expect("the discard option is branch 1");
    assert_eq!(discard_branch.label, "Discard a creature card");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("branch choice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    assert!(matches!(
        &engine
            .state
            .pending_resolution
            .as_ref()
            .expect("branch choice")
            .continuation,
        ResolutionContinuation::ProgramChoose { .. }
    ));
    engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("choose discard branch");
    let discard_payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("discard payment")
        .presentation
        .clone();
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("discard payment")
            .deciding_player,
        0
    );
    let discard_continuation = &engine
        .state
        .pending_resolution
        .as_ref()
        .expect("discard payment")
        .continuation;
    assert!(matches!(
        discard_continuation,
        ResolutionContinuation::ProgramChoose { .. }
    ));
    assert!(
        format!("{discard_continuation:?}").contains("PayingDiscardCardCost"),
        "the Program Choose owns its generation-bound discard-cost continuation"
    );
    assert_eq!(discard_payment.choice_kind, ChoiceKind::HandCards);
    assert_eq!((discard_payment.min, discard_payment.max), (0, 1));
    assert_eq!(discard_payment.candidates, vec![discarded]);
    let hand_before_payment = engine.state.players[0].hand.len();
    let library_before_payment = engine.state.players[0].library.len();
    let discard_generation_before = engine
        .state
        .zone_change_generation
        .get(&discarded)
        .copied()
        .unwrap_or(0);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("discard creature card");
    assert_eq!(
        engine
            .state
            .objects
            .get(&discarded)
            .expect("discarded card")
            .zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.zone_change_generation[&discarded] > discard_generation_before);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_payment,
        "one paid discard is followed by the branch's one draw"
    );
    assert_eq!(
        engine.state.players[0].library.len() + 1,
        library_before_payment,
        "the branch draws exactly once"
    );
    assert!(engine.state.pending_resolution.is_none());
    let after_payment = format!("{:?}", engine.state);
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .is_err());
    assert_eq!(
        format!("{:?}", engine.state),
        after_payment,
        "replaying the consumed offer cannot discard or draw again"
    );
}

#[test]
fn crypt_lurker_self_sacrifice_pays_and_its_trigger_draws_once() {
    let mut engine = engine();
    let crypt_lurker = inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    inject_library_card(&mut engine, 0, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    cast_crypt_lurker_until_branch_choice(&mut engine, crypt_lurker);
    let library_before = engine.state.players[0].library.len();
    let hand_before_payment = engine.state.players[0].hand.len();
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&crypt_lurker)
        .copied()
        .unwrap_or(0);

    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose the sacrifice branch");
    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("optional sacrifice payment")
        .presentation
        .clone();
    assert_eq!(payment.candidates, vec![crypt_lurker]);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![crypt_lurker]))
        .expect("Crypt Lurker can sacrifice itself for its already-triggered ability");

    assert_eq!(
        engine.state.objects[&crypt_lurker].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.zone_change_generation[&crypt_lurker] > generation_before);
    assert_eq!(engine.state.players[0].hand.len(), hand_before_payment + 1);
    assert_eq!(engine.state.players[0].library.len() + 1, library_before);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn crypt_lurker_optional_menu_and_both_costs_can_be_declined_or_left_empty() {
    // Declining the optional mode at the top level follows the empty Otherwise body.
    let mut engine = engine();
    let crypt_lurker = inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let creature_card = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    cast_crypt_lurker_until_branch_choice(&mut engine, crypt_lurker);
    let library_before = engine.state.players[0].library.len();
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the optional branch menu");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&creature_card].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(engine.state.players[0].library.len(), library_before);

    for branch in [0, 1] {
        for explicit_decline in [false, true] {
            let mut engine = self::engine();
            let crypt_lurker = inject_card_into_hand(&mut engine, 0, "crypt_lurker");
            let paid_object = if branch == 0 {
                inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears")
            } else {
                inject_card_into_hand(&mut engine, 0, "grizzly_bears")
            };
            give_mana(
                &mut engine,
                0,
                ManaGift {
                    b: 1,
                    c: 3,
                    ..Default::default()
                },
            );
            cast_crypt_lurker_until_branch_choice(&mut engine, crypt_lurker);
            let library_before = engine.state.players[0].library.len();
            engine
                .apply_scenario_command(0, &select_branch(branch))
                .expect("select one optional payment branch");
            let payment = &engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation;
            assert_eq!((payment.min, payment.max), (0, 1));
            assert!(payment.candidates.contains(&paid_object));
            let answer = if explicit_decline {
                submit_resolution_decision(ResolutionChoiceDecision::Decline)
            } else {
                submit_resolution_choice(Vec::new())
            };
            engine
                .apply_scenario_command(0, &answer)
                .unwrap_or_else(|error| panic!("branch {branch} empty/declined payment: {error}"));
            assert!(engine.state.pending_resolution.is_none());
            assert_eq!(engine.state.players[0].library.len(), library_before);
            assert_eq!(
                engine.state.objects[&paid_object].zone,
                if branch == 0 {
                    tricerules_core::Zone::Battlefield
                } else {
                    tricerules_core::Zone::Hand
                }
            );
        }
    }
}

#[test]
fn crypt_lurker_does_not_offer_fiery_temper_as_a_creature_discard_cost() {
    let mut engine = engine();
    let crypt_lurker = inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let fiery_temper = inject_card_into_hand(&mut engine, 0, "fiery_temper");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    cast_crypt_lurker_until_branch_choice(&mut engine, crypt_lurker);
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_scenario_command(0, &select_branch(1)).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    assert_eq!(
        engine.state.objects[&fiery_temper].zone,
        tricerules_core::Zone::Hand
    );
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn crypt_lurker_filtered_discard_answers_reject_invalid_objects_atomically() {
    let mut engine = engine();
    let crypt_lurker = inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let creature = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let noncreature = inject_card_into_hand(&mut engine, 0, "fiery_temper");
    let foreign_creature = inject_card_into_hand(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    cast_crypt_lurker_until_branch_choice(&mut engine, crypt_lurker);
    engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("choose the creature-discard branch");

    let mut unrelated = submit_resolution_choice(vec![creature]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = unrelated.cmd.as_mut() else {
        unreachable!()
    };
    answer.chosen_player_ids.push(1);
    for (label, answer) in [
        ("noncreature", submit_resolution_choice(vec![noncreature])),
        (
            "foreign card",
            submit_resolution_choice(vec![foreign_creature]),
        ),
        (
            "duplicate object",
            submit_resolution_choice(vec![creature, creature]),
        ),
        ("unrelated player field", unrelated),
    ] {
        let before = format!("{:?}", engine.state);
        assert!(
            engine.apply_scenario_command(0, &answer).is_err(),
            "{label} is outside the filtered discard payment"
        );
        assert_eq!(format!("{:?}", engine.state), before, "{label} is atomic");
        assert!(engine.state.pending_resolution.is_some());
    }
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![creature]))
        .expect("a current, owned creature remains a valid payment");
    assert_eq!(
        engine.state.objects[&creature].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&noncreature].zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(
        engine.state.objects[&foreign_creature].zone,
        tricerules_core::Zone::Hand
    );
}

#[test]
fn crypt_lurker_cost_discard_ignores_library_of_leng_and_resolves_madness_after_draw() {
    let mut engine = engine();
    let crypt_lurker = inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let wurm = inject_card_into_hand(&mut engine, 0, "arrogant_wurm");
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    inject_library_card(&mut engine, 0, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    cast_crypt_lurker_until_branch_choice(&mut engine, crypt_lurker);
    engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("choose the creature-discard branch");
    let hand_before_payment = engine.state.players[0].hand.len();
    let library_before_payment = engine.state.players[0].library.len();
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![wurm]))
        .expect("discard Arrogant Wurm as a resolution cost");
    assert_eq!(
        engine.state.objects[&wurm].zone,
        tricerules_core::Zone::Exile,
        "Madness replaces a cost discard; Library of Leng does not"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before_payment);
    assert_eq!(
        engine.state.players[0].library.len() + 1,
        library_before_payment
    );
    assert!(engine.state.pending_resolution.is_none());

    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the Madness cast after Crypt Lurker already drew");
    assert_eq!(
        engine.state.objects[&wurm].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn crypt_lurker_trigger_resolves_after_unsummon_returns_its_source() {
    let mut engine = engine();
    let crypt_lurker = inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let creature_card = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 1, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let library_before = engine.state.players[0].library.len();
    let index = hand_index_for_card(&engine, 0, "crypt_lurker");
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Crypt Lurker");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes so the opponent can return the source");
    let unsummon_index = hand_index_for_card(&engine, 1, "unsummon");
    engine
        .apply_scenario_command(1, &cast_spell(unsummon_index, target_object(crypt_lurker)))
        .expect("cast Unsummon with the ETB trigger waiting on the stack");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&crypt_lurker].zone,
        tricerules_core::Zone::Hand
    );

    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("the resolving trigger keeps its optional Program");
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![creature_card]))
        .expect("discard and draw after the source returned to hand");
    assert_eq!(
        engine.state.objects[&creature_card].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(engine.state.players[0].library.len() + 1, library_before);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn crypt_lurker_payer_concession_does_not_delegate_or_draw_in_three_player_game() {
    let (mut engine, _) = three_player_crypt_lurker_branch_choice(59_003);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("its controller's branch choice is pending")
            .deciding_player,
        0
    );
    let draw_counts_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| {
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn
        })
        .collect();

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the payer may concede during its own optional resolution choice");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[1].has_lost && !engine.state.players[2].has_lost);
    assert!(
        engine.state.outcome.is_none(),
        "the two survivors keep playing"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(find_resolution_choice(&departure).is_none());
    for (player, before) in engine.state.players.iter().zip(draw_counts_before) {
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn,
            before,
            "concession must not resolve the paid branch body or delegate its draw"
        );
    }
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[2].life, 20);
}

#[test]
fn crypt_lurker_payer_concession_does_not_delegate_or_draw_during_discard_payment() {
    let (mut engine, discard_candidate) = three_player_crypt_lurker_branch_choice(59_004);
    engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("choose the optional creature-discard branch");
    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the discard cost is pending")
        .clone();
    assert_eq!(payment.deciding_player, 0);
    assert_eq!(payment.presentation.choice_kind, ChoiceKind::HandCards);
    assert_eq!((payment.presentation.min, payment.presentation.max), (0, 1));
    assert_eq!(payment.presentation.candidates, vec![discard_candidate]);
    assert!(matches!(
        &payment.continuation,
        ResolutionContinuation::ProgramChoose { .. }
    ));
    assert!(format!("{:?}", payment.continuation).contains("PayingDiscardCardCost"));
    assert!(engine.state.players[0].hand.contains(&discard_candidate));
    let draw_counts_before: Vec<_> = engine
        .state
        .players
        .iter()
        .map(|player| {
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn
        })
        .collect();

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the payer may concede during its own discard-cost choice");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[1].has_lost && !engine.state.players[2].has_lost);
    assert!(
        engine.state.outcome.is_none(),
        "the two survivors keep playing"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(find_resolution_choice(&departure).is_none());
    for (player, before) in engine.state.players.iter().zip(draw_counts_before) {
        assert_eq!(
            engine
                .state
                .turn_history
                .current
                .player(player.id)
                .cards_drawn,
            before,
            "concession must not commit the discard payment or run the branch draw"
        );
    }
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[2].life, 20);
}

#[test]
fn crypt_lurker_sacrifice_branch_uses_optional_cost_object_payment() {
    let mut engine = engine();
    inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let sacrificed = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "crypt_lurker");
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Crypt Lurker");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose the optional sacrifice branch");

    let payment = &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation;
    assert_eq!(payment.choice_kind, ChoiceKind::CostObjects);
    assert_eq!((payment.min, payment.max), (0, 1));
    assert!(payment.candidates.contains(&sacrificed));
    let hand_before_payment = engine.state.players[0].hand.len();
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![sacrificed]))
        .expect("pay with the other creature");
    assert!(engine.state.players[0].graveyard.contains(&sacrificed));
    assert!(engine.state.players[0].battlefield.iter().any(|oid| {
        engine
            .state
            .objects
            .get(oid)
            .is_some_and(|object| object.card_id == "crypt_lurker")
    }));
    assert_eq!(engine.state.players[0].hand.len(), hand_before_payment + 1);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn crypt_lurker_rejects_a_stale_discard_without_consuming_the_choice() {
    let mut engine = engine();
    inject_card_into_hand(&mut engine, 0, "crypt_lurker");
    let candidate = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "crypt_lurker");
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Crypt Lurker");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch(1))
        .expect("choose discard branch");

    let offered_generation = engine
        .state
        .zone_change_generation
        .get(&candidate)
        .copied()
        .unwrap_or(0);
    engine.state.players[0].hand.retain(|oid| *oid != candidate);
    engine.state.players[0].graveyard.push(candidate);
    engine
        .state
        .objects
        .get_mut(&candidate)
        .expect("candidate")
        .zone = tricerules_core::Zone::Graveyard;
    // This fixture stages the same physical ID as a new Hand incarnation so the old prompt
    // must fail on the saved generation, even though the ID is again in the legal zone.
    let returned_generation = offered_generation + 2;
    engine.state.players[0]
        .graveyard
        .retain(|oid| *oid != candidate);
    engine.state.players[0].hand.push(candidate);
    engine
        .state
        .objects
        .get_mut(&candidate)
        .expect("returned candidate")
        .zone = tricerules_core::Zone::Hand;
    engine
        .state
        .zone_change_generation
        .insert(candidate, returned_generation);
    assert!(returned_generation > offered_generation);
    assert_eq!(
        engine.state.objects[&candidate].zone,
        tricerules_core::Zone::Hand
    );
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![candidate]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
}

#[test]
fn sparktongue_payment_creates_a_separately_targeted_reflexive_trigger() {
    let mut engine = engine();
    inject_card_into_hand(&mut engine, 0, "sparktongue_dragon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "sparktongue_dragon");
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Sparktongue Dragon");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose mana branch");
    let continuation = &engine
        .state
        .pending_resolution
        .as_ref()
        .expect("mana payment")
        .continuation;
    assert!(matches!(
        continuation,
        ResolutionContinuation::ProgramChoose { .. }
    ));
    assert!(
        continuation.mana_payment().is_some(),
        "the Program Choose continuation is parked in its mana-payment stage"
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {2}{R}");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_player(1),
                })),
            },
        )
        .expect("target opponent with reflexive trigger");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 17);
}

#[test]
fn sparktongue_reflexive_damage_can_target_an_opposing_permanent() {
    let mut engine = engine();
    let target = inject_creature_with_stats(&mut engine, 1, "wall_of_omens", 0, 4);
    inject_card_into_hand(&mut engine, 0, "sparktongue_dragon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "sparktongue_dragon");
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Sparktongue Dragon");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose the {2}{R} branch");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {2}{R}");
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(target),
                })),
            },
        )
        .expect("target the opposing Wall of Omens with the reflexive trigger");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&target].damage, 3);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn sparktongue_reflexive_damage_does_not_follow_a_same_object_reentry() {
    let mut engine = engine();
    engine.enable_dev_commands();
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "sparktongue_dragon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "sparktongue_dragon");
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .expect("cast Sparktongue Dragon");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose mana branch");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {2}{R}");
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(target),
                })),
            },
        )
        .expect("target the opposing Bear with the reflexive trigger");
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    let move_target = |zone: DevZone| RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 1,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: "Grizzly Bears".into(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    };
    engine
        .apply_scenario_command(0, &move_target(DevZone::Hand))
        .expect("move the targeted Bear away");
    engine
        .apply_scenario_command(0, &move_target(DevZone::Battlefield))
        .expect("return the same physical Bear as a new object");
    let returned_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    assert!(returned_generation > original_generation);

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&target].damage, 0);
    assert_eq!(engine.state.players[1].life, 20);
    assert!(engine.state.stack.is_empty());
}
