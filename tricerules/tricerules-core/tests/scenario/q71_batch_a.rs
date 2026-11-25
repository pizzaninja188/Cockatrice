//! Actual-card scenarios for the queue-v71 batch A cohorts.
//!
//! Expectations are printed Oracle values. Storm uses CR 702.40a copies that are not cast; each
//! targeted copy chooses its own target. Flashback uses CR 702.34a-d and always exiles. Cycling is
//! a hand-zone activated ability under CR 702.29a. Target legality follows CR 115.1/601.2c and
//! 608.2b.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, ActivateAbility, CastMethod, CastSpell, RuledCommand, SubmitResolutionChoice,
};

struct StormCase {
    id: &'static str,
    mana: ManaGift,
}

fn storm_cases() -> [StormCase; 9] {
    [
        StormCase {
            id: "astral_steel",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        StormCase {
            id: "empty_the_warrens",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        StormCase {
            id: "grapeshot",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        StormCase {
            id: "reaping_the_graves",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        },
        StormCase {
            id: "scattershot",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        StormCase {
            id: "temporal_fissure",
            mana: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
        },
        StormCase {
            id: "tendrils_of_agony",
            mana: ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
        },
        StormCase {
            id: "volcanic_awakening",
            mana: ManaGift {
                r: 2,
                c: 4,
                ..Default::default()
            },
        },
        StormCase {
            id: "weather_the_storm",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        },
    ]
}

struct FlashCase {
    id: &'static str,
    mana: ManaGift,
    flashback: ManaGift,
    damage: i32,
    gain: i32,
    card_type: &'static str,
}

fn flash_cases() -> [FlashCase; 4] {
    [
        FlashCase {
            id: "morbid_hunger",
            mana: ManaGift {
                b: 2,
                c: 4,
                ..Default::default()
            },
            flashback: ManaGift {
                b: 2,
                c: 7,
                ..Default::default()
            },
            damage: 3,
            gain: 3,
            card_type: "Sorcery",
        },
        FlashCase {
            id: "sacred_fire",
            mana: ManaGift {
                r: 1,
                w: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                r: 1,
                w: 1,
                c: 4,
                ..Default::default()
            },
            damage: 2,
            gain: 2,
            card_type: "Instant",
        },
        FlashCase {
            id: "scorching_missile",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            flashback: ManaGift {
                r: 1,
                c: 9,
                ..Default::default()
            },
            damage: 4,
            gain: 0,
            card_type: "Sorcery",
        },
        FlashCase {
            id: "smiting_helix",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            flashback: ManaGift {
                r: 1,
                w: 1,
                ..Default::default()
            },
            damage: 3,
            gain: 3,
            card_type: "Sorcery",
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn put_on_top(e: &mut GameEngine, player: usize, card: &str) -> u32 {
    let object = inject_library_card(e, player, card);
    e.state.players[player]
        .library
        .retain(|candidate| *candidate != object);
    e.state.players[player].library.push_front(object);
    object
}

fn hand_ability(e: &GameEngine, source: u32) -> RuledCommand {
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
            ability_index: 0,
            ..Default::default()
        })),
    }
}

fn cast_prelude(e: &mut GameEngine) {
    inject_card_into_hand(e, 0, "divination");
    give_mana(
        e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "divination");
    e.apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast prelude");
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.turn_history.current.spells_cast, 1,
        "one earlier spell this turn"
    );
}

fn cast_storm(e: &mut GameEngine, case: &StormCase, target: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
}

fn choose_copy_target(e: &mut GameEngine, target: u32, case: &str) {
    pass_both_players(e);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap_or_else(|| panic!("{case}: missing copy target choice"));
    assert_eq!(pending.deciding_player, 0, "{case}: caster chooses");
    let chosen = if pending.presentation.candidates.is_empty() {
        Vec::new()
    } else {
        vec![target]
    };
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                resolution_prompt_id: 0,
                chosen_object_ids: chosen,
                ..Default::default()
            })),
        },
    )
    .unwrap_or_else(|error| panic!("{case}: choose copy target: {error:?}"));
    assert_eq!(
        e.state.stack.iter().filter(|item| item.is_copy).count(),
        1,
        "{case}: exactly one Storm copy with one earlier spell"
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn q71_storm_untargeted_copies_multiply_the_printed_effect() {
    // Weather the Storm: 3 life for the original plus one copy.
    let mut e = engine(20_710_001);
    cast_prelude(&mut e);
    let weather = storm_cases()
        .into_iter()
        .find(|case| case.id == "weather_the_storm")
        .unwrap();
    cast_storm(&mut e, &weather, Vec::new());
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 26, "Weather the Storm: 3 + 3 life");
    assert_eq!(
        e.state.turn_history.current.spells_cast, 2,
        "the Storm copy is not cast"
    );

    // Empty the Warrens: two Goblin tokens for the original and two for the copy.
    let mut e = engine(20_710_002);
    cast_prelude(&mut e);
    let warrens = storm_cases()
        .into_iter()
        .find(|case| case.id == "empty_the_warrens")
        .unwrap();
    cast_storm(&mut e, &warrens, Vec::new());
    resolve_entire_stack_two_player(&mut e);
    let goblins = e.state.players[0]
        .battlefield
        .iter()
        .filter(|id| e.state.objects[id].card_id == "goblin_r_1_1")
        .count();
    assert_eq!(goblins, 4, "Empty the Warrens: two tokens twice");
    assert_eq!(
        e.state.turn_history.current.spells_cast, 2,
        "the Storm copy is not cast"
    );
}

