//! Ashiok's actual -1: mill the targeted player, then exile opponents' graveyards.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::{ContinuousEffectKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{self as rv1, dev_command, DevCommand, DevMoveCard, DevZone};

const ASHIOK: &str = "ashiok,_dream_render";

fn game(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get(ASHIOK).is_some(),
        "Ashiok, Dream Render must be registered"
    );
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1, 2],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("create a three-player game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn fixture_ashiok(engine: &mut GameEngine) -> u32 {
    // Synthetic battlefield placement and starting counters; the tested -1 is a real command.
    let object = inject_permanent_on_battlefield(engine, 0, ASHIOK);
    engine
        .state
        .objects
        .get_mut(&object)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    object
}

fn cast_ashiok(engine: &mut GameEngine, blue: u32, black: u32) -> u32 {
    let object = inject_card_into_hand(engine, 0, ASHIOK);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 1,
            u: blue,
            b: black,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, ASHIOK);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Ashiok with the actual hybrid cost");
    assert_eq!(
        (
            engine.state.players[0].mana_pool.colorless,
            engine.state.players[0].mana_pool.blue,
            engine.state.players[0].mana_pool.black
        ),
        (0, 0, 0)
    );
    pass_priority_round(engine);
    assert_eq!(engine.state.objects[&object].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&object].counter_count(CounterKind::Loyalty),
        5
    );
    object
}

fn place_top(engine: &mut GameEngine, owner: usize, ids: &[&str]) -> Vec<u32> {
    // Synthetic library placement makes the physical mill order deterministic.
    let objects: Vec<_> = ids
        .iter()
        .map(|id| inject_library_card(engine, owner, id))
        .collect();
    engine.state.players[owner]
        .library
        .retain(|id| !objects.contains(id));
    for &object in objects.iter().rev() {
        engine.state.players[owner].library.push_front(object);
    }
    objects
}

fn activate_minus(engine: &mut GameEngine, ashiok: u32, target: i32) {
    apply_ability(engine, 0, ashiok, 0, target_player(target)).expect("activate Ashiok's -1");
    assert_eq!(
        engine.state.objects[&ashiok].counter_count(CounterKind::Loyalty),
        4
    );
    assert_eq!(engine.state.stack.len(), 1);
}

fn resolve_one(engine: &mut GameEngine) -> RuledEventBatch {
    let remaining = engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
        - engine.state.passes_since_stack_change as usize;
    let mut batch = RuledEventBatch::default();
    for _ in 0..remaining {
        let actor = engine.state.priority_player_id();
        batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("pass priority to resolve");
    }
    batch
}

fn pass_to(engine: &mut GameEngine, player: i32) {
    while engine.state.priority_player_id() != player {
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("pass to the requested player");
    }
}

fn shuffled(batch: &RuledEventBatch, player: i32) -> bool {
    batch.events.iter().any(|event| {
        matches!(&event.ev,
        Some(Ev::Log(log)) if log.text == format!("P{player} shuffles their library."))
    })
}

