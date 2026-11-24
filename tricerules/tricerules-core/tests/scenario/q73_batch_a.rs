//! Actual-card scenarios for the queue-v73 batch A cohorts.
//!
//! Expectations are printed Oracle values. Cycling is a hand-zone activated ability (CR 702.29a)
//! with its printed cost (CR 118.3). Flashback is an alternative graveyard cost that always exiles
//! (CR 702.34a-d). Madness is an alternative exile cost offered when the card is discarded
//! (CR 702.35a-d); a spell cast for madness resolves normally. Target legality follows
//! CR 115.1/601.2c, and a spell whose only target is illegal does not resolve (CR 608.2b).
use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    AbilitySourceZone, ActivateAbility, CastMethod, CastSource, CastSpell,
    ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice, TargetRef,
};

struct CycleCase {
    id: &'static str,
    mana: ManaGift,
    cycle: ManaGift,
}

fn cycle_cases() -> [CycleCase; 4] {
    [
        CycleCase {
            id: "hieroglyphic_illumination",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            cycle: ManaGift {
                u: 1,
                ..Default::default()
            },
        },
        CycleCase {
            id: "boon_of_the_wish-giver",
            mana: ManaGift {
                u: 2,
                c: 4,
                ..Default::default()
            },
            cycle: ManaGift {
                c: 1,
                ..Default::default()
            },
        },
        CycleCase {
            id: "compelling_argument",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            cycle: ManaGift {
                u: 1,
                ..Default::default()
            },
        },
        CycleCase {
            id: "rejuvenate",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            cycle: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
    ]
}

struct FlashCase {
    id: &'static str,
    mana: ManaGift,
    flashback: ManaGift,
}

fn flash_cases() -> [FlashCase; 4] {
    [
        FlashCase {
            id: "chainers_edict",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                b: 2,
                c: 5,
                ..Default::default()
            },
        },
        FlashCase {
            id: "dematerialize",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            flashback: ManaGift {
                u: 2,
                c: 5,
                ..Default::default()
            },
        },
        FlashCase {
            id: "silent_departure",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            flashback: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
        },
        FlashCase {
            id: "rolling_temblor",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            flashback: ManaGift {
                r: 2,
                c: 4,
                ..Default::default()
            },
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

fn cycle_from_hand(e: &mut GameEngine, id: &str, cost: ManaGift) {
    let card = inject_card_into_hand(e, 0, id);
    let drawn = put_on_top(e, 0, "grizzly_bears");
    let hand_before = e.state.players[0].hand.len();
    let opponent_hand = e.state.players[1].hand.clone();
    give_mana(e, 0, gift(&cost));
    e.apply_scenario_command(0, &hand_ability(e, card))
        .unwrap_or_else(|error| panic!("{id}: cycle: {error:?}"));
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Graveyard,
        "{id}: cycling discards itself"
    );
    assert_eq!(pool(e, 0), (0, 0, 0, 0, 0, 0), "{id}: exact cycling cost");
    resolve_entire_stack_two_player(e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{id}: draws one");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "{id}: one-for-one"
    );
    assert_eq!(
        e.state.players[1].hand, opponent_hand,
        "{id}: opponent hand unchanged"
    );
}

fn flashback_cast(spell: u32, targets: Vec<TargetRef>, generation: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets,
            ..Default::default()
        })),
    }
}

fn madness_code(e: &GameEngine, oid: u32, targets: Vec<TargetRef>) -> RuledCommand {
    let offer_id = e
        .state
        .pending_resolution
        .as_ref()
        .expect("madness offer")
        .continuation
        .stack()
        .unwrap()
        .item
        .id as u64;
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            decision: ResolutionChoiceDecision::CastSpell as i32,
            cast_spell: Some(CastSpell {
                source: Some(CastSource {
                    location: Some(
                        tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(oid),
                    ),
                    expected_zone_change_generation: Some(e.state.zone_change_generation[&oid]),
                }),
                cast_method: CastMethod::Madness as i32,
                casting_permission_id: Some(offer_id),
                targets,
                ..Default::default()
            }),
            ..Default::default()
        })),
    }
}

