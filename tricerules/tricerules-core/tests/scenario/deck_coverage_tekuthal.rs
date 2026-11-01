//! Actual-card Tekuthal scenarios against the global card registry.
//!
//! `tekuthal_runtime.rs` remains the separate capability-only fixture suite. These cases bind the
//! approved combat, payment, proliferate-origin, and Ripples behavior to shipped card definitions.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::Keyword;
use tricerules_core::{AttachmentRecipient, Zone};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::ruled_event;

const TEKUTHAL_ID: &str = "tekuthal,_inquiry_dominus";
const TEKUTHAL_NAME: &str = "Tekuthal, Inquiry Dominus";

fn assert_global_card(card_id: &str, oracle_name: &str) {
    let definition = tricerules_cards::registry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} is in the global card registry"));
    assert_eq!(
        definition.name, oracle_name,
        "actual Oracle name for {card_id}"
    );
}

fn actual_engine(seed: u64, player_ids: &[i32]) -> GameEngine {
    assert_global_card(TEKUTHAL_ID, TEKUTHAL_NAME);
    let deck = deck_with("forest", &[]);
    let decks = player_ids.iter().map(|_| deck.clone()).collect();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        player_ids,
        20,
        Some(decks),
        true,
    )
    .expect("actual-card scenario engine uses the global card registry");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn assert_actual_tekuthal(engine: &GameEngine, object_id: u32) {
    assert_eq!(
        engine.state.objects[&object_id].card_id, TEKUTHAL_ID,
        "the scenario source is the actual registered Tekuthal definition"
    );
    assert_eq!(
        tricerules_cards::registry::global()
            .get(&engine.state.objects[&object_id].card_id)
            .expect("actual Tekuthal remains globally registered")
            .name,
        TEKUTHAL_NAME
    );
}

fn inject_actual_tekuthal(engine: &mut GameEngine, player_index: usize) -> u32 {
    let tekuthal = inject_creature_on_battlefield(engine, player_index, TEKUTHAL_ID);
    assert_actual_tekuthal(engine, tekuthal);
    tekuthal
}

fn public_permanent(
    batch: &RuledEventBatch,
    player_id: i32,
    object_id: u32,
) -> rv1::BattlefieldObject {
    let zone_view = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::ZoneView(view) => Some(view),
            _ => None,
        })
        .expect("engine response publishes the engine-owned battlefield view");
    zone_view
        .per_player
        .iter()
        .find(|view| view.player_id == player_id)
        .expect("the acting player receives a battlefield view")
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == object_id)
        .expect("the requested actual permanent appears in the public battlefield offer")
        .clone()
}

fn add_counter(engine: &mut GameEngine, object_id: u32, kind: CounterKind, count: u32) {
    let timestamp = engine.state.command_index;
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("counter recipient exists")
        .add_counters(kind, count, timestamp);
}

fn put_poison(engine: &mut GameEngine, player_id: i32, count: u32) {
    let player_index = engine
        .state
        .player_idx(player_id)
        .expect("scenario player exists");
    engine.state.players[player_index]
        .counters
        .insert(CounterKind::Poison, count);
}

fn counter_count(engine: &GameEngine, object_id: u32, kind: CounterKind) -> u32 {
    engine.state.objects[&object_id].counter_count(kind)
}

fn proliferate_offer(batch: &RuledEventBatch) -> rv1::ResolutionChoiceRequired {
    let offer = find_resolution_choice(batch).expect("the current Proliferate choice is published");
    assert_eq!(offer.choice_kind, rv1::ChoiceKind::Proliferate as i32);
    assert_ne!(offer.resolution_prompt_id, 0);
    offer
}

fn submit_current_choice(
    offer: &rv1::ResolutionChoiceRequired,
    object_ids: Vec<u32>,
    player_ids: Vec<i32>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: offer.resolution_prompt_id,
            chosen_object_ids: object_ids,
            chosen_player_ids: player_ids,
            decision: ResolutionChoiceDecision::Unspecified as i32,
            ..Default::default()
        })),
    }
}

fn replacement_offer(batch: &RuledEventBatch) -> rv1::ResolutionChoiceRequired {
    batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::ResolutionChoiceRequired(choice)
                if choice.choice_kind == rv1::ChoiceKind::ReplacementEffect as i32 =>
            {
                Some(choice.clone())
            }
            _ => None,
        })
        .expect("the current nested replacement ordering is published")
}

fn proliferated_log_count(batch: &RuledEventBatch, player_id: i32) -> usize {
    let expected = format!("P{player_id} proliferates.");
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .filter(|text| *text == expected)
        .count()
}

fn stack_resolved_count(batch: &RuledEventBatch, object_id: u32) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.ev.as_ref(),
                Some(ruled_event::Ev::StackResolved(resolved)) if resolved.object_id == object_id
            )
        })
        .count()
}

fn log_count(batch: &RuledEventBatch, expected: &str) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.ev.as_ref(),
                Some(ruled_event::Ev::Log(log)) if log.text == expected
            )
        })
        .count()
}

fn contains_choice_kind(batch: &RuledEventBatch, kind: rv1::ChoiceKind) -> bool {
    batch.events.iter().any(|event| {
        matches!(
            event.ev.as_ref(),
            Some(ruled_event::Ev::ResolutionChoiceRequired(choice))
                if choice.choice_kind == kind as i32
        )
    })
}

#[test]
fn actual_tekuthal_flying_combat_rejects_ground_block_atomically() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        91_001,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("actual-card flying combat engine");
    advance_to_declare_attackers(&mut engine);

    let tekuthal = inject_actual_tekuthal(&mut engine, 0);
    let ground_blocker = inject_creature_on_battlefield(&mut engine, 1, "coral_merfolk");
    let legal_flying_blocker = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    assert_global_card("coral_merfolk", "Coral Merfolk");
    assert_global_card("storm_crow", "Storm Crow");

    engine
        .apply_scenario_command(0, &declare_attackers(vec![tekuthal]))
        .expect("declare actual Tekuthal as the attacker");
    engine
        .apply_scenario_command(0, &pass())
        .expect("active player passes the declare-attackers step");
    engine
        .apply_scenario_command(1, &pass())
        .expect("defending player passes the declare-attackers step");
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );

    let before_state = format!("{:?}", engine.state);
    let before_diagnostics = engine
        .diagnostic_snapshot()
        .expect("capture state before the illegal ground block");
    let before_command_index = engine.state.command_index;
    let rejected = engine.apply_scenario_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: tekuthal,
            blocker_id: ground_blocker,
        }]),
    );
    assert!(
        matches!(rejected, Err(tricerules_core::EngineError::Illegal(_))),
        "a ground-only Coral Merfolk cannot block actual flying Tekuthal"
    );
    assert_eq!(format!("{:?}", engine.state), before_state);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before_diagnostics,
        "the rejected declaration changes no combat state"
    );
    assert_eq!(engine.state.command_index, before_command_index);

    engine
        .apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: tekuthal,
                blocker_id: legal_flying_blocker,
            }]),
        )
        .expect("the other actual flying creature is a legal blocker");
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
}

#[derive(Clone, Copy, Debug)]
enum CounterLayout {
    ThreeChargeOnOnePermanent,
    ChargeAndNightOnOnePermanent,
    OneEachAcrossThreePermanents,
    TwoChargeAndOneNightAcrossTwoPermanents,
}

#[derive(Clone, Copy)]
struct CounterSource {
    object_id: u32,
    kind: CounterKind,
    label: &'static str,
    count: u32,
}

#[derive(Clone, Copy)]
struct PhiPayment {
    label: &'static str,
    pay_life: [bool; 2],
    generic_mana: u32,
    blue_mana: u32,
    life_paid: u32,
}

const PHI_PAYMENTS: [PhiPayment; 5] = [
    PhiPayment {
        label: "blue then blue",
        pay_life: [false, false],
        generic_mana: 1,
        blue_mana: 2,
        life_paid: 0,
    },
    PhiPayment {
        label: "life at pip 1, blue at pip 2",
        pay_life: [true, false],
        generic_mana: 1,
        blue_mana: 1,
        life_paid: 2,
    },
    PhiPayment {
        label: "blue at pip 1, life at pip 2",
        pay_life: [false, true],
        generic_mana: 1,
        blue_mana: 1,
        life_paid: 2,
    },
    PhiPayment {
        label: "life at both pips",
        pay_life: [true, true],
        generic_mana: 1,
        blue_mana: 0,
        life_paid: 4,
    },
    PhiPayment {
        label: "generic one paid separately from both blue pips",
        pay_life: [false, false],
        generic_mana: 0,
        blue_mana: 3,
        life_paid: 0,
    },
];

fn flex_payments(payment: PhiPayment) -> Vec<rv1::FlexPipPayment> {
    vec![
        rv1::FlexPipPayment {
            pip_index: 1,
            pay_life: payment.pay_life[0],
        },
        rv1::FlexPipPayment {
            pip_index: 2,
            pay_life: payment.pay_life[1],
        },
    ]
}

#[derive(Clone)]
struct TekActivationOffer {
    source: rv1::CostObjectRef,
    ability_index: u32,
    cost_index: u32,
    group: rv1::CounterRemovalGroupChoices,
}

