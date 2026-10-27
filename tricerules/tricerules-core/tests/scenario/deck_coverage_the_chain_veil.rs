//! Actual Chain Veil casts and activation history; synthetic setup is identified below.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{AbilityCost, ActivationLimit, CounterKind};
use tricerules_cards::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineReplacement,
};
use tricerules_core::{AffectedScope, ContinuousEffect, TurnStep, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const VEIL: &str = "the_chain_veil";

fn game(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get(VEIL).is_some(),
        "The Chain Veil must be registered"
    );
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("plains", &[]), deck_with("island", &[])]),
        true,
    )
    .expect("create the Chain Veil scenario");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_veil(engine: &mut GameEngine) -> u32 {
    let veil = inject_card_into_hand(engine, 0, VEIL);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, VEIL);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast The Chain Veil for {4}");
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&veil].zone, Zone::Battlefield);
    veil
}

fn fixture_jace(engine: &mut GameEngine, owner: usize) -> u32 {
    // Jace and its initial loyalty are synthetic setup; the abilities are real commands.
    let jace = inject_permanent_on_battlefield(engine, owner, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&jace)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 3);
    jace
}

fn dev_move(engine: &mut GameEngine, owner: i32, name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move the fixture card through the engine");
}

fn advance_to_end_step(engine: &mut GameEngine) {
    for _ in 0..10 {
        if engine.state.turn_step == TurnStep::EndStep {
            break;
        }
        let actor = engine.state.active_player_id();
        engine
            .apply_scenario_command(actor, &primitive_yield())
            .expect("advance to end step");
    }
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

fn choose_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn set_fixture_controller(engine: &mut GameEngine, object: u32, from: usize, to: usize) {
    // Synthetic control change preserves the exact physical incarnation.
    engine.state.players[from]
        .battlefield
        .retain(|id| *id != object);
    engine.state.players[to].battlefield.push(object);
    let player = engine.state.players[to].id;
    let object = engine.state.objects.get_mut(&object).unwrap();
    object.controller = player;
    object.base_controller = player;
}

fn resolve_veil_permission(engine: &mut GameEngine, veil: u32) {
    give_mana(
        engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(engine, 0, veil, 0, vec![])
        .expect("pay {4} and tap Veil to activate its permission");
    assert!(engine.state.objects[&veil].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    pass_priority_round(engine);
}

fn cast_oath_with_fixture_target(engine: &mut GameEngine) {
    // The Forest and mana are synthetic setup; Oath's cast, target and entry are actual commands.
    let target = inject_permanent_on_battlefield(engine, 0, "forest");
    inject_card_into_hand(engine, 0, "oath_of_teferi");
    give_mana(
        engine,
        0,
        ManaGift {
            c: 3,
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "oath_of_teferi");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Oath");
    pass_priority_round(engine);
    engine
        .apply_scenario_command(0, &choose_targets(target_object(target)))
        .expect("choose Oath's other permanent");
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
}

fn activate_and_resolve(engine: &mut GameEngine, walker: u32) {
    apply_ability(engine, 0, walker, 0, vec![]).expect("activate the real loyalty ability");
    pass_priority_round(engine);
}

#[test]
fn veil_paid_permission_allows_one_additional_loyalty_activation() {
    fn emitted_flag(batch: &RuledEventBatch, object_id: u32) -> bool {
        batch
            .events
            .iter()
            .rev()
            .find_map(|event| match &event.ev {
                Some(Ev::ZoneView(view)) => view
                    .per_player
                    .iter()
                    .flat_map(|player| &player.battlefield_objects)
                    .find(|object| object.object_id == object_id)
                    .map(|object| object.activated_abilities[0].activatable),
                _ => None,
            })
            .expect("the command publishes the walker's updated ZoneView")
    }
    let mut engine = game(261_027_606);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect_err("ordinary allowance is exhausted");
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, veil, 0, vec![]).expect("activate the paid Veil ability");
    let grant = pass_both_players_with_batch(&mut engine);
    assert!(
        emitted_flag(&grant, jace),
        "grant resolution publishes the new offer"
    );
    let used = apply_ability(&mut engine, 0, jace, 0, vec![]).expect("use the grant");
    assert!(
        !emitted_flag(&used, jace),
        "grant consumption publishes the exhausted offer"
    );
    let settled = pass_both_players_with_batch(&mut engine);
    assert!(
        !emitted_flag(&settled, jace),
        "the offer remains exhausted with an empty stack"
    );
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        7
    );
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("one Veil resolution grants only one extra use");
}

#[test]
fn veil_spent_permission_is_not_refunded_when_oath_enters_later() {
    let mut engine = game(261_027_607);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    resolve_veil_permission(&mut engine, veil);
    activate_and_resolve(&mut engine, jace);
    cast_oath_with_fixture_target(&mut engine);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("Oath does not refund the already-spent Veil permission");
}

#[test]
fn veil_adds_one_permission_after_oaths_two_ordinary_uses() {
    let mut engine = game(261_027_608);
    cast_oath_with_fixture_target(&mut engine);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    activate_and_resolve(&mut engine, jace);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect_err("Oath grants two ordinary uses");
    resolve_veil_permission(&mut engine, veil);
    activate_and_resolve(&mut engine, jace);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        9
    );
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("three real uses exhaust Oath and Veil");
}

#[test]
fn veil_actual_rings_copy_grants_a_second_independent_permission() {
    let mut engine = game(261_027_609);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    inject_permanent_on_battlefield(&mut engine, 0, "rings_of_brighthearth");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, veil, 0, vec![]).expect("activate Veil and trigger actual Rings");
    assert_eq!(engine.state.stack.len(), 2);
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose Rings payment");
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {2} and copy the Veil ability");
    assert!(engine.state.stack.last().unwrap().is_copy);
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    dev_move(&mut engine, 0, "Rings of Brighthearth", DevZone::Graveyard);
    activate_and_resolve(&mut engine, jace);
    activate_and_resolve(&mut engine, jace);
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        9
    );
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("both resolved permissions were spent");
}

