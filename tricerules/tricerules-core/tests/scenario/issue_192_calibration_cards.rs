use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, ChooseTriggerTarget,
    ResolutionChoiceDecision, RuledCommand,
};

fn zone_ability(engine: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn bite_targets(source: u32, recipient: u32) -> Vec<TargetRef> {
    vec![
        TargetRef {
            object_id: source,
            group_index: 0,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
        TargetRef {
            object_id: recipient,
            group_index: 1,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
    ]
}

#[test]
fn issue_192_zog_mountaincycling_is_private_and_generation_bound() {
    let decks = Some(vec![
        deck_with("mountain", &["zog,_triceraton_castaway"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(192_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "zog,_triceraton_castaway");
    let zog =
        engine.state.players[0].hand[hand_index_for_card(&engine, 0, "zog,_triceraton_castaway")];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = zone_ability(&engine, zog);
    engine
        .apply_command(0, &command)
        .expect("activate Mountaincycling");
    assert_eq!(engine.state.objects[&zog].zone, Zone::Graveyard);
    engine.apply_command(0, &pass()).expect("controller passes");
    let batch = engine
        .apply_command(1, &pass())
        .expect("Mountaincycling resolves");
    let choice = find_resolution_choice(&batch).expect("private library search");
    assert!(!choice.candidate_object_ids.is_empty());
    assert!(choice
        .candidate_object_ids
        .iter()
        .all(|oid| engine.state.objects[oid].card_id == "mountain"));
    let mountain = choice.candidate_object_ids[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![mountain]))
        .expect("choose Mountain");
    assert_eq!(engine.state.objects[&mountain].zone, Zone::Hand);
    engine
        .apply_command(0, &command)
        .expect_err("stale hand action cannot be replayed");
}

#[test]
fn issue_192_return_to_sewers_uses_owner_choice_then_creates_working_mutagen() {
    let decks = Some(vec![
        deck_with("island", &["return_to_the_sewers", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(192_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "return_to_the_sewers");
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let counter_target = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "return_to_the_sewers");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Return to the Sewers");
    engine.apply_command(0, &pass()).expect("caster passes");
    let batch = engine.apply_command(1, &pass()).expect("resolve to choice");
    let choice = find_resolution_choice(&batch).expect("owner placement choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert!(engine.apply_command(0, &select_branch(1)).is_err());
    engine
        .apply_command(1, &select_branch(1))
        .expect("owner chooses bottom");
    assert_eq!(engine.state.objects[&target].zone, Zone::Library);
    assert_eq!(
        engine.state.players[1].library.back().copied(),
        Some(target)
    );

    let mutagens = battlefield_token_oids(&engine, 0, "mutagen");
    let [mutagen] = mutagens.as_slice() else {
        panic!("Return to the Sewers should create one Mutagen");
    };
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, *mutagen, 0, target_object(counter_target))
        .expect("activate Mutagen at sorcery speed");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&counter_target].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(battlefield_token_oids(&engine, 0, "mutagen").is_empty());
}

#[test]
fn issue_192_alliance_triggers_only_for_another_controlled_creature() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["mutant_town_musicians", "epf_point_squad", "grizzly_bears"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(192_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let musicians = relocate_to_battlefield(&mut engine, 0, "mutant_town_musicians", false);
    let squad = relocate_to_battlefield(&mut engine, 0, "epf_point_squad", false);
    relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    assert!(
        engine.state.stack.is_empty(),
        "opponent entry does not trigger Alliance"
    );

    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast controlled creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(musicians), Some(3));
    assert_eq!(
        engine.state.objects[&squad].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn issue_192_punk_frogs_ward_counters_the_exact_targeting_spell() {
    let decks = Some(vec![
        deck_with("island", &["unsummon"]),
        deck_with("forest", &["punk_frogs"]),
    ]);
    let mut engine = GameEngine::new(192_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let frogs = relocate_to_battlefield(&mut engine, 1, "punk_frogs", false);
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    let spell = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &cast_spell(slot, target_object(frogs)))
        .expect("target Punk Frogs");
    assert_eq!(engine.state.stack.len(), 2, "Ward is above Unsummon");
    pass_both_players(&mut engine);
    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward payment")
        .continuation
        .mana_payment()
        .expect("mana payment");
    assert_eq!(payment.generic_mana_cost, 3);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ward");
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&frogs].zone, Zone::Battlefield);
}

#[test]
fn issue_192_april_rejects_blockers_with_power_three_or_greater() {
    let decks = Some(vec![
        deck_with("plains", &["april_oneil,_kunoichi_trainee"]),
        deck_with("forest", &["grizzly_bears", "hill_giant"]),
    ]);
    let mut engine = GameEngine::new(192_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let april = move_ready_to_battlefield(&mut engine, 0, "april_oneil,_kunoichi_trainee");
    pass_both_players(&mut engine);
    let scry_cards = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("April scry")
        .presentation
        .candidates
        .to_vec();
    engine
        .apply_command(0, &submit_resolution_choice(scry_cards))
        .expect("bottom April's scry cohort");
    let bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let giant = relocate_to_battlefield(&mut engine, 1, "hill_giant", false);

    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![april]))
        .expect("attack with April");
    engine.apply_command(0, &pass()).expect("attacker passes");
    let blockers = engine.apply_command(1, &pass()).expect("defender passes");
    let legal = &blockers.legal_by_player[&1].legal_block_pairs;
    assert!(legal
        .iter()
        .any(|pair| pair.attacker_id == april && pair.blocker_id == bear));
    assert!(legal
        .iter()
        .all(|pair| pair.attacker_id != april || pair.blocker_id != giant));
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: april,
                blocker_id: giant,
            }]),
        )
        .expect_err("power-three creature cannot block April");
}

#[test]
fn issue_192_featherbrained_filcher_leaves_and_creates_working_food() {
    let decks = Some(vec![
        deck_with("island", &["featherbrained_filcher", "unsummon"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(192_006, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let filcher = relocate_to_battlefield(&mut engine, 0, "featherbrained_filcher", false);
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(filcher)))
        .expect("bounce Filcher");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&filcher].zone, Zone::Hand);
    let foods = battlefield_token_oids(&engine, 0, "food");
    let [food] = foods.as_slice() else {
        panic!("Filcher should create one Food");
    };

    engine.state.players[0].life = 17;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, *food, 0, vec![]).expect("activate Food");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
}

#[test]
fn issue_192_tenderize_and_bot_bashing_use_shared_damage_and_exile_pipelines() {
    let decks = Some(vec![
        deck_with(
            "forest",
            &["tenderize", "bot_bashing_time", "grizzly_bears"],
        ),
        deck_with("forest", &["colossal_dreadmaw", "grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(192_007, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let recipient = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    ensure_in_hand(&mut engine, 0, "tenderize");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let tenderize = hand_index_for_card(&engine, 0, "tenderize");
    engine
        .apply_command(0, &cast_spell(tenderize, bite_targets(source, recipient)))
        .expect("cast Tenderize");
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("source")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&recipient].damage, 3);

    let exile_target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "bot_bashing_time");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let bot_bashing = hand_index_for_card(&engine, 0, "bot_bashing_time");
    engine
        .apply_command(0, &cast_spell(bot_bashing, target_object(exile_target)))
        .expect("cast Bot Bashing Time");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&exile_target].zone, Zone::Exile);
}

#[test]
fn issue_192_skateboard_taps_then_attaches_power_and_haste() {
    let decks = Some(vec![
        deck_with("mountain", &["skateboard", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(192_008, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let creature = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine
        .state
        .objects
        .get_mut(&creature)
        .unwrap()
        .summoning_sick = true;
    let permanent = relocate_to_battlefield(&mut engine, 1, "forest", false);
    ensure_in_hand(&mut engine, 0, "skateboard");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "skateboard");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Skateboard");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(permanent))
        .expect("choose permanent to tap");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&permanent].tapped);

    let skateboard = battlefield_object_for_card(&engine, 0, "skateboard");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, skateboard, 0, target_object(creature))
        .expect("equip Skateboard");
    pass_both_players(&mut engine);
    assert_eq!(engine.effective_power(creature), Some(3));
    assert!(engine.effective_has_keyword(creature, Keyword::Haste));
    assert_eq!(
        engine.state.objects[&skateboard].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
}
