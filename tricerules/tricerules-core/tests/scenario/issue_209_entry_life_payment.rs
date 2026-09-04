use super::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{self as rv1, ResolutionChoiceDecision};

fn branch_choice(decision: ResolutionChoiceDecision) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: decision as i32,
            selected_branch_index: 0,
            cast_spell: None,
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: Vec::new(),
        })),
    }
}

fn watery_grave_engine(starting_life: i32) -> GameEngine {
    let decks = Some(vec![
        vec!["watery_grave".into(); 7],
        vec!["forest".into(); 7],
    ]);
    let mut engine =
        GameEngine::new(209_001, &[0, 1], starting_life, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn watery_grave_waits_for_life_payment_before_entering() {
    let mut engine = watery_grave_engine(20);
    let hand_index = hand_index_for_card(&engine, 0, "watery_grave");
    let object_id = engine.state.players[0].hand[hand_index];

    let played = engine
        .apply_command(0, &play_land(hand_index))
        .expect("play Watery Grave");

    assert_eq!(engine.state.objects[&object_id].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].life, 20);
    let choice = find_resolution_choice(&played).expect("entry payment choice");
    assert_eq!(choice.choice_kind(), rv1::ChoiceKind::ResolutionBranch);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.min, 0);
    assert_eq!(choice.max, 1);
    assert_eq!(choice.resolution_branches.len(), 1);
    assert!(choice.resolution_branches[0].selectable);

    let completed = engine
        .apply_command(0, &branch_choice(ResolutionChoiceDecision::SelectBranch))
        .expect("pay 2 life");

    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&object_id].tapped);
    assert!(completed.events.iter().any(|event| matches!(
        event.ev,
        Some(Ev::PermanentMoved(ref moved))
            if moved.object_id == object_id
                && moved.destination == rv1::permanent_moved::Destination::Battlefield as i32
    )));
}

#[test]
fn declining_or_becoming_unable_to_pay_makes_watery_grave_enter_tapped() {
    let mut declined = watery_grave_engine(20);
    let hand_index = hand_index_for_card(&declined, 0, "watery_grave");
    let object_id = declined.state.players[0].hand[hand_index];
    declined
        .apply_command(0, &play_land(hand_index))
        .expect("offer payment");
    declined
        .apply_command(0, &branch_choice(ResolutionChoiceDecision::Decline))
        .expect("decline payment");
    assert_eq!(declined.state.players[0].life, 20);
    assert_eq!(declined.state.objects[&object_id].zone, Zone::Battlefield);
    assert!(declined.state.objects[&object_id].tapped);

    let mut unable = watery_grave_engine(1);
    let hand_index = hand_index_for_card(&unable, 0, "watery_grave");
    let object_id = unable.state.players[0].hand[hand_index];
    let played = unable
        .apply_command(0, &play_land(hand_index))
        .expect("offer unaffordable payment");
    let choice = find_resolution_choice(&played).expect("entry payment choice");
    assert!(!choice.resolution_branches[0].selectable);
    assert!(unable
        .apply_command(0, &branch_choice(ResolutionChoiceDecision::SelectBranch),)
        .is_err());
    assert!(unable.state.pending_resolution.is_some());
    assert_eq!(unable.state.objects[&object_id].zone, Zone::Hand);
    unable
        .apply_command(0, &branch_choice(ResolutionChoiceDecision::Decline))
        .expect("decline unaffordable payment");
    assert_eq!(unable.state.players[0].life, 1);
    assert!(unable.state.objects[&object_id].tapped);
}

