use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use prost::Message;
use tricerules_cards::primitives::{
    ContinuousEffectKind, CounterKind, EffectDuration, Keyword, PermanentTypeFilter,
    TypeLineReplacement,
};
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, RuledEventBatch, TargetRef,
    TargetRefKind,
};

fn set_source_type_to_land(engine: &mut GameEngine, object_id: u32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind: ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Land],
            creature_types: Vec::new(),
            land_types: Vec::new(),
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });
}

fn assert_stack_ability_source(engine: &GameEngine, ability_id: u32, source_id: u32) {
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0);
    let ability = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == ability_id)
        .expect("the activated ability remains on the stack");
    assert_eq!(ability.source_permanent_id, Some(source_id));
    assert_eq!(ability.source_zone_change, source_generation);
}

fn record_tidebinder_command(
    engine: &mut GameEngine,
    commands: &mut Vec<(i32, RuledCommand)>,
    batches: &mut Vec<RuledEventBatch>,
    actor: i32,
    command: RuledCommand,
) {
    let command = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
    batches.push(
        engine
            .apply_scenario_command(actor, &command)
            .expect("recorded Tidebinder command is accepted"),
    );
    commands.push((actor, command));
}

fn choose_trigger_target(object_id: u32, kind: TargetRefKind) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: kind as i32,
                ..Default::default()
            }],
        })),
    }
}

fn choose_no_trigger_targets() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: Vec::new(),
        })),
    }
}

fn setup_tidebinder_over_hellhound(seed: u64) -> (GameEngine, u32, u32, u32) {
    let decks = Some(vec![
        deck_with(
            "island",
            &["fiery_hellhound", "tishanas_tidebinder", "unsummon"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let hellhound = relocate_to_battlefield(&mut engine, 0, "fiery_hellhound", false);
    ensure_in_hand(&mut engine, 0, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            u: 2,
            c: 2,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(0, &activate_ability(hellhound, 0, Vec::new()))
        .expect("activate Fiery Hellhound");
    let ability_id = engine.state.stack.last().expect("ability on stack").id;

    let tidebinder_slot = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    let tidebinder = engine.state.players[0].hand[tidebinder_slot];
    engine
        .apply_scenario_command(0, &cast_spell(tidebinder_slot, Vec::new()))
        .expect("cast Tishana's Tidebinder with flash");
    pass_both_players(&mut engine);
    assert!(engine.state.players[0].battlefield.contains(&tidebinder));
    assert_eq!(engine.state.pending_triggers.len(), 1);

    (engine, hellhound, ability_id, tidebinder)
}

fn setup_tidebinder_over_teferi(seed: u64, tidebinder_count: usize) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with(
            "island",
            &[
                "teferi,_temporal_pilgrim",
                "tishanas_tidebinder",
                "tishanas_tidebinder",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let teferi = relocate_to_battlefield(&mut engine, 0, "teferi,_temporal_pilgrim", false);
    engine
        .state
        .objects
        .get_mut(&teferi)
        .expect("Teferi")
        .set_counter(CounterKind::Loyalty, 4);

    while engine.state.players[0]
        .hand
        .iter()
        .filter_map(|object_id| engine.state.objects.get(object_id))
        .filter(|object| object.card_id == "tishanas_tidebinder")
        .count()
        < tidebinder_count
    {
        take_card_from_library_to_hand(&mut engine, 0, "tishanas_tidebinder");
    }
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            c: 4,
            ..Default::default()
        },
    );

    let registered_teferi = tricerules_cards::registry::global()
        .get("teferi,_temporal_pilgrim")
        .expect("the registered Teferi definition");
    assert_eq!(
        registered_teferi.primary_face().activated_abilities[0]
            .ability_id
            .as_str(),
        "activated_01"
    );
    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, teferi, 0, Vec::new()))
        .expect("activate Teferi's registered activated_01 loyalty-zero draw");
    let ability_id = engine
        .state
        .stack
        .last()
        .expect("Teferi activation on stack")
        .id;
    (engine, teferi, ability_id)
}

#[test]
fn issue_207_tidebinder_publishes_counters_and_links_ability_loss_to_its_source() {
    let (mut engine, hellhound, ability_id, tidebinder) = setup_tidebinder_over_hellhound(207_001);

    let trigger_batch = engine.initial_response_batch();
    let targets = &trigger_batch.legal_by_player[&0].valid_targets_by_ability
        [&(u64::from(tidebinder) << 32)]
        .groups[0];
    assert_eq!(targets.valid_stack_ids, vec![ability_id]);

    engine
        .apply_scenario_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the activated ability");
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first pass");
    let resolved = engine
        .apply_scenario_command(second, &pass())
        .expect("second pass");

    assert!(resolved.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability_id
    )));
    assert!(!resolved.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == ability_id
    )));
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert!(
        zone_view_ability_flags(&mut engine, 0, hellhound).is_empty(),
        "the exact permanent that sourced the countered ability loses its abilities"
    );

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(hellhound),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Flying),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index + 1,
    });
    assert!(
        engine.effective_has_keyword(hellhound, Keyword::Flying),
        "an ability granted after Tidebinder's timestamp remains"
    );

    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(tidebinder)))
        .expect("cast Unsummon at Tidebinder");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        !zone_view_ability_flags(&mut engine, 0, hellhound).is_empty(),
        "the original abilities return when Tidebinder leaves"
    );
}

