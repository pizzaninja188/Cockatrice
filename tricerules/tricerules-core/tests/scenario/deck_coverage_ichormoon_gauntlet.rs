use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::AbilityCost;
use tricerules_cards::{registry, CounterKind, Layout, ManaCost};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    dev_command, ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, DevCommand, DevMoveCard,
    DevZone, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

const ICHORMOON_GAUNTLET: &str = "ichormoon_gauntlet";

fn main_phase(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new Ichormoon scenario game");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
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

fn cast_bolt(engine: &mut GameEngine, caster: i32, defender: i32) {
    inject_card_into_hand(engine, caster as usize, "lightning_bolt");
    give_mana(
        engine,
        caster,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, caster as usize, "lightning_bolt");
    engine
        .apply_scenario_command(caster, &cast_spell(slot, target_player(defender)))
        .expect("cast Lightning Bolt");
}

fn move_ichormoon(engine: &mut GameEngine, owner: i32, zone: DevZone) {
    move_named_card(engine, owner, "Ichormoon Gauntlet", zone);
}

fn move_named_card(engine: &mut GameEngine, owner: i32, card_name: &str, zone: DevZone) {
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap_or_else(|_| panic!("move {card_name} through the engine"));
}

fn abilities_for_player(
    engine: &mut GameEngine,
    viewer: i32,
    object_id: u32,
) -> Vec<tricerules_proto::ruled::v1::AbilityInfo> {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| {
            let Some(Ev::ZoneView(view)) = &event.ev else {
                return None;
            };
            view.per_player
                .iter()
                .find(|player| player.player_id == viewer)
                .and_then(|player| {
                    player
                        .battlefield_objects
                        .iter()
                        .find(|object| object.object_id == object_id)
                })
                .map(|object| object.activated_abilities.clone())
        })
        .expect("the viewer's battlefield catalog contains the requested permanent")
}

fn ichormoon_grants(
    engine: &mut GameEngine,
    viewer: i32,
    recipient: u32,
) -> Vec<tricerules_proto::ruled::v1::AbilityInfo> {
    abilities_for_player(engine, viewer, recipient)
        .into_iter()
        .filter(|ability| {
            ability
                .presentation
                .as_ref()
                .is_some_and(|presentation| presentation.card_id == ICHORMOON_GAUNTLET)
        })
        .collect()
}

fn ichormoon_grant_with_cost(
    engine: &mut GameEngine,
    viewer: i32,
    recipient: u32,
    cost_label: &str,
) -> tricerules_proto::ruled::v1::AbilityInfo {
    ichormoon_grants(engine, viewer, recipient)
        .into_iter()
        .find(|ability| ability.cost_label == cost_label)
        .unwrap_or_else(|| panic!("missing Ichormoon grant with cost {cost_label:?}"))
}

fn choose_proliferate_objects(objects: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: objects,
            decision: ResolutionChoiceDecision::Unspecified as i32,
            ..Default::default()
        })),
    }
}

fn resolve_ichormoon_proliferate(engine: &mut GameEngine, chosen_object: u32) {
    let resolving = pass_both_players_with_batch(engine);
    let choice = find_resolution_choice(&resolving)
        .expect("the granted zero-loyalty ability resolves to a Proliferate choice");
    assert_eq!(choice.choice_kind, ChoiceKind::Proliferate as i32);
    engine
        .apply_scenario_command(0, &choose_proliferate_objects(vec![chosen_object]))
        .expect("choose one permanent for Proliferate");
}

