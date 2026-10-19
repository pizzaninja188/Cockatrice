use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

fn cast_unsummon_at_dirgur(seed: u64) -> (GameEngine, u32, u32) {
    let decks = vec![
        deck_with("island", &["unsummon"]),
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let dirgur = relocate_to_battlefield(
        &mut engine,
        1,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[unsummon];
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(dirgur)))
        .expect("cast Unsummon targeting Dirgur");
    (engine, dirgur, spell_id)
}

fn remove_spell_from_stack(engine: &mut GameEngine, spell_id: u32) {
    engine.state.stack.retain(|item| item.id != spell_id);
    engine.state.players[0].graveyard.push(spell_id);
    engine
        .state
        .objects
        .get_mut(&spell_id)
        .expect("targeting spell")
        .zone = tricerules_core::Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(spell_id)
        .or_default() += 1;
}

fn cast_unsummon_at_dirgur_three_player(seed: u64) -> (GameEngine, u32, u32) {
    let decks = vec![
        deck_with("island", &["unsummon"]),
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
        deck_with("swamp", &[]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(decks),
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    let dirgur = relocate_to_battlefield(
        &mut engine,
        1,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[unsummon];
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(dirgur)))
        .expect("cast Unsummon targeting Dirgur");
    (engine, dirgur, spell_id)
}

fn cast_unsummon_at_foreign_controlled_ward(
    seed: u64,
    ward_card_id: &str,
) -> (GameEngine, u32, u32, Option<u32>) {
    assert!(matches!(
        ward_card_id,
        "dirgur_island_dragon_skimming_strike" | "spectral_snatcher"
    ));
    let decks = vec![
        deck_with("island", &["unsummon", "grizzly_bears"]),
        deck_with("swamp", &[]),
        deck_with("island", &[]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        Some(decks),
        true,
    )
    .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    let ward_source = inject_creature_under_foreign_control(&mut engine, 1, 2, ward_card_id);
    let discard = if ward_card_id == "spectral_snatcher" {
        Some(relocate_to_hand(&mut engine, 0, "grizzly_bears"))
    } else {
        None
    };
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[unsummon];
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(ward_source)))
        .expect("cast Unsummon at an actual foreign-owned Ward source");
    (engine, ward_source, spell_id, discard)
}

fn synthetic_ward_program_engine(seed: u64, cost: &str) -> GameEngine {
    let definition = format!(
        r#"(id: "synthetic_ward_program_tail", name: "Synthetic Ward Program Tail",
            face_id: "synthetic_ward_program_tail", mana_cost: "{{0}}",
            types: ["Creature"], power: 2, toughness: 2,
            triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
                trigger: WheneverSelfBecomesTarget(source: SpellOrAbility, source_controller: Opponent),
                effect: [], program: Some((body: (steps: [
                    (id: "ward", kind: Perform(CounterTriggeringStackObjectUnlessPays(cost: {cost}))),
                    (id: "tail_gain", kind: Perform(GainLife(who: Controller, amount: 1)))
                ], yields: []))), modal: None, targeting: None, may: false,
                intervening_if: None, triggers_only_once: false, max_triggers_per_turn: None
            )])"#
    );
    let registry = Box::leak(Box::new(
        tricerules_card_model::CardRegistry::from_chunks_and_tokens(
            &[
                &definition,
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/unsummon.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
            ],
            &[],
        )
        .expect("synthetic Ward Program tail fixture is admitted"),
    ));
    let decks = Some(vec![vec!["island".into(); 30], vec!["island".into(); 30]]);
    GameEngine::new(registry, seed, &[0, 1], 20, decks, true)
        .expect("synthetic Ward Program tail engine")
}