#[test]
fn issue_207_counter_ability_rejects_a_spell_target() {
    let (mut engine, _, _, tidebinder) = setup_tidebinder_over_hellhound(207_002);
    let mut spell = engine.state.stack[0].clone();
    let spell_id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    spell.id = spell_id;
    spell.card_id = "grizzly_bears".into();
    spell.ability_text = None;
    spell.is_triggered = false;
    engine.state.stack.push(spell);

    let targets = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
        [&(u64::from(tidebinder) << 32)]
        .groups[0]
        .valid_stack_ids;
    assert!(!targets.contains(&spell_id));
    assert_eq!(targets.len(), 1, "only the activated ability is legal");
    assert!(engine
        .apply_scenario_command(0, &choose_trigger_target(spell_id, TargetRefKind::Stack),)
        .is_err());
}

#[test]
fn issue_207_tidebinder_leaving_before_resolution_prevents_only_the_ability_loss() {
    let (mut engine, hellhound, ability_id, tidebinder) = setup_tidebinder_over_hellhound(207_003);
    engine
        .apply_scenario_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the activated ability");

    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(tidebinder)))
        .expect("respond by removing Tidebinder");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&tidebinder].zone,
        tricerules_core::Zone::Hand
    );

    pass_both_players(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert!(
        !zone_view_ability_flags(&mut engine, 0, hellhound).is_empty(),
        "CR 611.2b prevents an already-ended duration from beginning"
    );
}

fn setup_wasp(seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("island", &["the_wondrous_wasp", "unsummon"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 0, "fiery_hellhound");
    ensure_in_hand(&mut engine, 0, "the_wondrous_wasp");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "the_wondrous_wasp");
    let wasp = engine.state.players[0].hand[slot];
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast The Wondrous Wasp with flash");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    (engine, target, wasp)
}

#[test]
fn issue_207_wasp_taps_and_removes_abilities_until_wasp_leaves() {
    let (mut engine, target, wasp) = setup_wasp(207_010);
    engine
        .apply_scenario_command(0, &choose_trigger_target(target, TargetRefKind::Permanent))
        .expect("choose Wasp target");
    pass_both_players(&mut engine);
    assert!(engine.state.objects[&target].tapped);
    assert!(zone_view_ability_flags(&mut engine, 0, target).is_empty());

    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(wasp)))
        .expect("cast Unsummon at Wasp");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&wasp].zone,
        tricerules_core::Zone::Hand
    );
    assert!(!zone_view_ability_flags(&mut engine, 0, target).is_empty());
}

#[test]
fn issue_207_wasp_leaving_before_resolution_still_taps_but_never_removes_abilities() {
    let (mut engine, target, wasp) = setup_wasp(207_011);
    engine
        .apply_scenario_command(0, &choose_trigger_target(target, TargetRefKind::Permanent))
        .expect("choose Wasp target");

    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(wasp)))
        .expect("remove Wasp before its trigger resolves");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);

    assert!(engine.state.objects[&target].tapped);
    assert!(!zone_view_ability_flags(&mut engine, 0, target).is_empty());
}

