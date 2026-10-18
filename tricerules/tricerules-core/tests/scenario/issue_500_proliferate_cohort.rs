//! Issue #500: card-specific compositions for the six core Atraxa Proliferate cards and Ezuri.
//! Live Oracle/rulings evidence is linked from the seven card definitions in
//! `tricerules-cards/data/`.
//! CR 601.2i / 603.2, 513.1 / 603.2b, 107.4f, and 701.34 govern these scenarios.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, ControllerReference, CounterKind, EffectDuration, Keyword,
};
use tricerules_cards::{CardFace, CardRegistry};
use tricerules_core::state::{AffectedScope, ContinuousEffect, CopiableValues};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::{
    FlexPipPayment, ResolutionChoiceDecision, SubmitResolutionChoice,
};

const COUNTER_REPLACEMENT_FIXTURE_ID: &str = "issue_500_double_counters_fixture";

fn cohort_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("cohort engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn four_player_cohort_engine(seed: u64) -> GameEngine {
    let deck = std::iter::repeat_n("forest".to_string(), 30).collect::<Vec<_>>();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2, 3],
        20,
        Some(vec![deck; 4]),
        true,
    )
    .expect("four-player cohort engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn resolve_ezuri_to_proliferate(engine: &mut GameEngine) {
    inject_card_into_hand(engine, 0, "ezuri,_stalker_of_spheres");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "ezuri,_stalker_of_spheres");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Ezuri");
    pass_priority_round(engine);
    assert_eq!(engine.state.stack.len(), 1, "Ezuri's ETB ability triggers");
    pass_priority_round(engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Ezuri's optional payment choice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select Ezuri's optional {3} payment");
    submit_mana_resolution_decision(engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {3} for Ezuri's two proliferations");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("first Proliferate choice")
            .presentation
            .choice_kind,
        ChoiceKind::Proliferate
    );
}

fn proliferate_choice(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            chosen_object_ids: objects,
            chosen_player_ids: players,
            decision: ResolutionChoiceDecision::Unspecified as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn set_counter_on_player(engine: &mut GameEngine, player_id: i32, kind: CounterKind, count: u32) {
    let player = engine.state.player_idx(player_id).expect("player exists");
    engine.state.players[player].counters.insert(kind, count);
}

fn add_permanent_counter(engine: &mut GameEngine, object_id: u32, kind: CounterKind, count: u32) {
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("permanent exists")
        .counters
        .insert(kind, count);
}

fn counter_replacement_fixture_face() -> CardFace {
    let fixture = r#"(
        id: "issue_500_double_counters_fixture",
        name: "Issue 500 Counter Replacement Fixture",
        face_id: "issue_500_double_counters_fixture",
        types: ["Enchantment"],
        static_abilities: [(
            ability_id: "static_01",
            presentation: Fallback,
            definition: DoubleEffectCountersPlacedOnPermanentsYouControl,
        )],
    )"#;
    CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect("issue #500 counter replacement fixture is valid card data")
        .get(COUNTER_REPLACEMENT_FIXTURE_ID)
        .expect("counter replacement fixture is loaded")
        .primary_face()
        .clone()
}

fn attach_counter_replacement_fixture(engine: &mut GameEngine, object_id: u32) {
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("counter replacement fixture permanent")
        .copiable_values = Some(CopiableValues {
        prototyped: false,
        source_card_id: COUNTER_REPLACEMENT_FIXTURE_ID.into(),
        source_face_index: 0,
        face: counter_replacement_fixture_face(),
        room_faces: None,
        display_name: "Issue 500 Counter Replacement Fixture".into(),
    });
}