fn cast_unsummon_at_spectral_snatcher(seed: u64) -> (GameEngine, u32, u32, u32) {
    let decks = vec![
        deck_with("island", &["unsummon", "grizzly_bears"]),
        deck_with("swamp", &["spectral_snatcher"]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let snatcher = relocate_to_battlefield(&mut engine, 1, "spectral_snatcher", false);
    ensure_in_hand(&mut engine, 0, "unsummon");
    let discard = relocate_to_hand(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[unsummon];
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(snatcher)))
        .expect("cast Unsummon targeting Spectral Snatcher");
    (engine, snatcher, spell_id, discard)
}

#[test]
fn opponent_targeting_dirgur_creates_a_public_ward_trigger() {
    let (engine, _, _) = cast_unsummon_at_dirgur(103_001);
    assert_eq!(engine.state.stack.len(), 2, "Ward must be above Unsummon");
    let ward = engine.state.stack.last().expect("Ward trigger");
    assert!(ward.is_triggered);
    assert_eq!(
        ward.ability_text.as_deref(),
        Some("Dirgur Island Dragon — triggered ability (triggered_01)")
    );
}

#[test]
fn paying_mana_preserves_the_exact_targeting_spell() {
    let (mut engine, dirgur, spell_id) = cast_unsummon_at_dirgur(103_002);
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward payment");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::ManaPayment);
    let payment = pending
        .continuation
        .mana_payment()
        .expect("Ward mana payment");
    assert_eq!(payment.generic_mana_cost, 2);
    assert!(
        payment.mana_cost.is_empty(),
        "pure generic Ward must reuse the auto-completing generic payment flow"
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay Ward");
    assert_eq!(
        engine
            .state
            .turn_history
            .current
            .player(0)
            .mana_spent_casting_spells,
        1,
        "only Unsummon's mana counts for Expend, not Ward's two mana"
    );
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));
    assert!(engine.state.pending_resolution.is_none());

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&dirgur].zone,
        tricerules_core::Zone::Hand
    );
}

#[test]
fn declining_mana_counters_the_exact_targeting_spell() {
    let (mut engine, _, spell_id) = cast_unsummon_at_dirgur(103_003);
    pass_both_players(&mut engine);
    let batch = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ward");

    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == spell_id
    )));
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == spell_id
    )));

    assert!(!engine.state.stack.iter().any(|item| item.id == spell_id));
    assert_eq!(
        engine.state.objects[&spell_id].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn paying_discard_moves_the_selected_physical_card_and_preserves_the_spell() {
    let (mut engine, _, spell_id, discard) = cast_unsummon_at_spectral_snatcher(103_004);
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward discard payment");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert!(pending.presentation.candidates.contains(&discard));

    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discard]))
        .expect("discard to pay Ward");
    assert_eq!(
        engine.state.objects[&discard].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn stale_discard_submission_is_rejected_without_clearing_the_choice() {
    let (mut engine, _, spell_id, discard) = cast_unsummon_at_spectral_snatcher(103_005);
    pass_both_players(&mut engine);
    engine.state.players[0]
        .hand
        .retain(|object_id| *object_id != discard);
    engine
        .state
        .objects
        .get_mut(&discard)
        .expect("discard candidate")
        .zone = tricerules_core::Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(discard)
        .or_default() += 1;

    let error = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discard]))
        .expect_err("stale physical card must be rejected");
    assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));
}