/// Puts `card` and a spare land in player 0's hand, then makes player 0 discard both to Mind Rot so
/// the Madness offer is waiting on player 0. Returns the engine and the exiled madness object.
fn madness_offer(card: &str, seed: u64) -> (GameEngine, u32) {
    let mut e = engine(seed);
    let oid = inject_card_into_hand(&mut e, 0, card);
    inject_card_into_hand(&mut e, 0, "forest");
    inject_card_into_hand(&mut e, 0, "mind_rot");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "mind_rot");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(0)))
        .expect("cast Mind Rot at self");
    e.apply_scenario_command(0, &pass()).expect("pass");
    let batch = e
        .apply_scenario_command(1, &pass())
        .expect("resolve Mind Rot");
    let req = find_resolution_choice(&batch).expect("discard choice");
    assert_eq!(req.deciding_player_id, 0, "discarding player chooses");
    let fuel = *e.state.players[0]
        .hand
        .iter()
        .find(|candidate| e.state.objects[*candidate].card_id == "forest")
        .expect("spare land in hand");
    e.apply_scenario_command(0, &submit_resolution_choice(vec![oid, fuel]))
        .expect("discard madness card and land");
    if e.state.pending_resolution.is_none() {
        e.apply_scenario_command(0, &pass()).expect("pass");
    }
    if e.state.pending_resolution.is_none() {
        e.apply_scenario_command(1, &pass())
            .expect("resolve madness trigger");
    }
    assert_eq!(
        e.state.objects[&oid].zone,
        Zone::Exile,
        "{card}: madness exiles the discarded card first"
    );
    assert!(
        e.state.pending_resolution.is_some(),
        "{card}: madness offer pending"
    );
    (e, oid)
}

// ---------------------------------------------------------------------------
// 01-cycling-card-advantage

#[test]
fn q73_cycling_canary_simplest_hieroglyphic_illumination_draws_two_and_cycles() {
    let case = cycle_cases()
        .into_iter()
        .find(|case| case.id == "hieroglyphic_illumination")
        .expect("Hieroglyphic Illumination");
    let mut e = engine(73_100_001);
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let first = put_on_top(&mut e, 0, "grizzly_bears");
    let second = put_on_top(&mut e, 0, "savannah_lions");
    let hand_before = e.state.players[0].hand.len();
    let opponent_hand = e.state.players[1].hand.clone();
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Hieroglyphic Illumination");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed cost");
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Graveyard,
        "spell resolves"
    );
    assert_eq!(e.state.objects[&first].zone, Zone::Hand, "first draw");
    assert_eq!(e.state.objects[&second].zone, Zone::Hand, "second draw");
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1 + 2,
        "draw exactly two"
    );
    assert_eq!(
        e.state.players[1].hand, opponent_hand,
        "opponent does not draw"
    );
    cycle_from_hand(&mut e, case.id, case.cycle);
}

#[test]
fn q73_cycling_canary_hardest_compelling_argument_mills_the_chosen_player_and_cycles() {
    let case = cycle_cases()
        .into_iter()
        .find(|case| case.id == "compelling_argument")
        .expect("Compelling Argument");
    let mut e = engine(73_100_002);
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    let mut topped = Vec::new();
    for card in [
        "grizzly_bears",
        "savannah_lions",
        "coral_merfolk",
        "walking_corpse",
        "balduvian_barbarians",
    ] {
        topped.push(put_on_top(&mut e, 1, card));
    }
    let controller_graveyard = e.state.players[0].graveyard.clone();
    let opponent_hand = e.state.players[1].hand.clone();
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Compelling Argument at the opponent");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact printed cost");
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Graveyard,
        "spell resolves"
    );
    for object in topped {
        assert_eq!(
            e.state.objects[&object].zone,
            Zone::Graveyard,
            "the chosen player's top five are milled"
        );
    }
    assert_eq!(
        e.state.players[1].hand, opponent_hand,
        "milling is not discarding or drawing"
    );
    assert_eq!(
        e.state.players[0].graveyard.len(),
        controller_graveyard.len() + 1,
        "only the resolving spell is in the caster's graveyard"
    );
    cycle_from_hand(&mut e, case.id, case.cycle);
}