#[test]
fn veil_permission_applies_separately_to_walkers_controlled_later() {
    let mut engine = game(261_027_610);
    let veil = cast_veil(&mut engine);
    resolve_veil_permission(&mut engine, veil);
    let jace = fixture_jace(&mut engine, 0);
    let chandra = inject_permanent_on_battlefield(&mut engine, 0, "chandra,_novice_pyromancer");
    engine
        .state
        .objects
        .get_mut(&chandra)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    for walker in [jace, chandra] {
        activate_and_resolve(&mut engine, walker);
        activate_and_resolve(&mut engine, walker);
        apply_ability(&mut engine, 0, walker, 0, vec![])
            .expect_err("each walker uses its permission once");
    }
}

#[test]
fn veil_keeps_timing_costs_and_the_abilitys_own_limit() {
    let mut engine = game(261_027_616);
    let veil = cast_veil(&mut engine);
    resolve_veil_permission(&mut engine, veil);
    let jace = fixture_jace(&mut engine, 0);
    // Synthetic copied face adds a per-ability limit to distinguish that independent restriction.
    let mut face = tricerules_cards::registry::global()
        .get("jace_beleren")
        .unwrap()
        .primary_face()
        .clone();
    face.activated_abilities[0].activation_limit =
        Some(ActivationLimit::PerTurn { max_activations: 1 });
    engine.state.objects.get_mut(&jace).unwrap().copiable_values =
        Some(tricerules_core::state::CopiableValues {
            prototyped: false,
            source_card_id: "jace_beleren".into(),
            source_face_index: 0,
            face,
            room_faces: None,
            display_name: "Synthetic individual-limit fixture".into(),
        });
    engine.state.turn_step = TurnStep::EndStep;
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("Veil does not change sorcery timing");
    engine.state.turn_step = TurnStep::Main1;
    apply_ability(&mut engine, 0, jace, 2, target_player(1))
        .expect_err("Veil does not pay loyalty costs");
    activate_and_resolve(&mut engine, jace);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("the same ability's own limit remains");
    apply_ability(&mut engine, 0, jace, 1, target_player(0))
        .expect("a different ability can use Veil's unspent permission");
    pass_priority_round(&mut engine);
    apply_ability(&mut engine, 0, jace, 1, target_player(0))
        .expect_err("the permission has now been used");
}

