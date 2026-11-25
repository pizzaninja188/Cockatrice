//! Actual-card scenarios for the queue-v86 01-ally-entry-counter cohort.
//!
//! Six Allies that print "Whenever this creature or another Ally you control enters, you may put
//! a +1/+1 counter on this creature" plus one additional printed clause: can't block (Bojuka
//! Brigand), Trample (Graypelt Hunter), first strike + vigilance (Kazandu Blademaster), Defender
//! (Makindi Shieldmate), Haste (Tuktuk Grunts), Flying (Umara Raptor).
//!
//! Expectations are printed Oracle values. CR 603.6a (the entry watcher includes this creature or
//! another Ally), 603.5 (the controller may decline the optional ability), 122.1 (the +1/+1
//! counter), 509.1b (the source-only blocking restriction), 702.19 (trample), 702.7 (first
//! strike), 702.20 (vigilance), 702.3 (defender), 702.10 (haste) and 702.9 (flying) govern.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    keywords: &'static [Keyword],
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "bojuka_brigand",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            keywords: &[],
        },
        Case {
            id: "graypelt_hunter",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
            keywords: &[Keyword::Trample],
        },
        Case {
            id: "kazandu_blademaster",
            mana: ManaGift {
                w: 2,
                ..Default::default()
            },
            body: (1, 1),
            keywords: &[Keyword::FirstStrike, Keyword::Vigilance],
        },
        Case {
            id: "makindi_shieldmate",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (0, 3),
            keywords: &[Keyword::Defender],
        },
        Case {
            id: "tuktuk_grunts",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            body: (2, 2),
            keywords: &[Keyword::Haste],
        },
        Case {
            id: "umara_raptor",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 1),
            keywords: &[Keyword::Flying],
        },
    ]
}

fn case(id: &'static str) -> Case {
    cases()
        .into_iter()
        .find(|case| case.id == id)
        .expect("known case")
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["grizzly_bears", "hada_freeblade", "coralhelm_guide"],
        ),
        deck_with(
            "forest",
            &["grizzly_bears", "hada_freeblade", "coralhelm_guide"],
        ),
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

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast cost", case.id);
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

fn plus_counters(e: &GameEngine, object: u32) -> u32 {
    e.state.objects[&object].counter_count(CounterKind::PlusOnePlusOne)
}

/// Pass priority for whichever player holds it, toward the requested step.
fn pass_once_for(e: &mut GameEngine) {
    let actor = e.state.priority_player_id();
    e.apply_scenario_command(actor, &pass())
        .expect("pass priority toward the requested step");
}

/// Answer every trigger-order prompt and accept every optional Ally counter branch until the
/// stack and all choices are empty.
fn settle_two_ally_triggers(e: &mut GameEngine) {
    for _ in 0..12 {
        answer_trigger_order_in_engine_order(e);
        if e.state.pending_resolution.is_some() {
            answer_optional_triggered_ability_choice(e, ResolutionChoiceDecision::SelectBranch);
            continue;
        }
        if e.state.stack.is_empty() {
            return;
        }
        pass_both_players(e);
    }
    panic!("Ally triggers did not settle");
}

/// Advance to `player`'s first main phase, resolving any cleanup discard along the way.
fn advance_to_player_main1(e: &mut GameEngine, player: i32) {
    for _ in 0..80 {
        if e.state.turn_step == tricerules_core::TurnStep::Main1
            && e.state.active_player_id() == player
        {
            return;
        }
        if let Some(cleanup_player) = e.state.cleanup_discard_player {
            let index = e.state.player_idx(cleanup_player).expect("cleanup player");
            let excess = e.state.players[index].hand.len().saturating_sub(7);
            e.apply_scenario_command(
                cleanup_player,
                &discard_cleanup_batch((0..excess as u32).collect()),
            )
            .expect("discard down to hand size during cleanup");
        } else {
            pass_once_for(e);
        }
    }
    panic!("game did not reach player {player}'s main phase");
}

/// Canary case: Bojuka Brigand is the cheapest and adds a printed blocking restriction.
#[test]
fn q86_ally_canary_bojuka_brigand_enters_then_gains_one_counter() {
    let case = case("bojuka_brigand");
    let mut e = engine(20_860_100);
    let source = cast_source(&mut e, &case);
    assert_eq!(
        e.state.objects[&source].card_id, case.id,
        "canary: the simplest card is cast as itself"
    );
    assert_eq!(
        plus_counters(&e, source),
        0,
        "the trigger is still awaiting its optional choice"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(1), Some(1)),
        "it entered with its unmodified printed body"
    );
    answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(
        plus_counters(&e, source),
        1,
        "the accepted trigger adds one counter"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(2), Some(2)),
        "printed 1/1 plus the counter"
    );
}

