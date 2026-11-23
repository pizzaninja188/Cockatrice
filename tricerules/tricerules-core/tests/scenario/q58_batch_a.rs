//! Actual-card scenarios for the queue-v58 batch A cohorts.
//!
//! Three stable typed shapes share one batch: an uncounterable spell with one effect, a
//! counterspell with one rider, and a destroy-target-enchantment spell with an optional rider.
//! Expectations are printed Oracle values. CR 701.6 counters a spell (and a spell that says it
//! can't be countered still resolves); CR 608.2b fizzles a spell whose only target left;
//! CR 701.25 performs the private surveil; CR 702.29a-b makes cycling and basic landcycling
//! hand-zone activated abilities; CR 702.34a-d governs flashback and its exile; CR 701.8
//! destroys; and CR 121.1 draws in printed order.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, CastMethod, CastSpell, ChoiceKind,
    RuledCommand,
};

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
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

fn hand_ability(e: &GameEngine, source: u32, ability_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            ..Default::default()
        })),
    }
}

fn flashback_cast(spell: u32, target: u32, generation: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets: target_object(target),
            ..Default::default()
        })),
    }
}

/// Pass priority until a resolution choice is parked, returning that parking batch.
fn pass_until_choice(e: &mut GameEngine) -> RuledEventBatch {
    for _ in 0..32 {
        let priority = e.state.priority_player_id();
        let batch = e
            .apply_scenario_command(priority, &pass())
            .expect("pass priority toward the resolution choice");
        if e.state.pending_resolution.is_some() || e.state.stack.is_empty() {
            return batch;
        }
    }
    panic!("resolution never reached a choice");
}

fn destroy_mana(id: &str) -> ManaGift {
    match id {
        "demystify" | "quiet_purity" => ManaGift {
            w: 1,
            ..Default::default()
        },
        _ => ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    }
}

fn cast_bear_and_pass(e: &mut GameEngine) -> u32 {
    let bear = cast_named(
        e,
        0,
        "grizzly_bears",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    e.apply_scenario_command(0, &pass())
        .expect("active player passes to the opponent");
    bear
}

fn seat_on_top(e: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(e, player, card_id))
        .collect();
    e.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        e.state.players[player].library.push_front(oid);
    }
    oids
}

fn counter_mana(id: &str) -> ManaGift {
    match id {
        "fervent_denial" => ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
        "neutralize" => ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
        "sinister_sabotage" => ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
        "traumatic_visions" => ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
        other => panic!("unknown counter {other}"),
    }
}

// ── 01 uncounterable spells ────────────────────────────────────────────────

#[test]
fn q58_uncounterable_canary_last_word() {
    let mut e = main1_engine(58_010_001, &[], &[]);
    let bear = cast_bear_and_pass(&mut e);
    // The opponent counters the Bear with Last Word, which cannot itself be countered.
    let last_word = cast_named(
        &mut e,
        1,
        "last_word",
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
        target_object(bear),
    );
    e.apply_scenario_command(1, &pass())
        .expect("Last Word's controller passes to the opponent");
    let counterspell = cast_named(
        &mut e,
        0,
        "counterspell",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        target_object(last_word),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&last_word].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&counterspell].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q58_uncounterable_dovins_veto_restricts_targets_and_resists_counters() {
    // A creature spell is not a legal target.
    let mut rejected = main1_engine(58_010_010, &[], &[]);
    let bear = cast_bear_and_pass(&mut rejected);
    inject_card_into_hand(&mut rejected, 1, "dovins_veto");
    give_mana(
        &mut rejected,
        1,
        ManaGift {
            w: 1,
            u: 1,
            ..Default::default()
        },
    );
    let veto_slot = hand_index_for_card(&rejected, 1, "dovins_veto");
    let hand_before = rejected.state.players[1].hand.len();
    assert!(
        rejected
            .apply_scenario_command(1, &cast_spell(veto_slot, target_object(bear)))
            .is_err(),
        "Dovin's Veto cannot target a creature spell"
    );
    assert_eq!(rejected.state.players[1].hand.len(), hand_before);
    assert_eq!(rejected.state.stack.len(), 1);

    // A noncreature spell is a legal target; the Veto itself cannot be countered.
    let mut e = main1_engine(58_010_011, &[], &[]);
    let bolt = cast_named(
        &mut e,
        0,
        "lightning_bolt",
        ManaGift {
            r: 1,
            ..Default::default()
        },
        target_player(1),
    );
    e.apply_scenario_command(0, &pass())
        .expect("active player passes to the opponent");
    let veto = cast_named(
        &mut e,
        1,
        "dovins_veto",
        ManaGift {
            w: 1,
            u: 1,
            ..Default::default()
        },
        target_object(bolt),
    );
    e.apply_scenario_command(1, &pass())
        .expect("Dovin's Veto controller passes to the opponent");
    let counterspell = cast_named(
        &mut e,
        0,
        "counterspell",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        target_object(veto),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&veto].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&counterspell].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 20, "the Bolt never resolved");
}

