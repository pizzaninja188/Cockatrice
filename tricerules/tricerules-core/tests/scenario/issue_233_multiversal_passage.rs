use super::helpers::*;
use tricerules_cards::primitives::{BasicLandType, ContinuousEffectKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{self as rv1, ResolutionChoiceDecision};

fn branch(index: u32, decision: ResolutionChoiceDecision) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: decision as i32,
            selected_branch_index: index,
            cast_spell: None,
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: Vec::new(),
        })),
    }
}

fn engine(starting_life: i32) -> GameEngine {
    let decks = Some(vec![
        vec!["multiversal_passage".into(); 7],
        vec!["forest".into(); 7],
    ]);
    let mut engine =
        GameEngine::new(233_001, &[0, 1], starting_life, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn ability_texts(engine: &mut GameEngine, oid: u32) -> Vec<String> {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == oid)
                .map(|object| {
                    object
                        .activated_abilities
                        .iter()
                        .map(|ability| ability.text.clone())
                        .collect()
                }),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn each_basic_land_type_is_chosen_before_payment_and_publishes_its_intrinsic_ability() {
    let cases = [
        ("Plains", "{T}: Add {W}."),
        ("Island", "{T}: Add {U}."),
        ("Swamp", "{T}: Add {B}."),
        ("Mountain", "{T}: Add {R}."),
        ("Forest", "{T}: Add {G}."),
    ];
    for (index, (land_type, mana_text)) in cases.into_iter().enumerate() {
        let mut engine = engine(20);
        let hand_index = hand_index_for_card(&engine, 0, "multiversal_passage");
        let object_id = engine.state.players[0].hand[hand_index];
        let played = engine
            .apply_command(0, &play_land(hand_index))
            .expect("play Passage");
        let type_choice = find_resolution_choice(&played).expect("type choice");
        assert_eq!(type_choice.choice_kind(), rv1::ChoiceKind::ResolutionBranch);
        assert_eq!(type_choice.min, 1);
        assert_eq!(type_choice.max, 1);
        assert_eq!(
            type_choice
                .resolution_branches
                .iter()
                .map(|option| option.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Plains", "Island", "Swamp", "Mountain", "Forest"]
        );
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Hand);

        let offered = engine
            .apply_command(
                0,
                &branch(index as u32, ResolutionChoiceDecision::SelectBranch),
            )
            .expect("choose type");
        let payment = find_resolution_choice(&offered).expect("payment choice");
        assert_eq!(payment.min, 0);
        assert_eq!(payment.max, 1);
        assert_eq!(payment.resolution_branches[0].label, "Pay 2 life");
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Hand);

        engine
            .apply_command(0, &branch(0, ResolutionChoiceDecision::Decline))
            .expect("decline payment");
        assert_eq!(engine.state.players[0].life, 20);
        assert!(engine.state.objects[&object_id].tapped);
        let characteristics = engine.characteristics(object_id).expect("characteristics");
        assert!(characteristics.has_type("Land"));
        assert!(characteristics.has_type(land_type));
        assert!(!characteristics
            .supertypes
            .iter()
            .any(|value| value == "Basic"));
        for other in ["Plains", "Island", "Swamp", "Mountain", "Forest"] {
            assert_eq!(characteristics.has_type(other), other == land_type);
        }
        assert_eq!(ability_texts(&mut engine, object_id), vec![mana_text]);
        assert!(zone_view_rules_annotation_labels(&mut engine, 0, object_id)
            .contains(&format!("Chosen basic land type: {land_type}")));
    }
}

#[test]
fn paying_two_life_enters_untapped_and_revalidates_the_type_reply() {
    let mut engine = engine(20);
    let hand_index = hand_index_for_card(&engine, 0, "multiversal_passage");
    let object_id = engine.state.players[0].hand[hand_index];
    engine
        .apply_command(0, &play_land(hand_index))
        .expect("offer type choice");

    assert!(engine
        .apply_command(1, &branch(0, ResolutionChoiceDecision::SelectBranch))
        .is_err());
    assert!(engine
        .apply_command(0, &branch(5, ResolutionChoiceDecision::SelectBranch))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Hand);

    engine
        .apply_command(0, &branch(1, ResolutionChoiceDecision::SelectBranch))
        .expect("choose Island");
    engine
        .apply_command(0, &branch(0, ResolutionChoiceDecision::SelectBranch))
        .expect("pay life");
    assert_eq!(engine.state.players[0].life, 18);
    assert!(!engine.state.objects[&object_id].tapped);
    assert!(engine
        .characteristics(object_id)
        .unwrap()
        .has_type("Island"));
}

#[test]
fn orb_of_dreams_can_be_ordered_on_either_side_of_the_combined_passage_replacement() {
    for choose_orb_first in [false, true] {
        let mut engine = engine(20);
        inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
        let hand_index = hand_index_for_card(&engine, 0, "multiversal_passage");
        let object_id = engine.state.players[0].hand[hand_index];
        let played = engine
            .apply_command(0, &play_land(hand_index))
            .expect("play land");
        let ordering = find_resolution_choice(&played).expect("replacement ordering");
        assert_eq!(ordering.choice_kind(), rv1::ChoiceKind::ReplacementEffect);
        let wanted = if choose_orb_first {
            "Orb of Dreams"
        } else {
            "Multiversal Passage"
        };
        let chosen = ordering
            .candidate_names
            .iter()
            .position(|name| name.starts_with(wanted))
            .expect("replacement label");
        let next = engine
            .apply_command(
                0,
                &submit_resolution_choice(vec![ordering.candidate_object_ids[chosen]]),
            )
            .expect("choose replacement");
        let type_choice = find_resolution_choice(&next).expect("Passage type choice");
        assert_eq!(type_choice.resolution_branches.len(), 5);
        engine
            .apply_command(0, &branch(4, ResolutionChoiceDecision::SelectBranch))
            .expect("choose Forest");
        engine
            .apply_command(0, &branch(0, ResolutionChoiceDecision::SelectBranch))
            .expect("pay life");
        assert_eq!(engine.state.players[0].life, 18);
        assert!(engine.state.objects[&object_id].tapped);
    }
}

#[test]
fn basic_land_setting_replaces_old_land_subtypes_and_copiable_abilities_but_not_grants() {
    let decks = Some(vec![
        vec![
            "watery_grave".into(),
            "gift_of_paradise".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(233_002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let watery_grave = inject_permanent_on_battlefield(&mut engine, 0, "watery_grave");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(watery_grave),
        affected: AffectedScope::Single(watery_grave),
        kind: ContinuousEffectKind::Layer4SetBasicLandType(BasicLandType::Forest),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });

    let characteristics = engine.characteristics(watery_grave).unwrap();
    assert!(characteristics.has_type("Land"));
    assert!(characteristics.has_type("Forest"));
    assert!(!characteristics.has_type("Island"));
    assert!(!characteristics.has_type("Swamp"));
    assert_eq!(
        ability_texts(&mut engine, watery_grave),
        vec!["{T}: Add {G}."]
    );

    ensure_card_in_hand(&mut engine, 0, "gift_of_paradise");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let gift = hand_index_for_card(&engine, 0, "gift_of_paradise");
    engine
        .apply_command(0, &cast_spell(gift, target_object(watery_grave)))
        .expect("cast Gift of Paradise");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        ability_texts(&mut engine, watery_grave),
        vec![
            "{T}: Add {G}.",
            "{T}: Add {W}{W} or {U}{U} or {B}{B} or {R}{R} or {G}{G}."
        ]
    );

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(watery_grave),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index.saturating_add(1),
    });
    assert!(ability_texts(&mut engine, watery_grave).is_empty());
}
