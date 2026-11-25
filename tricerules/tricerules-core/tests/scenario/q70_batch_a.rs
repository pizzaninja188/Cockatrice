//! Actual-card scenarios for the queue-v70 batch A cohorts.
//!
//! Three stable typed shapes: a one-shot pump with flashback, a tap-and-skip-untap spell with one
//! rider, and a land with one enters-the-battlefield trigger plus one mana ability. Expectations
//! are printed Oracle values. CR 115.1/601.2c choose targets at cast; CR 611.2a/613.4c/514.2 govern
//! pumps and keyword grants; CR 702.34a-d govern flashback's alternative cost and exile; CR 701.26
//! taps; CR 502.3/611.2a skip the next untap step; CR 701.16 creates a Clue; CR 702.29a-b govern
//! cycling; CR 121/701.9 govern draw then discard; CR 603.6a/614.1d/614.12 govern land entry;
//! CR 701.22a-b govern scry; CR 605.1a/605.3b govern mana abilities; CR 608.2b fizzles stale
//! targets.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, ActivateAbility, CastMethod, CastSpell, ChoiceKind, RuledCommand,
};

fn engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("island", opposing),
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

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn resolve_top_two_player(e: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    semantic::accepted(e, first, &pass());
    semantic::accepted(e, second, &pass())
}

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

// ── 01 flashback pump ──────────────────────────────────────────────────────

struct PumpCase {
    id: &'static str,
    mana: ManaGift,
    flashback: ManaGift,
    power: u32,
    toughness: u32,
    keyword: Option<Keyword>,
    card_type: &'static str,
}

fn pump_cases() -> [PumpCase; 4] {
    [
        PumpCase {
            id: "reckless_charge",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 0,
            keyword: Some(Keyword::Haste),
            card_type: "Sorcery",
        },
        PumpCase {
            id: "sylvan_might",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                g: 2,
                c: 2,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            keyword: Some(Keyword::Trample),
            card_type: "Instant",
        },
        PumpCase {
            id: "thrill_of_the_hunt",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 2,
            keyword: None,
            card_type: "Instant",
        },
        PumpCase {
            id: "wild_hunger",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            flashback: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 1,
            keyword: Some(Keyword::Trample),
            card_type: "Instant",
        },
    ]
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

fn assert_pump(e: &GameEngine, target: u32, case: &PumpCase, label: &str, casts: u32) {
    assert_eq!(
        e.effective_power(target),
        Some(2 + casts * case.power),
        "{}: {label} power",
        case.id
    );
    assert_eq!(
        e.effective_toughness(target),
        Some(2 + casts * case.toughness),
        "{}: {label} toughness",
        case.id
    );
    match case.keyword {
        Some(keyword) => assert!(
            e.effective_has_keyword(target, keyword),
            "{}: {label} {keyword:?}",
            case.id
        ),
        None => assert!(
            !e.effective_has_keyword(target, Keyword::Trample),
            "{}: {label} no keyword",
            case.id
        ),
    }
}

#[test]
fn q70_flashback_pump_canary_simplest_reckless_charge_pumps_and_exiles() {
    let mut e = engine(70_010_001, &[], &["grizzly_bears"]);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    let case = &pump_cases()[0];
    give_mana(&mut e, 0, gift(&case.mana));
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Reckless Charge");
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed mana");
    resolve_entire_stack_two_player(&mut e);
    assert_pump(&e, target, case, "hand cast", 1);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, target, generation))
        .expect("flashback Reckless Charge");
    resolve_entire_stack_two_player(&mut e);
    assert_pump(&e, target, case, "flashback cast", 2);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "CR 702.34a exiles the flashback spell"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(target), Some(2), "CR 514.2 expiry");
    assert_eq!(e.effective_toughness(target), Some(2));
    assert!(!e.effective_has_keyword(target, Keyword::Haste));
}