#[test]
fn veil_rechecks_its_intervening_condition_when_the_trigger_resolves() {
    let mut engine = game(261_027_611);
    cast_veil(&mut engine);
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    // Synthetic history change isolates CR 603.4's second check: no instant-speed loyalty
    // activation is admitted by the existing engine at this point in the end step.
    engine
        .state
        .turn_history
        .current
        .player_mut(0)
        .activated_planeswalker_loyalty = true;
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn veil_history_uses_the_sources_derived_type_before_costs_and_survives_sacrifice() {
    let mut nonwalker = game(261_027_612);
    let source = fixture_jace(&mut nonwalker, 0);
    // Synthetic type-setting effect keeps the real Jace ability but makes its source an artifact.
    nonwalker.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Artifact],
            creature_types: vec![],
            land_types: vec![],
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: nonwalker.state.command_index + 1,
    });
    assert!(!nonwalker
        .characteristics(source)
        .unwrap()
        .has_type("Planeswalker"));
    activate_and_resolve(&mut nonwalker, source);
    assert!(
        !nonwalker
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
    nonwalker
        .state
        .continuous_effects
        .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
    assert!(nonwalker
        .characteristics(source)
        .unwrap()
        .has_type("Planeswalker"));
    cast_veil(&mut nonwalker);
    advance_to_end_step(&mut nonwalker);
    assert_eq!(nonwalker.state.stack.len(), 1);
    pass_priority_round(&mut nonwalker);
    assert_eq!(nonwalker.state.players[0].life, 18);

    let mut sacrifice = game(261_027_613);
    let source = fixture_jace(&mut sacrifice, 0);
    // Synthetic copy adds a source-sacrifice cost to Jace's real +2 so the source disappears
    // during cost commitment; the pre-cost planeswalker fact must still be recorded.
    let mut face = tricerules_cards::registry::global()
        .get("jace_beleren")
        .unwrap()
        .primary_face()
        .clone();
    face.activated_abilities[0]
        .costs
        .push(AbilityCost::SacrificeSelf);
    sacrifice
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .copiable_values = Some(tricerules_core::state::CopiableValues {
        prototyped: false,
        source_card_id: "jace_beleren".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name: "Synthetic self-sacrifice fixture".into(),
    });
    activate_and_resolve(&mut sacrifice, source);
    assert_eq!(sacrifice.state.objects[&source].zone, Zone::Graveyard);
    assert!(
        sacrifice
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
    cast_veil(&mut sacrifice);
    advance_to_end_step(&mut sacrifice);
    assert!(sacrifice.state.stack.is_empty());
    assert_eq!(sacrifice.state.players[0].life, 20);
}

#[test]
fn veil_preserves_spent_permission_on_control_and_face_change_but_not_blink() {
    let mut engine = game(261_027_614);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    let generation = engine
        .state
        .zone_change_generation
        .get(&jace)
        .copied()
        .unwrap_or(0);
    activate_and_resolve(&mut engine, jace);
    resolve_veil_permission(&mut engine, veil);
    activate_and_resolve(&mut engine, jace);
    // Synthetic away/back control and face-status changes keep the same incarnation.
    set_fixture_controller(&mut engine, jace, 0, 1);
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    apply_ability(&mut engine, 1, jace, 0, vec![]).expect_err("P0's grant does not authorize P1");
    set_fixture_controller(&mut engine, jace, 1, 0);
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    *engine.state.face_change_generation.entry(jace).or_default() += 1;
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("returning control does not refund a spent grant");
    dev_move(&mut engine, 0, "Jace Beleren", DevZone::Hand);
    dev_move(&mut engine, 0, "Jace Beleren", DevZone::Battlefield);
    assert!(engine.state.zone_change_generation[&jace] > generation);
    activate_and_resolve(&mut engine, jace);
    activate_and_resolve(&mut engine, jace);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("the new incarnation can use this grant once");
}

#[test]
fn veil_rejects_unaffordable_and_tapped_activations_without_creating_grants() {
    let mut engine = game(261_027_620);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    apply_ability(&mut engine, 0, veil, 0, vec![]).expect_err("Veil requires {4}");
    assert!(!engine.state.objects[&veil].tapped);
    assert!(engine.state.stack.is_empty());
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect_err("failed payment created no grant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, veil, 0, vec![]).expect("pay and tap Veil");
    assert!(engine.state.objects[&veil].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, veil, 0, vec![])
        .expect_err("tapped Veil cannot be activated again");
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 4);
    pass_priority_round(&mut engine);
    activate_and_resolve(&mut engine, jace);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("only the paid resolution granted a use");
}

#[test]
fn veil_permission_survives_source_departure_after_activation() {
    let mut engine = game(261_027_621);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, veil, 0, vec![]).expect("activate Veil");
    dev_move(&mut engine, 0, "The Chain Veil", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&veil].zone, Zone::Graveyard);
    pass_priority_round(&mut engine);
    activate_and_resolve(&mut engine, jace);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("the source-independent grant is used once");
}

