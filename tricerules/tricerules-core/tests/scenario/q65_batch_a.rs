//! Actual-card scenarios for the queue-v65 batch A cohorts.
//!
//! Three stable typed shapes: an activated damage ability with a printed target restriction or
//! rider, an enters-the-battlefield +1/+1-counter placement with a printed target scope, and a
//! destroy-target-permanent spell or activated artifact. Expectations are printed Oracle values.
//! CR 602.2 and 118.3 govern activation and costs; CR 115.1/115.3 target count and distinctness;
//! CR 120.3 damage; CR 603.6a entry triggers; CR 122.1 counters; CR 701.8 destruction; CR 608.2b
//! removes an ability whose only target is illegal at resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef,
};

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn plus_one(e: &GameEngine, object: u32) -> u32 {
    e.state.objects[&object]
        .counters
        .get(&CounterKind::PlusOnePlusOne)
        .copied()
        .unwrap_or(0)
}

fn cast_named(
    e: &mut GameEngine,
    player: i32,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) -> u32 {
    inject_card_into_hand(e, player as usize, card);
    give_mana(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    semantic::accepted(e, player, &cast_spell(slot, targets));
    e.state.stack.last().expect("cast spell on stack").id
}

fn choose_trigger(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets,
            ..Default::default()
        })),
    }
}

/// Cast a creature, resolve the spell, and park its entry trigger for target selection.
fn cast_to_pending_etb(e: &mut GameEngine, card: &str, mana: ManaGift) -> u32 {
    let source = inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    give_mana(e, 0, gift(&mana));
    semantic::accepted(e, 0, &cast_spell(slot, vec![]));
    for _ in 0..e.state.players.len() {
        let player = e.state.priority_player_id();
        semantic::accepted(e, player, &pass());
    }
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "{card} enters before its trigger"
    );
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "{card} parks one entry-trigger target"
    );
    source
}

fn resolve_etb_with_targets(
    e: &mut GameEngine,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) -> u32 {
    let source = cast_to_pending_etb(e, card, mana);
    semantic::accepted(e, 0, &choose_trigger(targets));
    resolve_entire_stack_two_player(e);
    source
}

fn ready(e: &mut GameEngine, object: u32) {
    e.state
        .objects
        .get_mut(&object)
        .expect("object")
        .summoning_sick = false;
}

fn activate(
    e: &mut GameEngine,
    player: i32,
    permanent: u32,
    ability: u32,
    targets: Vec<TargetRef>,
) {
    semantic::accepted(
        e,
        player,
        &activate_ability_for(e, permanent, ability, targets),
    );
}

// ── 01 activated damage ────────────────────────────────────────────────────

