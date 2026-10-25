//! Equipment receipt Program scenarios. Direct controller edits are explicitly labeled engine fixtures.
use std::sync::OnceLock;

use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, ControllerReference, EffectDuration, PermanentTypeFilter,
    TypeLineAddition, TypeLineReplacement,
};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::{AttachmentRecipient, EngineDeck, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, TargetRef, TargetRefKind,
};

const KYOSHI_BATTLE_FAN: &str = "kyoshi_battle_fan";
const DOUBLING_SEASON: &str = "doubling_season";
const ALLY_TOKEN: &str = "ally_w_1_1";
const GENERIC_EQUIPMENT_REUSE: &str = "generic_equipment_reuse";
const GENERIC_OPTIONAL_EQUIPMENT_CHOICE: &str = "generic_optional_equipment_choice";
const GENERIC_OTHER_EQUIPMENT: &str = "generic_other_equipment";
const GENERIC_OTHER_EQUIPMENT_RON: &str = r#"
(
  id: "generic_other_equipment",
  name: "Generic Other Equipment",
  face_id: "generic_other_equipment",
  mana_cost: "{0}",
  types: ["Artifact", "Equipment"],
)
"#;
const GENERIC_OPTIONAL_EQUIPMENT_CHOICE_RON: &str = r#"
(
  id: "generic_optional_equipment_choice",
  name: "Generic Optional Equipment Choice",
  face_id: "generic_optional_equipment_choice",
  mana_cost: "{2}",
  types: ["Artifact", "Equipment"],
  triggered_abilities: [(
    ability_id: "triggered_01",
    presentation: Fallback,
    trigger: WhenSelfEntersBattlefield,
    effect: [],
    program: Some((
      body: (steps: [
        (id: "tap_target", bind: [], input: None,
          kind: Perform(Tap(subject: Chosen((kind: Creature, controller: Any))))),
        (id: "choose_equipment", bind: ["selected_equipment"], input: None,
          kind: Perform(ChoosePermanents(
            chooser: Controller,
            filter: (kind: AnyPermanent, controller: Any, required_subtypes: ["Equipment"]),
            min: 1,
            max: 1,
            constraints: [EquipmentAttachableTo(
              recipient: ChosenTarget(group_index: 1, target_index: 0))],
          ))),
        (id: "attach_equipment", bind: [], input: Some("selected_equipment"),
          kind: Perform(AttachEquipment(
            equipment: PreviousEffectObject,
            creature: Chosen((kind: Creature, controller: Any)),
          ))),
        (id: "tail", bind: [], input: None, kind: Perform(GainLife(amount: 1))),
      ], yields: []),
      target_bindings: [
        (instruction_id: "tap_target", role: Target, target_group_index: 0),
        (instruction_id: "attach_equipment", role: Target, target_group_index: 1),
      ],
    )),
    modal: None,
    targeting: Some((groups: [
      (min: 1, max: 1, prompt: "Choose a creature to tap", effect_indices: []),
      (min: 0, max: 1, prompt: "Choose an optional creature to equip", effect_indices: []),
    ])),
    may: false,
    intervening_if: None,
    triggers_only_once: false,
    max_triggers_per_turn: None,
  )],
)
"#;

fn generic_equipment_receipt_registry() -> &'static tricerules_cards::CardRegistry {
    static REGISTRY: OnceLock<tricerules_cards::CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let card = include_str!("../../../tricerules-cards/data/generated/k/kyoshi_battle_fan.ron")
            .replace("kyoshi_battle_fan", GENERIC_EQUIPMENT_REUSE)
            .replace("Kyoshi Battle Fan", "Generic Equipment");
        let card = card.replace(
            "token:\"ally_w_1_1\",count:1,who:Controller,tapped:false,sacrifice_timing:None",
            "token:\"soldier_w_1_1\",count:2,who:SourceController,tapped:true,sacrifice_timing:Some(NextEndStep)",
        );
        let card = card.replace(
            "(id:\"attach_created_tokens\",bind:[],input:Some(\"created_tokens\"),kind:Perform(AttachEquipment(equipment:Source,creature:PreviousEffectObjects)))],yields:[])",
            "(id:\"choose_attachment\",bind:[],input:None,kind:Choose(choice:\"attach_created_cohort\",chooser:Controller,optional:false,branches:[(branch_id:\"attach\",presentation:Fallback,cost:None,requirement:Always,body:(steps:[(id:\"attach_created_tokens\",bind:[],input:Some(\"created_tokens\"),kind:Perform(AttachEquipment(equipment:Source,creature:PreviousEffectObjects)))],yields:[]))],otherwise:(steps:[],yields:[]))),(id:\"tail\",bind:[],input:None,kind:Perform(GainLife(amount:1)))],yields:[])",
        );
        assert!(card.contains("token:\"soldier_w_1_1\",count:2"));
        assert!(card.contains("id:\"choose_attachment\""));
        assert!(card.contains("id:\"tail\",bind:[],input:None,kind:Perform(GainLife(amount:1))"));
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[&card],
            &[include_str!("../../../tricerules-cards/data/tokens/soldier_w_1_1.ron")],
        )
        .expect("generic Equipment receipt fixture validates without production card data")
    })
}