#[test]
fn q71_storm_targeted_copies_choose_independent_targets() {
    // Astral Steel: original and copy pump different creatures.
    let mut e = engine(20_711_001);
    cast_prelude(&mut e);
    let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let steel = storm_cases()
        .into_iter()
        .find(|c| c.id == "astral_steel")
        .unwrap();
    cast_storm(&mut e, &steel, target_object(first));
    choose_copy_target(&mut e, second, "astral_steel");
    assert_eq!(
        e.effective_power(first),
        Some(3),
        "Astral Steel first power"
    );
    assert_eq!(
        e.effective_toughness(first),
        Some(4),
        "Astral Steel first toughness"
    );
    assert_eq!(
        e.effective_power(second),
        Some(3),
        "Astral Steel second power"
    );
    assert_eq!(
        e.effective_toughness(second),
        Some(4),
        "Astral Steel second toughness"
    );

    // Grapeshot: original hits the player, the copy hits a creature.
    let mut e = engine(20_711_002);
    cast_prelude(&mut e);
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let grapeshot = storm_cases()
        .into_iter()
        .find(|c| c.id == "grapeshot")
        .unwrap();
    cast_storm(&mut e, &grapeshot, target_player(1));
    choose_copy_target(&mut e, creature, "grapeshot");
    assert_eq!(e.state.players[1].life, 19, "Grapeshot player damage");
    assert_eq!(
        e.state.objects[&creature].damage, 1,
        "Grapeshot creature damage"
    );

    // Reaping the Graves: original and copy return different creature cards.
    let mut e = engine(20_711_003);
    cast_prelude(&mut e);
    let first = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    let second = inject_graveyard_card(&mut e, 0, "storm_crow");
    let reaping = storm_cases()
        .into_iter()
        .find(|c| c.id == "reaping_the_graves")
        .unwrap();
    cast_storm(&mut e, &reaping, target_object(first));
    choose_copy_target(&mut e, second, "reaping_the_graves");
    assert_eq!(e.state.objects[&first].zone, Zone::Hand);
    assert_eq!(e.state.objects[&second].zone, Zone::Hand);

    // Scattershot: one damage to each of two different creatures.
    let mut e = engine(20_711_004);
    cast_prelude(&mut e);
    let first = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut e, 1, "storm_crow", 1, 2);
    let scattershot = storm_cases()
        .into_iter()
        .find(|c| c.id == "scattershot")
        .unwrap();
    cast_storm(&mut e, &scattershot, target_object(first));
    choose_copy_target(&mut e, second, "scattershot");
    assert_eq!(e.state.objects[&first].damage, 1);
    assert_eq!(e.state.objects[&second].damage, 1);

    // Temporal Fissure: original and copy return different permanents.
    let mut e = engine(20_711_005);
    cast_prelude(&mut e);
    let first = inject_permanent_on_battlefield(&mut e, 1, "ornithopter");
    let second = inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    let fissure = storm_cases()
        .into_iter()
        .find(|c| c.id == "temporal_fissure")
        .unwrap();
    cast_storm(&mut e, &fissure, target_object(first));
    choose_copy_target(&mut e, second, "temporal_fissure");
    assert_eq!(e.state.objects[&first].zone, Zone::Hand);
    assert_eq!(e.state.objects[&second].zone, Zone::Hand);

    // Tendrils of Agony: original and copy each drain the only opponent.
    let mut e = engine(20_711_006);
    cast_prelude(&mut e);
    let tendrils = storm_cases()
        .into_iter()
        .find(|c| c.id == "tendrils_of_agony")
        .unwrap();
    cast_storm(&mut e, &tendrils, target_player(1));
    let opponent = u32::try_from(e.state.players[1].id).unwrap();
    choose_copy_target(&mut e, opponent, "tendrils_of_agony");
    assert_eq!(e.state.players[1].life, 16, "Tendrils: lose 2 twice");
    assert_eq!(e.state.players[0].life, 24, "Tendrils: gain 2 twice");

    // Volcanic Awakening: original and copy destroy different lands.
    let mut e = engine(20_711_007);
    cast_prelude(&mut e);
    let first = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let second = inject_permanent_on_battlefield(&mut e, 1, "island");
    let awakening = storm_cases()
        .into_iter()
        .find(|c| c.id == "volcanic_awakening")
        .unwrap();
    cast_storm(&mut e, &awakening, target_object(first));
    choose_copy_target(&mut e, second, "volcanic_awakening");
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&second].zone, Zone::Graveyard);
}

