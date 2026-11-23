//! Actual-card scenarios for the queue-v62 batch A cohorts.
//!
//! Three stable typed shapes share one batch: one creature shrink with a printed rider, one
//! creature destruction under a printed power or mana-value filter, and one creature destruction
//! followed by surveil or scry. Expectations are printed Oracle values. CR 115.1/601.2c fixes the
//! target at cast; CR 608.2b fizzles a spell whose only target left; CR 611.2a/514.2 and 613.4c
//! bound the shrink; CR 701.8 destroys (701.19a-c prevents regeneration); CR 701.22 scries;
//! CR 701.25 surveils; CR 119.3 applies life loss; and CR 701.7a creates the Saproling token.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{AbilitySourceZone, ActivateAbility, ChoiceKind, RuledCommand};

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
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("forest", &[])]);
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
            matches!(kind, ChoiceKind::LibraryTop | ChoiceKind::LibraryLook),
            "library choice kind"
        );
        let candidates = choice.candidate_object_ids.clone();
        e.apply_scenario_command(0, &submit_resolution_choice(candidates))
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

// ── 01 one creature shrink plus one printed rider ─────────────────────────

fn shrink_cases() -> [(&'static str, ManaGift, (i32, i32)); 5] {
    [
        (
            "constricting_tendrils",
            ManaGift {
                u: 1,
                ..Default::default()
            },
            (-3, 0),
        ),
        (
            "dazzling_lights",
            ManaGift {
                u: 1,
                ..Default::default()
            },
            (-3, 0),
        ),
        (
            "jaces_scrutiny",
            ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            (-4, 0),
        ),
        (
            "ulcerate",
            ManaGift {
                b: 1,
                ..Default::default()
            },
            (-3, -3),
        ),
        (
            "fungal_infection",
            ManaGift {
                b: 1,
                ..Default::default()
            },
            (-1, -1),
        ),
    ]
}

#[test]
fn q62_shrink_canary_ulcerate_loses_life_only_when_it_resolves() {
    let mut e = engine(62_010_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
    let life_before = e.state.players[0].life;
    cast_named(
        &mut e,
        0,
        "ulcerate",
        ManaGift {
            b: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(3)),
        "printed -3/-3"
    );
    assert_eq!(
        e.state.players[0].life,
        life_before - 3,
        "printed 3 life loss"
    );

    let mut stale = engine(62_010_002);
    let target = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    let life_before = stale.state.players[0].life;
    cast_named(
        &mut stale,
        0,
        "ulcerate",
        ManaGift {
            b: 1,
            ..Default::default()
        },
        target_object(target),
    );
    move_to_exile(&mut stale, 1, target);
    pass_both_players(&mut stale);
    assert_eq!(
        stale.state.players[0].life, life_before,
        "an illegal sole target stops the whole spell, so no life is lost"
    );

    let mut rejected = engine(62_010_003);
    let artifact = inject_permanent_on_battlefield(&mut rejected, 1, "bonesplitter");
    inject_card_into_hand(&mut rejected, 0, "ulcerate");
    give_mana(
        &mut rejected,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let before = pool(&rejected);
    let slot = hand_index_for_card(&rejected, 0, "ulcerate");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
            .is_err(),
        "Ulcerate cannot target a noncreature permanent"
    );
    assert_eq!(pool(&rejected), before, "a rejected cast spends no mana");
}

#[test]
fn q62_shrink_canary_hardest_fungal_infection_creates_a_saproling() {
    let mut e = engine(62_011_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 1, 1);
    cast_named(
        &mut e,
        0,
        "fungal_infection",
        ManaGift {
            b: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Graveyard,
        "the -1/-1 is lethal to a 1/1"
    );
    assert_eq!(
        battlefield_token_oids(&e, 0, "saproling_g_1_1").len(),
        1,
        "exactly one Saproling token"
    );

    let mut stale = engine(62_011_002);
    let target = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
    cast_named(
        &mut stale,
        0,
        "fungal_infection",
        ManaGift {
            b: 1,
            ..Default::default()
        },
        target_object(target),
    );
    move_to_exile(&mut stale, 1, target);
    pass_both_players(&mut stale);
    assert!(
        battlefield_token_oids(&stale, 0, "saproling_g_1_1").is_empty(),
        "an illegal sole target stops the whole spell, so no token is created"
    );
}

#[test]
fn q62_shrink_cards_apply_their_exact_printed_pump() {
    for (index, (id, mana, expected)) in shrink_cases().into_iter().enumerate() {
        let mut e = engine(62_012_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 6, 6);
        cast_named(&mut e, 0, id, gift(&mana), target_object(target));
        if id == "dazzling_lights" {
            resolve_with_library_choice(&mut e);
        } else {
            resolve_entire_stack_two_player(&mut e);
        }
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some((6 + expected.0) as u32), Some((6 + expected.1) as u32)),
            "{id}: printed pump"
        );
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{id}: a 6/6 survives"
        );
    }
}

