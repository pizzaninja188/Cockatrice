use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, ProtectionCardType, ProtectionQuality,
};
use tricerules_cards::CardRegistry;
use tricerules_core::{AffectedScope, AttachmentRecipient, ContinuousEffect};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, RuledCommand, TargetRef,
};

fn swordsman_fixture() -> &'static str {
    r#"
(
  id: "issue_188_swordsman_fixture",
  name: "Issue 188 Swordsman Fixture",
  face_id: "issue_188_swordsman_fixture",
  mana_cost: "{1}{W}",
  types: ["Creature", "Human", "Villain"],
  power: 2,
  toughness: 2,
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: Fallback,
    trigger: WheneverPermanentEntersBattlefield(
      controller: Controller,
      filter: (permanent_type: Some(Creature), exclude_source: true),
      creature_filter: Some((required_subtypes: ["Villain"])),
    ),
    effect: [AttachEquipment(
      equipment: Chosen((kind: AnyPermanent, controller: You, required_subtypes: ["Equipment"])),
      creature: Chosen((kind: Creature, controller: You)),
    )],
    targeting: Some((groups: [
      (min: 0, max: 1, prompt: "Choose up to one target Equipment you control", effect_indices: [0]),
      (min: 1, max: 1, prompt: "Choose target creature you control", effect_indices: [0]),
    ])),
  )],
)
"#
}

