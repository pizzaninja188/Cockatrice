//! Actual-card scenarios for the queue-v67 batch A cohorts.
//!
//! Three stable typed shapes: a printed keyword plus one activated self-pump, an Aura that keeps
//! the enchanted creature from untapping (with printed entry riders), and creatures whose only
//! keyword is Ward (with a printed cost-reduction rider on the hardest card). Expectations are
//! printed Oracle values. CR 602.1/602.2 and 118.3 govern activation and cost payment; CR
//! 611.2a/613.4c and 514.2 the until-end-of-turn pump; CR 303.4/502.3 the attached untap lock;
//! CR 603.6a the Aura entry triggers; CR 120.4b/704.5f damage destruction; CR 601.2f/700.4 the
//! conditional cost reduction; CR 702.21a-c Ward.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{AttachmentRecipient, TurnStep, Zone};
use tricerules_proto::ruled::v1::{ResolutionChoiceDecision, TargetRef};

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[card]), deck_with("forest", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn hand_size(e: &GameEngine, player: usize) -> usize {
    e.state.players[player].hand.len()
}

fn target_object(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        counter_amount: 0,
        expected_zone_change_generation: None,
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: 0,
    }]
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

fn cast_from_hand(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) -> u32 {
    let id = inject_card_into_hand(e, player, card);
    give_mana(e, player as i32, mana);
    let slot = hand_index_for_card(e, player, card);
    semantic::accepted(e, player as i32, &cast_spell(slot, targets));
    id
}

#[track_caller]
fn hand_action_reduction(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, player, card_id) as u32;
    let batch = engine.initial_response_batch();
    let actions = &batch.legal_by_player[&(player as i32)].hand_actions;
    actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| {
            panic!(
                "missing cast action for {card_id}; published: {:?}",
                actions
                    .iter()
                    .map(|action| (action.hand_index, action.card_name.as_str()))
                    .collect::<Vec<_>>()
            )
        })
        .generic_cost_reduction
}