fn create_actual_teferi_emblem(engine: &mut GameEngine) -> u32 {
    let teferi = inject_permanent_on_battlefield(engine, 0, "teferi,_temporal_archmage");
    engine
        .state
        .objects
        .get_mut(&teferi)
        .expect("injected registered Teferi")
        .set_counter(CounterKind::Loyalty, 10);
    let ultimate = activate_ability_for(engine, teferi, 2, vec![]);
    engine
        .apply_scenario_command(0, &ultimate)
        .expect("activate Teferi, Temporal Archmage's actual -10 ability");
    assert_eq!(engine.state.objects[&teferi].zone, Zone::Graveyard);
    pass_both_players(engine);
    let emblem = engine
        .state
        .static_emblems
        .iter()
        .find(|emblem| emblem.controller == 0)
        .expect("the actual Teferi ultimate creates its controller's emblem");
    assert_eq!(
        emblem.effects,
        [tricerules_cards::primitives::StaticEmblemEffect::PlaneswalkerLoyaltyInstantTiming]
    );
    emblem.object_id
}

fn submit_counter_kind(
    engine: &mut GameEngine,
    player: i32,
    choice: &ResolutionChoiceRequired,
    kind_label: &str,
) {
    assert_eq!(choice.choice_kind, ChoiceKind::ResolutionBranch as i32);
    let branch = choice
        .resolution_branches
        .iter()
        .find(|branch| branch.label.eq_ignore_ascii_case(kind_label))
        .unwrap_or_else(|| {
            panic!(
                "missing offered branch {kind_label:?}: {:?}",
                choice.resolution_branches
            )
        });
    engine
        .apply_scenario_command(
            player,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: branch.branch_index,
                    ..Default::default()
                })),
            },
        )
        .expect("select one offered counter kind");
}

fn cast_ichormoon(engine: &mut GameEngine, previous_provider_trigger_target: Option<u32>) -> u32 {
    let source = inject_card_into_hand(engine, 0, ICHORMOON_GAUNTLET);
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, ICHORMOON_GAUNTLET);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast registered Ichormoon Gauntlet for {2}{U}");

    while !engine.state.pending_triggers.is_empty() {
        let target = previous_provider_trigger_target
            .expect("a previous Gauntlet trigger needs a permanent target");
        engine
            .apply_scenario_command(0, &choose_trigger_target(target))
            .expect("choose a target for the previous Gauntlet's cast trigger");
    }
    if engine.state.stack.iter().any(|item| item.is_triggered) {
        let resolving = pass_both_players_with_batch(engine);
        if engine.state.pending_resolution.is_some() {
            let choice = find_resolution_choice(&resolving)
                .expect("the previous Gauntlet trigger offers a current counter kind");
            assert_eq!(choice.choice_kind, ChoiceKind::ResolutionBranch as i32);
            let branch = choice
                .resolution_branches
                .first()
                .expect("the chosen trigger target has a positive counter")
                .label
                .clone();
            submit_counter_kind(engine, 0, &choice, &branch);
        }
    }
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(
        engine.state.stack.is_empty(),
        "the cast and its existing-provider triggers settle"
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    source
}

fn cast_carth_and_decline_entry_look(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "carth_the_lion");
    give_mana(
        engine,
        0,
        ManaGift {
            b: 1,
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "carth_the_lion");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast registered Carth the Lion for {2}{B}{G}");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);

    if !engine.state.stack.is_empty() || !engine.state.pending_triggers.is_empty() {
        let resolving = pass_both_players_with_batch(engine);
        if engine.state.pending_resolution.is_some() {
            let choice = find_resolution_choice(&resolving)
                .expect("Carth's entry look publishes its optional choice");
            assert_eq!(choice.choice_kind, ChoiceKind::LibraryLook as i32);
            assert_eq!((choice.min, choice.max), (0, 0));
            engine
                .apply_scenario_command(0, &submit_resolution_choice(vec![]))
                .expect("decline Carth's empty planeswalker selection");
        }
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    source
}

fn cast_jace_beleren(engine: &mut GameEngine, player: i32) -> u32 {
    let owner = engine
        .state
        .player_idx(player)
        .expect("the Jace controller is seated");
    let source = inject_card_into_hand(engine, owner, "jace_beleren");
    give_mana(
        engine,
        player,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, owner, "jace_beleren");
    engine
        .apply_scenario_command(player, &cast_spell(slot, vec![]))
        .expect("cast registered Jace Beleren for {1}{U}{U}");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    source
}

#[test]
fn ichormoon_complete_registry_identity_exists() {
    let registry = registry::global();
    assert_eq!(
        registry.id_for_name("Ichormoon Gauntlet"),
        Some(ICHORMOON_GAUNTLET)
    );
    let card = registry
        .get(ICHORMOON_GAUNTLET)
        .expect("complete Ichormoon Gauntlet definition");
    assert_eq!(card.id, ICHORMOON_GAUNTLET);
    assert_eq!(card.name, "Ichormoon Gauntlet");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), ICHORMOON_GAUNTLET);
    assert_eq!(face.mana_cost, ManaCost::parse("{2}{U}").unwrap());
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.activated_abilities.is_empty());
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(face.triggered_abilities.len(), 1);
}

