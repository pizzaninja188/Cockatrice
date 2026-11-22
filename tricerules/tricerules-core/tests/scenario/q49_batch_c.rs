//! Actual-card scenarios for the queue-v49 reviewed cohorts (batch C).
//!
//! Expectations are printed Oracle values. CR 603.6a governs the enters triggers,
//! CR 115.1d the graveyard-card target, CR 400.7 the new object after the move,
//! CR 605.1a/605.3a-b the sacrifice mana abilities, CR 603.4 raid intervening-if,
//! CR 508.1 the attacked-this-turn memory and CR 614.1c/614.12 enters-with-counters.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, RuledCommand};

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

fn choose_target(oid: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(oid),
        })),
    }
}

fn resolve_etb_target(e: &mut GameEngine, target: u32) {
    e.apply_scenario_command(0, &choose_target(target))
        .unwrap_or_else(|error| panic!("choose ETB target: {error:?}"));
    resolve_entire_stack_two_player(e);
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn advance_to_main2(e: &mut GameEngine) {
    for _ in 0..20 {
        let actor = e.state.priority_player_id();
        e.apply_scenario_command(actor, &pass())
            .expect("pass through combat");
        if e.state.turn_step == TurnStep::Main2 {
            return;
        }
    }
    panic!("combat did not reach main2");
}

// ── 06 Graveyard to hand ETB ───────────────────────────────────────────────

#[test]
fn q49_graveyard_hand_canary_simplest_archaeomender_returns_an_artifact() {
    let mut e = main1_engine(49_060_001, &["archaeomender"], &[]);
    let artifact = inject_graveyard_card(&mut e, 0, "bonesplitter");
    move_ready_to_battlefield(&mut e, 0, "archaeomender");
    resolve_etb_target(&mut e, artifact);
    assert!(e.state.players[0].hand.contains(&artifact));
    assert_eq!(e.state.objects[&artifact].zone, Zone::Hand);
}

#[test]
fn q49_graveyard_hand_canary_hardest_warden_of_the_eye_excludes_creature_and_land() {
    let mut e = main1_engine(49_060_002, &["warden_of_the_eye"], &[]);
    let instant = inject_graveyard_card(&mut e, 0, "shock");
    let creature = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let land = inject_graveyard_card(&mut e, 0, "forest");
    let warden = move_ready_to_battlefield(&mut e, 0, "warden_of_the_eye");
    assert_eq!(e.state.objects[&warden].zone, Zone::Battlefield);
    assert!(
        e.apply_scenario_command(0, &choose_target(creature))
            .is_err(),
        "a creature card is not a legal target"
    );
    assert!(
        e.apply_scenario_command(0, &choose_target(land)).is_err(),
        "a land card is not a legal target"
    );
    resolve_etb_target(&mut e, instant);
    assert!(e.state.players[0].hand.contains(&instant));
    assert_eq!(e.state.objects[&instant].zone, Zone::Hand);
    assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
}

#[test]
fn q49_graveyard_hand_cards_return_their_printed_graveyard_card() {
    struct Case {
        id: &'static str,
        card: &'static str,
        illegal: &'static str,
    }
    for (index, case) in [
        Case {
            id: "archaeomender",
            card: "bonesplitter",
            illegal: "grizzly_bears",
        },
        Case {
            id: "barrow_witches",
            card: "white_knight",
            illegal: "grizzly_bears",
        },
        Case {
            id: "golgari_findbroker",
            card: "grizzly_bears",
            illegal: "shock",
        },
        Case {
            id: "monk_idealist",
            card: "pacifism",
            illegal: "grizzly_bears",
        },
        Case {
            id: "restoration_gearsmith",
            card: "bonesplitter",
            illegal: "shock",
        },
        Case {
            id: "warden_of_the_eye",
            card: "shock",
            illegal: "grizzly_bears",
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_061_000 + index as u64, &[case.id], &[]);
        let legal = inject_graveyard_card(&mut e, 0, case.card);
        let second_legal = inject_graveyard_card(&mut e, 0, case.card);
        let opponent_legal = inject_graveyard_card(&mut e, 1, case.card);
        let illegal = inject_graveyard_card(&mut e, 0, case.illegal);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
        assert!(
            e.apply_scenario_command(0, &choose_target(illegal))
                .is_err(),
            "{}: {} is not a legal target",
            case.id,
            case.illegal
        );
        assert!(
            e.apply_scenario_command(0, &choose_target(opponent_legal))
                .is_err(),
            "{}: an opponent's graveyard card is not a legal target",
            case.id
        );
        resolve_etb_target(&mut e, legal);
        assert_eq!(
            e.state.objects[&legal].zone,
            Zone::Hand,
            "{}: returned {}",
            case.id,
            case.card
        );
        assert!(
            e.state.players[0].hand.contains(&legal),
            "{}: in hand",
            case.id
        );
        assert_eq!(
            e.state.objects[&illegal].zone,
            Zone::Graveyard,
            "{}: the illegal card stays",
            case.id
        );
        assert_eq!(
            e.state.objects[&second_legal].zone,
            Zone::Graveyard,
            "{}: only the chosen legal card returns",
            case.id
        );
        assert_eq!(
            e.state.objects[&opponent_legal].zone,
            Zone::Graveyard,
            "{}: the opponent's graveyard is untouched",
            case.id
        );
    }
}

#[test]
fn q49_graveyard_hand_stale_target_is_not_returned() {
    let mut e = main1_engine(49_062_001, &["archaeomender"], &[]);
    let artifact = inject_graveyard_card(&mut e, 0, "bonesplitter");
    move_ready_to_battlefield(&mut e, 0, "archaeomender");
    e.apply_scenario_command(0, &choose_target(artifact))
        .expect("choose the artifact");
    e.state.objects.get_mut(&artifact).expect("artifact").zone = Zone::Exile;
    e.state.players[0].graveyard.retain(|oid| *oid != artifact);
    e.state.players[0].exile.push(artifact);
    *e.state.zone_change_generation.entry(artifact).or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut e);
    assert!(
        !e.state.players[0].hand.contains(&artifact),
        "a stale graveyard target is not returned"
    );
    assert_eq!(e.state.objects[&artifact].zone, Zone::Exile);
}