#[test]
fn q70_flashback_pump_canary_hardest_wild_hunger_uses_off_color_flashback_and_exiles() {
    let mut e = engine(70_010_002, &[], &["grizzly_bears"]);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    let case = &pump_cases()[3];
    give_mana(&mut e, 0, gift(&case.mana));
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Wild Hunger");
    resolve_entire_stack_two_player(&mut e);
    assert_pump(&e, target, case, "hand cast", 1);

    let generation = e.state.zone_change_generation[&spell];
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &flashback_cast(spell, target, generation))
            .is_err(),
        "the red flashback cost cannot be paid with green mana"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    let pool_before = pool(&e, 0);
    give_mana(&mut e, 0, gift(&case.flashback));
    assert_ne!(pool(&e, 0), pool_before);
    e.apply_scenario_command(0, &flashback_cast(spell, target, generation))
        .expect("flashback Wild Hunger with its printed red cost");
    resolve_entire_stack_two_player(&mut e);
    assert_pump(&e, target, case, "flashback cast", 2);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}

#[test]
fn q70_flashback_pump_cards_apply_their_printed_pump_and_keyword() {
    for (index, case) in pump_cases().into_iter().enumerate() {
        let mut e = engine(70_011_000 + index as u64, &[], &["grizzly_bears"]);
        let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        let bystander = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        let _spell = inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_pump(&e, target, &case, "printed", 1);
        assert_eq!(
            e.effective_power(bystander),
            Some(2),
            "{}: only target",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bystander),
            Some(2),
            "{}: only target",
            case.id
        );
    }
}

#[test]
fn q70_flashback_pump_cards_flash_back_for_their_printed_cost_and_exile() {
    for (index, case) in pump_cases().into_iter().enumerate() {
        let mut e = engine(70_012_000 + index as u64, &[], &["grizzly_bears"]);
        let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .unwrap_or_else(|error| panic!("{}: first cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let generation = e.state.zone_change_generation[&spell];
        give_mana(&mut e, 0, gift(&case.flashback));
        e.apply_scenario_command(0, &flashback_cast(spell, target, generation))
            .unwrap_or_else(|error| panic!("{}: flashback: {error:?}", case.id));
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed flashback cost",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_pump(&e, target, &case, "flashback", 2);
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Exile,
            "{}: flashback exiles",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &flashback_cast(spell, target, generation))
                .is_err(),
            "{}: exiled cards cannot be flashed back again",
            case.id
        );
    }
}

#[test]
fn q70_flashback_pump_sorcery_respects_timing_while_instants_flash_back_any_time() {
    for (index, case) in pump_cases().into_iter().enumerate() {
        let mut e = engine(70_013_000 + index as u64, &[], &["grizzly_bears"]);
        let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .unwrap_or_else(|error| panic!("{}: first cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let generation = e.state.zone_change_generation[&spell];
        end_active_turn(&mut e, 0);
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.priority_player_id(),
            e.state.players[1].id,
            "{}: opponent's turn",
            case.id
        );
        e.apply_scenario_command(e.state.players[1].id, &pass())
            .expect("opponent passes priority");
        give_mana(&mut e, 0, gift(&case.flashback));
        let command = flashback_cast(spell, target, generation);
        if case.card_type == "Sorcery" {
            assert!(
                e.apply_scenario_command(0, &command).is_err(),
                "{}: a sorcery cannot be flashed back on the opponent's turn",
                case.id
            );
            assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        } else {
            e.apply_scenario_command(0, &command)
                .unwrap_or_else(|error| panic!("{}: instant flashback: {error:?}", case.id));
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(
                e.state.objects[&spell].zone,
                Zone::Exile,
                "{}: instant flashback resolves on the opponent's turn",
                case.id
            );
        }
    }
}

#[test]
fn q70_flashback_pump_cards_fizzle_on_a_stale_target_and_still_exile_after_flashback() {
    let case = &pump_cases()[0];
    let mut e = engine(70_014_001, &[], &["grizzly_bears", "grizzly_bears"]);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    let bystander = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.mana));
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast at the creature");
    move_to_graveyard(&mut e, 1, target);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(bystander),
        Some(2),
        "a fizzled pump does not affect another creature"
    );
    assert_eq!(e.effective_toughness(bystander), Some(2));
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    let generation = e.state.zone_change_generation[&spell];
    let flashback_target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, flashback_target, generation))
        .expect("flashback at the creature");
    move_to_graveyard(&mut e, 1, flashback_target);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "CR 702.34a exiles a fizzled flashback spell"
    );
    assert_eq!(e.effective_power(bystander), Some(2));
}

