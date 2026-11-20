//! Actual-card scenarios for the queue-v49 reviewed cohorts (batch B).
//!
//! Expectations are printed Oracle values. CR 702.8 governs flash, CR 303.4/303.4b
//! Aura attachment, CR 613.4c/613.1f the attached pump and keyword, CR 603.6a
//! landfall entry triggers, CR 603.2 becomes-tapped triggers, and CR 514.2 cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{ChoiceKind, ChooseTriggerTarget, RuledCommand};

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

fn cast_named(
    e: &mut GameEngine,
    player: i32,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) {
    inject_card_into_hand(e, player as usize, card);
    give_mana(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    e.apply_scenario_command(player, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
}

fn cast_aura_on(e: &mut GameEngine, card: &str, mana: ManaGift, target: u32) {
    cast_named(e, 0, card, mana, target_object(target));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects.get(&target).map(|o| o.zone),
        Some(Zone::Battlefield),
        "{card}: recipient"
    );
}

fn keywords(e: &GameEngine, oid: u32) -> Vec<Keyword> {
    e.characteristics(oid).expect("characteristics").keywords
}

fn choose_target(oid: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(oid),
        })),
    }
}

fn to_opponent_main1(e: &mut GameEngine) {
    end_active_turn(e, 0);
    for _ in 0..8 {
        if e.state.turn_step == TurnStep::Main1 {
            break;
        }
        pass_priority_round(e);
    }
    assert_eq!(e.state.turn_step, TurnStep::Main1, "opponent main 1");
    assert_eq!(e.state.active_player_id(), 1, "opponent is active");
}

fn play_one_land(e: &mut GameEngine, player: usize) {
    ensure_card_in_hand(e, player, "forest");
    let slot = hand_index_for_card(e, player, "forest");
    e.apply_scenario_command(player as i32, &play_land(slot))
        .expect("play a land");
    resolve_entire_stack_two_player(e);
}

fn tap_with_terashis_cry(e: &mut GameEngine, card: u32, trigger_target: Option<u32>) -> bool {
    inject_card_into_hand(e, 0, "terashis_cry");
    give_mana(
        e,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "terashis_cry");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(card)))
        .expect("cast Terashi's Cry");
    e.apply_scenario_command(0, &pass()).expect("caster pass");
    e.apply_scenario_command(1, &pass())
        .expect("opponent pass resolves the tap");
    if let Some(target) = trigger_target {
        e.apply_scenario_command(0, &choose_target(target))
            .expect("choose the becomes-tapped trigger target");
    }
    resolve_trigger_with_optional_scry(e)
}

fn resolve_trigger_with_optional_scry(e: &mut GameEngine) -> bool {
    let mut scry_seen = false;
    for _ in 0..8 {
        answer_trigger_order_in_engine_order(e);
        if e.state.stack.is_empty() {
            return scry_seen;
        }
        let batch = pass_both_players_with_batch(e);
        if let Some(choice) = find_resolution_choice(&batch) {
            assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
            assert_eq!((choice.min, choice.max), (0, 1));
            assert_eq!(choice.deciding_player_id, 0);
            scry_seen = true;
            let keep = choice
                .candidate_object_ids
                .first()
                .copied()
                .expect("scry candidate");
            e.apply_scenario_command(0, &submit_resolution_choice(vec![keep]))
                .expect("submit the scry choice");
        }
    }
    panic!("the becomes-tapped trigger did not resolve");
}

// ── 04 Flash Aura pump plus keyword ────────────────────────────────────────

#[test]
fn q49_flash_aura_canary_simplest_flaming_sword_pumps_and_grants_first_strike() {
    let mut e = main1_engine(49_040_001, &["flaming_sword"], &[]);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_aura_on(
        &mut e,
        "flaming_sword",
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
        bear,
    );
    assert_eq!(e.effective_power(bear), Some(3));
    assert_eq!(e.effective_toughness(bear), Some(2));
    assert!(keywords(&e, bear).contains(&Keyword::FirstStrike));
}

#[test]
fn q49_flash_aura_canary_hardest_spiteful_motives_casts_at_instant_timing() {
    let mut e = main1_engine(49_040_002, &["spiteful_motives", "lance"], &[]);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    to_opponent_main1(&mut e);
    e.apply_scenario_command(1, &pass())
        .expect("active player passes priority to the nonactive player");
    inject_card_into_hand(&mut e, 0, "lance");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "lance");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "a non-Flash Aura cannot be cast during the opponent's main phase"
    );
    cast_named(
        &mut e,
        0,
        "spiteful_motives",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(bear), Some(5));
    assert_eq!(e.effective_toughness(bear), Some(2));
    assert!(keywords(&e, bear).contains(&Keyword::FirstStrike));
}