// ── 07 Sacrifice for mana ──────────────────────────────────────────────────

#[test]
fn q49_sacrifice_mana_canary_simplest_morgue_toad_makes_blue_and_red() {
    let mut e = main1_engine(49_070_001, &["morgue_toad"], &[]);
    let toad = move_ready_to_battlefield(&mut e, 0, "morgue_toad");
    apply_ability(&mut e, 0, toad, 0, vec![]).expect("sacrifice Morgue Toad");
    assert_eq!(pool(&e), (0, 1, 0, 1, 0, 0));
    assert_eq!(e.state.objects[&toad].zone, Zone::Graveyard);
    assert!(
        e.state.stack.is_empty(),
        "a mana ability does not use the stack"
    );
}

#[test]
fn q49_sacrifice_mana_canary_hardest_overeager_apprentice_discards_then_makes_bbb() {
    let mut e = main1_engine(49_070_002, &["overeager_apprentice"], &[]);
    let apprentice = move_ready_to_battlefield(&mut e, 0, "overeager_apprentice");
    let pitched = inject_card_into_hand(&mut e, 0, "grizzly_bears");
    let hand_before = e.state.players[0].hand.len();
    let mut command = activate_ability_with_costs(
        apprentice,
        0,
        vec![],
        vec![hand_cost_selection(0, (hand_before - 1) as u32)],
    );
    if let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() {
        ability.expected_zone_change_generation = e
            .state
            .zone_change_generation
            .get(&apprentice)
            .copied()
            .unwrap_or(0);
    }
    e.apply_scenario_command(0, &command)
        .expect("discard and sacrifice Overeager Apprentice");
    assert_eq!(pool(&e), (0, 0, 3, 0, 0, 0));
    assert_eq!(e.state.objects[&apprentice].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1,
        "the discard cost was paid"
    );
    assert_eq!(e.state.objects[&pitched].zone, Zone::Graveyard);
}