#[test]
fn issue_207_optional_wasp_target_can_be_declined() {
    let (mut engine, target, _) = setup_wasp(207_012);
    engine
        .apply_scenario_command(0, &choose_no_trigger_targets())
        .expect("choose zero targets for Wasp");
    assert!(engine.state.pending_triggers.is_empty());
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(!engine.state.objects[&target].tapped);
    assert!(!zone_view_ability_flags(&mut engine, 0, target).is_empty());
}

#[test]
fn issue_207_tidebinder_can_counter_a_triggered_ability_and_affect_its_source() {
    let decks = Some(vec![
        deck_with("island", &["the_wondrous_wasp", "tishanas_tidebinder"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        207_020,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 0, "fiery_hellhound");
    ensure_in_hand(&mut engine, 0, "the_wondrous_wasp");
    ensure_in_hand(&mut engine, 0, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
    );

    let wasp_slot = hand_index_for_card(&engine, 0, "the_wondrous_wasp");
    let wasp = engine.state.players[0].hand[wasp_slot];
    engine
        .apply_scenario_command(0, &cast_spell(wasp_slot, Vec::new()))
        .expect("cast Wasp");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(target, TargetRefKind::Permanent))
        .expect("put Wasp trigger on the stack");
    let wasp_trigger = engine.state.stack.last().expect("Wasp trigger").id;

    let tidebinder_slot = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    let tidebinder = engine.state.players[0].hand[tidebinder_slot];
    engine
        .apply_scenario_command(0, &cast_spell(tidebinder_slot, Vec::new()))
        .expect("cast Tidebinder over the trigger");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert_eq!(
        engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
            [&(u64::from(tidebinder) << 32)]
            .groups[0]
            .valid_stack_ids,
        vec![wasp_trigger]
    );
    engine
        .apply_scenario_command(
            0,
            &choose_trigger_target(wasp_trigger, TargetRefKind::Stack),
        )
        .expect("target Wasp's triggered ability");
    pass_both_players(&mut engine);

    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == wasp_trigger));
    assert!(!engine.state.objects[&target].tapped);
    assert!(!engine.effective_has_keyword(wasp, Keyword::Flying));
}

#[test]
fn issue_207_tidebinder_counters_teferis_registered_zero_ability_and_removes_its_abilities() {
    let (mut engine, teferi, ability_id) = setup_tidebinder_over_teferi(207_030, 1);
    let starting_library = engine.state.players[0].library.len();
    let tidebinder_slot = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    let tidebinder = engine.state.players[0].hand[tidebinder_slot];
    engine
        .apply_scenario_command(0, &cast_spell(tidebinder_slot, Vec::new()))
        .expect("cast Tidebinder over Teferi's registered activated_01");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the real Teferi activation");

    let resolved = pass_both_players_with_batch(&mut engine);
    assert!(resolved.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability_id
    )));
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert_eq!(
        engine.state.players[0].library.len(),
        starting_library,
        "the countered registered draw ability does not draw"
    );
    assert!(
        zone_view_ability_flags(&mut engine, 0, teferi).is_empty(),
        "Tidebinder removes the registered planeswalker's abilities after countering its activation"
    );
    assert!(engine.state.continuous_effects.iter().any(|effect| {
        matches!(effect.affected, AffectedScope::Single(object) if object == teferi)
            && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
            && effect.source_id == Some(tidebinder)
    }));
}

#[test]
fn issue_207_no_tidebinder_target_produces_no_counter_receipt_and_teferis_ability_resolves() {
    let (mut engine, teferi, ability_id) = setup_tidebinder_over_teferi(207_031, 1);
    let starting_library = engine.state.players[0].library.len();
    let tidebinder_slot = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    let tidebinder = engine.state.players[0].hand[tidebinder_slot];
    engine
        .apply_scenario_command(0, &cast_spell(tidebinder_slot, Vec::new()))
        .expect("cast Tidebinder with a real ability on the stack");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_no_trigger_targets())
        .expect("choose zero targets for Tidebinder's optional target group");
    pass_both_players(&mut engine);

    assert!(engine.state.stack.iter().any(|item| item.id == ability_id));
    assert!(!engine.state.continuous_effects.iter().any(|effect| {
        matches!(effect.affected, AffectedScope::Single(object) if object == teferi)
            && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
            && effect.source_id == Some(tidebinder)
    }));
    assert!(!zone_view_ability_flags(&mut engine, 0, teferi).is_empty());

    pass_both_players(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert_eq!(
        engine.state.players[0].library.len(),
        starting_library - 1,
        "the ability resolves and draws when the counter trigger had no target receipt"
    );
}