#[test]
fn veil_countered_activated_ability_creates_no_permission() {
    let mut engine = game(261_027_622);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, veil, 0, vec![]).expect("activate Veil");
    let ability = engine.state.stack.last().unwrap().id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass to the opponent");
    inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 2,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("cast actual Tidebinder");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &choose_targets(vec![TargetRef {
                object_id: ability,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }]),
        )
        .expect("target Veil's activated ability");
    pass_priority_round(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == ability));
    assert!(engine.state.objects[&veil].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    apply_ability(&mut engine, 0, jace, 0, vec![])
        .expect_err("a countered Veil ability creates no grant");
}

#[test]
fn veil_grants_belong_to_actors_and_are_not_refunded_by_control_changes() {
    let mut engine = game(261_027_623);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    resolve_veil_permission(&mut engine, veil);
    // Synthetic control, phase and untap setup tests two actors in the same turn; both Veil
    // activations and both further Jace activations are real accepted commands.
    set_fixture_controller(&mut engine, veil, 0, 1);
    set_fixture_controller(&mut engine, jace, 0, 1);
    engine.state.objects.get_mut(&veil).unwrap().tapped = false;
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 1, veil, 0, vec![]).expect("P1 creates its own grant");
    pass_priority_round(&mut engine);
    apply_ability(&mut engine, 1, jace, 0, vec![])
        .expect("P1 uses its unused grant after P0's earlier use");
    pass_priority_round(&mut engine);
    apply_ability(&mut engine, 1, jace, 0, vec![]).expect_err("P1's grant is now spent");
    set_fixture_controller(&mut engine, jace, 1, 0);
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("P0's separate grant remains unused");
    pass_priority_round(&mut engine);
    set_fixture_controller(&mut engine, jace, 0, 1);
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    apply_ability(&mut engine, 1, jace, 0, vec![])
        .expect_err("returning control does not refund P1's grant");
}