fn choose_swordsman_targets(equipment: Option<u32>, creature: u32) -> RuledCommand {
    let mut targets = Vec::new();
    if let Some(object_id) = equipment {
        targets.push(TargetRef {
            object_id,
            group_index: 0,
            ..Default::default()
        });
    }
    targets.push(TargetRef {
        object_id: creature,
        group_index: 1,
        ..Default::default()
    });
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn grant_swordsman_trigger(engine: &mut GameEngine, source: u32) {
    let registry = CardRegistry::from_chunks_and_tokens(&[swordsman_fixture()], &[])
        .expect("Swordsman-shaped attachment ability validates");
    let ability = registry
        .get("issue_188_swordsman_fixture")
        .expect("fixture card")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

fn cast_vow_and_reach_choice(
    engine: &mut GameEngine,
    target: u32,
) -> tricerules_proto::ruled::v1::RuledEventBatch {
    ensure_in_hand(engine, 0, "vow_to_erebor");
    give_mana(
        engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "vow_to_erebor");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Vow to Erebor");
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first pass");
    engine
        .apply_scenario_command(second, &pass())
        .expect("second pass")
}

fn issue_188_vow_copy_game(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["vow_to_erebor", "dwarven_priest", "bonesplitter"],
        ),
        deck_with("island", &["twincast", "dwarven_priest", "bonesplitter"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("Vow and Twincast fixture cards validate");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_vow_and_twincast_to_copy_target_choice(engine: &mut GameEngine, target: u32) -> u32 {
    ensure_in_hand(engine, 0, "vow_to_erebor");
    give_mana(
        engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let vow_slot = hand_index_for_card(engine, 0, "vow_to_erebor");
    engine
        .apply_scenario_command(0, &cast_spell(vow_slot, target_object(target)))
        .expect("P0 casts Vow to Erebor");
    let vow_id = engine.state.stack.last().expect("Vow on stack").id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("P0 gives P1 a chance to copy Vow");

    ensure_in_hand(engine, 1, "twincast");
    give_mana(
        engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast_slot = hand_index_for_card(engine, 1, "twincast");
    engine
        .apply_scenario_command(1, &cast_spell(twincast_slot, target_object(vow_id)))
        .expect("P1 casts Twincast targeting Vow");
    pass_priority_round(engine);
    vow_id
}

#[test]
fn issue_188_attachment_vocabulary_loads() {
    let vow = tricerules_cards::registry::global()
        .get("vow_to_erebor")
        .expect("Vow to Erebor is registered")
        .primary_face();
    assert!(
        vow.spell_program.is_some(),
        "Vow's untap, pump, live Dwarf gate, Equipment choice, and attachment use its Program"
    );
    for card_id in [
        "conductive_machete",
        "cursed_windbreaker",
        "killers_mask",
        "kyoshi_battle_fan",
    ] {
        let face = tricerules_cards::registry::global()
            .get(card_id)
            .unwrap_or_else(|| panic!("{card_id} is registered"))
            .primary_face();
        assert!(
            face.triggered_abilities
                .iter()
                .any(|ability| ability.program.is_some()),
            "{card_id} uses its authored Program for Equipment ETB behavior"
        );
    }
    CardRegistry::from_chunks_and_tokens(&[swordsman_fixture()], &[])
        .expect("issue #188 two-target attachment vocabulary must load");

    let retired_vow_flat_shape = r#"
(
  id: "issue_188_retired_vow_flat_shape",
  name: "Issue 188 Retired Vow Flat Shape",
  face_id: "issue_188_retired_vow_flat_shape",
  mana_cost: "{1}{W}",
  types: ["Instant"],
  spell_effect: [
    Conditional(
      condition: ObjectMatches(
        object: ChosenTarget(group_index: 0, target_index: 0),
        filter: (kind: Creature, required_subtypes: ["Dwarf"]),
      ),
      effect: ChoosePermanents(
        chooser: Controller,
        filter: (kind: AnyPermanent, controller: You, required_subtypes: ["Equipment"]),
        min: 0,
        max: 1,
        constraints: [EquipmentAttachableTo(
          recipient: ChosenTarget(group_index: 0, target_index: 0),
        )],
      ),
    ),
    Conditional(
      condition: ObjectMatches(
        object: ChosenTarget(group_index: 0, target_index: 0),
        filter: (kind: Creature, required_subtypes: ["Dwarf"]),
      ),
      effect: AttachEquipment(
        equipment: PreviousEffectObject,
        creature: Chosen((kind: Creature, controller: You)),
      ),
    ),
  ],
  targeting: Some((groups: [(
    min: 1,
    max: 1,
    prompt: "Choose target creature you control",
    effect_indices: [1],
  )])),
)
"#;
    assert!(
        CardRegistry::from_chunks_and_tokens(&[retired_vow_flat_shape], &[]).is_err(),
        "the retired PreviousEffectObject/Chosen attachment must require a Program"
    );

    let retired_created_token_flat_shape = r#"
(
  id: "issue_188_retired_created_token_flat_shape",
  name: "Issue 188 Retired Created Token Flat Shape",
  face_id: "issue_188_retired_created_token_flat_shape",
  mana_cost: "{3}",
  types: ["Artifact", "Equipment"],
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: Fallback,
    trigger: WhenSelfEntersBattlefield,
    effect: [
      CreateTokens(token: "ally_w_1_1", count: 1, who: Controller,
        tapped: false, sacrifice_timing: None),
      AttachEquipment(equipment: Source, creature: PreviousEffectObjects),
    ],
    targeting: None,
    modal: None,
    may: false,
    intervening_if: None,
    triggers_only_once: false,
    max_triggers_per_turn: None,
  )],
)
"#;
    assert!(
        CardRegistry::from_chunks_and_tokens(&[retired_created_token_flat_shape], &[]).is_err(),
        "the retired Source/PreviousEffectObjects attachment must require a Program"
    );

    let retired_manifest_flat_shape = r#"
(
  id: "issue_188_retired_manifest_flat_shape",
  name: "Issue 188 Retired Manifest Flat Shape",
  face_id: "issue_188_retired_manifest_flat_shape",
  mana_cost: "{3}",
  types: ["Artifact", "Equipment"],
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: Fallback,
    trigger: WhenSelfEntersBattlefield,
    effect: [
      ManifestDread,
      AttachEquipment(equipment: Source, creature: PreviousEffectObject),
    ],
    targeting: None,
    modal: None,
    may: false,
    intervening_if: None,
    triggers_only_once: false,
    max_triggers_per_turn: None,
  )],
)
"#;
    assert!(
        CardRegistry::from_chunks_and_tokens(&[retired_manifest_flat_shape], &[]).is_err(),
        "the retired Source/PreviousEffectObject attachment must require a Program"
    );
}

#[test]
fn issue_188_vow_chooses_and_attaches_one_legal_equipment_to_a_dwarf() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["vow_to_erebor", "dwarven_priest", "bonesplitter"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        188_001,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #188 cards validate");
    advance_to_main1_from_game_start(&mut engine);
    let dwarf = relocate_to_battlefield(&mut engine, 0, "dwarven_priest", true);
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);
    let resolving = cast_vow_and_reach_choice(&mut engine, dwarf);

    let choice = find_resolution_choice(&resolving).expect("optional Equipment choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::PermanentObjects);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(choice.candidate_object_ids, vec![equipment]);
    assert!(!engine.state.objects[&dwarf].tapped);
    assert_eq!(engine.effective_power(dwarf), Some(4));
    assert_eq!(engine.effective_toughness(dwarf), Some(6));

    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![equipment]))
        .expect("attach Bonesplitter");
    assert_eq!(
        engine.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(dwarf))
    );
}

