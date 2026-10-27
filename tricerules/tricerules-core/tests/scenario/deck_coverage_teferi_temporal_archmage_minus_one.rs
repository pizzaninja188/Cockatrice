//! Actual Teferi, Temporal Archmage -1 activations through the global registered card.
//! The cohort's conditional ability removal is synthetic and exercises Blossombind's real
//! attached prohibition.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{ConditionObjectRef, CounterKind, GameCondition};
use tricerules_cards::{ContinuousEffectKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

const TEFERI: &str = "teferi,_temporal_archmage";
const CONTROLLER: i32 = 10;
const OPPONENT: i32 = 20;

fn setup(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    registry
        .get(TEFERI)
        .expect("Teferi, Temporal Archmage must be registered for its -1 scenarios");
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[CONTROLLER, OPPONENT],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("two-player Teferi scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_teferi(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, TEFERI);
    give_mana(
        engine,
        CONTROLLER,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, TEFERI);
    engine
        .apply_scenario_command(CONTROLLER, &cast_spell(slot, Vec::new()))
        .expect("cast Teferi for {4}{U}{U}");
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    pass_both_players(engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5,
        "Teferi enters with the registered starting loyalty"
    );
    source
}

fn cast_blossombind(engine: &mut GameEngine, creature: u32) -> u32 {
    let aura = inject_card_into_hand(engine, 0, "blossombind");
    give_mana(
        engine,
        CONTROLLER,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "blossombind");
    engine
        .apply_scenario_command(CONTROLLER, &cast_spell(slot, target_refs(&[creature])))
        .expect("cast Blossombind targeting the creature");
    assert_eq!(engine.state.players[0].mana_pool, Default::default());
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
    assert!(
        engine.state.objects[&creature].tapped,
        "Blossombind's ETB taps it"
    );
    aura
}

fn target_refs(objects: &[u32]) -> Vec<TargetRef> {
    objects
        .iter()
        .map(|object_id| TargetRef {
            object_id: *object_id,
            group_index: 0,
            kind: 0,
            ..Default::default()
        })
        .collect()
}

fn activate_minus_one(engine: &mut GameEngine, source: u32, objects: &[u32]) {
    let command = activate_ability_for(engine, source, 1, target_refs(objects));
    engine
        .apply_scenario_command(CONTROLLER, &command)
        .expect("activate Teferi's -1 ability");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        4,
        "the -1 loyalty cost is paid on activation"
    );
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .expect("ability on stack")
            .targets
            .len(),
        objects.len(),
        "the activated ability records every selected target"
    );
}

fn resolve_ability(engine: &mut GameEngine) -> RuledEventBatch {
    for _ in 0..4 {
        let batch = pass_both_players_with_batch(engine);
        if engine.state.stack.is_empty() {
            return batch;
        }
    }
    panic!("Teferi's -1 ability did not finish resolving");
}

fn inject_tapped_permanent(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object = inject_permanent_on_battlefield(engine, player, card_id);
    engine.state.objects.get_mut(&object).unwrap().tapped = true;
    object
}

fn inject_tapped_creature(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object = inject_creature_on_battlefield(engine, player, card_id);
    engine.state.objects.get_mut(&object).unwrap().tapped = true;
    object
}

fn untapped_ids(batch: &RuledEventBatch) -> Vec<u32> {
    batch
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref() {
            Some(Ev::PermanentsUntapped(untapped)) => Some(untapped.object_ids.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn assert_untapped_ids(batch: &RuledEventBatch, expected: &[u32]) {
    let mut actual = untapped_ids(batch);
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "only actual untap edges are published");
}

fn assert_loyalty_activation_rejected(engine: &mut GameEngine, source: u32, targets: &[u32]) {
    let before = engine
        .diagnostic_snapshot()
        .expect("snapshot before rejected target announcement");
    let command = activate_ability_for(engine, source, 1, target_refs(targets));
    assert!(engine.apply_scenario_command(CONTROLLER, &command).is_err());
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("snapshot after rejected target announcement"),
        before,
        "invalid targets do not pay loyalty or change state"
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );
    assert!(engine.state.stack.is_empty());
}

fn zone_generation(engine: &GameEngine, object: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object)
        .copied()
        .unwrap_or(0)
}