/// Hardest case: Kazandu Blademaster carries two printed keywords and costs {W}{W}.
#[test]
fn q86_ally_canary_kazandu_blademaster_keeps_both_keywords_and_gains_a_counter() {
    let case = case("kazandu_blademaster");
    let mut e = engine(20_860_101);
    let source = cast_source(&mut e, &case);
    for keyword in case.keywords {
        assert!(
            e.effective_has_keyword(source, *keyword),
            "printed {keyword:?} survives entry"
        );
    }
    answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(plus_counters(&e, source), 1);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(2), Some(2)),
        "printed 1/1 plus the counter"
    );
    for keyword in case.keywords {
        assert!(
            e.effective_has_keyword(source, *keyword),
            "printed {keyword:?} survives the counter"
        );
    }
}

#[test]
fn q86_ally_cards_decline_leaves_the_printed_body() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_861_000 + index as u64);
        let source = cast_source(&mut e, case);
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
        assert_eq!(
            plus_counters(&e, source),
            0,
            "{}: decline adds nothing",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(source, *keyword),
                "{}: printed {keyword:?}",
                case.id
            );
        }
    }
}

#[test]
fn q86_ally_cards_trigger_for_another_ally_you_control() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_862_000 + index as u64);
        let source = cast_source(&mut e, case);
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
        assert_eq!(plus_counters(&e, source), 0, "{}", case.id);

        // A second Ally entering afterwards triggers the watcher again. The new Ally triggers
        // both its own watcher and this one, so a trigger-order prompt appears before any
        // optional branch is offered.
        move_ready_to_battlefield(&mut e, 0, "hada_freeblade");
        settle_two_ally_triggers(&mut e);
        assert_eq!(
            plus_counters(&e, source),
            1,
            "{}: another Ally you control triggers the watcher",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0 + 1), Some(case.body.1 + 1)),
            "{}: printed body plus one counter",
            case.id
        );
    }
}

#[test]
fn q86_ally_cards_ignore_non_allies_and_opposing_allies() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_863_000 + index as u64);
        let source = cast_source(&mut e, case);
        answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);

        move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.pending_resolution.is_none(),
            "{}: a non-Ally entry never offers the optional trigger",
            case.id
        );
        // An opposing Ally still triggers its own watcher, so consume those offers, but our
        // source must not gain a counter from it (the filter is controller-scoped).
        move_ready_to_battlefield(&mut e, 1, "hada_freeblade");
        settle_two_ally_triggers(&mut e);
        assert_eq!(
            plus_counters(&e, source),
            0,
            "{}: no counter from a non-Ally or an opposing Ally",
            case.id
        );
    }
}

#[test]
fn q86_ally_cards_carry_their_exact_printed_characteristics() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(20_865_000 + index as u64);
        let source = cast_source(&mut e, case);
        assert_eq!(e.state.objects[&source].card_id, case.id, "{}", case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body before the optional choice",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(source, *keyword),
                "{}: printed {keyword:?}",
                case.id
            );
        }
        assert_eq!(
            e.effective_has_keyword(source, Keyword::Trample),
            case.keywords.contains(&Keyword::Trample),
            "{}: only the printed Trample",
            case.id
        );
        assert_eq!(
            e.effective_has_keyword(source, Keyword::Flying),
            case.keywords.contains(&Keyword::Flying),
            "{}: only the printed Flying",
            case.id
        );
    }
}

#[test]
fn q86_ally_cards_keep_the_printed_blocking_restriction() {
    let case = case("bojuka_brigand");
    let mut e = engine(20_866_000);
    let source = cast_source(&mut e, &case);
    answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(plus_counters(&e, source), 1);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, source),
        vec!["Can't block"],
        "the printed block restriction is still published after the counter"
    );

    // The printed "this creature can't block" still holds after the counter, and it is
    // source-only: with the opponent attacking, the source is never a legal blocker while a
    // plain creature you control is (CR 509.1b).
    let attacker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    end_active_turn(&mut e, 0);
    advance_to_player_main1(&mut e, 1);
    e.apply_scenario_command(1, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(&mut e);
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    e.apply_scenario_command(1, &declare_attackers(vec![attacker]))
        .expect("the opponent attacks");
    pass_both_players(&mut e);
    let legal = &e.initial_response_batch().legal_by_player[&0];
    let offered = |blocker: u32| {
        legal
            .legal_block_pairs
            .iter()
            .any(|pair| pair.blocker_id == blocker && pair.attacker_id == attacker)
    };
    assert!(!offered(source), "the restriction still forbids the block");
    assert!(offered(other), "the restriction is source-only");
    assert!(
        e.apply_scenario_command(
            0,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: source,
            }]),
        )
        .is_err(),
        "the engine rejects the forbidden block"
    );
}
