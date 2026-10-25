use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_core::Zone;

const SKIPPED_COUNTER_RECEIPT_FIXTURE: &str = r#"(
  id: "issue_478_skipped_counter_receipt",
  name: "Issue 478 Skipped Counter Receipt",
  face_id: "issue_478_skipped_counter_receipt",
  mana_cost: "{U}",
  types: ["Instant"],
  spell_program: Some((body: (steps: [
    (id: "counter_spell", bind: ["controller"],
      kind: Perform(CounterTargetSpell(spell_filter: (), unless_controller_pays: None))),
    (id: "create_delayed", input: Some("controller"),
      kind: Perform(CreateDelayedTrigger(
        subject: None,
        affected_player: Some(ProgramInputPlayer),
        ability: (
          ability_id: "delayed_01",
          presentation: Fallback,
          trigger: AtBeginningOfNextTurnUpkeep,
          effect: [],
          program: Some((body: (steps: [
            (id: "gain_life", kind: Perform(GainLife(who: AffectedPlayer, amount: 1))),
          ], yields: []), target_bindings: [])),
          modal: None, targeting: None, may: false, intervening_if: None,
          triggers_only_once: false, max_triggers_per_turn: None,
        ),
      ))),
    (id: "damage_player", kind: Perform(DamageTarget(
      amount: 1, target: (kind: AnyPlayer),
    ))),
    (id: "life_tail", kind: Perform(GainLife(who: Controller, amount: 1))),
  ], yields: []), target_bindings: [
    (instruction_id: "counter_spell", role: Spell, target_group_index: 0),
    (instruction_id: "damage_player", role: Target, target_group_index: 1),
  ])),
  targeting: Some((groups: [
    (min: 1, max: 1, prompt: "Choose target spell", effect_indices: []),
    (min: 1, max: 1, prompt: "Choose target player", effect_indices: []),
  ])),
)"#;

fn setup(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["eject", "opt", "grizzly_bears"]),
        deck_with("island", &["an_offer_you_cant_refuse"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "eject");
    ensure_in_hand(&mut engine, 1, "an_offer_you_cant_refuse");
    engine
}

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        expected_zone_change_generation: None,
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: TargetRefKind::Stack as i32,
    }]
}

#[test]
fn issue_478_legal_uncounterable_spell_gives_its_controller_treasure() {
    let mut engine = setup(478_101);
    let permanent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "eject");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(permanent)))
        .unwrap();
    let eject = engine.state.stack.last().unwrap().id;
    engine.apply_scenario_command(0, &pass()).unwrap();
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "an_offer_you_cant_refuse");
    engine
        .apply_scenario_command(1, &cast_spell(slot, stack_target(eject)))
        .unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    assert!(engine.state.stack.iter().any(|item| item.id == eject));
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 2);
    assert!(battlefield_token_oids(&engine, 1, "treasure").is_empty());
}

#[test]
fn issue_478_countered_spell_gives_target_controller_treasure() {
    let mut engine = setup(478_102);
    ensure_in_hand(&mut engine, 0, "opt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "opt");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap();
    let target = engine.state.stack.last().unwrap().id;
    engine.apply_scenario_command(0, &pass()).unwrap();
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "an_offer_you_cant_refuse");
    engine
        .apply_scenario_command(1, &cast_spell(slot, stack_target(target)))
        .unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
    answer_simultaneous_entry_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 2);
    assert!(battlefield_token_oids(&engine, 1, "treasure").is_empty());
}

#[test]
fn issue_478_illegal_stack_target_fizzles_without_treasure() {
    let mut engine = setup(478_103);
    ensure_in_hand(&mut engine, 0, "opt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "opt");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap();
    let target = engine.state.stack.last().unwrap().id;
    engine.apply_scenario_command(0, &pass()).unwrap();
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "an_offer_you_cant_refuse");
    engine
        .apply_scenario_command(1, &cast_spell(slot, stack_target(target)))
        .unwrap();
    // The target leaves the stack before Offer resolves, as after a responding counterspell.
    engine.state.stack.retain(|item| item.id != target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    engine.state.players[0].graveyard.push(target);
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    engine.apply_scenario_command(1, &pass()).unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
    assert!(battlefield_token_oids(&engine, 1, "treasure").is_empty());
}

#[test]
fn issue_478_rejects_creature_spells_at_cast() {
    let mut engine = setup(478_104);
    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap();
    let target = engine.state.stack.last().unwrap().id;
    engine.apply_scenario_command(0, &pass()).unwrap();
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 1, "an_offer_you_cant_refuse");
    let revision = engine.state.command_index;
    assert!(engine
        .apply_scenario_command(1, &cast_spell(slot, stack_target(target)))
        .is_err());
    assert_eq!(engine.state.command_index, revision);
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
}

#[test]
fn issue_478_skipped_counter_group_publishes_none_while_other_target_resolves() {
    let registry = Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                SKIPPED_COUNTER_RECEIPT_FIXTURE,
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/island.ron"),
            ],
            &[],
        )
        .expect("the private skipped-target Program fixture compiles"),
    ));
    let mut engine = GameEngine::new(
        registry,
        478_105,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["opt"]),
            deck_with("island", &["issue_478_skipped_counter_receipt"]),
        ]),
        true,
    )
    .expect("the private fixture and its required deck cards are registered");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "opt");
    ensure_in_hand(&mut engine, 1, "issue_478_skipped_counter_receipt");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let opt = hand_index_for_card(&engine, 0, "opt");
    engine
        .apply_scenario_command(0, &cast_spell(opt, vec![]))
        .unwrap();
    let target_spell = engine.state.stack.last().unwrap().id;
    engine.apply_scenario_command(0, &pass()).unwrap();

    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let receipt_spell = hand_index_for_card(&engine, 1, "issue_478_skipped_counter_receipt");
    let mut player_target = target_player(0).pop().unwrap();
    player_target.group_index = 1;
    let mut targets = stack_target(target_spell);
    targets.push(player_target);
    engine
        .apply_scenario_command(1, &cast_spell(receipt_spell, targets))
        .unwrap();

    // The counter target is illegal, but the still-legal player target keeps the spell resolving.
    engine.state.stack.retain(|item| item.id != target_spell);
    engine.state.objects.get_mut(&target_spell).unwrap().zone = Zone::Graveyard;
    engine.state.players[0].graveyard.push(target_spell);
    *engine
        .state
        .zone_change_generation
        .entry(target_spell)
        .or_default() += 1;
    let observers_before_resolution = engine.state.active_event_observers.len();
    engine.apply_scenario_command(1, &pass()).unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();

    assert_eq!(
        engine.state.players[0].life, 19,
        "the legal player target was damaged"
    );
    assert_eq!(
        engine.state.players[1].life, 21,
        "the spell continued to its tail"
    );
    assert_eq!(
        engine.state.active_event_observers.len(),
        observers_before_resolution,
        "typed None creates no delayed observer and does not fall back to the caster"
    );
}