#[test]
fn veil_history_resets_on_the_next_turn_and_penalty_can_trigger_again() {
    let mut engine = game(261_027_624);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    cast_veil(&mut engine);
    let old_turn = engine.state.turn_instance;
    for _ in 0..160 {
        if engine.state.turn_instance > old_turn
            && engine.state.active_player_id() == 0
            && engine.state.turn_step == TurnStep::Main1
        {
            break;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(player) => {
                let index = engine.state.player_idx(player).unwrap();
                let excess = engine.state.players[index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("advance actual turns and cleanup");
    }
    assert!(engine.state.turn_instance > old_turn);
    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    assert!(
        !engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 18);
}

#[test]
fn veil_permissions_expire_at_each_cleanup_including_a_repeated_cleanup() {
    fn fragile_until_cleanup(engine: &mut GameEngine) -> u32 {
        // Synthetic creature/counter and temporary layer-7 setup creates a state-based action
        // when the real cleanup removes the temporary effect.
        let creature = inject_creature_on_battlefield(engine, 0, "grizzly_bears");
        engine
            .state
            .objects
            .get_mut(&creature)
            .unwrap()
            .set_counter(CounterKind::MinusOneMinusOne, 3);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(creature),
            kind: ContinuousEffectKind::Layer7bSetPt {
                power: 4,
                toughness: 4,
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index + 1,
        });
        creature
    }
    fn grants(engine: &GameEngine) -> usize {
        engine.diagnostic_snapshot().unwrap()["state"]["chain_veil_permissions"]
            .as_array()
            .unwrap()
            .len()
    }

    let mut engine = game(261_027_625);
    let veil = cast_veil(&mut engine);
    let jace = fixture_jace(&mut engine, 0);
    activate_and_resolve(&mut engine, jace);
    resolve_veil_permission(&mut engine, veil);
    assert_eq!(grants(&engine), 1);
    let turn = engine.state.turn_instance;
    // Ba Sing Se's printed second ability explicitly requires sorcery timing.
    let sorcery_land = inject_permanent_on_battlefield(&mut engine, 0, "ba_sing_se");
    advance_to_end_step(&mut engine);
    let first = fragile_until_cleanup(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.cleanup_discard_player, Some(0));
    assert!(!engine.state.cleanup_priority_active);
    // Fund and ready the source synthetically so the rejection isolates the missing priority.
    engine.state.objects.get_mut(&veil).unwrap().tapped = false;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    assert!(matches!(
        apply_ability(&mut engine, 0, veil, 0, vec![]),
        Err(tricerules_core::EngineError::Illegal(
            "no abilities during cleanup"
        ))
    ));
    assert!(!engine.state.objects[&veil].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 4);
    assert!(engine.state.stack.is_empty());
    assert_eq!(grants(&engine), 1, "514.2 has not happened yet");
    resolve_cleanup_discards_if_any(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Cleanup);
    assert!(engine.state.cleanup_priority_active);
    assert_eq!(engine.state.turn_instance, turn);
    assert_eq!(engine.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(
        grants(&engine),
        0,
        "the first CR 514.2 boundary expired the grant"
    );
    assert!(
        engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );

    assert!(
        zone_view_ability_flags(&mut engine, 0, veil)[0],
        "Veil is offered with cleanup priority"
    );
    assert!(!zone_view_ability_flags(&mut engine, 0, jace)[0]);
    assert!(!zone_view_ability_flags(&mut engine, 0, sorcery_land)[1]);
    assert!(matches!(
        apply_ability(&mut engine, 0, jace, 0, vec![]),
        Err(tricerules_core::EngineError::Illegal(
            "ability only at sorcery speed"
        ))
    ));
    assert!(matches!(
        apply_ability(&mut engine, 0, sorcery_land, 1, vec![]),
        Err(tricerules_core::EngineError::Illegal(
            "ability only at sorcery speed"
        ))
    ));
    // Synthetic mana funds the real Veil activation in the cleanup priority window.
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, veil, 0, vec![]).expect("activate Veil during cleanup priority");
    pass_priority_round(&mut engine);
    assert_eq!(
        grants(&engine),
        1,
        "the new grant resolved during cleanup priority"
    );
    let second = fragile_until_cleanup(&mut engine);
    pass_priority_round(&mut engine);
    resolve_cleanup_discards_if_any(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Cleanup);
    assert!(engine.state.cleanup_priority_active);
    assert_eq!(
        engine.state.turn_instance, turn,
        "the second SBA created another priority window"
    );
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(
        grants(&engine),
        0,
        "repeated cleanup expired the newly created grant"
    );
    assert!(
        engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
    pass_priority_round(&mut engine);
    assert!(engine.state.turn_instance > turn);
    assert!(
        !engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
}

#[test]
fn veil_parked_cost_cancellation_keeps_grant_and_replays_then_retry_consumes_it() {
    use prost::Message;
    use tricerules_proto::ruled::v1 as rv1;

    fn fixture() -> (GameEngine, u32, u32) {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            261_027_626,
            &[0, 1, 2],
            20,
            None,
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let lattice = inject_card_into_hand(&mut engine, 2, "mycosynth_lattice");
        assert_eq!(
            move_ready_to_battlefield(&mut engine, 2, "mycosynth_lattice"),
            lattice
        );
        let jace = fixture_jace(&mut engine, 0);
        // Synthetic control/phase setup makes P1's real activation consume the ordinary use
        // while leaving P0's personal activation-history fact false.
        set_fixture_controller(&mut engine, jace, 0, 1);
        engine.state.active_player_idx = 1;
        engine.state.priority_idx = 1;
        apply_ability(&mut engine, 1, jace, 0, vec![]).expect("P1 uses the ordinary activation");
        pass_priority_round(&mut engine);
        set_fixture_controller(&mut engine, jace, 1, 0);
        engine.state.active_player_idx = 0;
        engine.state.priority_idx = 0;
        assert!(
            !engine
                .state
                .turn_history
                .current
                .player(0)
                .activated_planeswalker_loyalty
        );
        let veil = cast_veil(&mut engine);
        resolve_veil_permission(&mut engine, veil);
        inject_permanent_on_battlefield(&mut engine, 0, "laezel,_vlaakiths_champion");
        for _ in 0..3 {
            inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        }
        let recipient = inject_creature_with_stats(&mut engine, 0, "ancient_brontodon", 20, 20);
        // A synthetic copied Jace face adds the existing Sting-Slinger mana and Blight costs
        // to its real +2; no external card definition or Program is introduced.
        let registry = tricerules_cards::registry::global();
        let mut face = registry.get("jace_beleren").unwrap().primary_face().clone();
        let mana = registry
            .get("sting-slinger")
            .unwrap()
            .primary_face()
            .activated_abilities[0]
            .costs[0]
            .clone();
        face.activated_abilities[0]
            .costs
            .extend([mana, AbilityCost::Blight { count: 1 }]);
        engine.state.objects.get_mut(&jace).unwrap().copiable_values =
            Some(tricerules_core::state::CopiableValues {
                prototyped: false,
                source_card_id: "jace_beleren".into(),
                source_face_index: 0,
                face,
                room_faces: None,
                display_name: "Synthetic parked loyalty-cost fixture".into(),
            });
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 2,
                r: 1,
                ..Default::default()
            },
        );
        (engine, jace, recipient)
    }
    fn apply_wire(
        first: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
    ) -> RuledEventBatch {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let batch = first
            .apply_scenario_command(actor, &command)
            .expect("accepted original command");
        assert_eq!(
            batch,
            replay
                .apply_scenario_command(actor, &decoded)
                .expect("accepted wire replay")
        );
        batch
    }
    fn activation(engine: &GameEngine, jace: u32, recipient: u32) -> RuledCommand {
        activate_ability_with_costs(
            jace,
            0,
            vec![],
            vec![rv1::CostSelection {
                cost_index: 2,
                selection: Some(rv1::cost_selection::Selection::BattlefieldObjects(
                    rv1::CostObjectRefs {
                        objects: vec![rv1::CostObjectRef {
                            object_id: recipient,
                            zone_change_generation: engine
                                .state
                                .zone_change_generation
                                .get(&recipient)
                                .copied()
                                .unwrap_or(0),
                        }],
                    },
                )),
            }],
        )
    }
    fn finish_choices(first: &mut GameEngine, replay: &mut GameEngine) -> bool {
        let mut logged_cancellation = false;
        for _ in 0..12 {
            let Some(pending) = first.state.pending_resolution.as_ref() else {
                break;
            };
            assert_eq!(
                pending.presentation.choice_kind,
                ChoiceKind::ReplacementEffect
            );
            let mut command = submit_resolution_choice(vec![pending.presentation.candidates[0]]);
            let Some(Cmd::SubmitResolutionChoice(answer)) = command.cmd.as_mut() else {
                unreachable!()
            };
            answer.resolution_prompt_id = pending.resolution_prompt_id.get();
            let batch = apply_wire(first, replay, 0, command);
            logged_cancellation |= batch.events.iter().any(|event| {
                matches!(&event.ev, Some(Ev::Log(log)) if log.text.to_ascii_lowercase().contains("cancel"))
            });
        }
        assert!(
            first.state.pending_resolution.is_none(),
            "all replacement choices completed"
        );
        logged_cancellation
    }
    fn spent(engine: &GameEngine) -> usize {
        engine.diagnostic_snapshot().unwrap()["state"]["chain_veil_permissions"][0]["used_walkers"]
            .as_array()
            .unwrap()
            .len()
    }

    let (mut engine, jace, recipient) = fixture();
    let (mut replay, replay_jace, replay_recipient) = fixture();
    assert_eq!((jace, recipient), (replay_jace, replay_recipient));
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    let command = activation(&engine, jace, recipient);
    apply_wire(&mut engine, &mut replay, 0, command);
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(spent(&engine), 0);
    assert!(
        !engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
    apply_wire(&mut engine, &mut replay, 2, concede());
    assert!(
        finish_choices(&mut engine, &mut replay),
        "the stale off-color plan is accepted as a cancellation"
    );
    assert!(engine.state.blocking_choice().is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::MinusOneMinusOne),
        0
    );
    assert_eq!(
        (
            engine.state.players[0].mana_pool.blue,
            engine.state.players[0].mana_pool.red
        ),
        (2, 1)
    );
    assert_eq!(
        spent(&engine),
        0,
        "cancelled costs did not consume the selected grant"
    );
    assert!(
        !engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );

    let retry = activation(&engine, jace, recipient);
    apply_wire(&mut engine, &mut replay, 0, retry);
    assert!(engine.state.pending_resolution.is_some());
    assert!(!finish_choices(&mut engine, &mut replay));
    assert_eq!(engine.state.stack.len(), 1);
    assert!(engine.state.objects[&jace].counter_count(CounterKind::Loyalty) > 5);
    assert!(engine.state.objects[&recipient].counter_count(CounterKind::MinusOneMinusOne) > 0);
    assert!(
        engine
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
    assert_eq!(
        spent(&engine),
        1,
        "the successful retry consumes the original grant exactly once"
    );
    let actor = engine.state.priority_player_id();
    apply_wire(&mut engine, &mut replay, actor, pass());
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn veil_activation_history_replays_from_accepted_wire_commands() {
    use prost::Message;
    fn apply_wire(
        first: &mut GameEngine,
        replay: &mut GameEngine,
        actor: i32,
        command: RuledCommand,
    ) {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let event = first
            .apply_scenario_command(actor, &command)
            .expect("accepted original command");
        assert_eq!(
            event,
            replay
                .apply_scenario_command(actor, &decoded)
                .expect("accepted replay command")
        );
    }
    let mut first = game(261_027_615);
    let mut replay = game(261_027_615);
    let jace = fixture_jace(&mut first, 0);
    assert_eq!(jace, fixture_jace(&mut replay, 0));
    let command = activate_ability_for(&first, jace, 0, vec![]);
    apply_wire(&mut first, &mut replay, 0, command);
    for _ in 0..2 {
        let actor = first.state.priority_player_id();
        apply_wire(&mut first, &mut replay, actor, pass());
    }
    first.enable_dev_commands();
    replay.enable_dev_commands();
    apply_wire(
        &mut first,
        &mut replay,
        0,
        RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: 0,
                dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                    card_name: "Jace Beleren".into(),
                    zone: DevZone::Graveyard as i32,
                    ready: true,
                })),
            })),
        },
    );
    assert_eq!(first.state.objects[&jace].zone, Zone::Graveyard);
    assert!(
        first
            .state
            .turn_history
            .current
            .player(0)
            .activated_planeswalker_loyalty
    );
    let veil = inject_card_into_hand(&mut first, 0, VEIL);
    assert_eq!(veil, inject_card_into_hand(&mut replay, 0, VEIL));
    give_mana(
        &mut first,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    give_mana(
        &mut replay,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&first, 0, VEIL);
    apply_wire(&mut first, &mut replay, 0, cast_spell(slot, vec![]));
    for _ in 0..2 {
        let actor = first.state.priority_player_id();
        apply_wire(&mut first, &mut replay, actor, pass());
    }
    for _ in 0..10 {
        if first.state.turn_step == TurnStep::EndStep {
            break;
        }
        let actor = first.state.active_player_id();
        apply_wire(&mut first, &mut replay, actor, primitive_yield());
    }
    assert_eq!(first.state.turn_step, TurnStep::EndStep);
    assert!(first.state.stack.is_empty());
    assert_eq!(first.state.players[0].life, 20);
    assert_eq!(
        first.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}

#[test]
fn veil_loses_two_at_its_controllers_end_step_and_trigger_survives_source_departure() {
    let mut engine = game(261_027_600);
    let veil = cast_veil(&mut engine);
    advance_to_end_step(&mut engine);
    let trigger = engine
        .state
        .stack
        .last()
        .expect("the controller's end-step trigger")
        .id;
    dev_move(&mut engine, 0, "The Chain Veil", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&veil].zone, Zone::Graveyard);
    assert!(engine.state.stack.iter().any(|item| item.id == trigger));
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[1].life, 20);
    assert!(engine.state.stack.is_empty());

    let mut opponent_turn = game(261_027_601);
    let other_veil = cast_veil(&mut opponent_turn);
    set_fixture_controller(&mut opponent_turn, other_veil, 0, 1);
    advance_to_end_step(&mut opponent_turn);
    assert!(
        opponent_turn.state.stack.is_empty(),
        "P1's Veil does not trigger at P0's end step"
    );
    assert_eq!(opponent_turn.state.players[0].life, 20);
    assert_eq!(opponent_turn.state.players[1].life, 20);
}

