//! Existing-card regressions for the selected Myr Battlesphere damage prerequisites.
use super::helpers::*;
use tricerules_cards::{
    Color, ContinuousEffectKind, ControllerReference, EffectDuration, Keyword, PermanentTypeFilter,
    TypeLineAddition,
};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;

fn grant(engine: &mut GameEngine, oid: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn ready(owner: usize) -> (GameEngine, u32) {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        513_101,
        &[0, 1, 2],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_under_foreign_control(&mut engine, owner, 0, "scorch_spitter");
    engine.apply_command(0, &primitive_yield()).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    (engine, source)
}

fn attack(engine: &mut GameEngine, source: u32, kind: TargetRefKind, target: u32) {
    let assignment = engine.initial_response_batch().legal_by_player[&0]
        .legal_attack_assignments
        .iter()
        .find(|assignment| {
            assignment.attacker_object_id == source
                && assignment.defender.as_ref().is_some_and(|defender| {
                    defender.kind == kind as i32 && defender.object_id == target
                })
        })
        .cloned()
        .expect("engine-published attack");
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(rv1::DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .unwrap();
    assert_eq!(engine.state.stack.last().unwrap().card_id, "scorch_spitter");
    assert_eq!(engine.state.stack.last().unwrap().controller, 0);
}

#[test]
fn attacked_damage_lifelink_uses_current_source_controller_not_trigger_controller() {
    let (mut engine, source) = ready(0);
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
    );
    attack(&mut engine, source, TargetRefKind::Player, 1);
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(2),
        },
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(
        engine.state.players[0].life, 20,
        "frozen trigger controller gets no lifelink"
    );
    assert_eq!(
        engine.state.players[2].life, 21,
        "damage source's current controller gains life"
    );
}

#[test]
fn foreign_trigger_survives_source_owner_departure_with_source_qualities() {
    let (mut engine, source) = ready(2);
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
    );
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Deathtouch),
    );
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
    );
    attack(&mut engine, source, TargetRefKind::Player, 1);
    engine.apply_command(2, &concede()).unwrap();
    assert!(!engine.state.objects.contains_key(&source));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "controller 0's trigger survives owner 2"
    );
    assert_eq!(
        engine
            .state
            .last_known_controller_by_generation
            .get(&(source, 0)),
        Some(&0)
    );
    assert!(
        engine.state.last_known_keywords_by_generation[&(source, 0)].contains(&Keyword::Lifelink)
    );
    assert!(
        engine.state.last_known_keywords_by_generation[&(source, 0)].contains(&Keyword::Deathtouch)
    );
    assert_eq!(
        engine.state.last_known_colors_by_generation[&(source, 0)],
        vec![Color::Blue]
    );
    assert!(engine.state.last_known_types_by_generation[&(source, 0)].contains(&"Creature".into()));
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(engine.state.players[1].life, 19);
}

#[test]
fn attacked_animated_planeswalker_takes_source_deathtouch() {
    let (mut engine, source) = ready(0);
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Deathtouch),
    );
    let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .set_counter(tricerules_cards::primitives::CounterKind::Loyalty, 3);
    grant(
        &mut engine,
        walker,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            ..Default::default()
        }),
    );
    grant(
        &mut engine,
        walker,
        ContinuousEffectKind::Layer7bSetPt {
            power: 4,
            toughness: 4,
        },
    );
    attack(&mut engine, source, TargetRefKind::Permanent, walker);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&walker].zone,
        Zone::Graveyard,
        "one noncombat damage is lethal to the animated PW from deathtouch"
    );
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn departed_attacked_player_receives_no_damage_from_surviving_trigger() {
    let (mut engine, source) = ready(0);
    grant(
        &mut engine,
        source,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
    );
    attack(&mut engine, source, TargetRefKind::Player, 1);
    engine.apply_command(1, &concede()).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 20,
        "departed player is absent"
    );
    assert_eq!(
        engine.state.players[0].life, 20,
        "no phantom damage or lifelink"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn parked_attacked_damage_finishes_when_recipient_chooser_concedes() {
    let (mut engine, source) = ready(0);
    attack(&mut engine, source, TargetRefKind::Player, 1);
    engine.state.add_damage_prevention_shield(1, 1);
    engine.state.add_damage_prevention_shield(1, 1);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        1
    );
    let batch = engine.apply_command(1, &concede()).unwrap();
    assert!(
        engine.state.pending_resolution.is_none(),
        "no departed chooser remains"
    );
    assert!(engine.state.blocking_choice().is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[1].life, 20);
    assert!(batch.events.iter().all(|event| !matches!(event.ev,
        Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(ref choice)) if choice.deciding_player_id == 1)));
}
