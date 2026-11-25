//! Actual-card scenarios for the queue-v69 batch A cohorts.
//!
//! Three stable typed shapes: a black draw-two-and-lose-two spell with an optional scry rider, a
//! small Wizard with exactly two one-colored-mana `{C}, {T}` activated abilities, and an artifact
//! whose `{T}` mana ability offers all five colors with one additional clause. Expectations are
//! printed Oracle values. CR 121.2/119.3/608.2c govern the draw, life loss and order;
//! CR 701.22a-b scry; CR 602.1/602.2 and 118.3 activation and costs; CR 611.2a/613.4c/514.2 pumps
//! and grants; CR 115.1 targets; CR 701.26 tap; CR 605.1a/605.3b mana abilities; CR 603.6a enters
//! triggers; CR 701.21a sacrifice; CR 400.7 and 406.1 graveyard-to-exile movement.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::ChoiceKind;

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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn hand_size(e: &GameEngine, player: usize) -> usize {
    e.state.players[player].hand.len()
}

fn cast_from_hand(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    mana: ManaGift,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) -> u32 {
    let id = inject_card_into_hand(e, player, card);
    give_mana(e, player as i32, mana);
    let slot = hand_index_for_card(e, player, card);
    semantic::accepted(e, player as i32, &cast_spell(slot, targets));
    id
}

fn cast_and_resolve(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    mana: ManaGift,
) -> (u32, tricerules_proto::ruled::v1::RuledEventBatch) {
    let id = cast_from_hand(e, player, card, mana, vec![]);
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    semantic::accepted(e, first, &pass());
    let batch = semantic::accepted(e, second, &pass());
    (id, batch)
}

fn activate(
    e: &mut GameEngine,
    player: i32,
    permanent: u32,
    ability: u32,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) {
    semantic::accepted(
        e,
        player,
        &activate_ability_for(e, permanent, ability, targets),
    );
}

fn mana_activate(e: &mut GameEngine, player: i32, permanent: u32, option: u32) {
    let mut command = activate_ability_for(e, permanent, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    semantic::accepted(e, player, &command);
}

fn resolve_top_stack(e: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    semantic::accepted(e, first, &pass());
    semantic::accepted(e, second, &pass())
}

// ── 01 draw two and lose two ────────────────────────────────────────────────

#[test]
fn q69_draw_two_life_canary_simplest_nights_whisper_draws_two_and_loses_two() {
    let mut e = engine(69_010_001, "nights_whisper");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "cloud_spirit", "island"]);
    let hand_before = hand_size(&e, 0);
    let library_before = e.state.players[0].library.len();

    let (_, batch) = cast_and_resolve(
        &mut e,
        0,
        "nights_whisper",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    assert!(
        find_resolution_choice(&batch).is_none(),
        "Night's Whisper has no choice"
    );
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{1}}{{B}}");
    assert_eq!(e.state.players[0].life, 18, "printed lose 2 life");
    assert_eq!(
        hand_size(&e, 0),
        hand_before + 2,
        "inject and cast one, draw two"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 2,
        "two cards left the library"
    );
    for card in &top[..2] {
        assert_eq!(e.state.objects[card].zone, Zone::Hand);
    }
    assert_eq!(e.state.objects[&top[2]].zone, Zone::Library);
    assert_eq!(e.state.stack.len(), 0);
}

#[test]
fn q69_draw_two_life_canary_hardest_read_the_bones_scries_then_draws_and_loses_two() {
    let mut e = engine(69_010_002, "read_the_bones");
    let top = seat_on_top(
        &mut e,
        0,
        &[
            "grizzly_bears",
            "cloud_spirit",
            "island",
            "savannah_lions",
            "mountain",
        ],
    );
    let library_before = e.state.players[0].library.len();

    let (_, batch) = cast_and_resolve(
        &mut e,
        0,
        "read_the_bones",
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let choice = find_resolution_choice(&batch).expect("scry parks");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.min, 0);
    assert_eq!(choice.max, 2);
    assert_eq!(choice.deciding_player_id, e.state.players[0].id);
    assert_eq!(choice.candidate_object_ids, top[..2].to_vec());
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![top[1], top[0]]));
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{2}}{{B}}");
    assert_eq!(e.state.players[0].life, 18, "printed lose 2 life");
    for card in &top[2..4] {
        assert_eq!(
            e.state.objects[card].zone,
            Zone::Hand,
            "the post-scry top two are drawn"
        );
    }
    assert_eq!(
        e.state.objects[&top[4]].zone,
        Zone::Library,
        "only two cards are drawn"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 2,
        "scry does not change library size; draw removes two"
    );
    assert_eq!(e.state.stack.len(), 0);
}