#[test]
fn issue_188_vow_may_decline_the_equipment_choice() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["vow_to_erebor", "dwarven_priest", "bonesplitter"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        188_002,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let dwarf = relocate_to_battlefield(&mut engine, 0, "dwarven_priest", true);
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);

    let resolving = cast_vow_and_reach_choice(&mut engine, dwarf);
    assert!(find_resolution_choice(&resolving).is_some());
    engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline Vow's optional choice");

    assert_eq!(engine.state.objects[&equipment].attached_to, None);
    assert!(!engine.state.objects[&dwarf].tapped);
    assert_eq!(engine.effective_power(dwarf), Some(4));
}

#[test]
fn issue_188_vow_skips_the_equipment_choice_for_a_non_dwarf() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["vow_to_erebor", "grizzly_bears", "bonesplitter"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        188_003,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let creature = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", true);
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);

    let resolving = cast_vow_and_reach_choice(&mut engine, creature);

    assert!(find_resolution_choice(&resolving).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
    assert!(!engine.state.objects[&creature].tapped);
    assert_eq!(engine.effective_power(creature), Some(4));
}

#[test]
fn issue_188_vow_does_not_offer_equipment_that_cannot_legally_attach() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["vow_to_erebor", "dwarven_priest", "bonesplitter"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        188_004,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let dwarf = relocate_to_battlefield(&mut engine, 0, "dwarven_priest", true);
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(dwarf),
        kind: ContinuousEffectKind::Layer6AddProtection(ProtectionQuality::CardType(
            ProtectionCardType::Artifact,
        )),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    let resolving = cast_vow_and_reach_choice(&mut engine, dwarf);

    assert!(find_resolution_choice(&resolving).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
}

#[test]
fn issue_188_vow_rejects_the_same_equipment_object_after_it_returns_as_a_new_incarnation() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["vow_to_erebor", "dwarven_priest", "bonesplitter"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        188_009,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let dwarf = relocate_to_battlefield(&mut engine, 0, "dwarven_priest", true);
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);
    let resolving = cast_vow_and_reach_choice(&mut engine, dwarf);
    let choice = find_resolution_choice(&resolving).expect("Vow offers exact Equipment");
    let old_prompt_id = choice.resolution_prompt_id;
    let old_generation = engine
        .state
        .zone_change_generation
        .get(&equipment)
        .copied()
        .unwrap_or(0);

    // A direct fixture applies the exact out-and-back zone changes while the prompt is parked.
    // The public object ID is unchanged; the old choice still names the prior incarnation.
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != equipment);
    engine.state.players[0].graveyard.push(equipment);
    engine.state.objects.get_mut(&equipment).unwrap().zone = tricerules_core::Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(equipment)
        .or_default() += 1;
    engine.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != equipment);
    engine.state.players[0].battlefield.push(equipment);
    engine.state.objects.get_mut(&equipment).unwrap().zone = tricerules_core::Zone::Battlefield;
    *engine
        .state
        .zone_change_generation
        .entry(equipment)
        .or_default() += 1;
    assert!(engine.state.zone_change_generation[&equipment] > old_generation);

    let before = engine
        .diagnostic_snapshot()
        .expect("snapshot before Vow receives its stale Equipment incarnation");
    let seed_before = engine.state.seed;
    let command_before = engine.state.command_index;
    let mut stale_answer = submit_resolution_choice(vec![equipment]);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = stale_answer.cmd.as_mut() {
        answer.resolution_prompt_id = old_prompt_id;
    }
    assert!(engine.apply_command(0, &stale_answer).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after Vow rejects the old Equipment incarnation"),
        before
    );
    assert_eq!(engine.state.seed, seed_before);
    assert_eq!(engine.state.command_index, command_before);
    let pending = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(pending.resolution_prompt_id.get(), old_prompt_id);
    assert_eq!(pending.presentation.candidates, [equipment]);
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
    assert!(!engine.state.objects[&dwarf].tapped);
    assert_eq!(engine.effective_power(dwarf), Some(4));
}