#[test]
fn evolution_sage_triggers_on_a_land_entry_and_proliferates_each_existing_counter_kind() {
    let mut engine = cohort_engine(500_001);
    let sage = inject_creature_on_battlefield(&mut engine, 0, "evolution_sage");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    add_permanent_counter(&mut engine, bear, CounterKind::PlusOnePlusOne, 1);
    add_permanent_counter(&mut engine, bear, CounterKind::Stun, 1);
    set_counter_on_player(&mut engine, 1, CounterKind::Poison, 1);

    ensure_in_hand(&mut engine, 0, "forest");
    let land = hand_index_for_card(&engine, 0, "forest");
    engine
        .apply_scenario_command(0, &play_land(land))
        .expect("play a land under Evolution Sage");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the landfall trigger is on the stack"
    );
    pass_both_players(&mut engine);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Proliferate choice");
    assert_eq!(choice.presentation.choice_kind, ChoiceKind::Proliferate);
    assert!(choice.presentation.candidates.contains(&bear));

    engine
        .apply_scenario_command(0, &proliferate_choice(vec![bear], vec![1]))
        .expect("choose a permanent and an opponent");
    assert_eq!(
        engine.state.objects[&bear].counters[&CounterKind::PlusOnePlusOne],
        2
    );
    assert_eq!(engine.state.objects[&bear].counters[&CounterKind::Stun], 2);
    let opponent = engine.state.player_idx(1).unwrap();
    assert_eq!(
        engine.state.players[opponent].counters[&CounterKind::Poison],
        2
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&sage].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn atraxa_triggers_at_its_controllers_end_step() {
    let mut engine = cohort_engine(500_002);
    let atraxa = inject_creature_on_battlefield(&mut engine, 0, "atraxa,_praetors_voice");
    let atraxa_face = tricerules_cards::registry::global()
        .get("atraxa,_praetors_voice")
        .expect("Atraxa definition")
        .primary_face();
    assert_eq!(
        atraxa_face.keywords,
        vec![
            Keyword::Flying,
            Keyword::Vigilance,
            Keyword::Deathtouch,
            Keyword::Lifelink,
        ],
        "Atraxa's four printed keywords are part of its complete card definition"
    );
    set_counter_on_player(&mut engine, 0, CounterKind::Poison, 1);

    for _ in 0..8 {
        if engine.state.turn_step == tricerules_core::TurnStep::EndStep {
            break;
        }
        let priority_player = engine.state.priority_player_id();
        engine
            .apply_scenario_command(priority_player, &primitive_yield())
            .expect("advance toward the end step");
    }
    assert_eq!(engine.state.turn_step, tricerules_core::TurnStep::EndStep);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Atraxa's end-step trigger is on the stack"
    );

    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::Proliferate
    );
    engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![0]))
        .expect("proliferate the controller's poison counter");
    let controller = engine.state.player_idx(0).unwrap();
    assert_eq!(
        engine.state.players[controller].counters[&CounterKind::Poison],
        2
    );
    assert_eq!(
        engine.state.objects[&atraxa].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn inexorable_tide_triggers_on_a_creature_spell_and_resolves_before_it() {
    let mut engine = cohort_engine(500_003);
    inject_permanent_on_battlefield(&mut engine, 0, "inexorable_tide");
    let countered_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    add_permanent_counter(
        &mut engine,
        countered_creature,
        CounterKind::PlusOnePlusOne,
        1,
    );
    inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    grant_pool(&mut engine, 0);
    let bears = hand_index_for_card(&engine, 0, "grizzly_bears");

    engine
        .apply_scenario_command(0, &cast_spell(bears, vec![]))
        .expect("cast a creature spell under Inexorable Tide");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "the trigger is above the creature spell"
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the creature spell remains on the stack"
    );
    let creature_spell = engine.state.stack[0].id;
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Inexorable Tide's Proliferate choice");
    assert_eq!(choice.presentation.choice_kind, ChoiceKind::Proliferate);
    assert!(choice.presentation.candidates.contains(&countered_creature));
    assert_eq!(engine.state.stack[0].card_id, "grizzly_bears");
    assert!(
        !engine.state.players[0]
            .battlefield
            .contains(&creature_spell),
        "the creature spell has not resolved yet"
    );

    engine
        .apply_scenario_command(0, &proliferate_choice(vec![countered_creature], vec![]))
        .expect("choose the permanent for Inexorable Tide's Proliferate");
    assert_eq!(
        engine.state.objects[&countered_creature].counter_count(CounterKind::PlusOnePlusOne),
        2,
        "the creature spell's trigger proliferates before that spell resolves"
    );
    assert!(engine.state.pending_resolution.is_none());

    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 0);
    assert!(
        engine.state.players[0]
            .battlefield
            .contains(&creature_spell),
        "the cast creature spell itself resolved onto the battlefield"
    );
}

#[test]
fn flux_channeler_triggers_for_noncreature_spells_but_not_creatures() {
    let mut engine = cohort_engine(500_004);
    inject_creature_on_battlefield(&mut engine, 0, "flux_channeler");
    let countered_permanent = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    add_permanent_counter(
        &mut engine,
        countered_permanent,
        CounterKind::PlusOnePlusOne,
        1,
    );
    inject_card_into_hand(&mut engine, 0, "lightning_bolt");
    inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    grant_pool(&mut engine, 0);

    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_scenario_command(0, &cast_spell(bolt, target_player(1)))
        .expect("cast a noncreature spell");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "Flux Channeler triggers over the spell"
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&countered_permanent].counters[&CounterKind::PlusOnePlusOne],
        1
    );
    engine
        .apply_scenario_command(0, &proliferate_choice(vec![countered_permanent], vec![]))
        .expect("resolve the noncreature-spell trigger");
    assert_eq!(
        engine.state.objects[&countered_permanent].counters[&CounterKind::PlusOnePlusOne],
        2
    );
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[1].life, 17,
        "the original instant then resolves"
    );

    let bears = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_scenario_command(0, &cast_spell(bears, vec![]))
        .expect("cast a creature spell");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Flux Channeler ignores the creature spell"
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 0);
}

#[test]
fn tezzerets_gambit_draws_before_proliferating_and_accepts_phyrexian_life() {
    let mut engine = cohort_engine(500_005);
    let gambit = inject_card_into_hand(&mut engine, 0, "tezzerets_gambit");
    set_counter_on_player(&mut engine, 0, CounterKind::Poison, 1);
    grant_pool(&mut engine, 0);
    let hand_before_cast = engine.state.players[0].hand.len();
    let slot = hand_index_for_card(&engine, 0, "tezzerets_gambit");

    engine
        .apply_scenario_command(
            0,
            &cast_spell_flex(
                slot,
                vec![],
                vec![FlexPipPayment {
                    pip_index: 1,
                    pay_life: true,
                }],
            ),
        )
        .expect("pay the Phyrexian blue pip with life");
    assert_eq!(engine.state.players[0].life, 18);
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::Proliferate
    );
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_cast + 1,
        "the spell drew two cards before parking at Proliferate"
    );
    engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![0]))
        .expect("choose the player's poison counter");
    let controller = engine.state.player_idx(0).unwrap();
    assert_eq!(
        engine.state.players[controller].counters[&CounterKind::Poison],
        2
    );
    assert_eq!(
        engine.state.objects[&gambit].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn karns_bastion_keeps_its_colorless_mana_and_proliferate_abilities_distinct() {
    let mut engine = cohort_engine(500_006);
    let mana_bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    let proliferate_bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    set_counter_on_player(&mut engine, 1, CounterKind::Poison, 1);
    grant_pool(&mut engine, 0);
    let colorless_before = engine.state.players[0].mana_pool.colorless;

    apply_ability(&mut engine, 0, mana_bastion, 0, vec![]).expect("tap for colorless mana");
    assert_eq!(
        engine.state.players[0].mana_pool.colorless,
        colorless_before + 1
    );
    assert!(engine.state.objects[&mana_bastion].tapped);

    apply_ability(&mut engine, 0, proliferate_bastion, 1, vec![])
        .expect("activate the Proliferate ability");
    assert!(engine.state.objects[&proliferate_bastion].tapped);
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::Proliferate
    );
    engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![1]))
        .expect("proliferate the opponent's poison counter");
    let opponent = engine.state.player_idx(1).unwrap();
    assert_eq!(
        engine.state.players[opponent].counters[&CounterKind::Poison],
        2
    );
}