// ── 02 tap and skip untap ─────────────────────────────────────────────────

fn clue_count(e: &GameEngine) -> usize {
    battlefield_token_oids(e, 0, "clue").len()
}

#[test]
fn q70_tap_skip_canary_simplest_take_into_custody_taps_and_skips_untap() {
    let mut e = engine(70_020_001, &[], &["grizzly_bears"]);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "take_into_custody");
    let slot = hand_index_for_card(&e, 0, "take_into_custody");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Take into Custody");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&target].tapped);
    assert_eq!(e.state.skip_next_untap.len(), 1);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn q70_tap_skip_canary_hardest_snow_day_taps_two_draws_and_discards() {
    let mut e = engine(70_020_002, &[], &[]);
    let first = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let drawn_first = inject_library_card(&mut e, 0, "forest");
    let drawn_second = inject_library_card(&mut e, 0, "island");
    e.state.players[0]
        .library
        .retain(|oid| *oid != drawn_first && *oid != drawn_second);
    e.state.players[0].library.push_front(drawn_second);
    e.state.players[0].library.push_front(drawn_first);
    let hand_before = e.state.players[0].hand.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "snow_day");
    let slot = hand_index_for_card(&e, 0, "snow_day");
    e.apply_scenario_command(
        0,
        &cast_spell(
            slot,
            vec![target_object(first)[0], target_object(second)[0]],
        ),
    )
    .expect("cast Snow Day");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&first].tapped);
    assert!(e.state.objects[&second].tapped);
    assert_eq!(e.state.skip_next_untap.len(), 2);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 2,
        "both cards are drawn before the discard choice"
    );
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("the discard is pending");
    assert!(pending.presentation.candidates.contains(&drawn_first));
    assert!(pending.presentation.candidates.contains(&drawn_second));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![drawn_first]))
        .expect("discard a drawn card");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "draw two, then discard one"
    );
    assert_eq!(e.state.objects[&drawn_first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&drawn_second].zone, Zone::Hand);
}

#[test]
fn q70_tap_skip_press_for_answers_investigates_with_a_legal_target() {
    let mut e = engine(70_021_001, &[], &["grizzly_bears"]);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "press_for_answers");
    let slot = hand_index_for_card(&e, 0, "press_for_answers");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Press for Answers");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&target].tapped);
    assert_eq!(e.state.skip_next_untap.len(), 1);
    assert_eq!(clue_count(&e), 1, "Investigate creates one Clue");
}

#[test]
fn q70_tap_skip_cards_accept_already_tapped_creatures() {
    for (index, card) in ["take_into_custody", "frostveil_ambush"]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(70_022_000 + index as u64, &[], &["grizzly_bears"]);
        let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&target).expect("creature").tapped = true;
        let mana = if card == "take_into_custody" {
            ManaGift {
                u: 1,
                ..Default::default()
            }
        } else {
            ManaGift {
                u: 2,
                c: 3,
                ..Default::default()
            }
        };
        give_mana(&mut e, 0, mana);
        inject_card_into_hand(&mut e, 0, card);
        let slot = hand_index_for_card(&e, 0, card);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .unwrap_or_else(|error| panic!("{card}: cast at a tapped creature: {error:?}"));
        resolve_entire_stack_two_player(&mut e);
        assert!(e.state.objects[&target].tapped, "{card}");
        assert_eq!(e.state.skip_next_untap.len(), 1, "{card}");
    }
}

#[test]
fn q70_tap_skip_frostveil_ambush_cycles_from_hand() {
    let mut e = engine(70_023_001, &[], &[]);
    let source = inject_card_into_hand(&mut e, 0, "frostveil_ambush");
    let hand_before = e.state.players[0].hand.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(&e, source, 0))
        .expect("cycle Frostveil Ambush for {1}");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 1,
        "cycling discards one and draws one"
    );
}