#[test]
fn q49_sacrifice_mana_cards_produce_their_printed_mana() {
    struct Case {
        id: &'static str,
        cost: ManaGift,
        expected: (u32, u32, u32, u32, u32, u32),
    }
    for (index, case) in [
        Case {
            id: "basal_thrull",
            cost: ManaGift::default(),
            expected: (0, 0, 2, 0, 0, 0),
        },
        Case {
            id: "coal_golem",
            cost: ManaGift {
                c: 3,
                ..Default::default()
            },
            expected: (0, 0, 0, 3, 0, 0),
        },
        Case {
            id: "composite_golem",
            cost: ManaGift::default(),
            expected: (1, 1, 1, 1, 1, 0),
        },
        Case {
            id: "morgue_toad",
            cost: ManaGift::default(),
            expected: (0, 1, 0, 1, 0, 0),
        },
        Case {
            id: "satyr_hedonist",
            cost: ManaGift {
                r: 1,
                ..Default::default()
            },
            expected: (0, 0, 0, 3, 0, 0),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_071_000 + index as u64, &[case.id], &[]);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.cost));
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(pool(&e), case.expected, "{}", case.id);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: sacrificed",
            case.id
        );
    }
}

// ── 08 Raid simple (filled below) ──────────────────────────────────────────

// ── 08 Raid simple ─────────────────────────────────────────────────────────

fn choose_player_target(pid: i32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_player(pid),
        })),
    }
}