fn advance_to_active_player(engine: &mut GameEngine, player: i32) {
    for _ in 0..80 {
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = engine
                    .state
                    .player_idx(cleanup_player)
                    .expect("cleanup player");
                let excess = engine.state.players[player_index]
                    .hand
                    .len()
                    .saturating_sub(7);
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine
            .apply_scenario_command(actor, &command)
            .expect("pass through turn");
        if engine.state.active_player_id() == player && engine.state.turn_step == TurnStep::Upkeep {
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

/// Player 1 casts an instant at a permanent controlled by player 0 during player 0's main phase.
fn opponent_casts_at(e: &mut GameEngine, card: &str, mana: ManaGift, target: u32) -> u32 {
    semantic::accepted(e, 0, &pass());
    let id = inject_card_into_hand(e, 1, card);
    give_mana(e, 1, mana);
    let slot = hand_index_for_card(e, 1, card);
    semantic::accepted(e, 1, &cast_spell(slot, target_object(target)));
    id
}

// ── 01 keyword plus activated self-pump ────────────────────────────────────

#[test]
fn q67_pump_canary_simplest_anointed_chorister_pumps_and_expires() {
    let mut e = engine(67_010_001, "anointed_chorister");
    let cleric = move_ready_to_battlefield(&mut e, 0, "anointed_chorister");
    assert_eq!(
        (e.effective_power(cleric), e.effective_toughness(cleric)),
        (Some(1), Some(1)),
        "printed 1/1 body"
    );
    assert!(
        e.effective_has_keyword(cleric, Keyword::Lifelink),
        "printed lifelink"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
    );

    activate(&mut e, 0, cleric, 0, vec![]);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed {{4}}{{W}}");
    assert!(
        !e.state.objects[&cleric].tapped,
        "the activation does not tap"
    );
    assert_eq!(
        (e.effective_power(cleric), e.effective_toughness(cleric)),
        (Some(1), Some(1)),
        "the pump waits for resolution"
    );
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(cleric), e.effective_toughness(cleric)),
        (Some(4), Some(4)),
        "printed +3/+3"
    );
    assert!(
        e.effective_has_keyword(cleric, Keyword::Lifelink),
        "lifelink is not part of the pump"
    );

    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(cleric), e.effective_toughness(cleric)),
        (Some(1), Some(1)),
        "CR 514.2: the pump expires in cleanup"
    );

    // CR 118.3/602.2: in a fresh main phase, insufficient mana is rejected without spending
    // anything and without pumping the creature.
    let mut e = engine(67_010_004, "anointed_chorister");
    let cleric = move_ready_to_battlefield(&mut e, 0, "anointed_chorister");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    let before = pool(&e);
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, cleric, 0, vec![]))
            .is_err(),
        "three generic plus {{W}} cannot pay {{4}}{{W}}"
    );
    assert_eq!(pool(&e), before, "no mana was spent");
    assert_eq!(
        (e.effective_power(cleric), e.effective_toughness(cleric)),
        (Some(1), Some(1))
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q67_pump_canary_hardest_rakdos_trumpeter_uses_its_off_color_cost() {
    let mut e = engine(67_010_002, "rakdos_trumpeter");
    let trumpeter = move_ready_to_battlefield(&mut e, 0, "rakdos_trumpeter");
    assert_eq!(
        (
            e.effective_power(trumpeter),
            e.effective_toughness(trumpeter)
        ),
        (Some(1), Some(3))
    );
    assert!(e.effective_has_keyword(trumpeter, Keyword::Menace));
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
    );

    activate(&mut e, 0, trumpeter, 0, vec![]);
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "printed {{3}}{{R}} paid in red"
    );
    assert!(!e.state.objects[&trumpeter].tapped);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (
            e.effective_power(trumpeter),
            e.effective_toughness(trumpeter)
        ),
        (Some(3), Some(3)),
        "printed +2/+0"
    );
    assert!(e.effective_has_keyword(trumpeter, Keyword::Menace));

    end_active_turn(&mut e, 0);
    assert_eq!(
        (
            e.effective_power(trumpeter),
            e.effective_toughness(trumpeter)
        ),
        (Some(1), Some(3))
    );

    // Pardic Collaborator's {B} activation cannot be paid with red mana.
    let mut e = engine(67_010_003, "pardic_collaborator");
    let pardic = move_ready_to_battlefield(&mut e, 0, "pardic_collaborator");
    assert_eq!(
        (e.effective_power(pardic), e.effective_toughness(pardic)),
        (Some(2), Some(2))
    );
    assert!(e.effective_has_keyword(pardic, Keyword::FirstStrike));
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, pardic, 0, vec![]))
            .is_err(),
        "the printed {{B}} activation rejects red mana"
    );
    assert_eq!(
        (e.effective_power(pardic), e.effective_toughness(pardic)),
        (Some(2), Some(2)),
        "the rejected activation does not pump"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, pardic, 0, vec![]);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(pardic), e.effective_toughness(pardic)),
        (Some(3), Some(3)),
        "printed +1/+1"
    );
}

#[test]
fn q67_pump_cards_keep_their_printed_keyword_and_do_not_tap() {
    for (seed, card, mana, body, pumped, keyword) in [
        (
            67_011_001,
            "anointed_chorister",
            ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            (1, 1),
            (4, 4),
            Keyword::Lifelink,
        ),
        (
            67_011_002,
            "pardic_collaborator",
            ManaGift {
                b: 1,
                ..Default::default()
            },
            (2, 2),
            (3, 3),
            Keyword::FirstStrike,
        ),
        (
            67_011_003,
            "rakdos_trumpeter",
            ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            (1, 3),
            (3, 3),
            Keyword::Menace,
        ),
    ] {
        let mut e = engine(seed, card);
        let creature = move_ready_to_battlefield(&mut e, 0, card);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(body.0), Some(body.1)),
            "{card}: printed body"
        );
        assert!(e.effective_has_keyword(creature, keyword), "{card}");
        give_mana(&mut e, 0, mana);
        activate(&mut e, 0, creature, 0, vec![]);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(pumped.0), Some(pumped.1)),
            "{card}: printed pump"
        );
        assert!(!e.state.objects[&creature].tapped, "{card}: no tap cost");
        assert!(
            e.effective_has_keyword(creature, keyword),
            "{card}: keyword persists"
        );
    }
}