#[test]
fn q71_storm_rejects_an_illegal_target_without_spending() {
    let mut e = engine(20_712_001);
    assert!(tricerules_cards::registry::global()
        .get("grapeshot")
        .is_some());
    cast_prelude(&mut e);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let grapeshot = storm_cases()
        .into_iter()
        .find(|c| c.id == "grapeshot")
        .unwrap();
    inject_card_into_hand(&mut e, 0, grapeshot.id);
    give_mana(&mut e, 0, gift(&grapeshot.mana));
    let slot = hand_index_for_card(&e, 0, grapeshot.id);
    let pool_before = pool(&e, 0);
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
            .is_err(),
        "Grapeshot cannot target a land"
    );
    assert_eq!(pool(&e, 0), pool_before);
    assert!(e.state.stack.is_empty());

    let mut e = engine(20_712_002);
    assert!(tricerules_cards::registry::global()
        .get("scattershot")
        .is_some());
    cast_prelude(&mut e);
    let scattershot = storm_cases()
        .into_iter()
        .find(|c| c.id == "scattershot")
        .unwrap();
    inject_card_into_hand(&mut e, 0, scattershot.id);
    give_mana(&mut e, 0, gift(&scattershot.mana));
    let slot = hand_index_for_card(&e, 0, scattershot.id);
    let pool_before = pool(&e, 0);
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .is_err(),
        "Scattershot requires a creature target"
    );
    assert_eq!(pool(&e, 0), pool_before, "Scattershot: no mana spent");
    assert!(
        e.state.stack.is_empty(),
        "Scattershot: no spell on the stack"
    );
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

#[test]
fn q71_flashback_damage_cards_deal_damage_gain_life_and_exile() {
    for case in flash_cases() {
        let mut e = engine(20_720_000 + case.damage as u64);
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .unwrap_or_else(|error| panic!("{}: first cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - case.damage,
            "{}: first damage",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            20 + case.gain,
            "{}: first life gain",
            case.id
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{}: exact mana", case.id);

        let generation = e.state.zone_change_generation[&spell];
        give_mana(&mut e, 0, gift(&case.flashback));
        e.apply_scenario_command(0, &flashback_cast(spell, 1, generation))
            .unwrap_or_else(|error| panic!("{}: flashback: {error:?}", case.id));
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: exact flashback",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - 2 * case.damage,
            "{}: flashback damage repeats",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            20 + 2 * case.gain,
            "{}: flashback life gain repeats",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Exile,
            "{}: flashback exiles",
            case.id
        );
        let exiled_generation = e.state.zone_change_generation[&spell];
        give_mana(&mut e, 0, gift(&case.flashback));
        assert!(
            e.apply_scenario_command(0, &flashback_cast(spell, 1, exiled_generation))
                .is_err(),
            "{}: an exiled flashback card cannot be recast",
            case.id
        );
    }
}

#[test]
fn q71_flashback_damage_cards_fizzle_without_life_gain_and_still_exile() {
    for case in flash_cases()
        .into_iter()
        .filter(|case| case.id != "scorching_missile")
    {
        let mut e = engine(20_721_000 + case.damage as u64);
        let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .expect("cast at creature");
        move_to_graveyard(&mut e, 1, creature);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: stale target no damage",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: stale target no life",
            case.id
        );
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);

        let generation = e.state.zone_change_generation[&spell];
        let flashback_target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
        give_mana(&mut e, 0, gift(&case.flashback));
        e.apply_scenario_command(0, &flashback_cast(spell, flashback_target, generation))
            .expect("flashback at creature");
        move_to_graveyard(&mut e, 1, flashback_target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: fizzled flashback no damage",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: fizzled flashback no life",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Exile,
            "{}: fizzled flashback still exiles",
            case.id
        );
    }
}

