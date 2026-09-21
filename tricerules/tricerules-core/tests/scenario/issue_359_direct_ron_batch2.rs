//! Issue #359 (Spree cohort) and #338 (Pyrrhic Strike) — command-boundary scenarios for the
//! reviewed direct-RON batch.
//!
//! Three Steps Ahead, Trash the Town, Insatiable Avarice and Jailbreak Scheme print
//! `Spree (Choose one or more additional costs.)`; Pyrrhic Strike prints an optional blight that
//! upgrades "choose one" to "choose both". Exact Scryfall records and `rulings_uri` were fetched
//! 2026-09-21. Every expectation is the reviewed printed Oracle behavior.
//!
//! Governing CR concepts: 702.171a-e (spree), 601.2b/f-h (announced additional costs),
//! 700.2a/c/e (mode announcement, printed resolution order, linked choices), 115.1/608.2b
//! (targeting and revalidation), 701.7 (destroy), 701.30/118.8 (blight), 121.1 (draw),
//! 122.1 (counters), 509.1b/611.3 (combat restrictions), 611.2c (granted triggered abilities),
//! 400.7 (new-object identity for token copies), and 701.18/608.2h (search and owner placement).

use super::helpers::*;
use tricerules_cards::primitives::ContinuousEffectKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, CastCostGroupSelection, CastMethod, CastSpell, ResolutionChoiceDecision,
    SelectedSpellMode,
};

fn spree_option(option_index: u32) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        ..Default::default()
    }
}

fn blight_option(engine: &GameEngine, object_id: u32) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index: 0,
        selected_object: Some(
            tricerules_proto::ruled::v1::cast_cost_group_selection::SelectedObject::PermanentId(
                object_id,
            ),
        ),
        expected_zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
        ..Default::default()
    }
}

fn cast_spree(
    hand_card_index: usize,
    modes: Vec<(u32, Vec<TargetRef>)>,
    options: Vec<u32>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(hand_card_index)),
            selected_modes: modes
                .into_iter()
                .map(|(mode_index, targets)| SelectedSpellMode {
                    mode_index,
                    targets,
                })
                .collect(),
            cast_cost_group_selections: options.into_iter().map(spree_option).collect(),
            ..Default::default()
        })),
    }
}

fn cast_spree_blight(
    hand_card_index: usize,
    modes: Vec<(u32, Vec<TargetRef>)>,
    engine: &GameEngine,
    blight_object: u32,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(hand_card_index)),
            selected_modes: modes
                .into_iter()
                .map(|(mode_index, targets)| SelectedSpellMode {
                    mode_index,
                    targets,
                })
                .collect(),
            cast_cost_group_selections: vec![blight_option(engine, blight_object)],
            ..Default::default()
        })),
    }
}

fn target_stack(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

fn submit_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(
            tricerules_proto::ruled::v1::SubmitResolutionChoice {
                chosen_object_ids: Vec::new(),
                selected_branch_index: index,
                decision: ResolutionChoiceDecision::SelectBranch as i32,
                ..Default::default()
            },
        )),
    }
}

fn engine(seed: u64, p0_extra: &[&str], p1_extra: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", p0_extra),
        deck_with("forest", p1_extra),
    ]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