#[test]
fn q73_cycling_card_advantage_cards_apply_their_printed_values_and_cycle() {
    for (index, case) in cycle_cases().into_iter().enumerate() {
        let mut e = engine(73_101_000 + index as u64);
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let opponent_life = e.state.players[1].life;
        let opponent_graveyard = e.state.players[1].graveyard.len();
        let mut topped = Vec::new();
        for card in ["grizzly_bears", "savannah_lions", "coral_merfolk"] {
            topped.push(put_on_top(&mut e, 0, card));
        }
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let targets = if case.id == "compelling_argument" {
            target_player(1)
        } else {
            vec![]
        };
        e.apply_scenario_command(0, &cast_spell(slot, targets))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed cost",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: spell resolves",
            case.id
        );
        match case.id {
            "hieroglyphic_illumination" => {
                assert_eq!(
                    e.state.players[0].hand.len(),
                    hand_before - 1 + 2,
                    "Hieroglyphic Illumination draws two"
                );
                assert_eq!(
                    e.state.objects[&topped[2]].zone,
                    Zone::Hand,
                    "the top card is drawn"
                );
                assert_eq!(
                    e.state.objects[&topped[1]].zone,
                    Zone::Hand,
                    "the second card is drawn"
                );
                assert_eq!(
                    e.state.objects[&topped[0]].zone,
                    Zone::Library,
                    "only two cards are drawn"
                );
            }
            "boon_of_the_wish-giver" => {
                assert_eq!(
                    e.state.players[0].hand.len(),
                    hand_before - 1 + 4,
                    "Boon of the Wish-Giver draws four"
                );
            }
            "compelling_argument" => {
                assert_eq!(
                    e.state.players[1].graveyard.len(),
                    opponent_graveyard + 5,
                    "Compelling Argument mills the chosen player five"
                );
                assert_eq!(
                    e.state.players[0].hand.len(),
                    hand_before - 1,
                    "Compelling Argument does not draw"
                );
            }
            "rejuvenate" => {
                assert_eq!(
                    e.state.players[0].life, 26,
                    "Rejuvenate gains exactly six life"
                );
                assert_eq!(
                    e.state.players[1].life, opponent_life,
                    "Rejuvenate does not gain the opponent life"
                );
                assert_eq!(
                    e.state.players[0].hand.len(),
                    hand_before - 1,
                    "Rejuvenate does not draw"
                );
            }
            other => panic!("unhandled {other}"),
        }
        cycle_from_hand(&mut e, case.id, case.cycle);
    }
}

#[test]
fn q73_cycling_card_advantage_rejects_missing_targets_and_unfunded_cycling() {
    let case = cycle_cases()
        .into_iter()
        .find(|case| case.id == "compelling_argument")
        .expect("Compelling Argument");
    let mut e = engine(73_102_001);
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    let before = pool(&e, 0);
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .is_err(),
        "Compelling Argument needs a player target"
    );
    assert_eq!(pool(&e, 0), before, "rejected cast spends nothing");
    assert_eq!(e.state.objects[&spell].zone, Zone::Hand);

    for case in cycle_cases() {
        let mut e = engine(73_102_100 + case.mana.c as u64);
        let card = inject_card_into_hand(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: case.cycle.c.saturating_sub(1),
                w: case.cycle.w.saturating_sub(1),
                u: case.cycle.u.saturating_sub(1),
                ..Default::default()
            },
        );
        assert!(
            e.apply_scenario_command(0, &hand_ability(&e, card))
                .is_err(),
            "{}: cycling needs its full printed mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&card].zone,
            Zone::Hand,
            "{}: rejected cycling keeps the card in hand",
            case.id
        );
    }
}