#[test]
fn ichormoon_noncreature_cast_targets_on_stack_then_adds_one_chosen_kind_after_source_leaves() {
    let mut engine = main_phase(26_102_801);
    let source = inject_permanent_on_battlefield(&mut engine, 0, ICHORMOON_GAUNTLET);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    engine
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::Poison, 1);
    cast_bolt(&mut engine, 0, 1);

    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(0, &choose_trigger_target(target))
        .expect("choose a target permanent when the triggered ability is put on the stack");
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    let trigger = engine
        .state
        .stack
        .last()
        .expect("trigger above the cast spell");
    assert_eq!(trigger.source_permanent_id, Some(source));
    assert_eq!(trigger.targets.len(), 1);
    assert_eq!(trigger.targets[0].object_id, target);
    assert_eq!(
        trigger.targets[0].zone_change_generation,
        Some(semantic::generation(&engine, target))
    );

    move_ichormoon(&mut engine, 0, DevZone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    let resolving = pass_both_players_with_batch(&mut engine);
    assert!(
        engine.state.pending_resolution.is_some(),
        "the captured trigger is parked at its active resolution choice"
    );
    let choice = find_resolution_choice(&resolving).unwrap_or_else(|| {
        panic!(
            "captured trigger resolution did not offer a kind; pending={:?}, stack={:?}",
            engine
                .state
                .pending_resolution
                .as_ref()
                .map(|pending| &pending.presentation),
            engine
                .state
                .stack
                .iter()
                .map(|item| (
                    item.card_id.as_str(),
                    item.source_permanent_id,
                    item.targets.as_slice()
                ))
                .collect::<Vec<_>>()
        )
    });
    assert_eq!(choice.resolution_branches.len(), 2);
    assert!(choice
        .resolution_branches
        .iter()
        .any(|branch| branch.label == "Add one charge counter"));
    assert!(choice
        .resolution_branches
        .iter()
        .any(|branch| branch.label == "Add one poison counter"));
    submit_counter_kind(&mut engine, 0, &choice, "Add one poison counter");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Poison),
        2
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "only the cast spell remains after the trigger completes"
    );
    resolve_entire_stack_two_player(&mut engine);
}

#[test]
fn ichormoon_counterless_target_is_legal_and_resolves_without_a_counter_choice() {
    let mut engine = main_phase(26_102_802);
    inject_permanent_on_battlefield(&mut engine, 0, ICHORMOON_GAUNTLET);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    cast_bolt(&mut engine, 0, 1);

    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(0, &choose_trigger_target(target))
        .expect("a permanent with no counters is still a legal target");
    pass_both_players(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.objects[&target].counters.is_empty());
    assert_eq!(engine.state.stack.len(), 1, "only Lightning Bolt remains");
    resolve_entire_stack_two_player(&mut engine);
}

