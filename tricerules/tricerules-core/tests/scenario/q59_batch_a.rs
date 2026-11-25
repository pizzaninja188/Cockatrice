//! Actual-card scenarios for the queue-v59 batch A cohorts.
//!
//! Three stable typed shapes share one batch: a reveal-hand discard with an optional rider, a
//! threaten spell with one rider, and a filtered counterspell that draws. Expectations are
//! printed Oracle values. CR 701.9 discards, 400.2 (a hand is a hidden zone) and 701.20 (reveal),
//! 121.1 draws, 701.25 surveils, 611.2a/613.1b change control, 701.26 untaps, 702.10 grants
//! haste, 702.19 grants trample, 701.22 scries, 701.6 counters, and 608.2b/c fizzle or resume in
//! printed order.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
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

fn clear_hand_into_library(e: &mut GameEngine, player: usize) {
    let cleared = std::mem::take(&mut e.state.players[player].hand);
    e.state.players[player].library.extend(cleared);
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

// ── 01 reveal-hand discard with an optional rider ─────────────────────────

#[test]
fn q59_hand_discard_canary_dark_inquiry() {
    let mut e = main1_engine(59_010_001, &[], &[]);
    clear_hand_into_library(&mut e, 1);
    let bear = inject_card_into_hand(&mut e, 1, "grizzly_bears");
    let land = inject_card_into_hand(&mut e, 1, "forest");
    let inquiry = cast_named(
        &mut e,
        0,
        "dark_inquiry",
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
        target_player(1),
    );
    e.apply_scenario_command(0, &pass())
        .expect("active player passes to the opponent");
    let parked = e
        .apply_scenario_command(1, &pass())
        .expect("Dark Inquiry parks for the discard choice");
    let choice = find_resolution_choice(&parked).expect("public hand choice");
    assert_eq!(
        choice.public_reveal.as_ref().unwrap().zone_owner_player_id,
        1
    );
    assert!(choice.candidate_object_ids.contains(&bear));
    assert!(choice.candidate_object_ids.contains(&land));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![bear]))
        .expect("choose the nonland card");
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&land].zone, Zone::Hand);
    assert_eq!(e.state.objects[&inquiry].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].hand, vec![land]);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q59_hand_discard_target_scope_and_empty_hand_draw() {
    // The three opponent-only discard spells reject their own controller as a target.
    for (card, mana) in [
        (
            "dark_inquiry",
            ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            "brainbite",
            ManaGift {
                u: 1,
                b: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            "thought_erasure",
            ManaGift {
                u: 1,
                b: 1,
                ..Default::default()
            },
        ),
    ] {
        let mut rejected = main1_engine(59_010_010, &[], &[]);
        inject_card_into_hand(&mut rejected, 0, card);
        give_mana(&mut rejected, 0, mana);
        let slot = hand_index_for_card(&rejected, 0, card);
        let hand_before = rejected.state.players[0].hand.len();
        assert!(
            rejected
                .apply_scenario_command(0, &cast_spell(slot, target_player(0)))
                .is_err(),
            "{card} cannot target its controller"
        );
        assert_eq!(rejected.state.players[0].hand.len(), hand_before);
        assert!(rejected.state.stack.is_empty());
    }

    // Distress may target any player, including its controller.
    let mut self_case = main1_engine(59_010_011, &[], &[]);
    clear_hand_into_library(&mut self_case, 0);
    let own_bear = inject_card_into_hand(&mut self_case, 0, "grizzly_bears");
    let own_land = inject_card_into_hand(&mut self_case, 0, "forest");
    cast_named(
        &mut self_case,
        0,
        "distress",
        ManaGift {
            b: 2,
            ..Default::default()
        },
        target_player(0),
    );
    let parked_batch = pass_both_players_with_batch(&mut self_case);
    let choice = find_resolution_choice(&parked_batch).expect("Distress parks");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(
        choice
            .public_reveal
            .as_ref()
            .expect("Distress reveals")
            .zone_owner_player_id,
        0,
        "targeting yourself still reveals your hand"
    );
    self_case
        .apply_scenario_command(0, &submit_resolution_choice(vec![own_bear]))
        .expect("choose own nonland card");
    assert_eq!(self_case.state.objects[&own_bear].zone, Zone::Graveyard);
    assert_eq!(self_case.state.objects[&own_land].zone, Zone::Hand);

    // Brainbite with a nonempty hand parks for the discard, then draws.
    let mut nonempty = main1_engine(59_010_013, &[], &[]);
    clear_hand_into_library(&mut nonempty, 1);
    let other = inject_card_into_hand(&mut nonempty, 1, "storm_crow");
    let library_before = nonempty.state.players[0].library.len();
    let brainbite = cast_named(
        &mut nonempty,
        0,
        "brainbite",
        ManaGift {
            u: 1,
            b: 1,
            c: 2,
            ..Default::default()
        },
        target_player(1),
    );
    nonempty
        .apply_scenario_command(0, &pass())
        .expect("p0 pass");
    let parked = nonempty
        .apply_scenario_command(1, &pass())
        .expect("Brainbite parks for the discard");
    assert_eq!(nonempty.state.players[0].library.len(), library_before);
    find_resolution_choice(&parked).expect("discard choice");
    nonempty
        .apply_scenario_command(0, &submit_resolution_choice(vec![other]))
        .expect("discard the only card");
    assert_eq!(nonempty.state.objects[&other].zone, Zone::Graveyard);
    assert_eq!(nonempty.state.players[0].library.len(), library_before - 1);
    assert_eq!(nonempty.state.objects[&brainbite].zone, Zone::Graveyard);

    // Brainbite against an empty hand offers no choice but still draws.
    let mut empty = main1_engine(59_010_012, &[], &[]);
    clear_hand_into_library(&mut empty, 1);
    let library_before = empty.state.players[0].library.len();
    let brainbite = cast_named(
        &mut empty,
        0,
        "brainbite",
        ManaGift {
            u: 1,
            b: 1,
            c: 2,
            ..Default::default()
        },
        target_player(1),
    );
    pass_both_players(&mut empty);
    assert!(empty.state.pending_resolution.is_none());
    assert_eq!(empty.state.players[0].library.len(), library_before - 1);
    assert_eq!(empty.state.objects[&brainbite].zone, Zone::Graveyard);
}

#[test]
fn q59_hand_discard_thought_erasure_surveils_after_discard() {
    let mut e = main1_engine(59_010_020, &[], &[]);
    clear_hand_into_library(&mut e, 1);
    let bear = inject_card_into_hand(&mut e, 1, "grizzly_bears");
    inject_card_into_hand(&mut e, 1, "forest");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears"]);
    cast_named(
        &mut e,
        0,
        "thought_erasure",
        ManaGift {
            u: 1,
            b: 1,
            ..Default::default()
        },
        target_player(1),
    );
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    let parked = e
        .apply_scenario_command(1, &pass())
        .expect("Thought Erasure parks for the discard");
    find_resolution_choice(&parked).expect("discard choice");
    let resumed = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![bear]))
        .expect("resume after the discard");
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    let surveil = find_resolution_choice(&resumed).expect("surveil parks after the discard");
    assert_eq!(surveil.candidate_object_ids, vec![top[0]]);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put the looked-at card into the graveyard");
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Graveyard);

    // An empty hand skips the discard but still surveils (2018-10-05 ruling).
    let mut empty = main1_engine(59_010_021, &[], &[]);
    clear_hand_into_library(&mut empty, 1);
    let kept = seat_on_top(&mut empty, 0, &["grizzly_bears"]);
    cast_named(
        &mut empty,
        0,
        "thought_erasure",
        ManaGift {
            u: 1,
            b: 1,
            ..Default::default()
        },
        target_player(1),
    );
    pass_both_players(&mut empty);
    assert!(empty.state.pending_resolution.is_some());
    empty
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("keep the card on top");
    assert_eq!(
        empty.state.players[0].library.front().copied(),
        Some(kept[0])
    );
}