// ---------------------------------------------------------------------------
// 02-flashback-control

#[test]
fn q73_flashback_canary_simplest_silent_departure_bounces_twice_and_exiles() {
    let case = flash_cases()
        .into_iter()
        .find(|case| case.id == "silent_departure")
        .expect("Silent Departure");
    let mut e = engine(73_200_001);
    let first = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(first)))
        .expect("cast Silent Departure");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact mana cost");
    assert_eq!(
        e.state.objects[&first].zone,
        Zone::Hand,
        "target creature returns to its owner's hand"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    let second = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, target_object(second), generation))
        .expect("flashback Silent Departure");
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact flashback cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&second].zone,
        Zone::Hand,
        "flashback repeats the bounce"
    );
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "flashback exiles the spell"
    );
    let exiled_generation = e.state.zone_change_generation[&spell];
    let fresh = inject_creature_with_stats(&mut e, 1, "balduvian_barbarians", 2, 2);
    give_mana(&mut e, 0, gift(&case.flashback));
    let exiled_flashback = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(CastSource {
                location: Some(
                    tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(spell),
                ),
                expected_zone_change_generation: Some(exiled_generation),
            }),
            targets: target_object(fresh),
            ..Default::default()
        })),
    };
    assert!(
        e.apply_scenario_command(0, &exiled_flashback).is_err(),
        "flashback cannot be cast from exile"
    );
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "the exiled card stays exiled"
    );
}

#[test]
fn q73_flashback_canary_hardest_chainers_edict_edicts_twice_and_exiles() {
    let case = flash_cases()
        .into_iter()
        .find(|case| case.id == "chainers_edict")
        .expect("Chainer's Edict");
    let mut e = engine(73_200_002);
    let first = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
    let own = inject_creature_with_stats(&mut e, 0, "coral_merfolk", 2, 2);
    let spell = inject_card_into_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(&case.mana));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Chainer's Edict");
    e.apply_scenario_command(0, &pass()).expect("pass");
    let batch = e
        .apply_scenario_command(1, &pass())
        .expect("edict resolves");
    let req = find_resolution_choice(&batch).expect("edict sacrifice choice");
    assert_eq!(req.deciding_player_id, 1, "the targeted player chooses");
    e.apply_scenario_command(1, &submit_resolution_choice(vec![first]))
        .expect("opponent sacrifices");
    assert_eq!(e.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&second].zone, Zone::Battlefield);
    assert_eq!(
        e.state.objects[&own].zone,
        Zone::Battlefield,
        "the caster's creature is untouched"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);

    let replacement = inject_creature_with_stats(&mut e, 1, "balduvian_barbarians", 2, 2);
    let generation = e.state.zone_change_generation[&spell];
    give_mana(&mut e, 0, gift(&case.flashback));
    e.apply_scenario_command(0, &flashback_cast(spell, target_player(1), generation))
        .expect("flashback Chainer's Edict");
    e.apply_scenario_command(0, &pass()).expect("pass");
    let batch = e
        .apply_scenario_command(1, &pass())
        .expect("flashback edict resolves");
    let req = find_resolution_choice(&batch).expect("second sacrifice choice");
    assert_eq!(req.deciding_player_id, 1);
    e.apply_scenario_command(1, &submit_resolution_choice(vec![replacement]))
        .expect("opponent sacrifices again");
    assert_eq!(e.state.objects[&replacement].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Exile,
        "flashback exiles the spell"
    );
}