fn generic_equipment_receipt_setup(seed: u64) -> GameEngine {
    let decks = vec![
        EngineDeck {
            mainboard: vec![GENERIC_EQUIPMENT_REUSE.into(); 20],
            commanders: vec![],
        };
        2
    ];
    let mut engine = GameEngine::new_with_commander_decks(
        generic_equipment_receipt_registry(),
        seed,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("generic Equipment receipt fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn generic_optional_equipment_choice_registry() -> &'static tricerules_cards::CardRegistry {
    static REGISTRY: OnceLock<tricerules_cards::CardRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                GENERIC_OPTIONAL_EQUIPMENT_CHOICE_RON,
                GENERIC_OTHER_EQUIPMENT_RON,
            ],
            &[include_str!(
                "../../../tricerules-cards/data/tokens/elf_warrior_g_1_1.ron"
            )],
        )
        .expect("generic optional Equipment choice fixture validates")
    })
}

fn generic_optional_equipment_choice_setup(seed: u64) -> GameEngine {
    let decks = vec![
        EngineDeck {
            mainboard: vec![GENERIC_OPTIONAL_EQUIPMENT_CHOICE.into(); 20],
            commanders: vec![],
        };
        2
    ];
    let mut engine = GameEngine::new_with_commander_decks(
        generic_optional_equipment_choice_registry(),
        seed,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("generic optional Equipment choice fixture engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_trigger_targets(ids: &[u32]) -> RuledCommand {
    choose_trigger_target_groups(
        &ids.iter()
            .map(|&object_id| (0, object_id))
            .collect::<Vec<_>>(),
    )
}

fn choose_trigger_target_groups(targets: &[(u32, u32)]) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: targets
                .iter()
                .map(|&(group_index, object_id)| TargetRef {
                    kind: TargetRefKind::Permanent as i32,
                    object_id,
                    group_index,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        })),
    }
}

fn setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn four_player_setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2, 3],
        20,
        None,
        true,
    )
    .expect("four-player engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

/// Change control of the exact Equipment incarnation after its ETB trigger is on the stack.
/// This is an engine-state fixture for the chooser distinction, not legal-card-flow coverage.
fn transfer_equipment_to_p2_fixture(engine: &mut GameEngine, equipment: u32) {
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != equipment);
    engine.state.players[1].battlefield.push(equipment);
    let object = engine
        .state
        .objects
        .get_mut(&equipment)
        .expect("exact Equipment object");
    object.base_controller = engine.state.players[0].id;
    object.controller = engine.state.players[1].id;
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(equipment),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(engine.state.players[1].id),
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    engine.initial_response_batch();
}

fn actual_kyoshi_with_pending_etb(seed: u64, doubled: bool) -> (GameEngine, u32, u64) {
    let mut engine = setup(seed);
    if doubled {
        inject_permanent_on_battlefield(&mut engine, 0, DOUBLING_SEASON);
    }
    inject_card_into_hand(&mut engine, 0, KYOSHI_BATTLE_FAN);
    grant_pool(&mut engine, 0);
    let card_slot = hand_index_for_card(&engine, 0, KYOSHI_BATTLE_FAN);
    engine
        .apply_scenario_command(0, &cast_spell(card_slot, vec![]))
        .expect("cast actual Kyoshi Battle Fan");
    pass_both_players(&mut engine);

    let equipment = battlefield_object_for_card(&engine, 0, KYOSHI_BATTLE_FAN);
    let trigger = engine
        .state
        .stack
        .last()
        .expect("actual Kyoshi ETB trigger");
    assert!(trigger.is_triggered);
    assert_eq!(trigger.source_permanent_id, Some(equipment));
    assert_eq!(trigger.controller, 0);
    let source_generation = trigger.source_zone_change;
    (engine, equipment, source_generation)
}