#[test]
fn veil_remembers_a_prior_countered_loyalty_activation_after_its_walker_leaves() {
    let mut engine = game(261_027_602);
    let jace = fixture_jace(&mut engine, 0);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("activate Jace's +2 before Veil enters");
    let ability = engine.state.stack.last().unwrap().id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass to the opponent");
    inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 2,
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("cast Tidebinder");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &choose_targets(vec![TargetRef {
                object_id: ability,
                kind: TargetRefKind::Stack as i32,
                ..Default::default()
            }]),
        )
        .expect("target the loyalty ability");
    pass_priority_round(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == ability));
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        5
    );
    dev_move(&mut engine, 0, "Jace Beleren", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&jace].zone, Zone::Graveyard);
    cast_veil(&mut engine);
    advance_to_end_step(&mut engine);
    assert!(
        engine.state.stack.is_empty(),
        "the successful activation prevents the trigger"
    );
    assert_eq!(engine.state.players[0].life, 20);
}

#[test]
fn veil_does_not_count_another_players_loyalty_activation() {
    let mut engine = game(261_027_603);
    let jace = fixture_jace(&mut engine, 0);
    apply_ability(&mut engine, 0, jace, 0, vec![]).expect("P0 activates Jace");
    pass_priority_round(&mut engine);
    let veil = cast_veil(&mut engine);
    set_fixture_controller(&mut engine, veil, 0, 1);
    // Synthetic phase ownership keeps the same turn history while testing P1's own end step.
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    advance_to_end_step(&mut engine);
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .expect("P1's end-step trigger")
            .controller,
        1
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[1].life, 18);
    assert_eq!(engine.state.players[0].life, 20);
}

