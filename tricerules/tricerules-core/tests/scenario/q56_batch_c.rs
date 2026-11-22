//! Actual-card scenarios for the queue-v56 batch C cohorts.
//!
//! Expectations are printed Oracle values. CR 115.1/601.2c choose graveyard and battlefield
//! targets at cast; CR 400.7 gives a zone-changed card a new identity; CR 701.8 destroys;
//! CR 701.16/111.10 create a Clue; CR 118.8/601.2f-h pay the sacrifice as an additional cost;
//! CR 121.1 draws; CR 702.34a-d govern flashback; CR 608.2b fizzles a spell whose only target is
//! illegal.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    CastCostGroupSelection, CastMethod, CastSpell, CostObjectRef, CostObjectRefs, RuledCommand,
    TargetRef,
};

struct YardCase {
    id: &'static str,
    mana: ManaGift,
    flashback: ManaGift,
    filter: YardFilter,
}

#[derive(Clone, Copy)]
enum YardFilter {
    Any,
    Creature,
    InstantOrSorcery,
}

fn yard_cases() -> [YardCase; 3] {
    [
        YardCase {
            id: "dryads_revival",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            flashback: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            filter: YardFilter::Any,
        },
        YardCase {
            id: "morgue_theft",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            filter: YardFilter::Creature,
        },
        YardCase {
            id: "mystic_retrieval",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            flashback: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            filter: YardFilter::InstantOrSorcery,
        },
    ]
}

struct DestroyCase {
    id: &'static str,
    mana: ManaGift,
    filter: DestroyFilter,
}

#[derive(Clone, Copy)]
enum DestroyFilter {
    PowerAtMost2,
    PowerAtLeast4,
    ArtifactOrEnchantment,
}

fn destroy_cases() -> [DestroyCase; 3] {
    [
        DestroyCase {
            id: "foul_play",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            filter: DestroyFilter::PowerAtMost2,
        },
        DestroyCase {
            id: "humble_the_brute",
            mana: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            filter: DestroyFilter::PowerAtLeast4,
        },
        DestroyCase {
            id: "root_out",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            filter: DestroyFilter::ArtifactOrEnchantment,
        },
    ]
}

struct SacCase {
    id: &'static str,
    mana: ManaGift,
    flashback: Option<ManaGift>,
    treasure: bool,
}

fn sac_cases() -> [SacCase; 3] {
    [
        SacCase {
            id: "costly_plunder",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            flashback: None,
            treasure: false,
        },
        SacCase {
            id: "deadly_dispute",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            flashback: None,
            treasure: true,
        },
        SacCase {
            id: "eviscerators_insight",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            flashback: Some(ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            }),
            treasure: false,
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("swamp", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn object_ref(e: &GameEngine, object_id: u32) -> CostObjectRef {
    CostObjectRef {
        object_id,
        zone_change_generation: e
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
    }
}

fn object_cost(e: &GameEngine, option_index: u32, objects: &[u32]) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        battlefield_objects: Some(CostObjectRefs {
            objects: objects.iter().map(|oid| object_ref(e, *oid)).collect(),
        }),
        ..Default::default()
    }
}

fn graveyard_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        group_index: 0,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Graveyard as i32,
        ..Default::default()
    }]
}

fn flashback_cast(spell: u32, generation: u64, targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets,
            ..Default::default()
        })),
    }
}

// ---------------------------------------------------------------------------
// 07-flashback-graveyard

fn cast_yard(e: &mut GameEngine, case: &YardCase, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, graveyard_target(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
}

#[test]
fn q56_c_canary_simplest_dryads_revival_returns_a_card_and_flashes_back() {
    let case = &yard_cases()[0];
    let mut e = engine(56_700_001);
    let first = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let second = inject_graveyard_card(&mut e, 0, "forest");
    cast_yard(&mut e, case, first);
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&first].zone, Zone::Hand);
    assert!(e.state.players[0].hand.contains(&first));
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);

    let spell = e
        .state
        .objects
        .values()
        .find(|object| object.card_id == case.id)
        .expect("resolved spell")
        .id;
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(
        0,
        &flashback_cast(spell, generation, graveyard_target(second)),
    )
    .expect("flashback");
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact flashback cost");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&second].zone, Zone::Hand);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}

#[test]
fn q56_c_canary_hardest_mystic_retrieval_filters_and_exiles_after_flashback() {
    let case = &yard_cases()[2];
    let mut e = engine(56_700_002);
    let instant = inject_graveyard_card(&mut e, 0, "lightning_bolt");
    let creature = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    cast_yard(&mut e, case, instant);
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&instant].zone, Zone::Hand);
    assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);

    let spell = e
        .state
        .objects
        .values()
        .find(|object| object.card_id == case.id)
        .expect("resolved spell")
        .id;
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    assert!(
        e.apply_scenario_command(
            0,
            &flashback_cast(spell, generation, graveyard_target(creature)),
        )
        .is_err(),
        "the creature card is an illegal target"
    );
    assert_eq!(
        pool(&e, 0),
        (0, 0, 0, 1, 0, 2),
        "rejected target keeps mana"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    let bolt = inject_graveyard_card(&mut e, 0, "shock");
    e.apply_scenario_command(
        0,
        &flashback_cast(spell, generation, graveyard_target(bolt)),
    )
    .expect("flashback at an instant card");
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Hand);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}