#[test]
fn ward_does_not_trigger_for_its_controllers_spell() {
    let decks = vec![
        deck_with(
            "island",
            &["unsummon", "dirgur_island_dragon_skimming_strike"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        103_006,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let dirgur = relocate_to_battlefield(
        &mut engine,
        0,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(dirgur)))
        .expect("target own Ward permanent");
    assert_eq!(engine.state.stack.len(), 1);
    assert!(engine.state.pending_trigger_order.is_none());
}

#[test]
fn ward_persists_after_its_source_leaves_the_battlefield() {
    let (mut engine, dirgur, spell_id) = cast_unsummon_at_dirgur(103_007);
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != dirgur);
    engine.state.players[1].graveyard.push(dirgur);
    engine.state.objects.get_mut(&dirgur).expect("Dirgur").zone = tricerules_core::Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(dirgur)
        .or_default() += 1;

    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ward after source departure");
    assert!(!engine.state.stack.iter().any(|item| item.id == spell_id));
}

#[test]
fn begun_ward_payment_survives_its_controller_and_source_owner_conceding() {
    let (mut engine, _, spell_id) = cast_unsummon_at_dirgur_three_player(103_071);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Ward starts its payment")
            .deciding_player,
        0
    );

    engine
        .apply_scenario_command(1, &concede())
        .expect("Ward controller and source owner may concede during the begun payment");
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the started Ward transaction remains with its payer");
    assert_eq!(pending.deciding_player, 0);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("payer completes Ward after its controller leaves");
    assert!(engine.state.pending_resolution.is_none());
    assert!(
        engine.state.stack.iter().any(|item| item.id == spell_id),
        "paying the preserved Ward transaction keeps the exact targeting spell"
    );
}

#[test]
fn departed_ward_payer_does_not_leave_a_stale_payment_for_an_absent_target() {
    let (mut engine, _, spell_id) = cast_unsummon_at_dirgur_three_player(103_072);
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_some());

    engine
        .apply_scenario_command(0, &concede())
        .expect("the Ward payer may concede while the payment is open");
    assert!(
        !engine.state.stack.iter().any(|item| item.id == spell_id),
        "the departing payer's owned targeting spell leaves the stack"
    );
    assert!(
        engine.state.pending_resolution.is_none(),
        "departure forces the payment outcome and never retains a stale Ward prompt"
    );
}

#[test]
fn ward_forces_unpaid_when_its_payer_leaves_before_resolution_begins() {
    let (mut engine, _, spell_id) = cast_unsummon_at_dirgur_three_player(103_078);
    engine
        .apply_scenario_command(0, &concede())
        .expect("the spell controller may leave before Ward begins resolving");
    assert!(!engine.state.stack.iter().any(|item| item.id == spell_id));

    pass_priority_round(&mut engine);
    assert!(engine.state.players[0].has_lost);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn foreign_owned_mana_ward_survives_controller_departure_during_payment() {
    let (mut engine, _, spell_id, _) =
        cast_unsummon_at_foreign_controlled_ward(103_073, "dirgur_island_dragon_skimming_strike");
    pass_priority_round(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.presentation.choice_kind == ChoiceKind::ManaPayment));

    engine
        .apply_scenario_command(2, &concede())
        .expect("the Ward controller may leave while its begun payment is parked");
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.deciding_player == 0));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("the payer completes Ward after its distinct controller leaves");
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));
}

#[test]
fn foreign_owned_mana_ward_survives_source_owner_departure_during_payment() {
    let (mut engine, _, spell_id, _) =
        cast_unsummon_at_foreign_controlled_ward(103_074, "dirgur_island_dragon_skimming_strike");
    pass_priority_round(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.presentation.choice_kind == ChoiceKind::ManaPayment));

    engine
        .apply_scenario_command(1, &concede())
        .expect("the physical Ward source owner may leave while its controller remains");
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.deciding_player == 0));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("the payer completes Ward after its distinct source owner leaves");
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));
}

#[test]
fn foreign_owned_discard_ward_survives_controller_departure_during_payment() {
    let (mut engine, _, spell_id, discard) =
        cast_unsummon_at_foreign_controlled_ward(103_075, "spectral_snatcher");
    let discard = discard.expect("Spectral Snatcher discard payment card");
    pass_priority_round(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.presentation.choice_kind == ChoiceKind::HandCards));

    engine
        .apply_scenario_command(2, &concede())
        .expect("the Ward controller may leave while its begun discard payment is parked");
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.deciding_player == 0));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discard]))
        .expect("the payer completes discard Ward after its distinct controller leaves");
    assert_eq!(
        engine.state.objects[&discard].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));
}