#[test]
fn veil_does_not_count_a_rejected_loyalty_activation() {
    let mut engine = game(261_027_604);
    let jace = fixture_jace(&mut engine, 0);
    apply_ability(&mut engine, 0, jace, 2, target_player(1))
        .expect_err("Jace cannot pay ten loyalty from three");
    assert_eq!(
        engine.state.objects[&jace].counter_count(CounterKind::Loyalty),
        3
    );
    assert!(engine.state.stack.is_empty());
    cast_veil(&mut engine);
    advance_to_end_step(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 18);
}

#[test]
fn veil_does_not_count_a_loyalty_ability_copy_as_its_controllers_activation() {
    let mut engine = game(261_027_605);
    cast_veil(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 1, "rings_of_brighthearth");
    let jace = fixture_jace(&mut engine, 1);
    // Synthetic active-player setup permits P1's real loyalty activation in this turn history.
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 1, jace, 0, vec![]).expect("P1 activates Jace and triggers Rings");
    assert_eq!(engine.state.stack.len(), 2);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_scenario_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose the Rings payment");
    submit_mana_resolution_decision(&mut engine, 1, ResolutionChoiceDecision::PayMana)
        .expect("pay for the actual Rings copy");
    assert!(engine.state.stack.last().unwrap().is_copy);
    // Synthetic copy control isolates P0 resolving a copied ability from P1's actual activation.
    engine.state.stack.last_mut().unwrap().controller = 0;
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    advance_to_end_step(&mut engine);
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .expect("P0 did not activate a loyalty ability")
            .controller,
        0
    );
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.players[0].life, 18);
}