fn resolve_actual_kyoshi_trigger(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first player passes the pending Kyoshi trigger");
    let resolved = engine
        .apply_scenario_command(second, &pass())
        .expect("resolve actual Kyoshi trigger");
    let entry_order_answers = answer_simultaneous_entry_order_in_engine_order(engine);
    entry_order_answers
        .last()
        .map(|(_, _, batch)| batch.clone())
        .unwrap_or(resolved)
}

fn add_equipment_fixture_effect(
    engine: &mut GameEngine,
    equipment: u32,
    kind: ContinuousEffectKind,
) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(equipment),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn kyoshi_doubling_season_offers_exact_token_cohort_to_equipment_controller() {
    let mut engine = setup(20_261_022);
    inject_permanent_on_battlefield(&mut engine, 0, DOUBLING_SEASON);
    inject_card_into_hand(&mut engine, 0, KYOSHI_BATTLE_FAN);
    grant_pool(&mut engine, 0);
    let card_slot = hand_index_for_card(&engine, 0, KYOSHI_BATTLE_FAN);
    engine
        .apply_scenario_command(0, &cast_spell(card_slot, vec![]))
        .expect("cast actual Kyoshi Battle Fan");

    // Resolve only the Equipment spell, leaving its actual ETB trigger on the stack.
    pass_both_players(&mut engine);
    let equipment = battlefield_object_for_card(&engine, 0, KYOSHI_BATTLE_FAN);
    assert_eq!(engine.state.objects[&equipment].controller, 0);
    assert_eq!(engine.state.stack.len(), 1, "Kyoshi ETB trigger is pending");
    let trigger = engine.state.stack.last().expect("Kyoshi ETB trigger");
    assert!(
        trigger.is_triggered,
        "the pending stack object is Kyoshi's actual trigger"
    );
    assert_eq!(trigger.controller, 0, "P1 controls the triggered ability");
    assert_eq!(trigger.source_permanent_id, Some(equipment));
    let trigger_id = trigger.id;

    transfer_equipment_to_p2_fixture(&mut engine, equipment);
    assert_eq!(engine.state.objects[&equipment].controller, 1);
    let pending_trigger = engine.state.stack.last().expect("same ETB trigger");
    assert_eq!(
        pending_trigger.id, trigger_id,
        "the fixture keeps the same trigger"
    );
    assert_eq!(
        pending_trigger.controller, 0,
        "the trigger remains controlled by P1"
    );
    assert_eq!(pending_trigger.source_permanent_id, Some(equipment));

    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first player passes");
    let resolved = engine
        .apply_scenario_command(second, &pass())
        .expect("resolve Kyoshi ETB trigger");
    let entry_order_answers = answer_simultaneous_entry_order_in_engine_order(&mut engine);
    let resolved = entry_order_answers
        .last()
        .map(|(_, _, batch)| batch.clone())
        .unwrap_or(resolved);

    let mut allies = battlefield_token_oids(&engine, 0, ALLY_TOKEN);
    allies.sort_unstable();
    assert_eq!(
        allies.len(),
        2,
        "Doubling Season doubles the actual token after entry ordering; battlefield={:?}, events={:?}",
        engine
            .state
            .objects
            .values()
            .filter(|object| object.zone == Zone::Battlefield)
            .map(|object| (object.id, object.card_id.as_str(), object.controller, object.is_token()))
            .collect::<Vec<_>>(),
        resolved.events
    );
    assert!(allies.iter().all(|id| {
        engine.state.objects[id].zone == Zone::Battlefield
            && engine.state.objects[id].token_origin.is_some()
    }));

    let choice = find_resolution_choice(&resolved)
        .expect("Kyoshi must offer the two created Allies for attachment");
    assert_eq!(choice.choice_kind(), ChoiceKind::PermanentObjects);
    assert_eq!(
        choice.deciding_player_id, 1,
        "the current Equipment controller chooses"
    );
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(choice.candidate_object_ids, allies);

    let selected = choice.candidate_object_ids[1];
    let prompt_id = choice.resolution_prompt_id;
    let before_malformed = engine
        .diagnostic_snapshot()
        .expect("snapshot before a malformed P2 answer");
    let seed_before_malformed = engine.state.seed;
    let command_before_malformed = engine.state.command_index;
    let mut malformed = submit_resolution_choice(vec![selected]);
    if let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(answer)) =
        malformed.cmd.as_mut()
    {
        answer.resolution_prompt_id = prompt_id;
        answer.chosen_player_ids.push(0);
    }
    assert!(engine.apply_command(1, &malformed).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after malformed P2 answer"),
        before_malformed
    );
    assert_eq!(engine.state.seed, seed_before_malformed);
    assert_eq!(engine.state.command_index, command_before_malformed);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get(),
        prompt_id
    );

    let before_nonmember = engine
        .diagnostic_snapshot()
        .expect("snapshot before P2 submits an object outside the receipt");
    let seed_before_nonmember = engine.state.seed;
    let command_before_nonmember = engine.state.command_index;
    let mut nonmember = submit_resolution_choice(vec![equipment]);
    if let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(answer)) =
        nonmember.cmd.as_mut()
    {
        answer.resolution_prompt_id = prompt_id;
    }
    assert!(engine.apply_command(1, &nonmember).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after nonmember P2 answer"),
        before_nonmember
    );
    assert_eq!(engine.state.seed, seed_before_nonmember);
    assert_eq!(engine.state.command_index, command_before_nonmember);

    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![selected]))
        .expect("P2 chooses one exact created Ally");
    assert_eq!(
        engine.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(selected))
    );
}