#[test]
fn q58_uncounterable_void_rend_and_wreak_havoc_follow_their_filters() {
    // Void Rend destroys a nonland permanent and rejects a land target.
    let mut creature_case = main1_engine(58_010_020, &[], &[]);
    let bear = inject_creature_on_battlefield(&mut creature_case, 1, "grizzly_bears");
    cast_named(
        &mut creature_case,
        0,
        "void_rend",
        ManaGift {
            w: 1,
            u: 1,
            b: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut creature_case);
    assert_eq!(creature_case.state.objects[&bear].zone, Zone::Graveyard);

    let mut land_case = main1_engine(58_010_021, &[], &[]);
    let forest = inject_permanent_on_battlefield(&mut land_case, 1, "forest");
    inject_card_into_hand(&mut land_case, 0, "void_rend");
    give_mana(
        &mut land_case,
        0,
        ManaGift {
            w: 1,
            u: 1,
            b: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&land_case, 0, "void_rend");
    assert!(
        land_case
            .apply_scenario_command(0, &cast_spell(slot, target_object(forest)))
            .is_err(),
        "Void Rend cannot target a land"
    );
    assert_eq!(
        land_case.state.objects[&forest].zone,
        Zone::Battlefield,
        "the rejected cast leaves the land alone"
    );

    // Wreak Havoc destroys an artifact or a land and rejects an enchantment.
    let mut artifact_case = main1_engine(58_010_022, &[], &[]);
    let artifact = inject_permanent_on_battlefield(&mut artifact_case, 1, "mind_stone");
    cast_named(
        &mut artifact_case,
        0,
        "wreak_havoc",
        ManaGift {
            r: 1,
            g: 1,
            c: 2,
            ..Default::default()
        },
        target_object(artifact),
    );
    resolve_entire_stack_two_player(&mut artifact_case);
    assert_eq!(artifact_case.state.objects[&artifact].zone, Zone::Graveyard);

    let mut land_target = main1_engine(58_010_023, &[], &[]);
    let land = inject_permanent_on_battlefield(&mut land_target, 1, "forest");
    cast_named(
        &mut land_target,
        0,
        "wreak_havoc",
        ManaGift {
            r: 1,
            g: 1,
            c: 2,
            ..Default::default()
        },
        target_object(land),
    );
    resolve_entire_stack_two_player(&mut land_target);
    assert_eq!(land_target.state.objects[&land].zone, Zone::Graveyard);

    let mut enchantment_case = main1_engine(58_010_024, &[], &[]);
    let enchantment = inject_permanent_on_battlefield(&mut enchantment_case, 1, "favorable_winds");
    inject_card_into_hand(&mut enchantment_case, 0, "wreak_havoc");
    give_mana(
        &mut enchantment_case,
        0,
        ManaGift {
            r: 1,
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&enchantment_case, 0, "wreak_havoc");
    assert!(
        enchantment_case
            .apply_scenario_command(0, &cast_spell(slot, target_object(enchantment)))
            .is_err(),
        "Wreak Havoc cannot target an enchantment"
    );
}

#[test]
fn q58_uncounterable_supreme_verdict_sweeps_through_a_counter_attempt() {
    let mut e = main1_engine(58_010_030, &[], &["favorable_winds"]);
    let own_bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing_bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "favorable_winds");
    let verdict = cast_named(
        &mut e,
        0,
        "supreme_verdict",
        ManaGift {
            w: 2,
            u: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    e.apply_scenario_command(0, &pass())
        .expect("active player passes to the opponent");
    let counterspell = cast_named(
        &mut e,
        1,
        "counterspell",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        target_object(verdict),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&own_bear].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing_bear].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&enchantment].zone,
        Zone::Battlefield,
        "noncreature permanents survive"
    );
    assert_eq!(e.state.objects[&counterspell].zone, Zone::Graveyard);
}

// ── 02 counter with a rider ────────────────────────────────────────────────

#[test]
fn q58_counter_riders_canary_sinister_sabotage() {
    let mut e = main1_engine(58_020_001, &[], &[]);
    let bear = cast_bear_and_pass(&mut e);
    let top = seat_on_top(&mut e, 1, &["grizzly_bears", "storm_crow"]);
    let sabotage = cast_named(
        &mut e,
        1,
        "sinister_sabotage",
        counter_mana("sinister_sabotage"),
        target_object(bear),
    );
    let parked = pass_until_choice(&mut e);
    let choice = find_resolution_choice(&parked).expect("private surveil choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(
        choice.candidate_object_ids,
        vec![top[0]],
        "surveil 1 looks at exactly the top card"
    );
    assert!(choice.public_reveal.is_none(), "surveil is private");
    e.apply_scenario_command(1, &submit_resolution_choice(vec![top[0]]))
        .expect("put the looked-at card into the graveyard");
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[1].library.front(),
        Some(&top[1]),
        "the other library card stays on top"
    );
    assert_eq!(e.state.objects[&sabotage].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q58_counter_riders_counter_the_target_and_apply_their_printed_rider() {
    for (index, id) in [
        "fervent_denial",
        "neutralize",
        "sinister_sabotage",
        "traumatic_visions",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(58_020_100 + index as u64, &[], &[]);
        let bear = cast_bear_and_pass(&mut e);
        let counter = cast_named(&mut e, 1, id, counter_mana(id), target_object(bear));
        if id == "sinister_sabotage" {
            seat_on_top(&mut e, 1, &["grizzly_bears"]);
            let parked = pass_until_choice(&mut e);
            let choice = find_resolution_choice(&parked).expect("surveil choice");
            let looked_at = choice.candidate_object_ids.clone();
            e.apply_scenario_command(1, &submit_resolution_choice(looked_at))
                .expect("surveil");
        } else {
            resolve_entire_stack_two_player(&mut e);
        }
        assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard, "{id}");
        assert_eq!(e.state.objects[&counter].zone, Zone::Graveyard, "{id}");
        assert!(e.state.stack.is_empty(), "{id}");
        assert_eq!(e.state.players[0].life, 20, "{id}: no life change");
        assert_eq!(e.state.players[1].life, 20, "{id}: no life change");
    }
}

#[test]
fn q58_counter_riders_reject_a_non_spell_target() {
    for (index, id) in [
        "fervent_denial",
        "neutralize",
        "sinister_sabotage",
        "traumatic_visions",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(58_020_200 + index as u64, &[], &[]);
        e.apply_scenario_command(0, &pass())
            .expect("pass to the opponent");
        inject_card_into_hand(&mut e, 1, id);
        give_mana(&mut e, 1, counter_mana(id));
        let slot = hand_index_for_card(&e, 1, id);
        let hand_before = e.state.players[1].hand.len();
        assert!(
            e.apply_scenario_command(1, &cast_spell(slot, target_player(0)))
                .is_err(),
            "{id}: a player is not a spell target"
        );
        assert_eq!(e.state.players[1].hand.len(), hand_before, "{id}");
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

#[test]
fn q58_counter_riders_fizzle_when_the_target_spell_leaves_the_stack() {
    for (index, id) in [
        "fervent_denial",
        "neutralize",
        "sinister_sabotage",
        "traumatic_visions",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(58_020_300 + index as u64, &[], &[]);
        let bear = cast_bear_and_pass(&mut e);
        cast_named(&mut e, 1, id, counter_mana(id), target_object(bear));
        // Remove the target spell from the stack before the counter resolves.
        e.state.stack.retain(|entry| entry.id != bear);
        e.state.objects.get_mut(&bear).expect("bear spell").zone = Zone::Graveyard;
        e.state.players[0].graveyard.push(bear);
        *e.state.zone_change_generation.entry(bear).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert!(e.state.stack.is_empty(), "{id}");
        assert!(
            e.state.pending_resolution.is_none(),
            "{id}: no surveil rider for a fizzled spell"
        );
        assert_eq!(e.state.players[0].life, 20, "{id}");
        assert_eq!(e.state.players[1].life, 20, "{id}");
    }
}

#[test]
fn q58_counter_riders_cycling_discards_self_and_draws() {
    let mut e = main1_engine(58_020_400, &[], &[]);
    let source = inject_card_into_hand(&mut e, 0, "neutralize");
    let hand_before = e.state.players[0].hand.len();
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let activation = hand_ability(&e, source, 0);
    semantic::accepted(&mut e, 0, &activation);
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].hand.len(), hand_before);
    assert_eq!(e.state.players[0].library.len(), library_before - 1);
}

#[test]
fn q58_counter_riders_flashback_casts_and_exiles() {
    let mut e = main1_engine(58_020_500, &[], &[]);
    let first_bear = cast_bear_and_pass(&mut e);
    let denial = cast_named(
        &mut e,
        1,
        "fervent_denial",
        counter_mana("fervent_denial"),
        target_object(first_bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&first_bear].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&denial].zone, Zone::Graveyard);

    let second_bear = cast_bear_and_pass(&mut e);
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            c: 5,
            ..Default::default()
        },
    );
    let generation = e.state.zone_change_generation[&denial];
    semantic::accepted(&mut e, 1, &flashback_cast(denial, second_bear, generation));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&second_bear].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&denial].zone,
        Zone::Exile,
        "CR 702.34a exiles a spell cast with flashback"
    );
    let pool = e.state.players[1].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "the printed flashback cost is paid exactly"
    );
}