#[test]
fn q62_shrink_riders_apply_only_after_a_legal_resolution() {
    // Dazzling Lights surveils exactly two.
    let mut surveil = engine(62_013_001);
    let target = inject_creature_on_battlefield(&mut surveil, 1, "grizzly_bears");
    cast_named(
        &mut surveil,
        0,
        "dazzling_lights",
        ManaGift {
            u: 1,
            ..Default::default()
        },
        target_object(target),
    );
    let batch = pass_both_players_with_batch(&mut surveil);
    let choice = find_resolution_choice(&batch).expect("Dazzling Lights surveil 2");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids.len(), 2);
    let candidates = choice.candidate_object_ids.clone();
    surveil
        .apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("complete the surveil choice");

    // Jace's Scrutiny investigates exactly once.
    let mut clue = engine(62_013_002);
    let target = inject_creature_on_battlefield(&mut clue, 1, "grizzly_bears");
    cast_named(
        &mut clue,
        0,
        "jaces_scrutiny",
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut clue);
    assert_eq!(battlefield_token_oids(&clue, 0, "clue").len(), 1);

    // Fungal Infection creates one Saproling.
    let mut saproling = engine(62_013_003);
    let target = inject_creature_with_stats(&mut saproling, 1, "grizzly_bears", 2, 2);
    cast_named(
        &mut saproling,
        0,
        "fungal_infection",
        ManaGift {
            b: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut saproling);
    assert_eq!(
        battlefield_token_oids(&saproling, 0, "saproling_g_1_1").len(),
        1
    );

    // Ulcerate loses exactly three life.
    let mut life = engine(62_013_004);
    let target = inject_creature_on_battlefield(&mut life, 1, "grizzly_bears");
    let before = life.state.players[0].life;
    cast_named(
        &mut life,
        0,
        "ulcerate",
        ManaGift {
            b: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut life);
    assert_eq!(life.state.players[0].life, before - 3);
}

#[test]
fn q62_shrink_cards_reject_noncreatures_and_fizzle_stale_targets() {
    for (index, (id, mana, _)) in shrink_cases().into_iter().enumerate() {
        let mut rejected = engine(62_014_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut rejected, 1, "bonesplitter");
        inject_card_into_hand(&mut rejected, 0, id);
        give_mana(&mut rejected, 0, gift(&mana));
        let before = pool(&rejected);
        let slot = hand_index_for_card(&rejected, 0, id);
        assert!(
            rejected
                .apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
                .is_err(),
            "{id}: a noncreature target is illegal"
        );
        assert_eq!(
            pool(&rejected),
            before,
            "{id}: rejected casts spend no mana"
        );

        let mut stale = engine(62_015_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut stale, 1, "grizzly_bears");
        let life_before = stale.state.players[0].life;
        let clues_before = battlefield_token_oids(&stale, 0, "clue").len();
        let saprolings_before = battlefield_token_oids(&stale, 0, "saproling_g_1_1").len();
        cast_named(&mut stale, 0, id, gift(&mana), target_object(target));
        move_to_exile(&mut stale, 1, target);
        let batch = pass_both_players_with_batch(&mut stale);
        assert!(
            find_resolution_choice(&batch).is_none(),
            "{id}: no rider after an illegal sole target"
        );
        assert_eq!(
            stale.state.players[0].life, life_before,
            "{id}: no life rider"
        );
        assert_eq!(
            battlefield_token_oids(&stale, 0, "clue").len(),
            clues_before,
            "{id}: no Clue rider"
        );
        assert_eq!(
            battlefield_token_oids(&stale, 0, "saproling_g_1_1").len(),
            saprolings_before,
            "{id}: no Saproling rider"
        );
        assert!(stale.state.stack.is_empty(), "{id}: the spell fizzled");
    }
}

#[test]
fn q62_shrink_constricting_tendrils_cycles_for_two() {
    let mut e = engine(62_016_001);
    let card = inject_card_into_hand(&mut e, 0, "constricting_tendrils");
    let drawn = inject_library_card(&mut e, 0, "grizzly_bears");
    e.state.players[0].library.retain(|oid| *oid != drawn);
    e.state.players[0].library.push_front(drawn);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(&e, card, 0))
        .expect("activate Cycling {2}");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "Cycling spends exactly {{2}}");
    assert_eq!(e.state.objects[&card].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
}