#[test]
fn q71_flashback_damage_target_scope_and_timing() {
    // Scorching Missile takes a player or planeswalker, not a creature.
    let mut e = engine(20_722_001);
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let cases = flash_cases();
    give_mana(&mut e, 0, gift(&cases[2].mana));
    inject_card_into_hand(&mut e, 0, "scorching_missile");
    let slot = hand_index_for_card(&e, 0, "scorching_missile");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .is_err(),
        "Scorching Missile rejects a creature"
    );

    // A sorcery cannot be flashback-cast on the opponent's turn; Sacred Fire can.
    for case in cases {
        let mut e = engine(20_722_100 + case.damage as u64);
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
            .expect("first cast");
        resolve_entire_stack_two_player(&mut e);
        let generation = e.state.zone_change_generation[&spell];
        end_active_turn(&mut e, 0);
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        e.apply_scenario_command(e.state.players[1].id, &pass())
            .expect("opponent passes");
        give_mana(&mut e, 0, gift(&case.flashback));
        let command = flashback_cast(spell, 1, generation);
        if case.card_type == "Sorcery" {
            assert!(
                e.apply_scenario_command(0, &command).is_err(),
                "{}: sorcery timing",
                case.id
            );
            assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
        } else {
            e.apply_scenario_command(0, &command)
                .unwrap_or_else(|error| panic!("{}: instant flashback: {error:?}", case.id));
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(e.state.objects[&spell].zone, Zone::Exile, "{}", case.id);
        }
    }
}

#[test]
fn q71_cycling_removal_cards_cycle_for_the_printed_cost() {
    for (id, cost) in [
        (
            "forsake_the_worldly",
            ManaGift {
                c: 2,
                ..Default::default()
            },
        ),
        (
            "hush",
            ManaGift {
                c: 2,
                ..Default::default()
            },
        ),
        (
            "scrap",
            ManaGift {
                c: 2,
                ..Default::default()
            },
        ),
        (
            "volcanic_submersion",
            ManaGift {
                c: 2,
                ..Default::default()
            },
        ),
        (
            "wipe_clean",
            ManaGift {
                c: 3,
                ..Default::default()
            },
        ),
    ] {
        let mut e = engine(20_730_000 + cost.c as u64);
        let card = inject_card_into_hand(&mut e, 0, id);
        let drawn = put_on_top(&mut e, 0, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        let opponent_hand = e.state.players[1].hand.clone();
        give_mana(&mut e, 0, cost);
        e.apply_scenario_command(0, &hand_ability(&e, card))
            .unwrap_or_else(|error| panic!("{id}: cycle: {error:?}"));
        assert_eq!(
            e.state.objects[&card].zone,
            Zone::Graveyard,
            "{id}: discards self"
        );
        assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "{id}: exact cost");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{id}: draws one");
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{id}");
        assert_eq!(
            e.state.players[1].hand, opponent_hand,
            "{id}: opponent unchanged"
        );
    }
}