// ── 02 threaten with one rider ─────────────────────────────────────────────

#[test]
fn q59_threaten_canary_limits_of_solidarity() {
    let mut e = main1_engine(59_020_001, &[], &[]);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state.objects.get_mut(&bear).expect("bear").tapped = true;
    cast_named(
        &mut e,
        0,
        "limits_of_solidarity",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].controller, 0);
    assert!(!e.state.objects[&bear].tapped);
    assert!(e.effective_has_keyword(bear, tricerules_cards::Keyword::Haste));
    let pool = e.state.players[0].mana_pool;
    assert_eq!(
        (pool.red, pool.colorless, pool.green, pool.blue),
        (0, 0, 0, 0),
        "the printed cost is paid exactly"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(e.state.objects[&bear].controller, 1);
    assert!(!e.effective_has_keyword(bear, tricerules_cards::Keyword::Haste));
}

#[test]
fn q59_threaten_limits_of_solidarity_cycles_from_hand() {
    let mut e = main1_engine(59_020_002, &[], &[]);
    let source = inject_card_into_hand(&mut e, 0, "limits_of_solidarity");
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
fn q59_threaten_portent_scrys_last_and_fizzles_on_stale_target() {
    let mut e = main1_engine(59_020_010, &[], &[]);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears"]);
    cast_named(
        &mut e,
        0,
        "portent_of_betrayal",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
        target_object(bear),
    );
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    let parked = e
        .apply_scenario_command(1, &pass())
        .expect("Portent parks for the scry");
    assert_eq!(e.state.objects[&bear].controller, 0);
    assert!(e.effective_has_keyword(bear, tricerules_cards::Keyword::Haste));
    let scry = find_resolution_choice(&parked).expect("scry parks after the control change");
    assert_eq!(scry.candidate_object_ids, vec![top[0]]);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("put the scried card on the bottom");
    assert_eq!(e.state.players[0].library.back().copied(), Some(top[0]));

    // A sole target that dies before resolution means no control change and no scry.
    let mut stale = main1_engine(59_020_011, &[], &[]);
    let doomed = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    let library_before = stale.state.players[0].library.clone();
    cast_named(
        &mut stale,
        0,
        "portent_of_betrayal",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
        target_object(doomed),
    );
    stale.apply_scenario_command(0, &pass()).expect("p0 pass");
    inject_card_into_hand(&mut stale, 1, "doom_blade");
    give_mana(
        &mut stale,
        1,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&stale, 1, "doom_blade");
    stale
        .apply_scenario_command(1, &cast_spell(slot, target_object(doomed)))
        .expect("destroy the only target in response");
    resolve_entire_stack_two_player(&mut stale);
    assert_eq!(stale.state.objects[&doomed].zone, Zone::Graveyard);
    assert!(stale.state.pending_resolution.is_none());
    assert_eq!(stale.state.players[0].library, library_before);
}

#[test]
fn q59_threaten_traitorous_blood_grants_trample_and_haste() {
    let mut e = main1_engine(59_020_020, &[], &[]);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state.objects.get_mut(&bear).expect("bear").tapped = true;
    cast_named(
        &mut e,
        0,
        "traitorous_blood",
        ManaGift {
            r: 2,
            c: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bear].controller, 0);
    assert!(!e.state.objects[&bear].tapped);
    assert!(e.effective_has_keyword(bear, tricerules_cards::Keyword::Trample));
    assert!(e.effective_has_keyword(bear, tricerules_cards::Keyword::Haste));
    end_active_turn(&mut e, 0);
    assert_eq!(e.state.objects[&bear].controller, 1);
    assert!(!e.effective_has_keyword(bear, tricerules_cards::Keyword::Trample));

    // The printed ruling allows a tapped creature its controller already controls.
    let mut own = main1_engine(59_020_021, &[], &[]);
    let own_bear = inject_creature_on_battlefield(&mut own, 0, "grizzly_bears");
    own.state.objects.get_mut(&own_bear).expect("bear").tapped = true;
    cast_named(
        &mut own,
        0,
        "traitorous_blood",
        ManaGift {
            r: 2,
            c: 1,
            ..Default::default()
        },
        target_object(own_bear),
    );
    resolve_entire_stack_two_player(&mut own);
    assert_eq!(own.state.objects[&own_bear].controller, 0);
    assert!(!own.state.objects[&own_bear].tapped);
}

// ── 03 filtered counterspell with a draw rider ─────────────────────────────

#[test]
fn q59_counter_filters_canary_scatter_arc() {
    let mut e = main1_engine(59_030_001, &[], &[]);
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
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    let library_before = e.state.players[1].library.len();
    let arc = cast_named(
        &mut e,
        1,
        "scatter_arc",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_object(bolt),
    );
    e.apply_scenario_command(1, &pass()).expect("p1 pass");
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&arc].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 20, "the Bolt never resolved");
    assert_eq!(e.state.players[1].library.len(), library_before - 1);

    // A creature spell is not a noncreature spell.
    let mut rejected = main1_engine(59_030_002, &[], &[]);
    let bear = cast_named(
        &mut rejected,
        0,
        "grizzly_bears",
        ManaGift {
            g: 2,
            ..Default::default()
        },
        vec![],
    );
    rejected
        .apply_scenario_command(0, &pass())
        .expect("p0 pass");
    inject_card_into_hand(&mut rejected, 1, "scatter_arc");
    give_mana(
        &mut rejected,
        1,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&rejected, 1, "scatter_arc");
    assert!(
        rejected
            .apply_scenario_command(1, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "Scatter Arc cannot target a creature spell"
    );
}