fn current_tek_activation_offer(
    engine: &mut GameEngine,
    actor: i32,
    source_id: u32,
) -> TekActivationOffer {
    let initial = engine.initial_response_batch();
    let source_view = public_permanent(&initial, actor, source_id);
    assert_actual_tekuthal(engine, source_id);
    assert_eq!(source_view.activated_abilities.len(), 1);
    let ability = &source_view.activated_abilities[0];
    assert!(
        ability.activatable,
        "the actual activated ability is offered"
    );
    let source = rv1::CostObjectRef {
        object_id: source_view.object_id,
        zone_change_generation: source_view.zone_change_generation,
    };
    let ability_key = (u64::from(source.object_id) << 32) | u64::from(ability.ability_index);
    let legal_costs = initial.legal_by_player[&actor]
        .cost_choices_by_ability
        .get(&ability_key)
        .expect("the public current ability offer includes its legal counter-removal group");
    assert!(legal_costs.non_mana_costs_payable);
    let choice = legal_costs
        .choices
        .iter()
        .find(|choice| choice.counter_removal_group.is_some())
        .expect("the actual cost exposes its counter-removal group");
    let group = choice
        .counter_removal_group
        .clone()
        .expect("the selected cost choice is the aggregate group");
    assert_eq!(group.required_count, 3);
    TekActivationOffer {
        source,
        ability_index: ability.ability_index,
        cost_index: choice.cost_index,
        group,
    }
}

fn offered_contribution(
    group: &rv1::CounterRemovalGroupChoices,
    source: CounterSource,
) -> rv1::CounterRemovalContribution {
    let candidate = group
        .candidates
        .iter()
        .find(|candidate| {
            candidate
                .object
                .as_ref()
                .is_some_and(|object| object.object_id == source.object_id)
        })
        .expect("the actual counter-bearing permanent is in the current public offer");
    let object = candidate
        .object
        .expect("the public counter candidate includes its generation-bound object receipt");
    let option = candidate
        .options
        .iter()
        .find(|option| option.label == source.label && option.available_count >= source.count)
        .expect("the current counter option supports the requested amount");
    rv1::CounterRemovalContribution {
        object: Some(object),
        option_id: option.option_id,
        count: source.count,
    }
}

fn begin_tek_activation(
    offer: &TekActivationOffer,
    contributions: Vec<rv1::CounterRemovalContribution>,
    flex: Vec<rv1::FlexPipPayment>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::BeginAbilityActivation(rv1::BeginAbilityActivation {
            source_object_id: offer.source.object_id,
            expected_zone_change_generation: offer.source.zone_change_generation,
            ability_index: offer.ability_index,
            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
            cost_selections: vec![rv1::CostSelection {
                cost_index: offer.cost_index,
                selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
                    rv1::CounterRemovalGroupSelection { contributions },
                )),
            }],
            flex_payments: flex,
            ..Default::default()
        })),
    }
}

fn commit_tek_activation(
    engine: &GameEngine,
    pending: &rv1::PendingAbilityActivation,
    source: &rv1::CostObjectRef,
    mana: rv1::PaymentMana,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CommitAbilityActivation(rv1::CommitAbilityActivation {
            transaction_id: pending.transaction_id,
            expected_revision: pending.revision,
            payment: Some(rv1::PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(*source),
                mana: Some(mana),
                ..Default::default()
            }),
            ..Default::default()
        })),
    }
}

fn counter_sources(
    engine: &mut GameEngine,
    actor_index: usize,
    layout: CounterLayout,
) -> Vec<CounterSource> {
    match layout {
        CounterLayout::ThreeChargeOnOnePermanent => {
            let ring = inject_permanent_on_battlefield(engine, actor_index, "replicating_ring");
            add_counter(engine, ring, CounterKind::Charge, 3);
            vec![CounterSource {
                object_id: ring,
                kind: CounterKind::Charge,
                label: "charge",
                count: 3,
            }]
        }
        CounterLayout::ChargeAndNightOnOnePermanent => {
            let ring = inject_permanent_on_battlefield(engine, actor_index, "replicating_ring");
            add_counter(engine, ring, CounterKind::Charge, 2);
            add_counter(engine, ring, CounterKind::Night, 1);
            vec![
                CounterSource {
                    object_id: ring,
                    kind: CounterKind::Charge,
                    label: "charge",
                    count: 2,
                },
                CounterSource {
                    object_id: ring,
                    kind: CounterKind::Night,
                    label: "night",
                    count: 1,
                },
            ]
        }
        CounterLayout::OneEachAcrossThreePermanents => {
            let ring = inject_permanent_on_battlefield(engine, actor_index, "replicating_ring");
            let chalice =
                inject_permanent_on_battlefield(engine, actor_index, "everflowing_chalice");
            let bears = inject_creature_on_battlefield(engine, actor_index, "grizzly_bears");
            add_counter(engine, ring, CounterKind::Charge, 1);
            add_counter(engine, chalice, CounterKind::Charge, 1);
            add_counter(engine, bears, CounterKind::Night, 1);
            vec![
                CounterSource {
                    object_id: ring,
                    kind: CounterKind::Charge,
                    label: "charge",
                    count: 1,
                },
                CounterSource {
                    object_id: chalice,
                    kind: CounterKind::Charge,
                    label: "charge",
                    count: 1,
                },
                CounterSource {
                    object_id: bears,
                    kind: CounterKind::Night,
                    label: "night",
                    count: 1,
                },
            ]
        }
        CounterLayout::TwoChargeAndOneNightAcrossTwoPermanents => {
            let chalice =
                inject_permanent_on_battlefield(engine, actor_index, "everflowing_chalice");
            let bears = inject_creature_on_battlefield(engine, actor_index, "grizzly_bears");
            add_counter(engine, chalice, CounterKind::Charge, 2);
            add_counter(engine, bears, CounterKind::Night, 1);
            vec![
                CounterSource {
                    object_id: chalice,
                    kind: CounterKind::Charge,
                    label: "charge",
                    count: 2,
                },
                CounterSource {
                    object_id: bears,
                    kind: CounterKind::Night,
                    label: "night",
                    count: 1,
                },
            ]
        }
    }
}

