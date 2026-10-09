//! Issue #500: card-specific compositions for the six core Atraxa Proliferate cards and Ezuri.
//! Live Oracle/rulings evidence is linked from the seven card definitions in
//! `tricerules-cards/data/`.
//! CR 601.2i / 603.2, 513.1 / 603.2b, 107.4f, and 701.34 govern these scenarios.

use super::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_cards::{CardFace, CardRegistry};
use tricerules_core::state::CopiableValues;
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

fn proliferate_choice(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
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
        .apply_command(0, &play_land(land))
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
        .apply_command(0, &proliferate_choice(vec![bear], vec![1]))
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
            .apply_command(priority_player, &primitive_yield())
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
        .apply_command(0, &proliferate_choice(vec![], vec![0]))
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
        .apply_command(0, &cast_spell(bears, vec![]))
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
        .apply_command(0, &proliferate_choice(vec![countered_creature], vec![]))
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
        .apply_command(0, &cast_spell(bolt, target_player(1)))
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
        .apply_command(0, &proliferate_choice(vec![countered_permanent], vec![]))
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
        .apply_command(0, &cast_spell(bears, vec![]))
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
        .apply_command(
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
        .apply_command(0, &proliferate_choice(vec![], vec![0]))
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
        .apply_command(0, &proliferate_choice(vec![], vec![1]))
        .expect("proliferate the opponent's poison counter");
    let opponent = engine.state.player_idx(1).unwrap();
    assert_eq!(
        engine.state.players[opponent].counters[&CounterKind::Poison],
        2
    );
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
        .apply_command(0, &proliferate_choice(vec![bear], vec![1]))
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
        .apply_command(0, &cast_spell(slot, vec![]))
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
        .apply_command(
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
        .apply_command(0, &cast_spell(slot, vec![]))
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
        .apply_command(0, &select_branch(0))
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
        .apply_command(0, &proliferate_choice(vec![], vec![]))
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
    assert!(engine.apply_command(1, &pass()).is_err());

    engine
        .apply_command(0, &proliferate_choice(vec![bear], vec![0]))
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