#[test]
fn foreign_owned_discard_ward_survives_source_owner_departure_during_payment() {
    let (mut engine, _, spell_id, discard) =
        cast_unsummon_at_foreign_controlled_ward(103_076, "spectral_snatcher");
    let discard = discard.expect("Spectral Snatcher discard payment card");
    pass_priority_round(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.presentation.choice_kind == ChoiceKind::HandCards));

    engine
        .apply_scenario_command(1, &concede())
        .expect("the physical Ward source owner may leave while its controller remains");
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.deciding_player == 0));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discard]))
        .expect("the payer completes discard Ward after its distinct source owner leaves");
    assert_eq!(
        engine.state.objects[&discard].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));
}

#[test]
fn departed_discard_ward_payer_does_not_leave_a_stale_choice_for_an_absent_target() {
    let (mut engine, _, spell_id, _) =
        cast_unsummon_at_foreign_controlled_ward(103_077, "spectral_snatcher");
    pass_priority_round(&mut engine);
    assert!(engine
        .state
        .pending_resolution
        .as_ref()
        .is_some_and(|pending| pending.presentation.choice_kind == ChoiceKind::HandCards));

    engine
        .apply_scenario_command(0, &concede())
        .expect("the discard Ward payer may concede while the choice is open");
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.stack.iter().any(|item| item.id == spell_id));
}

fn run_synthetic_ward_program_tail(case: &str, cost: &str) {
    let mut engine = synthetic_ward_program_engine(103_100, cost);
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 1, "synthetic_ward_program_tail");
    let discard = if case.starts_with("discard") {
        Some(inject_card_into_hand(&mut engine, 0, "grizzly_bears"))
    } else {
        None
    };
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[unsummon];
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(source)))
        .unwrap_or_else(|error| panic!("{case}: cast targeting spell: {error}"));
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap_or_else(|| panic!("{case}: Ward Program payment is open"));
    let choice_kind = pending.presentation.choice_kind;
    if case.starts_with("mana") {
        assert_eq!(choice_kind, ChoiceKind::ManaPayment, "{case}");
        if case == "mana_pay" {
            give_mana(
                &mut engine,
                0,
                ManaGift {
                    c: 2,
                    ..Default::default()
                },
            );
            submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
                .unwrap_or_else(|error| panic!("{case}: pay Ward: {error}"));
        } else {
            engine
                .apply_scenario_command(
                    0,
                    &submit_resolution_decision(ResolutionChoiceDecision::Decline),
                )
                .unwrap_or_else(|error| panic!("{case}: decline Ward: {error}"));
        }
    } else {
        assert_eq!(choice_kind, ChoiceKind::HandCards, "{case}");
        if case == "discard_pay" {
            let discard = discard.expect("discard fixture object");
            assert!(pending.presentation.candidates.contains(&discard));
            engine
                .apply_scenario_command(0, &submit_resolution_choice(vec![discard]))
                .unwrap_or_else(|error| panic!("{case}: discard to pay Ward: {error}"));
            assert_eq!(
                engine.state.objects[&discard].zone,
                tricerules_core::Zone::Graveyard,
                "the exact physical payment card leaves hand"
            );
        } else {
            engine
                .apply_scenario_command(
                    0,
                    &submit_resolution_decision(ResolutionChoiceDecision::Decline),
                )
                .unwrap_or_else(|error| panic!("{case}: decline Ward: {error}"));
        }
    }

    assert!(engine.state.pending_resolution.is_none(), "{case}");
    assert_eq!(
        engine.state.players[1].life, 21,
        "{case}: the Program tail runs exactly once after the Ward payment"
    );
    assert_eq!(
        engine.state.stack.iter().any(|item| item.id == spell_id),
        case.ends_with("pay"),
        "{case}: payment preserves the exact targeting spell; declining counters it"
    );
}