#[test]
fn q71_cycling_removal_rejects_unfunded_or_partial_cycling() {
    for (id, partial) in [
        (
            "forsake_the_worldly",
            ManaGift {
                c: 1,
                ..Default::default()
            },
        ),
        (
            "wipe_clean",
            ManaGift {
                c: 2,
                ..Default::default()
            },
        ),
    ] {
        let mut e = engine(20_731_000 + partial.c as u64);
        let definition = tricerules_cards::registry::global()
            .get(id)
            .unwrap_or_else(|| panic!("{id}: missing registration"));
        assert_eq!(
            definition.primary_face().activated_abilities.len(),
            1,
            "{id}: printed cycling ability"
        );
        let card = inject_card_into_hand(&mut e, 0, id);
        let pool_before = pool(&e, 0);
        e.apply_scenario_command(0, &hand_ability(&e, card))
            .expect_err("cycling needs its printed mana");
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{id}");
        assert_eq!(pool(&e, 0), pool_before, "{id}: no mana spent");
        give_mana(&mut e, 0, partial);
        let partial_pool = pool(&e, 0);
        e.apply_scenario_command(0, &hand_ability(&e, card))
            .expect_err("one mana short is not enough");
        assert_eq!(e.state.objects[&card].zone, Zone::Hand, "{id}");
        assert_eq!(pool(&e, 0), partial_pool, "{id}: partial mana kept");
    }
}

#[test]
fn q71_cycling_removal_bodies_hit_their_printed_permanent_types() {
    // Scrap destroys an artifact.
    let mut e = engine(20_732_001);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "ornithopter");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "scrap");
    let slot = hand_index_for_card(&e, 0, "scrap");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
        .expect("cast Scrap");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);

    // Forsake the Worldly exiles either an artifact or an enchantment.
    for (index, card) in ["ornithopter", "propaganda"].into_iter().enumerate() {
        let mut e = engine(20_732_010 + index as u64);
        let permanent = inject_permanent_on_battlefield(&mut e, 1, card);
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, "forsake_the_worldly");
        let slot = hand_index_for_card(&e, 0, "forsake_the_worldly");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(permanent)))
            .expect("cast Forsake the Worldly");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&permanent].zone, Zone::Exile, "{card}");
    }

    // Hush destroys all enchantments, including the caster's.
    let mut e = engine(20_732_020);
    let own = inject_permanent_on_battlefield(&mut e, 0, "propaganda");
    let opposing = inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "hush");
    let slot = hand_index_for_card(&e, 0, "hush");
    e.apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Hush");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard);

    // Volcanic Submersion destroys an artifact or a land.
    for (index, card) in ["ornithopter", "forest"].into_iter().enumerate() {
        let mut e = engine(20_732_030 + index as u64);
        let permanent = inject_permanent_on_battlefield(&mut e, 1, card);
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, "volcanic_submersion");
        let slot = hand_index_for_card(&e, 0, "volcanic_submersion");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(permanent)))
            .expect("cast Volcanic Submersion");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&permanent].zone, Zone::Graveyard, "{card}");
    }

    // Wipe Clean exiles an enchantment.
    let mut e = engine(20_732_040);
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "wipe_clean");
    let slot = hand_index_for_card(&e, 0, "wipe_clean");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(enchantment)))
        .expect("cast Wipe Clean");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Exile);
}

#[test]
fn q71_cycling_removal_stale_targets_fizzle_and_wrong_targets_are_rejected() {
    // Scrap fizzles when its artifact leaves before resolution.
    let mut e = engine(20_733_001);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "ornithopter");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let spell = inject_card_into_hand(&mut e, 0, "scrap");
    let slot = hand_index_for_card(&e, 0, "scrap");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
        .expect("cast Scrap");
    move_to_graveyard(&mut e, 1, artifact);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);

    // Wrong permanent types are rejected without spending.
    for (id, mana, wrong) in [
        (
            "scrap",
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            "forest",
        ),
        (
            "wipe_clean",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            "ornithopter",
        ),
        (
            "volcanic_submersion",
            ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            "propaganda",
        ),
    ] {
        let mut e = engine(20_733_100 + mana.c as u64);
        let permanent = inject_permanent_on_battlefield(&mut e, 1, wrong);
        give_mana(&mut e, 0, gift(&mana));
        inject_card_into_hand(&mut e, 0, id);
        let slot = hand_index_for_card(&e, 0, id);
        let pool_before = pool(&e, 0);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(permanent)))
                .is_err(),
            "{id}: rejects {wrong}"
        );
        assert_eq!(pool(&e, 0), pool_before, "{id}: no mana spent");
        assert!(e.state.stack.is_empty(), "{id}");
    }
}

#[test]
fn q71_storm_canary_simplest_weather_the_storm_copies_once() {
    let mut e = engine(20_740_001);
    cast_prelude(&mut e);
    let case = storm_cases()
        .into_iter()
        .find(|case| case.id == "weather_the_storm")
        .unwrap();
    cast_storm(&mut e, &case, Vec::new());
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 26);
    assert_eq!(e.state.turn_history.current.spells_cast, 2);
}

