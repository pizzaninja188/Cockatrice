//! Actual-card scenarios for the queue-v61 batch A cohorts.
//!
//! Three stable typed shapes share one batch: a land-destruction sorcery with one printed rider,
//! a targeted drain spell, and a creature pump that grants one keyword then draws. Expectations
//! are printed Oracle values. CR 701.8 destroys; CR 701.22 scries; CR 701.23/701.24 search and
//! shuffle; CR 121.1 draws; CR 702.29a-b governs cycling; CR 702.34a-d governs flashback and its
//! exile; CR 119.3 applies life loss and gain; CR 611.2a/514.2 bound the pump; CR 613.4c and
//! 613.1f apply the pump and keyword layers; and CR 608.2b fizzles a spell whose only target left.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, ActivateAbility, CastMethod, CastSpell, ChoiceKind, RuledCommand,
};

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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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

fn resolve_with_library_choice(e: &mut GameEngine) {
    let batch = pass_both_players_with_batch(e);
    if let Some(choice) = find_resolution_choice(&batch) {
        let kind = choice.choice_kind();
        assert!(
            matches!(
                kind,
                ChoiceKind::LibraryTop | ChoiceKind::LibraryLook | ChoiceKind::LibrarySearch
            ),
            "library choice kind"
        );
        let candidates = choice.candidate_object_ids.clone();
        let chosen = if kind == ChoiceKind::LibrarySearch {
            vec![*candidates.first().expect("a legal search candidate")]
        } else {
            candidates
        };
        e.apply_scenario_command(0, &submit_resolution_choice(chosen))
            .expect("complete the library choice");
    }
    assert!(e.state.pending_resolution.is_none());
}

fn move_to_exile(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].exile.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Exile;
    *e.state.zone_change_generation.entry(object).or_insert(0) += 1;
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

fn land_mana(id: &str) -> ManaGift {
    match id {
        "earth_rift" | "lay_waste" | "rubble_reading" => ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
        "frenzied_tilling" => ManaGift {
            r: 1,
            g: 1,
            c: 3,
            ..Default::default()
        },
        "implode" => ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
        other => panic!("unknown land-destruction mana for {other}"),
    }
}

fn drain_mana(id: &str) -> (ManaGift, i32, bool) {
    match id {
        "sovereigns_bite" => (
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            3,
            false,
        ),
        "soul_feast" => (
            ManaGift {
                b: 2,
                c: 3,
                ..Default::default()
            },
            4,
            false,
        ),
        "last_caress" => (
            ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            1,
            true,
        ),
        other => panic!("unknown drain mana for {other}"),
    }
}

fn pump_mana(id: &str) -> (ManaGift, u32, u32, Keyword) {
    match id {
        "guided_strike" => (
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            1,
            0,
            Keyword::FirstStrike,
        ),
        "moment_of_defiance" => (
            ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            2,
            1,
            Keyword::Lifelink,
        ),
        "wildsize" => (
            ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            2,
            2,
            Keyword::Trample,
        ),
        other => panic!("unknown pump mana for {other}"),
    }
}

// ── 01 destroy target land with one rider ─────────────────────────────────

#[test]
fn q61_land_destruction_canary_implode() {
    let mut e = engine(61_010_001);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let hand_before = e.state.players[0].hand.len();
    cast_named(
        &mut e,
        0,
        "implode",
        land_mana("implode"),
        target_object(land),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert!(e.state.players[1].graveyard.contains(&land));
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "Implode replaces its cast card and draws one"
    );

    let mut rejected = engine(61_010_002);
    let creature = inject_creature_on_battlefield(&mut rejected, 1, "grizzly_bears");
    inject_card_into_hand(&mut rejected, 0, "implode");
    give_mana(&mut rejected, 0, land_mana("implode"));
    let slot = hand_index_for_card(&rejected, 0, "implode");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .is_err(),
        "Implode cannot target a creature"
    );
}

#[test]
fn q61_land_destruction_canary_earth_rift_flashbacks_and_exiles() {
    let mut e = engine(61_010_003);
    let first = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let rift = cast_named(
        &mut e,
        0,
        "earth_rift",
        land_mana("earth_rift"),
        target_object(first),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&rift].zone, Zone::Graveyard);

    let second = inject_permanent_on_battlefield(&mut e, 1, "forest");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            c: 5,
            ..Default::default()
        },
    );
    let generation = e.state.zone_change_generation[&rift];
    semantic::accepted(&mut e, 0, &flashback_cast(rift, second, generation));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&rift].zone, Zone::Exile);
}