#[test]
fn entry_life_payment_revalidates_player_payload_and_current_life() {
    let mut engine = watery_grave_engine(2);
    let hand_index = hand_index_for_card(&engine, 0, "watery_grave");
    let object_id = engine.state.players[0].hand[hand_index];
    engine
        .apply_command(0, &play_land(hand_index))
        .expect("offer payment");

    assert!(engine
        .apply_command(1, &branch_choice(ResolutionChoiceDecision::SelectBranch),)
        .is_err());
    assert!(engine.state.pending_resolution.is_some());

    let mut malformed = branch_choice(ResolutionChoiceDecision::SelectBranch);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!();
    };
    answer.chosen_object_ids.push(object_id);
    assert!(engine.apply_command(0, &malformed).is_err());
    assert!(engine.state.pending_resolution.is_some());

    engine.state.players[0].life = 1;
    assert!(engine
        .apply_command(0, &branch_choice(ResolutionChoiceDecision::SelectBranch),)
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Hand);
    engine
        .apply_command(0, &branch_choice(ResolutionChoiceDecision::Decline))
        .expect("decline after affordability changed");
    assert!(engine.state.objects[&object_id].tapped);
}

#[test]
fn paying_never_clears_orb_of_dreams_tapped_entry_in_either_order() {
    for choose_orb_first in [false, true] {
        let mut engine = watery_grave_engine(20);
        inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
        let hand_index = hand_index_for_card(&engine, 0, "watery_grave");
        let object_id = engine.state.players[0].hand[hand_index];
        let played = engine
            .apply_command(0, &play_land(hand_index))
            .expect("play Watery Grave with Orb present");
        let ordering = find_resolution_choice(&played).expect("CR 616 ordering choice");
        assert_eq!(ordering.choice_kind(), rv1::ChoiceKind::ReplacementEffect);
        assert_eq!(ordering.candidate_names.len(), 2);
        let wanted = if choose_orb_first {
            "Orb of Dreams"
        } else {
            "Watery Grave"
        };
        let index = ordering
            .candidate_names
            .iter()
            .position(|name| name.starts_with(wanted))
            .expect("replacement label");
        let payment = engine
            .apply_command(
                0,
                &submit_resolution_choice(vec![ordering.candidate_object_ids[index]]),
            )
            .expect("choose replacement order");
        let choice = find_resolution_choice(&payment).expect("life payment choice");
        assert_eq!(choice.choice_kind(), rv1::ChoiceKind::ResolutionBranch);
        engine
            .apply_command(0, &branch_choice(ResolutionChoiceDecision::SelectBranch))
            .expect("pay life");

        assert_eq!(engine.state.players[0].life, 18);
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Battlefield);
        assert!(engine.state.objects[&object_id].tapped);
    }
}

#[test]
fn an_entry_copy_rechecks_the_copied_watery_grave_ability() {
    let decks = Some(vec![
        vec![
            "watery_grave".into(),
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(209_002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let watery_index = hand_index_for_card(&engine, 0, "watery_grave");
    engine
        .apply_command(0, &play_land(watery_index))
        .expect("play Watery Grave");
    engine
        .apply_command(0, &branch_choice(ResolutionChoiceDecision::Decline))
        .expect("decline first payment");
    let watery_grave = battlefield_object_for_card(&engine, 0, "watery_grave");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(watery_grave),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec!["Shapeshifter".into()],
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let clone_index = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone_index, Vec::new()))
        .expect("cast Clone");
    pass_both_players(&mut engine);
    let copy_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copy source choice");
    assert_eq!(
        copy_choice.presentation.choice_kind,
        rv1::ChoiceKind::CopySource
    );
    assert!(copy_choice.presentation.candidates.contains(&watery_grave));
    engine
        .apply_command(0, &submit_resolution_choice(vec![watery_grave]))
        .expect("copy Watery Grave");

    let payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copied entry payment");
    assert_eq!(
        payment.presentation.choice_kind,
        rv1::ChoiceKind::ResolutionBranch
    );
    engine
        .apply_command(0, &branch_choice(ResolutionChoiceDecision::SelectBranch))
        .expect("pay for copied entry ability");

    assert_eq!(engine.state.players[0].life, 18);
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert!(!engine.state.objects[&clone].tapped);
    assert_eq!(engine.state.objects[&clone].copy_revision, 1);
}
