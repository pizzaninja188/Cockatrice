use super::helpers::*;

fn setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn produce_restricted_mana(engine: &mut GameEngine, card_id: &str) -> (u32, u32) {
    let source = inject_permanent_on_battlefield(engine, 0, card_id);
    engine
        .apply_command(0, &activate_ability_for(engine, source, 0, vec![]))
        .unwrap_or_else(|error| panic!("activate {card_id}: {error}"));
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("restricted contribution")
        .restriction_group_id;
    (source, group)
}

fn spend_restricted_on_ability(
    engine: &mut GameEngine,
    source: u32,
    group: u32,
    color: char,
) -> Result<RuledEventBatch, tricerules_core::EngineError> {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    let mut selection = ManaSpendSelection {
        restriction_group_id: group,
        ..Default::default()
    };
    match color {
        'R' => selection.r = 1,
        'U' => selection.u = 1,
        _ => unreachable!(),
    }
    ability.restricted_mana.push(selection);
    engine.apply_command(0, &command)
}

#[test]
fn purple_dragon_punks_mana_pays_any_mana_or_nonmana_activated_ability() {
    let mut engine = setup(190_001);
    let hellhound = inject_permanent_on_battlefield(&mut engine, 0, "fiery_hellhound");
    let devotee = inject_permanent_on_battlefield(&mut engine, 0, "jeskai_devotee");
    let (_, group) = produce_restricted_mana(&mut engine, "purple_dragon_punks");
    produce_restricted_mana(&mut engine, "purple_dragon_punks");

    let legal = engine.initial_response_batch();
    for source in [hellhound, devotee] {
        let key = u64::from(source) << 32;
        assert_eq!(
            legal.legal_by_player[&0].mana_payment_by_ability[&key]
                .eligible_restricted_mana_group_ids,
            [group]
        );
    }

    spend_restricted_on_ability(&mut engine, hellhound, group, 'R').expect("pay a nonmana ability");
    spend_restricted_on_ability(&mut engine, devotee, group, 'R').expect("pay a mana ability");
    assert!(engine.state.players[0].restricted_mana.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);
}

#[test]
fn all_nonspell_permission_includes_abilities_but_the_narrow_permission_excludes_special_actions() {
    for (card_id, expected_special_action) in
        [("purple_dragon_punks", false), ("hydraulic_helper", true)]
    {
        let mut engine = setup(190_010 + u64::from(expected_special_action));
        let devotee = inject_permanent_on_battlefield(&mut engine, 0, "jeskai_devotee");
        let angel = inject_permanent_on_battlefield(&mut engine, 0, "serra_angel");
        engine.state.objects.get_mut(&angel).unwrap().face_down = true;
        let (_, group) = produce_restricted_mana(&mut engine, card_id);

        let legal = engine.initial_response_batch();
        let ability_key = u64::from(devotee) << 32;
        assert_eq!(
            legal.legal_by_player[&0].mana_payment_by_ability[&ability_key]
                .eligible_restricted_mana_group_ids,
            [group]
        );
        let turn_face_up = legal.legal_by_player[&0]
            .permanent_actions
            .iter()
            .find(|action| action.object_id == angel)
            .expect("turn-face-up action");
        assert_eq!(
            turn_face_up
                .eligible_restricted_mana_group_ids
                .contains(&group),
            expected_special_action
        );
        let action_generation = turn_face_up.zone_change_generation;

        if expected_special_action {
            give_mana(
                &mut engine,
                0,
                ManaGift {
                    w: 2,
                    c: 2,
                    ..Default::default()
                },
            );
            execute_permanent_action_with_payment(
                &mut engine,
                0,
                RuledCommand {
                    cmd: Some(Cmd::ExecutePermanentAction(ExecutePermanentAction {
                        kind: PermanentActionKind::TurnFaceUp as i32,
                        object_id: angel,
                        expected_zone_change_generation: action_generation,
                        restricted_mana: vec![ManaSpendSelection {
                            restriction_group_id: group,
                            u: 1,
                            ..Default::default()
                        }],
                        ..Default::default()
                    })),
                },
            )
            .expect("all-nonspell mana pays a special action");
            assert!(!engine.state.objects[&angel].face_down);
            assert!(engine.state.players[0].restricted_mana.is_empty());
        } else {
            spend_restricted_on_ability(&mut engine, devotee, group, 'R')
                .expect("narrow permission pays an activated ability");
        }
    }
}