#[test]
fn q65_ping_canary_simplest_goblin_fireslinger_hits_a_player() {
    let mut e = semantic::main_phase(65_010_001);
    let slinger = cast_named(
        &mut e,
        0,
        "goblin_fireslinger",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        vec![],
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&slinger].zone, Zone::Battlefield);
    assert_eq!(
        (e.effective_power(slinger), e.effective_toughness(slinger)),
        (Some(1), Some(1))
    );
    ready(&mut e, slinger);
    activate(&mut e, 0, slinger, 0, target_player(1));
    assert_eq!(e.state.players[1].life, 20, "damage only on resolution");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 19,
        "one damage to the target player"
    );
    assert_eq!(e.state.players[0].life, 20, "no damage to the controller");
    assert!(
        e.state.objects[&slinger].tapped,
        "tap is the activation cost"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q65_ping_canary_hardest_fireslinger_self_damage_and_stale_target() {
    // Target player: the printed 1 damage to any target and 1 damage to you both apply.
    let mut e = semantic::main_phase(65_010_002);
    let slinger = cast_named(
        &mut e,
        0,
        "fireslinger",
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    ready(&mut e, slinger);
    activate(&mut e, 0, slinger, 0, target_player(1));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 19, "printed damage to the target");
    assert_eq!(e.state.players[0].life, 19, "printed damage to you");
    assert!(e.state.objects[&slinger].tapped);

    // Stale sole target: the whole ability is removed, including the self-damage.
    let mut e = semantic::main_phase(65_010_003);
    let slinger = cast_named(
        &mut e,
        0,
        "fireslinger",
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    ready(&mut e, slinger);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
    activate(&mut e, 0, slinger, 0, target_object(bear));
    semantic::accepted(&mut e, 0, &pass());
    cast_named(
        &mut e,
        1,
        "unsummon",
        ManaGift {
            u: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&bear].zone,
        Zone::Hand,
        "the target left the battlefield"
    );
    assert_eq!(
        e.state.players[0].life, 20,
        "stale target cancels self-damage"
    );
    assert_eq!(e.state.players[1].life, 20, "no damage was dealt");
    assert!(e.state.stack.is_empty());
}

#[test]
fn q65_ping_cards_apply_printed_damage_costs_and_target_scope() {
    // Valakut Invoker: {8} for 3 damage to any target.
    let mut e = semantic::main_phase(65_011_001);
    let invoker = cast_named(
        &mut e,
        0,
        "valakut_invoker",
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    ready(&mut e, invoker);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 8,
            ..Default::default()
        },
    );
    activate(&mut e, 0, invoker, 0, target_player(1));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 17);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{8}} activation");
    assert!(
        !e.state.objects[&invoker].tapped,
        "the ability does not tap it"
    );

    // Aeolipile: {1}, {T}, sacrifice for 2 damage; the artifact is the sacrificed source.
    let mut e = semantic::main_phase(65_011_002);
    let artifact = cast_named(
        &mut e,
        0,
        "aeolipile",
        ManaGift {
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, artifact, 0, target_player(1));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 18);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{1}} activation");

    // Goblin Fireslinger cannot target a creature; its printed scope is player or planeswalker.
    let mut e = semantic::main_phase(65_011_003);
    let slinger = cast_named(
        &mut e,
        0,
        "goblin_fireslinger",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    ready(&mut e, slinger);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    assert!(
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, slinger, 0, target_object(bear))
        )
        .is_err(),
        "a creature is not a player or planeswalker"
    );
    activate(&mut e, 0, slinger, 0, target_player(1));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 19);
}

#[test]
fn q65_ping_canary_goblin_fireslinger_hits_a_planeswalker() {
    let mut e = semantic::main_phase(65_012_001);
    let jace = cast_named(
        &mut e,
        0,
        "jace_beleren",
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&jace].counters.get(&CounterKind::Loyalty),
        Some(&3),
        "printed starting loyalty"
    );
    let slinger = cast_named(
        &mut e,
        0,
        "goblin_fireslinger",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    ready(&mut e, slinger);
    activate(&mut e, 0, slinger, 0, target_object(jace));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&jace].counters.get(&CounterKind::Loyalty),
        Some(&2),
        "one damage removes one loyalty"
    );
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[1].life, 20);
}

// ── 02 enters-the-battlefield counters ─────────────────────────────────────

#[test]
fn q65_counters_canary_simplest_supply_line_cranes_counters_a_chosen_creature() {
    let mut e = semantic::main_phase(65_020_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let cranes = resolve_etb_with_targets(
        &mut e,
        "supply-line_cranes",
        ManaGift {
            w: 2,
            c: 3,
            ..Default::default()
        },
        target_object(target),
    );
    assert_eq!(plus_one(&e, target), 1);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(3))
    );
    assert_eq!(plus_one(&e, cranes), 0, "the source itself is unchanged");
    assert_eq!(
        (e.effective_power(cranes), e.effective_toughness(cranes)),
        (Some(2), Some(4))
    );
    assert!(e.effective_has_keyword(cranes, Keyword::Flying));
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
}

