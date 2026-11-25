//! Actual-card scenarios for the queue-v82 02-etb-stun-riders cohort.
//!
//! The shared clause is an enters trigger that taps one chosen opponent creature and puts a stun
//! counter on it. Expectations are printed Oracle values. CR 603.6a stages the trigger, CR 115.1
//! chooses the target, CR 701.26 taps, CR 122.1f creates the stun counter, CR 509.1b publishes the
//! Gilded Scuttler restriction, and CR 702.21 governs Frostfist Strider's Ward {2}. The pinned
//! rulings establish that an already-tapped target is legal and stun replaces untapping.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "gilded_scuttler",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 3,
        },
        Case {
            id: "frostfist_strider",
            mana: ManaGift {
                u: 2,
                c: 3,
                ..Default::default()
            },
            power: 4,
            toughness: 4,
        },
        Case {
            id: "referee_squad",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn choose_trigger_target(target: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(target),
        })),
    }
}

fn advance_to_next_upkeep(e: &mut GameEngine) {
    let starting_turn = e.state.turn_instance;
    for _ in 0..240 {
        if e.state.turn_instance > starting_turn && e.state.turn_step == TurnStep::Upkeep {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance to the next upkeep");
    }
    panic!("turn advancement stalled before the next upkeep");
}

fn cast_and_stun(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    pass_both_players(e);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "{}: one entry trigger",
        case.id
    );
    e.apply_scenario_command(0, &choose_trigger_target(target))
        .expect("choose the stun target");
    resolve_entire_stack_two_player(e);
    e.state
        .objects
        .iter()
        .find_map(|(id, object)| {
            (object.card_id == case.id && object.zone == Zone::Battlefield).then_some(*id)
        })
        .unwrap_or_else(|| panic!("{} did not stay on the battlefield", case.id))
}

fn opponent_casts_at(e: &mut GameEngine, card: &str, mana: ManaGift, target: u32) -> u32 {
    semantic::accepted(e, 0, &pass());
    let id = inject_card_into_hand(e, 1, card);
    give_mana(e, 1, mana);
    let slot = hand_index_for_card(e, 1, card);
    semantic::accepted(e, 1, &cast_spell(slot, target_object(target)));
    id
}

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
fn q82_stun_canary_simplest_gilded_scuttler_taps_and_stuns() {
    let case = &cases()[0];
    let mut e = engine(82_020_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let source = cast_and_stun(&mut e, case, target);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed cost is spent");
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "printed body"
    );
    assert!(e.state.objects[&target].tapped, "the target is tapped");
    assert_eq!(
        e.state.objects[&target].counter_count(CounterKind::Stun),
        1,
        "one stun counter is placed"
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, source),
        vec!["Can't be blocked"],
        "the printed source-only restriction is published"
    );
}

#[test]
fn q82_stun_canary_hardest_frostfist_ward_pays_or_counters() {
    let case = &cases()[1];

    let mut paid = engine(82_021_001);
    let target = inject_creature_on_battlefield(&mut paid, 1, "grizzly_bears");
    let strider = cast_and_stun(&mut paid, case, target);
    assert_eq!(
        paid.state.objects[&target].counter_count(CounterKind::Stun),
        1
    );
    opponent_casts_at(
        &mut paid,
        "doom_blade",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        strider,
    );
    assert_eq!(paid.state.stack.len(), 2, "Ward is above Doom Blade");
    pay_ward(
        &mut paid,
        1,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    pass_both_players(&mut paid);
    assert_eq!(
        paid.state.objects[&strider].zone,
        Zone::Graveyard,
        "paying the printed {{2}} lets Doom Blade resolve"
    );

    let mut declined = engine(82_021_002);
    let target = inject_creature_on_battlefield(&mut declined, 1, "grizzly_bears");
    let strider = cast_and_stun(&mut declined, case, target);
    opponent_casts_at(
        &mut declined,
        "doom_blade",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        strider,
    );
    decline_ward(&mut declined, 1);
    pass_both_players(&mut declined);
    assert_eq!(
        declined.state.objects[&strider].zone,
        Zone::Battlefield,
        "declining Ward counters the spell"
    );
}

#[test]
fn q82_stun_cards_apply_entry_effect_body_and_keywords() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(82_022_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let source = cast_and_stun(&mut e, &case, target);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert!(e.state.objects[&target].tapped, "{}: tap", case.id);
        assert_eq!(
            e.state.objects[&target].counter_count(CounterKind::Stun),
            1,
            "{}: stun",
            case.id
        );
        match case.id {
            "gilded_scuttler" => {
                assert_eq!(
                    zone_view_rules_annotation_labels(&mut e, 0, source),
                    vec!["Can't be blocked"],
                    "{}: restriction",
                    case.id
                );
            }
            "frostfist_strider" => {}
            "referee_squad" => {
                assert!(
                    e.effective_has_keyword(source, Keyword::Convoke),
                    "{}: convoke",
                    case.id
                );
                assert!(
                    e.effective_has_keyword(source, Keyword::Vigilance),
                    "{}: vigilance",
                    case.id
                );
            }
            other => panic!("unexpected case {other}"),
        }
    }
}

#[test]
fn q82_stun_cards_accept_an_already_tapped_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(82_023_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&target).expect("target").tapped = true;
        cast_and_stun(&mut e, &case, target);
        assert!(
            e.state.objects[&target].tapped,
            "{}: an already-tapped target stays tapped",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].counter_count(CounterKind::Stun),
            1,
            "{}: the stun counter is still placed",
            case.id
        );
    }
}

#[test]
fn q82_stun_counter_replaces_the_next_untap() {
    let case = &cases()[0];
    let mut e = engine(82_024_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_stun(&mut e, case, target);
    assert!(e.state.objects[&target].tapped);
    assert_eq!(e.state.objects[&target].counter_count(CounterKind::Stun), 1);
    advance_to_next_upkeep(&mut e);
    assert!(
        e.state.objects[&target].tapped,
        "the stun counter replaces the untap"
    );
    assert_eq!(
        e.state.objects[&target].counter_count(CounterKind::Stun),
        0,
        "exactly one stun counter is removed"
    );
}

#[test]
fn q82_stun_trigger_rejects_the_controllers_own_creature() {
    let case = &cases()[0];
    let mut e = engine(82_025_001);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let _opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.mana));
    inject_card_into_hand(&mut e, 0, case.id);
    let hand_slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast Gilded Scuttler");
    pass_both_players(&mut e);
    assert_eq!(e.state.pending_triggers.len(), 1);
    let before = e.diagnostic_snapshot().expect("diagnostic snapshot");
    e.apply_scenario_command(0, &choose_trigger_target(own))
        .expect_err("the trigger requires an opponent's creature");
    assert_eq!(
        e.diagnostic_snapshot().expect("diagnostic snapshot"),
        before,
        "the rejected target preserves state"
    );
}