#[test]
fn q59_counter_filters_halt_order_targets_only_artifacts() {
    let mut e = main1_engine(59_030_010, &[], &[]);
    let stone = cast_named(
        &mut e,
        0,
        "mind_stone",
        ManaGift {
            c: 3,
            ..Default::default()
        },
        vec![],
    );
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    let library_before = e.state.players[1].library.len();
    cast_named(
        &mut e,
        1,
        "halt_order",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        target_object(stone),
    );
    e.apply_scenario_command(1, &pass()).expect("p1 pass");
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    assert_eq!(e.state.objects[&stone].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].library.len(), library_before - 1);
    assert!(e.state.stack.is_empty());

    // A creature spell is not an artifact spell.
    let mut rejected = main1_engine(59_030_011, &[], &[]);
    let bear = cast_named(
        &mut rejected,
        0,
        "grizzly_bears",
        ManaGift {
            g: 2,
            ..Default::default()
        },
        vec![],
    );
    rejected
        .apply_scenario_command(0, &pass())
        .expect("p0 pass");
    inject_card_into_hand(&mut rejected, 1, "halt_order");
    give_mana(
        &mut rejected,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&rejected, 1, "halt_order");
    assert!(
        rejected
            .apply_scenario_command(1, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "Halt Order cannot target a creature spell"
    );
    assert_eq!(rejected.state.stack.len(), 1);
}

