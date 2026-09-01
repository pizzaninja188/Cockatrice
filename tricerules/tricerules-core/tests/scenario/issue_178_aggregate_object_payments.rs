use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, CostChoiceKind, CostObjectRef, CostObjectRefs, CostSelection,
    ObjectContributionKind,
};

fn object_ref(engine: &GameEngine, object_id: u32) -> CostObjectRef {
    CostObjectRef {
        object_id,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
    }
}

fn aggregate_selection(
    cost_index: u32,
    objects: Vec<CostObjectRef>,
    graveyard: bool,
) -> CostSelection {
    let objects = CostObjectRefs { objects };
    CostSelection {
        cost_index,
        selection: Some(if graveyard {
            Selection::GraveyardObjects(objects)
        } else {
            Selection::BattlefieldObjects(objects)
        }),
    }
}

fn relocate_to_graveyard(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = take_oid_from_library_or_hand(engine, player, card_id);
    engine.state.players[player].graveyard.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Graveyard;
    object_id
}

#[test]
fn forensic_researcher_publishes_and_pays_mana_value_aggregate() {
    let mut engine = GameEngine::new(
        178_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "island",
                &["forensic_researcher", "lightning_bolt", "grizzly_bears"],
            ),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let researcher = relocate_to_battlefield(&mut engine, 0, "forensic_researcher", false);
    let bolt = relocate_to_graveyard(&mut engine, 0, "lightning_bolt");
    let bear = relocate_to_graveyard(&mut engine, 0, "grizzly_bears");
    let opposing_bear = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);

    let legal = engine.initial_response_batch();
    let key = (u64::from(researcher) << 32) | 1;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&key];
    assert!(choices.non_mana_costs_payable, "{choices:?}");
    let aggregate = choices
        .choices
        .iter()
        .find(|choice| choice.cost_index == 1)
        .expect("Collect Evidence aggregate choice");
    assert_eq!(aggregate.kind(), CostChoiceKind::Exile);
    let constraint = aggregate
        .aggregate_minimum
        .as_ref()
        .expect("aggregate threshold");
    assert_eq!(constraint.minimum, 3);
    assert_eq!(
        constraint.contribution_kind(),
        ObjectContributionKind::ManaValue
    );
    assert_eq!(
        aggregate
            .candidate_objects
            .iter()
            .map(|candidate| {
                (
                    candidate.object.as_ref().unwrap().object_id,
                    candidate.contribution,
                )
            })
            .collect::<Vec<_>>(),
        vec![(bolt, 1), (bear, 2)]
    );

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                researcher,
                1,
                target_object(opposing_bear),
                vec![aggregate_selection(
                    1,
                    vec![object_ref(&engine, bolt), object_ref(&engine, bear)],
                    true,
                )],
            ),
        )
        .expect("pay the aggregate graveyard cost atomically");
    assert!(engine.state.objects[&researcher].tapped);
    assert_eq!(engine.state.objects[&bolt].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Exile);
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert!(engine.state.objects[&opposing_bear].tapped);
}

#[test]
fn mossbridge_troll_publishes_and_pays_current_power_aggregate() {
    let mut engine = GameEngine::new(
        178_002,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "forest",
                &["mossbridge_troll", "colossal_dreadmaw", "serra_angel"],
            ),
            deck_with("island", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let troll = relocate_to_battlefield(&mut engine, 0, "mossbridge_troll", false);
    let dreadmaw = relocate_to_battlefield(&mut engine, 0, "colossal_dreadmaw", false);
    let angel = relocate_to_battlefield(&mut engine, 0, "serra_angel", false);

    let legal = engine.initial_response_batch();
    let key = u64::from(troll) << 32;
    let choices = &legal.legal_by_player[&0].cost_choices_by_ability[&key];
    assert!(choices.non_mana_costs_payable, "{choices:?}");
    let aggregate = &choices.choices[0];
    assert_eq!(aggregate.kind(), CostChoiceKind::Tap);
    assert!(!aggregate.candidate_ids.contains(&troll));
    let constraint = aggregate.aggregate_minimum.as_ref().unwrap();
    assert_eq!(constraint.minimum, 10);
    assert_eq!(
        constraint.contribution_kind(),
        ObjectContributionKind::CurrentPower
    );
    assert_eq!(
        aggregate
            .candidate_objects
            .iter()
            .map(|candidate| candidate.contribution)
            .sum::<i64>(),
        10
    );

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                troll,
                0,
                vec![],
                vec![aggregate_selection(
                    0,
                    vec![object_ref(&engine, dreadmaw), object_ref(&engine, angel)],
                    false,
                )],
            ),
        )
        .expect("tap creatures with total current power ten");
    assert!(!engine.state.objects[&troll].tapped);
    assert!(engine.state.objects[&dreadmaw].tapped);
    assert!(engine.state.objects[&angel].tapped);
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    assert_eq!(engine.characteristics(troll).unwrap().power, Some(25));
}