#[test]
fn synthetic_ward_program_tail_resumes_once_after_mana_pay() {
    run_synthetic_ward_program_tail("mana_pay", "Mana(\"{2}\")");
}

#[test]
fn synthetic_ward_program_tail_resumes_once_after_mana_decline() {
    run_synthetic_ward_program_tail("mana_decline", "Mana(\"{2}\")");
}

#[test]
fn synthetic_ward_program_tail_resumes_once_after_discard_pay() {
    run_synthetic_ward_program_tail("discard_pay", "DiscardCard(filter: None)");
}

#[test]
fn synthetic_ward_program_tail_resumes_once_after_discard_decline() {
    run_synthetic_ward_program_tail("discard_decline", "DiscardCard(filter: None)");
}

#[test]
fn ward_mana_decline_remains_available_when_the_bound_stack_object_is_absent() {
    let (mut engine, _, spell_id) = cast_unsummon_at_dirgur(103_008);
    remove_spell_from_stack(&mut engine, spell_id);

    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("an absent captured target does not skip the Ward offer")
            .presentation
            .choice_kind,
        ChoiceKind::ManaPayment
    );
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ward while its captured target is absent");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ward_mana_payment_remains_available_when_the_bound_stack_object_is_absent() {
    let (mut engine, _, spell_id) = cast_unsummon_at_dirgur(103_079);
    remove_spell_from_stack(&mut engine, spell_id);
    pass_both_players(&mut engine);

    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("an absent captured target does not skip the Ward offer")
            .presentation
            .choice_kind,
        ChoiceKind::ManaPayment
    );
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay Ward while its captured target is absent");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ward_discard_payment_remains_available_when_the_bound_stack_object_is_absent() {
    let (mut engine, _, spell_id, discard) = cast_unsummon_at_spectral_snatcher(103_080);
    remove_spell_from_stack(&mut engine, spell_id);
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("an absent captured target does not skip the discard Ward offer");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert!(pending.presentation.candidates.contains(&discard));

    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discard]))
        .expect("pay Ward while its captured target is absent");
    assert_eq!(
        engine.state.objects[&discard].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ward_can_counter_an_exact_activated_ability_without_a_zone_move() {
    let decks = vec![
        deck_with("island", &["prodigal_sorcerer"]),
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        103_009,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let prodigal = relocate_to_battlefield(&mut engine, 0, "prodigal_sorcerer", false);
    let dirgur = relocate_to_battlefield(
        &mut engine,
        1,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    engine
        .apply_scenario_command(0, &activate_ability(prodigal, 0, target_object(dirgur)))
        .expect("activate Prodigal Sorcerer at Dirgur");
    assert_eq!(engine.state.stack.len(), 2);
    let ability_id = engine.state.stack[0].id;

    pass_both_players(&mut engine);
    let countered = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ward for activated ability");
    assert!(countered.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(event)) if event.object_id == ability_id
    )));
    assert!(!countered.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(event)) if event.object_id == ability_id
    )));
    assert!(!engine.state.stack.iter().any(|item| item.id == ability_id));
}

