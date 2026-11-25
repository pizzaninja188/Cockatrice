//! Actual-card scenarios for the queue-v82 01-etb-tap-riders cohort.
//!
//! Each card has an enters trigger that taps one chosen permanent; Merfolk Trickster narrows the
//! target to an opponent's creature and removes its abilities, while Harrier Strix adds a
//! draw-then-discard activation. Expectations are printed Oracle values. CR 603.6a stages the
//! trigger, CR 115.1 chooses the target, CR 701.26 taps, CR 613.1f/613.4 and 514.2 govern the
//! temporary ability removal, and CR 121.1/701.9 govern the loot.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, RuledCommand,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
    flying: bool,
    target_is_creature_only: bool,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "network_disruptor",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            flying: true,
            target_is_creature_only: false,
        },
        Case {
            id: "harrier_strix",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            flying: true,
            target_is_creature_only: false,
        },
        Case {
            id: "merfolk_trickster",
            mana: ManaGift {
                u: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            flying: false,
            target_is_creature_only: true,
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
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

fn choose_trigger_target(target: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(target),
        })),
    }
}

fn cast_and_tap(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    give_mana(
        e,
        0,
        ManaGift {
            w: case.mana.w,
            u: case.mana.u,
            b: case.mana.b,
            r: case.mana.r,
            g: case.mana.g,
            c: case.mana.c,
        },
    );
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
        .expect("choose the tap target");
    resolve_entire_stack_two_player(e);
    e.state
        .objects
        .iter()
        .find_map(|(id, object)| {
            (object.card_id == case.id && object.zone == Zone::Battlefield).then_some(*id)
        })
        .unwrap_or_else(|| panic!("{} did not stay on the battlefield", case.id))
}

#[test]
fn q82_tap_canary_simplest_network_disruptor_taps_any_permanent() {
    let case = &cases()[0];
    let mut e = engine(82_040_001);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let source = cast_and_tap(&mut e, case, land);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed cost is spent");
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "printed body"
    );
    assert!(
        e.effective_has_keyword(source, Keyword::Flying),
        "printed flying"
    );
    assert!(
        e.state.objects[&land].tapped,
        "the printed target permanent is tapped"
    );
}

#[test]
fn q82_tap_canary_hardest_merfolk_trickster_removes_abilities_until_cleanup() {
    let case = &cases()[2];
    let mut e = engine(82_041_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "cloud_spirit");
    assert!(e.effective_has_keyword(target, Keyword::Flying));
    let source = cast_and_tap(&mut e, case, target);
    assert!(
        e.effective_has_keyword(source, Keyword::Flash),
        "printed flash"
    );
    assert!(e.state.objects[&target].tapped, "the target is tapped");
    assert!(
        !e.effective_has_keyword(target, Keyword::Flying),
        "the target loses all abilities"
    );
    end_active_turn(&mut e, 0);
    assert!(
        e.effective_has_keyword(target, Keyword::Flying),
        "CR 514.2: the ability removal expires in cleanup"
    );
}

#[test]
fn q82_tap_cards_apply_entry_effect_body_and_keywords() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(82_042_000 + index as u64);
        let target = if case.target_is_creature_only {
            inject_creature_on_battlefield(&mut e, 1, "grizzly_bears")
        } else {
            inject_permanent_on_battlefield(&mut e, 1, "forest")
        };
        let source = cast_and_tap(&mut e, &case, target);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert_eq!(
            e.effective_has_keyword(source, Keyword::Flying),
            case.flying,
            "{}: printed flying",
            case.id
        );
        assert!(
            e.state.objects[&target].tapped,
            "{}: the chosen permanent is tapped",
            case.id
        );
    }
}

#[test]
fn q82_tap_harrier_strix_already_tapped_target_is_legal() {
    let case = &cases()[1];
    let mut e = engine(82_043_001);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    e.state.objects.get_mut(&land).expect("land").tapped = true;
    cast_and_tap(&mut e, case, land);
    assert!(
        e.state.objects[&land].tapped,
        "the pinned ruling allows an already-tapped target"
    );
}

#[test]
fn q82_tap_harrier_strix_loots_privately() {
    let case = &cases()[1];
    let mut e = engine(82_044_001);
    let strix = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let top = inject_library_card(&mut e, 0, "grizzly_bears");
    let next = inject_library_card(&mut e, 0, "forest");
    e.state.players[0]
        .library
        .retain(|oid| *oid != top && *oid != next);
    e.state.players[0].library.push_front(top);
    e.state.players[0].library.insert(1, next);
    let hand_before = e.state.players[0].hand.len();
    let opponent_hand_before = e.state.players[1].hand.clone();
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let command = activate_ability_for(&e, strix, 0, vec![]);
    semantic::accepted(&mut e, 0, &command);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed {{2}}{{U}}");
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
    let pending_resolution = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the mandatory discard parks");
    assert_eq!(
        pending_resolution.deciding_player, e.state.players[0].id,
        "only the looting controller discards"
    );
    let pending = pending_resolution.presentation.clone();
    assert_eq!(pending.choice_kind, ChoiceKind::HandCards);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the draw completed before the discard choice"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 1,
        "exactly one card was drawn"
    );
    assert!(e.state.players[0].hand.contains(&top));
    assert!(
        pending
            .candidates
            .iter()
            .all(|oid| e.state.players[0].hand.contains(oid)),
        "the discard candidates are exactly from the controller's hand"
    );
    assert!(
        pending
            .candidates
            .iter()
            .all(|oid| !e.state.players[1].hand.contains(oid)),
        "the opponent's hidden hand is not offered"
    );
    let discarded = pending.candidates[0];
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![discarded]));
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "draw one then discard one leaves the hand size unchanged"
    );
    assert_eq!(
        e.state.players[1].hand, opponent_hand_before,
        "opponent hand unchanged"
    );
    assert_eq!(e.state.objects[&discarded].zone, Zone::Graveyard);
}

#[test]
fn q82_tap_merfolk_trickster_rejects_your_own_creature() {
    let case = &cases()[2];
    let mut e = engine(82_045_001);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let _opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, case.id);
    let hand_slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast Merfolk Trickster");
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

#[test]
fn q82_tap_stale_target_does_not_retarget() {
    let case = &cases()[0];
    let mut e = engine(82_046_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.mana));
    inject_card_into_hand(&mut e, 0, case.id);
    let hand_slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast Network Disruptor");
    pass_both_players(&mut e);
    assert_eq!(e.state.pending_triggers.len(), 1);
    e.apply_scenario_command(0, &choose_trigger_target(target))
        .expect("choose the doomed target");
    e.state.players[1].battlefield.retain(|oid| *oid != target);
    e.state.players[1].graveyard.push(target);
    e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
    resolve_entire_stack_two_player(&mut e);
    assert!(
        !e.state.objects[&other].tapped,
        "the trigger does not retarget another permanent"
    );
}
