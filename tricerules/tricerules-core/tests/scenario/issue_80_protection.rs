use super::helpers::*;
use tricerules_core::{AttachmentRecipient, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, BlockPair, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("advance to beginning of combat");
    pass_both_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) {
    pass_both_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
}

#[test]
fn protection_from_red_prevents_pyroclasm_damage() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "feat_of_resistance",
                "pyroclasm",
                "pyroclasm",
                "white_knight",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(8001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let knight = relocate_to_battlefield(&mut engine, 0, "white_knight", false);

    ensure_in_hand(&mut engine, 0, "feat_of_resistance");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let feat = hand_index_for_card(&engine, 0, "feat_of_resistance");
    engine
        .apply_command(0, &cast_spell(feat, target_object(knight)))
        .expect("cast Feat of Resistance");
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("protection quality choice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("protection quality choice");
    let tricerules_core::state::ResolutionContinuation::AuthoredBranch { branch, .. } =
        &pending.continuation
    else {
        panic!("authored protection branches")
    };
    let branches = &branch.branches;
    assert_eq!(branches.len(), 5);
    assert_eq!(branches[3].label, "Red");
    engine
        .apply_command(0, &select_branch(3))
        .expect("choose protection from red");
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, knight)
        .contains(&"Protection from red".to_string()));

    ensure_in_hand(&mut engine, 0, "pyroclasm");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let pyroclasm = hand_index_for_card(&engine, 0, "pyroclasm");
    engine
        .apply_command(0, &cast_spell(pyroclasm, Vec::new()))
        .expect("cast Pyroclasm");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&knight].damage, 0,
        "CR 702.16e: protection from red prevents Pyroclasm's red damage"
    );

    engine
        .state
        .damage_prevention_prohibitions
        .push(tricerules_core::state::DamagePreventionProhibition { source_id: None });
    ensure_in_hand(&mut engine, 0, "pyroclasm");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let pyroclasm = hand_index_for_card(&engine, 0, "pyroclasm");
    engine
        .apply_command(0, &cast_spell(pyroclasm, Vec::new()))
        .expect("cast second Pyroclasm");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&knight].damage, 2,
        "CR 101.2: an unpreventable-damage rule overrides protection"
    );
}

#[test]
fn protection_from_black_rejects_black_spell_targeting() {
    let decks = Some(vec![
        deck_with("swamp", &["murder"]),
        deck_with("plains", &["white_knight"]),
    ]);
    let mut engine = GameEngine::new(8002, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let knight = relocate_to_battlefield(&mut engine, 1, "white_knight", false);
    ensure_in_hand(&mut engine, 0, "murder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );

    let murder = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(murder, target_object(knight)))
        .expect_err("protection from black makes White Knight an illegal target");
    assert!(engine.state.stack.is_empty());
}

#[test]
fn protection_from_creatures_rejects_creature_blockers() {
    let mut engine = GameEngine::new(8003, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let chaplain = inject_creature_on_battlefield(&mut engine, 0, "beloved_chaplain");
    let blocker = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![chaplain]))
        .expect("declare Beloved Chaplain");
    pass_to_declare_blockers(&mut engine);

    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: chaplain,
                blocker_id: blocker,
            }]),
        )
        .expect_err("protection from creatures makes the block illegal");
}

#[test]
fn protection_granted_after_blockers_keeps_the_block_and_prevents_damage() {
    let decks = Some(vec![
        deck_with("plains", &["feat_of_resistance", "white_knight"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(8005, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let knight = relocate_to_battlefield(&mut engine, 0, "white_knight", false);
    let blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![knight]))
        .expect("declare White Knight");
    pass_to_declare_blockers(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: knight,
                blocker_id: blocker,
            }]),
        )
        .expect("declare block before protection is granted");

    ensure_in_hand(&mut engine, 0, "feat_of_resistance");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let feat = hand_index_for_card(&engine, 0, "feat_of_resistance");
    engine
        .apply_command(0, &cast_spell(feat, target_object(knight)))
        .expect("cast Feat after blockers");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &select_branch(4))
        .expect("choose protection from green");

    assert_eq!(
        engine.state.combat.as_ref().expect("combat").blockers[&knight],
        vec![blocker],
        "CR 702.16n: gaining protection does not remove an already-declared block"
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&knight].damage, 0,
        "the green blocker cannot damage the protected creature"
    );
}

#[test]
fn protection_from_artifacts_detaches_equipment() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["apostles_blessing", "bonesplitter", "grizzly_bears"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(8004, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let creature = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);

    ensure_in_hand(&mut engine, 0, "bonesplitter");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "bonesplitter");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Bonesplitter");
    pass_both_players(&mut engine);
    let equipment = battlefield_object_for_card(&engine, 0, "bonesplitter");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability(equipment, 0, target_object(creature)))
        .expect("equip creature");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );

    ensure_in_hand(&mut engine, 0, "apostles_blessing");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let blessing = hand_index_for_card(&engine, 0, "apostles_blessing");
    let blessing_oid = engine.state.players[0].hand[blessing];
    engine
        .apply_command(0, &cast_spell(blessing, target_object(creature)))
        .expect("cast Apostle's Blessing");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&blessing_oid].zone,
        Zone::Stack,
        "a spell remains on the stack while its resolution-time choice is pending"
    );
    assert!(!engine.state.players[0].graveyard.contains(&blessing_oid));
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("protection quality choice");
    let tricerules_core::state::ResolutionContinuation::AuthoredBranch { branch, .. } =
        &pending.continuation
    else {
        panic!("authored protection branches")
    };
    let branches = &branch.branches;
    assert_eq!(branches.len(), 6);
    assert_eq!(branches[0].label, "artifacts");
    engine
        .apply_command(0, &select_branch(0))
        .expect("choose protection from artifacts");

    assert_eq!(engine.state.objects[&blessing_oid].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&equipment].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
    assert_eq!(engine.effective_power(creature), Some(2));
}