#[test]
fn issue_188_twincast_vow_copy_retargets_for_copy_controller_and_runs_alias_branch_once() {
    let mut engine = issue_188_vow_copy_game(188_010);
    let original_dwarf = relocate_to_battlefield(&mut engine, 0, "dwarven_priest", true);
    let original_equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);
    let copy_dwarf = relocate_to_battlefield(&mut engine, 1, "dwarven_priest", true);
    let copy_equipment = relocate_to_battlefield(&mut engine, 1, "bonesplitter", false);

    let vow_id = cast_vow_and_twincast_to_copy_target_choice(&mut engine, original_dwarf);
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&original_dwarf)
        .copied()
        .unwrap_or(0);
    let copy_generation = engine
        .state
        .zone_change_generation
        .get(&copy_dwarf)
        .copied()
        .unwrap_or(0);
    let copy_target_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Twincast asks its controller to choose the copied target");
    assert_eq!(copy_target_choice.deciding_player, 1);
    assert_eq!(copy_target_choice.presentation.candidates, [copy_dwarf]);
    assert_eq!(
        (
            copy_target_choice.presentation.min,
            copy_target_choice.presentation.max
        ),
        (0, 1)
    );
    assert!(copy_target_choice
        .presentation
        .candidates
        .contains(&copy_dwarf));
    assert!(!copy_target_choice
        .presentation
        .candidates
        .contains(&original_dwarf));

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![copy_dwarf]))
        .expect("P1 retargets the copy at a creature P1 controls");
    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy && item.card_id == "vow_to_erebor")
        .expect("the copied Vow is waiting above the original");
    assert_eq!(copy.targets.len(), 1);
    assert_eq!(copy.targets[0].object_id, copy_dwarf);
    assert_eq!(copy.targets[0].group_index, 0);
    assert_eq!(
        copy.targets[0].zone_change_generation,
        Some(copy_generation)
    );

    let copy_resolution = pass_both_players_with_batch(&mut engine);
    let copy_equipment_choice = find_resolution_choice(&copy_resolution)
        .expect("copied Vow reaches its Dwarf Equipment branch");
    assert_eq!(copy_equipment_choice.deciding_player_id, 1);
    assert_eq!(
        (copy_equipment_choice.min, copy_equipment_choice.max),
        (0, 1)
    );
    assert_eq!(copy_equipment_choice.candidate_object_ids, [copy_equipment]);
    assert!(!copy_equipment_choice
        .candidate_object_ids
        .contains(&original_equipment));
    assert!(!engine.state.objects[&copy_dwarf].tapped);
    assert_eq!(engine.effective_power(copy_dwarf), Some(4));
    assert_eq!(engine.effective_toughness(copy_dwarf), Some(6));
    assert!(engine.state.objects[&original_dwarf].tapped);
    assert_eq!(engine.effective_power(original_dwarf), Some(2));
    assert_eq!(engine.effective_toughness(original_dwarf), Some(4));

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![copy_equipment]))
        .expect("P1 attaches exactly the offered Equipment to the copied target");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, vow_id);
    assert!(!engine.state.stack[0].is_copy);
    assert_eq!(
        engine.state.objects[&copy_equipment].attached_to,
        Some(AttachmentRecipient::Object(copy_dwarf))
    );
    assert_eq!(engine.state.objects[&original_equipment].attached_to, None);

    let original_resolution = pass_both_players_with_batch(&mut engine);
    let original_equipment_choice = find_resolution_choice(&original_resolution)
        .expect("the original Vow independently reaches its Equipment branch");
    assert_eq!(original_equipment_choice.deciding_player_id, 0);
    assert_eq!(
        original_equipment_choice.candidate_object_ids,
        [original_equipment]
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&original_dwarf)
            .copied()
            .unwrap_or(0),
        original_generation
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![original_equipment]))
        .expect("P0 attaches Equipment to the original target");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&original_equipment].attached_to,
        Some(AttachmentRecipient::Object(original_dwarf))
    );
    assert_eq!(engine.effective_power(original_dwarf), Some(4));
    assert_eq!(engine.effective_toughness(original_dwarf), Some(6));
    assert_eq!(engine.effective_power(copy_dwarf), Some(4));
    assert_eq!(engine.effective_toughness(copy_dwarf), Some(6));
}