#[test]
fn q59_counter_filters_deny_entry_loots() {
    let mut e = main1_engine(59_030_020, &[], &[]);
    let bear = cast_named(
        &mut e,
        0,
        "grizzly_bears",
        ManaGift {
            g: 2,
            ..Default::default()
        },
        vec![],
    );
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    let library_before = e.state.players[1].library.len();
    cast_named(
        &mut e,
        1,
        "deny_entry",
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
        target_object(bear),
    );
    e.apply_scenario_command(1, &pass()).expect("p1 pass");
    let parked = e
        .apply_scenario_command(0, &pass())
        .expect("Deny Entry resolves to the loot");
    assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].library.len(), library_before - 1);
    let choice = find_resolution_choice(&parked).expect("loot parks for the discard");
    assert_eq!((choice.min, choice.max), (1, 1));
    let hand_before_discard = e.state.players[1].hand.len();
    let discarded = e.state.players[1].hand[0];
    e.apply_scenario_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("discard one card");
    assert_eq!(
        e.state.players[1].hand.len(),
        hand_before_discard - 1,
        "draw one, then discard one"
    );
    assert_eq!(e.state.objects[&discarded].zone, Zone::Graveyard);

    // An artifact spell is not a creature spell.
    let mut rejected = main1_engine(59_030_021, &[], &[]);
    let stone = cast_named(
        &mut rejected,
        0,
        "mind_stone",
        ManaGift {
            c: 3,
            ..Default::default()
        },
        vec![],
    );
    rejected
        .apply_scenario_command(0, &pass())
        .expect("p0 pass");
    inject_card_into_hand(&mut rejected, 1, "deny_entry");
    give_mana(
        &mut rejected,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&rejected, 1, "deny_entry");
    assert!(
        rejected
            .apply_scenario_command(1, &cast_spell(slot, target_object(stone)))
            .is_err(),
        "Deny Entry cannot target an artifact spell"
    );
}