#[test]
fn q58_counter_riders_basic_landcycling_searches_a_basic_land() {
    let decks = Some(vec![
        vec!["grizzly_bears".to_string(); 20],
        vec!["forest".to_string(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        58_020_600,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let source = inject_card_into_hand(&mut e, 0, "traumatic_visions");
    let mountain = inject_library_card(&mut e, 0, "mountain");
    let forest = inject_library_card(&mut e, 0, "forest");
    e.apply_scenario_command(0, &hand_ability(&e, source, 0))
        .expect_err("the full cost is checked before any discard");
    assert_eq!(e.state.objects[&source].zone, Zone::Hand);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let activation = hand_ability(&e, source, 0);
    semantic::accepted(&mut e, 0, &activation);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    e.apply_scenario_command(0, &pass())
        .expect("controller passes");
    let search = e
        .apply_scenario_command(1, &pass())
        .expect("basic landcycling resolves to a private search");
    let choice = find_resolution_choice(&search).expect("private library search");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, vec![mountain, forest]);
    let completed = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![mountain]))
        .expect("choose the basic land");
    assert_eq!(e.state.objects[&mountain].zone, Zone::Hand);
    assert!(completed.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
            if log.text == "P0 shuffles their library."
    )));
}

// ── 03 destroy target enchantment ─────────────────────────────────────────