#[test]
fn q70_tap_skip_all_illegal_targets_fizzle_without_riders() {
    // Snow Day's pinned ruling: all-illegal targets remove the spell and skip draw/discard.
    let mut e = engine(70_024_001, &[], &["grizzly_bears"]);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    let hand_before = e.state.players[0].hand.len();
    let library_before = e.state.players[0].library.len();
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "snow_day");
    let slot = hand_index_for_card(&e, 0, "snow_day");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Snow Day at a creature");
    move_to_graveyard(&mut e, 1, target);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before);
    assert_eq!(e.state.players[0].library.len(), library_before);
    assert!(e.state.pending_resolution.is_none());
    assert_eq!(e.state.skip_next_untap.len(), 0);

    // Press for Answers' pinned ruling: no Clue when its only target is illegal.
    let mut press = engine(70_024_002, &[], &["grizzly_bears"]);
    let press_target = move_ready_to_battlefield(&mut press, 1, "grizzly_bears");
    give_mana(
        &mut press,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut press, 0, "press_for_answers");
    let press_slot = hand_index_for_card(&press, 0, "press_for_answers");
    press
        .apply_scenario_command(0, &cast_spell(press_slot, target_object(press_target)))
        .expect("cast Press for Answers at a creature");
    move_to_graveyard(&mut press, 1, press_target);
    resolve_entire_stack_two_player(&mut press);
    assert_eq!(clue_count(&press), 0);
    assert_eq!(press.state.skip_next_untap.len(), 0);
}

#[test]
fn q70_tap_skip_cards_reject_unfunded_casts_and_cycling() {
    let mut e = engine(70_025_001, &[], &["grizzly_bears"]);
    let target = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "snow_day");
    let slot = hand_index_for_card(&e, 0, "snow_day");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .is_err(),
        "Snow Day cannot be cast for one blue mana"
    );
    assert_eq!(pool(&e, 0), (0, 1, 0, 0, 0, 0));
    assert!(e.state.stack.is_empty());

    let mut cycle = engine(70_025_002, &[], &[]);
    let source = inject_card_into_hand(&mut cycle, 0, "frostveil_ambush");
    assert!(
        cycle
            .apply_scenario_command(0, &hand_ability(&cycle, source, 0))
            .is_err(),
        "cycling {{1}} cannot be paid with no mana"
    );
    assert_eq!(cycle.state.objects[&source].zone, Zone::Hand);
}

// ── 03 enters-the-battlefield utility lands ───────────────────────────────

fn color_mask(color: char) -> (u32, u32, u32, u32, u32, u32) {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'c' => (0, 0, 0, 0, 0, 1),
        _ => unreachable!("printed mana"),
    }
}

fn mana_activate(e: &GameEngine, land: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(e, land, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for builds an ActivateAbility command")
    };
    activation.mana_option_index = option;
    command
}

#[test]
fn q70_lands_canary_simplest_radiant_fountain_gains_and_taps_for_colorless() {
    let mut e = engine(70_030_001, &["radiant_fountain"], &[]);
    let land = move_ready_to_battlefield(&mut e, 0, "radiant_fountain");
    assert_eq!(e.state.players[0].life, 20);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 22, "enters and gains exactly two");
    assert_eq!(e.state.players[1].life, 20);
    e.state.objects.get_mut(&land).expect("land").tapped = false;
    e.apply_scenario_command(0, &mana_activate(&e, land, 0))
        .expect("tap for colorless");
    assert_eq!(pool(&e, 0), color_mask('c'));
    assert!(e.state.objects[&land].tapped);
    assert!(e.state.stack.is_empty(), "mana ability uses no stack");
}

#[test]
fn q70_lands_kabira_crossroads_enters_tapped_gains_and_taps_for_white() {
    let mut e = engine(70_031_001, &["kabira_crossroads"], &[]);
    let land = move_ready_to_battlefield(&mut e, 0, "kabira_crossroads");
    assert!(e.state.objects[&land].tapped, "enters tapped");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 22);
    e.state.objects.get_mut(&land).expect("land").tapped = false;
    e.apply_scenario_command(0, &mana_activate(&e, land, 0))
        .expect("tap for white");
    assert_eq!(pool(&e, 0), color_mask('w'));
    assert!(e.state.objects[&land].tapped);
}