#[test]
fn issue_188_twincast_vow_copy_with_no_legal_retarget_does_not_choose_a_late_substitute() {
    let mut engine = issue_188_vow_copy_game(188_011);
    let original_dwarf = relocate_to_battlefield(&mut engine, 0, "dwarven_priest", true);
    let original_equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&original_dwarf)
        .copied()
        .unwrap_or(0);

    let vow_id = cast_vow_and_twincast_to_copy_target_choice(&mut engine, original_dwarf);
    let copy_target_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Twincast still prompts when the copy has no legal new target");
    assert_eq!(copy_target_choice.deciding_player, 1);
    assert!(copy_target_choice.presentation.candidates.is_empty());
    assert_eq!(
        (
            copy_target_choice.presentation.min,
            copy_target_choice.presentation.max
        ),
        (0, 1)
    );

    engine
        .apply_scenario_command(1, &submit_resolution_choice(Vec::new()))
        .expect("P1 keeps the original target when no new target is legal for P1");
    let copy = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy && item.card_id == "vow_to_erebor")
        .expect("the copied Vow remains targeted at the original P0 creature");
    assert_eq!(copy.targets.len(), 1);
    assert_eq!(copy.targets[0].object_id, original_dwarf);
    assert_eq!(copy.targets[0].group_index, 0);
    assert_eq!(
        copy.targets[0].zone_change_generation,
        Some(original_generation)
    );

    let late_substitute = relocate_to_battlefield(&mut engine, 1, "dwarven_priest", true);
    let late_equipment = relocate_to_battlefield(&mut engine, 1, "bonesplitter", false);
    let copy_resolution = pass_both_players_with_batch(&mut engine);
    assert!(
        find_resolution_choice(&copy_resolution).is_none(),
        "the copied spell's illegal target fizzles instead of substituting a newly legal target"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, vow_id);
    assert!(!engine.state.stack[0].is_copy);
    assert!(engine.state.objects[&original_dwarf].tapped);
    assert_eq!(engine.effective_power(original_dwarf), Some(2));
    assert_eq!(engine.effective_toughness(original_dwarf), Some(4));
    assert!(engine.state.objects[&late_substitute].tapped);
    assert_eq!(engine.effective_power(late_substitute), Some(2));
    assert_eq!(engine.effective_toughness(late_substitute), Some(4));
    assert_eq!(engine.state.objects[&late_equipment].attached_to, None);
    assert_eq!(engine.state.objects[&original_equipment].attached_to, None);

    let original_resolution = pass_both_players_with_batch(&mut engine);
    let original_choice = find_resolution_choice(&original_resolution)
        .expect("the independently legal original Vow still resolves on its target");
    assert_eq!(original_choice.deciding_player_id, 0);
    assert_eq!(original_choice.candidate_object_ids, [original_equipment]);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![original_equipment]))
        .expect("resolve the original Vow without inheriting the copy's target choice");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.effective_power(original_dwarf), Some(4));
    assert_eq!(engine.effective_toughness(original_dwarf), Some(6));
    assert!(engine.state.objects[&late_substitute].tapped);
    assert_eq!(engine.effective_power(late_substitute), Some(2));
    assert_eq!(
        engine.state.objects[&original_equipment].attached_to,
        Some(AttachmentRecipient::Object(original_dwarf))
    );
    assert_eq!(engine.state.objects[&late_equipment].attached_to, None);
}