// ── 02 Aura untap lock ─────────────────────────────────────────────────────

fn assert_locked_through_untap(e: &mut GameEngine, host: u32, controller: i32) {
    e.state.objects.get_mut(&host).unwrap().tapped = true;
    advance_to_active_player(e, controller);
    assert!(
        e.state.objects[&host].tapped,
        "the Aura keeps the creature tapped through its controller's untap step"
    );
}

#[test]
fn q67_aura_canary_simplest_dehydration_locks_only_untapping() {
    let mut e = semantic::main_phase(67_020_001);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let aura = cast_from_hand(
        &mut e,
        0,
        "dehydration",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    assert_eq!(e.state.objects[&bear].controller, 1, "control is unchanged");
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "the Aura does not change the body"
    );
    assert_locked_through_untap(&mut e, bear, 1);
}

#[test]
fn q67_aura_canary_hardest_frozen_solid_destroys_a_damaged_host() {
    let mut e = semantic::main_phase(67_020_002);
    let giant = inject_creature_with_stats(&mut e, 1, "hill_giant", 3, 3);
    assert_eq!(
        (e.effective_power(giant), e.effective_toughness(giant)),
        (Some(3), Some(3))
    );
    let aura = cast_from_hand(
        &mut e,
        0,
        "frozen_solid",
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
        target_object(giant),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&aura].zone, Zone::Battlefield);

    // Two damage leaves the 3/3 alive; the Aura's damage trigger then destroys it.
    inject_card_into_hand(&mut e, 0, "shock");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "shock");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(giant)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&giant].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&aura].zone,
        Zone::Graveyard,
        "the Aura follows its host"
    );

    // A surviving host stays tapped through its controller's untap step.
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let lock = cast_from_hand(
        &mut e,
        0,
        "frozen_solid",
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&lock].zone, Zone::Battlefield);
    assert_locked_through_untap(&mut e, bear, 1);
}

#[test]
fn q67_aura_eternity_snare_draws_on_entry_and_locks() {
    let mut e = semantic::main_phase(67_021_001);
    inject_card_into_hand(&mut e, 0, "eternity_snare");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 5,
            ..Default::default()
        },
    );
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let before = hand_size(&e, 0);
    let slot = hand_index_for_card(&e, 0, "eternity_snare");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(bear)));
    assert_eq!(hand_size(&e, 0), before - 1, "the Aura left the hand");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        hand_size(&e, 0),
        before,
        "the entry trigger draws exactly one card"
    );
    let aura = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|id| e.state.objects[id].card_id == "eternity_snare")
        .expect("Eternity Snare on the battlefield");
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    assert_locked_through_untap(&mut e, bear, 1);
}

#[test]
fn q67_aura_fizzles_when_its_target_leaves_before_resolution() {
    let mut e = semantic::main_phase(67_021_003);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let before = hand_size(&e, 0);
    let aura = cast_from_hand(
        &mut e,
        0,
        "eternity_snare",
        ManaGift {
            u: 1,
            c: 5,
            ..Default::default()
        },
        target_object(bear),
    );
    assert_eq!(hand_size(&e, 0), before, "injected Aura left the hand");
    semantic::accepted(&mut e, 0, &pass());
    let unsummon = cast_from_hand(
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
        "the target left before the Aura resolved"
    );
    assert_eq!(
        e.state.objects[&aura].zone,
        Zone::Graveyard,
        "the Aura fizzles and is not attached"
    );
    assert_eq!(
        hand_size(&e, 0),
        before,
        "Eternity Snare never entered, so it draws nothing"
    );
    assert_eq!(
        e.state.objects[&unsummon].zone,
        Zone::Graveyard,
        "the response resolved normally"
    );
}