#[test]
fn q73_flashback_control_cards_apply_their_printed_bodies_from_hand_and_graveyard() {
    for (index, case) in flash_cases().into_iter().enumerate() {
        let mut e = engine(73_201_000 + index as u64);
        let mut temblor_flier = None;
        match case.id {
            "chainers_edict" => {
                let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
                inject_card_into_hand(&mut e, 0, case.id);
                give_mana(&mut e, 0, gift(&case.mana));
                let slot = hand_index_for_card(&e, 0, case.id);
                e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                    .expect("cast edict");
                e.apply_scenario_command(0, &pass()).expect("pass");
                let batch = e.apply_scenario_command(1, &pass()).expect("resolve");
                find_resolution_choice(&batch).expect("sacrifice choice");
                e.apply_scenario_command(1, &submit_resolution_choice(vec![creature]))
                    .expect("sacrifice");
                assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);
            }
            "dematerialize" => {
                let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
                inject_card_into_hand(&mut e, 0, case.id);
                give_mana(&mut e, 0, gift(&case.mana));
                let slot = hand_index_for_card(&e, 0, case.id);
                e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                    .expect("cast Dematerialize");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(
                    e.state.objects[&land].zone,
                    Zone::Hand,
                    "Dematerialize returns any permanent"
                );
            }
            "silent_departure" => {
                let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
                inject_card_into_hand(&mut e, 0, case.id);
                give_mana(&mut e, 0, gift(&case.mana));
                let slot = hand_index_for_card(&e, 0, case.id);
                e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                    .expect("cast Silent Departure");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(e.state.objects[&creature].zone, Zone::Hand);
            }
            "rolling_temblor" => {
                let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
                let opposing = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
                let flier = inject_creature_with_stats(&mut e, 1, "ravenous_skirge", 2, 2);
                assert!(
                    e.effective_has_keyword(flier, Keyword::Flying),
                    "the fixture flier is a printed flying creature"
                );
                inject_card_into_hand(&mut e, 0, case.id);
                give_mana(&mut e, 0, gift(&case.mana));
                let slot = hand_index_for_card(&e, 0, case.id);
                e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                    .expect("cast Rolling Temblor");
                resolve_entire_stack_two_player(&mut e);
                assert_eq!(e.state.objects[&own].zone, Zone::Graveyard);
                assert_eq!(e.state.objects[&opposing].zone, Zone::Graveyard);
                assert_eq!(
                    e.state.objects[&flier].zone,
                    Zone::Battlefield,
                    "flying creatures are excluded"
                );
                assert_eq!(e.state.players[0].life, 20);
                assert_eq!(e.state.players[1].life, 20);
                temblor_flier = Some(flier);
            }
            other => panic!("unhandled {other}"),
        }
        let spell_oid = e.state.players[0]
            .graveyard
            .iter()
            .copied()
            .find(|oid| e.state.objects[oid].card_id == case.id)
            .expect("first cast is in the graveyard");
        let generation = e.state.zone_change_generation[&spell_oid];
        give_mana(&mut e, 0, gift(&case.flashback));
        let (targets, expected) = match case.id {
            "chainers_edict" => {
                let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
                (target_player(1), Some(creature))
            }
            "dematerialize" => {
                let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
                (target_object(land), Some(land))
            }
            "silent_departure" => {
                let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
                (target_object(creature), Some(creature))
            }
            "rolling_temblor" => {
                inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
                inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
                (vec![], None)
            }
            other => panic!("unhandled {other}"),
        };
        e.apply_scenario_command(0, &flashback_cast(spell_oid, targets, generation))
            .unwrap_or_else(|error| panic!("{}: flashback: {error:?}", case.id));
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: exact flashback cost",
            case.id
        );
        if case.id == "chainers_edict" {
            e.apply_scenario_command(0, &pass()).expect("pass");
            let batch = e.apply_scenario_command(1, &pass()).expect("resolve");
            let req = find_resolution_choice(&batch).expect("second sacrifice choice");
            e.apply_scenario_command(
                1,
                &submit_resolution_choice(vec![req.candidate_object_ids[0]]),
            )
            .expect("sacrifice");
        } else {
            resolve_entire_stack_two_player(&mut e);
        }
        match case.id {
            "dematerialize" | "silent_departure" => assert_eq!(
                e.state.objects[&expected.expect("flashback target")].zone,
                Zone::Hand,
                "{}: flashback repeats the bounce",
                case.id
            ),
            "rolling_temblor" => {
                assert_eq!(
                    e.state
                        .players
                        .iter()
                        .flat_map(|player| player.graveyard.iter())
                        .filter(|oid| {
                            let card = &e.state.objects[oid].card_id;
                            card == "grizzly_bears" || card == "savannah_lions"
                        })
                        .count(),
                    4,
                    "the first two nonfliers and the flashback pair die"
                );
                let flier = temblor_flier.expect("flying fixture");
                assert_eq!(
                    e.state.objects[&flier].zone,
                    Zone::Battlefield,
                    "the flier survives the flashback sweep too"
                );
                assert_eq!(
                    e.state.objects[&flier].damage, 0,
                    "the flier takes no damage from either sweep"
                );
            }
            _ => {}
        }
        assert_eq!(
            e.state.objects[&spell_oid].zone,
            Zone::Exile,
            "{}: flashback exiles",
            case.id
        );
    }
}