#[test]
fn q49_flash_aura_cards_apply_their_printed_pump_and_keyword() {
    struct Case {
        id: &'static str,
        mana: ManaGift,
        pump: (i32, i32),
        keyword: Keyword,
    }
    for (index, case) in [
        Case {
            id: "epic_proportions",
            mana: ManaGift {
                g: 2,
                c: 4,
                ..Default::default()
            },
            pump: (5, 5),
            keyword: Keyword::Trample,
        },
        Case {
            id: "flaming_sword",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            pump: (1, 0),
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "spiteful_motives",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            pump: (3, 0),
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "tiger_claws",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            pump: (1, 1),
            keyword: Keyword::Trample,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_041_000 + index as u64, &[case.id], &[]);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 1);
        cast_aura_on(&mut e, case.id, gift(&case.mana), bear);
        assert_eq!(
            e.effective_power(bear),
            Some((2 + case.pump.0) as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bear),
            Some((2 + case.pump.1) as u32),
            "{}",
            case.id
        );
        assert!(keywords(&e, bear).contains(&case.keyword), "{}", case.id);
        assert_eq!(e.effective_power(bystander), Some(2), "{}", case.id);
        assert_eq!(e.effective_toughness(bystander), Some(1), "{}", case.id);
        assert!(
            !keywords(&e, bystander).contains(&case.keyword),
            "{}: only the enchanted creature",
            case.id
        );
        let aura = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(bear)),
            "{}: attached",
            case.id
        );
    }
}

#[test]
fn q49_flash_aura_spell_shape_rejects_noncreature_targets_and_fizzles_on_stale_target() {
    let cases = [
        (
            "epic_proportions",
            ManaGift {
                g: 2,
                c: 4,
                ..Default::default()
            },
        ),
        (
            "flaming_sword",
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "spiteful_motives",
            ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "tiger_claws",
            ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        ),
    ];
    for (index, (id, mana)) in cases.into_iter().enumerate() {
        let mut e = main1_engine(49_042_000 + index as u64 * 2, &[id], &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        let _ = inject_card_into_hand(&mut e, 0, id);
        give_mana(&mut e, 0, gift(&mana));
        let slot = hand_index_for_card(&e, 0, id);
        let aura = e.state.players[0].hand[slot];
        let hand_before = e.state.players[0].hand.len();
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{id}: a noncreature is not an Enchant creature target"
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{id}: hand");
        assert_eq!(e.state.objects[&aura].zone, Zone::Hand, "{id}: aura hand");
        assert!(e.state.stack.is_empty(), "{id}: stack");

        let mut e = main1_engine(49_042_001 + index as u64 * 2, &[id], &[]);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&mana));
        let _ = inject_card_into_hand(&mut e, 0, id);
        let slot = hand_index_for_card(&e, 0, id);
        let aura = e.state.players[0].hand[slot];
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .unwrap_or_else(|error| panic!("{id}: cast the Aura: {error:?}"));
        cast_named(
            &mut e,
            0,
            "go_for_the_throat",
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            target_object(creature),
        );
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Stack,
            "{id}: the Aura is still on the stack after the target dies"
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Graveyard,
            "{id}: the Aura fizzles when its target is gone"
        );
    }
}

// ── 05 Landfall simple ─────────────────────────────────────────────────────

#[test]
fn q49_landfall_canary_simplest_kazandu_nectarpot_gains_life() {
    let mut e = main1_engine(49_050_001, &["kazandu_nectarpot"], &[]);
    move_ready_to_battlefield(&mut e, 0, "kazandu_nectarpot");
    assert_eq!(e.state.players[0].life, 20);
    play_one_land(&mut e, 0);
    assert_eq!(e.state.players[0].life, 21);
}

#[test]
fn q49_landfall_canary_hardest_baloth_woodcrasher_pumps_and_tramples() {
    let mut e = main1_engine(49_050_002, &["baloth_woodcrasher"], &[]);
    let baloth = move_ready_to_battlefield(&mut e, 0, "baloth_woodcrasher");
    assert_eq!(e.effective_power(baloth), Some(4));
    assert_eq!(e.effective_toughness(baloth), Some(4));
    assert!(!keywords(&e, baloth).contains(&Keyword::Trample));
    play_one_land(&mut e, 0);
    assert_eq!(e.effective_power(baloth), Some(8));
    assert_eq!(e.effective_toughness(baloth), Some(8));
    assert!(keywords(&e, baloth).contains(&Keyword::Trample));
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(baloth), Some(4), "CR 514.2");
    assert_eq!(e.effective_toughness(baloth), Some(4));
    assert!(!keywords(&e, baloth).contains(&Keyword::Trample));
}