// ── 02 filtered creature destruction with printed riders ──────────────────

fn filtered_cases() -> [(&'static str, ManaGift, &'static str, &'static str); 5] {
    [
        (
            "easy_prey",
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            "grizzly_bears",
            "juggernaut",
        ),
        (
            "swat",
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            "grizzly_bears",
            "juggernaut",
        ),
        (
            "reprisal",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            "juggernaut",
            "grizzly_bears",
        ),
        (
            "smother",
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            "grizzly_bears",
            "juggernaut",
        ),
        (
            "radiants_judgment",
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            "juggernaut",
            "grizzly_bears",
        ),
    ]
}

fn inject_case_creature(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    match card {
        "juggernaut" => inject_creature_with_stats(e, player, card, 5, 3),
        _ => inject_creature_on_battlefield(e, player, card),
    }
}

#[test]
fn q62_destroy_filtered_canary_reprisal_prevents_regeneration() {
    let mut e = engine(62_020_001);
    let target = inject_case_creature(&mut e, 1, "juggernaut");
    e.state
        .objects
        .get_mut(&target)
        .expect("creature")
        .regeneration_shields = 1;
    cast_named(
        &mut e,
        0,
        "reprisal",
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Graveyard,
        "Reprisal cannot be replaced by regeneration"
    );

    let mut rejected = engine(62_020_002);
    let small = inject_case_creature(&mut rejected, 1, "grizzly_bears");
    inject_card_into_hand(&mut rejected, 0, "reprisal");
    give_mana(
        &mut rejected,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let before = pool(&rejected);
    let slot = hand_index_for_card(&rejected, 0, "reprisal");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(small)))
            .is_err(),
        "Reprisal cannot target power 2"
    );
    assert_eq!(pool(&rejected), before);
}

#[test]
fn q62_destroy_filtered_canary_hardest_smother_filters_mana_value() {
    let mut e = engine(62_021_001);
    let target = inject_case_creature(&mut e, 1, "grizzly_bears");
    e.state
        .objects
        .get_mut(&target)
        .expect("creature")
        .regeneration_shields = 1;
    cast_named(
        &mut e,
        0,
        "smother",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        target_object(target),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Graveyard,
        "Smother destroys a mana value 2 creature without regeneration"
    );

    let mut rejected = engine(62_021_002);
    let large = inject_case_creature(&mut rejected, 1, "juggernaut");
    inject_card_into_hand(&mut rejected, 0, "smother");
    give_mana(
        &mut rejected,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&rejected, 0, "smother");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(large)))
            .is_err(),
        "Smother cannot target mana value 4"
    );
}