#[test]
fn generic_equipment_receipt_uses_generation_safe_chooser_and_resumes_tail_once() {
    let mut engine = generic_equipment_receipt_setup(20_261_029);
    let unrelated = inject_creature_on_battlefield(&mut engine, 0, "soldier_w_1_1");
    inject_card_into_hand(&mut engine, 0, GENERIC_EQUIPMENT_REUSE);
    grant_pool(&mut engine, 0);
    let card_slot = hand_index_for_card(&engine, 0, GENERIC_EQUIPMENT_REUSE);
    engine
        .apply_scenario_command(0, &cast_spell(card_slot, vec![]))
        .expect("cast generic Equipment receipt fixture");
    pass_both_players(&mut engine);

    let equipment = battlefield_object_for_card(&engine, 0, GENERIC_EQUIPMENT_REUSE);
    let resolved = resolve_actual_kyoshi_trigger(&mut engine);
    let mut created_soldiers = battlefield_token_oids(&engine, 0, "soldier_w_1_1");
    created_soldiers.sort_unstable();
    assert_eq!(
        created_soldiers.len(),
        3,
        "the two created tokens and an unrelated Soldier are present"
    );
    created_soldiers.retain(|soldier| *soldier != unrelated);
    assert_eq!(
        created_soldiers.len(),
        2,
        "both tokens in the receipt enter"
    );
    assert!(created_soldiers
        .iter()
        .all(|soldier| engine.state.objects[soldier].tapped));
    let choice = find_resolution_choice(&resolved)
        .expect("the nested child Choose reaches AttachEquipment with the created cohort");
    assert_eq!(choice.candidate_object_ids, created_soldiers);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(
        choice.prompt_text.contains("Generic Equipment"),
        "the shared prompt names the actual source: {}",
        choice.prompt_text
    );

    let selected = created_soldiers[1];
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected]))
        .expect("attach the chosen member of the captured token cohort");
    assert_eq!(
        engine.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(selected))
    );
    assert_eq!(
        engine.state.players[0].life, 21,
        "the Program tail runs once"
    );

    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("main one to combat");
    assert_eq!(engine.state.turn_step, TurnStep::BeginCombat);
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("begin combat");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_scenario_command(0, &primitive_yield())
            .expect("skip attackers");
    }
    assert_eq!(engine.state.turn_step, TurnStep::EndCombat);
    assert!(created_soldiers
        .iter()
        .all(|soldier| engine.state.objects.contains_key(soldier)));
    assert!(engine.state.objects.contains_key(&unrelated));
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("end combat to main two");
    assert_eq!(engine.state.turn_step, TurnStep::Main2);
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("main two to end step");
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the delayed sacrifice trigger is pending"
    );
    resolve_entire_stack_two_player(&mut engine);
    for soldier in created_soldiers {
        assert!(
            !engine.state.objects.contains_key(&soldier),
            "only the exact token receipt cohort is sacrificed"
        );
    }
    assert!(
        engine.state.objects.contains_key(&unrelated),
        "the pre-existing same-token Soldier is outside the observed cohort"
    );
    assert!(engine.state.observed_object_cohorts.is_empty());
}