#[test]
fn q56_c_flashback_graveyard_cards_return_the_chosen_card_and_exile_the_spell() {
    for (index, case) in yard_cases().iter().enumerate() {
        let mut e = engine(56_701_000 + index as u64);
        let (target_card, illegal_card) = match case.filter {
            YardFilter::Any => ("forest", "grizzly_bears"),
            YardFilter::Creature => ("grizzly_bears", "forest"),
            YardFilter::InstantOrSorcery => ("lightning_bolt", "grizzly_bears"),
        };
        let target = inject_graveyard_card(&mut e, 0, target_card);
        let illegal = inject_graveyard_card(&mut e, 0, illegal_card);
        let opponent = inject_graveyard_card(&mut e, 1, target_card);
        cast_yard(&mut e, case, target);
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Hand, "{}", case.id);
        assert_eq!(
            e.state.objects[&illegal].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&opponent].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );

        let spell = e
            .state
            .objects
            .values()
            .find(|object| object.card_id == case.id && object.zone == Zone::Graveyard)
            .expect("resolved spell")
            .id;
        let generation = e.state.zone_change_generation[&spell];
        let next = inject_graveyard_card(&mut e, 0, target_card);
        give_mana(&mut e, 0, gift(&case.flashback));
        e.apply_scenario_command(
            0,
            &flashback_cast(spell, generation, graveyard_target(next)),
        )
        .unwrap_or_else(|error| panic!("{}: flashback: {error:?}", case.id));
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&next].zone, Zone::Hand, "{}", case.id);
        assert_eq!(e.state.objects[&spell].zone, Zone::Exile, "{}", case.id);
    }
}

#[test]
fn q56_c_flashback_graveyard_cards_reject_an_opponents_graveyard() {
    for (index, case) in yard_cases().iter().enumerate() {
        let mut e = engine(56_702_000 + index as u64);
        let opponent = inject_graveyard_card(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let before = pool(&e, 0);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, graveyard_target(opponent)))
                .is_err(),
            "{}: only the caster's graveyard is legal",
            case.id
        );
        assert_eq!(pool(&e, 0), before, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

// ---------------------------------------------------------------------------
// 08-destroy-investigate

fn cast_destroy(e: &mut GameEngine, case: &DestroyCase, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
}

#[test]
fn q56_c_canary_simplest_foul_play_destroys_a_small_creature_and_investigates() {
    let case = &destroy_cases()[0];
    let mut e = engine(56_800_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_destroy(&mut e, case, target);
    assert!(
        battlefield_token_oids(&e, 0, "clue").is_empty(),
        "no Clue before resolution"
    );
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 1);
}

#[test]
fn q56_c_canary_hardest_root_out_destroys_an_artifact_or_enchantment_and_investigates() {
    let case = &destroy_cases()[2];
    let mut e = engine(56_800_002);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bone_saw");
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_destroy(&mut e, case, artifact);
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 1);

    let mut second = engine(56_800_003);
    let artifact = inject_permanent_on_battlefield(&mut second, 1, "bone_saw");
    let creature = inject_creature_on_battlefield(&mut second, 1, "grizzly_bears");
    inject_card_into_hand(&mut second, 0, case.id);
    give_mana(&mut second, 0, gift(&case.mana));
    let slot = hand_index_for_card(&second, 0, case.id);
    assert!(
        second
            .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .is_err(),
        "a creature is not an artifact or enchantment"
    );
    assert_eq!(second.state.objects[&artifact].zone, Zone::Battlefield);
}

#[test]
fn q56_c_destroy_investigate_cards_apply_their_power_filters() {
    for (index, case) in destroy_cases().iter().enumerate() {
        let (legal_power, illegal_power) = match case.filter {
            DestroyFilter::PowerAtMost2 => (2u32, 3u32),
            DestroyFilter::PowerAtLeast4 => (4, 3),
            DestroyFilter::ArtifactOrEnchantment => (2, 2),
        };
        let mut e = engine(56_801_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", legal_power, 2);
        let illegal = inject_creature_with_stats(&mut e, 1, "grizzly_bears", illegal_power, 2);
        if matches!(case.filter, DestroyFilter::ArtifactOrEnchantment) {
            // The legal target for Root Out is an artifact, not a creature.
            e.state.players[1].battlefield.retain(|oid| *oid != target);
            e.state.objects.get_mut(&target).unwrap().zone = Zone::Exile;
            e.state.players[1].exile.push(target);
            *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
            let bone_saw = inject_permanent_on_battlefield(&mut e, 1, "bone_saw");
            cast_destroy(&mut e, case, bone_saw);
            pass_both_players(&mut e);
            assert_eq!(
                e.state.objects[&bone_saw].zone,
                Zone::Graveyard,
                "{}",
                case.id
            );
        } else {
            cast_destroy(&mut e, case, target);
            pass_both_players(&mut e);
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Graveyard,
                "{}",
                case.id
            );
        }
        assert_eq!(
            e.state.objects[&illegal].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            battlefield_token_oids(&e, 0, "clue").len(),
            1,
            "{}",
            case.id
        );
    }
}

