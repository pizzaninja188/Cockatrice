//! Queue-v29 canaries: "{T}, Sacrifice an artifact: This creature deals 2 damage to any target."
//!
//! CR 302.6/602.5a, 602.1/602.2, 701.21a, 115.1 and 608.2b. Expectations are printed Oracle values.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn case(id: &'static str) -> Case {
    match id {
        "orcish_vandal" => Case {
            id,
            power: 1,
            toughness: 1,
        },
        "barrage_ogre" => Case {
            id,
            power: 3,
            toughness: 3,
        },
        "orcish_mechanics" => Case {
            id,
            power: 1,
            toughness: 1,
        },
        _ => unreachable!(),
    }
}

fn cases() -> [Case; 3] {
    [
        case("orcish_mechanics"),
        case("orcish_vandal"),
        case("barrage_ogre"),
    ]
}

fn mana_for(id: &str) -> ManaGift {
    match id {
        "orcish_mechanics" => ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
        "orcish_vandal" => ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
        "barrage_ogre" => ManaGift {
            r: 2,
            c: 3,
            ..Default::default()
        },
        _ => unreachable!(),
    }
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
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

fn ready_source(e: &mut GameEngine, case: &Case) -> u32 {
    let source = move_ready_to_battlefield(e, 0, case.id);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "{}: printed body",
        case.id
    );
    source
}

fn activation_command(
    e: &GameEngine,
    source: u32,
    artifact: u32,
    target: u32,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_with_costs(
        source,
        0,
        target_object(target),
        vec![permanent_cost_selection(1, artifact)],
    );
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = e
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    command
}

fn activate(e: &mut GameEngine, source: u32, artifact: u32, target: u32) {
    e.apply_scenario_command(0, &activation_command(e, source, artifact, target))
        .expect("activate the artifact-sacrifice ping");
    resolve_entire_stack_two_player(e);
}

#[test]
fn q29_sac_artifact_ping_canary_simplest() {
    let case = case("orcish_vandal");
    let mut e = engine(29_010_001, case.id);
    let source = ready_source(&mut e, &case);
    let artifact = inject_permanent_on_battlefield(&mut e, 0, "swiftfoot_boots");
    let victim = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    activate(&mut e, source, artifact, victim);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert!(e.state.objects[&source].tapped);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q29_sac_artifact_ping_canary_hardest() {
    let case = case("barrage_ogre");
    let mut e = engine(29_010_002, case.id);
    let source = ready_source(&mut e, &case);
    let artifact = inject_permanent_on_battlefield(&mut e, 0, "swiftfoot_boots");
    let victim = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    activate(&mut e, source, artifact, victim);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert!(e.state.objects[&source].tapped);
}

#[test]
fn q29_sac_artifact_ping_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_011_000 + index as u64, case.id);
        give_mana(&mut e, 0, gift(&mana_for(case.id)));
        inject_card_into_hand(&mut e, 0, case.id);
        let hand = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        let pool = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed cost",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
    }
}

#[test]
fn q29_sac_artifact_ping_cards_deal_exactly_two_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_012_000 + index as u64, case.id);
        let source = ready_source(&mut e, &case);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, "swiftfoot_boots");
        let victim = inject_creature_with_stats(&mut e, 1, "gavony_unhallowed", 2, 4);
        activate(&mut e, source, artifact, victim);
        assert_eq!(e.state.objects[&victim].damage, 2, "{}", case.id);
        assert_eq!(
            (e.effective_power(victim), e.effective_toughness(victim)),
            (Some(2), Some(4)),
            "{}: two damage does not kill a 2/4",
            case.id
        );
    }
}

#[test]
fn q29_sac_artifact_ping_cards_reject_missing_artifact_tapped_and_sick_sources() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut no_artifact = engine(29_013_000 + index as u64, case.id);
        let source = ready_source(&mut no_artifact, &case);
        let victim = inject_creature_on_battlefield(&mut no_artifact, 1, "grizzly_bears");
        let command = activation_command(&no_artifact, source, 999_999, victim);
        assert!(
            no_artifact.apply_scenario_command(0, &command).is_err(),
            "{}: no artifact is available to sacrifice",
            case.id
        );
        assert!(!no_artifact.state.objects[&source].tapped, "{}", case.id);

        let mut tapped = engine(29_014_000 + index as u64, case.id);
        let source = ready_source(&mut tapped, &case);
        let artifact = inject_permanent_on_battlefield(&mut tapped, 0, "swiftfoot_boots");
        let victim = inject_creature_on_battlefield(&mut tapped, 1, "grizzly_bears");
        activate(&mut tapped, source, artifact, victim);
        let second_artifact = inject_permanent_on_battlefield(&mut tapped, 0, "swiftfoot_boots");
        let second_victim = inject_creature_on_battlefield(&mut tapped, 1, "grizzly_bears");
        assert!(
            tapped
                .apply_scenario_command(
                    0,
                    &activation_command(&tapped, source, second_artifact, second_victim),
                )
                .is_err(),
            "{}: a tapped source cannot activate again",
            case.id
        );

        let mut sick = engine(29_015_000 + index as u64, case.id);
        give_mana(&mut sick, 0, gift(&mana_for(case.id)));
        inject_card_into_hand(&mut sick, 0, case.id);
        let hand = hand_index_for_card(&sick, 0, case.id);
        sick.apply_scenario_command(0, &cast_spell(hand, vec![]))
            .expect("cast the summoning-sick source");
        resolve_entire_stack_two_player(&mut sick);
        let source = battlefield_object_for_card(&sick, 0, case.id);
        let artifact = inject_permanent_on_battlefield(&mut sick, 0, "swiftfoot_boots");
        let victim = inject_creature_on_battlefield(&mut sick, 1, "grizzly_bears");
        assert!(
            sick.apply_scenario_command(0, &activation_command(&sick, source, artifact, victim),)
                .is_err(),
            "{}: summoning sickness rejects the {{T}} cost",
            case.id
        );
    }
}

#[test]
fn q29_sac_artifact_ping_stale_target_still_pays_the_artifact_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_016_000 + index as u64, case.id);
        let source = ready_source(&mut e, &case);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, "swiftfoot_boots");
        let victim = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let command = activation_command(&e, source, artifact, victim);
        e.apply_scenario_command(0, &command)
            .expect("activate before the target leaves");
        e.state.players[1]
            .battlefield
            .retain(|candidate| *candidate != victim);
        e.state.players[1].graveyard.push(victim);
        e.state.objects.get_mut(&victim).expect("object").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(victim).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: the cost was already paid",
            case.id
        );
        assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
        assert_eq!(e.state.players[1].life, 20, "{}: no player damage", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