#[test]
fn each_ward_instance_creates_an_independent_orderable_trigger() {
    let decks = vec![
        deck_with("island", &["unsummon"]),
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        103_010,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let dirgur = relocate_to_battlefield(
        &mut engine,
        1,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    let ward = tricerules_cards::registry::global()
        .get("dirgur_island_dragon_skimming_strike")
        .expect("Dirgur definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(dirgur),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ward)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[unsummon];
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(dirgur)))
        .expect("target two Ward instances");

    let order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("controller orders simultaneous Ward triggers");
    assert_eq!(order.deciding_player, 1);
    assert_eq!(order.candidates.len(), 2);
    assert!(order.candidates.iter().all(|candidate| {
        candidate.trigger_context.targeting_stack_object
            == Some(tricerules_core::state::StackObjectRef {
                object_id: spell_id,
                zone_change_generation: Some(engine.state.zone_change_generation[&spell_id]),
            })
    }));
}

#[test]
fn ward_can_counter_the_exact_targeted_triggered_ability() {
    let decks = vec![
        deck_with("mountain", &["flametongue_kavu"]),
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        103_011,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let dirgur = relocate_to_battlefield(
        &mut engine,
        1,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    ensure_in_hand(&mut engine, 0, "flametongue_kavu");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );
    let kavu = hand_index_for_card(&engine, 0, "flametongue_kavu");
    engine
        .apply_scenario_command(0, &cast_spell(kavu, Vec::new()))
        .expect("cast Flametongue Kavu");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(dirgur),
                })),
            },
        )
        .expect("target Dirgur with the ETB trigger");
    assert_eq!(engine.state.stack.len(), 2);
    let triggered_ability_id = engine.state.stack[0].id;

    pass_both_players(&mut engine);
    let countered = engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ward for triggered ability");
    assert!(countered.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(event)) if event.object_id == triggered_ability_id
    )));
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == triggered_ability_id));
}

#[test]
fn ward_can_counter_a_targeting_spell_copy_without_moving_a_card() {
    let decks = vec![
        deck_with(
            "mountain",
            &["lightning_bolt", "dirgur_island_dragon_skimming_strike"],
        ),
        deck_with("island", &["twincast"]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        103_012,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let dirgur = relocate_to_battlefield(
        &mut engine,
        0,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_scenario_command(0, &cast_spell(bolt, target_player(1)))
        .expect("cast Lightning Bolt");
    let bolt_id = engine.state.stack[0].id;
    engine
        .apply_scenario_command(0, &pass())
        .expect("pass priority to the Twincast player");

    ensure_in_hand(&mut engine, 1, "twincast");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast = hand_index_for_card(&engine, 1, "twincast");
    engine
        .apply_scenario_command(1, &cast_spell(twincast, target_object(bolt_id)))
        .expect("cast Twincast");
    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![dirgur]))
        .expect("retarget the copy to Dirgur");
    let copy_id = engine
        .state
        .stack
        .iter()
        .find(|item| item.is_copy)
        .expect("spell copy")
        .id;
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .expect("Ward trigger")
            .ability_text
            .as_deref(),
        Some("Dirgur Island Dragon — triggered ability (triggered_01)")
    );

    pass_both_players(&mut engine);
    let countered = engine
        .apply_scenario_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ward for spell copy");
    assert!(countered.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(event)) if event.object_id == copy_id
    )));
    assert!(!countered.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(event)) if event.object_id == copy_id
    )));
}

#[test]
fn mana_abilities_remain_available_during_a_ward_payment() {
    let decks = vec![
        deck_with("island", &["unsummon", "island", "island"]),
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        103_013,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let first_island = relocate_to_battlefield(&mut engine, 0, "island", false);
    let second_island = relocate_to_battlefield(&mut engine, 0, "island", false);
    let dirgur = relocate_to_battlefield(
        &mut engine,
        1,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[unsummon];
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(dirgur)))
        .expect("cast Unsummon");
    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_some());

    let original_offer_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward payment prompt")
        .resolution_prompt_id
        .get();
    assert_ne!(original_offer_id, 0);

    let unchanged_refresh = engine
        .apply_scenario_command(0, &activate_ability(first_island, 0, Vec::new()))
        .expect("first mana ability during Ward");
    let unchanged_offer = find_resolution_choice(&unchanged_refresh)
        .expect("the same Ward payment offer is republished");
    assert_eq!(unchanged_offer.resolution_prompt_id, original_offer_id);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get(),
        original_offer_id,
        "an answer-semantics-preserving refresh retains its prompt ID"
    );

    let changed_refresh = engine
        .apply_scenario_command(0, &activate_ability(second_island, 0, Vec::new()))
        .expect("second mana ability during Ward");
    let changed_offer = find_resolution_choice(&changed_refresh)
        .expect("the same Ward payment offer is republished with refreshed readiness");
    let current_offer_id = changed_offer.resolution_prompt_id;
    assert_eq!(current_offer_id, original_offer_id);
    assert!(changed_offer.payment_currently_legal);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get(),
        current_offer_id
    );

    submit_mana_resolution(
        &mut engine,
        0,
        SubmitResolutionChoice {
            resolution_prompt_id: current_offer_id,
            decision: ResolutionChoiceDecision::PayMana as i32,
            ..Default::default()
        },
    )
    .expect("pay Ward with mana produced during the prompt");
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));

    let mut stale_answer = submit_resolution_decision(ResolutionChoiceDecision::Decline);
    let Some(Cmd::SubmitResolutionChoice(answer)) = stale_answer.cmd.as_mut() else {
        unreachable!()
    };
    answer.resolution_prompt_id = original_offer_id;
    let unchanged_state = format!("{:?}", engine.state);
    assert!(engine.apply_command(0, &stale_answer).is_err());
    assert_eq!(
        format!("{:?}", engine.state),
        unchanged_state,
        "the completed Ward payment ID cannot mutate the resumed resolution"
    );
}

