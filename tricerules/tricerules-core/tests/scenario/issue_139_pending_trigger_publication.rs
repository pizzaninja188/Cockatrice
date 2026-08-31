use crate::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, TriggerCondition, TriggeredAbilityDef,
};
use tricerules_cards::CardRegistry;
use tricerules_core::state::PendingTrigger;
use tricerules_core::{AffectedScope, ContinuousEffect};

fn targeted_graveyard_trigger(trigger: TriggerCondition) -> TriggeredAbilityDef {
    let mut ability = CardRegistry::global()
        .get("gravedigger")
        .expect("Gravedigger definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = trigger;
    ability
}

fn published_graveyard_targets(
    engine: &mut GameEngine,
    source_id: u32,
    ability_index: usize,
) -> Vec<u32> {
    let key = (u64::from(source_id) << 32) | ability_index as u64;
    engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability[&key].groups[0]
        .valid_graveyard_ids
        .clone()
}

#[test]
fn issue_139_refresh_publishes_targets_for_granted_trigger_beyond_printed_abilities() {
    let mut engine = GameEngine::new(139_001, &[0, 1], 20, None, true).expect("new engine");
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let graveyard_creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let ability = targeted_graveyard_trigger(TriggerCondition::WheneverSelfAttacks {
        minimum_other_attackers: 0,
    });
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    advance_to_declare_attackers(&mut engine);
    let trigger_batch = engine
        .apply_command(0, &declare_attackers(vec![source]))
        .expect("declare the granted-trigger source as an attacker");
    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("granted targeted trigger is pending");
    assert_eq!(
        pending.ability_index, 0,
        "the first granted ability follows the source's empty printed ability list"
    );
    let event_targets = trigger_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::TriggerNeedsTarget(prompt)) => prompt.targets.as_ref(),
            _ => None,
        })
        .expect("initial trigger prompt publishes targets");
    assert_eq!(
        event_targets.groups[0].valid_graveyard_ids,
        vec![graveyard_creature]
    );

    assert_eq!(
        published_graveyard_targets(&mut engine, source, 0),
        vec![graveyard_creature],
        "a reconnect snapshot must publish from the stored granted ability"
    );
    let key = u64::from(source) << 32;
    assert!(
        !engine.initial_response_batch().legal_by_player[&1]
            .valid_targets_by_ability
            .contains_key(&key),
        "only the trigger controller receives its target candidates"
    );

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(graveyard_creature),
                })),
            },
        )
        .expect("choose the published graveyard target");
    pass_both_players(&mut engine);
    assert!(engine.state.players[0].hand.contains(&graveyard_creature));
}

#[test]
fn issue_139_refresh_publishes_targets_from_stored_non_primary_face_ability() {
    let mut engine = GameEngine::new(139_002, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "village_ironsmith_ironfang");
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("transforming source")
        .face_up_index = 1;
    engine.state.face_change_generation.insert(source, 1);
    let graveyard_creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let illegal_battlefield_target =
        inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let ability = targeted_graveyard_trigger(TriggerCondition::WhenSelfEntersBattlefield);
    let trigger_id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    engine.state.pending_triggers.push_back(PendingTrigger {
        object_id: trigger_id,
        source_permanent_id: source,
        source_face_index: 1,
        source_zone_change: engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0),
        source_face_change: 1,
        ability_index: 0,
        ability: ability.clone(),
        ability_text: ability.fallback_text("Village Ironsmith"),
        presentation: None,
        card_id: "village_ironsmith_ironfang".into(),
        controller: 0,
        may: ability.may,
        trigger_context: Default::default(),
    });

    assert_eq!(
        published_graveyard_targets(&mut engine, source, 0),
        vec![graveyard_creature],
        "a reconnect snapshot must not substitute the primary face's ability at index zero"
    );

    let rejected = engine.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                decline: false,
                selected_modes: Vec::new(),
                targets: target_object(illegal_battlefield_target),
            })),
        },
    );
    assert!(
        rejected.is_err(),
        "submission validation remains engine-authoritative"
    );
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "an illegal target must not consume the pending trigger"
    );

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(graveyard_creature),
                })),
            },
        )
        .expect("the stored face-one ability validates its legal target");
    pass_both_players(&mut engine);
    assert!(engine.state.players[0].hand.contains(&graveyard_creature));
}