#[test]
fn ashiok_hybrid_cast_mills_target_then_exiles_all_opponents_graveyards() {
    for (run, blue, black) in [(0, 2, 0), (1, 1, 1), (2, 0, 2)] {
        let mut engine = game(261_027_700 + run);
        let ashiok = cast_ashiok(&mut engine, blue, black);
        let milled = place_top(
            &mut engine,
            1,
            &["grizzly_bears", "island", "sol_ring", "mountain"],
        );
        let own = inject_graveyard_card(&mut engine, 0, "forest");
        let first = inject_graveyard_card(&mut engine, 1, "storm_crow");
        let second = inject_graveyard_card(&mut engine, 2, "mountain");
        activate_minus(&mut engine, ashiok, 1);
        let batch = resolve_one(&mut engine);
        assert!(find_resolution_choice(&batch).is_none());
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.objects[&own].zone, Zone::Graveyard);
        for object in milled.iter().copied().chain([first, second]) {
            assert_eq!(engine.state.objects[&object].zone, Zone::Exile);
        }
        for object in milled {
            let destinations: Vec<_> = batch
                .events
                .iter()
                .filter_map(|event| match &event.ev {
                    Some(Ev::PermanentMoved(moved)) if moved.object_id == object => {
                        Some(moved.destination)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(
                destinations,
                [
                    rv1::permanent_moved::Destination::Graveyard as i32,
                    rv1::permanent_moved::Destination::Exile as i32,
                ],
                "the mill occurs before the graveyard exile in one resolution"
            );
        }
    }
}

#[test]
fn ashiok_can_mill_its_controller_without_exiling_that_graveyard() {
    let mut engine = game(261_027_703);
    let ashiok = fixture_ashiok(&mut engine);
    let own = place_top(
        &mut engine,
        0,
        &["grizzly_bears", "island", "sol_ring", "mountain"],
    );
    let first = inject_graveyard_card(&mut engine, 1, "storm_crow");
    let second = inject_graveyard_card(&mut engine, 2, "forest");
    activate_minus(&mut engine, ashiok, 0);
    resolve_one(&mut engine);
    for object in own {
        assert_eq!(engine.state.objects[&object].zone, Zone::Graveyard);
    }
    for object in [first, second] {
        assert_eq!(engine.state.objects[&object].zone, Zone::Exile);
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn ashiok_mills_available_cards_from_a_short_library_then_exiles() {
    for remaining in 0..=2 {
        let mut engine = game(261_027_704 + remaining as u64);
        let ashiok = fixture_ashiok(&mut engine);
        // Synthetic fixture move leaves exactly the requested number of cards in the library.
        while engine.state.players[1].library.len() > remaining {
            let object = engine.state.players[1].library.pop_back().unwrap();
            engine.state.objects.get_mut(&object).unwrap().zone = Zone::Hand;
            engine.state.players[1].hand.push(object);
        }
        let top: Vec<_> = engine.state.players[1].library.iter().copied().collect();
        let existing = inject_graveyard_card(&mut engine, 1, "storm_crow");
        let other = inject_graveyard_card(&mut engine, 2, "mountain");
        activate_minus(&mut engine, ashiok, 1);
        resolve_one(&mut engine);
        assert!(engine.state.players[1].library.is_empty());
        assert!(
            !engine.state.players[1].has_lost,
            "milling an empty library is not a failed draw"
        );
        for object in top.into_iter().chain([existing, other]) {
            assert_eq!(engine.state.objects[&object].zone, Zone::Exile);
        }
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn ashiok_minus_one_resolves_after_its_source_leaves() {
    let mut engine = game(261_027_707);
    let ashiok = fixture_ashiok(&mut engine);
    let top = place_top(
        &mut engine,
        1,
        &["grizzly_bears", "island", "sol_ring", "mountain"],
    );
    let other = inject_graveyard_card(&mut engine, 2, "forest");
    activate_minus(&mut engine, ashiok, 1);
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_scenario_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 0,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Ashiok, Dream Render".into(),
                        zone: DevZone::Graveyard as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move Ashiok through the real zone-change path");
    assert_eq!(engine.state.objects[&ashiok].zone, Zone::Graveyard);
    resolve_one(&mut engine);
    assert_eq!(engine.state.objects[&ashiok].zone, Zone::Graveyard);
    for object in top.into_iter().chain([other]) {
        assert_eq!(engine.state.objects[&object].zone, Zone::Exile);
    }
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ashiok_all_illegal_target_stops_the_whole_ability() {
    let mut engine = game(261_027_708);
    let ashiok = fixture_ashiok(&mut engine);
    let surviving_opponent_card = inject_graveyard_card(&mut engine, 2, "storm_crow");
    let own = inject_graveyard_card(&mut engine, 0, "forest");
    activate_minus(&mut engine, ashiok, 1);
    engine
        .apply_scenario_command(1, &concede())
        .expect("the only target leaves the game");
    assert!(engine.state.players[1].has_lost);
    resolve_one(&mut engine);
    assert_eq!(
        engine.state.objects[&surviving_opponent_card].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&ashiok].counter_count(CounterKind::Loyalty),
        4
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn ashiok_blocks_opponents_evolving_wilds_but_preserves_its_shuffle() {
    let mut engine = game(261_027_710);
    cast_ashiok(&mut engine, 1, 1);
    // Synthetic placement supplies actual search observer and source; Wilds pays real costs.
    let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
    let wilds = inject_permanent_on_battlefield(&mut engine, 1, "evolving_wilds");
    let forest = inject_library_card(&mut engine, 1, "forest");
    let library_count = engine.state.players[1].library.len();
    pass_to(&mut engine, 1);
    apply_ability(&mut engine, 1, wilds, 0, vec![]).expect("tap and sacrifice Evolving Wilds");
    assert_eq!(engine.state.objects[&wilds].zone, Zone::Graveyard);
    let batch = resolve_one(&mut engine);
    assert!(
        find_resolution_choice(&batch).is_none(),
        "a prohibited search offers no private candidates"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(
        shuffled(&batch, 1),
        "the mandatory shuffle remains even when the search is prohibited"
    );
    assert_eq!(engine.state.players[1].library.len(), library_count);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Library);
    assert!(
        !engine
            .state
            .stack
            .iter()
            .any(|item| item.source_permanent_id == Some(wan)),
        "Wan Shi Tong does not observe a search that did not happen"
    );
    assert_eq!(
        engine.state.objects[&wan].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}

#[test]
fn ashiok_removes_solemn_simulacrums_optional_search_without_shuffling() {
    let mut engine = game(261_027_711);
    cast_ashiok(&mut engine, 1, 1);
    let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
    inject_card_into_hand(&mut engine, 1, "solemn_simulacrum");
    let solemn = move_ready_to_battlefield(&mut engine, 1, "solemn_simulacrum");
    assert_eq!(engine.state.objects[&solemn].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "actual Solemn entry trigger is on the stack"
    );
    let library_before = engine.state.players[1].library.clone();
    let batch = resolve_one(&mut engine);
    assert!(
        find_resolution_choice(&batch).is_none(),
        "a prohibited optional search is not offered"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(
        !shuffled(&batch, 1),
        "the declined-impossible optional search has no contingent shuffle"
    );
    assert_eq!(engine.state.players[1].library, library_before);
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.source_permanent_id == Some(wan)));
}

#[test]
fn ashiok_allows_boseiju_when_controller_and_searcher_differ() {
    for (seed, actor, owner) in [(261_027_712, 1, 2), (261_027_713, 0, 1)] {
        let mut engine = game(seed);
        cast_ashiok(&mut engine, 1, 1);
        let target = inject_permanent_on_battlefield(&mut engine, owner, "sol_ring");
        let land = inject_library_card(&mut engine, owner, "forest");
        let source = inject_card_into_hand(&mut engine, actor, "boseiju,_who_endures");
        let actor_id = engine.state.players[actor].id;
        let owner_id = engine.state.players[owner].id;
        give_mana(
            &mut engine,
            actor_id,
            ManaGift {
                c: 1,
                g: 1,
                ..Default::default()
            },
        );
        pass_to(&mut engine, actor_id);
        let mut command = activate_ability_for(&engine, source, 1, target_object(target));
        let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
            unreachable!()
        };
        ability.source_zone = rv1::AbilitySourceZone::Hand as i32;
        engine
            .apply_scenario_command(actor_id, &command)
            .expect("activate actual Boseiju channel");
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        let batch = resolve_one(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
        let choice = find_resolution_choice(&batch)
            .expect("the other player's optional search remains available");
        assert_eq!(choice.deciding_player_id, owner_id);
        let batch = engine
            .apply_scenario_command(
                owner_id,
                &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
            )
            .expect("choose the allowed search");
        assert!(find_resolution_choice(&batch)
            .unwrap()
            .candidate_object_ids
            .contains(&land));
        let completed = engine
            .apply_scenario_command(owner_id, &submit_resolution_choice(vec![land]))
            .expect("find the other player's land");
        assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
        assert!(shuffled(&completed, owner_id));
        assert!(engine.state.pending_resolution.is_none());
    }
}

fn blocked_gifts_case(empty: bool) {
    let mut engine = game(261_027_714 + u64::from(empty));
    cast_ashiok(&mut engine, 1, 1);
    let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
    if empty {
        // Synthetic setup empties the opponent's library without performing a draw.
        while let Some(object) = engine.state.players[1].library.pop_back() {
            engine.state.objects.get_mut(&object).unwrap().zone = Zone::Hand;
            engine.state.players[1].hand.push(object);
        }
    } else {
        inject_library_card(&mut engine, 1, "island");
    }
    let gifts = inject_card_into_hand(&mut engine, 1, "gifts_ungiven");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 3,
            u: 1,
            ..Default::default()
        },
    );
    pass_to(&mut engine, 1);
    let slot = hand_index_for_card(&engine, 1, "gifts_ungiven");
    engine
        .apply_scenario_command(1, &cast_spell(slot, target_player(0)))
        .expect("cast actual Gifts");
    let batch = resolve_one(&mut engine);
    assert!(
        find_resolution_choice(&batch).is_none(),
        "no search or empty-partition choice is offered"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&gifts].zone, Zone::Graveyard);
    assert!(
        shuffled(&batch, 1),
        "the Program reaches its unconditional shuffle tail"
    );
    assert!(
        !engine
            .state
            .stack
            .iter()
            .any(|item| item.source_permanent_id == Some(wan)),
        "no search event occurs, including with an empty library"
    );
}

#[test]
fn ashiok_blocked_gifts_uses_empty_result_and_shuffles() {
    blocked_gifts_case(false);
}

#[test]
fn ashiok_blocks_gifts_empty_library_before_the_search_event_shortcut() {
    blocked_gifts_case(true);
}

#[test]
fn ashiok_removes_only_library_from_say_its_names_search_scope() {
    let mut engine = game(261_027_716);
    cast_ashiok(&mut engine, 1, 1);
    let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
    let source = inject_graveyard_card(&mut engine, 1, "say_its_name");
    let others = [
        inject_graveyard_card(&mut engine, 1, "say_its_name"),
        inject_graveyard_card(&mut engine, 1, "say_its_name"),
    ];
    let altanak = inject_card_into_hand(&mut engine, 1, "altanak,_the_thrice-called");
    // Synthetic active-turn setup permits the printed sorcery-speed graveyard ability.
    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 1;
    let object_ref = |object_id| rv1::CostObjectRef {
        object_id,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
    };
    let command = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
            source_object_id: source,
            ability_index: 0,
            cost_selections: vec![rv1::CostSelection {
                cost_index: 1,
                selection: Some(rv1::cost_selection::Selection::GraveyardObjects(
                    rv1::CostObjectRefs {
                        objects: others.into_iter().map(object_ref).collect(),
                    },
                )),
            }],
            source_zone: rv1::AbilitySourceZone::Graveyard as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(1, &command)
        .expect("pay the real three-card exile cost");
    let batch = resolve_one(&mut engine);
    let zones = find_resolution_choice(&batch).expect("hand/graveyard scopes remain available");
    let library = rv1::ChoiceCandidateSourceZone::Library as i32;
    let hand = rv1::ChoiceCandidateSourceZone::Hand as i32;
    let graveyard = rv1::ChoiceCandidateSourceZone::Graveyard as i32;
    assert!(
        zones
            .resolution_branches
            .iter()
            .all(|branch| !branch.search_zones.contains(&library)),
        "the opponent cannot select a scope including Library"
    );
    assert!(zones
        .resolution_branches
        .iter()
        .any(|branch| branch.search_zones.contains(&hand)));
    assert!(zones
        .resolution_branches
        .iter()
        .any(|branch| branch.search_zones.contains(&graveyard)));
    let branch = zones
        .resolution_branches
        .iter()
        .find(|branch| branch.search_zones == [hand])
        .unwrap()
        .branch_index;
    let batch = engine
        .apply_scenario_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: branch,
                    ..Default::default()
                })),
            },
        )
        .expect("select Hand only");
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&altanak));
    let complete = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![altanak]))
        .expect("find Altanak in hand");
    assert_eq!(engine.state.objects[&altanak].zone, Zone::Battlefield);
    // Oracle says "If you search your library this way, shuffle":
    // https://mtg-jp.com/products/card-gallery/0000283/673602/
    assert!(
        !shuffled(&complete, 1),
        "the Hand-only search does not shuffle the library"
    );
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.source_permanent_id == Some(wan)));
}