fn accepted_payment_case(seed: u64, layout: CounterLayout, payment: PhiPayment) {
    let mut engine = actual_engine(seed, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut engine, 0);
    let sources = counter_sources(&mut engine, 0, layout);
    let actor_index = engine.state.player_idx(0).unwrap();
    engine.state.players[actor_index].mana_pool.colorless = payment.generic_mana;
    engine.state.players[actor_index].mana_pool.blue = payment.blue_mana;

    let offer = current_tek_activation_offer(&mut engine, 0, tekuthal);
    let contributions = sources
        .iter()
        .map(|source| offered_contribution(&offer.group, *source))
        .collect();
    let begin = begin_tek_activation(&offer, contributions, flex_payments(payment));
    engine
        .apply_scenario_command(0, &begin)
        .unwrap_or_else(|error| panic!("{} / {layout:?}: {error}", payment.label));
    let pending = engine
        .state
        .pending_ability_activation
        .clone()
        .expect("the accepted full indexed Phyrexian payment opens one pending transaction");
    assert_eq!(pending.source_object_id, offer.source.object_id);
    assert_eq!(pending.ability_index, offer.ability_index);
    assert_eq!(pending.flex_payments, flex_payments(payment));
    assert_eq!(pending.cost_selections.len(), 1);

    let commit = commit_tek_activation(
        &engine,
        &pending,
        &offer.source,
        rv1::PaymentMana {
            c: payment.generic_mana,
            u: payment.blue_mana,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &commit)
        .unwrap_or_else(|error| panic!("commit {} / {layout:?}: {error}", payment.label));

    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(
        engine.state.stack.len(),
        1,
        "one actual ability is on the stack"
    );
    assert_eq!(
        engine.state.players[actor_index].life,
        20 - payment.life_paid as i32
    );
    assert_eq!(engine.state.players[actor_index].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[actor_index].mana_pool.blue, 0);
    for source in sources {
        assert_eq!(
            counter_count(&engine, source.object_id, source.kind),
            0,
            "{} counters are paid exactly once ({layout:?}, {})",
            source.label,
            payment.label
        );
    }
    assert_eq!(
        counter_count(&engine, tekuthal, CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn actual_tekuthal_phyrexian_payments_use_current_offers_and_counter_distributions() {
    let mut seed = 91_100;
    for layout in [
        CounterLayout::ThreeChargeOnOnePermanent,
        CounterLayout::ChargeAndNightOnOnePermanent,
        CounterLayout::OneEachAcrossThreePermanents,
        CounterLayout::TwoChargeAndOneNightAcrossTwoPermanents,
    ] {
        for payment in PHI_PAYMENTS {
            accepted_payment_case(seed, layout, payment);
            seed += 1;
        }
    }
}

fn assert_rejected_atomically(
    engine: &mut GameEngine,
    actor: i32,
    command: &RuledCommand,
    label: &str,
) {
    let before_state = format!("{:?}", engine.state);
    let before_diagnostics = engine
        .diagnostic_snapshot()
        .expect("capture diagnostics before direct invalid command");
    let before_command_index = engine.state.command_index;
    let rejected = engine.apply_scenario_command(actor, command);
    assert!(
        matches!(rejected, Err(tricerules_core::EngineError::Illegal(_))),
        "{label} must reach the production validator and be rejected"
    );
    assert_eq!(format!("{:?}", engine.state), before_state, "{label}");
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        before_diagnostics,
        "{label} preserves the full diagnostic state"
    );
    assert_eq!(engine.state.command_index, before_command_index, "{label}");
}

#[test]
fn actual_tekuthal_phyrexian_rejections_preserve_pending_costs_and_command_index() {
    let mut insufficient_life = actual_engine(91_300, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut insufficient_life, 0);
    let sources = counter_sources(
        &mut insufficient_life,
        0,
        CounterLayout::ThreeChargeOnOnePermanent,
    );
    let actor_index = insufficient_life.state.player_idx(0).unwrap();
    insufficient_life.state.players[actor_index].life = 1;
    let offer = current_tek_activation_offer(&mut insufficient_life, 0, tekuthal);
    let contributions = sources
        .iter()
        .map(|source| offered_contribution(&offer.group, *source))
        .collect();
    let life_begin = begin_tek_activation(&offer, contributions, flex_payments(PHI_PAYMENTS[3]));
    assert_rejected_atomically(
        &mut insufficient_life,
        0,
        &life_begin,
        "insufficient life to pay both currently chosen Phyrexian pips",
    );
    assert!(insufficient_life.state.pending_ability_activation.is_none());

    let mut insufficient_mana = actual_engine(91_301, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut insufficient_mana, 0);
    let sources = counter_sources(
        &mut insufficient_mana,
        0,
        CounterLayout::ThreeChargeOnOnePermanent,
    );
    let offer = current_tek_activation_offer(&mut insufficient_mana, 0, tekuthal);
    let contributions = sources
        .iter()
        .map(|source| offered_contribution(&offer.group, *source))
        .collect();
    let begin = begin_tek_activation(&offer, contributions, flex_payments(PHI_PAYMENTS[3]));
    insufficient_mana
        .apply_scenario_command(0, &begin)
        .expect("accept current counter receipts and both life choices before mana commit");
    let pending = insufficient_mana
        .state
        .pending_ability_activation
        .clone()
        .expect("Begin locks one actual payment transaction");
    let pending_before = pending.clone();
    let commit = commit_tek_activation(
        &insufficient_mana,
        &pending,
        &offer.source,
        rv1::PaymentMana::default(),
    );
    assert_rejected_atomically(
        &mut insufficient_mana,
        0,
        &commit,
        "insufficient generic mana at direct Commit",
    );
    assert_eq!(
        insufficient_mana.state.pending_ability_activation.as_ref(),
        Some(&pending_before),
        "an underfunded Commit preserves the accepted choices and locked costs"
    );
    assert_eq!(insufficient_mana.state.players[0].life, 20);
    assert_eq!(
        counter_count(&insufficient_mana, sources[0].object_id, sources[0].kind),
        3
    );

    let mut malformed = actual_engine(91_302, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut malformed, 0);
    let sources = counter_sources(&mut malformed, 0, CounterLayout::ThreeChargeOnOnePermanent);
    let offer = current_tek_activation_offer(&mut malformed, 0, tekuthal);
    let mut undercounted = offered_contribution(&offer.group, sources[0]);
    undercounted.count = 2;
    let malformed_begin =
        begin_tek_activation(&offer, vec![undercounted], flex_payments(PHI_PAYMENTS[0]));
    assert_rejected_atomically(
        &mut malformed,
        0,
        &malformed_begin,
        "malformed aggregate contribution count",
    );
    assert!(malformed.state.pending_ability_activation.is_none());

    let mut stale = offered_contribution(&offer.group, sources[0]);
    stale
        .object
        .as_mut()
        .expect("current offered receipt exists")
        .zone_change_generation += 1;
    let stale_begin = begin_tek_activation(&offer, vec![stale], flex_payments(PHI_PAYMENTS[0]));
    assert_rejected_atomically(
        &mut malformed,
        0,
        &stale_begin,
        "stale generation on an otherwise current contribution receipt",
    );
}

#[test]
fn actual_tekuthal_karns_bastion_and_gambit_each_offer_two_fresh_proliferates() {
    assert_global_card("karns_bastion", "Karn's Bastion");
    let mut bastion_engine = actual_engine(91_400, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut bastion_engine, 0);
    let bastion = inject_permanent_on_battlefield(&mut bastion_engine, 0, "karns_bastion");
    put_poison(&mut bastion_engine, 0, 1);
    grant_pool(&mut bastion_engine, 0);
    let before = bastion_engine.initial_response_batch();
    let bastion_offer = public_permanent(&before, 0, bastion);
    let bastion_definition = tricerules_cards::registry::global()
        .get("karns_bastion")
        .expect("actual Karn's Bastion is globally registered");
    let definition_abilities = &bastion_definition.primary_face().activated_abilities;
    let mana_index = definition_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == "activated_01")
        .expect("actual activated_01 is Karn's Bastion mana ability");
    let proliferate_index = definition_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == "activated_02")
        .expect("actual activated_02 is Karn's Bastion Proliferate ability");
    assert_ne!(mana_index, proliferate_index);
    assert_eq!(
        bastion_offer.activated_abilities.len(),
        definition_abilities.len(),
        "the public offer preserves both actual Karn's Bastion abilities"
    );
    let mana_offer = bastion_offer
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_index as usize == mana_index)
        .expect("actual mana slot appears in the current public offer");
    assert!(mana_offer.is_mana_ability);
    let bastion_proliferate_ability = bastion_offer
        .activated_abilities
        .iter()
        .find(|ability| ability.ability_index as usize == proliferate_index)
        .expect("actual Proliferate slot appears in the current public offer");
    assert!(bastion_proliferate_ability.activatable);
    assert!(!bastion_proliferate_ability.is_mana_ability);
    assert_eq!(
        bastion_offer.zone_change_generation,
        *bastion_engine
            .state
            .zone_change_generation
            .get(&bastion)
            .unwrap_or(&0)
    );
    apply_ability(
        &mut bastion_engine,
        0,
        bastion_offer.object_id,
        bastion_proliferate_ability.ability_index,
        vec![],
    )
    .expect("activate actual Karn's Bastion Proliferate using its current public ability index");
    let bastion_stack_item = bastion_engine.state.stack.last().unwrap().id;
    assert!(bastion_engine.state.objects[&bastion].tapped);
    let mut batch = pass_both_players_with_batch(&mut bastion_engine);
    let mut bastion_resolutions = 0;
    for leaf in 0..2 {
        let offer = proliferate_offer(&batch);
        assert_eq!(offer.candidate_player_ids, vec![0]);
        batch = bastion_engine
            .apply_scenario_command(0, &submit_current_choice(&offer, vec![], vec![0]))
            .expect("choose the current player receipt for actual Bastion Proliferate");
        assert_eq!(proliferated_log_count(&batch, 0), 1);
        bastion_resolutions += stack_resolved_count(&batch, bastion_stack_item);
        assert_eq!(
            counter_count(&bastion_engine, tekuthal, CounterKind::PlusOnePlusOne),
            0
        );
        if leaf == 0 {
            assert_eq!(
                bastion_engine.state.players[0].counters[&CounterKind::Poison],
                2
            );
            assert_eq!(proliferate_offer(&batch).candidate_player_ids, vec![0]);
        }
    }
    assert_eq!(
        bastion_engine.state.players[0].counters[&CounterKind::Poison],
        3
    );
    assert_eq!(
        bastion_resolutions, 1,
        "the actual Bastion activation resolves once"
    );
    assert!(bastion_engine.state.stack.is_empty());
    assert!(bastion_engine.state.pending_resolution.is_none());

    assert_global_card("tezzerets_gambit", "Tezzeret's Gambit");
    let mut gambit_engine = actual_engine(91_401, &[0, 1]);
    inject_actual_tekuthal(&mut gambit_engine, 0);
    put_poison(&mut gambit_engine, 0, 1);
    let gambit = inject_card_into_hand(&mut gambit_engine, 0, "tezzerets_gambit");
    grant_pool(&mut gambit_engine, 0);
    let hand_after_cast = {
        let slot = hand_index_for_card(&gambit_engine, 0, "tezzerets_gambit");
        gambit_engine
            .apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast actual Tezzeret's Gambit");
        gambit_engine.state.players[0].hand.len()
    };
    let mut batch = pass_both_players_with_batch(&mut gambit_engine);
    assert_eq!(
        gambit_engine.state.players[0].hand.len(),
        hand_after_cast + 2
    );
    let gambit_stack_id = gambit;
    let mut gambit_resolutions = 0;
    for leaf in 0..2 {
        let offer = proliferate_offer(&batch);
        assert_eq!(offer.candidate_player_ids, vec![0]);
        batch = gambit_engine
            .apply_scenario_command(0, &submit_current_choice(&offer, vec![], vec![0]))
            .expect("choose the current player receipt for actual Gambit Proliferate");
        assert_eq!(proliferated_log_count(&batch, 0), 1);
        gambit_resolutions += stack_resolved_count(&batch, gambit_stack_id);
        assert_eq!(
            gambit_engine.state.players[0].hand.len(),
            hand_after_cast + 2
        );
        if leaf == 0 {
            assert_eq!(
                gambit_engine.state.players[0].counters[&CounterKind::Poison],
                2
            );
            assert_eq!(proliferate_offer(&batch).candidate_player_ids, vec![0]);
        }
    }
    assert_eq!(
        gambit_engine.state.players[0].counters[&CounterKind::Poison],
        3
    );
    assert_eq!(gambit_resolutions, 1, "Gambit's actual spell resolves once");
    assert_eq!(
        gambit_engine.state.players[0].hand.len(),
        hand_after_cast + 2
    );
    assert_eq!(gambit_engine.state.objects[&gambit].zone, Zone::Graveyard);
    assert!(gambit_engine.state.stack.is_empty());
    assert!(gambit_engine.state.pending_resolution.is_none());
}

