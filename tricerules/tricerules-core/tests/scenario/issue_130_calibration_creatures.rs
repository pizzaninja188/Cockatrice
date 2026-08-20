use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, SelectedSpellMode, TargetRef,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("island".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_trigger_targets(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

fn grouped(ids: &[u32]) -> Vec<TargetRef> {
    ids.iter()
        .copied()
        .map(|object_id| TargetRef {
            object_id,
            damage_amount: 0,
            group_index: 0,
            kind: 0,
        })
        .collect()
}

fn cast_creature(engine: &mut GameEngine, card_id: &str) -> u32 {
    let object_id = inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    let index = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(index, Vec::new()))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    pass_both_players(engine);
    object_id
}

#[test]
fn iceridge_serpent_rejects_own_creature_and_bounces_opponents_creature() {
    let mut engine = engine(130_001);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_creature(&mut engine, "iceridge_serpent");

    assert!(engine
        .apply_command(0, &choose_trigger_targets(grouped(&[own])))
        .is_err());
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(0, &choose_trigger_targets(grouped(&[opposing])))
        .expect("choose opponent's creature");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Hand);
}

#[test]
fn apothecary_stomper_supports_targeted_counter_and_untargeted_life_modes() {
    let mut counters = engine(130_010);
    let target = inject_creature_on_battlefield(&mut counters, 0, "grizzly_bears");
    cast_creature(&mut counters, "apothecary_stomper");
    counters
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: vec![SelectedSpellMode {
                        mode_index: 0,
                        targets: grouped(&[target]),
                    }],
                    targets: Vec::new(),
                })),
            },
        )
        .expect("choose counter mode");
    pass_both_players(&mut counters);
    assert_eq!(
        counters.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        2
    );

    let mut life = engine(130_011);
    cast_creature(&mut life, "apothecary_stomper");
    life.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                decline: false,
                selected_modes: vec![SelectedSpellMode {
                    mode_index: 1,
                    targets: Vec::new(),
                }],
                targets: Vec::new(),
            })),
        },
    )
    .expect("choose life mode");
    pass_both_players(&mut life);
    assert_eq!(life.state.players[0].life, 24);
}

#[test]
fn felidar_savior_accepts_zero_or_two_distinct_other_controlled_targets() {
    let mut zero = engine(130_020);
    cast_creature(&mut zero, "felidar_savior");
    zero.apply_command(0, &choose_trigger_targets(Vec::new()))
        .expect("choose zero targets");
    pass_both_players(&mut zero);

    let mut two = engine(130_021);
    let first = inject_creature_on_battlefield(&mut two, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut two, 0, "storm_crow");
    let source = cast_creature(&mut two, "felidar_savior");
    assert!(two
        .apply_command(0, &choose_trigger_targets(grouped(&[first, first])))
        .is_err());
    assert!(two
        .apply_command(0, &choose_trigger_targets(grouped(&[first, source])))
        .is_err());
    two.apply_command(0, &choose_trigger_targets(grouped(&[first, second])))
        .expect("choose two other controlled creatures");
    pass_both_players(&mut two);
    for object_id in [first, second] {
        assert_eq!(
            two.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne),
            1
        );
    }

    let mut partial = engine(130_022);
    let legal = inject_creature_on_battlefield(&mut partial, 0, "grizzly_bears");
    let departed = inject_creature_on_battlefield(&mut partial, 0, "storm_crow");
    cast_creature(&mut partial, "felidar_savior");
    partial
        .apply_command(0, &choose_trigger_targets(grouped(&[legal, departed])))
        .expect("choose two targets before one leaves");
    partial.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != departed);
    partial.state.players[0].graveyard.push(departed);
    partial
        .state
        .objects
        .get_mut(&departed)
        .expect("departed")
        .zone = Zone::Graveyard;
    pass_both_players(&mut partial);
    assert_eq!(
        partial.state.objects[&legal].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        partial.state.objects[&departed].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn dwynens_elite_rechecks_the_other_elf_condition_on_resolution() {
    let mut engine = engine(130_030);
    let other_elf = inject_creature_on_battlefield(&mut engine, 0, "elvish_visionary");
    cast_creature(&mut engine, "dwynens_elite");
    assert_eq!(engine.state.stack.len(), 1);

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != other_elf);
    engine.state.players[0].graveyard.push(other_elf);
    engine.state.objects.get_mut(&other_elf).expect("Elf").zone = Zone::Graveyard;
    pass_both_players(&mut engine);
    assert!(battlefield_token_oids(&engine, 0, "elf_warrior_g_1_1").is_empty());
}