#[test]
fn optional_equipment_choice_without_a_legal_creature_completes_empty_and_runs_tail_once() {
    let mut engine = generic_optional_equipment_choice_setup(20_261_030);
    let required_target = inject_creature_on_battlefield(&mut engine, 0, "elf_warrior_g_1_1");
    inject_card_into_hand(&mut engine, 0, GENERIC_OPTIONAL_EQUIPMENT_CHOICE);
    grant_pool(&mut engine, 0);
    let card_slot = hand_index_for_card(&engine, 0, GENERIC_OPTIONAL_EQUIPMENT_CHOICE);
    engine
        .apply_scenario_command(0, &cast_spell(card_slot, vec![]))
        .expect("cast optional Equipment choice fixture");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &choose_trigger_targets(&[required_target]))
        .expect("choose the required group 0 target and omit optional group 1");
    let equipment = battlefield_object_for_card(&engine, 0, GENERIC_OPTIONAL_EQUIPMENT_CHOICE);

    let resolved = resolve_actual_kyoshi_trigger(&mut engine);

    assert!(find_resolution_choice(&resolved).is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.players[0].life, 21,
        "the tail resumes exactly once"
    );
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
    assert!(engine.state.objects[&required_target].tapped);
}

#[test]
fn optional_equipment_choice_attaches_to_group_one_target() {
    let mut engine = generic_optional_equipment_choice_setup(20_261_031);
    let tap_target = inject_creature_on_battlefield(&mut engine, 0, "elf_warrior_g_1_1");
    let attach_target = inject_creature_on_battlefield(&mut engine, 0, "elf_warrior_g_1_1");
    let other_equipment = inject_permanent_on_battlefield(&mut engine, 0, GENERIC_OTHER_EQUIPMENT);
    inject_card_into_hand(&mut engine, 0, GENERIC_OPTIONAL_EQUIPMENT_CHOICE);
    grant_pool(&mut engine, 0);
    let card_slot = hand_index_for_card(&engine, 0, GENERIC_OPTIONAL_EQUIPMENT_CHOICE);
    engine
        .apply_scenario_command(0, &cast_spell(card_slot, vec![]))
        .expect("cast optional Equipment choice fixture");
    pass_both_players(&mut engine);
    let equipment = battlefield_object_for_card(&engine, 0, GENERIC_OPTIONAL_EQUIPMENT_CHOICE);
    engine
        .apply_scenario_command(
            0,
            &choose_trigger_target_groups(&[(0, tap_target), (1, attach_target)]),
        )
        .expect("choose distinct creatures for group 0 and the Equipment's group 1 recipient");

    let resolved = resolve_actual_kyoshi_trigger(&mut engine);
    let choice = find_resolution_choice(&resolved)
        .expect("group 1's selected creature makes the Equipment choice legal");
    assert_eq!(choice.candidate_object_ids.len(), 2);
    assert!(choice.candidate_object_ids.contains(&equipment));
    assert!(choice.candidate_object_ids.contains(&other_equipment));
    assert_eq!((choice.min, choice.max), (1, 1));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![equipment]))
        .expect("choose this Equipment for the selected group 1 creature");

    assert!(engine.state.objects[&tap_target].tapped);
    assert_eq!(
        engine.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(attach_target))
    );
    assert_eq!(
        engine.state.players[0].life, 21,
        "the Program tail runs once after the group 1 attachment"
    );
}