#[test]
fn q70_lands_canary_hardest_new_benalia_scries_privately_and_taps_for_white() {
    let mut e = engine(70_032_001, &["new_benalia"], &[]);
    let scried = inject_library_card(&mut e, 0, "storm_crow");
    e.state.players[0].library.retain(|oid| *oid != scried);
    e.state.players[0].library.push_front(scried);
    let land = move_ready_to_battlefield(&mut e, 0, "new_benalia");
    assert!(e.state.objects[&land].tapped, "enters tapped");
    let batch = resolve_top_two_player(&mut e);
    let choice = find_resolution_choice(&batch).expect("private scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [scried]);
    assert!(choice.public_reveal.is_none(), "scry is private");
    e.apply_scenario_command(0, &submit_resolution_choice(vec![scried]))
        .expect("put the card on the bottom");
    assert_eq!(e.state.players[0].library.back(), Some(&scried));
    e.state.objects.get_mut(&land).expect("land").tapped = false;
    e.apply_scenario_command(0, &mana_activate(&e, land, 0))
        .expect("tap for white");
    assert_eq!(pool(&e, 0), color_mask('w'));
}

#[test]
fn q70_lands_zhalfirin_void_scries_and_taps_for_colorless() {
    let mut e = engine(70_033_001, &["zhalfirin_void"], &[]);
    let scried = inject_library_card(&mut e, 0, "storm_crow");
    e.state.players[0].library.retain(|oid| *oid != scried);
    e.state.players[0].library.push_front(scried);
    let land = move_ready_to_battlefield(&mut e, 0, "zhalfirin_void");
    assert!(!e.state.objects[&land].tapped, "does not enter tapped");
    let batch = resolve_top_two_player(&mut e);
    let choice = find_resolution_choice(&batch).expect("private scry choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!(choice.candidate_object_ids, [scried]);
    e.apply_scenario_command(0, &submit_resolution_choice(vec![scried]))
        .expect("put the card on the bottom");
    e.state.objects.get_mut(&land).expect("land").tapped = false;
    e.apply_scenario_command(0, &mana_activate(&e, land, 0))
        .expect("tap for colorless");
    assert_eq!(pool(&e, 0), color_mask('c'));
}

#[test]
fn q70_lands_mana_abilities_require_an_untapped_controller_source() {
    let mut e = engine(70_034_001, &["zhalfirin_void"], &[]);
    let land = inject_permanent_on_battlefield(&mut e, 0, "zhalfirin_void");
    e.apply_scenario_command(0, &mana_activate(&e, land, 0))
        .expect("first activation");
    assert_eq!(pool(&e, 0), color_mask('c'));
    assert!(
        e.apply_scenario_command(0, &mana_activate(&e, land, 0))
            .is_err(),
        "a tapped land cannot produce again"
    );
    assert_eq!(pool(&e, 0), color_mask('c'));

    let mut foreign = engine(70_034_002, &["zhalfirin_void"], &[]);
    let foreign_land = inject_permanent_on_battlefield(&mut foreign, 1, "zhalfirin_void");
    assert!(
        foreign
            .apply_scenario_command(0, &mana_activate(&foreign, foreign_land, 0))
            .is_err(),
        "only the controller may activate the mana ability"
    );
    assert_eq!(pool(&foreign, 0), (0, 0, 0, 0, 0, 0));
    assert!(!foreign.state.objects[&foreign_land].tapped);
}

#[test]
fn q70_lands_do_not_trigger_from_the_graveyard() {
    for (index, card) in ["radiant_fountain", "kabira_crossroads"]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(70_035_000 + index as u64, &[card], &[]);
        inject_graveyard_card(&mut e, 0, card);
        pass_both_players(&mut e);
        assert_eq!(e.state.players[0].life, 20, "{card}: no entry, no life");
        assert_eq!(e.state.players[1].life, 20, "{card}: opponent unchanged");
    }
}