#[test]
fn q69_draw_two_life_demons_due_is_an_instant_with_printed_scry_two() {
    let mut e = engine(69_011_001, "demons_due");
    let top = seat_on_top(
        &mut e,
        0,
        &["grizzly_bears", "cloud_spirit", "island", "savannah_lions"],
    );
    let (_, batch) = cast_and_resolve(
        &mut e,
        0,
        "demons_due",
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let choice = find_resolution_choice(&batch).expect("scry parks");
    assert_eq!(choice.candidate_object_ids, top[..2].to_vec());
    semantic::accepted(&mut e, 0, &submit_resolution_choice(top[..2].to_vec()));
    assert_eq!(e.state.players[0].life, 18);
    assert_eq!(e.state.objects[&top[2]].zone, Zone::Hand);
    assert_eq!(e.state.objects[&top[3]].zone, Zone::Hand);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q69_draw_two_life_cards_require_their_exact_mana_and_leave_state_unchanged() {
    for (index, card) in ["nights_whisper", "demons_due", "read_the_bones"]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(69_012_000 + index as u64, card);
        let top = seat_on_top(&mut e, 0, &["grizzly_bears", "cloud_spirit"]);
        let hand_before = hand_size(&e, 0);
        let library_before = e.state.players[0].library.len();
        inject_card_into_hand(&mut e, 0, card);
        let slot = hand_index_for_card(&e, 0, card);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{card}: the printed mana is required"
        );
        assert_eq!(hand_size(&e, 0), hand_before + 1, "{card}: no card is cast");
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{card}: no card is drawn"
        );
        assert_eq!(e.state.players[0].life, 20, "{card}: no life is lost");
        assert!(e.state.stack.is_empty(), "{card}: no spell is on the stack");
        for oid in &top {
            assert_eq!(e.state.objects[oid].zone, Zone::Library);
        }
    }
}

// ── 02 two-ability Wizards ──────────────────────────────────────────────────

#[test]
fn q69_guildmage_canary_simplest_armorer_pumps_both_stats_and_expires() {
    let mut e = engine(69_020_001, "armorer_guildmage");
    let source = move_ready_to_battlefield(&mut e, 0, "armorer_guildmage");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 0, target_object(bear));
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{B}}");
    assert!(e.state.objects[&source].tapped, "the {{T}} cost is paid");
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "the pump waits for resolution"
    );
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(3), Some(2)),
        "printed +1/+0"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "CR 514.2: the pump expires in cleanup"
    );

    let mut e = engine(69_020_002, "armorer_guildmage");
    let source = move_ready_to_battlefield(&mut e, 0, "armorer_guildmage");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 1, target_object(bear));
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{G}}");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(3)),
        "printed +0/+1"
    );
}

#[test]
fn q69_guildmage_canary_hardest_senate_gains_life_and_loots_privately() {
    let mut e = engine(69_021_001, "senate_guildmage");
    let source = move_ready_to_battlefield(&mut e, 0, "senate_guildmage");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 0, vec![]);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{W}}");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 22, "printed gain 2 life");

    let mut e = engine(69_021_002, "senate_guildmage");
    let source = move_ready_to_battlefield(&mut e, 0, "senate_guildmage");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "cloud_spirit"]);
    let hand_before = hand_size(&e, 0);
    let opponent_hand = hand_size(&e, 1);
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 1, vec![]);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{U}}");
    resolve_entire_stack_two_player(&mut e);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the mandatory discard parks")
        .presentation
        .clone();
    assert_eq!(pending.choice_kind, ChoiceKind::HandCards);
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .expect("pending discard")
            .deciding_player,
        e.state.players[0].id
    );
    assert_eq!(
        hand_size(&e, 0),
        hand_before + 1,
        "the draw is already in hand"
    );
    assert_eq!(
        e.state.objects[&top[0]].zone,
        Zone::Hand,
        "the drawn card is the top card"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 1,
        "exactly one card is drawn"
    );
    let discarded = pending.candidates[0];
    assert!(
        e.state.players[0].hand.contains(&discarded),
        "only a card from the controller's hand may be discarded"
    );
    assert!(
        !e.state.players[1].hand.contains(&discarded),
        "the opponent's hand is private and unavailable"
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![discarded]));
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        hand_size(&e, 0),
        hand_before,
        "draw one then discard one leaves the hand size unchanged"
    );
    assert_eq!(hand_size(&e, 1), opponent_hand, "opponent hand unchanged");
    assert_eq!(
        e.state.objects[&discarded].zone,
        Zone::Graveyard,
        "the chosen card is discarded"
    );
}