#[test]
fn karns_bastion_accepts_zero_recipients_while_counters_are_eligible() {
    let mut engine = cohort_engine(500_014);
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    add_permanent_counter(&mut engine, bear, CounterKind::PlusOnePlusOne, 1);
    set_counter_on_player(&mut engine, 1, CounterKind::Poison, 1);
    grant_pool(&mut engine, 0);

    apply_ability(&mut engine, 0, bastion, 1, vec![]).expect("activate Karn's Bastion");
    let batch = pass_both_players_with_batch(&mut engine);
    let offer = find_resolution_choice(&batch).expect("Proliferate recipient choice");
    assert_eq!(offer.choice_kind(), ChoiceKind::Proliferate);
    assert!(offer.candidate_object_ids.contains(&bear));
    assert!(offer.candidate_player_ids.contains(&1));

    engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![]))
        .expect("choosing zero recipients is allowed");
    assert_eq!(
        engine.state.objects[&bear].counters[&CounterKind::PlusOnePlusOne],
        1
    );
    let opponent = engine.state.player_idx(1).expect("opponent");
    assert_eq!(
        engine.state.players[opponent].counters[&CounterKind::Poison],
        1
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn proliferate_observes_counter_replacement_and_poison_state_based_loss() {
    let mut engine = cohort_engine(500_008);
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    let replacement = inject_permanent_on_battlefield(&mut engine, 0, "island");
    attach_counter_replacement_fixture(&mut engine, replacement);
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    add_permanent_counter(&mut engine, bear, CounterKind::PlusOnePlusOne, 1);
    set_counter_on_player(&mut engine, 1, CounterKind::Poison, 9);
    grant_pool(&mut engine, 0);

    apply_ability(&mut engine, 0, bastion, 1, vec![]).expect("activate Karn's Bastion");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &proliferate_choice(vec![bear], vec![1]))
        .expect("double a permanent counter and add the tenth poison counter");

    let opponent = engine.state.player_idx(1).unwrap();
    assert_eq!(
        engine.state.objects[&bear].counters[&CounterKind::PlusOnePlusOne],
        3,
        "the effect counter replacement doubles the one counter placed by Proliferate"
    );
    assert_eq!(
        engine.state.players[opponent].counters[&CounterKind::Poison],
        10
    );
    assert!(engine.state.players[opponent].has_lost);
}

#[test]
fn ezuri_declining_the_optional_branch_does_not_pay_or_proliferate() {
    let mut engine = cohort_engine(500_009);
    let ezuri = inject_card_into_hand(&mut engine, 0, "ezuri,_stalker_of_spheres");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    add_permanent_counter(&mut engine, bear, CounterKind::PlusOnePlusOne, 1);
    set_counter_on_player(&mut engine, 0, CounterKind::Poison, 1);
    grant_pool(&mut engine, 0);
    let hand_before_cast = engine.state.players[0].hand.len();
    let slot = hand_index_for_card(&engine, 0, "ezuri,_stalker_of_spheres");

    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Ezuri");
    pass_both_players(&mut engine);
    assert!(engine.state.players[0].battlefield.contains(&ezuri));
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("optional ETB choice")
            .presentation
            .choice_kind,
        ChoiceKind::ResolutionBranch
    );

    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ezuri's optional payment branch");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_cast - 1,
        "declining does not draw"
    );
    assert_eq!(
        engine.state.objects[&bear].counters[&CounterKind::PlusOnePlusOne],
        1
    );
    assert_eq!(engine.state.players[0].counters[&CounterKind::Poison], 1);
}

#[test]
fn ezuri_pays_once_proliferates_twice_without_priority_and_triggers_even_on_zero() {
    let mut engine = cohort_engine(500_007);
    let ezuri = inject_card_into_hand(&mut engine, 0, "ezuri,_stalker_of_spheres");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    add_permanent_counter(&mut engine, bear, CounterKind::PlusOnePlusOne, 1);
    set_counter_on_player(&mut engine, 0, CounterKind::Poison, 1);
    set_counter_on_player(&mut engine, 1, CounterKind::Poison, 1);
    grant_pool(&mut engine, 0);
    let hand_before_cast = engine.state.players[0].hand.len();
    let slot = hand_index_for_card(&engine, 0, "ezuri,_stalker_of_spheres");

    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Ezuri");
    pass_both_players(&mut engine);
    assert!(engine.state.players[0].battlefield.contains(&ezuri));
    assert_eq!(engine.state.stack.len(), 1, "Ezuri's ETB ability triggers");
    pass_both_players(&mut engine);
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
        .apply_scenario_command(0, &select_branch(0))
        .expect("pay {3} for two proliferations");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::ManaPayment
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay the branch's {3} cost");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::Proliferate
    );
    engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![]))
        .expect("choose zero recipients for the first proliferation");
    assert_eq!(
        engine.state.objects[&bear].counters[&CounterKind::PlusOnePlusOne],
        1
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .choice_kind,
        ChoiceKind::Proliferate,
        "the second action asks for a separate choice without giving priority"
    );
    assert!(engine.apply_scenario_command(1, &pass()).is_err());

    engine
        .apply_scenario_command(0, &proliferate_choice(vec![bear], vec![0]))
        .expect("choose a different set for the second proliferation");
    assert_eq!(
        engine.state.objects[&bear].counters[&CounterKind::PlusOnePlusOne],
        2
    );
    let controller = engine.state.player_idx(0).unwrap();
    let opponent = engine.state.player_idx(1).unwrap();
    assert_eq!(
        engine.state.players[controller].counters[&CounterKind::Poison],
        2
    );
    assert_eq!(
        engine.state.players[opponent].counters[&CounterKind::Poison],
        1
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_cast + 1,
        "both completed Proliferate actions triggered one Ezuri draw each"
    );
}