#[test]
fn issue_207_later_tidebinder_trigger_counters_shared_activation_and_earlier_trigger_fizzles() {
    let (mut engine, teferi, ability_id) = setup_tidebinder_over_teferi(207_032, 2);
    let starting_library = engine.state.players[0].library.len();

    let first_slot = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    let first_tidebinder = engine.state.players[0].hand[first_slot];
    engine
        .apply_scenario_command(0, &cast_spell(first_slot, Vec::new()))
        .expect("cast the earlier Tidebinder");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("the earlier Tidebinder ETB targets Teferi's still-stacked activation");

    let second_slot = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    let second_tidebinder = engine.state.players[0].hand[second_slot];
    engine
        .apply_scenario_command(0, &cast_spell(second_slot, Vec::new()))
        .expect("cast a second Tidebinder before the first trigger resolves");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("the later Tidebinder ETB targets the same activation");

    let later_resolution = pass_both_players_with_batch(&mut engine);
    assert!(later_resolution.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability_id
    )));
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert_eq!(engine.state.players[0].library.len(), starting_library);
    assert!(engine.state.continuous_effects.iter().any(|effect| {
        matches!(effect.affected, AffectedScope::Single(object) if object == teferi)
            && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
            && effect.source_id == Some(second_tidebinder)
    }));
    assert!(!engine.state.continuous_effects.iter().any(|effect| {
        effect.source_id == Some(first_tidebinder)
            && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
    }));

    let earlier_resolution = pass_both_players_with_batch(&mut engine);
    assert!(!earlier_resolution.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability_id
    )));
    assert!(!engine.state.continuous_effects.iter().any(|effect| {
        effect.source_id == Some(first_tidebinder)
            && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
    }));
    assert!(zone_view_ability_flags(&mut engine, 0, teferi).is_empty());
}

#[test]
fn issue_207_tidebinder_checks_the_countered_sources_current_types() {
    let (mut engine, hellhound, ability_id, tidebinder) = setup_tidebinder_over_hellhound(207_021);
    let generation = engine.state.zone_change_generation.get(&hellhound).copied();

    engine
        .apply_scenario_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the ability while its source is a creature");

    // Prepared-state fixture: after target selection, before counter resolution, the source changes type.
    set_source_type_to_land(&mut engine, hellhound);
    assert!(engine
        .characteristics(hellhound)
        .expect("live source characteristics")
        .has_type("Land"));
    assert_stack_ability_source(&engine, ability_id, hellhound);
    assert_eq!(
        engine.state.zone_change_generation.get(&hellhound).copied(),
        generation
    );

    let resolution = pass_both_players_with_batch(&mut engine);
    assert!(resolution.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability_id
    )));
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert!(
        !engine.state.continuous_effects.iter().any(|effect| {
            matches!(effect.affected, AffectedScope::Single(object) if object == hellhound)
                && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
                && effect.source_id == Some(tidebinder)
        }),
        "the source is no longer an artifact, creature, or planeswalker at resolution"
    );
    assert_eq!(
        engine.state.zone_change_generation.get(&hellhound).copied(),
        generation
    );
}

#[test]
fn issue_207_tidebinder_uses_live_creature_type_after_target_selection() {
    let (mut engine, hellhound, ability_id, tidebinder) = setup_tidebinder_over_hellhound(207_023);
    let generation = engine.state.zone_change_generation.get(&hellhound).copied();

    // This prepared-state fixture starts with an ineligible source, then changes it after target
    // selection and before counter resolution.
    set_source_type_to_land(&mut engine, hellhound);
    assert!(engine
        .characteristics(hellhound)
        .expect("land-layered source characteristics")
        .has_type("Land"));
    engine
        .apply_scenario_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the still-stacked ability from a currently nonqualifying source");

    // Remove only the fixture layer effect; the same battlefield object now evaluates as its
    // printed creature type before Tidebinder's result condition is checked.
    engine.state.continuous_effects.retain(|effect| {
        !(matches!(effect.affected, AffectedScope::Single(object) if object == hellhound)
            && matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)))
    });
    assert!(engine
        .characteristics(hellhound)
        .expect("restored source characteristics")
        .is_creature());
    assert_stack_ability_source(&engine, ability_id, hellhound);
    assert_eq!(
        engine.state.zone_change_generation.get(&hellhound).copied(),
        generation
    );

    let resolution = pass_both_players_with_batch(&mut engine);
    assert!(resolution.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability_id
    )));
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert!(engine.state.continuous_effects.iter().any(|effect| {
        matches!(effect.affected, AffectedScope::Single(object) if object == hellhound)
            && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
            && effect.source_id == Some(tidebinder)
    }));
    assert_eq!(
        engine.state.zone_change_generation.get(&hellhound).copied(),
        generation
    );
}