#[test]
fn delta_bloodflies_rechecks_the_live_counter_condition() {
    let mut engine = engine(130_040);
    inject_card_into_hand(&mut engine, 0, "delta_bloodflies");
    let bloodflies = relocate_to_battlefield(&mut engine, 0, "delta_bloodflies", false);
    engine
        .state
        .objects
        .get_mut(&bloodflies)
        .expect("Bloodflies")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    engine
        .apply_command(0, &declare_attackers(vec![bloodflies]))
        .expect("attack");
    assert_eq!(engine.state.stack.len(), 1);
    engine
        .state
        .objects
        .get_mut(&bloodflies)
        .expect("Bloodflies")
        .set_counter(CounterKind::PlusOnePlusOne, 0);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn watcher_of_the_wayside_mills_the_chosen_player_then_gains_life() {
    let mut engine = engine(130_050);
    let library_before = engine.state.players[1].library.len();
    cast_creature(&mut engine, "watcher_of_the_wayside");
    engine
        .apply_command(0, &choose_trigger_targets(target_player(1)))
        .expect("target the opponent");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.players[1].library.len(), library_before - 2);
    assert_eq!(engine.state.players[1].graveyard.len(), 2);
    assert_eq!(engine.state.players[0].life, 22);
}

#[test]
fn sanguine_syphoner_drains_each_opponent_recipient_when_it_attacks() {
    let mut engine = engine(130_060);
    let syphoner = inject_creature_on_battlefield(&mut engine, 0, "sanguine_syphoner");
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![syphoner]))
        .expect("attack with Sanguine Syphoner");
    assert_eq!(engine.state.stack.len(), 1);
    pass_both_players(&mut engine);

    assert_eq!(engine.state.players[0].life, 21);
    assert_eq!(engine.state.players[1].life, 19);
}

#[test]
fn underfoot_underdogs_revalidates_power_and_publishes_a_resolved_restriction() {
    let mut stale = engine(130_070);
    let underdogs = inject_creature_on_battlefield(&mut stale, 0, "underfoot_underdogs");
    let target = inject_creature_on_battlefield(&mut stale, 0, "grizzly_bears");
    grant_pool(&mut stale, 0);
    stale
        .apply_command(0, &activate_ability(underdogs, 0, target_object(target)))
        .expect("activate on a power-2 creature");
    stale
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .add_counters(CounterKind::PlusOnePlusOne, 1, stale.state.command_index);
    pass_both_players(&mut stale);
    assert!(zone_view_rules_annotation_labels(&mut stale, 0, target).is_empty());

    let mut resolved = engine(130_071);
    let underdogs = inject_creature_on_battlefield(&mut resolved, 0, "underfoot_underdogs");
    let target = inject_creature_on_battlefield(&mut resolved, 0, "grizzly_bears");
    grant_pool(&mut resolved, 0);
    resolved
        .apply_command(0, &activate_ability(underdogs, 0, target_object(target)))
        .expect("activate on a power-2 creature");
    pass_both_players(&mut resolved);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut resolved, 0, target),
        vec!["Can't be blocked"]
    );
}

#[test]
fn elfsworn_giant_landfall_creates_the_new_elf_warrior_token() {
    let mut engine = engine(130_080);
    inject_creature_on_battlefield(&mut engine, 0, "elfsworn_giant");
    inject_card_into_hand(&mut engine, 0, "forest");
    let forest = hand_index_for_card(&engine, 0, "forest");
    engine
        .apply_command(0, &play_land(forest))
        .expect("play a Forest");
    assert_eq!(engine.state.stack.len(), 1);
    pass_both_players(&mut engine);

    assert_eq!(
        battlefield_token_oids(&engine, 0, "elf_warrior_g_1_1").len(),
        1
    );
}