#[test]
fn declining_ward_rewinds_mana_abilities_activated_during_payment() {
    let decks = vec![
        deck_with("island", &["unsummon", "island", "island"]),
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
    ];
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        103_017,
        &[0, 1],
        20,
        Some(decks),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let first_island = relocate_to_battlefield(&mut engine, 0, "island", false);
    let second_island = relocate_to_battlefield(&mut engine, 0, "island", false);
    let dirgur = relocate_to_battlefield(
        &mut engine,
        1,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    ensure_in_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_scenario_command(0, &cast_spell(unsummon, target_object(dirgur)))
        .expect("cast Unsummon");
    pass_both_players(&mut engine);

    engine
        .apply_scenario_command(0, &activate_ability(first_island, 0, Vec::new()))
        .expect("first mana ability during Ward");
    engine
        .apply_scenario_command(0, &activate_ability(second_island, 0, Vec::new()))
        .expect("second mana ability during Ward");
    assert_eq!(engine.state.players[0].mana_pool.blue, 2);
    assert!(engine.state.objects[&first_island].tapped);
    assert!(engine.state.objects[&second_island].tapped);

    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline after partially staging Ward mana");
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert!(!engine.state.objects[&first_island].tapped);
    assert!(!engine.state.objects[&second_island].tapped);
    assert!(engine.state.undoable_mana_abilities.is_empty());
}

#[test]
fn ward_discard_counters_automatically_when_the_payer_has_no_card() {
    let (mut engine, _, spell_id, _) = cast_unsummon_at_spectral_snatcher(103_014);
    for card in std::mem::take(&mut engine.state.players[0].hand) {
        engine.state.players[0].graveyard.push(card);
        engine.state.objects.get_mut(&card).expect("hand card").zone =
            tricerules_core::Zone::Graveyard;
        *engine.state.zone_change_generation.entry(card).or_default() += 1;
    }

    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine.state.stack.iter().any(|item| item.id == spell_id));
    assert_eq!(
        engine.state.objects[&spell_id].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn ward_payment_replays_deterministically_for_the_same_seed_and_commands() {
    fn replay() -> (Vec<u32>, Vec<u32>, usize, u64) {
        let (mut engine, _, spell_id, discard) = cast_unsummon_at_spectral_snatcher(103_015);
        pass_both_players(&mut engine);
        engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![discard]))
            .expect("pay Ward by discarding");
        pass_both_players(&mut engine);
        (
            engine.state.players[0].graveyard.clone(),
            engine.state.players[1].hand.clone(),
            engine.state.stack.len(),
            engine.state.command_index + u64::from(spell_id),
        )
    }

    assert_eq!(replay(), replay());
}