#[test]
fn ashiok_search_restriction_tracks_live_source_and_captured_ability_controller() {
    // Each Wilds activation and search is real; control/suppression mutations isolate the
    // live static at resolution, after the source has already paid its sacrifice cost.
    for case in 0..7 {
        let mut engine = game(261_027_720 + case);
        let ashiok = cast_ashiok(&mut engine, 1, 1);
        let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
        let wilds = inject_permanent_on_battlefield(&mut engine, 1, "evolving_wilds");
        inject_library_card(&mut engine, 1, "forest");
        pass_to(&mut engine, 1);
        apply_ability(&mut engine, 1, wilds, 0, vec![]).expect("activate Wilds");
        assert_eq!(engine.state.objects[&wilds].zone, Zone::Graveyard);

        if matches!(case, 2 | 3) {
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(ashiok),
                kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
                condition: None,
                duration: EffectDuration::Indefinite,
                timestamp: engine.state.command_index + 1,
            });
            if case == 3 {
                engine.state.continuous_effects.retain(|effect| {
                    !matches!(effect.kind, ContinuousEffectKind::Layer6RemoveAllAbilities)
                });
            }
        }
        if matches!(case, 4 | 5) {
            let controller = if case == 4 { 1 } else { 2 };
            engine.state.players[0]
                .battlefield
                .retain(|object| *object != ashiok);
            engine.state.players[controller].battlefield.push(ashiok);
            let object = engine.state.objects.get_mut(&ashiok).unwrap();
            object.base_controller = controller as i32;
            object.controller = controller as i32;
        }
        if case == 1 {
            engine.enable_dev_commands();
            let actor = engine.state.priority_player_id();
            engine
                .apply_scenario_command(
                    actor,
                    &RuledCommand {
                        cmd: Some(Cmd::DevCommand(DevCommand {
                            target_player_id: 0,
                            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                                card_name: "Ashiok, Dream Render".into(),
                                zone: DevZone::Graveyard as i32,
                                ready: false,
                            })),
                        })),
                    },
                )
                .expect("remove Ashiok before the search begins");
        }
        if case == 6 {
            // Its sacrificed physical source is no longer a battlefield permanent; changing
            // that object's field cannot change the captured controller of the ability.
            engine.state.objects.get_mut(&wilds).unwrap().controller = 0;
        }
        let batch = resolve_one(&mut engine);
        let blocked = matches!(case, 0 | 3 | 5 | 6);
        if blocked {
            assert!(find_resolution_choice(&batch).is_none(), "case {case}");
            assert!(shuffled(&batch, 1), "mandatory shuffle in case {case}");
            assert!(!engine
                .state
                .stack
                .iter()
                .any(|item| item.source_permanent_id == Some(wan)));
        } else {
            let choice = find_resolution_choice(&batch).expect("a legal search is offered");
            assert_eq!(choice.deciding_player_id, 1);
            let complete = engine
                .apply_scenario_command(1, &submit_resolution_choice(vec![]))
                .expect("fail to find in the allowed search");
            assert!(shuffled(&complete, 1));
            assert!(
                engine
                    .state
                    .stack
                    .iter()
                    .any(|item| item.source_permanent_id == Some(wan)),
                "a legal search fires its observer even when nothing is found"
            );
        }
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn ashiok_rejects_a_previously_offered_optional_search_if_the_static_becomes_active() {
    let mut engine = game(261_027_727);
    let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
    inject_card_into_hand(&mut engine, 1, "solemn_simulacrum");
    move_ready_to_battlefield(&mut engine, 1, "solemn_simulacrum");
    let offered = resolve_one(&mut engine);
    assert!(
        find_resolution_choice(&offered).is_some(),
        "the real Solemn trigger offers its search"
    );
    let library_before = engine.state.players[1].library.clone();
    // Synthetic timing isolates a newly active static while the existing Program choice is parked.
    fixture_ashiok(&mut engine);
    engine
        .apply_scenario_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect_err("a previously offered search is no longer selectable");
    assert!(engine.state.pending_resolution.is_some());
    let finished = engine
        .apply_scenario_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("the original chooser can still decline");
    assert!(!shuffled(&finished, 1));
    assert_eq!(engine.state.players[1].library, library_before);
    assert!(engine.state.pending_resolution.is_none());
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.source_permanent_id == Some(wan)));
}