#[test]
fn q62_destroy_filtered_cards_enforce_their_printed_filters() {
    for (index, (id, mana, legal, illegal)) in filtered_cases().into_iter().enumerate() {
        let mut e = engine(62_022_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, legal);
        let bystander = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_named(&mut e, 0, id, gift(&mana), target_object(target));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{id}: exact printed cost");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{id}: legal filtered target is destroyed"
        );
        assert_eq!(
            e.state.objects[&bystander].zone,
            Zone::Battlefield,
            "{id}: a bystander is untouched"
        );

        let mut rejected = engine(62_023_000 + index as u64);
        let target = inject_case_creature(&mut rejected, 1, illegal);
        inject_card_into_hand(&mut rejected, 0, id);
        give_mana(&mut rejected, 0, gift(&mana));
        let before = pool(&rejected);
        let slot = hand_index_for_card(&rejected, 0, id);
        assert!(
            rejected
                .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
                .is_err(),
            "{id}: illegal filtered target is rejected"
        );
        assert_eq!(
            pool(&rejected),
            before,
            "{id}: rejected casts spend no mana"
        );
        assert!(rejected.state.stack.is_empty(), "{id}");
    }
}

#[test]
fn q62_destroy_filtered_cards_handle_regeneration_where_printed() {
    for (index, (id, mana, legal, _)) in filtered_cases().into_iter().enumerate() {
        let mut e = engine(62_024_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, legal);
        e.state
            .objects
            .get_mut(&target)
            .expect("creature")
            .regeneration_shields = 1;
        cast_named(&mut e, 0, id, gift(&mana), target_object(target));
        resolve_entire_stack_two_player(&mut e);
        if matches!(id, "reprisal" | "smother") {
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Graveyard,
                "{id}: printed regeneration prevention"
            );
        } else {
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Battlefield,
                "{id}: an ordinary destroy is replaced by the shield"
            );
        }
    }
}

#[test]
fn q62_destroy_filtered_cycling_cards_cycle_for_two() {
    for (index, id) in ["easy_prey", "swat", "radiants_judgment"]
        .into_iter()
        .enumerate()
    {
        let mut e = engine(62_025_000 + index as u64);
        let card = inject_card_into_hand(&mut e, 0, id);
        let drawn = inject_library_card(&mut e, 0, "grizzly_bears");
        e.state.players[0].library.retain(|oid| *oid != drawn);
        e.state.players[0].library.push_front(drawn);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        e.apply_scenario_command(0, &hand_ability(&e, card, 0))
            .unwrap_or_else(|error| panic!("{id}: activate Cycling {{2}}: {error:?}"));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{id}: spends exactly {{2}}");
        assert_eq!(e.state.objects[&card].zone, Zone::Graveyard, "{id}");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{id}");
    }
}