#[test]
fn both_cards_allow_artifact_spells_and_reject_nonartifact_spells_atomically() {
    for (seed, card_id, color) in [
        (190_020, "purple_dragon_punks", 'R'),
        (190_021, "hydraulic_helper", 'U'),
    ] {
        let mut artifact_engine = setup(seed);
        let (_, group) = produce_restricted_mana(&mut artifact_engine, card_id);
        inject_card_into_hand(&mut artifact_engine, 0, "bonesplitter");
        let slot = hand_index_for_card(&artifact_engine, 0, "bonesplitter");
        let mut command = cast_spell(slot, vec![]);
        let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
            unreachable!()
        };
        cast.restricted_mana.push(ManaSpendSelection {
            restriction_group_id: group,
            r: u32::from(color == 'R'),
            u: u32::from(color == 'U'),
            ..Default::default()
        });
        artifact_engine
            .apply_command(0, &command)
            .expect("restricted mana casts an artifact spell");

        let mut creature_engine = setup(seed + 100);
        let (_, group) = produce_restricted_mana(&mut creature_engine, card_id);
        inject_card_into_hand(&mut creature_engine, 0, "coral_merfolk");
        give_mana(
            &mut creature_engine,
            0,
            ManaGift {
                u: u32::from(color == 'R'),
                c: u32::from(color == 'U'),
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&creature_engine, 0, "coral_merfolk");
        let mut command = cast_spell(slot, vec![]);
        let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
            unreachable!()
        };
        cast.restricted_mana.push(ManaSpendSelection {
            restriction_group_id: group,
            r: u32::from(color == 'R'),
            u: u32::from(color == 'U'),
            ..Default::default()
        });
        let pool_before = creature_engine.state.players[0].mana_pool;
        let restricted_before = creature_engine.state.players[0].restricted_mana.clone();
        creature_engine
            .apply_command(0, &command)
            .expect_err("restricted mana cannot cast a nonartifact spell");
        assert_eq!(creature_engine.state.players[0].mana_pool, pool_before);
        assert_eq!(
            creature_engine.state.players[0].restricted_mana,
            restricted_before
        );
    }
}

fn ward_payment_fixture(seed: u64, mana_source_card_id: &str) -> (GameEngine, u32, u32) {
    let mut engine = setup(seed);
    let mana_source = inject_permanent_on_battlefield(&mut engine, 0, mana_source_card_id);
    let warded =
        inject_permanent_on_battlefield(&mut engine, 1, "dirgur_island_dragon_skimming_strike");
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &cast_spell(slot, target_object(warded)))
        .expect("cast Unsummon at Ward permanent");
    pass_both_players(&mut engine);
    assert!(
        engine.state.pending_resolution.is_some(),
        "Ward payment prompt"
    );
    engine
        .apply_command(0, &activate_ability_for(&engine, mana_source, 0, vec![]))
        .expect("activate restricted mana ability during Ward payment");
    let group = engine.state.players[0]
        .restricted_mana
        .last()
        .expect("restricted contribution")
        .restriction_group_id;
    (engine, group, spell_id)
}

fn ward_payment_answer(group: u32, color: char) -> SubmitResolutionChoice {
    SubmitResolutionChoice {
        decision: tricerules_proto::ruled::v1::ResolutionChoiceDecision::PayMana as i32,
        restricted_mana: vec![ManaSpendSelection {
            restriction_group_id: group,
            r: u32::from(color == 'R'),
            u: u32::from(color == 'U'),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn hydraulic_helper_mana_pays_ward_but_punks_mana_does_not() {
    let (mut helper_engine, helper_group, spell_id) =
        ward_payment_fixture(190_030, "hydraulic_helper");
    give_mana(
        &mut helper_engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    submit_mana_resolution(
        &mut helper_engine,
        0,
        ward_payment_answer(helper_group, 'U'),
    )
    .expect("Hydraulic Helper mana pays Ward");
    assert!(helper_engine.state.players[0].restricted_mana.is_empty());
    assert!(helper_engine
        .state
        .stack
        .iter()
        .any(|item| item.id == spell_id));

    let (mut punks_engine, punks_group, _) = ward_payment_fixture(190_031, "purple_dragon_punks");
    give_mana(
        &mut punks_engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let restricted_before = punks_engine.state.players[0].restricted_mana.clone();
    submit_mana_resolution(&mut punks_engine, 0, ward_payment_answer(punks_group, 'R'))
        .expect_err("Purple Dragon Punks mana cannot pay Ward");
    assert_eq!(
        punks_engine.state.players[0].restricted_mana,
        restricted_before
    );
    assert!(punks_engine.state.pending_resolution.is_some());
}

#[test]
fn declining_a_resolution_payment_rewinds_restricted_mana_produced_during_the_prompt() {
    let (mut engine, _, _) = ward_payment_fixture(190_032, "hydraulic_helper");
    let helper = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| engine.state.objects[oid].card_id == "hydraulic_helper")
        .expect("Hydraulic Helper");
    assert!(engine.state.objects[&helper].tapped);
    assert_eq!(engine.state.players[0].restricted_mana.len(), 1);

    engine
        .apply_command(
            0,
            &submit_resolution_decision(
                tricerules_proto::ruled::v1::ResolutionChoiceDecision::Decline,
            ),
        )
        .expect("decline Ward payment");
    assert!(!engine.state.objects[&helper].tapped);
    assert!(engine.state.players[0].restricted_mana.is_empty());
    assert!(engine.state.undoable_mana_abilities.is_empty());
}
