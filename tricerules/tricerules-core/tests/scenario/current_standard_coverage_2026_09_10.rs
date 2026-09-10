use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand,
    SubmitResolutionChoice,
};

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

#[test]
fn heated_argument_is_registered_in_the_current_standard_audit() {
    let definition = tricerules_cards::CardRegistry::global()
        .get("heated_argument")
        .expect("Heated Argument is in the current-Standard coverage cohort");
    assert_eq!(definition.name, "Heated Argument");
    assert_eq!(definition.primary_face().spell_effect.len(), 2);
}

#[test]
fn environmental_scientist_search_is_optional_private_and_basic_only() {
    let decks = Some(vec![
        deck_with("forest", &["environmental_scientist"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(910_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "environmental_scientist");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "environmental_scientist");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Environmental Scientist");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    let branch = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("optional search branch");
    assert!(branch.presentation.candidates.is_empty());
    engine
        .apply_command(0, &select_branch(0))
        .expect("choose to search");
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("private library choice");
    assert!(!choice.presentation.candidates.is_empty());
    assert!(choice
        .presentation
        .candidates
        .iter()
        .all(|oid| engine.state.objects[oid].card_id == "forest"));
    let forest = choice.presentation.candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose basic land");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Hand);
}

#[test]
fn hire_a_crew_pumps_its_token_and_only_controlled_creatures() {
    let decks = Some(vec![
        deck_with("mountain", &["hire_a_crew", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(910_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let friendly = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let opposing = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "hire_a_crew");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "hire_a_crew");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Hire a Crew");
    resolve_entire_stack_two_player(&mut engine);

    let villains = battlefield_token_oids(&engine, 0, "villain_b_2_1_menace");
    let [villain] = villains.as_slice() else {
        panic!("Hire a Crew should create one Villain");
    };
    assert_eq!(engine.effective_power(*villain), Some(3));
    assert!(engine.effective_has_keyword(*villain, Keyword::Menace));
    assert_eq!(engine.effective_power(friendly), Some(3));
    assert_eq!(engine.effective_power(opposing), Some(2));
}

#[test]
fn front_porch_sentries_rejects_friendly_target_and_uses_death_lki() {
    let decks = Some(vec![
        deck_with(
            "swamp",
            &["front_porch_sentries", "murder", "grizzly_bears"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(910_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let sentries = relocate_to_battlefield(&mut engine, 0, "front_porch_sentries", false);
    let friendly = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let opposing = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    ensure_in_hand(&mut engine, 0, "murder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "murder");
    engine
        .apply_command(0, &cast_spell(slot, target_object(sentries)))
        .expect("cast Murder");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine.apply_command(1, &pass()).expect("Murder resolves");
    assert_eq!(engine.state.objects[&sentries].zone, Zone::Graveyard);
    assert!(engine
        .apply_command(0, &choose_trigger_target(friendly))
        .is_err());
    engine
        .apply_command(0, &choose_trigger_target(opposing))
        .expect("choose opposing creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(opposing), Some(1));
    assert_eq!(engine.effective_toughness(opposing), Some(1));
}

#[test]
fn mountain_kings_return_recruit_creates_a_token_only_for_a_nonland_discard() {
    fn resolve(discard_nonland: bool) -> GameEngine {
        let decks = Some(vec![
            deck_with("plains", &["the_mountain-kings_return", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(
            910_010 + u64::from(discard_nonland),
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        let discard_id = if discard_nonland {
            "grizzly_bears"
        } else {
            "plains"
        };
        ensure_in_hand(&mut engine, 0, discard_id);
        let discard_oid = engine.state.players[0].hand[hand_index_for_card(&engine, 0, discard_id)];
        move_ready_to_battlefield(&mut engine, 0, "the_mountain-kings_return");
        pass_both_players(&mut engine);
        let choice = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Recruit discard choice");
        assert!(choice.presentation.candidates.contains(&discard_oid));
        engine
            .apply_command(0, &submit_resolution_choice(vec![discard_oid]))
            .expect("discard for Recruit");
        assert_eq!(engine.state.objects[&discard_oid].zone, Zone::Graveyard);
        engine
    }

    assert_eq!(
        battlefield_token_oids(&resolve(true), 0, "human_soldier_w_1_1").len(),
        1
    );
    assert!(battlefield_token_oids(&resolve(false), 0, "human_soldier_w_1_1").is_empty());
}