#[test]
fn actual_tekuthal_ezuri_pays_once_and_delays_four_draws_until_parent_finishes() {
    assert_global_card("ezuri,_stalker_of_spheres", "Ezuri, Stalker of Spheres");
    let mut engine = actual_engine(91_500, &[0, 1]);
    inject_actual_tekuthal(&mut engine, 0);
    put_poison(&mut engine, 1, 1);
    grant_pool(&mut engine, 0);

    let ezuri = inject_card_into_hand(&mut engine, 0, "ezuri,_stalker_of_spheres");
    let ezuri_slot = hand_index_for_card(&engine, 0, "ezuri,_stalker_of_spheres");
    engine
        .apply_scenario_command(0, &cast_spell(ezuri_slot, vec![]))
        .expect("cast the actual Ezuri card");
    pass_both_players_with_batch(&mut engine);
    assert_eq!(engine.state.objects[&ezuri].zone, Zone::Battlefield);
    let etb_item_id = engine
        .state
        .stack
        .last()
        .expect("actual Ezuri ETB ability is on the stack")
        .id;
    assert_eq!(
        engine.state.stack.last().unwrap().source_permanent_id,
        Some(ezuri)
    );
    assert_eq!(engine.state.stack.last().unwrap().ability_index, Some(0));

    let etb_batch = pass_both_players_with_batch(&mut engine);
    let mut parent_resolutions = stack_resolved_count(&etb_batch, etb_item_id);
    let branch = find_resolution_choice(&etb_batch).expect("Ezuri offers its optional ETB branch");
    assert_eq!(branch.choice_kind, rv1::ChoiceKind::ResolutionBranch as i32);
    assert_eq!(branch.deciding_player_id, 0);
    assert_ne!(branch.resolution_prompt_id, 0);
    let branch_batch = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: branch.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: 0,
                    ..Default::default()
                })),
            },
        )
        .expect("select the current optional branch of the actual ETB");
    parent_resolutions += stack_resolved_count(&branch_batch, etb_item_id);
    let payment = find_resolution_choice(&branch_batch).expect("Ezuri offers one mana payment");
    assert_eq!(payment.choice_kind, rv1::ChoiceKind::ManaPayment as i32);
    assert_ne!(payment.resolution_prompt_id, 0);
    let mana_before = engine.state.players[0].mana_pool.colorless
        + engine.state.players[0].mana_pool.white
        + engine.state.players[0].mana_pool.blue
        + engine.state.players[0].mana_pool.black
        + engine.state.players[0].mana_pool.red
        + engine.state.players[0].mana_pool.green;
    let payment_batch =
        submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
            .expect("pay the actual optional {3} cost once");
    parent_resolutions += stack_resolved_count(&payment_batch, etb_item_id);
    let mana_after = engine.state.players[0].mana_pool.colorless
        + engine.state.players[0].mana_pool.white
        + engine.state.players[0].mana_pool.blue
        + engine.state.players[0].mana_pool.black
        + engine.state.players[0].mana_pool.red
        + engine.state.players[0].mana_pool.green;
    assert_eq!(mana_before - mana_after, 3);
    let hand_before_draw_triggers = engine.state.players[0].hand.len();
    let mut batch = payment_batch;
    for leaf in 0..4 {
        let offer = proliferate_offer(&batch);
        assert_eq!(offer.deciding_player_id, 0);
        assert!(offer.candidate_object_ids.is_empty());
        assert_eq!(offer.candidate_player_ids, vec![1]);
        batch = engine
            .apply_scenario_command(0, &submit_current_choice(&offer, vec![], vec![1]))
            .expect("answer only the current actual Proliferate child");
        assert_eq!(proliferated_log_count(&batch, 0), 1);
        parent_resolutions += stack_resolved_count(&batch, etb_item_id);
        assert_eq!(
            engine.state.players[1].counters[&CounterKind::Poison],
            2 + leaf as u32
        );
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before_draw_triggers,
            "Ezuri's draw triggers wait while the ETB parent is parked"
        );
        if leaf < 3 {
            assert!(engine.state.pending_trigger_order.is_none());
            assert!(engine.state.stack.is_empty());
            assert_eq!(proliferate_offer(&batch).candidate_player_ids, vec![1]);
        }
    }
    assert_eq!(
        parent_resolutions, 1,
        "the actual Ezuri ETB parent completes once"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_draw_triggers
    );

    let trigger_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("four actual Ezuri draw triggers wait until all four child actions finish");
    assert_eq!(trigger_order.deciding_player, 0);
    assert_eq!(trigger_order.candidates.len(), 4);
    assert!(trigger_order.candidates.iter().all(|trigger| {
        trigger.source_permanent_id == ezuri
            && trigger.controller == 0
            && trigger.card_id == "ezuri,_stalker_of_spheres"
            && trigger.ability_index == 1
    }));
    assert!(answer_trigger_order_in_engine_order(&mut engine));
    assert_eq!(engine.state.stack.len(), 4);
    for expected_remaining in (1..=4).rev() {
        assert_eq!(engine.state.stack.len(), expected_remaining);
        pass_both_players_with_batch(&mut engine);
        assert_eq!(engine.state.stack.len(), expected_remaining - 1);
    }
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before_draw_triggers + 4,
        "the four actual triggered abilities each draw one card after the parent"
    );
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 5);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.pending_trigger_order.is_none());
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}

fn cast_actual_ripples(engine: &mut GameEngine, seed_card_id: &str) -> (u32, RuledEventBatch) {
    assert_global_card("ripples_of_potential", "Ripples of Potential");
    let spell = inject_card_into_hand(engine, 0, seed_card_id);
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, seed_card_id);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Ripples of Potential from hand");
    let batch = pass_both_players_with_batch(engine);
    assert_eq!(engine.state.objects[&spell].card_id, "ripples_of_potential");
    (spell, batch)
}