#[test]
fn q73_flashback_control_rejects_wrong_targets_and_illegal_flashback() {
    // Silent Departure cannot target a land, and a stale creature target fizzles without a return.
    let mut e = engine(73_202_001);
    let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let spell = inject_card_into_hand(&mut e, 0, "silent_departure");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            u: 1,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "silent_departure");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
            .is_err(),
        "Silent Departure requires a creature"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Hand);

    let mut e = engine(73_202_002);
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let opponent_hand = e.state.players[1].hand.len();
    inject_card_into_hand(&mut e, 0, "silent_departure");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            u: 1,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "silent_departure");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast at creature");
    move_to_graveyard(&mut e, 1, creature);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].graveyard.len(), 1);
    assert_eq!(
        e.state.players[1].hand.len(),
        opponent_hand,
        "a fizzled bounce does not return the departed creature"
    );

    // A sorcery cannot be flashback-cast during the opponent's turn.
    let mut e = engine(73_202_003);
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let spell = inject_card_into_hand(&mut e, 0, "silent_departure");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            u: 1,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "silent_departure");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("first cast");
    resolve_entire_stack_two_player(&mut e);
    let generation = e.state.zone_change_generation[&spell];
    end_active_turn(&mut e, 0);
    pass_both_players(&mut e);
    pass_both_players(&mut e);
    assert_eq!(
        e.state.priority_player_id(),
        e.state.players[1].id,
        "the opponent's turn"
    );
    e.apply_scenario_command(e.state.players[1].id, &pass())
        .expect("opponent passes priority");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            u: 1,
            c: 4,
            ..Default::default()
        }),
    );
    let other = inject_creature_with_stats(&mut e, 1, "savannah_lions", 2, 2);
    assert!(
        e.apply_scenario_command(0, &flashback_cast(spell, target_object(other), generation))
            .is_err(),
        "a sorcery must obey sorcery timing even with flashback"
    );
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
}

// ---------------------------------------------------------------------------
// 03-madness-spells