#[test]
fn q61_land_destruction_cards_hit_lands_and_reject_creatures() {
    for (index, id) in [
        "earth_rift",
        "frenzied_tilling",
        "implode",
        "lay_waste",
        "rubble_reading",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = engine(61_011_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        cast_named(&mut e, 0, id, land_mana(id), target_object(land));
        resolve_with_library_choice(&mut e);
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Graveyard,
            "{id} must destroy the target land"
        );

        let mut rejected = engine(61_012_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut rejected, 1, "grizzly_bears");
        inject_card_into_hand(&mut rejected, 0, id);
        give_mana(&mut rejected, 0, land_mana(id));
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
fn q61_land_destruction_riders_draw_scry_search_and_cycle() {
    // Implode draws exactly one.
    let mut draw = engine(61_013_001);
    let land = inject_permanent_on_battlefield(&mut draw, 1, "forest");
    let hand_before = draw.state.players[0].hand.len();
    cast_named(
        &mut draw,
        0,
        "implode",
        land_mana("implode"),
        target_object(land),
    );
    resolve_entire_stack_two_player(&mut draw);
    assert_eq!(draw.state.players[0].hand.len(), hand_before + 1);

    // Rubble Reading scries exactly two after destroying.
    let mut scry = engine(61_013_002);
    let land = inject_permanent_on_battlefield(&mut scry, 1, "forest");
    cast_named(
        &mut scry,
        0,
        "rubble_reading",
        land_mana("rubble_reading"),
        target_object(land),
    );
    let batch = pass_both_players_with_batch(&mut scry);
    assert_eq!(scry.state.objects[&land].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&batch).expect("Rubble Reading scry 2");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids.len(), 2);
    let candidates = choice.candidate_object_ids.clone();
    scry.apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("put both scried cards on the bottom");

    // Frenzied Tilling finds a basic land and puts it onto the battlefield tapped.
    let mut search = engine(61_013_003);
    let land = inject_permanent_on_battlefield(&mut search, 1, "forest");
    let forest = inject_library_card(&mut search, 0, "forest");
    cast_named(
        &mut search,
        0,
        "frenzied_tilling",
        land_mana("frenzied_tilling"),
        target_object(land),
    );
    let batch = pass_both_players_with_batch(&mut search);
    assert_eq!(search.state.objects[&land].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&batch).expect("Frenzied Tilling basic-land search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&forest));
    search
        .apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose the basic land");
    assert_eq!(search.state.objects[&forest].zone, Zone::Battlefield);
    assert!(search.state.objects[&forest].tapped);

    // Lay Waste cycles from hand for {2}.
    let mut cycling = engine(61_013_004);
    let card = inject_card_into_hand(&mut cycling, 0, "lay_waste");
    let drawn = inject_library_card(&mut cycling, 0, "grizzly_bears");
    cycling.state.players[0].library.retain(|oid| *oid != drawn);
    cycling.state.players[0].library.push_front(drawn);
    give_mana(
        &mut cycling,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    cycling
        .apply_scenario_command(0, &hand_ability(&cycling, card, 0))
        .expect("activate Cycling {2}");
    assert_eq!(cycling.state.objects[&card].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut cycling);
    assert_eq!(cycling.state.objects[&drawn].zone, Zone::Hand);
}

#[test]
fn q61_land_destruction_handles_stale_and_indestructible_targets() {
    // A stale target fizzles the whole spell; Implode draws nothing.
    let mut stale = engine(61_014_001);
    let land = inject_permanent_on_battlefield(&mut stale, 1, "forest");
    cast_named(
        &mut stale,
        0,
        "implode",
        land_mana("implode"),
        target_object(land),
    );
    move_to_exile(&mut stale, 1, land);
    let hand_before = stale.state.players[0].hand.len();
    pass_both_players(&mut stale);
    assert_eq!(stale.state.objects[&land].zone, Zone::Exile);
    assert_eq!(stale.state.players[0].hand.len(), hand_before);
    assert!(stale.state.stack.is_empty());

    // Frenzied Tilling's 2013-01-24 ruling: an illegal target skips the search entirely.
    let mut skipped = engine(61_014_002);
    let land = inject_permanent_on_battlefield(&mut skipped, 1, "forest");
    inject_card_into_hand(&mut skipped, 0, "frenzied_tilling");
    give_mana(&mut skipped, 0, land_mana("frenzied_tilling"));
    let slot = hand_index_for_card(&skipped, 0, "frenzied_tilling");
    semantic::accepted(&mut skipped, 0, &cast_spell(slot, target_object(land)));
    move_to_exile(&mut skipped, 1, land);
    let batch = pass_both_players_with_batch(&mut skipped);
    assert!(find_resolution_choice(&batch).is_none());
    assert_eq!(skipped.state.objects[&land].zone, Zone::Exile);
    assert!(skipped.state.stack.is_empty());

    // Rubble Reading's 2019-01-25 ruling: an indestructible land survives but the scry still happens.
    let mut hard = engine(61_014_003);
    let citadel = inject_permanent_on_battlefield(&mut hard, 1, "darksteel_citadel");
    cast_named(
        &mut hard,
        0,
        "rubble_reading",
        land_mana("rubble_reading"),
        target_object(citadel),
    );
    let batch = pass_both_players_with_batch(&mut hard);
    assert_eq!(hard.state.objects[&citadel].zone, Zone::Battlefield);
    let choice = find_resolution_choice(&batch).expect("scry after an indestructible survives");
    assert_eq!((choice.min, choice.max), (0, 2));
    let candidates = choice.candidate_object_ids.clone();
    hard.apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("put both scried cards on the bottom");
}

// ── 02 target player loses life and you gain life ─────────────────────────

#[test]
fn q61_drain_life_canary_sovereigns_bite() {
    let mut e = engine(61_020_001);
    let opponent_before = e.state.players[1].life;
    let own_before = e.state.players[0].life;
    cast_named(
        &mut e,
        0,
        "sovereigns_bite",
        drain_mana("sovereigns_bite").0,
        target_player(1),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, opponent_before - 3);
    assert_eq!(e.state.players[0].life, own_before + 3);

    let mut own = engine(61_020_002);
    let life_before = own.state.players[0].life;
    cast_named(
        &mut own,
        0,
        "sovereigns_bite",
        drain_mana("sovereigns_bite").0,
        target_player(0),
    );
    resolve_entire_stack_two_player(&mut own);
    assert_eq!(own.state.players[0].life, life_before);
}

#[test]
fn q61_drain_life_cards_apply_exact_amounts_and_printed_draws() {
    for (index, id) in ["last_caress", "soul_feast", "sovereigns_bite"]
        .into_iter()
        .enumerate()
    {
        let (mana, amount, draws) = drain_mana(id);
        let mut e = engine(61_021_000 + index as u64);
        let opponent_before = e.state.players[1].life;
        let own_before = e.state.players[0].life;
        let hand_before_cast = e.state.players[0].hand.len();
        cast_named(&mut e, 0, id, mana, target_player(1));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            opponent_before - amount,
            "{id}: exact life loss"
        );
        assert_eq!(
            e.state.players[0].life,
            own_before + amount,
            "{id}: exact life gain"
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before_cast + if draws { 1 } else { 0 },
            "{id}: draw only where printed"
        );
    }
}

#[test]
fn q61_drain_life_requires_a_player_target() {
    for (index, id) in ["last_caress", "soul_feast", "sovereigns_bite"]
        .into_iter()
        .enumerate()
    {
        let (mana, _, _) = drain_mana(id);
        let mut e = engine(61_022_000 + index as u64);
        inject_card_into_hand(&mut e, 0, id);
        give_mana(&mut e, 0, mana);
        let hand_before = e.state.players[0].hand.len();
        let pool_before = pool(&e);
        let slot = hand_index_for_card(&e, 0, id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{id}: the player target is required"
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{id}");
        assert_eq!(
            pool(&e),
            pool_before,
            "{id}: the rejected cast spends no mana"
        );
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

// ── 03 target creature gets +N/+N and gains a keyword, then draw ───────────

#[test]
fn q61_pump_keyword_draw_canary_guided_strike() {
    let mut e = engine(61_030_001);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let hand_before_cast = e.state.players[0].hand.len();
    cast_named(
        &mut e,
        0,
        "guided_strike",
        pump_mana("guided_strike").0,
        target_object(target),
    );
    assert_eq!(
        e.effective_power(target),
        Some(2),
        "pump is not applied at cast time"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(3));
    assert_eq!(e.effective_toughness(target), Some(2));
    assert!(e.effective_has_keyword(target, Keyword::FirstStrike));
    assert_eq!(e.state.players[0].hand.len(), hand_before_cast + 1);
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(target), Some(2));
    assert!(!e.effective_has_keyword(target, Keyword::FirstStrike));

    let mut rejected = engine(61_030_002);
    let land = inject_permanent_on_battlefield(&mut rejected, 1, "forest");
    inject_card_into_hand(&mut rejected, 0, "guided_strike");
    give_mana(&mut rejected, 0, pump_mana("guided_strike").0);
    let slot = hand_index_for_card(&rejected, 0, "guided_strike");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(land)))
            .is_err(),
        "Guided Strike cannot target a land"
    );
}

#[test]
fn q61_pump_keyword_draw_cards_apply_exact_stats_keywords_and_draw() {
    for (index, id) in ["guided_strike", "moment_of_defiance", "wildsize"]
        .into_iter()
        .enumerate()
    {
        let (mana, power, toughness, keyword) = pump_mana(id);
        for target_player in [0usize, 1usize] {
            let mut e = engine(61_031_000 + (index * 2 + target_player) as u64);
            let target = inject_creature_on_battlefield(&mut e, target_player, "grizzly_bears");
            let hand_before_cast = e.state.players[0].hand.len();
            cast_named(&mut e, 0, id, gift(&mana), target_object(target));
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(
                e.effective_power(target),
                Some(2 + power),
                "{id}: power of player {target_player}'s creature"
            );
            assert_eq!(
                e.effective_toughness(target),
                Some(2 + toughness),
                "{id}: toughness of player {target_player}'s creature"
            );
            assert!(e.effective_has_keyword(target, keyword), "{id}: keyword");
            assert_eq!(
                e.state.players[0].hand.len(),
                hand_before_cast + 1,
                "{id}: draw"
            );
            end_active_turn(&mut e, 0);
            assert_eq!(e.effective_power(target), Some(2), "{id}: expiry");
            assert!(
                !e.effective_has_keyword(target, keyword),
                "{id}: keyword expiry"
            );
        }
    }
}

#[test]
fn q61_pump_keyword_draw_fizzles_without_drawing_when_the_target_leaves() {
    for (index, id) in ["guided_strike", "moment_of_defiance", "wildsize"]
        .into_iter()
        .enumerate()
    {
        let (mana, _, _, _) = pump_mana(id);
        let mut e = engine(61_032_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let hand_before_cast = e.state.players[0].hand.len();
        cast_named(&mut e, 0, id, mana, target_object(creature));
        e.apply_scenario_command(0, &pass())
            .expect("pump controller passes");
        inject_card_into_hand(&mut e, 1, "unsummon");
        give_mana(
            &mut e,
            1,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let unsummon_slot = hand_index_for_card(&e, 1, "unsummon");
        e.apply_scenario_command(1, &cast_spell(unsummon_slot, target_object(creature)))
            .expect("opponent casts Unsummon in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&creature].zone, Zone::Hand, "{id}");
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before_cast + 1,
            "{id}: the returned creature offsets the cast and no draw happens"
        );
        assert_eq!(
            count_card_id_in_graveyard(&e, 0, id),
            1,
            "{id}: the fizzled pump is in its owner's graveyard"
        );
    }
}

#[test]
fn q61_pump_keyword_draw_requires_a_creature_target() {
    for (index, id) in ["guided_strike", "moment_of_defiance", "wildsize"]
        .into_iter()
        .enumerate()
    {
        let (mana, _, _, _) = pump_mana(id);
        let mut e = engine(61_033_000 + index as u64);
        inject_card_into_hand(&mut e, 0, id);
        give_mana(&mut e, 0, mana);
        let hand_before = e.state.players[0].hand.len();
        let pool_before = pool(&e);
        let slot = hand_index_for_card(&e, 0, id);
        for (illegal, label) in [
            (vec![], "no target"),
            (
                target_object(inject_permanent_on_battlefield(&mut e, 1, "forest")),
                "land",
            ),
            (target_player(1), "player"),
        ] {
            assert!(
                e.apply_scenario_command(0, &cast_spell(slot, illegal))
                    .is_err(),
                "{id}: a {label} is not a legal creature target"
            );
            assert_eq!(e.state.players[0].hand.len(), hand_before, "{id}: {label}");
            assert_eq!(pool(&e), pool_before, "{id}: {label} spends no mana");
            assert!(e.state.stack.is_empty(), "{id}: {label}");
        }
    }
}