#[test]
fn q49_landfall_cards_apply_their_printed_effect() {
    struct Case {
        id: &'static str,
        own_life: i32,
        opposing_life: i32,
        own_creature: Option<(u32, u32, bool)>,
    }
    for (index, case) in [
        Case {
            id: "baloth_woodcrasher",
            own_life: 20,
            opposing_life: 20,
            own_creature: Some((8, 8, true)),
        },
        Case {
            id: "canyon_jerboa",
            own_life: 20,
            opposing_life: 20,
            own_creature: Some((3, 2, false)),
        },
        Case {
            id: "kazandu_nectarpot",
            own_life: 21,
            opposing_life: 20,
            own_creature: None,
        },
        Case {
            id: "tunneling_geopede",
            own_life: 20,
            opposing_life: 19,
            own_creature: None,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_051_000 + index as u64, &[case.id], &["grizzly_bears"]);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let own_bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 1);
        let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        play_one_land(&mut e, 0);
        assert_eq!(e.state.players[0].life, case.own_life, "{}", case.id);
        assert_eq!(e.state.players[1].life, case.opposing_life, "{}", case.id);
        if let Some((power, toughness, trample)) = case.own_creature {
            let body = if case.id == "canyon_jerboa" {
                own_bystander
            } else {
                source
            };
            assert_eq!(e.effective_power(body), Some(power), "{}", case.id);
            assert_eq!(e.effective_toughness(body), Some(toughness), "{}", case.id);
            assert_eq!(
                keywords(&e, body).contains(&Keyword::Trample),
                trample,
                "{}",
                case.id
            );
        }
        if case.id == "baloth_woodcrasher" {
            assert_eq!(e.effective_power(own_bystander), Some(2), "{}", case.id);
            assert_eq!(e.effective_toughness(own_bystander), Some(1), "{}", case.id);
        }
        if case.id == "canyon_jerboa" {
            assert_eq!(e.effective_power(source), Some(2), "{}", case.id);
            assert_eq!(e.effective_toughness(source), Some(3), "{}", case.id);
        }
        assert_eq!(e.effective_power(opposing), Some(2), "{}", case.id);
        assert_eq!(e.effective_toughness(opposing), Some(2), "{}", case.id);
    }
}

#[test]
fn q49_landfall_nonland_entry_does_not_trigger() {
    let mut e = main1_engine(49_052_001, &["kazandu_nectarpot"], &[]);
    move_ready_to_battlefield(&mut e, 0, "kazandu_nectarpot");
    cast_named(
        &mut e,
        0,
        "grizzly_bears",
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].life, 20,
        "a creature entry is not landfall"
    );
    play_one_land(&mut e, 0);
    assert_eq!(e.state.players[0].life, 21);
}

// ── 09 Becomes tapped triggers ─────────────────────────────────────────────

#[test]
fn q49_becomes_tapped_canary_simplest_spireside_infiltrator_damages_each_opponent() {
    let mut e = main1_engine(49_090_001, &["spireside_infiltrator"], &[]);
    let source = move_ready_to_battlefield(&mut e, 0, "spireside_infiltrator");
    assert_eq!(e.state.players[1].life, 20);
    assert!(!tap_with_terashis_cry(&mut e, source, None));
    assert!(e.state.objects[&source].tapped);
    assert_eq!(e.state.players[1].life, 19);
}

#[test]
fn q49_becomes_tapped_canary_hardest_goblin_medics_targets_damage() {
    let mut e = main1_engine(49_090_002, &["goblin_medics"], &["grizzly_bears"]);
    let source = move_ready_to_battlefield(&mut e, 0, "goblin_medics");
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    assert!(!tap_with_terashis_cry(&mut e, source, Some(target)));
    assert!(e.state.objects[&source].tapped);
    assert_eq!(e.state.objects[&target].damage, 1);
}

#[test]
fn q49_becomes_tapped_cards_apply_their_printed_effect() {
    struct Case {
        id: &'static str,
        targeted: bool,
        own_life: i32,
        opposing_life: i32,
        scry: bool,
    }
    for (index, case) in [
        Case {
            id: "goblin_medics",
            targeted: true,
            own_life: 20,
            opposing_life: 20,
            scry: false,
        },
        Case {
            id: "moonfolk_puzzlemaker",
            targeted: false,
            own_life: 20,
            opposing_life: 20,
            scry: true,
        },
        Case {
            id: "night_market_lookout",
            targeted: false,
            own_life: 21,
            opposing_life: 19,
            scry: false,
        },
        Case {
            id: "samite_herbalist",
            targeted: false,
            own_life: 21,
            opposing_life: 20,
            scry: true,
        },
        Case {
            id: "spireside_infiltrator",
            targeted: false,
            own_life: 20,
            opposing_life: 19,
            scry: false,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_091_000 + index as u64, &[case.id], &["grizzly_bears"]);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        seed_library(&mut e, 0, &["island", "mountain"]);
        let library_before = e.state.players[0].library.len();
        let scry_seen = tap_with_terashis_cry(&mut e, source, case.targeted.then_some(target));
        assert_eq!(scry_seen, case.scry, "{}: scry choice", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(e.state.players[0].life, case.own_life, "{}", case.id);
        assert_eq!(e.state.players[1].life, case.opposing_life, "{}", case.id);
        if case.targeted {
            assert_eq!(e.state.objects[&target].damage, 1, "{}", case.id);
        }
        if case.id == "moonfolk_puzzlemaker" {
            assert!(
                e.effective_has_keyword(source, Keyword::Flying),
                "{}: flying",
                case.id
            );
        }
        if case.scry {
            assert_eq!(
                e.state.players[0].library.len(),
                library_before,
                "{}: scry does not draw",
                case.id
            );
        }
    }
}

fn seed_library(e: &mut GameEngine, player: usize, cards: &[&str]) {
    for card in cards {
        inject_library_card(e, player, card);
    }
}