#[test]
fn q58_destroy_enchantment_canary_demystify() {
    let mut e = main1_engine(58_030_001, &[], &["favorable_winds"]);
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "favorable_winds");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "demystify",
        ManaGift {
            w: 1,
            ..Default::default()
        },
        target_object(enchantment),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&bear].zone, Zone::Battlefield);

    let mut rejected = main1_engine(58_030_002, &[], &[]);
    let creature = inject_creature_on_battlefield(&mut rejected, 1, "grizzly_bears");
    inject_card_into_hand(&mut rejected, 0, "demystify");
    give_mana(
        &mut rejected,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&rejected, 0, "demystify");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .is_err(),
        "Demystify cannot target a creature"
    );
}

#[test]
fn q58_destroy_enchantment_cards_destroy_only_enchantments() {
    for (index, id) in [
        "aura_blast",
        "clear",
        "demystify",
        "quiet_purity",
        "ray_of_revelation",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(58_030_100 + index as u64, &[], &[]);
        let enchantment = inject_permanent_on_battlefield(&mut e, 1, "favorable_winds");
        let hand_before = e.state.players[0].hand.len();
        cast_named(&mut e, 0, id, destroy_mana(id), target_object(enchantment));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard, "{id}");
        let expected_hand = if id == "aura_blast" {
            hand_before + 1
        } else {
            hand_before
        };
        assert_eq!(
            e.state.players[0].hand.len(),
            expected_hand,
            "{id}: only Aura Blast draws"
        );

        let mut rejected = main1_engine(58_030_200 + index as u64, &[], &[]);
        let creature = inject_creature_on_battlefield(&mut rejected, 1, "grizzly_bears");
        inject_card_into_hand(&mut rejected, 0, id);
        give_mana(&mut rejected, 0, destroy_mana(id));
        let slot = hand_index_for_card(&rejected, 0, id);
        assert!(
            rejected
                .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{id} cannot target a creature"
        );
    }
}

