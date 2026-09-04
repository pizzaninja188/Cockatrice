use super::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
};

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
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
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
        .apply_command(0, &activate_ability(hellhound, 0, Vec::new()))
        .expect("activate Fiery Hellhound");
    let ability_id = engine.state.stack.last().expect("ability on stack").id;

    let tidebinder_slot = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    let tidebinder = engine.state.players[0].hand[tidebinder_slot];
    engine
        .apply_command(0, &cast_spell(tidebinder_slot, Vec::new()))
        .expect("cast Tishana's Tidebinder with flash");
    pass_both_players(&mut engine);
    assert!(engine.state.players[0].battlefield.contains(&tidebinder));
    assert_eq!(engine.state.pending_triggers.len(), 1);

    (engine, hellhound, ability_id, tidebinder)
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
        .apply_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the activated ability");
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine.apply_command(first, &pass()).expect("first pass");
    let resolved = engine.apply_command(second, &pass()).expect("second pass");

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
        .apply_command(0, &cast_spell(unsummon, target_object(tidebinder)))
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
        .apply_command(0, &choose_trigger_target(spell_id, TargetRefKind::Stack),)
        .is_err());
}

#[test]
fn issue_207_tidebinder_leaving_before_resolution_prevents_only_the_ability_loss() {
    let (mut engine, hellhound, ability_id, tidebinder) = setup_tidebinder_over_hellhound(207_003);
    engine
        .apply_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the activated ability");

    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(tidebinder)))
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
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
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
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast The Wondrous Wasp with flash");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    (engine, target, wasp)
}

#[test]
fn issue_207_wasp_taps_and_removes_abilities_until_wasp_leaves() {
    let (mut engine, target, wasp) = setup_wasp(207_010);
    engine
        .apply_command(0, &choose_trigger_target(target, TargetRefKind::Permanent))
        .expect("choose Wasp target");
    pass_both_players(&mut engine);
    assert!(engine.state.objects[&target].tapped);
    assert!(zone_view_ability_flags(&mut engine, 0, target).is_empty());

    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(wasp)))
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
        .apply_command(0, &choose_trigger_target(target, TargetRefKind::Permanent))
        .expect("choose Wasp target");

    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(wasp)))
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
        .apply_command(0, &choose_no_trigger_targets())
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
    let mut engine = GameEngine::new(207_020, &[0, 1], 20, decks, true).expect("new game");
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
        .apply_command(0, &cast_spell(wasp_slot, Vec::new()))
        .expect("cast Wasp");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_target(target, TargetRefKind::Permanent))
        .expect("put Wasp trigger on the stack");
    let wasp_trigger = engine.state.stack.last().expect("Wasp trigger").id;

    let tidebinder_slot = hand_index_for_card(&engine, 0, "tishanas_tidebinder");
    let tidebinder = engine.state.players[0].hand[tidebinder_slot];
    engine
        .apply_command(0, &cast_spell(tidebinder_slot, Vec::new()))
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
        .apply_command(
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
fn issue_207_tidebinder_checks_the_countered_sources_current_types() {
    let (mut engine, _, ability_id, tidebinder) = setup_tidebinder_over_hellhound(207_021);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let generation = engine
        .state
        .zone_change_generation
        .get(&land)
        .copied()
        .unwrap_or(0);
    let ability = engine
        .state
        .stack
        .iter_mut()
        .find(|item| item.id == ability_id)
        .expect("activated ability");
    ability.source_permanent_id = Some(land);
    ability.source_zone_change = generation;
    ability.card_id = "forest".into();

    engine
        .apply_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the land-sourced ability");
    pass_both_players(&mut engine);
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
    assert!(!engine.state.continuous_effects.iter().any(|effect| {
        matches!(effect.affected, AffectedScope::Single(object) if object == land)
            && effect.kind == ContinuousEffectKind::Layer6RemoveAllAbilities
            && effect.source_id == Some(tidebinder)
    }));
}

#[test]
fn issue_207_countered_ability_source_zone_change_does_not_affect_its_new_incarnation() {
    let (mut engine, hellhound, ability_id, _) = setup_tidebinder_over_hellhound(207_022);
    engine
        .apply_command(0, &choose_trigger_target(ability_id, TargetRefKind::Stack))
        .expect("target the activated ability");

    ensure_in_hand(&mut engine, 0, "unsummon");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(hellhound)))
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