#[test]
fn ashiok_does_not_retroactively_cancel_an_already_started_search() {
    let mut engine = game(261_027_728);
    let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
    let wilds = inject_permanent_on_battlefield(&mut engine, 1, "evolving_wilds");
    let forest = inject_library_card(&mut engine, 1, "forest");
    pass_to(&mut engine, 1);
    apply_ability(&mut engine, 1, wilds, 0, vec![]).expect("activate Wilds");
    let search = resolve_one(&mut engine);
    assert!(find_resolution_choice(&search)
        .unwrap()
        .candidate_object_ids
        .contains(&forest));
    // Synthetic insertion changes the static only after the private search has begun.
    fixture_ashiok(&mut engine);
    let complete = engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![forest]))
        .expect("finish the already-started search");
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert!(shuffled(&complete, 1));
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.source_permanent_id == Some(wan)));
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn ashiok_does_not_prevent_opponents_scrying_or_drawing() {
    let mut engine = game(261_027_729);
    cast_ashiok(&mut engine, 1, 1);
    let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
    let top = place_top(&mut engine, 1, &["island"])[0];
    inject_card_into_hand(&mut engine, 1, "opt");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    pass_to(&mut engine, 1);
    let slot = hand_index_for_card(&engine, 1, "opt");
    engine
        .apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("cast actual Opt");
    let batch = resolve_one(&mut engine);
    let choice = find_resolution_choice(&batch).expect("Scry remains available");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop);
    assert!(choice.candidate_object_ids.contains(&top));
    assert!(choice.public_reveal.is_none());
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![]))
        .expect("keep the top card");
    assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.source_permanent_id == Some(wan)));
}