fn dev_move_named(engine: &mut GameEngine, card_name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: CONTROLLER,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: card_name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("move the named physical card through a synthetic dev command");
}

#[test]
fn teferi_minus_one_accepts_zero_one_and_four_any_permanent_targets() {
    let mut zero = setup(26_102_711);
    let zero_source = cast_teferi(&mut zero);
    activate_minus_one(&mut zero, zero_source, &[]);
    let zero_batch = resolve_ability(&mut zero);
    assert!(zero.state.stack.is_empty());
    assert_untapped_ids(&zero_batch, &[]);

    let mut one = setup(26_102_712);
    let one_source = cast_teferi(&mut one);
    let tapped_land = inject_tapped_permanent(&mut one, 0, "forest");
    activate_minus_one(&mut one, one_source, &[tapped_land]);
    let one_batch = resolve_ability(&mut one);
    assert!(!one.state.objects[&tapped_land].tapped);
    assert_untapped_ids(&one_batch, &[tapped_land]);

    let mut four = setup(26_102_713);
    let four_source = cast_teferi(&mut four);
    let targets = [
        inject_tapped_permanent(&mut four, 0, "forest"),
        inject_tapped_permanent(&mut four, 0, "sol_ring"),
        inject_tapped_permanent(&mut four, 1, "propaganda"),
        inject_tapped_creature(&mut four, 1, "grizzly_bears"),
    ];
    activate_minus_one(&mut four, four_source, &targets);
    let four_batch = resolve_ability(&mut four);
    assert!(targets
        .iter()
        .all(|object| !four.state.objects[object].tapped));
    assert_untapped_ids(&four_batch, &targets);
}

#[test]
fn teferi_minus_one_rejects_five_or_duplicate_targets_without_paying() {
    let mut engine = setup(26_102_714);
    let source = cast_teferi(&mut engine);
    let targets = [
        inject_tapped_permanent(&mut engine, 0, "forest"),
        inject_tapped_permanent(&mut engine, 0, "sol_ring"),
        inject_tapped_permanent(&mut engine, 0, "island"),
        inject_tapped_permanent(&mut engine, 0, "propaganda"),
        inject_tapped_creature(&mut engine, 1, "grizzly_bears"),
    ];

    assert_loyalty_activation_rejected(&mut engine, source, &targets);
    assert_loyalty_activation_rejected(&mut engine, source, &[targets[0], targets[0]]);
}

#[test]
fn teferi_minus_one_skips_partial_and_all_stale_target_generations() {
    let mut partial = setup(26_102_715);
    let source = cast_teferi(&mut partial);
    let stale = inject_tapped_permanent(&mut partial, 0, "island");
    let valid = inject_tapped_creature(&mut partial, 0, "grizzly_bears");
    let original_generation = zone_generation(&partial, stale);
    activate_minus_one(&mut partial, source, &[stale, valid]);

    dev_move_named(&mut partial, "Island", DevZone::Graveyard);
    dev_move_named(&mut partial, "Island", DevZone::Battlefield);
    assert_eq!(partial.state.objects[&stale].zone, Zone::Battlefield);
    assert_ne!(zone_generation(&partial, stale), original_generation);
    partial.state.objects.get_mut(&stale).unwrap().tapped = true;

    let partial_batch = resolve_ability(&mut partial);
    assert!(partial.state.objects[&stale].tapped);
    assert!(!partial.state.objects[&valid].tapped);
    assert_untapped_ids(&partial_batch, &[valid]);

    let mut all_stale = setup(26_102_716);
    let all_stale_source = cast_teferi(&mut all_stale);
    let only_target = inject_tapped_permanent(&mut all_stale, 0, "island");
    let old_generation = zone_generation(&all_stale, only_target);
    activate_minus_one(&mut all_stale, all_stale_source, &[only_target]);
    dev_move_named(&mut all_stale, "Island", DevZone::Graveyard);
    dev_move_named(&mut all_stale, "Island", DevZone::Battlefield);
    assert_ne!(zone_generation(&all_stale, only_target), old_generation);
    all_stale
        .state
        .objects
        .get_mut(&only_target)
        .unwrap()
        .tapped = true;

    let all_stale_batch = resolve_ability(&mut all_stale);
    assert!(all_stale.state.objects[&only_target].tapped);
    assert_untapped_ids(&all_stale_batch, &[]);
}