#[test]
fn ezuri_program_proliferate_delegates_after_controller_leaves_and_keeps_each_choice_fresh() {
    let mut engine = four_player_cohort_engine(500_013);
    inject_creature_on_battlefield(&mut engine, 2, "laezel,_vlaakiths_champion");
    let bear = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    add_permanent_counter(&mut engine, bear, CounterKind::PlusOnePlusOne, 1);
    set_counter_on_player(&mut engine, 2, CounterKind::Poison, 1);
    resolve_ezuri_to_proliferate(&mut engine);

    let original = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("original Proliferate prompt")
        .resolution_prompt_id
        .get();
    assert_ne!(original, 0);
    let tricerules_core::state::ResolutionContinuation::ProgramProliferate {
        candidate_player_ids,
        ..
    } = &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .continuation
    else {
        panic!("typed Proliferate continuation expected");
    };
    assert_eq!(candidate_player_ids, &[2]);
    let departed = engine
        .apply_scenario_command(0, &concede())
        .expect("P0 may leave while its started program Proliferate is parked");
    assert!(engine.state.players[0].has_lost);
    assert!(!engine.state.players[engine.state.player_idx(2).unwrap()].has_lost);
    assert_eq!(
        engine.state.players[engine.state.player_idx(2).unwrap()].counters[&CounterKind::Poison],
        1
    );
    let selector = find_resolution_choice(&departed)
        .expect("the next live player selects a delegate for P0's started choice");
    let tricerules_core::state::ResolutionContinuation::ProgramProliferate {
        candidate_generations,
        candidate_player_ids,
        ..
    } = &engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .continuation
    else {
        panic!("typed Proliferate continuation survives controller concession");
    };
    assert_eq!(candidate_generations.len(), 1);
    assert_eq!(candidate_player_ids, &[2]);
    assert_eq!(selector.deciding_player_id, 1);
    assert_ne!(selector.resolution_prompt_id, 0);
    assert_eq!(selector.choice_kind, ChoiceKind::ResolutionBranch as i32);
    assert_eq!(
        selector
            .resolution_branches
            .iter()
            .map(|branch| branch.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "P1 makes the choice",
            "P2 makes the choice",
            "P3 makes the choice"
        ]
    );

    let selector_answer = SubmitResolutionChoice {
        resolution_prompt_id: selector.resolution_prompt_id,
        decision: ResolutionChoiceDecision::SelectBranch as i32,
        selected_branch_index: 1,
        ..Default::default()
    };
    let delegated = engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(selector_answer.clone())),
            },
        )
        .expect("P1 delegates the original menu to P2");
    let original_menu = find_resolution_choice(&delegated).expect("P2 receives Proliferate");
    assert_eq!(original_menu.deciding_player_id, 2);
    assert_eq!(original_menu.choice_kind, ChoiceKind::Proliferate as i32);
    assert_ne!(original_menu.resolution_prompt_id, 0);
    assert_eq!(original_menu.candidate_player_ids, vec![2]);
    let delegated_state = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(
        (
            delegated_state.presentation.min,
            delegated_state.presentation.max
        ),
        (0, 2),
        "delegate prompt should expose both current recipients"
    );

    let before_stale = engine
        .diagnostic_snapshot()
        .expect("snapshot before stale delegate");
    let seed_before_stale = engine.state.seed;
    let command_before_stale = engine.state.command_index;
    assert!(engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(selector_answer)),
            },
        )
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after stale delegate"),
        before_stale
    );
    assert_eq!(engine.state.seed, seed_before_stale);
    assert_eq!(engine.state.command_index, command_before_stale);

    let first = engine
        .apply_scenario_command(2, &proliferate_choice(vec![bear], vec![2]))
        .expect("P2 chooses the permanent and player recipients");
    assert_eq!(
        engine.state.objects[&bear].counters[&CounterKind::PlusOnePlusOne],
        2,
        "P2's Lae'zel does not apply when P2 merely delegates the departed P0's Proliferate choice"
    );
    assert_eq!(
        engine.state.players[engine.state.player_idx(2).unwrap()].counters[&CounterKind::Poison],
        2
    );
    let next_selector = find_resolution_choice(&first)
        .expect("the next Proliferate operation has its own fresh departed-controller selector");
    assert_eq!(next_selector.deciding_player_id, 1);
    assert!(next_selector.resolution_prompt_id > selector.resolution_prompt_id);

    let before_stale_selector = engine
        .diagnostic_snapshot()
        .expect("snapshot before stale selector");
    assert!(engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: selector.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after stale selector"),
        before_stale_selector
    );

    let self_delegated = engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: next_selector.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("P1 may select themself as the next action's delegate");
    let second_menu = find_resolution_choice(&self_delegated).expect("P1 receives second choice");
    assert_eq!(second_menu.deciding_player_id, 1);
    assert!(second_menu.resolution_prompt_id > next_selector.resolution_prompt_id);
    engine
        .apply_scenario_command(1, &proliferate_choice(vec![], vec![]))
        .expect("P1 chooses zero recipients on the second Proliferate");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ezuri_live_delegate_keeps_unchanged_recipient_offer_after_unrelated_concession() {
    let mut engine = four_player_cohort_engine(500_017);
    set_counter_on_player(&mut engine, 2, CounterKind::Poison, 1);
    resolve_ezuri_to_proliferate(&mut engine);

    let selector_batch = engine
        .apply_scenario_command(0, &concede())
        .expect("the resolving controller may concede after the program began");
    let selector = find_resolution_choice(&selector_batch).expect("P1 selects a delegate");
    assert_eq!(selector.deciding_player_id, 1);
    let delegate_batch = engine
        .apply_scenario_command(1, &select_branch(1))
        .expect("P1 delegates to P2");
    let offer = find_resolution_choice(&delegate_batch).expect("P2 receives the recipient offer");
    assert_eq!(offer.deciding_player_id, 2);
    assert_eq!(offer.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(offer.candidate_player_ids, vec![2]);
    assert_ne!(offer.resolution_prompt_id, 0);

    let unrelated_departure = engine
        .apply_scenario_command(3, &concede())
        .expect("an unrelated non-recipient may concede while P2's offer is open");
    assert!(engine.state.players[engine.state.player_idx(3).unwrap()].has_lost);
    assert!(
        find_resolution_choice(&unrelated_departure).is_none(),
        "an unchanged live delegate offer should not be replaced with a new selector menu"
    );
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the existing recipient offer remains pending");
    assert_eq!(pending.deciding_player, 2);
    assert_eq!(
        pending.resolution_prompt_id.get(),
        offer.resolution_prompt_id
    );
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::Proliferate);
    assert_eq!(pending.presentation.min, 0);
    assert_eq!(pending.presentation.max, 1);

    let first = engine
        .apply_scenario_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: offer.resolution_prompt_id,
                    chosen_player_ids: vec![2],
                    ..Default::default()
                })),
            },
        )
        .expect("the same live delegate may complete the unchanged offer");
    let next_selector = find_resolution_choice(&first).expect("the next operation delegates anew");
    assert_eq!(next_selector.deciding_player_id, 1);
    let second_offer = engine
        .apply_scenario_command(1, &select_branch(0))
        .expect("P1 may self-delegate the second operation");
    assert_eq!(
        find_resolution_choice(&second_offer)
            .unwrap()
            .deciding_player_id,
        1
    );
    engine
        .apply_scenario_command(1, &proliferate_choice(vec![], vec![]))
        .expect("P1 completes the second operation with zero recipients");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ezuri_live_delegate_gets_a_fresh_offer_after_a_recipient_concedes() {
    let mut engine = four_player_cohort_engine(500_018);
    set_counter_on_player(&mut engine, 2, CounterKind::Poison, 1);
    set_counter_on_player(&mut engine, 3, CounterKind::Poison, 1);
    resolve_ezuri_to_proliferate(&mut engine);

    let selector_batch = engine
        .apply_scenario_command(0, &concede())
        .expect("the resolving controller may concede after the program began");
    let selector = find_resolution_choice(&selector_batch).expect("P1 selects a delegate");
    assert_eq!(selector.deciding_player_id, 1);
    assert_ne!(selector.resolution_prompt_id, 0);
    let delegate_batch = engine
        .apply_scenario_command(1, &select_branch(1))
        .expect("P1 delegates to P2");
    let old_offer =
        find_resolution_choice(&delegate_batch).expect("P2 receives the recipient offer");
    assert_eq!(old_offer.deciding_player_id, 2);
    assert_eq!(old_offer.candidate_player_ids, vec![2, 3]);

    let candidate_departure = engine
        .apply_scenario_command(3, &concede())
        .expect("a recipient may concede while the delegate is choosing");
    assert!(engine.state.players[engine.state.player_idx(3).unwrap()].has_lost);
    let refreshed = find_resolution_choice(&candidate_departure)
        .expect("P2 receives a fresh recipient offer after P3 is pruned");
    assert_eq!(refreshed.deciding_player_id, 2);
    assert_eq!(refreshed.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(refreshed.candidate_player_ids, vec![2]);
    assert_ne!(
        refreshed.resolution_prompt_id,
        old_offer.resolution_prompt_id
    );

    let before_stale = engine
        .diagnostic_snapshot()
        .expect("snapshot before the old recipient prompt");
    let seed_before_stale = engine.state.seed;
    let command_before_stale = engine.state.command_index;
    assert!(engine
        .apply_scenario_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: old_offer.resolution_prompt_id,
                    chosen_player_ids: vec![2, 3],
                    ..Default::default()
                })),
            },
        )
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after the stale recipient prompt"),
        before_stale
    );
    assert_eq!(engine.state.seed, seed_before_stale);
    assert_eq!(engine.state.command_index, command_before_stale);

    let first = engine
        .apply_scenario_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: refreshed.resolution_prompt_id,
                    chosen_player_ids: vec![2],
                    ..Default::default()
                })),
            },
        )
        .expect("P2 completes the updated offer with the remaining player");
    let next_selector = find_resolution_choice(&first).expect("the next operation delegates anew");
    assert_eq!(next_selector.deciding_player_id, 1);
    let second_offer = engine
        .apply_scenario_command(1, &select_branch(0))
        .expect("P1 may self-delegate the second operation");
    assert_eq!(
        find_resolution_choice(&second_offer)
            .unwrap()
            .deciding_player_id,
        1
    );
    engine
        .apply_scenario_command(1, &proliferate_choice(vec![], vec![]))
        .expect("P1 completes the second operation with zero recipients");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ezuri_program_proliferate_no_candidate_fast_paths_still_fire_both_events() {
    let mut engine = cohort_engine(500_014);
    let ezuri = inject_card_into_hand(&mut engine, 0, "ezuri,_stalker_of_spheres");
    grant_pool(&mut engine, 0);
    let before_hand = engine.state.players[0].hand.len();
    let slot = hand_index_for_card(&engine, 0, "ezuri,_stalker_of_spheres");
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Ezuri without any counters in the game");
    pass_both_players(&mut engine);
    assert!(engine.state.players[0].battlefield.contains(&ezuri));
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select the optional proliferate program");
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {3} for both Proliferate operations");
    assert!(
        engine.state.pending_resolution.is_none(),
        "each resultless no-candidate Proliferate fast path must complete without fabricating a recipient prompt"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        before_hand + 1,
        "the two Proliferated events each trigger one Ezuri draw"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ezuri_program_choose_payment_readiness_refresh_keeps_prompt_and_pays_once() {
    let mut engine = cohort_engine(500_020);
    let mountains = (0..3)
        .map(|_| inject_permanent_on_battlefield(&mut engine, 0, "mountain"))
        .collect::<Vec<_>>();
    set_counter_on_player(&mut engine, 1, CounterKind::Poison, 1);
    inject_card_into_hand(&mut engine, 0, "ezuri,_stalker_of_spheres");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(
            0,
            &cast_spell(
                hand_index_for_card(&engine, 0, "ezuri,_stalker_of_spheres"),
                vec![],
            ),
        )
        .expect("cast Ezuri with exactly its spell cost");
    let hand_after_cast = engine.state.players[0].hand.len();
    pass_priority_round(&mut engine);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.players[0].mana_pool.white
            + engine.state.players[0].mana_pool.blue
            + engine.state.players[0].mana_pool.black
            + engine.state.players[0].mana_pool.red
            + engine.state.players[0].mana_pool.green
            + engine.state.players[0].mana_pool.colorless,
        0,
        "the mana pool is empty before the original payment offer"
    );
    let payment_offer_batch = engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("select Ezuri's optional {3} payment");

    let initial_offer = find_resolution_choice(&payment_offer_batch)
        .expect("the payment offer is visible before mana is produced");
    assert_eq!(
        initial_offer.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ManaPayment as i32
    );
    assert_eq!(initial_offer.generic_mana_cost, 3);
    assert!(!initial_offer.payment_currently_legal);
    let prompt_id = initial_offer.resolution_prompt_id;
    let chooser = initial_offer.deciding_player_id;
    let source = initial_offer.source_object_id;
    let branch_options = initial_offer.resolution_branches.clone();
    let candidates = initial_offer.candidate_object_ids.clone();
    let initial_pending = engine.state.pending_resolution.as_ref().unwrap();
    let initial_source_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let initial_continuation = format!("{:?}", initial_pending.continuation);
    let mut observed_prompt_ids = Vec::new();
    let mut observed_payment_legality = Vec::new();

    for (index, mountain) in mountains.into_iter().enumerate() {
        let batch = apply_ability(&mut engine, 0, mountain, 0, vec![])
            .expect("each Mountain activation is accepted during the payment choice");
        let offer = find_resolution_choice(&batch)
            .or_else(|| find_resolution_choice(&engine.initial_response_batch()))
            .expect("the current payment offer is available after each activation");
        assert_eq!(offer.deciding_player_id, chooser);
        assert_eq!(offer.source_object_id, source);
        assert_eq!(offer.generic_mana_cost, 3);
        assert_eq!(offer.resolution_branches, branch_options);
        assert_eq!(offer.candidate_object_ids, candidates);
        assert_eq!(offer.payment_currently_legal, index == 2);
        observed_prompt_ids.push(offer.resolution_prompt_id);
        observed_payment_legality.push(offer.payment_currently_legal);
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(
            engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            initial_source_generation,
            "the payment source incarnation is unchanged after mana activation {index}"
        );
        assert_eq!(
            format!("{:?}", pending.continuation),
            initial_continuation,
            "the parked continuation is unchanged after mana activation {index}"
        );
    }
    assert_eq!(observed_payment_legality, vec![false, false, true]);
    assert_eq!(
        observed_prompt_ids,
        vec![prompt_id; 3],
        "mana availability changes only payment readiness, not answer identity"
    );
    assert_eq!(engine.state.players[0].mana_pool.red, 3);

    let mut answer = SubmitResolutionChoice {
        resolution_prompt_id: prompt_id,
        decision: ResolutionChoiceDecision::PayMana as i32,
        ..Default::default()
    };
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the same Ezuri Choose payment remains pending");
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&pending.presentation.source_object_id)
        .copied()
        .unwrap_or(0);
    let proposal = rv1::PaymentSelection {
        expected_state_revision: engine.state.command_index,
        source: Some(rv1::CostObjectRef {
            object_id: pending.presentation.source_object_id,
            zone_change_generation: source_generation,
        }),
        mana: Some(rv1::PaymentMana {
            r: 3,
            ..Default::default()
        }),
        ..Default::default()
    };
    answer.payment = Some(proposal);
    let preview = engine.preview_payment(
        0,
        &rv1::PreviewPayment {
            transaction_id: 1,
            revision: engine.state.command_index,
            resolution_choice: Some(answer.clone()),
            ..Default::default()
        },
    );
    assert!(
        preview.valid,
        "the current prompt accepts a fresh payment preview"
    );
    assert!(preview.complete, "the three red mana pay the generic cost");
    answer.payment = preview.selection;

    let mut stale_revision = answer.clone();
    stale_revision
        .payment
        .as_mut()
        .expect("previewed payment")
        .expected_state_revision -= 1;
    let state_before_stale_revision = format!("{:?}", engine.state);
    let command_index_before_stale_revision = engine.state.command_index;
    assert!(engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(stale_revision)),
            },
        )
        .is_err());
    assert_eq!(format!("{:?}", engine.state), state_before_stale_revision);
    assert_eq!(
        engine.state.command_index,
        command_index_before_stale_revision
    );

    let paid = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(answer.clone())),
            },
        )
        .expect("submit the currently previewed payment");
    let first_proliferate =
        find_resolution_choice(&paid).expect("Ezuri's first Proliferate choice");
    assert_eq!(
        first_proliferate.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::Proliferate as i32
    );
    assert_ne!(first_proliferate.resolution_prompt_id, prompt_id);
    let stale_payment_state = format!("{:?}", engine.state);
    assert!(engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(answer)),
            },
        )
        .is_err());
    assert_eq!(format!("{:?}", engine.state), stale_payment_state);

    engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![1]))
        .expect("resolve Ezuri's first proliferate");
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&2)
    );
    engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![1]))
        .expect("resolve Ezuri's second proliferate");
    assert_eq!(
        engine.state.players[1].counters.get(&CounterKind::Poison),
        Some(&3)
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_after_cast + 2,
        "the paid Program Choose body runs once and produces exactly two proliferate triggers"
    );
}

