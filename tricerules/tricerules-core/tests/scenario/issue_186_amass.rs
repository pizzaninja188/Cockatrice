use super::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, CounterKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
};
use tricerules_cards::CardRegistry;
use tricerules_core::state::{AffectedScope, ContinuousEffect};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("mountain".to_string(), 20).collect(),
        std::iter::repeat_n("forest".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put_flunkies_trigger_on_stack(engine: &mut GameEngine) {
    inject_card_into_hand(engine, 0, "goblin-town_flunkies");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "goblin-town_flunkies");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Goblin-town Flunkies");
    pass_both_players(engine);
    assert!(engine
        .state
        .stack
        .last()
        .is_some_and(|item| item.is_triggered));
}

fn seed_army_counter(engine: &mut GameEngine, object_id: u32) {
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("Army")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
}

#[test]
fn issue_186_registers_the_unblocked_amass_consumers() {
    let registry = CardRegistry::global();

    for card_id in ["goblin-town_flunkies", "misty_mountains_raider"] {
        assert!(
            registry.get(card_id).is_some(),
            "issue #186 requires {card_id} to be implemented"
        );
    }

    for token_id in ["goblin_army_b_0_0", "zombie_army_b_0_0"] {
        assert!(registry.is_token(token_id), "missing {token_id}");
    }
}

#[test]
fn no_army_creates_the_right_token_then_amasses_one() {
    let mut engine = engine(186_001);
    put_flunkies_trigger_on_stack(&mut engine);
    pass_both_players(&mut engine);

    let armies = battlefield_token_oids(&engine, 0, "goblin_army_b_0_0");
    let [army] = armies.as_slice() else {
        panic!("Amass Goblins must create exactly one Goblin Army");
    };
    let object = &engine.state.objects[army];
    assert_eq!(object.owner, 0);
    assert_eq!(object.controller, 0);
    assert_eq!(object.counter_count(CounterKind::PlusOnePlusOne), 1);
    let characteristics = engine.characteristics(*army).expect("Army characteristics");
    assert!(characteristics.is_creature());
    assert!(characteristics.has_type("Army"));
    assert!(characteristics.has_type("Goblin"));
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(1), Some(1))
    );
}

#[test]
fn one_existing_army_is_chosen_automatically_and_keeps_both_subtypes() {
    let mut engine = engine(186_002);
    let army = inject_creature_on_battlefield(&mut engine, 0, "zombie_army_b_0_0");
    seed_army_counter(&mut engine, army);

    put_flunkies_trigger_on_stack(&mut engine);
    pass_both_players(&mut engine);

    assert!(battlefield_token_oids(&engine, 0, "goblin_army_b_0_0").is_empty());
    assert_eq!(
        engine.state.objects[&army].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    let characteristics = engine.characteristics(army).expect("Army characteristics");
    assert!(characteristics.has_type("Army"));
    assert!(characteristics.has_type("Zombie"));
    assert!(characteristics.has_type("Goblin"));
}

#[test]
fn added_army_subtype_is_generation_bound_and_clears_on_departure() {
    let mut engine = engine(186_009);
    let army = inject_creature_on_battlefield(&mut engine, 0, "zombie_army_b_0_0");
    seed_army_counter(&mut engine, army);
    put_flunkies_trigger_on_stack(&mut engine);
    pass_both_players(&mut engine);
    assert!(engine
        .characteristics(army)
        .expect("Army characteristics")
        .has_type("Goblin"));

    inject_card_into_hand(&mut engine, 0, "unsummon");
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(slot, target_object(army)))
        .expect("cast Unsummon");
    pass_both_players(&mut engine);

    assert!(!engine.state.continuous_effects.iter().any(
        |effect| matches!(effect.affected, AffectedScope::Single(candidate) if candidate == army)
    ));
}