#[test]
fn q58_destroy_enchantment_riders_draw_cycle_and_flashback() {
    // Aura Blast draws its card.
    let mut draw = main1_engine(58_030_300, &[], &[]);
    let enchantment = inject_permanent_on_battlefield(&mut draw, 1, "favorable_winds");
    let hand_before = draw.state.players[0].hand.len();
    cast_named(
        &mut draw,
        0,
        "aura_blast",
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
        target_object(enchantment),
    );
    resolve_entire_stack_two_player(&mut draw);
    assert_eq!(draw.state.players[0].hand.len(), hand_before + 1);

    // Clear cycles from hand.
    let mut cycle = main1_engine(58_030_301, &[], &[]);
    let clear = inject_card_into_hand(&mut cycle, 0, "clear");
    let hand_before = cycle.state.players[0].hand.len();
    give_mana(
        &mut cycle,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let activation = hand_ability(&cycle, clear, 0);
    semantic::accepted(&mut cycle, 0, &activation);
    pass_both_players(&mut cycle);
    assert_eq!(cycle.state.objects[&clear].zone, Zone::Graveyard);
    assert_eq!(cycle.state.players[0].hand.len(), hand_before);

    // Ray of Revelation destroys one enchantment, then flashbacks to destroy another and exiles.
    let mut flashback = main1_engine(58_030_302, &[], &[]);
    let first = inject_permanent_on_battlefield(&mut flashback, 1, "favorable_winds");
    let ray = cast_named(
        &mut flashback,
        0,
        "ray_of_revelation",
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
        target_object(first),
    );
    resolve_entire_stack_two_player(&mut flashback);
    assert_eq!(flashback.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(flashback.state.objects[&ray].zone, Zone::Graveyard);
    let second = inject_permanent_on_battlefield(&mut flashback, 1, "favorable_winds");
    give_mana(
        &mut flashback,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let generation = flashback.state.zone_change_generation[&ray];
    semantic::accepted(&mut flashback, 0, &flashback_cast(ray, second, generation));
    resolve_entire_stack_two_player(&mut flashback);
    assert_eq!(flashback.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(flashback.state.objects[&ray].zone, Zone::Exile);
}

#[test]
fn q58_destroy_enchantment_stale_target_fizzles_without_the_rider() {
    let mut e = main1_engine(58_030_400, &[], &[]);
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "favorable_winds");
    let hand_before = e.state.players[0].hand.len();
    cast_named(
        &mut e,
        0,
        "aura_blast",
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
        target_object(enchantment),
    );
    // The target leaves before Aura Blast resolves.
    e.state.players[1]
        .battlefield
        .retain(|candidate| *candidate != enchantment);
    e.state.players[1].graveyard.push(enchantment);
    e.state.objects.get_mut(&enchantment).expect("object").zone = Zone::Graveyard;
    *e.state
        .zone_change_generation
        .entry(enchantment)
        .or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the draw rider does not happen when the only target is illegal"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q58_uncounterable_removal_rechecks_a_stale_target() {
    fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
        e.state.players[player]
            .battlefield
            .retain(|candidate| *candidate != object);
        e.state.players[player].graveyard.push(object);
        e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(object).or_insert(0) += 1;
    }

    fn expect_fizzle(e: &mut GameEngine, spell: &str) {
        let first = e.state.priority_player_id();
        e.apply_scenario_command(first, &pass())
            .expect("first player passes to resolve");
        let second = e.state.priority_player_id();
        let batch = e
            .apply_scenario_command(second, &pass())
            .expect("second player passes to resolve");
        assert!(
            batch.events.iter().any(|event| matches!(
                &event.ev,
                Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
                    if log.text.contains(spell) && log.text.contains("fizzles (no legal targets)")
            )),
            "{spell} must fizzle rather than resolve: {:?}",
            batch.events
        );
    }

    // Void Rend: the sole target leaves before resolution, so the spell fizzles.
    let mut void_rend = main1_engine(58_010_040, &[], &[]);
    let creature = inject_creature_on_battlefield(&mut void_rend, 1, "grizzly_bears");
    let rend = cast_named(
        &mut void_rend,
        0,
        "void_rend",
        ManaGift {
            w: 1,
            u: 1,
            b: 1,
            ..Default::default()
        },
        target_object(creature),
    );
    move_to_graveyard(&mut void_rend, 1, creature);
    expect_fizzle(&mut void_rend, "Void Rend");
    assert_eq!(void_rend.state.objects[&rend].zone, Zone::Graveyard);
    assert_eq!(
        void_rend.state.objects[&creature].zone,
        Zone::Graveyard,
        "the already-departed target is not destroyed again"
    );
    assert!(void_rend.state.stack.is_empty());

    // Wreak Havoc: the same stale-target recheck for an artifact target.
    let mut wreak = main1_engine(58_010_041, &[], &[]);
    let artifact = inject_permanent_on_battlefield(&mut wreak, 1, "mind_stone");
    let havoc = cast_named(
        &mut wreak,
        0,
        "wreak_havoc",
        ManaGift {
            r: 1,
            g: 1,
            c: 2,
            ..Default::default()
        },
        target_object(artifact),
    );
    move_to_graveyard(&mut wreak, 1, artifact);
    expect_fizzle(&mut wreak, "Wreak Havoc");
    assert_eq!(wreak.state.objects[&havoc].zone, Zone::Graveyard);
    assert_eq!(wreak.state.objects[&artifact].zone, Zone::Graveyard);
    assert!(wreak.state.stack.is_empty());
}

#[test]
fn q58_counter_riders_surveil_applies_to_an_uncounterable_target() {
    let mut e = main1_engine(58_020_700, &[], &[]);
    let verdict = cast_named(
        &mut e,
        0,
        "supreme_verdict",
        ManaGift {
            w: 2,
            u: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    e.apply_scenario_command(0, &pass())
        .expect("active player passes to the opponent");
    let top = seat_on_top(&mut e, 1, &["grizzly_bears"]);
    let sabotage = cast_named(
        &mut e,
        1,
        "sinister_sabotage",
        counter_mana("sinister_sabotage"),
        target_object(verdict),
    );
    let parked = pass_until_choice(&mut e);
    let choice =
        find_resolution_choice(&parked).expect("surveil choice for an uncounterable target");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.candidate_object_ids, vec![top[0]]);
    e.apply_scenario_command(1, &submit_resolution_choice(vec![top[0]]))
        .expect("the surveil rider still happens");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&verdict].zone,
        Zone::Graveyard,
        "the uncounterable Verdict resolved"
    );
    assert_eq!(e.state.objects[&sabotage].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&top[0]].zone,
        Zone::Graveyard,
        "the private surveil still applied"
    );
    assert!(e.state.stack.is_empty());
}
