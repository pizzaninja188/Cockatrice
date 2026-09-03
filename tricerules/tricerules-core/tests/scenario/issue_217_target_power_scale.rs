use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["mightform_harmonizer"]),
            forest_only_deck(),
        ]),
        true,
    )
    .expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn add_power_modifier(engine: &mut GameEngine, target: u32, delta_power: i32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(target),
        kind: ContinuousEffectKind::PtModify {
            delta_power,
            delta_toughness: 0,
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

fn put_landfall_on_stack(engine: &mut GameEngine, target: u32) -> u32 {
    let source = relocate_to_battlefield(engine, 0, "mightform_harmonizer", false);
    inject_card_into_hand(engine, 0, "forest");
    let forest = hand_index_for_card(engine, 0, "forest");
    engine
        .apply_command(0, &play_land(forest))
        .expect("play a Forest");
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: target_object(target),
                    ..Default::default()
                })),
            },
        )
        .expect("choose landfall target");
    source
}

#[test]
fn issue_217_doubles_signed_power_at_resolution_and_freezes_the_modifier() {
    for (seed, before_resolution, scale_delta, visible_power, after_later_modifier) in [
        (217_001_u64, 3_i32, 5_i32, 10_u32, 11_u32),
        (217_002, -2, 0, 0, 1),
        (217_003, -4, -2, 0, 0),
    ] {
        let mut engine = engine(seed);
        let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let source = put_landfall_on_stack(&mut engine, target);

        add_power_modifier(&mut engine, target, before_resolution);
        if before_resolution == 3 {
            engine.state.players[0]
                .battlefield
                .retain(|object_id| *object_id != source);
            engine.state.players[0].hand.push(source);
            engine.state.objects.get_mut(&source).expect("source").zone = Zone::Hand;
            *engine
                .state
                .zone_change_generation
                .entry(source)
                .or_default() += 1;
        }

        pass_both_players(&mut engine);
        assert_eq!(engine.effective_power(target), Some(visible_power));
        assert!(matches!(
            engine.state.continuous_effects.last().map(|effect| &effect.kind),
            Some(ContinuousEffectKind::PtModify {
                delta_power,
                delta_toughness: 0,
            }) if *delta_power == scale_delta
        ));

        add_power_modifier(&mut engine, target, 1);
        assert_eq!(
            engine.effective_power(target),
            Some(after_later_modifier),
            "the resolved doubling remains a fixed layer-7c modifier"
        );
    }
}

#[test]
fn issue_217_rejects_an_opponent_target_and_ignores_a_stale_generation() {
    let mut illegal = engine(217_010);
    let opponent = inject_creature_on_battlefield(&mut illegal, 1, "grizzly_bears");
    relocate_to_battlefield(&mut illegal, 0, "mightform_harmonizer", false);
    inject_card_into_hand(&mut illegal, 0, "forest");
    let forest = hand_index_for_card(&illegal, 0, "forest");
    illegal
        .apply_command(0, &play_land(forest))
        .expect("play a Forest");
    assert!(illegal
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: target_object(opponent),
                    ..Default::default()
                })),
            },
        )
        .is_err());

    let mut stale = engine(217_011);
    let target = inject_creature_on_battlefield(&mut stale, 0, "grizzly_bears");
    put_landfall_on_stack(&mut stale, target);
    stale.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != target);
    stale.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;
    *stale
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    stale.state.objects.get_mut(&target).expect("target").zone = Zone::Battlefield;
    stale.state.players[0].battlefield.push(target);
    *stale
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;

    pass_both_players(&mut stale);
    assert_eq!(stale.effective_power(target), Some(2));
}

#[test]
fn issue_217_only_its_controllers_land_entry_triggers() {
    let mut engine = engine(217_020);
    relocate_to_battlefield(&mut engine, 0, "mightform_harmonizer", false);

    move_ready_to_battlefield(&mut engine, 1, "forest");

    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
}