#[test]
fn q65_counters_canary_hardest_basris_acolyte_two_distinct_other_creatures() {
    let mut e = semantic::main_phase(65_020_002);
    let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
    let enemy = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let acolyte = cast_to_pending_etb(
        &mut e,
        "basris_acolyte",
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    assert!(e.effective_has_keyword(acolyte, Keyword::Lifelink));

    // "other" excludes the source itself, and the controller scope excludes an opponent.
    assert!(
        e.apply_scenario_command(0, &choose_trigger(target_object(acolyte)))
            .is_err(),
        "the Acolyte cannot target itself"
    );
    assert!(
        e.apply_scenario_command(0, &choose_trigger(target_object(enemy)))
            .is_err(),
        "an opponent's creature is out of scope"
    );
    // The pinned 2020-06-23 ruling: the same creature cannot be targeted twice.
    assert!(
        e.apply_scenario_command(
            0,
            &choose_trigger(vec![target_object(first)[0], target_object(first)[0]])
        )
        .is_err(),
        "duplicate targets are rejected"
    );
    semantic::accepted(
        &mut e,
        0,
        &choose_trigger(vec![target_object(first)[0], target_object(second)[0]]),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one(&e, first), 1);
    assert_eq!(plus_one(&e, second), 1);
    assert_eq!(plus_one(&e, enemy), 0);
    assert_eq!(plus_one(&e, acolyte), 0);
}

#[test]
fn q65_counters_cards_apply_their_printed_target_scopes() {
    // Gavony Silversmith: up to two target creatures, no controller restriction; zero is legal.
    let mut e = semantic::main_phase(65_021_001);
    let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let source = resolve_etb_with_targets(
        &mut e,
        "gavony_silversmith",
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
        vec![target_object(mine)[0], target_object(theirs)[0]],
    );
    assert_eq!(plus_one(&e, mine), 1);
    assert_eq!(plus_one(&e, theirs), 1);
    assert_eq!(plus_one(&e, source), 0);

    let mut e = semantic::main_phase(65_021_002);
    let source = resolve_etb_with_targets(
        &mut e,
        "gavony_silversmith",
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
        vec![],
    );
    assert_eq!(plus_one(&e, source), 0, "zero targets is legal");

    // Keen-Eyed Raven: another creature you control, not itself and not an opponent's creature.
    let mut e = semantic::main_phase(65_021_003);
    let raven = inject_creature_on_battlefield(&mut e, 0, "keen-eyed_raven");
    let mine = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let theirs = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    assert!(e.effective_has_keyword(raven, Keyword::Flying));
    let source = cast_to_pending_etb(
        &mut e,
        "keen-eyed_raven",
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &choose_trigger(target_object(source)))
            .is_err(),
        "another excludes the Raven itself"
    );
    assert!(
        e.apply_scenario_command(0, &choose_trigger(target_object(theirs)))
            .is_err(),
        "only a creature you control is legal"
    );
    semantic::accepted(&mut e, 0, &choose_trigger(target_object(mine)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(plus_one(&e, mine), 1);
    assert_eq!(plus_one(&e, source), 0);
    assert_eq!(
        plus_one(&e, raven),
        0,
        "the pre-existing Raven is not the chosen target"
    );

    // A lone Keen-Eyed Raven has no legal target; the trigger is simply removed.
    let mut e = semantic::main_phase(65_021_004);
    let source = cast_named(
        &mut e,
        0,
        "keen-eyed_raven",
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    assert!(e.state.stack.is_empty(), "no legal target, no trigger");
    assert!(e.state.pending_triggers.is_empty());
    assert!(e.state.blocking_choice().is_none());
}

#[test]
fn q65_counters_partial_stale_targets_still_counter_the_remaining_creature() {
    let mut e = semantic::main_phase(65_022_001);
    let kept = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let bounced = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
    let _acolyte = cast_to_pending_etb(
        &mut e,
        "basris_acolyte",
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    semantic::accepted(
        &mut e,
        0,
        &choose_trigger(vec![target_object(kept)[0], target_object(bounced)[0]]),
    );
    semantic::accepted(&mut e, 0, &pass());
    cast_named(
        &mut e,
        1,
        "unsummon",
        ManaGift {
            u: 1,
            ..Default::default()
        },
        target_object(bounced),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bounced].zone, Zone::Hand);
    assert_eq!(plus_one(&e, bounced), 0);
    assert_eq!(
        plus_one(&e, kept),
        1,
        "the remaining legal target still gets its counter"
    );
}

// ── 03 destroy target permanent ────────────────────────────────────────────

#[test]
fn q65_destroy_canary_simplest_universal_solvent_activates_and_sacrifices() {
    let mut e = semantic::main_phase(65_030_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let solvent = cast_named(
        &mut e,
        0,
        "universal_solvent",
        ManaGift {
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
    resolve_entire_stack_two_player(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    activate(&mut e, 0, solvent, 0, target_object(target));
    assert_eq!(
        e.state.objects[&solvent].zone,
        Zone::Graveyard,
        "sacrifice is an activation cost, not a resolution effect"
    );
    assert!(e.state.players[0].graveyard.contains(&solvent));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&target].owner, e.state.players[1].id,
        "the destroyed creature is owned by its original controller"
    );
    assert!(
        e.state.players[1].graveyard.contains(&target),
        "the destroyed creature reaches its owner's graveyard"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{7}} activation");
    assert!(e.state.stack.is_empty());
}

#[test]
fn q65_destroy_canary_hardest_vindicate_any_permanent_and_stale_target() {
    let mut e = semantic::main_phase(65_030_002);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let vindicate = cast_named(
        &mut e,
        0,
        "vindicate",
        ManaGift {
            w: 1,
            b: 1,
            c: 1,
            ..Default::default()
        },
        target_object(land),
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert!(e.state.players[1].graveyard.contains(&land));
    assert_eq!(e.state.objects[&vindicate].zone, Zone::Graveyard);
    assert!(e.state.players[0].graveyard.contains(&vindicate));

    let mut e = semantic::main_phase(65_030_003);
    let bear = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let vindicate = cast_named(
        &mut e,
        0,
        "vindicate",
        ManaGift {
            w: 1,
            b: 1,
            c: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    semantic::accepted(&mut e, 0, &pass());
    cast_named(
        &mut e,
        1,
        "unsummon",
        ManaGift {
            u: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Hand);
    assert_eq!(
        e.state.objects[&vindicate].zone,
        Zone::Graveyard,
        "the stale-targeted spell still goes to the graveyard"
    );
    assert!(e.state.players[0].graveyard.contains(&vindicate));
}

#[test]
fn q65_destroy_cards_destroy_their_printed_permanent_types() {
    // Desert Twister destroys an enchantment for {4}{G}{G}.
    let mut e = semantic::main_phase(65_031_001);
    let ring = inject_permanent_on_battlefield(&mut e, 1, "pacifism");
    cast_named(
        &mut e,
        0,
        "desert_twister",
        ManaGift {
            g: 2,
            c: 4,
            ..Default::default()
        },
        target_object(ring),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&ring].zone, Zone::Graveyard);
    assert!(e.state.players[1].graveyard.contains(&ring));
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));

    // Vindicate destroys an artifact and it reaches its owner's graveyard.
    let mut e = semantic::main_phase(65_031_003);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "sol_ring");
    cast_named(
        &mut e,
        0,
        "vindicate",
        ManaGift {
            w: 1,
            b: 1,
            c: 1,
            ..Default::default()
        },
        target_object(artifact),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&artifact].owner, e.state.players[1].id);
    assert!(e.state.players[1].graveyard.contains(&artifact));

    // Universal Solvent can destroy a land; the artifact itself is sacrificed as a cost.
    let mut e = semantic::main_phase(65_031_002);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let solvent = cast_named(
        &mut e,
        0,
        "universal_solvent",
        ManaGift {
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    activate(&mut e, 0, solvent, 0, target_object(land));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert!(e.state.players[1].graveyard.contains(&land));
    assert_eq!(e.state.objects[&solvent].zone, Zone::Graveyard);
    assert!(e.state.players[0].graveyard.contains(&solvent));
}

#[test]
fn q65_destroy_spells_reject_nonpermanent_targets() {
    let mut e = semantic::main_phase(65_032_001);
    inject_card_into_hand(&mut e, 0, "vindicate");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "vindicate");
    let pool_before = pool(&e);
    let hand_before = e.state.players[0].hand.len();
    let command_index_before = e.state.command_index;
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .is_err(),
        "a player is not a permanent"
    );
    assert_eq!(pool(&e), pool_before, "no mana spent on the rejected cast");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "no card left hand"
    );
    assert_eq!(
        e.state.command_index, command_index_before,
        "the rejected cast commits no command"
    );
    assert!(e.state.stack.is_empty());
}