#[test]
fn issue_359_three_steps_ahead_modes_and_linked_costs() {
    // Counter mode: counter a spell on the stack, paying only the linked {1}{U}.
    let mut e = engine(359_501, &["three_steps_ahead", "grizzly_bears"], &[]);
    ensure_card_in_hand(&mut e, 0, "three_steps_ahead");
    ensure_card_in_hand(&mut e, 0, "grizzly_bears");
    let bear_slot = hand_index_for_card(&e, 0, "grizzly_bears");
    let bear_card = e.state.players[0].hand[bear_slot];
    e.apply_command(0, &cast_spell(bear_slot, vec![]))
        .expect("cast Grizzly Bears");
    let bear_spell = e.state.stack.last().expect("bear spell").id;
    let slot = hand_index_for_card(&e, 0, "three_steps_ahead");
    semantic::accepted(
        &mut e,
        0,
        &cast_spree(slot, vec![(0, target_stack(bear_spell))], vec![0]),
    );
    let stack = e.state.stack.last().expect("spree spell");
    assert_eq!(stack.chosen_modes.len(), 1);
    assert_eq!(stack.chosen_modes[0].mode_id.as_str(), "counter");
    assert_eq!(stack.cast_cost_receipts.len(), 1);
    // Base {U} plus {1}{U}: blue 9 -> 7. Grizzly Bears already spent one colorless ({1}{G}),
    // so the pool's colorless count goes 9 -> 8 -> 7.
    assert_eq!(e.state.players[0].mana_pool.blue, 7);
    assert_eq!(e.state.players[0].mana_pool.colorless, 7);
    semantic::complete(&mut e, 8, |_| None).require_exercised();
    assert_eq!(e.state.objects[&bear_card].zone, Zone::Graveyard);

    // Copy mode: create a token copy of a creature you control for {3}.
    let mut copy = engine(359_502, &["three_steps_ahead"], &[]);
    let creature = inject_creature_on_battlefield(&mut copy, 0, "grizzly_bears");
    ensure_card_in_hand(&mut copy, 0, "three_steps_ahead");
    let slot = hand_index_for_card(&copy, 0, "three_steps_ahead");
    semantic::accepted(
        &mut copy,
        0,
        &cast_spree(slot, vec![(1, target_object(creature))], vec![1]),
    );
    assert_eq!(
        copy.state.stack.last().unwrap().chosen_modes[0]
            .mode_id
            .as_str(),
        "copy"
    );
    // Base {U} plus the linked {3}: blue 9 -> 8, colorless 9 -> 6.
    assert_eq!(copy.state.players[0].mana_pool.blue, 8);
    assert_eq!(copy.state.players[0].mana_pool.colorless, 6);
    semantic::complete(&mut copy, 8, |_| None).require_exercised();
    let bears = copy.state.players[0]
        .battlefield
        .iter()
        .filter(|oid| copy.state.objects[oid].card_id == "grizzly_bears")
        .count();
    assert_eq!(bears, 2, "the original plus its token copy");

    // Draw-discard mode: draw two, then discard one for {2}.
    let mut draw = engine(359_503, &["three_steps_ahead"], &[]);
    ensure_card_in_hand(&mut draw, 0, "three_steps_ahead");
    let discard = inject_card_into_hand(&mut draw, 0, "grizzly_bears");
    let slot = hand_index_for_card(&draw, 0, "three_steps_ahead");
    let hand_before = draw.state.players[0].hand.len();
    semantic::accepted(&mut draw, 0, &cast_spree(slot, vec![(2, vec![])], vec![2]));
    // Base {U} plus the linked {2}: blue 9 -> 8, colorless 9 -> 7.
    assert_eq!(draw.state.players[0].mana_pool.blue, 8);
    assert_eq!(draw.state.players[0].mana_pool.colorless, 7);
    semantic::accepted(&mut draw, 0, &pass());
    let batch = semantic::accepted(&mut draw, 1, &pass());
    let choice = find_resolution_choice(&batch).expect("discard choice after drawing two");
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.deciding_player_id, 0);
    semantic::accepted(&mut draw, 0, &submit_resolution_choice(vec![discard]));
    assert_eq!(draw.state.objects[&discard].zone, Zone::Graveyard);
    assert_eq!(
        draw.state.players[0].hand.len(),
        hand_before - 1 + 2 - 1,
        "cast, draw two, then discard one"
    );
}

#[test]
fn issue_359_trash_the_town_modes_and_linked_costs() {
    // Counter mode: exactly two +1/+1 counters.
    let mut counters = engine(359_511, &["trash_the_town"], &[]);
    let creature = inject_creature_with_stats(&mut counters, 1, "grizzly_bears", 2, 2);
    ensure_card_in_hand(&mut counters, 0, "trash_the_town");
    let slot = hand_index_for_card(&counters, 0, "trash_the_town");
    semantic::accepted(
        &mut counters,
        0,
        &cast_spree(slot, vec![(0, target_object(creature))], vec![0]),
    );
    semantic::complete(&mut counters, 8, |_| None).require_exercised();
    assert_eq!(counters.effective_power(creature), Some(4));
    assert_eq!(counters.effective_toughness(creature), Some(4));

    // Trample mode: trample until end of turn.
    let mut trample = engine(359_512, &["trash_the_town"], &[]);
    let creature = inject_creature_with_stats(&mut trample, 1, "grizzly_bears", 2, 2);
    ensure_card_in_hand(&mut trample, 0, "trash_the_town");
    let slot = hand_index_for_card(&trample, 0, "trash_the_town");
    semantic::accepted(
        &mut trample,
        0,
        &cast_spree(slot, vec![(1, target_object(creature))], vec![1]),
    );
    semantic::complete(&mut trample, 8, |_| None).require_exercised();
    assert!(trample.effective_has_keyword(creature, tricerules_cards::Keyword::Trample));

    // Grant mode: a live granted combat-damage triggered ability until end of turn.
    let mut grant = engine(359_513, &["trash_the_town"], &[]);
    let creature = inject_creature_with_stats(&mut grant, 1, "grizzly_bears", 2, 2);
    ensure_card_in_hand(&mut grant, 0, "trash_the_town");
    let slot = hand_index_for_card(&grant, 0, "trash_the_town");
    semantic::accepted(
        &mut grant,
        0,
        &cast_spree(slot, vec![(2, target_object(creature))], vec![2]),
    );
    semantic::complete(&mut grant, 8, |_| None).require_exercised();
    let granted = grant
        .state
        .continuous_effects
        .iter()
        .find_map(|effect| match &effect.kind {
            ContinuousEffectKind::GrantTriggeredAbility(ability) => Some(ability.clone()),
            _ => None,
        })
        .expect("the quoted triggered ability is granted until end of turn");
    assert_eq!(
        granted.trigger,
        tricerules_cards::primitives::TriggerCondition::WheneverSelfDealsCombatDamageToPlayer
    );
    assert_eq!(
        granted.effect,
        [tricerules_cards::primitives::SpellEffectKind::Draw {
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            count: tricerules_cards::primitives::Amount::Fixed(2),
        }]
    );
}

