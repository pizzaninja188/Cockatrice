use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_core::Zone;

const DRAIN_DECK: &[&str] = &[
    "countersquall",
    "undermine",
    "psychic_barrier",
    "punish_ignorance",
    "opt",
];

fn setup(seed: u64, players: &[i32]) -> GameEngine {
    let mut decks = vec![deck_with("island", &["eject", "opt", "grizzly_bears"])];
    for _ in 1..players.len() {
        decks.push(deck_with("island", DRAIN_DECK));
    }
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        players,
        20,
        Some(decks),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        counter_amount: 0,
        expected_zone_change_generation: None,
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: TargetRefKind::Stack as i32,
    }]
}

fn all_mana() -> ManaGift {
    ManaGift {
        w: 2,
        u: 4,
        b: 2,
        g: 2,
        c: 2,
        ..Default::default()
    }
}

fn life(engine: &GameEngine, player: i32) -> i32 {
    engine.state.players[engine.state.player_idx(player).unwrap()].life
}

/// Player 0 casts `card` (targeting a fresh permanent for Eject) and passes priority.
fn opponent_casts(engine: &mut GameEngine, card: &str) -> u32 {
    ensure_in_hand(engine, 0, card);
    give_mana(engine, 0, all_mana());
    let targets = if card == "eject" {
        let permanent = inject_creature_on_battlefield(engine, 1, "grizzly_bears");
        target_object(permanent)
    } else {
        vec![]
    };
    let slot = hand_index_for_card(engine, 0, card);
    engine
        .apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap();
    let spell = engine.state.stack.last().unwrap().id;
    engine.apply_scenario_command(0, &pass()).unwrap();
    spell
}

fn cast_drain(engine: &mut GameEngine, caster: i32, drain: &str, target: u32) {
    ensure_in_hand(engine, caster as usize, drain);
    give_mana(engine, caster, all_mana());
    let slot = hand_index_for_card(engine, caster as usize, drain);
    engine
        .apply_scenario_command(caster, &cast_spell(slot, stack_target(target)))
        .unwrap();
}

fn resolve_two_player(engine: &mut GameEngine) {
    engine.apply_scenario_command(1, &pass()).unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
}

#[test]
fn issue_529_countered_spell_controller_loses_the_printed_life() {
    for (seed, drain, target_card, loss) in [
        (529_101, "countersquall", "opt", 2),
        (529_102, "undermine", "opt", 3),
        (529_103, "psychic_barrier", "grizzly_bears", 1),
        (529_104, "punish_ignorance", "opt", 3),
    ] {
        let mut engine = setup(seed, &[0, 1]);
        let target = opponent_casts(&mut engine, target_card);
        cast_drain(&mut engine, 1, drain, target);
        resolve_two_player(&mut engine);

        assert_eq!(
            engine.state.objects[&target].zone,
            Zone::Graveyard,
            "{drain}"
        );
        assert_eq!(life(&engine, 0), 20 - loss, "{drain}");
        let caster_gain = if drain == "punish_ignorance" { 3 } else { 0 };
        assert_eq!(life(&engine, 1), 20 + caster_gain, "{drain}");
    }
}

#[test]
fn issue_529_legal_uncounterable_target_still_drains() {
    for (seed, drain, loss, gain) in [
        (529_201, "countersquall", 2, 0),
        (529_202, "punish_ignorance", 3, 3),
    ] {
        let mut engine = setup(seed, &[0, 1]);
        let eject = opponent_casts(&mut engine, "eject");
        cast_drain(&mut engine, 1, drain, eject);
        resolve_two_player(&mut engine);
        answer_simultaneous_entry_order_in_engine_order(&mut engine);

        assert!(
            engine.state.stack.iter().any(|item| item.id == eject),
            "{drain}: an uncounterable spell stays on the stack"
        );
        assert_eq!(life(&engine, 0), 20 - loss, "{drain}");
        assert_eq!(life(&engine, 1), 20 + gain, "{drain}");
    }
}

#[test]
fn issue_529_illegal_sole_target_fizzles_without_life_change() {
    let mut engine = setup(529_301, &[0, 1]);
    let target = opponent_casts(&mut engine, "opt");
    cast_drain(&mut engine, 1, "punish_ignorance", target);
    // The target leaves the stack before resolution, as after a responding counterspell.
    engine.state.stack.retain(|item| item.id != target);
    engine.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    engine.state.players[0].graveyard.push(target);
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_two_player(&mut engine);

    assert!(engine.state.stack.is_empty());
    assert_eq!(life(&engine, 0), 20);
    assert_eq!(
        life(&engine, 1),
        20,
        "a fizzled Punish Ignorance gains no life"
    );
}

#[test]
fn issue_529_spell_type_filters_reject_at_cast() {
    for (seed, drain, target_card) in [
        (529_401, "countersquall", "grizzly_bears"),
        (529_402, "psychic_barrier", "opt"),
    ] {
        let mut engine = setup(seed, &[0, 1]);
        let target = opponent_casts(&mut engine, target_card);
        ensure_in_hand(&mut engine, 1, drain);
        give_mana(&mut engine, 1, all_mana());
        let slot = hand_index_for_card(&engine, 1, drain);
        let revision = engine.state.command_index;
        assert!(
            engine
                .apply_scenario_command(1, &cast_spell(slot, stack_target(target)))
                .is_err(),
            "{drain} rejects {target_card}"
        );
        assert_eq!(engine.state.command_index, revision);
        assert_eq!(life(&engine, 0), 20);
    }
}

#[test]
fn issue_529_multiplayer_drains_only_the_target_controller() {
    let mut engine = setup(529_501, &[0, 1, 2]);
    let target = opponent_casts(&mut engine, "opt");
    engine.apply_scenario_command(1, &pass()).unwrap();
    cast_drain(&mut engine, 2, "undermine", target);
    engine.apply_scenario_command(2, &pass()).unwrap();
    engine.apply_scenario_command(0, &pass()).unwrap();
    engine.apply_scenario_command(1, &pass()).unwrap();

    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(life(&engine, 0), 17);
    assert_eq!(life(&engine, 1), 20);
    assert_eq!(life(&engine, 2), 20);
}

#[test]
fn issue_529_countering_your_own_spell_drains_you() {
    let mut engine = setup(529_601, &[0, 1]);
    let target = opponent_casts(&mut engine, "opt");
    // Player 1 responds with its own Opt, then counters that Opt.
    ensure_in_hand(&mut engine, 1, "opt");
    give_mana(&mut engine, 1, all_mana());
    let slot = hand_index_for_card(&engine, 1, "opt");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .unwrap();
    let own = engine.state.stack.last().unwrap().id;
    cast_drain(&mut engine, 1, "undermine", own);
    resolve_two_player(&mut engine);

    assert_eq!(engine.state.objects[&own].zone, Zone::Graveyard);
    assert!(engine.state.stack.iter().any(|item| item.id == target));
    assert_eq!(life(&engine, 1), 17);
    assert_eq!(life(&engine, 0), 20);
}