#[test]
fn kyoshi_single_created_ally_auto_attaches_without_a_choice() {
    let (mut engine, equipment, _source_generation) =
        actual_kyoshi_with_pending_etb(20_261_026, false);
    let resolved = resolve_actual_kyoshi_trigger(&mut engine);

    let allies = battlefield_token_oids(&engine, 0, ALLY_TOKEN);
    assert_eq!(
        allies.len(),
        1,
        "actual Kyoshi creates one Ally without Doubling Season"
    );
    assert!(engine.state.objects[&allies[0]].token_origin.is_some());
    assert_eq!(
        engine.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(allies[0]))
    );
    assert!(find_resolution_choice(&resolved).is_none());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn kyoshi_pre_trigger_invalid_source_still_creates_tokens_without_attachment_or_prompt() {
    #[derive(Clone, Copy)]
    enum SourceFixture {
        Gone,
        LeftAndReturned,
        NoLongerEquipment,
        CreatureEquipment,
    }

    for (index, fixture) in [
        SourceFixture::Gone,
        SourceFixture::LeftAndReturned,
        SourceFixture::NoLongerEquipment,
        SourceFixture::CreatureEquipment,
    ]
    .into_iter()
    .enumerate()
    {
        let (mut engine, equipment, source_generation) =
            actual_kyoshi_with_pending_etb(20_261_027 + index as u64, true);
        assert_eq!(
            engine.state.stack.last().unwrap().source_zone_change,
            source_generation,
            "the pending trigger retains the exact source generation"
        );

        match fixture {
            SourceFixture::Gone => {
                // Direct state fixture: remove the physical Equipment while its actual ETB ability
                // stays on the stack, and give the departed object a new zone-change generation.
                for player in &mut engine.state.players {
                    player
                        .battlefield
                        .retain(|object_id| *object_id != equipment);
                }
                let owner = engine.state.objects[&equipment].owner;
                let owner_index = engine.state.player_idx(owner).expect("Equipment owner");
                engine.state.players[owner_index].graveyard.push(equipment);
                let object = engine.state.objects.get_mut(&equipment).unwrap();
                object.zone = Zone::Graveyard;
                object.attached_to = None;
                *engine
                    .state
                    .zone_change_generation
                    .entry(equipment)
                    .or_default() += 1;
            }
            SourceFixture::LeftAndReturned => {
                // Direct state fixture: same ObjectId, but the source now has the generation of a
                // new CR 400.7 battlefield incarnation after leaving and returning.
                *engine
                    .state
                    .zone_change_generation
                    .entry(equipment)
                    .or_default() += 2;
            }
            SourceFixture::NoLongerEquipment => {
                add_equipment_fixture_effect(
                    &mut engine,
                    equipment,
                    ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
                        card_types: vec![PermanentTypeFilter::Artifact],
                        creature_types: Vec::new(),
                        land_types: Vec::new(),
                    }),
                );
                assert!(!engine
                    .characteristics(equipment)
                    .unwrap()
                    .has_type("Equipment"));
            }
            SourceFixture::CreatureEquipment => {
                add_equipment_fixture_effect(
                    &mut engine,
                    equipment,
                    ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                        card_types: vec![PermanentTypeFilter::Creature],
                        creature_types: vec!["Human".into()],
                        land_types: Vec::new(),
                    }),
                );
                add_equipment_fixture_effect(
                    &mut engine,
                    equipment,
                    ContinuousEffectKind::Layer7bSetPt {
                        power: 1,
                        toughness: 1,
                    },
                );
                let characteristics = engine.characteristics(equipment).unwrap();
                assert!(characteristics.has_type("Equipment"));
                assert!(characteristics.is_creature());
            }
        }
        engine.initial_response_batch();

        let resolved = resolve_actual_kyoshi_trigger(&mut engine);
        let allies = battlefield_token_oids(&engine, 0, ALLY_TOKEN);
        assert_eq!(
            allies.len(),
            2,
            "source fixture {index} must still resolve Kyoshi's doubled token instruction"
        );
        assert!(allies.iter().all(|id| {
            engine.state.objects[id].zone == Zone::Battlefield
                && engine.state.objects[id].token_origin.is_some()
        }));
        assert_eq!(engine.state.objects[&equipment].attached_to, None);
        assert!(find_resolution_choice(&resolved).is_none());
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn kyoshi_departed_p2_chooser_skips_already_departed_p3_for_p4_after_source_reverts() {
    let mut engine = four_player_setup(20_261_024);
    inject_permanent_on_battlefield(&mut engine, 0, DOUBLING_SEASON);
    inject_card_into_hand(&mut engine, 0, KYOSHI_BATTLE_FAN);
    grant_pool(&mut engine, 0);
    let card_slot = hand_index_for_card(&engine, 0, KYOSHI_BATTLE_FAN);
    engine
        .apply_scenario_command(0, &cast_spell(card_slot, vec![]))
        .expect("cast actual Kyoshi Battle Fan");
    pass_priority_round(&mut engine);
    let equipment = battlefield_object_for_card(&engine, 0, KYOSHI_BATTLE_FAN);
    transfer_equipment_to_p2_fixture(&mut engine, equipment);
    pass_priority_round(&mut engine);

    let initial = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("two exact created Allies require a chooser");
    assert_eq!(initial.deciding_player, 1, "P2 controls the Equipment");
    let old_prompt_id = initial.resolution_prompt_id.get();
    let allies = initial.presentation.candidates.clone();
    assert_eq!(allies.len(), 2, "Doubling Season creates two exact Allies");

    engine
        .apply_scenario_command(2, &concede())
        .expect("P3, who is not the chooser, leaves after P2's offer");
    let unchanged = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P2 keeps the unchanged offer");
    assert_eq!(unchanged.deciding_player, 1);
    assert_eq!(unchanged.resolution_prompt_id.get(), old_prompt_id);
    assert_eq!(unchanged.presentation.candidates, allies);

    let p2_departure = engine
        .apply_scenario_command(1, &concede())
        .expect("offered P2 leaves after P3 has already departed");
    assert_eq!(engine.state.objects[&equipment].controller, 0);
    let p4_offer = find_resolution_choice(&p2_departure)
        .expect("the captured seat cursor skips P3 and offers P4");
    assert_eq!(p4_offer.deciding_player_id, 3);
    assert_eq!(p4_offer.candidate_object_ids, allies);
    assert_ne!(p4_offer.resolution_prompt_id, 0);
    assert_ne!(
        p4_offer.resolution_prompt_id, old_prompt_id,
        "a delegated offer must not reuse the displaced P2 prompt ID"
    );

    let before_stale_authorized = engine
        .diagnostic_snapshot()
        .expect("snapshot before P4 submits P2's old prompt ID");
    let seed_before_stale_authorized = engine.state.seed;
    let command_before_stale_authorized = engine.state.command_index;
    assert!(engine
        .apply_command(
            3,
            &tricerules_proto::ruled::v1::RuledCommand {
                cmd: Some(
                    tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(
                        tricerules_proto::ruled::v1::SubmitResolutionChoice {
                            resolution_prompt_id: old_prompt_id,
                            chosen_object_ids: vec![allies[0]],
                            ..Default::default()
                        },
                    )
                ),
            },
        )
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after P4 submits stale P2 ID"),
        before_stale_authorized
    );
    assert_eq!(engine.state.seed, seed_before_stale_authorized);
    assert_eq!(engine.state.command_index, command_before_stale_authorized);

    let before_stale = engine
        .diagnostic_snapshot()
        .expect("snapshot before the departed P2's stale answer");
    let seed_before_stale = engine.state.seed;
    let command_before_stale = engine.state.command_index;
    assert!(engine
        .apply_command(
            1,
            &tricerules_proto::ruled::v1::RuledCommand {
                cmd: Some(
                    tricerules_proto::ruled::v1::ruled_command::Cmd::SubmitResolutionChoice(
                        tricerules_proto::ruled::v1::SubmitResolutionChoice {
                            resolution_prompt_id: old_prompt_id,
                            chosen_object_ids: vec![allies[0]],
                            ..Default::default()
                        },
                    )
                ),
            },
        )
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after stale P2 answer"),
        before_stale
    );
    assert_eq!(engine.state.seed, seed_before_stale);
    assert_eq!(engine.state.command_index, command_before_stale);

    engine
        .apply_scenario_command(3, &submit_resolution_choice(vec![allies[1]]))
        .expect("P4 chooses one exact Ally after P2 leaves");
    assert_eq!(
        engine.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(allies[1]))
    );
}