#[test]
fn teferi_minus_one_stun_and_prohibition_do_not_emit_false_untap_edges() {
    let mut engine = setup(26_102_717);
    let source = cast_teferi(&mut engine);
    let stunned = inject_tapped_creature(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&stunned)
        .unwrap()
        .set_counter(CounterKind::Stun, 1);
    let prohibited = inject_tapped_permanent(&mut engine, 0, "forest");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(prohibited),
        kind: ContinuousEffectKind::ProhibitUntap,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    let already_untapped = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let plain = inject_tapped_permanent(&mut engine, 1, "propaganda");

    activate_minus_one(
        &mut engine,
        source,
        &[stunned, prohibited, already_untapped, plain],
    );
    let batch = resolve_ability(&mut engine);

    assert!(engine.state.objects[&stunned].tapped);
    assert_eq!(
        engine.state.objects[&stunned].counter_count(CounterKind::Stun),
        0,
        "the attempted untap removes one stun counter instead"
    );
    assert!(engine.state.objects[&prohibited].tapped);
    assert!(!engine.state.objects[&already_untapped].tapped);
    assert!(!engine.state.objects[&plain].tapped);
    assert_untapped_ids(&batch, &[plain]);
}

#[test]
fn teferi_minus_one_prepares_synthetic_conditional_prohibition_cohort_in_both_orders() {
    for (offset, reverse) in [(0, false), (1, true)] {
        let mut engine = setup(26_102_718 + offset);
        let source = cast_teferi(&mut engine);
        let first = inject_tapped_permanent(&mut engine, 0, "island");
        let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let aura = cast_blossombind(&mut engine, second);

        // Synthetic layer-6 removal suppresses the real Blossombind Aura abilities only while
        // `first` is tapped. This makes its actual attached cant-untap ability conditional.
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(first),
            affected: AffectedScope::Single(aura),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: Some(GameCondition::ObjectTapped {
                object: ConditionObjectRef::Source,
                tapped: true,
            }),
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: engine.state.command_index,
        });
        assert!(engine.state.objects[&first].tapped);

        let seeker = inject_creature_on_battlefield(&mut engine, 0, "seeker_of_skybreak");
        let initial_probe = activate_ability_for(&engine, seeker, 0, target_refs(&[second]));
        engine
            .apply_scenario_command(CONTROLLER, &initial_probe)
            .expect("Seeker can legally target Blossombind's tapped creature");
        assert!(
            engine.state.objects[&seeker].tapped,
            "Seeker pays its tap cost"
        );
        let initial_batch = resolve_ability(&mut engine);
        assert!(!engine.state.objects[&second].tapped);
        assert_untapped_ids(&initial_batch, &[second]);

        // Recreate tapped targets and ready the same Seeker for the post-cohort probe.
        engine.state.objects.get_mut(&second).unwrap().tapped = true;
        engine.state.objects.get_mut(&seeker).unwrap().tapped = false;

        let targets = if reverse {
            [second, first]
        } else {
            [first, second]
        };
        activate_minus_one(&mut engine, source, &targets);
        let batch = resolve_ability(&mut engine);
        assert!(!engine.state.objects[&first].tapped);
        assert!(!engine.state.objects[&second].tapped);
        assert_untapped_ids(&batch, &[first, second]);

        // Blossombind's actual attached prohibition is back after `first` untaps. Confirm it by
        // attempting a second ordinary targeted untap with the same Seeker.
        engine.state.objects.get_mut(&second).unwrap().tapped = true;
        let control = activate_ability_for(&engine, seeker, 0, target_refs(&[second]));
        engine
            .apply_scenario_command(CONTROLLER, &control)
            .expect("Seeker can legally target the tapped creature");
        assert!(
            engine.state.objects[&seeker].tapped,
            "Seeker pays its tap cost"
        );
        let control = resolve_ability(&mut engine);
        assert!(engine.state.objects[&second].tapped);
        assert_untapped_ids(&control, &[]);
    }
}