#[test]
fn q62_destroy_filtered_cards_fizzle_on_stale_targets() {
    for (index, (id, mana, legal, _)) in filtered_cases().into_iter().enumerate() {
        let mut e = engine(62_026_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, legal);
        let spelling = cast_named(&mut e, 0, id, gift(&mana), target_object(target));
        move_to_exile(&mut e, 1, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{id}");
        assert_eq!(
            e.state.objects[&spelling].zone,
            Zone::Graveyard,
            "{id}: the spell fizzled into its owner's graveyard"
        );
        assert!(e.state.players[0].graveyard.contains(&spelling), "{id}");
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

#[test]
fn q62_destroy_filtered_cards_recheck_filters_on_resolution() {
    for (index, (id, mana, legal, _)) in filtered_cases().into_iter().enumerate() {
        let mut e = engine(62_027_000 + index as u64);
        let target = inject_case_creature(&mut e, 1, legal);
        let spelling = cast_named(&mut e, 0, id, gift(&mana), target_object(target));
        match id {
            "easy_prey" | "smother" => {
                e.state.objects.get_mut(&target).expect("creature").card_id = "juggernaut".into();
            }
            "swat" => e.state.objects.get_mut(&target).expect("creature").power = Some(5),
            "reprisal" | "radiants_judgment" => {
                e.state.objects.get_mut(&target).expect("creature").power = Some(1);
            }
            _ => unreachable!(),
        }
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{id}: a target that stopped matching the filter survives"
        );
        assert_eq!(
            e.state.objects[&spelling].zone,
            Zone::Graveyard,
            "{id}: the spell fizzled"
        );
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

// ── 03 destroy one creature then surveil or scry ──────────────────────────

fn library_cases() -> [(&'static str, ManaGift, u32, bool); 3] {
    [
        (
            "hes_dead,_jim",
            ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            1,
            true,
        ),
        (
            "deadly_visit",
            ManaGift {
                b: 2,
                c: 3,
                ..Default::default()
            },
            2,
            true,
        ),
        (
            "get_the_point",
            ManaGift {
                b: 1,
                r: 1,
                c: 3,
                ..Default::default()
            },
            1,
            false,
        ),
    ]
}

#[test]
fn q62_destroy_surveil_canary_hes_dead_jim_surveils_one() {
    let mut e = engine(62_030_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "hes_dead,_jim",
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
        target_object(target),
    );
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&batch).expect("He's Dead, Jim surveil 1");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(choice.candidate_object_ids.len(), 1);

    let mut rejected = engine(62_030_002);
    let artifact = inject_permanent_on_battlefield(&mut rejected, 1, "bonesplitter");
    inject_card_into_hand(&mut rejected, 0, "hes_dead,_jim");
    give_mana(
        &mut rejected,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&rejected, 0, "hes_dead,_jim");
    assert!(
        rejected
            .apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
            .is_err(),
        "He's Dead, Jim cannot target a noncreature permanent"
    );
}

#[test]
fn q62_destroy_surveil_canary_hardest_deadly_visit_surveils_two() {
    let mut e = engine(62_031_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_named(
        &mut e,
        0,
        "deadly_visit",
        ManaGift {
            b: 2,
            c: 3,
            ..Default::default()
        },
        target_object(target),
    );
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&batch).expect("Deadly Visit surveil 2");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(choice.candidate_object_ids.len(), 2);
    let candidates = choice.candidate_object_ids.clone();
    e.apply_scenario_command(0, &submit_resolution_choice(candidates))
        .expect("complete the surveil choice");
}

#[test]
fn q62_destroy_surveil_scry_cards_apply_the_printed_library_filter() {
    for (index, (id, mana, count, surveil)) in library_cases().into_iter().enumerate() {
        let mut e = engine(62_032_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_named(&mut e, 0, id, gift(&mana), target_object(target));
        let batch = pass_both_players_with_batch(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard, "{id}");
        let choice =
            find_resolution_choice(&batch).unwrap_or_else(|| panic!("{id}: library filter"));
        assert_eq!(
            choice.choice_kind(),
            if surveil {
                ChoiceKind::LibraryLook
            } else {
                ChoiceKind::LibraryTop
            },
            "{id}"
        );
        assert_eq!((choice.min, choice.max), (0, count), "{id}: printed count");
        assert_eq!(
            choice.candidate_object_ids.len() as u32,
            count,
            "{id}: looks at exactly {count}"
        );
        let candidates = choice.candidate_object_ids.clone();
        e.apply_scenario_command(0, &submit_resolution_choice(candidates))
            .expect("complete the library choice");
    }
}

#[test]
fn q62_destroy_surveil_scry_cards_fizzle_without_a_library_filter() {
    for (index, (id, mana, _, _)) in library_cases().into_iter().enumerate() {
        let mut e = engine(62_033_000 + index as u64);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_named(&mut e, 0, id, gift(&mana), target_object(target));
        move_to_exile(&mut e, 1, target);
        let batch = pass_both_players_with_batch(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{id}");
        assert!(
            find_resolution_choice(&batch).is_none(),
            "{id}: no surveil or scry after an illegal sole target"
        );
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

#[test]
fn q62_destroy_surveil_scry_cards_still_filter_after_an_indestructible_target() {
    for (index, (id, mana, count, _)) in library_cases().into_iter().enumerate() {
        let mut e = engine(62_034_000 + index as u64);
        let target = inject_permanent_on_battlefield(&mut e, 1, "darksteel_myr");
        cast_named(&mut e, 0, id, gift(&mana), target_object(target));
        let batch = pass_both_players_with_batch(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{id}: indestructible creature survives"
        );
        let choice = find_resolution_choice(&batch)
            .unwrap_or_else(|| panic!("{id}: a legal but undestroyed target still filters"));
        assert_eq!((choice.min, choice.max), (0, count), "{id}");
        let candidates = choice.candidate_object_ids.clone();
        e.apply_scenario_command(0, &submit_resolution_choice(candidates))
            .expect("complete the library choice");
    }
}