#[test]
fn issue_359_insatiable_avarice_modes_and_linked_costs() {
    // Tutor mode: search for a card and put it on top of the library.
    let mut tutor = engine(359_521, &["insatiable_avarice"], &[]);
    ensure_card_in_hand(&mut tutor, 0, "insatiable_avarice");
    let mountain = inject_library_card(&mut tutor, 0, "mountain");
    let slot = hand_index_for_card(&tutor, 0, "insatiable_avarice");
    semantic::accepted(&mut tutor, 0, &cast_spree(slot, vec![(0, vec![])], vec![0]));
    semantic::accepted(&mut tutor, 0, &pass());
    let batch = semantic::accepted(&mut tutor, 1, &pass());
    let choice = find_resolution_choice(&batch).expect("library search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(choice.candidate_object_ids.contains(&mountain));
    semantic::accepted(&mut tutor, 0, &submit_resolution_choice(vec![mountain]));
    assert_eq!(tutor.state.players[0].library.front(), Some(&mountain));

    // Draw-drain mode: a target player draws three and loses three life.
    let mut drain = engine(359_522, &["insatiable_avarice"], &[]);
    ensure_card_in_hand(&mut drain, 0, "insatiable_avarice");
    let slot = hand_index_for_card(&drain, 0, "insatiable_avarice");
    let hand_before = drain.state.players[1].hand.len();
    let life_before = drain.state.players[1].life;
    semantic::accepted(
        &mut drain,
        0,
        &cast_spree(slot, vec![(1, target_player(1))], vec![1]),
    );
    semantic::complete(&mut drain, 8, |_| None).require_exercised();
    assert_eq!(drain.state.players[1].hand.len(), hand_before + 3);
    assert_eq!(drain.state.players[1].life, life_before - 3);
}

#[test]
fn issue_359_jailbreak_scheme_modes_and_linked_costs() {
    // Counter-block mode: +1/+1 counter and unblockable this turn.
    let mut lock = engine(359_531, &["jailbreak_scheme"], &[]);
    let creature = inject_creature_with_stats(&mut lock, 1, "grizzly_bears", 2, 2);
    ensure_card_in_hand(&mut lock, 0, "jailbreak_scheme");
    let slot = hand_index_for_card(&lock, 0, "jailbreak_scheme");
    semantic::accepted(
        &mut lock,
        0,
        &cast_spree(slot, vec![(0, target_object(creature))], vec![0]),
    );
    semantic::complete(&mut lock, 8, |_| None).require_exercised();
    assert_eq!(lock.effective_power(creature), Some(3));
    assert!(
        lock.state.continuous_effects.iter().any(|effect| {
            matches!(
                &effect.kind,
                ContinuousEffectKind::CombatRestriction(restriction) if restriction.cant_be_blocked
            )
        }),
        "the target becomes unblockable this turn"
    );

    // Library-choice mode: the owner puts the target on top or bottom of their library.
    let mut place = engine(359_532, &["jailbreak_scheme"], &[]);
    let victim = inject_creature_with_stats(&mut place, 1, "grizzly_bears", 2, 2);
    ensure_card_in_hand(&mut place, 0, "jailbreak_scheme");
    let slot = hand_index_for_card(&place, 0, "jailbreak_scheme");
    semantic::accepted(
        &mut place,
        0,
        &cast_spree(slot, vec![(1, target_object(victim))], vec![1]),
    );
    semantic::accepted(&mut place, 0, &pass());
    let batch = semantic::accepted(&mut place, 1, &pass());
    let choice = find_resolution_choice(&batch).expect("owner placement choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    semantic::accepted(&mut place, 1, &submit_branch(0));
    assert_eq!(place.state.objects[&victim].zone, Zone::Library);
    assert_eq!(place.state.players[1].library.front(), Some(&victim));

    // The owner may instead choose the bottom.
    let mut bottom = engine(359_533, &["jailbreak_scheme"], &[]);
    let victim = inject_creature_with_stats(&mut bottom, 1, "grizzly_bears", 2, 2);
    ensure_card_in_hand(&mut bottom, 0, "jailbreak_scheme");
    let slot = hand_index_for_card(&bottom, 0, "jailbreak_scheme");
    semantic::accepted(
        &mut bottom,
        0,
        &cast_spree(slot, vec![(1, target_object(victim))], vec![1]),
    );
    semantic::accepted(&mut bottom, 0, &pass());
    let batch = semantic::accepted(&mut bottom, 1, &pass());
    let _ = find_resolution_choice(&batch).expect("owner placement choice");
    semantic::accepted(&mut bottom, 1, &submit_branch(1));
    assert_eq!(bottom.state.players[1].library.back(), Some(&victim));
}

#[test]
fn issue_359_pyrrhic_strike_blight_unlocks_both_modes() {
    // One mode without the blight is legal.
    let mut single = engine(359_541, &["pyrrhic_strike"], &[]);
    let artifact = inject_permanent_on_battlefield(&mut single, 1, "swiftfoot_boots");
    ensure_card_in_hand(&mut single, 0, "pyrrhic_strike");
    let slot = hand_index_for_card(&single, 0, "pyrrhic_strike");
    semantic::accepted(
        &mut single,
        0,
        &cast_spree(slot, vec![(0, target_object(artifact))], vec![]),
    );
    semantic::complete(&mut single, 8, |_| None).require_exercised();
    assert_eq!(single.state.objects[&artifact].zone, Zone::Graveyard);

    // Both modes without the blight is illegal.
    let mut both = engine(359_542, &["pyrrhic_strike"], &[]);
    let artifact = inject_permanent_on_battlefield(&mut both, 1, "swiftfoot_boots");
    let big = inject_creature_with_stats(&mut both, 1, "hill_giant", 3, 3);
    ensure_card_in_hand(&mut both, 0, "pyrrhic_strike");
    let slot = hand_index_for_card(&both, 0, "pyrrhic_strike");
    assert!(
        both.apply_command(
            0,
            &cast_spree(
                slot,
                vec![(0, target_object(artifact)), (1, target_object(big))],
                vec![],
            ),
        )
        .is_err(),
        "choosing both modes requires paying the blight additional cost"
    );

    // Paying the blight while choosing only one mode is also illegal.
    let mut paid_one = engine(359_545, &["pyrrhic_strike"], &[]);
    let blight_target = inject_creature_with_stats(&mut paid_one, 0, "grizzly_bears", 2, 4);
    let artifact = inject_permanent_on_battlefield(&mut paid_one, 1, "swiftfoot_boots");
    ensure_card_in_hand(&mut paid_one, 0, "pyrrhic_strike");
    let slot = hand_index_for_card(&paid_one, 0, "pyrrhic_strike");
    let command = cast_spree_blight(
        slot,
        vec![(0, target_object(artifact))],
        &paid_one,
        blight_target,
    );
    assert!(
        paid_one.apply_command(0, &command).is_err(),
        "paying the blight requires choosing both modes"
    );

    // Paying blight 2 (both counters on one creature) allows both modes.
    let mut blighted = engine(359_543, &["pyrrhic_strike"], &[]);
    let blight_target = inject_creature_with_stats(&mut blighted, 0, "grizzly_bears", 2, 4);
    let artifact = inject_permanent_on_battlefield(&mut blighted, 1, "swiftfoot_boots");
    let big = inject_creature_with_stats(&mut blighted, 1, "hill_giant", 3, 3);
    ensure_card_in_hand(&mut blighted, 0, "pyrrhic_strike");
    let slot = hand_index_for_card(&blighted, 0, "pyrrhic_strike");
    let command = cast_spree_blight(
        slot,
        vec![(0, target_object(artifact)), (1, target_object(big))],
        &blighted,
        blight_target,
    );
    semantic::accepted(&mut blighted, 0, &command);
    semantic::complete(&mut blighted, 8, |_| None).require_exercised();
    assert_eq!(
        blighted.state.objects[&blight_target]
            .counters
            .get(&tricerules_cards::CounterKind::MinusOneMinusOne),
        Some(&2)
    );
    assert_eq!(blighted.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(blighted.state.objects[&big].zone, Zone::Graveyard);

    // A creature with mana value below 3 is not a legal mode-two target.
    let mut small = engine(359_544, &["pyrrhic_strike"], &[]);
    let small_creature = inject_creature_with_stats(&mut small, 1, "grizzly_bears", 2, 2);
    ensure_card_in_hand(&mut small, 0, "pyrrhic_strike");
    let slot = hand_index_for_card(&small, 0, "pyrrhic_strike");
    assert!(
        small
            .apply_command(
                0,
                &cast_spree(slot, vec![(1, target_object(small_creature))], vec![]),
            )
            .is_err(),
        "mode two requires mana value 3 or greater"
    );
}