#[test]
fn issue_188_swordsman_shape_attaches_optional_equipment_to_required_creature() {
    let decks = Some(vec![
        deck_with("island", &["a.i.m._bot", "bonesplitter"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        188_005,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);
    grant_swordsman_trigger(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "a.i.m._bot");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "a.i.m._bot");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast entering Villain");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_swordsman_targets(Some(equipment), recipient))
        .expect("choose optional Equipment and required creature");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(recipient))
    );
}

#[test]
fn issue_188_swordsman_shape_allows_omitting_the_optional_equipment() {
    let decks = Some(vec![
        deck_with("island", &["a.i.m._bot", "bonesplitter"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        188_006,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);
    grant_swordsman_trigger(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "a.i.m._bot");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "a.i.m._bot");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast entering Villain");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_swordsman_targets(None, recipient))
        .expect("choose only the required creature");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&equipment].attached_to, None);
}

#[test]
fn issue_188_swordsman_shape_ignores_a_stale_equipment_target() {
    let decks = Some(vec![
        deck_with("island", &["a.i.m._bot", "bonesplitter"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        188_007,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);
    grant_swordsman_trigger(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "a.i.m._bot");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "a.i.m._bot");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast entering Villain");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_swordsman_targets(Some(equipment), recipient))
        .expect("choose both targets");
    *engine
        .state
        .zone_change_generation
        .entry(equipment)
        .or_default() += 1;
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&equipment].attached_to, None);
}

#[test]
fn issue_188_swordsman_shape_rechecks_attachment_protection_on_resolution() {
    let decks = Some(vec![
        deck_with("island", &["a.i.m._bot", "bonesplitter"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        188_008,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let recipient = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);
    grant_swordsman_trigger(&mut engine, source);
    ensure_in_hand(&mut engine, 0, "a.i.m._bot");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "a.i.m._bot");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast entering Villain");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_swordsman_targets(Some(equipment), recipient))
        .expect("choose both targets");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(recipient),
        kind: ContinuousEffectKind::Layer6AddProtection(ProtectionQuality::CardType(
            ProtectionCardType::Artifact,
        )),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&equipment].attached_to, None);
}

#[test]
fn issue_188_attachment_vocabulary_rejects_malformed_constraints() {
    let missing_equipment_filter = swordsman_fixture().replace(
        "required_subtypes: [\"Equipment\"]",
        "required_subtypes: []",
    );
    let error = CardRegistry::from_chunks_and_tokens(&[&missing_equipment_filter], &[])
        .expect_err("Equipment target must be constrained by subtype");
    assert!(error
        .to_string()
        .contains("equipment target must require Equipment"));

    let unknown_group = r#"
(
  id: "issue_188_bad_constraint",
  name: "Issue 188 Bad Constraint",
  face_id: "issue_188_bad_constraint",
  mana_cost: "{W}",
  types: ["Instant"],
  spell_effect: [
    ChoosePermanents(
      chooser: Controller,
      filter: (kind: AnyPermanent, controller: You, required_subtypes: ["Equipment"]),
      min: 0,
      max: 1,
      constraints: [EquipmentAttachableTo(
        recipient: ChosenTarget(group_index: 7, target_index: 0),
      )],
    ),
    PumpTarget(power: 1, toughness: 1,
      subject: Chosen((kind: Creature, controller: You))),
  ],
  targeting: Some((groups: [(
    min: 1,
    max: 1,
    prompt: "Choose target creature you control",
    effect_indices: [1],
  )])),
)
"#;
    let error = CardRegistry::from_chunks_and_tokens(&[unknown_group], &[])
        .expect_err("constraint target group must exist");
    assert!(error.to_string().contains("unknown target group"));
}