#[test]
fn q71_storm_canary_hardest_tendrils_of_agony_drains_each_copy() {
    let mut e = engine(20_740_002);
    cast_prelude(&mut e);
    let case = storm_cases()
        .into_iter()
        .find(|case| case.id == "tendrils_of_agony")
        .unwrap();
    cast_storm(&mut e, &case, target_player(1));
    let opponent = u32::try_from(e.state.players[1].id).unwrap();
    choose_copy_target(&mut e, opponent, "tendrils_of_agony");
    assert_eq!(e.state.players[1].life, 16);
    assert_eq!(e.state.players[0].life, 24);

    let spell = e.state.players[0]
        .graveyard
        .iter()
        .copied()
        .find(|id| e.state.objects[id].card_id == "tendrils_of_agony")
        .expect("Tendrils in graveyard");
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
}

#[test]
fn q71_flashback_canary_simplest_scorching_missile_flashbacks() {
    let case = flash_cases()
        .into_iter()
        .find(|case| case.id == "scorching_missile")
        .unwrap();
    let mut e = engine(20_740_003);
    give_mana(&mut e, 0, gift(&case.mana));
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Scorching Missile");
    resolve_entire_stack_two_player(&mut e);
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, 1, generation))
        .expect("flashback Scorching Missile");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 12);
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}

#[test]
fn q71_flashback_canary_hardest_smiting_helix_off_color_flashback() {
    let case = flash_cases()
        .into_iter()
        .find(|case| case.id == "smiting_helix")
        .unwrap();
    let mut e = engine(20_740_004);
    give_mana(&mut e, 0, gift(&case.mana));
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Smiting Helix");
    resolve_entire_stack_two_player(&mut e);
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, 1, generation))
        .expect("white-and-black flashback cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 14);
    assert_eq!(e.state.players[0].life, 26);
    assert_eq!(e.state.objects[&spell].zone, Zone::Exile);
}

#[test]
fn q71_cycling_canary_simplest_scrap_cycles_and_destroys() {
    let mut e = engine(20_740_005);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "ornithopter");
    let cycled = inject_card_into_hand(&mut e, 0, "scrap");
    let drawn = put_on_top(&mut e, 0, "grizzly_bears");
    let opponent_hand = e.state.players[1].hand.clone();
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(&e, cycled))
        .expect("cycle Scrap");
    assert_eq!(e.state.objects[&cycled].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
    assert_eq!(e.state.players[1].hand, opponent_hand);

    let cast = inject_card_into_hand(&mut e, 0, "scrap");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(
        0,
        &cast_spell(hand_index_for_card(&e, 0, "scrap"), target_object(artifact)),
    )
    .expect("cast Scrap");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&cast].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].hand, opponent_hand);
}

#[test]
fn q71_cycling_canary_hardest_hush_cycles_and_destroys_all_enchantments() {
    let mut e = engine(20_740_006);
    let own = inject_permanent_on_battlefield(&mut e, 0, "propaganda");
    let opposing = inject_permanent_on_battlefield(&mut e, 1, "propaganda");
    let cycled = inject_card_into_hand(&mut e, 0, "hush");
    let drawn = put_on_top(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(&e, cycled))
        .expect("cycle Hush");
    assert_eq!(e.state.objects[&cycled].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);

    let cast = inject_card_into_hand(&mut e, 0, "hush");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 3,
            ..Default::default()
        },
    );
    e.apply_scenario_command(
        0,
        &cast_spell(hand_index_for_card(&e, 0, "hush"), Vec::new()),
    )
    .expect("cast Hush");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&cast].zone, Zone::Graveyard);
}

#[test]
fn q71_storm_with_no_earlier_spells_creates_no_copy() {
    let mut e = engine(20_740_007);
    let weather = storm_cases()
        .into_iter()
        .find(|case| case.id == "weather_the_storm")
        .unwrap();
    cast_storm(&mut e, &weather, Vec::new());
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 23, "one printed Weather effect");
    assert_eq!(
        e.state.turn_history.current.spells_cast, 1,
        "no copy is cast"
    );
    assert!(e.state.stack.is_empty(), "no copy remains on the stack");
}
