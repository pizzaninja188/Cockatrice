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
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Vow to Erebor");
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine.apply_command(first, &pass()).expect("first pass");
    engine.apply_command(second, &pass()).expect("second pass")
}

#[test]
fn issue_188_attachment_vocabulary_loads() {
    let fixture = r#"
(
  id: "issue_188_fixture",
  name: "Issue 188 Fixture",
  face_id: "issue_188_fixture",
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
    AttachEquipment(
      equipment: PreviousEffectObject,
      creature: Chosen((kind: Creature, controller: You)),
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

    CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect("issue #188 attachment vocabulary must load");
    CardRegistry::from_chunks_and_tokens(&[swordsman_fixture()], &[])
        .expect("issue #188 two-target attachment vocabulary must load");
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
    let mut engine =
        GameEngine::new(188_001, &[0, 1], 20, decks, true).expect("issue #188 cards validate");
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
        .apply_command(0, &submit_resolution_choice(vec![equipment]))
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
    let mut engine = GameEngine::new(188_002, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let dwarf = relocate_to_battlefield(&mut engine, 0, "dwarven_priest", true);
    let equipment = relocate_to_battlefield(&mut engine, 0, "bonesplitter", false);

    let resolving = cast_vow_and_reach_choice(&mut engine, dwarf);
    assert!(find_resolution_choice(&resolving).is_some());
    engine
        .apply_command(0, &submit_resolution_choice(Vec::new()))
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
    let mut engine = GameEngine::new(188_003, &[0, 1], 20, decks, true).expect("new game");
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
    let mut engine = GameEngine::new(188_004, &[0, 1], 20, decks, true).expect("new game");
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
fn issue_188_swordsman_shape_attaches_optional_equipment_to_required_creature() {
    let decks = Some(vec![
        deck_with("island", &["a.i.m._bot", "bonesplitter"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(188_005, &[0, 1], 20, decks, true).expect("new game");
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
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast entering Villain");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_swordsman_targets(Some(equipment), recipient))
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
    let mut engine = GameEngine::new(188_006, &[0, 1], 20, decks, true).expect("new game");
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
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast entering Villain");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_swordsman_targets(None, recipient))
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
    let mut engine = GameEngine::new(188_007, &[0, 1], 20, decks, true).expect("new game");
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
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast entering Villain");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_swordsman_targets(Some(equipment), recipient))
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
    let mut engine = GameEngine::new(188_008, &[0, 1], 20, decks, true).expect("new game");
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
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast entering Villain");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_swordsman_targets(Some(equipment), recipient))
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
    AttachEquipment(
      equipment: PreviousEffectObject,
      creature: Chosen((kind: Creature, controller: You)),
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
    let error = CardRegistry::from_chunks_and_tokens(&[unknown_group], &[])
        .expect_err("constraint target group must exist");
    assert!(error.to_string().contains("unknown target group"));
}