#[test]
fn ichormoon_trigger_filters_out_creature_spells_opponent_casts_and_a_departed_source() {
    let mut creature_case = main_phase(26_102_803);
    inject_permanent_on_battlefield(&mut creature_case, 0, ICHORMOON_GAUNTLET);
    inject_card_into_hand(&mut creature_case, 0, "grizzly_bears");
    give_mana(
        &mut creature_case,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    let bears = hand_index_for_card(&creature_case, 0, "grizzly_bears");
    creature_case
        .apply_scenario_command(0, &cast_spell(bears, vec![]))
        .expect("cast a creature spell");
    assert!(creature_case.state.pending_triggers.is_empty());
    assert_eq!(creature_case.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut creature_case);

    let mut opponent_case = main_phase(26_102_804);
    inject_permanent_on_battlefield(&mut opponent_case, 0, ICHORMOON_GAUNTLET);
    opponent_case
        .apply_scenario_command(0, &pass())
        .expect("pass priority to the opponent");
    cast_bolt(&mut opponent_case, 1, 0);
    assert!(opponent_case.state.pending_triggers.is_empty());
    assert_eq!(opponent_case.state.stack.len(), 1);

    let mut departed_case = main_phase(26_102_805);
    inject_permanent_on_battlefield(&mut departed_case, 0, ICHORMOON_GAUNTLET);
    move_ichormoon(&mut departed_case, 0, DevZone::Graveyard);
    cast_bolt(&mut departed_case, 0, 1);
    assert!(departed_case.state.pending_triggers.is_empty());
    assert_eq!(departed_case.state.stack.len(), 1);
}

#[test]
fn ichormoon_granted_abilities_filter_recipients_share_caps_and_follow_generations() {
    let mut engine = main_phase(26_102_806);
    let source = cast_ichormoon(&mut engine, None);
    let own_walker = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
    let opposing_walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&own_walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);

    let initial_grants = ichormoon_grants(&mut engine, 0, own_walker);
    assert_eq!(
        initial_grants.len(),
        2,
        "one provider grants both abilities; unfiltered offers: {:#?}",
        abilities_for_player(&mut engine, 0, own_walker)
    );
    assert!(initial_grants
        .iter()
        .any(|ability| ability.cost_label == "+0"));
    assert!(initial_grants
        .iter()
        .any(|ability| ability.cost_label == "-12"));
    let proliferate_offer = initial_grants
        .iter()
        .find(|ability| ability.cost_label == "+0")
        .expect("the live +0 grant offer");
    assert_eq!(proliferate_offer.text, "+0: Proliferate.");
    assert_eq!(
        proliferate_offer
            .presentation
            .as_ref()
            .expect("published presentation reference")
            .fallback_text,
        "+0: Proliferate."
    );
    let extra_turn_offer = initial_grants
        .iter()
        .find(|ability| ability.cost_label == "-12")
        .expect("the live -12 grant offer");
    assert_eq!(
        extra_turn_offer.text,
        "-12: Take an extra turn after this one."
    );
    assert_eq!(
        extra_turn_offer
            .presentation
            .as_ref()
            .expect("published presentation reference")
            .fallback_text,
        "-12: Take an extra turn after this one."
    );
    assert!(ichormoon_grants(&mut engine, 0, source).is_empty());
    assert!(ichormoon_grants(&mut engine, 1, opposing_walker).is_empty());
    assert!(ichormoon_grants(&mut engine, 0, own_creature).is_empty());

    let old_source_generation = semantic::generation(&engine, source);
    move_ichormoon(&mut engine, 0, DevZone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(ichormoon_grants(&mut engine, 0, own_walker).is_empty());
    move_ichormoon(&mut engine, 0, DevZone::Battlefield);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_ne!(semantic::generation(&engine, source), old_source_generation);
    assert_eq!(ichormoon_grants(&mut engine, 0, own_walker).len(), 2);

    let second_source = cast_ichormoon(&mut engine, Some(source));
    let duplicate_grants = ichormoon_grants(&mut engine, 0, own_walker);
    let zero_grants = duplicate_grants
        .iter()
        .filter(|ability| ability.cost_label == "+0")
        .collect::<Vec<_>>();
    let ultimate_grants = duplicate_grants
        .iter()
        .filter(|ability| ability.cost_label == "-12")
        .collect::<Vec<_>>();
    assert_eq!(
        zero_grants.len(),
        2,
        "duplicate providers keep two +0 slots"
    );
    assert_eq!(
        ultimate_grants.len(),
        2,
        "duplicate providers keep two -12 slots"
    );
    assert_ne!(
        zero_grants[0].ability_index, zero_grants[1].ability_index,
        "the duplicate grants occupy distinct engine slots"
    );
    assert_ne!(
        ultimate_grants[0].ability_index, ultimate_grants[1].ability_index,
        "the duplicate ultimates occupy distinct engine slots"
    );
    let first_ultimate_index = ultimate_grants[0].ability_index;

    let first_zero = zero_grants[0].ability_index;
    engine
        .apply_scenario_command(
            0,
            &activate_ability_for(&engine, own_walker, first_zero, vec![]),
        )
        .expect("activate one offered Ichormoon zero-loyalty slot");
    assert_eq!(
        engine.state.objects[&own_walker].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .unwrap()
            .activated_ability
            .as_ref()
            .unwrap()
            .costs,
        [AbilityCost::Loyalty(0)],
        "the granted slot retains its actual zero-loyalty cost"
    );
    resolve_ichormoon_proliferate(&mut engine, own_walker);
    assert_eq!(
        engine.state.objects[&own_walker].counter_count(CounterKind::Loyalty),
        6
    );
    assert!(engine.state.stack.is_empty());

    let after_first_use = ichormoon_grants(&mut engine, 0, own_walker);
    let blocked_zero_slots = after_first_use
        .iter()
        .filter(|ability| ability.cost_label == "+0")
        .collect::<Vec<_>>();
    assert_eq!(blocked_zero_slots.len(), 2);

    // Prepared-state funding makes the actual -12 grant affordable without advancing the turn
    // or changing the planeswalker's incarnation after its +0 activation used the shared cap.
    let walker_generation_before_funding = semantic::generation(&engine, own_walker);
    let turn_instance_before_funding = engine.state.turn_instance;
    let turn_before_funding = engine.state.turn;
    engine
        .state
        .objects
        .get_mut(&own_walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 12);
    assert_eq!(
        engine.state.objects[&own_walker].counter_count(CounterKind::Loyalty),
        12,
        "prepared state gives the -12 ability enough loyalty to pay its cost"
    );
    assert_eq!(
        semantic::generation(&engine, own_walker),
        walker_generation_before_funding,
        "prepared-state funding keeps the same planeswalker incarnation"
    );
    assert_eq!(engine.state.turn_instance, turn_instance_before_funding);
    assert_eq!(engine.state.turn, turn_before_funding);
    let funded_ultimate_index = ichormoon_grants(&mut engine, 0, own_walker)
        .into_iter()
        .find(|ability| {
            ability.cost_label == "-12" && ability.ability_index == first_ultimate_index
        })
        .map(|ability| ability.ability_index)
        .expect("the actual -12 grant remains in its stable slot");
    let before_affordable_ultimate_attempt = engine.diagnostic_snapshot().unwrap();
    assert!(
        engine
            .apply_scenario_command(
                0,
                &activate_ability_for(&engine, own_walker, funded_ultimate_index, vec![]),
            )
            .is_err(),
        "the shared loyalty cap rejects the affordable -12 after this planeswalker used +0"
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before_affordable_ultimate_attempt,
        "the affordable -12 rejection leaves prepared game state unchanged"
    );
    assert_eq!(
        semantic::generation(&engine, own_walker),
        walker_generation_before_funding,
        "the rejected -12 keeps the same planeswalker incarnation"
    );
    assert_eq!(engine.state.turn_instance, turn_instance_before_funding);
    assert_eq!(engine.state.turn, turn_before_funding);

    let before_second_provider_attempt = engine.diagnostic_snapshot().unwrap();
    assert!(
        engine
            .apply_scenario_command(
                0,
                &activate_ability_for(
                    &engine,
                    own_walker,
                    blocked_zero_slots[1].ability_index,
                    vec![]
                ),
            )
            .is_err(),
        "two providers cannot bypass the recipient's shared loyalty cap"
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before_second_provider_attempt,
        "the rejected duplicate-provider activation is atomic"
    );

    move_ichormoon(&mut engine, 0, DevZone::Graveyard);
    let reentered_provider = if engine.state.objects[&source].zone == Zone::Graveyard {
        source
    } else {
        second_source
    };
    assert_eq!(
        engine.state.objects[&reentered_provider].zone,
        Zone::Graveyard
    );
    let provider_generation = semantic::generation(&engine, reentered_provider);
    move_ichormoon(&mut engine, 0, DevZone::Battlefield);
    assert_eq!(
        engine.state.objects[&reentered_provider].zone,
        Zone::Battlefield
    );
    assert_ne!(
        semantic::generation(&engine, reentered_provider),
        provider_generation
    );
    let after_provider_reentry = ichormoon_grants(&mut engine, 0, own_walker);
    let zero_slots_after_provider_reentry = after_provider_reentry
        .iter()
        .filter(|ability| ability.cost_label == "+0")
        .collect::<Vec<_>>();
    assert_eq!(zero_slots_after_provider_reentry.len(), 2);
    assert!(
        zero_slots_after_provider_reentry
            .iter()
            .all(|ability| !ability.activatable),
        "a provider's new zone incarnation does not reset the recipient's spent allowance"
    );

    let old_recipient_generation = semantic::generation(&engine, own_walker);
    move_named_card(&mut engine, 0, "Jace Beleren", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&own_walker].zone, Zone::Graveyard);
    move_named_card(&mut engine, 0, "Jace Beleren", DevZone::Battlefield);
    assert_eq!(engine.state.objects[&own_walker].zone, Zone::Battlefield);
    assert_ne!(
        semantic::generation(&engine, own_walker),
        old_recipient_generation
    );
    engine
        .state
        .objects
        .get_mut(&own_walker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    let fresh_generation_grants = ichormoon_grants(&mut engine, 0, own_walker);
    let fresh_zero_slots = fresh_generation_grants
        .iter()
        .filter(|ability| ability.cost_label == "+0")
        .collect::<Vec<_>>();
    assert_eq!(fresh_zero_slots.len(), 2);
    assert!(
        fresh_zero_slots.iter().all(|ability| ability.activatable),
        "a returned recipient incarnation has a fresh loyalty allowance"
    );
    engine
        .apply_scenario_command(
            0,
            &activate_ability_for(
                &engine,
                own_walker,
                fresh_zero_slots[0].ability_index,
                vec![],
            ),
        )
        .expect("the new recipient incarnation can use a granted slot this turn");
    resolve_ichormoon_proliferate(&mut engine, own_walker);
    assert_eq!(
        engine.state.objects[&own_walker].counter_count(CounterKind::Loyalty),
        6
    );
}

#[test]
fn ichormoon_minus_twelve_uses_recipient_loyalty_and_is_captured_without_its_provider() {
    let mut engine = main_phase(26_102_807);
    let recipient = cast_jace_beleren(&mut engine, 0);
    let source = cast_ichormoon(&mut engine, None);
    engine
        .state
        .objects
        .get_mut(&recipient)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 11);

    let too_small = ichormoon_grant_with_cost(&mut engine, 0, recipient, "-12");
    let before_rejection = engine.diagnostic_snapshot().unwrap();
    assert!(
        engine
            .apply_scenario_command(
                0,
                &activate_ability_for(&engine, recipient, too_small.ability_index, vec![])
            )
            .is_err(),
        "eleven recipient loyalty cannot pay the actual -12 cost"
    );
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_rejection);

    engine
        .state
        .objects
        .get_mut(&recipient)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 12);
    let payable = ichormoon_grant_with_cost(&mut engine, 0, recipient, "-12");
    assert!(payable.activatable);
    engine
        .apply_scenario_command(
            0,
            &activate_ability_for(&engine, recipient, payable.ability_index, vec![]),
        )
        .expect("activate the granted -12 loyalty ability with twelve counters");
    assert_eq!(
        engine.state.objects[&recipient].counter_count(CounterKind::Loyalty),
        0
    );
    assert_eq!(engine.state.objects[&recipient].zone, Zone::Graveyard);
    let activation_id = engine
        .state
        .stack
        .last()
        .expect("captured ability on stack")
        .id;
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .unwrap()
            .activated_ability
            .as_ref()
            .unwrap()
            .costs,
        [AbilityCost::Loyalty(-12)]
    );

    move_ichormoon(&mut engine, 0, DevZone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == activation_id));
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());

    let normal_turn = engine.state.turn;
    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.state.active_player_id(),
        0,
        "the captured ability schedules its controller's extra turn"
    );
    assert_eq!(engine.state.turn_instance, 2);
    assert_eq!(
        engine.state.turn, normal_turn,
        "the extra turn does not advance the normal turn cycle"
    );
}