#[test]
fn actual_tekuthal_ripples_unions_children_after_nested_replacement_suspension() {
    assert_global_card("doubling_season", "Doubling Season");
    let mut engine = actual_engine(91_600, &[0, 1]);
    inject_actual_tekuthal(&mut engine, 0);
    let doublers = [
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season"),
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season"),
    ];
    let first = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let untouched = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    for permanent in [first, second, untouched] {
        add_counter(&mut engine, permanent, CounterKind::PlusOnePlusOne, 1);
    }
    put_poison(&mut engine, 1, 1);
    let (spell, mut batch) = cast_actual_ripples(&mut engine, "ripples_of_potential");
    let mut parent_resolutions = 0;

    let first_offer = proliferate_offer(&batch);
    assert!(first_offer.candidate_object_ids.contains(&first));
    assert!(first_offer.candidate_object_ids.contains(&second));
    assert!(first_offer.candidate_player_ids.contains(&1));
    batch = engine
        .apply_scenario_command(
            0,
            &submit_current_choice(&first_offer, vec![first], vec![1]),
        )
        .expect("select only permanent A and player 1 in the first current child");
    let first_nested = replacement_offer(&batch);
    assert_eq!(first_nested.replacement_options.len(), 2);
    assert_eq!(first_nested.deciding_player_id, 0);
    assert_ne!(first_nested.resolution_prompt_id, 0);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get(),
        first_nested.resolution_prompt_id
    );
    let mut first_doubler_ids = first_nested
        .replacement_options
        .iter()
        .map(|option| option.source_object_id)
        .collect::<Vec<_>>();
    first_doubler_ids.sort_unstable();
    let mut expected_doubler_ids = doublers.to_vec();
    expected_doubler_ids.sort_unstable();
    assert_eq!(first_doubler_ids, expected_doubler_ids);

    batch = engine
        .apply_scenario_command(
            0,
            &submit_current_choice(
                &first_nested,
                vec![first_nested.replacement_options[0].application_id],
                vec![],
            ),
        )
        .expect("resume the exact first nested replacement offer");
    assert_eq!(proliferated_log_count(&batch, 0), 1);
    assert_eq!(
        counter_count(&engine, first, CounterKind::PlusOnePlusOne),
        5
    );
    assert_eq!(
        counter_count(&engine, second, CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        counter_count(&engine, untouched, CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 2);
    assert_eq!(stack_resolved_count(&batch, spell), 0);
    assert_eq!(
        proliferate_offer(&batch).candidate_object_ids,
        vec![first, second, untouched]
    );
    assert!(
        !contains_choice_kind(&batch, rv1::ChoiceKind::PermanentObjects),
        "Ripples cannot ask for its phase cohort before its second child completes"
    );

    let second_offer = proliferate_offer(&batch);
    batch = engine
        .apply_scenario_command(
            0,
            &submit_current_choice(&second_offer, vec![second], vec![]),
        )
        .expect("select only permanent B in the second current child");
    let second_nested = replacement_offer(&batch);
    assert_eq!(second_nested.replacement_options.len(), 2);
    batch = engine
        .apply_scenario_command(
            0,
            &submit_current_choice(
                &second_nested,
                vec![second_nested.replacement_options[0].application_id],
                vec![],
            ),
        )
        .expect("resume the exact second nested replacement offer");
    assert_eq!(proliferated_log_count(&batch, 0), 1);
    assert_eq!(
        counter_count(&engine, first, CounterKind::PlusOnePlusOne),
        5
    );
    assert_eq!(
        counter_count(&engine, second, CounterKind::PlusOnePlusOne),
        5
    );
    assert_eq!(
        counter_count(&engine, untouched, CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(engine.state.players[1].counters[&CounterKind::Poison], 2);
    assert_eq!(stack_resolved_count(&batch, spell), 0);

    let phase_offer =
        find_resolution_choice(&batch).expect("both children produce Ripples' cohort");
    assert_eq!(
        phase_offer.choice_kind,
        rv1::ChoiceKind::PermanentObjects as i32
    );
    assert_eq!(phase_offer.candidate_object_ids, vec![first, second]);
    assert!(phase_offer.candidate_player_ids.is_empty());
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        vec![first, second],
        "each actual child receipt appears once in the union"
    );
    batch = engine
        .apply_scenario_command(
            0,
            &submit_current_choice(&phase_offer, vec![first, second], vec![]),
        )
        .expect("phase the two current cohort recipients after both children finish");
    parent_resolutions += stack_resolved_count(&batch, spell);
    assert_eq!(parent_resolutions, 1);
    assert_eq!(log_count(&batch, "Ripples of Potential resolves."), 1);
    assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
    assert!(engine.state.objects[&first].phase_status.is_some());
    assert!(engine.state.objects[&second].phase_status.is_some());
    assert!(engine.state.objects[&untouched].phase_status.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn actual_tekuthal_ripples_one_empty_child_retains_only_the_other_receipt() {
    let mut engine = actual_engine(91_601, &[0, 1]);
    inject_actual_tekuthal(&mut engine, 0);
    let first = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let second = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    add_counter(&mut engine, first, CounterKind::PlusOnePlusOne, 1);
    add_counter(&mut engine, second, CounterKind::PlusOnePlusOne, 1);
    let (spell, batch) = cast_actual_ripples(&mut engine, "ripples_of_potential");

    let first_offer = proliferate_offer(&batch);
    let first_child = engine
        .apply_scenario_command(0, &submit_current_choice(&first_offer, vec![], vec![]))
        .expect("accept an empty first Proliferate subset");
    assert_eq!(proliferated_log_count(&first_child, 0), 1);
    let second_offer = proliferate_offer(&first_child);
    let second_child = engine
        .apply_scenario_command(
            0,
            &submit_current_choice(&second_offer, vec![second], vec![]),
        )
        .expect("select only B in the remaining current Proliferate child");
    assert_eq!(proliferated_log_count(&second_child, 0), 1);
    let phase_offer = find_resolution_choice(&second_child)
        .expect("the one accepted receipt is the exact phase cohort");
    assert_eq!(
        phase_offer.choice_kind,
        rv1::ChoiceKind::PermanentObjects as i32
    );
    assert_eq!(phase_offer.candidate_object_ids, vec![second]);
    let completed = engine
        .apply_scenario_command(
            0,
            &submit_current_choice(&phase_offer, vec![second], vec![]),
        )
        .expect("phase only the permanent with an accepted child receipt");
    assert_eq!(stack_resolved_count(&completed, spell), 1);
    assert_eq!(
        counter_count(&engine, first, CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        counter_count(&engine, second, CounterKind::PlusOnePlusOne),
        2
    );
    assert!(engine.state.objects[&first].phase_status.is_none());
    assert!(engine.state.objects[&second].phase_status.is_some());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn actual_tekuthal_ripples_both_empty_children_finish_without_a_phase_cohort() {
    let mut engine = actual_engine(91_602, &[0, 1]);
    inject_actual_tekuthal(&mut engine, 0);
    let permanent = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    add_counter(&mut engine, permanent, CounterKind::PlusOnePlusOne, 1);
    let (spell, batch) = cast_actual_ripples(&mut engine, "ripples_of_potential");

    let first_offer = proliferate_offer(&batch);
    let first_child = engine
        .apply_scenario_command(0, &submit_current_choice(&first_offer, vec![], vec![]))
        .expect("accept an empty first child");
    assert_eq!(proliferated_log_count(&first_child, 0), 1);
    let second_offer = proliferate_offer(&first_child);
    let completed = engine
        .apply_scenario_command(0, &submit_current_choice(&second_offer, vec![], vec![]))
        .expect("accept an empty second child");
    assert_eq!(proliferated_log_count(&completed, 0), 1);
    assert!(!contains_choice_kind(
        &completed,
        rv1::ChoiceKind::PermanentObjects
    ));
    assert_eq!(stack_resolved_count(&completed, spell), 1);
    assert_eq!(log_count(&completed, "Ripples of Potential resolves."), 1);
    assert_eq!(
        counter_count(&engine, permanent, CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine.state.objects[&permanent].phase_status.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn actual_tekuthal_ripples_excludes_tatterkite_and_player_from_phase_recipients() {
    assert_global_card("tatterkite", "Tatterkite");
    let mut engine = actual_engine(91_603, &[0, 1]);
    inject_actual_tekuthal(&mut engine, 0);
    let tatterkite = inject_creature_on_battlefield(&mut engine, 0, "tatterkite");
    add_counter(&mut engine, tatterkite, CounterKind::Charge, 1);
    put_poison(&mut engine, 1, 1);
    let (spell, batch) = cast_actual_ripples(&mut engine, "ripples_of_potential");

    let mut batch = batch;
    for child in 0..2 {
        let offer = proliferate_offer(&batch);
        assert_eq!(offer.candidate_object_ids, vec![tatterkite]);
        assert_eq!(offer.candidate_player_ids, vec![1]);
        batch = engine
            .apply_scenario_command(0, &submit_current_choice(&offer, vec![tatterkite], vec![1]))
            .expect("select current actual Tatterkite and player receipts");
        assert_eq!(proliferated_log_count(&batch, 0), 1);
        assert_eq!(counter_count(&engine, tatterkite, CounterKind::Charge), 1);
        assert_eq!(
            engine.state.players[1].counters[&CounterKind::Poison],
            2 + child as u32
        );
        if child == 0 {
            assert_eq!(
                proliferate_offer(&batch).candidate_object_ids,
                vec![tatterkite]
            );
            assert_eq!(proliferate_offer(&batch).candidate_player_ids, vec![1]);
            assert!(!contains_choice_kind(
                &batch,
                rv1::ChoiceKind::PermanentObjects
            ));
        }
    }
    assert_eq!(stack_resolved_count(&batch, spell), 1);
    assert_eq!(log_count(&batch, "Ripples of Potential resolves."), 1);
    assert!(engine.state.objects[&tatterkite].phase_status.is_none());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

fn actual_permanent_target(engine: &GameEngine, object_id: u32) -> rv1::TargetRef {
    assert!(
        engine.state.objects.contains_key(&object_id),
        "the target object exists in the scenario state"
    );
    let generation = engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0);
    rv1::TargetRef {
        object_id,
        group_index: 0,
        kind: rv1::TargetRefKind::Permanent as i32,
        expected_zone_change_generation: Some(generation),
        ..Default::default()
    }
}

fn make_actual_tekuthal_artifact(engine: &mut GameEngine, tekuthal: u32, coating: u32) {
    let offer = engine.initial_response_batch();
    let coating_view = public_permanent(&offer, 0, coating);
    let ability = coating_view
        .activated_abilities
        .iter()
        .find(|ability| !ability.is_mana_ability && ability.activatable)
        .expect("actual Liquimetal Coating offers its AddTypes ability");
    let source = rv1::CostObjectRef {
        object_id: coating_view.object_id,
        zone_change_generation: coating_view.zone_change_generation,
    };
    let command = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
            source_object_id: source.object_id,
            ability_index: ability.ability_index,
            expected_zone_change_generation: source.zone_change_generation,
            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
            targets: vec![actual_permanent_target(engine, tekuthal)],
            payment: Some(rv1::PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(source),
                mana: Some(rv1::PaymentMana::default()),
                ..Default::default()
            }),
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(0, &command)
        .expect("actual Liquimetal Coating targets actual Tekuthal");
    resolve_entire_stack_two_player(engine);
    assert!(engine
        .characteristics(tekuthal)
        .is_some_and(|characteristics| {
            characteristics.is_creature() && characteristics.has_type("Artifact")
        }));
}

#[test]
fn actual_tekuthal_accepted_source_sacrifice_commit_and_cancel() {
    assert_global_card("liquimetal_coating", "Liquimetal Coating");
    assert_global_card("krark-clan_ironworks", "Krark-Clan Ironworks");

    let mut engine = actual_engine(91_700, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut engine, 0);
    let coating = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_coating");
    let ironworks = inject_permanent_on_battlefield(&mut engine, 0, "krark-clan_ironworks");
    make_actual_tekuthal_artifact(&mut engine, tekuthal, coating);

    let actor_index = engine.state.player_idx(0).expect("actor seat");
    let payment = PHI_PAYMENTS[1];
    engine.state.players[actor_index].mana_pool.blue = payment.blue_mana;
    let sources = counter_sources(&mut engine, 0, CounterLayout::OneEachAcrossThreePermanents);
    let offer = current_tek_activation_offer(&mut engine, 0, tekuthal);
    let contributions = sources
        .iter()
        .map(|source| offered_contribution(&offer.group, *source))
        .collect();
    let begin = begin_tek_activation(&offer, contributions, flex_payments(payment));
    let begin_batch = engine
        .apply_scenario_command(0, &begin)
        .expect("accept current counter receipts and indexed Phyrexian choices");
    let pending_before_kci = engine
        .state
        .pending_ability_activation
        .clone()
        .expect("accepted Begin opens the payment window");
    assert_eq!(pending_before_kci.source_object_id, offer.source.object_id);
    assert_eq!(pending_before_kci.flex_payments, flex_payments(payment));
    assert_eq!(pending_before_kci.cost_selections.len(), 1);
    let actor_pending = begin_batch.legal_by_player[&0]
        .pending_ability_activation
        .as_ref()
        .expect("actor receives the full accepted payment rows");
    assert_eq!(
        actor_pending.cost_selections,
        pending_before_kci.cost_selections
    );
    assert_eq!(
        actor_pending.flex_payments,
        pending_before_kci.flex_payments
    );

    let payment_offer = engine.initial_response_batch();
    let ironworks_view = public_permanent(&payment_offer, 0, ironworks);
    let ironworks_ability = ironworks_view
        .activated_abilities
        .iter()
        .find(|ability| ability.is_mana_ability && ability.activatable)
        .expect("actual Krark-Clan Ironworks is offered during payment");
    let ironworks_ref = rv1::CostObjectRef {
        object_id: ironworks_view.object_id,
        zone_change_generation: ironworks_view.zone_change_generation,
    };
    let ironworks_key = (u64::from(ironworks) << 32) | u64::from(ironworks_ability.ability_index);
    let ironworks_choices = payment_offer.legal_by_player[&0]
        .cost_choices_by_ability
        .get(&ironworks_key)
        .expect("current KCI offer includes sacrifice choices");
    let tekuthal_candidate = ironworks_choices
        .choices
        .iter()
        .flat_map(|choice| &choice.candidate_objects)
        .find(|candidate| {
            candidate
                .object
                .as_ref()
                .is_some_and(|object| object.object_id == offer.source.object_id)
        })
        .expect("current KCI offer includes actual Tekuthal as an artifact sacrifice");
    let source_cost_ref = tekuthal_candidate
        .object
        .expect("KCI candidate includes Tekuthal's current generation receipt");
    assert_eq!(source_cost_ref, offer.source);
    let kci_cost_choice = ironworks_choices
        .choices
        .iter()
        .find(|choice| {
            choice
                .candidate_objects
                .iter()
                .any(|candidate| candidate.object.as_ref() == Some(&source_cost_ref))
        })
        .expect("the exact Tekuthal receipt belongs to an offered KCI cost");
    let kci_command = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
            source_object_id: ironworks_ref.object_id,
            ability_index: ironworks_ability.ability_index,
            expected_zone_change_generation: ironworks_ref.zone_change_generation,
            source_zone: rv1::AbilitySourceZone::Battlefield as i32,
            cost_selections: vec![rv1::CostSelection {
                cost_index: kci_cost_choice.cost_index,
                selection: Some(rv1::cost_selection::Selection::PermanentId(
                    source_cost_ref.object_id,
                )),
            }],
            payment: Some(rv1::PaymentSelection {
                expected_state_revision: engine.state.command_index,
                source: Some(ironworks_ref),
                mana: Some(rv1::PaymentMana::default()),
                ..Default::default()
            }),
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(0, &kci_command)
        .expect("actual KCI sacrifices the same Tekuthal and produces two mana");
    assert_eq!(engine.state.objects[&tekuthal].zone, Zone::Graveyard);
    let sacrificed_generation = engine.state.zone_change_generation[&tekuthal];
    assert_ne!(sacrificed_generation, offer.source.zone_change_generation);
    assert_eq!(engine.state.players[actor_index].mana_pool.colorless, 2);
    assert_eq!(engine.state.players[actor_index].mana_pool.blue, 1);

    let pending = engine
        .state
        .pending_ability_activation
        .clone()
        .expect("the accepted Tekuthal payment survives KCI's sacrifice");
    assert_eq!(pending.transaction_id, pending_before_kci.transaction_id);
    assert_eq!(pending.revision, pending_before_kci.revision);
    assert_eq!(pending.cost_selections, pending_before_kci.cost_selections);
    assert_eq!(pending.flex_payments, pending_before_kci.flex_payments);
    assert_eq!(engine.state.players[actor_index].life, 20);
    for source in &sources {
        assert_eq!(
            counter_count(&engine, source.object_id, source.kind),
            source.count
        );
    }

    let accepted_commit = commit_tek_activation(
        &engine,
        &pending,
        &offer.source,
        rv1::PaymentMana {
            c: payment.generic_mana,
            u: payment.blue_mana,
            ..Default::default()
        },
    );
    let accepted_commit_message = match accepted_commit.cmd.as_ref() {
        Some(Cmd::CommitAbilityActivation(message)) => message.clone(),
        _ => panic!("the payment helper constructs a CommitAbilityActivation command"),
    };
    assert_eq!(
        accepted_commit_message.transaction_id,
        pending.transaction_id
    );
    assert_eq!(accepted_commit_message.expected_revision, pending.revision);
    let commit_payment = accepted_commit_message
        .payment
        .as_ref()
        .expect("commit supplies the separate mana payment");
    assert_eq!(commit_payment.source.as_ref(), Some(&offer.source));
    assert_eq!(
        commit_payment
            .source
            .as_ref()
            .unwrap()
            .zone_change_generation,
        offer.source.zone_change_generation,
        "commit retains the original source generation after sacrifice"
    );
    let mut stale_commit = accepted_commit.clone();
    if let Some(Cmd::CommitAbilityActivation(stale)) = stale_commit.cmd.as_mut() {
        stale.expected_revision = pending.revision.wrapping_add(1);
    } else {
        panic!("the payment helper constructs a CommitAbilityActivation command");
    }
    assert_rejected_atomically(
        &mut engine,
        0,
        &stale_commit,
        "stale Tekuthal commit revision",
    );

    let before_preview = format!("{:?}", engine.state);
    let diagnostics_before_preview = engine
        .diagnostic_snapshot()
        .expect("capture diagnostics before payment preview");
    let command_index_before_preview = engine.state.command_index;
    let preview = engine.preview_payment_scenario(
        0,
        &rv1::PreviewPayment {
            transaction_id: pending.transaction_id,
            revision: command_index_before_preview,
            commit_ability_activation: Some(accepted_commit_message),
            ..Default::default()
        },
    );
    assert!(preview.complete, "{}", preview.error);
    assert!(preview.error.is_empty(), "{}", preview.error);
    assert_eq!(format!("{:?}", engine.state), before_preview);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        diagnostics_before_preview
    );
    assert_eq!(engine.state.command_index, command_index_before_preview);

    let mut cancelled = engine.clone();
    let cancel_batch = cancelled
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CancelAbilityActivation(rv1::CancelAbilityActivation {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                })),
            },
        )
        .expect("cancel closes payment after the independent KCI action");
    assert!(cancelled.state.pending_ability_activation.is_none());
    assert!(cancel_batch.legal_by_player[&0]
        .pending_ability_activation
        .is_none());
    assert_eq!(cancelled.state.objects[&tekuthal].zone, Zone::Graveyard);
    assert_eq!(
        cancelled.state.zone_change_generation[&tekuthal],
        sacrificed_generation
    );
    assert_eq!(cancelled.state.players[actor_index].mana_pool.colorless, 2);
    assert_eq!(cancelled.state.players[actor_index].mana_pool.blue, 1);
    assert_eq!(cancelled.state.players[actor_index].life, 20);
    for source in &sources {
        assert_eq!(
            counter_count(&cancelled, source.object_id, source.kind),
            source.count
        );
    }
    assert!(cancelled.state.stack.is_empty());

    engine
        .apply_scenario_command(0, &accepted_commit)
        .expect("commit uses the original Tekuthal generation and locked rows");
    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(engine.state.command_index, command_index_before_preview + 1);
    assert_eq!(engine.state.players[actor_index].life, 18);
    assert_eq!(engine.state.players[actor_index].mana_pool.colorless, 1);
    assert_eq!(engine.state.players[actor_index].mana_pool.blue, 0);
    assert_eq!(engine.state.objects[&tekuthal].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&tekuthal],
        sacrificed_generation
    );
    for source in &sources {
        assert_eq!(counter_count(&engine, source.object_id, source.kind), 0);
    }
    assert_eq!(engine.state.stack.len(), 1);
    assert_rejected_atomically(
        &mut engine,
        0,
        &accepted_commit,
        "duplicate Tekuthal commit",
    );
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&tekuthal].zone, Zone::Graveyard);
    assert_eq!(
        counter_count(
            &engine,
            tekuthal,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        0,
        "the departed source receives no Indestructible counter"
    );
    assert!(!engine.effective_has_keyword(tekuthal, Keyword::Indestructible));
}

fn actual_tekuthal_indestructible_placement_case(
    seed: u64,
    counter_modifier: Option<(usize, &'static str)>,
    expected_counters: u32,
) {
    let mut engine = actual_engine(seed, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut engine, 0);
    match counter_modifier {
        Some((controller, "doubling_season")) => {
            inject_permanent_on_battlefield(&mut engine, controller, "doubling_season");
        }
        Some((controller, "vorinclex,_monstrous_raider")) => {
            inject_creature_on_battlefield(&mut engine, controller, "vorinclex,_monstrous_raider");
        }
        None => {}
        Some((_, card_id)) => panic!("unexpected counter replacement {card_id}"),
    }

    let actor_index = engine.state.player_idx(0).expect("actor seat");
    let payment = PHI_PAYMENTS[1];
    engine.state.players[actor_index].mana_pool.colorless = payment.generic_mana;
    engine.state.players[actor_index].mana_pool.blue = payment.blue_mana;
    let sources = counter_sources(&mut engine, 0, CounterLayout::ThreeChargeOnOnePermanent);
    let offer = current_tek_activation_offer(&mut engine, 0, tekuthal);
    let contributions = sources
        .iter()
        .map(|source| offered_contribution(&offer.group, *source))
        .collect();
    let begin = begin_tek_activation(&offer, contributions, flex_payments(payment));
    engine
        .apply_scenario_command(0, &begin)
        .expect("accept the same three current counter receipts and indexed Phyrexian choices");
    let pending = engine
        .state
        .pending_ability_activation
        .clone()
        .expect("accepted activation has a payment transaction");
    assert_eq!(pending.flex_payments, flex_payments(payment));
    assert_eq!(pending.cost_selections.len(), 1);

    let commit = commit_tek_activation(
        &engine,
        &pending,
        &offer.source,
        rv1::PaymentMana {
            c: payment.generic_mana,
            u: payment.blue_mana,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &commit)
        .expect("commit the same paid Tekuthal activation");
    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(engine.state.players[actor_index].life, 18);
    assert_eq!(engine.state.players[actor_index].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[actor_index].mana_pool.blue, 0);
    for source in &sources {
        assert_eq!(counter_count(&engine, source.object_id, source.kind), 0);
    }
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        counter_count(
            &engine,
            tekuthal,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        expected_counters
    );
    assert_eq!(
        engine.effective_has_keyword(tekuthal, Keyword::Indestructible),
        expected_counters > 0,
        "the effective keyword matches the actual counter outcome"
    );
}

#[test]
fn actual_tekuthal_indestructible_placement() {
    assert_global_card("doubling_season", "Doubling Season");
    assert_global_card("vorinclex,_monstrous_raider", "Vorinclex, Monstrous Raider");

    actual_tekuthal_indestructible_placement_case(91_701, None, 1);
    actual_tekuthal_indestructible_placement_case(91_702, Some((0, "doubling_season")), 2);
    actual_tekuthal_indestructible_placement_case(
        91_703,
        Some((1, "vorinclex,_monstrous_raider")),
        0,
    );
}

#[test]
fn actual_tekuthal_counter_prohibition_preserves_paid_costs() {
    assert_global_card("blossombind", "Blossombind");
    let mut engine = actual_engine(91_704, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut engine, 0);
    let aura = inject_card_into_hand(&mut engine, 0, "blossombind");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let aura_slot = hand_index_for_card(&engine, 0, "blossombind");
    let aura_target = actual_permanent_target(&engine, tekuthal);
    engine
        .apply_scenario_command(0, &cast_spell(aura_slot, vec![aura_target]))
        .expect("cast actual Blossombind targeting actual Tekuthal");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(tekuthal))
    );
    assert!(engine.state.objects[&tekuthal].tapped);

    let actor_index = engine.state.player_idx(0).expect("actor seat");
    let payment = PHI_PAYMENTS[1];
    engine.state.players[actor_index].mana_pool.colorless = payment.generic_mana;
    engine.state.players[actor_index].mana_pool.blue = payment.blue_mana;
    let sources = counter_sources(&mut engine, 0, CounterLayout::OneEachAcrossThreePermanents);
    let offer = current_tek_activation_offer(&mut engine, 0, tekuthal);
    assert!(engine.state.objects[&tekuthal].tapped);
    let contributions = sources
        .iter()
        .map(|source| offered_contribution(&offer.group, *source))
        .collect();
    let begin = begin_tek_activation(&offer, contributions, flex_payments(payment));
    engine
        .apply_scenario_command(0, &begin)
        .expect("tapped Tekuthal remains activatable because its ability has no tap cost");
    let pending = engine
        .state
        .pending_ability_activation
        .clone()
        .expect("accepted activation opens payment");
    assert_eq!(pending.flex_payments, flex_payments(payment));
    assert_eq!(pending.cost_selections.len(), 1);

    let commit = commit_tek_activation(
        &engine,
        &pending,
        &offer.source,
        rv1::PaymentMana {
            c: payment.generic_mana,
            u: payment.blue_mana,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &commit)
        .expect("commit the paid activation while Blossombind prohibits Tekuthal counters");
    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(engine.state.players[actor_index].life, 18);
    assert_eq!(engine.state.players[actor_index].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[actor_index].mana_pool.blue, 0);
    for source in &sources {
        assert_eq!(counter_count(&engine, source.object_id, source.kind), 0);
    }
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        counter_count(
            &engine,
            tekuthal,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        0
    );
    assert!(!engine.effective_has_keyword(tekuthal, Keyword::Indestructible));
}

fn commit_actual_tekuthal_for_followup(
    engine: &mut GameEngine,
    tekuthal: u32,
    layout: CounterLayout,
) -> (Vec<CounterSource>, rv1::CostObjectRef, u32) {
    let actor_index = engine.state.player_idx(0).expect("actor seat");
    let payment = PHI_PAYMENTS[1];
    engine.state.players[actor_index].mana_pool.colorless = payment.generic_mana;
    engine.state.players[actor_index].mana_pool.blue = payment.blue_mana;
    let sources = counter_sources(engine, 0, layout);
    let offer = current_tek_activation_offer(engine, 0, tekuthal);
    let contributions = sources
        .iter()
        .map(|source| offered_contribution(&offer.group, *source))
        .collect();
    let begin = begin_tek_activation(&offer, contributions, flex_payments(payment));
    engine
        .apply_scenario_command(0, &begin)
        .expect("accept current actual counter receipts and indexed Phyrexian choices");
    let pending = engine
        .state
        .pending_ability_activation
        .clone()
        .expect("accepted actual activation opens payment");
    assert_eq!(pending.flex_payments, flex_payments(payment));
    assert_eq!(pending.cost_selections.len(), 1);

    let commit = commit_tek_activation(
        engine,
        &pending,
        &offer.source,
        rv1::PaymentMana {
            c: payment.generic_mana,
            u: payment.blue_mana,
            ..Default::default()
        },
    );
    engine
        .apply_scenario_command(0, &commit)
        .expect("commit the actual paid Tekuthal activation");
    assert!(engine.state.pending_ability_activation.is_none());
    assert_eq!(
        engine.state.players[actor_index].life,
        20 - payment.life_paid as i32
    );
    assert_eq!(engine.state.players[actor_index].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[actor_index].mana_pool.blue, 0);
    for source in &sources {
        assert_eq!(counter_count(engine, source.object_id, source.kind), 0);
    }
    assert_eq!(engine.state.stack.len(), 1);
    let stack_item = engine
        .state
        .stack
        .last()
        .expect("activation is on the stack");
    assert_eq!(stack_item.source_permanent_id, Some(tekuthal));
    assert_eq!(
        stack_item.source_zone_change,
        offer.source.zone_change_generation
    );
    (sources, offer.source, stack_item.id)
}

fn cast_actual_murder(engine: &mut GameEngine, player: i32, target: u32) -> u32 {
    let player_seat = engine.state.player_idx(player).expect("Murder caster seat");
    let murder = inject_card_into_hand(engine, player_seat, "murder");
    give_mana(
        engine,
        player,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, player_seat, "murder");
    let target = actual_permanent_target(engine, target);
    let cast = cast_spell(slot, vec![target]);
    engine
        .apply_scenario_command(player, &cast)
        .expect("cast actual Murder at the current permanent generation");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&murder].zone, Zone::Graveyard);
    murder
}

#[test]
fn actual_tekuthal_counter_removal_loses_protection() {
    assert_global_card("murder", "Murder");
    assert_global_card("nesting_grounds", "Nesting Grounds");
    let mut engine = actual_engine(91_705, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut engine, 0);
    let grounds = inject_permanent_on_battlefield(&mut engine, 0, "nesting_grounds");
    let destination = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let (sources, _activation_source, _activation_id) = commit_actual_tekuthal_for_followup(
        &mut engine,
        tekuthal,
        CounterLayout::ThreeChargeOnOnePermanent,
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        counter_count(
            &engine,
            tekuthal,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        1
    );
    assert!(engine.effective_has_keyword(tekuthal, Keyword::Indestructible));
    for source in &sources {
        assert_eq!(counter_count(&engine, source.object_id, source.kind), 0);
    }

    cast_actual_murder(&mut engine, 0, tekuthal);
    assert_eq!(engine.state.objects[&tekuthal].zone, Zone::Battlefield);
    assert_eq!(
        counter_count(
            &engine,
            tekuthal,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        1,
        "Murder does not destroy Tekuthal while its sole protection counter remains"
    );
    assert!(engine.effective_has_keyword(tekuthal, Keyword::Indestructible));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let current_offer = engine.initial_response_batch();
    let grounds_view = public_permanent(&current_offer, 0, grounds);
    let move_ability = grounds_view
        .activated_abilities
        .iter()
        .find(|ability| !ability.is_mana_ability && ability.activatable)
        .expect("the actual Nesting Grounds move ability is offered");
    let ability_key =
        (u64::from(grounds_view.object_id) << 32) | u64::from(move_ability.ability_index);
    let targets = current_offer.legal_by_player[&0]
        .valid_targets_by_ability
        .get(&ability_key)
        .expect("current Nesting Grounds offer includes its target groups");
    assert_eq!(targets.groups.len(), 2);
    assert!(targets.groups[0].valid_permanent_ids.contains(&tekuthal));
    assert!(targets.groups[1].valid_permanent_ids.contains(&destination));
    let tekuthal_view = public_permanent(&current_offer, 0, tekuthal);
    let destination_view = public_permanent(&current_offer, 0, destination);
    let move_command = activate_ability_for(
        &engine,
        grounds,
        move_ability.ability_index,
        vec![
            rv1::TargetRef {
                object_id: tekuthal_view.object_id,
                group_index: 0,
                kind: rv1::TargetRefKind::Permanent as i32,
                expected_zone_change_generation: Some(tekuthal_view.zone_change_generation),
                ..Default::default()
            },
            rv1::TargetRef {
                object_id: destination_view.object_id,
                group_index: 1,
                kind: rv1::TargetRefKind::Permanent as i32,
                expected_zone_change_generation: Some(destination_view.zone_change_generation),
                ..Default::default()
            },
        ],
    );
    engine
        .apply_scenario_command(0, &move_command)
        .expect("activate the actual sorcery-speed Nesting Grounds ability");
    let grounds_activation_id = engine
        .state
        .stack
        .last()
        .expect("Nesting Grounds activation is on the stack")
        .id;
    assert!(engine.state.objects[&grounds].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);

    let mut counter_choice = None;
    for _ in 0..2 {
        let priority = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(priority, &pass())
            .expect("both players pass to resolve the current counter move");
        if engine.state.pending_resolution.is_some() {
            counter_choice = Some(
                find_resolution_choice(&batch)
                    .expect("Nesting Grounds publishes its current counter-kind choice"),
            );
            break;
        }
    }
    let counter_choice = counter_choice.expect("the sole counter move requests a kind choice");
    assert_eq!(
        counter_choice.choice_kind,
        rv1::ChoiceKind::ResolutionBranch as i32
    );
    let selected_branch = counter_choice
        .resolution_branches
        .iter()
        .position(|branch| branch.label.to_ascii_lowercase().contains("indestructible"))
        .expect("current counter-kind offer includes the sole Indestructible counter")
        as u32;
    let moved = engine
        .apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    resolution_prompt_id: counter_choice.resolution_prompt_id,
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: selected_branch,
                    ..Default::default()
                })),
            },
        )
        .expect("move the offered Indestructible counter");
    assert_eq!(
        counter_count(
            &engine,
            tekuthal,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        0
    );
    assert!(!engine.effective_has_keyword(tekuthal, Keyword::Indestructible));
    assert_eq!(
        counter_count(
            &engine,
            destination,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        1
    );
    assert!(engine.effective_has_keyword(destination, Keyword::Indestructible));
    assert_eq!(stack_resolved_count(&moved, grounds_activation_id), 1);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());

    cast_actual_murder(&mut engine, 0, tekuthal);
    assert_eq!(engine.state.objects[&tekuthal].zone, Zone::Graveyard);
    assert_eq!(
        counter_count(
            &engine,
            tekuthal,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        0
    );
    assert_eq!(engine.state.objects[&destination].zone, Zone::Battlefield);
    assert!(engine.effective_has_keyword(destination, Keyword::Indestructible));
}

#[test]
fn actual_tekuthal_tidebinder_counters_activation_after_costs_are_paid() {
    assert_global_card("tishanas_tidebinder", "Tishana's Tidebinder");
    let mut engine = actual_engine(91_706, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut engine, 0);
    let (sources, activation_source, activation_id) = commit_actual_tekuthal_for_followup(
        &mut engine,
        tekuthal,
        CounterLayout::ThreeChargeOnOnePermanent,
    );
    assert_eq!(engine.state.stack.last().unwrap().id, activation_id);
    assert_eq!(
        engine.state.stack.last().unwrap().source_zone_change,
        activation_source.zone_change_generation
    );

    if engine.state.priority_player_id() == 0 {
        engine
            .apply_scenario_command(0, &pass())
            .expect("pass priority to the opponent for its flash spell");
    }
    let tidebinder = inject_card_into_hand(&mut engine, 1, "tishanas_tidebinder");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let tidebinder_slot = hand_index_for_card(&engine, 1, "tishanas_tidebinder");
    engine
        .apply_scenario_command(1, &cast_spell(tidebinder_slot, Vec::new()))
        .expect("cast actual Tishana's Tidebinder with flash");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&tidebinder].zone, Zone::Battlefield);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == activation_id));
    let current_trigger_offer = engine.initial_response_batch();
    let offered_stack_targets = current_trigger_offer.legal_by_player[&1]
        .valid_targets_by_ability
        .values()
        .find(|target_offer| {
            target_offer
                .groups
                .iter()
                .any(|group| group.valid_stack_ids.contains(&activation_id))
        })
        .expect("current Tidebinder trigger offer includes the Tekuthal activation");
    assert_eq!(offered_stack_targets.groups.len(), 1);
    assert!(offered_stack_targets.groups[0]
        .valid_stack_ids
        .contains(&activation_id));
    let choose_activation = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(rv1::ChooseTriggerTarget {
            targets: vec![rv1::TargetRef {
                object_id: activation_id,
                kind: rv1::TargetRefKind::Stack as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(1, &choose_activation)
        .expect("choose the current actual Tekuthal activation for Tidebinder");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&tekuthal].zone, Zone::Battlefield);
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == activation_id));
    assert_eq!(
        counter_count(
            &engine,
            tekuthal,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        0,
        "countering the actual activation leaves no Indestructible counter"
    );
    assert_eq!(engine.state.players[0].life, 18);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    for source in &sources {
        assert_eq!(counter_count(&engine, source.object_id, source.kind), 0);
    }
    assert!(
        !engine.effective_has_keyword(tekuthal, Keyword::Flying),
        "Tidebinder also removes abilities from the creature whose ability it countered"
    );
}