#[test]
fn ashiok_blocked_search_replays_only_accepted_wire_commands() {
    use prost::Message;
    fn fixture() -> (GameEngine, u32, u32) {
        let mut engine = game(261_027_730);
        cast_ashiok(&mut engine, 1, 1);
        let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
        let wilds = inject_permanent_on_battlefield(&mut engine, 1, "evolving_wilds");
        inject_library_card(&mut engine, 1, "forest");
        (engine, wan, wilds)
    }
    let (mut engine, wan, wilds) = fixture();
    let (mut replay, replay_wan, replay_wilds) = fixture();
    assert_eq!((wan, wilds), (replay_wan, replay_wilds));
    let apply = |engine: &mut GameEngine, replay: &mut GameEngine, actor, command: RuledCommand| {
        let decoded = RuledCommand::decode(command.encode_to_vec().as_slice()).unwrap();
        let actual = engine
            .apply_scenario_command(actor, &command)
            .expect("accepted command");
        assert_eq!(
            actual,
            replay
                .apply_scenario_command(actor, &decoded)
                .expect("accepted replay")
        );
        actual
    };
    while engine.state.priority_player_id() != 1 {
        let actor = engine.state.priority_player_id();
        apply(&mut engine, &mut replay, actor, pass());
    }
    let activate = activate_ability_for(&engine, wilds, 0, vec![]);
    apply(&mut engine, &mut replay, 1, activate);
    let mut last = RuledEventBatch::default();
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        last = apply(&mut engine, &mut replay, actor, pass());
    }
    assert!(find_resolution_choice(&last).is_none());
    assert!(shuffled(&last, 1));
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.source_permanent_id == Some(wan)));
    assert_eq!(
        engine.state.players[1].library,
        replay.state.players[1].library
    );
    assert_eq!(engine.state.command_index, replay.state.command_index);
    let actor = engine.state.priority_player_id();
    apply(&mut engine, &mut replay, actor, pass());
}