#[test]
fn multiple_armies_use_a_public_generation_bound_choice() {
    let mut engine = engine(186_003);
    let goblin = inject_creature_on_battlefield(&mut engine, 0, "goblin_army_b_0_0");
    let zombie = inject_creature_on_battlefield(&mut engine, 0, "zombie_army_b_0_0");
    seed_army_counter(&mut engine, goblin);
    seed_army_counter(&mut engine, zombie);

    put_flunkies_trigger_on_stack(&mut engine);
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("multiple Armies require a choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::PermanentObjects
    );
    assert_eq!(pending.presentation.candidates, vec![goblin, zombie]);

    engine
        .apply_command(0, &submit_resolution_choice(vec![zombie]))
        .expect("choose Zombie Army");
    assert_eq!(
        engine.state.objects[&goblin].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        engine.state.objects[&zombie].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert!(engine
        .characteristics(zombie)
        .expect("chosen Army")
        .has_type("Goblin"));
}

#[test]
fn stale_army_choice_is_rejected_without_losing_the_pending_choice() {
    let mut engine = engine(186_004);
    let first = inject_creature_on_battlefield(&mut engine, 0, "goblin_army_b_0_0");
    let second = inject_creature_on_battlefield(&mut engine, 0, "zombie_army_b_0_0");
    seed_army_counter(&mut engine, first);
    seed_army_counter(&mut engine, second);
    put_flunkies_trigger_on_stack(&mut engine);
    pass_both_players(&mut engine);

    *engine
        .state
        .zone_change_generation
        .entry(first)
        .or_insert(0) += 1;
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![first]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_command(0, &submit_resolution_choice(vec![second]))
        .expect("other Army remains a valid choice");
}

#[test]
fn changeling_is_an_army_without_client_side_type_inference() {
    let mut engine = engine(186_005);
    let changeling = inject_creature_on_battlefield(&mut engine, 0, "chitinous_graspling");
    put_flunkies_trigger_on_stack(&mut engine);
    pass_both_players(&mut engine);

    assert!(battlefield_token_oids(&engine, 0, "goblin_army_b_0_0").is_empty());
    assert_eq!(
        engine.state.objects[&changeling].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn counter_prohibition_does_not_prevent_the_subtype_addition() {
    let mut engine = engine(186_006);
    let tatterkite = inject_creature_on_battlefield(&mut engine, 0, "tatterkite");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(tatterkite),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: Vec::<PermanentTypeFilter>::new(),
            creature_types: vec!["Army".to_string()],
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    put_flunkies_trigger_on_stack(&mut engine);
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&tatterkite].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    let characteristics = engine.characteristics(tatterkite).expect("Tatterkite");
    assert!(characteristics.has_type("Army"));
    assert!(characteristics.has_type("Goblin"));
}

#[test]
fn token_entry_replacement_choice_resumes_amass_before_finishing() {
    let mut engine = engine(186_007);
    put_flunkies_trigger_on_stack(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");

    pass_both_players(&mut engine);
    let application = {
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("CR 616 replacement choice");
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ReplacementEffect
        );
        pending.presentation.candidates[0]
    };
    engine
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .expect("choose an entry replacement");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    let armies = battlefield_token_oids(&engine, 0, "goblin_army_b_0_0");
    let [army] = armies.as_slice() else {
        panic!("replacement resumption must not duplicate the Army token");
    };
    assert!(engine.state.objects[army].tapped);
    assert_eq!(
        engine.state.objects[army].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn misty_mountains_raider_amasses_two_when_its_controller_attacks() {
    let mut engine = engine(186_008);
    let army = inject_creature_on_battlefield(&mut engine, 0, "zombie_army_b_0_0");
    seed_army_counter(&mut engine, army);
    let raider = inject_creature_on_battlefield(&mut engine, 0, "misty_mountains_raider");

    engine
        .apply_command(0, &primitive_yield())
        .expect("advance to beginning of combat");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    engine
        .apply_command(0, &declare_attackers(vec![raider]))
        .expect("attack with Misty Mountains Raider");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&army].counter_count(CounterKind::PlusOnePlusOne),
        3
    );
    assert!(engine
        .characteristics(army)
        .expect("Army characteristics")
        .has_type("Goblin"));
}