#[test]
fn actual_tekuthal_prepared_return_generation_rejects_old_source_effect() {
    let mut engine = actual_engine(91_707, &[0, 1]);
    let tekuthal = inject_actual_tekuthal(&mut engine, 0);
    let actor_index = engine.state.player_idx(0).expect("actor seat");
    let (sources, activation_source, activation_id) = commit_actual_tekuthal_for_followup(
        &mut engine,
        tekuthal,
        CounterLayout::ThreeChargeOnOnePermanent,
    );
    let old_stack_source = engine
        .state
        .stack
        .iter()
        .find(|item| item.id == activation_id)
        .expect("committed actual activation remains on the stack")
        .clone();
    assert_eq!(old_stack_source.source_permanent_id, Some(tekuthal));
    assert_eq!(
        old_stack_source.source_zone_change,
        activation_source.zone_change_generation
    );

    // PREPARED leave/return setup only: preserve the exact ObjectId while the old activation is
    // stacked, and model two zone changes. This is not an ordinary sorcery-speed game line.
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&tekuthal)
        .copied()
        .unwrap_or(0);
    assert_eq!(original_generation, old_stack_source.source_zone_change);
    assert!(engine.state.players[actor_index]
        .battlefield
        .contains(&tekuthal));
    let departed_generation = original_generation.saturating_add(1);
    {
        let object = engine
            .state
            .objects
            .get_mut(&tekuthal)
            .expect("prepared source object exists");
        object.zone = Zone::Graveyard;
        object.tapped = false;
        object.damage = 0;
        object.counters.clear();
        object.counter_timestamps.clear();
    }
    engine.state.players[actor_index]
        .battlefield
        .retain(|object_id| *object_id != tekuthal);
    engine.state.players[actor_index].graveyard.push(tekuthal);
    engine
        .state
        .zone_change_generation
        .insert(tekuthal, departed_generation);

    let returned_generation = departed_generation.saturating_add(1);
    engine.state.players[actor_index]
        .graveyard
        .retain(|object_id| *object_id != tekuthal);
    engine.state.players[actor_index].battlefield.push(tekuthal);
    {
        let object = engine
            .state
            .objects
            .get_mut(&tekuthal)
            .expect("the same actual source object returns");
        object.zone = Zone::Battlefield;
        object.tapped = false;
        object.summoning_sick = true;
        object.damage = 0;
        object.counters.clear();
        object.counter_timestamps.clear();
    }
    engine
        .state
        .zone_change_generation
        .insert(tekuthal, returned_generation);
    assert_actual_tekuthal(&engine, tekuthal);
    assert_ne!(returned_generation, old_stack_source.source_zone_change);
    let returned_view = public_permanent(&engine.initial_response_batch(), 0, tekuthal);
    assert_eq!(
        returned_view.object_id,
        old_stack_source.source_permanent_id.unwrap()
    );
    assert_eq!(returned_view.zone_change_generation, returned_generation);
    assert_ne!(
        returned_view.zone_change_generation,
        old_stack_source.source_zone_change
    );

    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&tekuthal].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&tekuthal],
        returned_generation
    );
    assert_eq!(
        counter_count(
            &engine,
            tekuthal,
            CounterKind::Keyword(Keyword::Indestructible)
        ),
        0,
        "the old source incarnation cannot place onto the returned actual object"
    );
    assert!(!engine.effective_has_keyword(tekuthal, Keyword::Indestructible));
    assert_eq!(engine.state.players[actor_index].life, 18);
    assert_eq!(engine.state.players[actor_index].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[actor_index].mana_pool.blue, 0);
    for source in &sources {
        assert_eq!(counter_count(&engine, source.object_id, source.kind), 0);
    }
}