fn to_raid_main2(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("ap begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("nap begin combat");
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
    let attacker = battlefield_object_for_card(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare an attacker for raid");
    advance_to_main2(e);
    assert_eq!(e.state.turn_step, TurnStep::Main2);
}

#[test]
fn q49_raid_canary_simplest_swaggering_corsair_enters_with_a_counter_after_attacking() {
    let mut e = main1_engine(49_080_001, &["swaggering_corsair"], &[]);
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    to_raid_main2(&mut e);
    let corsair = move_ready_to_battlefield(&mut e, 0, "swaggering_corsair");
    assert_eq!(
        e.state.objects[&corsair].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn q49_raid_without_an_attack_does_not_add_the_counter() {
    let mut e = main1_engine(49_080_002, &["swaggering_corsair"], &[]);
    let corsair = move_ready_to_battlefield(&mut e, 0, "swaggering_corsair");
    assert_eq!(
        e.state.objects[&corsair].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn q49_raid_canary_hardest_deadeye_tormentor_forces_a_discard() {
    let mut e = main1_engine(49_080_003, &["deadeye_tormentor"], &[]);
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    to_raid_main2(&mut e);
    let opponent_hand_before = e.state.players[1].hand.len();
    move_ready_to_battlefield(&mut e, 0, "deadeye_tormentor");
    e.apply_scenario_command(0, &choose_player_target(1))
        .expect("choose the opposing player");
    let batch = pass_both_players_with_batch(&mut e);
    let choice = find_resolution_choice(&batch).expect("discard choice");
    e.apply_scenario_command(
        1,
        &submit_resolution_choice(vec![choice.candidate_object_ids[0]]),
    )
    .expect("discard one card");
    assert_eq!(e.state.players[1].hand.len(), opponent_hand_before - 1);
}

#[test]
fn q49_raid_cards_apply_their_printed_effect() {
    for (index, id) in [
        "deadeye_tormentor",
        "mardu_heart-piercer",
        "storm_fleet_pyromancer",
        "swaggering_corsair",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_081_000 + index as u64, &[id], &[]);
        inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        to_raid_main2(&mut e);
        let life_before = e.state.players[1].life;
        let source = move_ready_to_battlefield(&mut e, 0, id);
        match id {
            "deadeye_tormentor" => {
                let hand_before = e.state.players[1].hand.len();
                e.apply_scenario_command(0, &choose_player_target(1))
                    .expect("choose the opposing player");
                let batch = pass_both_players_with_batch(&mut e);
                let choice = find_resolution_choice(&batch).expect("discard choice");
                e.apply_scenario_command(
                    1,
                    &submit_resolution_choice(vec![choice.candidate_object_ids[0]]),
                )
                .expect("discard one card");
                assert_eq!(e.state.players[1].hand.len(), hand_before - 1, "{id}");
            }
            "mardu_heart-piercer" | "storm_fleet_pyromancer" => {
                e.apply_scenario_command(0, &choose_player_target(1))
                    .expect("choose the opposing player");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(e.state.players[1].life, life_before - 2, "{id}");
            }
            "swaggering_corsair" => {
                assert_eq!(
                    e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
                    1,
                    "{id}"
                );
            }
            other => panic!("unhandled {other}"),
        }
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

#[test]
fn q49_raid_triggers_require_an_attack() {
    for (index, id) in [
        "deadeye_tormentor",
        "mardu_heart-piercer",
        "storm_fleet_pyromancer",
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_082_000 + index as u64, &[id], &[]);
        let life_before = e.state.players[1].life;
        let hand_before = e.state.players[1].hand.len();
        move_ready_to_battlefield(&mut e, 0, id);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{id}: no staged raid trigger without an attack"
        );
        assert!(
            e.state.stack.is_empty(),
            "{id}: no trigger without an attack"
        );
        assert_eq!(e.state.players[1].life, life_before, "{id}: no damage");
        assert_eq!(
            e.state.players[1].hand.len(),
            hand_before,
            "{id}: no discard"
        );
    }
}

#[test]
fn q49_raid_remembers_an_attacker_that_left_the_battlefield() {
    let mut e = main1_engine(49_082_010, &["mardu_heart-piercer"], &[]);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("ap begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("nap begin combat");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    pass_both_players(&mut e);
    cast_named(
        &mut e,
        0,
        "go_for_the_throat",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
        target_object(attacker),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&attacker].zone, Zone::Graveyard);
    advance_to_main2(&mut e);
    let life_before = e.state.players[1].life;
    move_ready_to_battlefield(&mut e, 0, "mardu_heart-piercer");
    e.apply_scenario_command(0, &choose_player_target(1))
        .expect("choose the opposing player");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, life_before - 2);
}

#[test]
fn q49_raid_damage_targets_a_creature_and_rejects_a_land() {
    for (index, id) in ["mardu_heart-piercer", "storm_fleet_pyromancer"]
        .into_iter()
        .enumerate()
    {
        let mut e = main1_engine(49_082_020 + index as u64, &[id], &["grizzly_bears"]);
        inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let victim = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 3);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        to_raid_main2(&mut e);
        move_ready_to_battlefield(&mut e, 0, id);
        assert!(
            e.apply_scenario_command(0, &choose_target(land)).is_err(),
            "{id}: a land is not an any-target damage target"
        );
        e.apply_scenario_command(0, &choose_target(victim))
            .expect("choose the opposing creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&victim].damage, 2, "{id}");
    }
}

#[test]
fn q49_raid_deadeye_tormentor_requires_an_opponent_and_discards() {
    let mut e = main1_engine(49_082_030, &["deadeye_tormentor"], &[]);
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    to_raid_main2(&mut e);
    let hand_before = e.state.players[1].hand.len();
    move_ready_to_battlefield(&mut e, 0, "deadeye_tormentor");
    assert!(
        e.apply_scenario_command(0, &choose_player_target(0))
            .is_err(),
        "the controller is not an opponent target"
    );
    e.apply_scenario_command(0, &choose_player_target(1))
        .expect("choose the opposing player");
    let batch = pass_both_players_with_batch(&mut e);
    let choice = find_resolution_choice(&batch).expect("discard choice");
    let discarded = choice.candidate_object_ids[0];
    e.apply_scenario_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("discard one card");
    assert_eq!(e.state.players[1].hand.len(), hand_before - 1);
    assert_eq!(e.state.objects[&discarded].zone, Zone::Graveyard);
}