#[test]
fn issue_207_countered_ability_source_zone_change_does_not_affect_its_new_incarnation() {
    let (mut engine, hellhound, ability_id, _) = setup_tidebinder_over_hellhound(207_022);
    engine
        .apply_scenario_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the activated ability");

    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(hellhound)))
        .expect("remove the ability source before Tidebinder resolves");
    pass_both_players(&mut engine);
    let departed_generation = engine.state.zone_change_generation[&hellhound];
    pass_both_players(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));

    let returned = move_ready_to_battlefield(&mut engine, 0, "fiery_hellhound");
    assert_eq!(returned, hellhound);
    assert!(engine.state.zone_change_generation[&returned] >= departed_generation);
    assert!(!zone_view_ability_flags(&mut engine, 0, returned).is_empty());
}

#[test]
fn issue_207_prepared_state_tidebinder_command_tail_replays_identically() {
    // The shared fixture prefix prepares the game, activated ability, and pending ETB trigger.
    // This test replays only the serialized, accepted command tail; it is not a full legal-session
    // replay claim for the fixture setup.
    let (mut original, hellhound, ability_id, tidebinder) =
        setup_tidebinder_over_hellhound(207_024);
    let (mut replay, replay_hellhound, replay_ability_id, replay_tidebinder) =
        setup_tidebinder_over_hellhound(207_024);
    assert_eq!(replay_hellhound, hellhound);
    assert_eq!(replay_ability_id, ability_id);
    assert_eq!(replay_tidebinder, tidebinder);
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        original.diagnostic_snapshot().unwrap(),
        "both engines begin from the same prepared fixture state"
    );

    let mut commands = Vec::new();
    let mut batches = Vec::new();
    record_tidebinder_command(
        &mut original,
        &mut commands,
        &mut batches,
        0,
        choose_trigger_target(ability_id, TargetRefKind::Stack),
    );
    for _ in 0..original.state.players.len() {
        let actor = original.state.priority_player_id();
        record_tidebinder_command(&mut original, &mut commands, &mut batches, actor, pass());
    }

    assert!(!original
        .state
        .stack
        .iter()
        .any(|item| item.id == ability_id));
    assert!(original.state.pending_triggers.is_empty());
    assert!(original.state.pending_trigger_order.is_none());
    assert_eq!(
        original
            .state
            .zone_change_generation
            .get(&hellhound)
            .copied(),
        replay.state.zone_change_generation.get(&hellhound).copied()
    );
    assert_eq!(
        batches
            .iter()
            .flat_map(|batch| &batch.events)
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability_id
            ))
            .count(),
        1,
        "the accepted Tidebinder tail counters the activation exactly once"
    );
    assert_eq!(
        original
            .state
            .continuous_effects
            .iter()
            .filter(|effect| {
                effect.source_id == Some(tidebinder)
                    && matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
            })
            .count(),
        1,
        "the accepted Tidebinder tail creates one source ability-removal effect"
    );

    let replayed_batches = commands
        .iter()
        .map(|(actor, command)| replay.apply_scenario_command(*actor, command).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(replayed_batches, batches);
    assert_eq!(replay.state.command_index, original.state.command_index);
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        original.diagnostic_snapshot().unwrap()
    );
    assert!(replay.state.pending_triggers.is_empty());
    assert!(replay.state.pending_trigger_order.is_none());
    assert!(!replay
        .state
        .stack
        .iter()
        .any(|item| item.id == replay_ability_id));
    assert_eq!(
        replay
            .state
            .continuous_effects
            .iter()
            .filter(|effect| {
                effect.source_id == Some(replay_tidebinder)
                    && matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
            })
            .count(),
        1
    );
}