#[test]
fn ichormoon_carth_cost_changes_and_teferi_emblem_allow_the_real_grants_at_instant_speed() {
    for (case, with_emblem) in [false, true].into_iter().enumerate() {
        let mut engine = main_phase(26_102_808 + case as u64);
        if with_emblem {
            create_actual_teferi_emblem(&mut engine);
        }
        // Carth is cast for its real static effect; its optional entry look is declined. Jace
        // placements are fixture setup, while both activations use the registered definitions.
        cast_carth_and_decline_entry_look(&mut engine);
        let source = cast_ichormoon(&mut engine, None);
        let plus_walker = inject_permanent_on_battlefield(&mut engine, 0, "jace_beleren");
        let ultimate_walker =
            inject_permanent_on_battlefield(&mut engine, 0, "jace,_unraveler_of_secrets");
        engine
            .state
            .objects
            .get_mut(&plus_walker)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 5);
        engine
            .state
            .objects
            .get_mut(&ultimate_walker)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 12);
        let plus_offer = ichormoon_grant_with_cost(&mut engine, 0, plus_walker, "+1");
        let ultimate_offer = ichormoon_grant_with_cost(&mut engine, 0, ultimate_walker, "-11");
        assert_eq!(plus_offer.cost_label, "+1", "Carth modifies 0 to +1");
        assert_eq!(
            ultimate_offer.cost_label, "-11",
            "Carth modifies -12 to -11"
        );
        assert_eq!(plus_offer.text, "+1: Proliferate.");
        assert_eq!(
            plus_offer
                .presentation
                .as_ref()
                .expect("published Carth presentation reference")
                .fallback_text,
            "+1: Proliferate."
        );
        assert_eq!(
            ultimate_offer.text,
            "-11: Take an extra turn after this one."
        );
        assert_eq!(
            ultimate_offer
                .presentation
                .as_ref()
                .expect("published Carth presentation reference")
                .fallback_text,
            "-11: Take an extra turn after this one."
        );
        assert!(ultimate_offer.activatable);
        assert!(
            engine.state.stack.is_empty(),
            "no cast or entry trigger remains before turn advance"
        );
        assert!(engine.state.pending_triggers.is_empty());
        assert!(engine.state.pending_resolution.is_none());

        end_active_turn(&mut engine, 0);
        advance_to_main1_from_game_start(&mut engine);
        assert_eq!(engine.state.active_player_id(), 1);
        cast_bolt(&mut engine, 1, 0);
        engine
            .apply_scenario_command(1, &pass())
            .expect("the Bolt caster passes priority to the planeswalker controller");
        assert_eq!(engine.state.priority_player_id(), 0);

        let offturn_offer = ichormoon_grant_with_cost(&mut engine, 0, plus_walker, "+1");
        assert_eq!(offturn_offer.cost_label, "+1");
        let offturn_command =
            activate_ability_for(&engine, plus_walker, offturn_offer.ability_index, vec![]);
        if with_emblem {
            engine
                .apply_scenario_command(0, &offturn_command)
                .expect("Teferi's actual emblem permits the granted loyalty ability over the Bolt");
            assert_eq!(
                engine.state.objects[&plus_walker].counter_count(CounterKind::Loyalty),
                6
            );
            assert_eq!(
                engine.state.stack.last().unwrap().activated_ability.as_ref().unwrap().costs,
                [AbilityCost::Loyalty(0)],
                "the captured grant retains its Oracle zero cost while Carth's live modifier pays +1"
            );
            resolve_ichormoon_proliferate(&mut engine, plus_walker);
            assert_eq!(
                engine.state.objects[&plus_walker].counter_count(CounterKind::Loyalty),
                7
            );
        } else {
            let before_rejection = engine.diagnostic_snapshot().unwrap();
            assert!(
                engine.apply_scenario_command(0, &offturn_command).is_err(),
                "Carth changes costs but does not itself grant instant timing"
            );
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before_rejection);
        }
        pass_both_players(&mut engine);
        assert!(
            engine.state.stack.is_empty(),
            "the pending Bolt resolves normally"
        );

        end_active_turn(&mut engine, 1);
        advance_to_main1_from_game_start(&mut engine);
        assert_eq!(engine.state.active_player_id(), 0);
        if !with_emblem {
            let main_phase_offer = ichormoon_grant_with_cost(&mut engine, 0, plus_walker, "+1");
            engine
                .apply_scenario_command(
                    0,
                    &activate_ability_for(
                        &engine,
                        plus_walker,
                        main_phase_offer.ability_index,
                        vec![],
                    ),
                )
                .expect("the granted +0 is available to activate at sorcery speed");
            assert_eq!(
                engine.state.objects[&plus_walker].counter_count(CounterKind::Loyalty),
                6
            );
            resolve_ichormoon_proliferate(&mut engine, plus_walker);
            assert_eq!(
                engine.state.objects[&plus_walker].counter_count(CounterKind::Loyalty),
                7
            );
        }

        let ultimate_offer = ichormoon_grant_with_cost(&mut engine, 0, ultimate_walker, "-11");
        assert!(ultimate_offer.activatable);
        engine
            .apply_scenario_command(
                0,
                &activate_ability_for(
                    &engine,
                    ultimate_walker,
                    ultimate_offer.ability_index,
                    vec![],
                ),
            )
            .expect("pay Carth's actual -11 combined cost on a different recipient");
        assert_eq!(
            engine.state.objects[&ultimate_walker].counter_count(CounterKind::Loyalty),
            1,
            "Carth changes the recipient payment from eleven negative loyalty to eleven"
        );
        assert_eq!(
            engine
                .state
                .stack
                .last()
                .unwrap()
                .activated_ability
                .as_ref()
                .unwrap()
                .costs,
            [AbilityCost::Loyalty(-12)],
            "the ability definition remains the exact Ichormoon -12"
        );

        move_ichormoon(&mut engine, 0, DevZone::Graveyard);
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        pass_both_players(&mut engine);
        let normal_turn = engine.state.turn;
        end_active_turn(&mut engine, 0);
        assert_eq!(
            engine.state.active_player_id(),
            0,
            "the captured granted ability schedules the extra turn"
        );
        assert_eq!(engine.state.turn, normal_turn);
    }
}