#[test]
fn q67_aura_mystic_restraints_flashes_in_and_taps_on_entry() {
    let mut e = semantic::main_phase(67_021_002);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    // Move to player 1's upkeep, then pass priority so player 0 may use Flash.
    advance_to_active_player(&mut e, 1);
    semantic::accepted(&mut e, 1, &pass());
    let aura = cast_from_hand(
        &mut e,
        0,
        "mystic_restraints",
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&aura].zone,
        Zone::Battlefield,
        "Flash allowed the Aura in the opponent's upkeep"
    );
    assert!(
        e.state.objects[&bear].tapped,
        "the entry trigger taps the host"
    );
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    assert!(
        e.characteristics(aura)
            .expect("Aura characteristics")
            .keywords
            .contains(&Keyword::Flash),
        "printed flash"
    );
    assert_locked_through_untap(&mut e, bear, 1);
}

// ── 03 Ward creatures ──────────────────────────────────────────────────────

fn pay_ward(e: &mut GameEngine, player: i32, mana: ManaGift) {
    pass_both_players(e);
    assert!(
        e.state.pending_resolution.is_some(),
        "Ward payment is offered"
    );
    give_mana(e, player, mana);
    submit_mana_resolution_decision(e, player, ResolutionChoiceDecision::PayMana)
        .expect("pay Ward");
}

fn decline_ward(e: &mut GameEngine, player: i32) {
    pass_both_players(e);
    assert!(
        e.state.pending_resolution.is_some(),
        "Ward payment is offered"
    );
    semantic::accepted(
        e,
        player,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    );
}

#[test]
fn q67_ward_canary_simplest_tomakul_honor_guard_charges_two() {
    let mut e = engine(67_030_001, "tomakul_honor_guard");
    let guard = move_ready_to_battlefield(&mut e, 0, "tomakul_honor_guard");
    assert_eq!(
        (e.effective_power(guard), e.effective_toughness(guard)),
        (Some(3), Some(1))
    );
    opponent_casts_at(
        &mut e,
        "doom_blade",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        guard,
    );
    assert_eq!(e.state.stack.len(), 2, "Ward trigger above Doom Blade");
    decline_ward(&mut e, 1);
    assert_eq!(
        e.state.objects[&guard].zone,
        Zone::Battlefield,
        "declining Ward counters the spell"
    );

    // Paying {2} preserves the spell; the poison is resolved by its own effect.
    opponent_casts_at(
        &mut e,
        "doom_blade",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        guard,
    );
    pay_ward(
        &mut e,
        1,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects[&guard].zone,
        Zone::Graveyard,
        "the paid-for Doom Blade resolves"
    );
}

#[test]
fn q67_ward_canary_hardest_purple_worm_reduces_once_and_keeps_ward() {
    let mut e = semantic::main_phase(67_030_002);
    let worm = inject_card_into_hand(&mut e, 0, "purple_worm");
    assert_eq!(hand_action_reduction(&mut e, 0, "purple_worm"), 0);
    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // Two deaths still reduce only the printed {2} once (CR 601.2f/700.4).
    for victim in [first, second] {
        inject_card_into_hand(&mut e, 0, "murder");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "murder");
        semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(victim)));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    }
    assert_eq!(e.state.turn_history.current.creatures_died, 2);
    assert_eq!(
        hand_action_reduction(&mut e, 0, "purple_worm"),
        2,
        "the reduction is capped at {{2}}"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 2,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "purple_worm");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&worm].zone, Zone::Battlefield);
    assert_eq!(
        (e.effective_power(worm), e.effective_toughness(worm)),
        (Some(8), Some(7))
    );

    // Ward {2}: the opposing Doom Blade must pay exactly two more mana.
    opponent_casts_at(
        &mut e,
        "doom_blade",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        worm,
    );
    pay_ward(
        &mut e,
        1,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&worm].zone, Zone::Graveyard);
}

#[test]
fn q67_ward_rimeshield_frost_giant_charges_three() {
    let mut e = engine(67_031_001, "rimeshield_frost_giant");
    let giant = move_ready_to_battlefield(&mut e, 0, "rimeshield_frost_giant");
    assert_eq!(
        (e.effective_power(giant), e.effective_toughness(giant)),
        (Some(4), Some(5))
    );
    opponent_casts_at(
        &mut e,
        "doom_blade",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        giant,
    );
    pay_ward(
        &mut e,
        1,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&giant].zone, Zone::Graveyard);
}