#[test]
fn q56_c_destroy_investigate_cards_do_not_investigate_for_a_stale_target() {
    for (index, case) in destroy_cases().iter().enumerate() {
        let mut e = engine(56_802_000 + index as u64);
        let target = match case.filter {
            DestroyFilter::ArtifactOrEnchantment => {
                inject_permanent_on_battlefield(&mut e, 1, "bone_saw")
            }
            DestroyFilter::PowerAtMost2 => {
                inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2)
            }
            DestroyFilter::PowerAtLeast4 => {
                inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 2)
            }
        };
        cast_destroy(&mut e, case, target);
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).unwrap().zone = Zone::Exile;
        e.state.players[1].exile.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{}", case.id);
        assert!(
            battlefield_token_oids(&e, 0, "clue").is_empty(),
            "{}: a fizzled spell does not investigate",
            case.id
        );
    }
}

// ---------------------------------------------------------------------------
// 09-sac-draw

fn cast_sacrificing(e: &mut GameEngine, case: &SacCase, victim: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(
        0,
        &cast_spell_with_cast_cost_groups(slot, vec![], vec![object_cost(e, 0, &[victim])]),
    )
    .unwrap_or_else(|error| panic!("{}: sacrifice and cast: {error:?}", case.id));
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
}

#[test]
fn q56_c_canary_simplest_costly_plunder_sacrifices_an_artifact_and_draws_two() {
    let case = &sac_cases()[0];
    let mut e = engine(56_900_001);
    let victim = inject_permanent_on_battlefield(&mut e, 0, "bone_saw");
    let opponent_hand = e.state.players[1].hand.len();
    cast_sacrificing(&mut e, case, victim);
    let hand_after_payment = e.state.players[0].hand.len();
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_after_payment + 2);
    assert_eq!(e.state.players[1].hand.len(), opponent_hand);
}

#[test]
fn q56_c_canary_hardest_eviscerators_insight_flashes_back_with_its_sacrifice_cost() {
    let case = &sac_cases()[2];
    let mut e = engine(56_900_002);
    let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_sacrificing(&mut e, case, first);
    let hand_after_payment = e.state.players[0].hand.len();
    pass_both_players(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_after_payment + 2);
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);

    let spell = e
        .state
        .objects
        .values()
        .find(|object| object.card_id == case.id && object.zone == Zone::Graveyard)
        .expect("resolved spell")
        .id;
    let generation = e.state.zone_change_generation[&spell];
    let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let flashback = case.flashback.as_ref().expect("flashback cost");
    give_mana(&mut e, 0, gift(flashback));
    let before = e.state.players[0].hand.len();
    let command = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            cast_cost_group_selections: vec![object_cost(&e, 0, &[second])],
            ..Default::default()
        })),
    };
    e.apply_scenario_command(0, &command)
        .expect("flashback with the additional sacrifice");
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[0].hand.len(), before + 2);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}

#[test]
fn q56_c_sac_draw_cards_sacrifice_an_artifact_or_creature_and_draw_exactly_two() {
    for (index, case) in sac_cases().iter().enumerate() {
        let mut e = engine(56_901_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, "bone_saw");
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let victim = if index % 2 == 0 { artifact } else { creature };
        cast_sacrificing(&mut e, case, victim);
        let hand_after_payment = e.state.players[0].hand.len();
        let opponent_hand = e.state.players[1].hand.len();
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_after_payment + 2,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[1].hand.len(), opponent_hand, "{}", case.id);
        assert_eq!(
            e.state.objects[&victim].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            battlefield_token_oids(&e, 0, "treasure").len(),
            if case.treasure { 1 } else { 0 },
            "{}: only Deadly Dispute makes a Treasure",
            case.id
        );
    }
}

#[test]
fn q56_c_sac_draw_cards_require_an_artifact_or_creature_and_reject_a_land() {
    for (index, case) in sac_cases().iter().enumerate() {
        let mut e = engine(56_902_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let command_index = e.state.command_index;
        let hand_before = e.state.players[0].hand.len();
        assert!(
            e.apply_scenario_command(0, &cast_spell_with_cast_cost_groups(slot, vec![], vec![]))
                .is_err(),
            "{}: the sacrifice is mandatory",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        assert!(
            e.apply_scenario_command(
                0,
                &cast_spell_with_cast_cost_groups(slot, vec![], vec![object_cost(&e, 0, &[land])])
            )
            .is_err(),
            "{}: a land is not an artifact or creature",
            case.id
        );
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