#[test]
fn q73_madness_spells_apply_their_printed_bodies_from_hand() {
    // Dark Withering: destroy only a nonblack creature, for its printed cost.
    let mut e = engine(73_300_001);
    let black = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            b: 2,
            c: 4,
            ..Default::default()
        }),
    );
    let spell = inject_card_into_hand(&mut e, 0, "dark_withering");
    let slot = hand_index_for_card(&e, 0, "dark_withering");
    let before = pool(&e, 0);
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(black)))
            .is_err(),
        "Dark Withering cannot target a black creature"
    );
    assert_eq!(pool(&e, 0), before, "rejected cast spends nothing");
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Hand,
        "rejected cast keeps the card in hand"
    );

    let mut e = engine(73_300_011);
    let victim = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let black = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);
    let spell = inject_card_into_hand(&mut e, 0, "dark_withering");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            b: 2,
            c: 4,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "dark_withering");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(victim)))
        .expect("cast Dark Withering");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&black].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));

    // Distemper of the Blood: +2/+2 and trample until end of turn on exactly one target.
    let mut e = engine(73_300_002);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
    inject_card_into_hand(&mut e, 0, "distemper_of_the_blood");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "distemper_of_the_blood");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Distemper of the Blood");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(4), Some(4))
    );
    assert!(e.effective_has_keyword(target, Keyword::Trample));
    assert_eq!(
        (
            e.effective_power(bystander),
            e.effective_toughness(bystander)
        ),
        (Some(2), Some(2))
    );
    assert!(!e.effective_has_keyword(bystander, Keyword::Trample));
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(2)),
        "the pump expires at cleanup"
    );
    assert!(!e.effective_has_keyword(target, Keyword::Trample));

    // Frantic Purification: destroy an enchantment only.
    let mut e = engine(73_300_003);
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "pacifism");
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    inject_card_into_hand(&mut e, 0, "frantic_purification");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "frantic_purification");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .is_err(),
        "Frantic Purification cannot target a creature"
    );
    let slot = hand_index_for_card(&e, 0, "frantic_purification");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(enchantment)))
        .expect("cast Frantic Purification");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);

    // Just the Wind: return a creature, not a land.
    let mut e = engine(73_300_004);
    let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    inject_card_into_hand(&mut e, 0, "just_the_wind");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        }),
    );
    let slot = hand_index_for_card(&e, 0, "just_the_wind");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
            .is_err(),
        "Just the Wind cannot target a land"
    );
    let slot = hand_index_for_card(&e, 0, "just_the_wind");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Just the Wind");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));
    assert_eq!(e.state.objects[&creature].zone, Zone::Hand);

    // Obsessive Search: draw one, no scry.
    let mut e = engine(73_300_005);
    let spell = inject_card_into_hand(&mut e, 0, "obsessive_search");
    let drawn = put_on_top(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            u: 1,
            ..Default::default()
        }),
    );
    let hand_before = e.state.players[0].hand.len();
    let slot = hand_index_for_card(&e, 0, "obsessive_search");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Obsessive Search");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
    assert_eq!(e.state.players[0].hand.len(), hand_before);
    assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q73_madness_canary_obsessive_search_casts_from_exile_for_its_madness_cost() {
    let (mut e, oid) = madness_offer("obsessive_search", 73_301_001);
    let drawn = put_on_top(&mut e, 0, "grizzly_bears");
    let hand_before = e.state.players[0].hand.len();
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            u: 1,
            ..Default::default()
        }),
    );
    e.apply_scenario_command(0, &madness_code(&e, oid, vec![]))
        .expect("cast Obsessive Search for madness");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0), "exact madness cost");
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "the madness spell is cast from exile, so only the draw changes the hand"
    );
    assert_eq!(
        e.state.objects[&oid].zone,
        Zone::Graveyard,
        "an instant cast for madness resolves to the graveyard"
    );
}

#[test]
fn q73_madness_canary_dark_withering_casts_from_exile_and_binds_its_target() {
    let (mut e, oid) = madness_offer("dark_withering", 73_301_002);
    let victim = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    give_mana(
        &mut e,
        0,
        gift(&ManaGift {
            b: 1,
            ..Default::default()
        }),
    );
    e.apply_scenario_command(0, &madness_code(&e, oid, target_object(victim)))
        .expect("cast Dark Withering for madness");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&oid].zone, Zone::Graveyard);
    assert_eq!(pool(&e, 0), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q73_madness_decline_is_final_and_puts_the_card_into_the_graveyard() {
    let (mut e, oid) = madness_offer("obsessive_search", 73_301_003);
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline madness");
    assert_eq!(
        e.state.objects[&oid].zone,
        Zone::Graveyard,
        "declining madness puts the card into its owner's graveyard"
    );
    assert!(
        e.state.pending_resolution.is_none(),
        "the declined madness offer is gone"
    );
    assert!(
        e.state.active_exile_play_permissions.is_empty(),
        "no lingering exile cast permission remains"
    );
}