#[test]
fn ezuri_live_delegate_redelegates_after_the_current_delegate_concedes() {
    let mut engine = four_player_cohort_engine(500_019);
    inject_creature_on_battlefield(&mut engine, 1, "laezel,_vlaakiths_champion");
    set_counter_on_player(&mut engine, 1, CounterKind::Poison, 1);
    resolve_ezuri_to_proliferate(&mut engine);

    let controller_departure = engine
        .apply_scenario_command(0, &concede())
        .expect("the resolving controller may concede while its started program is parked");
    let first_selector = find_resolution_choice(&controller_departure)
        .expect("P1 selects a delegate for the original controller's choice");
    assert_eq!(first_selector.deciding_player_id, 1);
    assert_eq!(
        first_selector
            .resolution_branches
            .iter()
            .map(|branch| branch.label.as_str())
            .collect::<Vec<_>>(),
        vec![
            "P1 makes the choice",
            "P2 makes the choice",
            "P3 makes the choice"
        ]
    );
    let original_offer_batch = engine
        .apply_scenario_command(1, &select_branch(1))
        .expect("P1 delegates the original choice to P2");
    let original_offer = find_resolution_choice(&original_offer_batch)
        .expect("P2 receives the first recipient offer");
    assert_eq!(original_offer.deciding_player_id, 2);
    assert_eq!(original_offer.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(original_offer.candidate_player_ids, vec![1]);

    let delegate_departure = engine
        .apply_scenario_command(2, &concede())
        .expect("the active delegate may leave while selecting Proliferate recipients");
    assert!(engine.state.players[engine.state.player_idx(2).unwrap()].has_lost);
    let after_departure = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the started Proliferate instruction remains parked")
        .resolution_prompt_id
        .get();
    let selector = find_resolution_choice(&delegate_departure)
        .expect("P1 receives a fresh selector after the delegate leaves");
    assert_eq!(after_departure, selector.resolution_prompt_id);
    assert!(selector.resolution_prompt_id > first_selector.resolution_prompt_id);

    let before_stale = engine
        .diagnostic_snapshot()
        .expect("snapshot before the departed delegate's stale answer");
    let seed_before_stale = engine.state.seed;
    let command_before_stale = engine.state.command_index;
    assert!(engine
        .apply_scenario_command(
            2,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: original_offer.resolution_prompt_id,
                    chosen_player_ids: vec![1],
                    ..Default::default()
                })),
            },
        )
        .is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after stale delegate answer"),
        before_stale
    );
    assert_eq!(engine.state.seed, seed_before_stale);
    assert_eq!(engine.state.command_index, command_before_stale);

    assert_eq!(selector.deciding_player_id, 1);
    assert_eq!(
        selector
            .resolution_branches
            .iter()
            .map(|branch| branch.label.as_str())
            .collect::<Vec<_>>(),
        vec!["P1 makes the choice", "P3 makes the choice"]
    );

    let delegated = engine
        .apply_scenario_command(1, &select_branch(1))
        .expect("P1 selects the remaining live delegate P3");
    let offer = find_resolution_choice(&delegated).expect("P3 receives the same Proliferate menu");
    assert_eq!(offer.deciding_player_id, 3);
    assert_eq!(offer.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(offer.candidate_player_ids, vec![1]);
    assert_ne!(offer.resolution_prompt_id, 0);

    let first = engine
        .apply_scenario_command(3, &proliferate_choice(vec![], vec![1]))
        .expect("P3 makes the first choice for the departed P0's program");
    assert_eq!(
        engine.state.players[engine.state.player_idx(1).unwrap()].counters[&CounterKind::Poison],
        2,
        "P1's Lae'zel does not apply just because P3 answers for the departed P0"
    );
    let second_selector = find_resolution_choice(&first)
        .expect("the second Proliferate operation is separately delegated");
    assert_eq!(second_selector.deciding_player_id, 1);
    assert!(second_selector.resolution_prompt_id > selector.resolution_prompt_id);

    let second_offer = engine
        .apply_scenario_command(1, &select_branch(0))
        .expect("P1 may self-delegate the second operation");
    let second_offer = find_resolution_choice(&second_offer).expect("P1 receives the second menu");
    assert_eq!(second_offer.deciding_player_id, 1);
    assert!(second_offer.resolution_prompt_id > second_selector.resolution_prompt_id);
    assert_eq!(second_offer.candidate_player_ids, vec![1]);
    engine
        .apply_scenario_command(1, &proliferate_choice(vec![], vec![1]))
        .expect("P1 completes the second operation exactly once");
    assert_eq!(
        engine.state.players[engine.state.player_idx(1).unwrap()].counters[&CounterKind::Poison],
        3,
        "both Proliferate events retain P0's actor; neither delegated choice doubles P1's counters"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ezuri_program_counter_replacement_reoffers_after_616_1_decider_leaves() {
    let mut engine = four_player_cohort_engine(500_015);
    let p2 = engine.state.players[2].id;
    let p3 = engine.state.players[3].id;
    let target = inject_creature_on_battlefield(&mut engine, 3, "grizzly_bears");
    let doublers = [
        inject_permanent_on_battlefield(&mut engine, 3, "doubling_season"),
        inject_permanent_on_battlefield(&mut engine, 3, "doubling_season"),
        inject_permanent_on_battlefield(&mut engine, 3, "doubling_season"),
    ];
    add_permanent_counter(&mut engine, target, CounterKind::PlusOnePlusOne, 1);
    for (timestamp, object_id) in std::iter::once(target).chain(doublers).enumerate() {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(object_id),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(p2),
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: timestamp as u64,
        });
    }
    assert_eq!(engine.characteristics(target).unwrap().controller, p2);
    assert!(doublers.iter().all(|&object_id| engine
        .characteristics(object_id)
        .unwrap()
        .controller
        == p2));

    resolve_ezuri_to_proliferate(&mut engine);
    let paused = engine
        .apply_scenario_command(0, &proliferate_choice(vec![target], vec![]))
        .expect("P0 chooses the borrowed permanent");
    let old_decider_prompt = find_resolution_choice(&paused)
        .expect("P2 orders its three Doubling Season replacement effects");
    assert_eq!(
        old_decider_prompt.choice_kind,
        ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(old_decider_prompt.deciding_player_id, p2);
    assert_eq!(old_decider_prompt.replacement_options.len(), 3);
    assert!(old_decider_prompt
        .replacement_options
        .iter()
        .all(|option| doublers.contains(&option.source_object_id)));
    let old_prompt_id = old_decider_prompt.resolution_prompt_id;
    let old_application_id = old_decider_prompt.candidate_object_ids[0];

    let departed = engine
        .apply_scenario_command(2, &concede())
        .expect("P2 may leave while the started counter replacement is undecided");
    assert!(engine.state.players[engine.state.player_idx(p2).unwrap()].has_lost);
    assert_eq!(engine.characteristics(target).unwrap().controller, p3);
    assert!(doublers.iter().all(|&object_id| engine
        .characteristics(object_id)
        .unwrap()
        .controller
        == p3));
    let first_refreshed = find_resolution_choice(&departed)
        .expect("P3 receives the current 616.1 order choice after control reverts");
    assert_eq!(
        first_refreshed.choice_kind,
        ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(first_refreshed.deciding_player_id, p3);
    assert_eq!(first_refreshed.replacement_options.len(), 3);
    assert!(first_refreshed
        .replacement_options
        .iter()
        .all(|option| doublers.contains(&option.source_object_id)));

    let before_old_actor = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_scenario_command(
            p2,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: old_prompt_id,
                    chosen_object_ids: vec![old_application_id],
                    ..Default::default()
                })),
            },
        )
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_old_actor);

    let next = engine
        .apply_scenario_command(
            p3,
            &submit_resolution_choice(vec![first_refreshed.candidate_object_ids[0]]),
        )
        .expect("P3 chooses one of the still applicable replacement effects");
    let same_actor_refreshed = find_resolution_choice(&next)
        .expect("two remaining Doubling Seasons require another choice by P3");
    assert_eq!(same_actor_refreshed.deciding_player_id, p3);
    assert_ne!(
        same_actor_refreshed.resolution_prompt_id, first_refreshed.resolution_prompt_id,
        "the same actor receives a fresh prompt after their previous order answer"
    );
    let before_stale_same_actor = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_scenario_command(
            p3,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: first_refreshed.resolution_prompt_id,
                    chosen_object_ids: vec![first_refreshed.candidate_object_ids[1]],
                    ..Default::default()
                })),
            },
        )
        .is_err());
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before_stale_same_actor
    );

    let next_proliferate = engine
        .apply_scenario_command(
            p3,
            &submit_resolution_choice(vec![same_actor_refreshed.candidate_object_ids[0]]),
        )
        .expect("P3 orders the next replacement; the final sole effect applies automatically");
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        9,
        "three Doubling Seasons multiply the one counter added by Proliferate; the original counter remains"
    );
    let second_operation = find_resolution_choice(&next_proliferate)
        .expect("the exact Proliferate program cursor resumes once after replacement order");
    assert_eq!(second_operation.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(second_operation.deciding_player_id, 0);
    engine
        .apply_scenario_command(0, &proliferate_choice(vec![], vec![]))
        .expect("P0 chooses no recipients for the second typed operation");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ezuri_program_counter_replacement_survives_original_controller_departure() {
    let mut engine = four_player_cohort_engine(500_016);
    let p0 = engine.state.players[0].id;
    let p3 = engine.state.players[3].id;
    let target = inject_creature_on_battlefield(&mut engine, 3, "grizzly_bears");
    let doublers = [
        inject_permanent_on_battlefield(&mut engine, 3, "doubling_season"),
        inject_permanent_on_battlefield(&mut engine, 3, "doubling_season"),
    ];
    add_permanent_counter(&mut engine, target, CounterKind::PlusOnePlusOne, 1);

    resolve_ezuri_to_proliferate(&mut engine);
    let first_order = engine
        .apply_scenario_command(0, &proliferate_choice(vec![target], vec![]))
        .expect("P0 chooses P3's permanent");
    let recipient_prompt = find_resolution_choice(&first_order)
        .expect("P3 receives the counter-replacement ordering choice");
    assert_eq!(
        recipient_prompt.choice_kind,
        ChoiceKind::ReplacementEffect as i32
    );
    assert_eq!(recipient_prompt.deciding_player_id, p3);
    assert_eq!(recipient_prompt.replacement_options.len(), 2);
    assert!(recipient_prompt
        .replacement_options
        .iter()
        .all(|option| doublers.contains(&option.source_object_id)));

    let departed = engine
        .apply_scenario_command(0, &concede())
        .expect("P0 may leave while the started program is waiting on P3");
    assert!(engine.state.players[engine.state.player_idx(p0).unwrap()].has_lost);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        p3,
        "the independent CR 616.1 replacement decider remains P3"
    );
    assert!(
        find_resolution_choice(&departed).is_none(),
        "the same live actor and unchanged menu keep their existing prompt"
    );

    let next = engine
        .apply_scenario_command(
            p3,
            &submit_resolution_choice(vec![recipient_prompt.candidate_object_ids[0]]),
        )
        .expect("P3 orders the replacements after the original controller leaves");
    let selector = find_resolution_choice(&next)
        .expect("the parent program advances to the second delegated Proliferate action");
    assert_eq!(selector.choice_kind, ChoiceKind::ResolutionBranch as i32);
    assert_eq!(selector.deciding_player_id, 1);
    assert_ne!(selector.resolution_prompt_id, 0);
    let selected = engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: selector.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("P1 may self-delegate the second Proliferate action");
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        5,
        "two Doubling Seasons multiply the one counter added by Proliferate; the original counter remains"
    );
    let second = find_resolution_choice(&selected).expect("P1 receives the original menu");
    assert_eq!(second.choice_kind, ChoiceKind::Proliferate as i32);
    assert_eq!(second.deciding_player_id, 1);
    engine
        .apply_scenario_command(1, &proliferate_choice(vec![], vec![]))
        .expect("P1 chooses zero recipients");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}