#[test]
fn ashiok_does_not_interrupt_started_search_or_its_copy_entry_choice() {
    for leave in [false, true] {
        let mut engine = game(261_027_731 + u64::from(leave));
        let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
        let copy_source = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
        let forge = inject_permanent_on_battlefield(&mut engine, 1, "kuldotha_forgemaster");
        let first = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
        let second = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
        let steel = inject_library_card(&mut engine, 1, "sculpting_steel");
        pass_to(&mut engine, 1);
        let mut command = activate_ability_for(&engine, forge, 0, vec![]);
        let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
            unreachable!()
        };
        ability.cost_selections = vec![CostSelection {
            cost_index: 1,
            selection: Some(rv1::cost_selection::Selection::BattlefieldObjects(
                rv1::CostObjectRefs {
                    objects: [forge, first, second]
                        .into_iter()
                        .map(|object_id| rv1::CostObjectRef {
                            object_id,
                            zone_change_generation: engine
                                .state
                                .zone_change_generation
                                .get(&object_id)
                                .copied()
                                .unwrap_or(0),
                        })
                        .collect(),
                },
            )),
        }];
        engine
            .apply_scenario_command(1, &command)
            .expect("pay Forgemaster's real costs");
        let ability_id = engine.state.stack.last().unwrap().id;
        let search = resolve_one(&mut engine);
        assert!(find_resolution_choice(&search)
            .unwrap()
            .candidate_object_ids
            .contains(&steel));
        // The restriction appears only after the real search has begun.
        fixture_ashiok(&mut engine);
        let entry = engine
            .apply_scenario_command(1, &submit_resolution_choice(vec![steel]))
            .expect("choose the found Sculpting Steel");
        let choice = find_resolution_choice(&entry).expect("the actual copy-entry choice parks");
        assert_eq!(choice.deciding_player_id, 1);
        assert_eq!(choice.choice_kind(), ChoiceKind::CopySource);
        assert!(choice.candidate_object_ids.contains(&copy_source));
        let mut events = entry.events;
        if leave {
            events.extend(
                engine
                    .apply_scenario_command(2, &concede())
                    .expect("an unrelated player can leave during the entry choice")
                    .events,
            );
            assert!(engine.state.pending_resolution.is_some());
        }
        let finished = engine
            .apply_scenario_command(1, &submit_resolution_choice(vec![copy_source]))
            .expect("finish the already-started search and copy entry");
        assert!(shuffled(&finished, 1));
        events.extend(finished.events);
        assert_eq!(engine.state.objects[&steel].zone, Zone::Battlefield);
        assert!(engine.characteristics(steel).unwrap().is_artifact());
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == ability_id))
                .count(),
            1
        );
        assert!(engine
            .state
            .stack
            .iter()
            .any(|item| item.source_permanent_id == Some(wan)));
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn ashiok_does_not_interrupt_started_commander_search_on_departure() {
    for departure in [None, Some(0), Some(1)] {
        let mut engine = game(261_027_733 + departure.map_or(0, |player| player as u64 + 1));
        let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
        let commander = inject_library_card(&mut engine, 1, "atraxa,_praetors_voice");
        // This designates the physical object for the 903.9b choice; it is not a deck-list test.
        engine.state.players[1]
            .declared_commander_object_ids
            .push(commander);
        inject_card_into_hand(&mut engine, 1, "demonic_tutor");
        give_mana(
            &mut engine,
            1,
            ManaGift {
                c: 1,
                b: 1,
                ..Default::default()
            },
        );
        // Synthetic turn setup enables the actual sorcery cast and typed search.
        engine.state.active_player_idx = 1;
        engine.state.priority_idx = 1;
        let slot = hand_index_for_card(&engine, 1, "demonic_tutor");
        engine
            .apply_scenario_command(1, &cast_spell(slot, vec![]))
            .expect("cast Demonic Tutor");
        let spell = engine.state.stack.last().unwrap().id;
        let search = resolve_one(&mut engine);
        assert!(find_resolution_choice(&search)
            .unwrap()
            .candidate_object_ids
            .contains(&commander));
        // Synthetic late insertion cannot stop an already begun search.
        fixture_ashiok(&mut engine);
        let owner_choice = engine
            .apply_scenario_command(1, &submit_resolution_choice(vec![commander]))
            .expect("search selects the declared commander");
        let choice =
            find_resolution_choice(&owner_choice).expect("commander owner chooses destination");
        assert_eq!(choice.deciding_player_id, 1);
        assert_eq!(engine.state.objects[&commander].zone, Zone::Library);
        if let Some(player) = departure {
            let left = engine
                .apply_scenario_command(player, &concede())
                .expect("player concedes");
            if player == 1 {
                assert!(engine.state.pending_resolution.is_none());
                assert!(!engine.state.objects.contains_key(&commander));
                assert!(!engine.state.objects.contains_key(&spell));
                assert!(
                    !shuffled(&left, 1),
                    "a departed owner has no library to shuffle"
                );
                continue;
            }
            assert!(
                engine.state.pending_resolution.is_some(),
                "owner's choice survives blocker departure"
            );
        }
        let choice = RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                decision: ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: 1,
                ..Default::default()
            })),
        };
        let finished = engine
            .apply_scenario_command(1, &choice)
            .expect("the owner chooses hand");
        assert_eq!(engine.state.objects[&commander].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&spell].zone, Zone::Graveyard);
        assert!(shuffled(&finished, 1));
        assert_eq!(
            finished
                .events
                .iter()
                .filter(|event| matches!(&event.ev,
            Some(Ev::StackResolved(resolved)) if resolved.object_id == spell))
                .count(),
            1
        );
        if departure.is_none() {
            assert!(engine
                .state
                .stack
                .iter()
                .any(|item| item.source_permanent_id == Some(wan)));
        }
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn say_its_name_unrestricted_zone_scopes_preserve_conditional_search_and_shuffle() {
    for (case, selected_zone) in [
        rv1::ChoiceCandidateSourceZone::Hand,
        rv1::ChoiceCandidateSourceZone::Graveyard,
        rv1::ChoiceCandidateSourceZone::Library,
    ]
    .into_iter()
    .enumerate()
    {
        let mut engine = game(261_027_737 + case as u64);
        let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
        let source = inject_graveyard_card(&mut engine, 1, "say_its_name");
        let others = [
            inject_graveyard_card(&mut engine, 1, "say_its_name"),
            inject_graveyard_card(&mut engine, 1, "say_its_name"),
        ];
        inject_library_card(&mut engine, 1, "altanak,_the_thrice-called");
        engine.state.active_player_idx = 1;
        engine.state.priority_idx = 1;
        let command = RuledCommand {
            cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
                source_object_id: source,
                ability_index: 0,
                cost_selections: vec![rv1::CostSelection {
                    cost_index: 1,
                    selection: Some(rv1::cost_selection::Selection::GraveyardObjects(
                        rv1::CostObjectRefs {
                            objects: others
                                .into_iter()
                                .map(|object_id| rv1::CostObjectRef {
                                    object_id,
                                    zone_change_generation: engine
                                        .state
                                        .zone_change_generation
                                        .get(&object_id)
                                        .copied()
                                        .unwrap_or(0),
                                })
                                .collect(),
                        },
                    )),
                }],
                source_zone: rv1::AbilitySourceZone::Graveyard as i32,
                expected_zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&source)
                    .copied()
                    .unwrap_or(0),
                ..Default::default()
            })),
        };
        engine
            .apply_scenario_command(1, &command)
            .expect("pay Say Its Name's actual cost");
        let batch = resolve_one(&mut engine);
        let scopes = find_resolution_choice(&batch).expect("choose an unrestricted scope");
        let branch = scopes
            .resolution_branches
            .iter()
            .find(|branch| branch.search_zones == [selected_zone as i32])
            .unwrap()
            .branch_index;
        let scope_command = RuledCommand {
            cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                decision: ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: branch,
                ..Default::default()
            })),
        };
        engine
            .apply_scenario_command(1, &scope_command)
            .expect("choose the permitted scope");
        let complete = engine
            .apply_scenario_command(1, &submit_resolution_choice(vec![]))
            .expect("choose no matching card");
        let searched_library = selected_zone == rv1::ChoiceCandidateSourceZone::Library;
        assert_eq!(shuffled(&complete, 1), searched_library);
        assert_eq!(
            engine
                .state
                .stack
                .iter()
                .any(|item| item.source_permanent_id == Some(wan)),
            searched_library
        );
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn ashiok_blocked_fabled_passage_binds_empty_land_and_finishes_its_real_tail() {
    let mut engine = game(261_027_741);
    cast_ashiok(&mut engine, 1, 1);
    let wan = inject_permanent_on_battlefield(&mut engine, 0, "wan_shi_tong,_librarian");
    // Synthetic tapped lands ensure the real four-land condition is true after the cost.
    let lands: Vec<_> = (0..4)
        .map(|_| inject_permanent_on_battlefield(&mut engine, 1, "forest"))
        .collect();
    for &land in &lands {
        engine.state.objects.get_mut(&land).unwrap().tapped = true;
    }
    let passage = inject_permanent_on_battlefield(&mut engine, 1, "fabled_passage");
    let candidate = inject_library_card(&mut engine, 1, "forest");
    pass_to(&mut engine, 1);
    apply_ability(&mut engine, 1, passage, 0, vec![])
        .expect("pay tap and sacrifice Fabled Passage");
    assert_eq!(engine.state.objects[&passage].zone, Zone::Graveyard);
    let ability = engine
        .state
        .stack
        .last()
        .expect("Fabled Passage ability")
        .id;
    let batch = resolve_one(&mut engine);
    assert!(find_resolution_choice(&batch).is_none());
    assert!(shuffled(&batch, 1), "the mandatory shuffle still occurs");
    assert_eq!(engine.state.objects[&candidate].zone, Zone::Library);
    assert!(
        lands.iter().all(|land| engine.state.objects[land].tapped),
        "the empty found-land binding cannot untap a different land"
    );
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
        Some(Ev::StackResolved(resolved)) if resolved.object_id == ability))
            .count(),
        1
    );
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.source_permanent_id == Some(wan)));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}