#[test]
fn q69_guildmage_bant_grants_trample_and_flying_until_cleanup() {
    for (ability, keyword, mana) in [
        (
            0u32,
            Keyword::Trample,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        ),
        (
            1u32,
            Keyword::Flying,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        ),
    ] {
        let mut e = engine(69_022_000 + ability as u64, "bant_battlemage");
        let source = move_ready_to_battlefield(&mut e, 0, "bant_battlemage");
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert!(!e.effective_has_keyword(bear, keyword));
        give_mana(&mut e, 0, mana);
        activate(&mut e, 0, source, ability, target_object(bear));
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.effective_has_keyword(bear, keyword),
            "ability {ability} grants the printed keyword"
        );
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(bear, keyword),
            "the grant expires in cleanup"
        );
    }
}

#[test]
fn q69_guildmage_naya_pumps_and_taps_the_chosen_creature() {
    let mut e = engine(69_023_001, "naya_battlemage");
    let source = move_ready_to_battlefield(&mut e, 0, "naya_battlemage");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 0, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(4), "printed +2/+0");
    assert_eq!(e.effective_toughness(bear), Some(2));

    let mut e = engine(69_023_002, "naya_battlemage");
    let source = move_ready_to_battlefield(&mut e, 0, "naya_battlemage");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    assert!(!e.state.objects[&bear].tapped);
    activate(&mut e, 0, source, 1, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&bear].tapped, "printed tap target creature");
}

#[test]
fn q69_guildmage_shaper_grants_first_strike_and_pumps() {
    let mut e = engine(69_024_001, "shaper_guildmage");
    let source = move_ready_to_battlefield(&mut e, 0, "shaper_guildmage");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 0, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.effective_has_keyword(bear, Keyword::FirstStrike),
        "shaper_guildmage: first ability grants first strike"
    );

    let mut e = engine(69_024_002, "shaper_guildmage");
    let source = move_ready_to_battlefield(&mut e, 0, "shaper_guildmage");
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 1, target_object(bear));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(3), "printed +1/+0");
    assert_eq!(e.effective_toughness(bear), Some(2));
}

#[test]
fn q69_guildmage_cards_reject_unfunded_wrong_target_and_tapped_source() {
    for (index, (id, first_mana, second_mana, targets_creature)) in [
        ("armorer_guildmage", "b", "g", true),
        ("bant_battlemage", "g", "u", true),
        ("naya_battlemage", "r", "w", true),
        ("senate_guildmage", "w", "u", false),
        ("shaper_guildmage", "w", "b", true),
    ]
    .into_iter()
    .enumerate()
    {
        let mana = |value: &str| match value {
            "w" => ManaGift {
                w: 1,
                ..Default::default()
            },
            "u" => ManaGift {
                u: 1,
                ..Default::default()
            },
            "b" => ManaGift {
                b: 1,
                ..Default::default()
            },
            "r" => ManaGift {
                r: 1,
                ..Default::default()
            },
            "g" => ManaGift {
                g: 1,
                ..Default::default()
            },
            other => panic!("unexpected mana {other}"),
        };
        let targets = |oid: u32| {
            if targets_creature {
                target_object(oid)
            } else {
                vec![]
            }
        };

        let mut e = engine(69_025_000 + index as u64 * 10, id);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, targets(bear)))
                .is_err(),
            "{id}: the printed mana is required"
        );
        assert_eq!(e.state.command_index, command_index, "{id}");
        assert!(
            !e.state.objects[&source].tapped,
            "{id}: no tap cost is paid"
        );
        assert!(e.state.stack.is_empty(), "{id}");
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{id}");

        let wrong = match first_mana {
            "w" => "u",
            "u" => "b",
            "b" => "r",
            "r" => "g",
            "g" => "w",
            other => panic!("unexpected mana {other}"),
        };
        let mut e = engine(69_025_003 + index as u64 * 10, id);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, mana(wrong));
        let pool_before = pool(&e, 0);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, targets(bear)))
                .is_err(),
            "{id}: the printed color is required"
        );
        assert_eq!(e.state.command_index, command_index, "{id}");
        assert!(
            !e.state.objects[&source].tapped,
            "{id}: no tap cost is paid for the wrong color"
        );
        assert_eq!(
            pool(&e, 0),
            pool_before,
            "{id}: the wrong color is not spent"
        );

        if targets_creature {
            let mut e = engine(69_025_001 + index as u64 * 10, id);
            let source = move_ready_to_battlefield(&mut e, 0, id);
            let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
            give_mana(&mut e, 0, mana(first_mana));
            let pool_before = pool(&e, 0);
            let command_index = e.state.command_index;
            assert!(
                e.apply_scenario_command(
                    0,
                    &activate_ability_for(&e, source, 0, target_object(land))
                )
                .is_err(),
                "{id}: a land is not a creature target"
            );
            assert_eq!(e.state.command_index, command_index, "{id}");
            assert!(
                !e.state.objects[&source].tapped,
                "{id}: no tap cost is paid"
            );
            assert_eq!(pool(&e, 0), pool_before, "{id}: no mana is spent");
        }

        let mut e = engine(69_025_002 + index as u64 * 10, id);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, mana(first_mana));
        activate(&mut e, 0, source, 0, targets(bear));
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.objects[&source].tapped,
            "{id}: first ability tapped"
        );
        give_mana(&mut e, 0, mana(second_mana));
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 1, targets(bear)))
                .is_err(),
            "{id}: a tapped source cannot pay a second tap cost"
        );
        assert_eq!(e.state.command_index, command_index, "{id}");
    }
}