#[test]
fn q59_counter_filters_stale_target_fizzles_without_drawing() {
    let mut e = main1_engine(59_030_030, &[], &[]);
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
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    let arc = cast_named(
        &mut e,
        1,
        "scatter_arc",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_object(bolt),
    );
    e.apply_scenario_command(1, &pass()).expect("p1 pass");
    // P0 counters the Bolt itself, removing Scatter Arc's only target.
    let counter = cast_named(
        &mut e,
        0,
        "counterspell",
        ManaGift {
            u: 2,
            ..Default::default()
        },
        target_object(bolt),
    );
    let library_before = e.state.players[1].library.clone();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&counter].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&arc].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[1].library, library_before,
        "the fizzled Scatter Arc never draws"
    );
}

#[test]
fn q59_counter_filters_uncounterable_target_still_draws() {
    let mut e = main1_engine(59_030_040, &[], &[]);
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
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    let last_word = cast_named(
        &mut e,
        1,
        "last_word",
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
        target_object(bolt),
    );
    e.apply_scenario_command(1, &pass()).expect("p1 pass");
    let library_before = e.state.players[0].library.len();
    let arc = cast_named(
        &mut e,
        0,
        "scatter_arc",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_object(last_word),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&arc].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 1,
        "the rider still draws when the target cannot be countered"
    );
    assert_eq!(
        e.state.objects[&last_word].zone,
        Zone::Graveyard,
        "Last Word resolved and countered the Bolt"
    );
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q59_hand_discard_thought_erasure_fizzles_without_surveil_when_target_leaves() {
    let decks = Some(vec![
        deck_with("swamp", &["thought_erasure"]),
        vec!["forest".into(); 20],
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        59_010_030,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("three-player engine");
    advance_to_main1_from_game_start(&mut e);
    let library_before = e.state.players[0].library.clone();
    cast_named(
        &mut e,
        0,
        "thought_erasure",
        ManaGift {
            u: 1,
            b: 1,
            ..Default::default()
        },
        target_player(2),
    );
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    e.apply_scenario_command(1, &pass()).expect("p1 pass");
    e.apply_scenario_command(2, &concede())
        .expect("the targeted player concedes with the spell on the stack");
    for _ in 0..12 {
        if e.state.stack.is_empty() {
            break;
        }
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("remaining players pass");
    }
    assert!(e.state.stack.is_empty());
    assert!(
        e.state.pending_resolution.is_none(),
        "the fizzled Thought Erasure never surveils"
    );
    assert_eq!(
        e.state.players[0].library, library_before,
        "the controller's library is untouched"
    );
}