// ── 03 any-color mana rocks ─────────────────────────────────────────────────

#[test]
fn q69_rocks_canary_simplest_mana_geode_enters_scries_one_then_makes_any_color() {
    let mut e = engine(69_030_001, "mana_geode");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "cloud_spirit"]);
    let source = move_ready_to_battlefield(&mut e, 0, "mana_geode");
    let library_at_choice = e.state.players[0].library.len();
    let batch = resolve_top_stack(&mut e);
    let choice = find_resolution_choice(&batch).expect("the enters trigger parks a scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.min, 0);
    assert_eq!(choice.max, 1);
    assert_eq!(choice.candidate_object_ids, top[..1].to_vec());
    assert_eq!(choice.deciding_player_id, e.state.players[0].id);
    semantic::accepted(&mut e, 0, &submit_resolution_choice(top[..1].to_vec()));
    assert_eq!(
        e.state.players[0].library.back(),
        Some(&top[0]),
        "the chosen card goes to the bottom"
    );
    assert_eq!(e.state.players[0].library.len(), library_at_choice);

    mana_activate(&mut e, 0, source, 0);
    assert_eq!(
        pool(&e, 0),
        (1, 0, 0, 0, 0, 0),
        "the chosen white option is produced"
    );
    assert!(e.state.objects[&source].tapped, "the {{T}} cost is paid");
    assert!(e.state.stack.is_empty(), "CR 605.3b: mana ability");
}

#[test]
fn q69_rocks_canary_hardest_honored_heirloom_exiles_a_graveyard_card() {
    let mut e = engine(69_031_001, "honored_heirloom");
    let source = move_ready_to_battlefield(&mut e, 0, "honored_heirloom");
    let card = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 1, target_object(card));
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{2}}");
    assert!(e.state.objects[&source].tapped);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Exile,
        "the target card is exiled"
    );
    assert_eq!(e.state.players[1].graveyard.len(), 0);

    let mut e = engine(69_031_002, "honored_heirloom");
    let source = move_ready_to_battlefield(&mut e, 0, "honored_heirloom");
    let card = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    let other = inject_graveyard_card(&mut e, 1, "cloud_spirit");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 1, target_object(card));
    e.state.players[1].graveyard.retain(|oid| *oid != card);
    e.state.players[1].hand.push(card);
    e.state.objects.get_mut(&card).expect("object").zone = Zone::Hand;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Hand,
        "the stale target is not exiled"
    );
    assert_eq!(
        e.state.objects[&other].zone,
        Zone::Graveyard,
        "no other card is exiled"
    );
    assert_eq!(e.state.players[1].graveyard, vec![other]);

    let mut e = engine(69_031_003, "honored_heirloom");
    let source = move_ready_to_battlefield(&mut e, 0, "honored_heirloom");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 1, target_object(bear)))
            .is_err(),
        "a battlefield permanent is not a graveyard card"
    );
    assert!(!e.state.objects[&source].tapped);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 2), "no mana is spent");
}

#[test]
fn q69_rocks_each_card_offers_its_printed_any_color_option() {
    for (index, (id, option, expected)) in [
        ("mana_geode", 0u32, (1, 0, 0, 0, 0, 0)),
        ("bronze_walrus", 1, (0, 1, 0, 0, 0, 0)),
        ("spinning_wheel", 2, (0, 0, 1, 0, 0, 0)),
        ("honored_heirloom", 3, (0, 0, 0, 1, 0, 0)),
        ("letter_of_acceptance", 4, (0, 0, 0, 0, 1, 0)),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(69_032_000 + index as u64, id);
        let top = seat_on_top(&mut e, 0, &["grizzly_bears", "cloud_spirit"]);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        if id == "mana_geode" || id == "bronze_walrus" {
            let batch = resolve_top_stack(&mut e);
            let choice = find_resolution_choice(&batch).expect("enters scry parks");
            assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
            semantic::accepted(&mut e, 0, &submit_resolution_choice(top[..1].to_vec()));
        }
        mana_activate(&mut e, 0, source, option);
        assert_eq!(pool(&e, 0), expected, "{id}: printed option {option}");
        assert!(e.state.objects[&source].tapped, "{id}");
    }
}

#[test]
fn q69_rocks_bronze_walrus_scries_two_on_entry() {
    let mut e = engine(69_033_001, "bronze_walrus");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "cloud_spirit", "island"]);
    let source = move_ready_to_battlefield(&mut e, 0, "bronze_walrus");
    assert_eq!(e.effective_power(source), Some(2));
    assert_eq!(e.effective_toughness(source), Some(2));
    let library_at_choice = e.state.players[0].library.len();
    let batch = resolve_top_stack(&mut e);
    let choice = find_resolution_choice(&batch).expect("scry 2 parks");
    assert_eq!(choice.max, 2);
    assert_eq!(choice.candidate_object_ids, top[..2].to_vec());
    semantic::accepted(&mut e, 0, &submit_resolution_choice(top[..2].to_vec()));
    assert_eq!(e.state.players[0].library.len(), library_at_choice);
    assert_eq!(e.state.players[0].library.back(), Some(&top[1]));
}

#[test]
fn q69_rocks_spinning_wheel_taps_a_target_creature() {
    let mut e = engine(69_034_001, "spinning_wheel");
    let source = move_ready_to_battlefield(&mut e, 0, "spinning_wheel");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 1, target_object(bear));
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{5}}");
    assert!(e.state.objects[&source].tapped);
    assert!(!e.state.objects[&bear].tapped, "tap waits for resolution");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&bear].tapped, "printed tap target creature");

    let mut e = engine(69_034_002, "spinning_wheel");
    let source = move_ready_to_battlefield(&mut e, 0, "spinning_wheel");
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 1, target_object(land)))
            .is_err(),
        "a land is not a creature target"
    );
    assert!(!e.state.objects[&source].tapped);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 5), "no mana is spent");
}

#[test]
fn q69_rocks_letter_of_acceptance_sacrifices_itself_to_draw() {
    let mut e = engine(69_035_001, "letter_of_acceptance");
    let source = move_ready_to_battlefield(&mut e, 0, "letter_of_acceptance");
    let top = seat_on_top(&mut e, 0, &["grizzly_bears", "cloud_spirit"]);
    let hand_before = hand_size(&e, 0);
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    activate(&mut e, 0, source, 1, vec![]);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed {{2}}");
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Graveyard,
        "sacrificed as a cost"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(hand_size(&e, 0), hand_before + 1, "printed draw a card");
    assert_eq!(e.state.players[0].library.len(), library_before - 1);
    assert_eq!(e.state.objects[&top[0]].zone, Zone::Hand);
}

#[test]
fn q69_rocks_mana_abilities_reject_a_tapped_source_without_producing() {
    for (index, id) in [
        "mana_geode",
        "bronze_walrus",
        "spinning_wheel",
        "honored_heirloom",
        "letter_of_acceptance",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(69_036_000 + index as u64, id);
        let source = move_ready_to_battlefield(&mut e, 0, id);
        if id == "mana_geode" || id == "bronze_walrus" {
            let batch = resolve_top_stack(&mut e);
            let choice = find_resolution_choice(&batch).expect("enters scry parks");
            semantic::accepted(
                &mut e,
                0,
                &submit_resolution_choice(choice.candidate_object_ids[..1].to_vec()),
            );
        }
        e.state.objects.get_mut(&source).expect("source").tapped = true;
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{id}: a tapped source cannot pay {{T}}"
        );
        assert_eq!(e.state.command_index, command_index, "{id}");
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{id}: no mana is produced");
    }
}
