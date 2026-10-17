use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, ChoiceCandidateSourceZone, ChoiceKind,
    ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            cast_spell: None,
            spell_cast_announcement: None,
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: vec![],
            chosen_player_ids: vec![],
        })),
    }
}

fn decline_resolution_choice() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: 0,
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::Decline as i32,
            selected_branch_index: 0,
            cast_spell: None,
            spell_cast_announcement: None,
            chosen_combat_defender: None,
            payment: None,
            restricted_mana: vec![],
            chosen_player_ids: vec![],
        })),
    }
}

fn put_on_top(engine: &mut GameEngine, player: usize, ids: &[&str]) -> Vec<u32> {
    let objects: Vec<u32> = ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|oid| !objects.contains(oid));
    for object_id in objects.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    objects
}

fn zone_ability(
    engine: &GameEngine,
    source: u32,
    source_zone: AbilitySourceZone,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            ability_index: 0,
            targets,
            source_zone: source_zone as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ..Default::default()
        })),
    }
}

#[test]
fn living_phone_uses_printed_power_and_random_bottom_is_replay_deterministic() {
    fn play() -> Vec<u32> {
        let decks = Some(vec![
            deck_with("plains", &["living_phone", "lightning_bolt"]),
            deck_with("island", &[]),
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            11000,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new");
        advance_to_main1_from_game_start(&mut engine);
        let living = relocate_to_battlefield(&mut engine, 0, "living_phone", false);
        ensure_in_hand(&mut engine, 0, "lightning_bolt");
        let looked_at = put_on_top(
            &mut engine,
            0,
            &[
                "grizzly_bears",
                "hill_giant",
                "forest",
                "reckless_waif_merciless_predator",
                "lightning_bolt",
                "island",
            ],
        );
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
        engine
            .apply_scenario_command(0, &cast_spell(bolt, target_object(living)))
            .expect("cast Bolt at Living Phone");
        pass_both_players(&mut engine);
        assert_eq!(engine.state.objects[&living].zone, Zone::Graveyard);

        engine
            .apply_scenario_command(0, &pass())
            .expect("controller pass");
        let batch = engine
            .apply_scenario_command(1, &pass())
            .expect("resolve death trigger");
        let choice = find_resolution_choice(&batch).expect("Living Phone look choice");
        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
        assert_eq!(choice.candidate_object_ids, looked_at[..5]);
        assert_eq!(
            choice.candidate_selectable,
            [true, false, false, true, false]
        );

        engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![looked_at[0]]))
            .expect("choose printed power 2 creature");
        assert!(engine.state.players[0].hand.contains(&looked_at[0]));
        let library = engine.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(library.first().copied(), Some(looked_at[5]));
        library
    }

    assert_eq!(play(), play());
}

#[test]
fn say_its_name_mills_before_publishing_the_optional_current_graveyard_choice() {
    let decks = Some(vec![
        deck_with("forest", &["say_its_name"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        11001,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "say_its_name");
    let milled = put_on_top(&mut engine, 0, &["grizzly_bears", "forest", "island"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "say_its_name");
    engine
        .apply_scenario_command(0, &cast_spell(index, vec![]))
        .expect("cast");
    engine
        .apply_scenario_command(0, &pass())
        .expect("caster pass");
    let batch = engine
        .apply_scenario_command(1, &pass())
        .expect("resolve to choice");

    let choice = find_resolution_choice(&batch).expect("post-mill graveyard choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::GraveyardCards);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert_eq!(choice.candidate_object_ids, milled);
    assert_eq!(
        choice.candidate_source_zones,
        [ChoiceCandidateSourceZone::Graveyard as i32; 3]
    );
    assert!(milled
        .iter()
        .all(|oid| engine.state.objects[oid].zone == Zone::Graveyard));

    engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline optional return");
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn uncharted_voyage_asks_the_target_owner_then_resumes_with_casters_surveil() {
    let decks = Some(vec![
        deck_with("island", &["uncharted_voyage"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        11002,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "uncharted_voyage");
    let target = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(&engine, 0, "uncharted_voyage");
    engine
        .apply_scenario_command(0, &cast_spell(index, target_object(target)))
        .expect("cast");
    engine
        .apply_scenario_command(0, &pass())
        .expect("caster pass");
    let batch = engine
        .apply_scenario_command(1, &pass())
        .expect("resolve to owner choice");
    let choice = find_resolution_choice(&batch).expect("owner top/bottom choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(choice.resolution_branches.len(), 2);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert!(engine.state.players[1].battlefield.contains(&target));
    assert!(
        engine.apply_scenario_command(0, &select_branch(0)).is_err(),
        "caster is not the owner"
    );
    assert!(
        engine
            .apply_scenario_command(1, &decline_resolution_choice())
            .is_err(),
        "owner placement is mandatory"
    );

    let resumed = engine
        .apply_scenario_command(1, &select_branch(0))
        .expect("owner chooses top");
    assert_eq!(
        engine.state.players[1].library.front().copied(),
        Some(target)
    );
    assert_eq!(engine.state.objects[&target].zone, Zone::Library);
    let surveil =
        find_resolution_choice(&resumed).expect("caster's surveil resumes after placement");
    assert_eq!(surveil.deciding_player_id, 0);
    assert_eq!(surveil.choice_kind(), ChoiceKind::LibraryLook);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("keep surveilled card on top");
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn embermouth_rechecks_the_dragon_condition_when_the_search_completes() {
    fn resolve(with_dragon: bool, add_dragon_after_search_offer: bool) -> (GameEngine, u32, u64) {
        let decks = Some(vec![
            deck_with("mountain", &["embermouth_sentinel"]),
            deck_with("island", &[]),
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            11003,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new");
        advance_to_main1_from_game_start(&mut engine);
        ensure_in_hand(&mut engine, 0, "embermouth_sentinel");
        if with_dragon {
            inject_creature_on_battlefield(&mut engine, 0, "dirgur_island_dragon_skimming_strike");
        }
        let basic = inject_library_card(&mut engine, 0, "forest");
        let basic_generation = engine
            .state
            .zone_change_generation
            .get(&basic)
            .copied()
            .unwrap_or(0);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let sentinel = hand_index_for_card(&engine, 0, "embermouth_sentinel");
        engine
            .apply_scenario_command(0, &cast_spell(sentinel, vec![]))
            .expect("cast Sentinel");
        pass_both_players(&mut engine);
        engine
            .apply_scenario_command(0, &pass())
            .expect("controller pass");
        let optional = engine
            .apply_scenario_command(1, &pass())
            .expect("open optional ETB choice");
        assert_eq!(
            find_resolution_choice(&optional)
                .expect("optional search branch")
                .choice_kind(),
            ChoiceKind::ResolutionBranch
        );
        let search = engine
            .apply_scenario_command(0, &select_branch(0))
            .expect("take search branch");
        let choice = find_resolution_choice(&search).expect("basic-land search");
        assert!(choice.candidate_object_ids.contains(&basic));
        if add_dragon_after_search_offer {
            inject_creature_on_battlefield(&mut engine, 0, "dirgur_island_dragon_skimming_strike");
        }
        let completion = engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![basic]))
            .expect("choose exact basic");
        assert_eq!(
            completion
                .events
                .iter()
                .filter(|event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library."))
                .count(),
            1,
            "one accepted search shuffles once"
        );
        let reveals = completion
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(Ev::CardsRevealed(reveal)) => Some(reveal),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(reveals.len(), 1, "the selected card is revealed once");
        assert_eq!(reveals[0].cards.len(), 1);
        assert_eq!(reveals[0].cards[0].object_id, basic);
        (engine, basic, basic_generation)
    }

    let (without_dragon, top, top_generation) = resolve(false, false);
    assert_eq!(without_dragon.state.objects[&top].zone, Zone::Library);
    assert_eq!(without_dragon.state.players[0].library.front(), Some(&top));
    assert_eq!(
        without_dragon
            .state
            .zone_change_generation
            .get(&top)
            .copied()
            .unwrap_or(0),
        top_generation,
        "the exact selected object remains in the library without a zone-change generation"
    );

    let (with_dragon, battlefield, battlefield_generation) = resolve(true, false);
    assert_eq!(
        with_dragon.state.objects[&battlefield].zone,
        Zone::Battlefield
    );
    assert!(with_dragon.state.objects[&battlefield].tapped);
    assert_eq!(
        with_dragon
            .state
            .zone_change_generation
            .get(&battlefield)
            .copied()
            .unwrap_or(0),
        battlefield_generation + 1
    );

    let (dragon_added_after_offer, battlefield, before_generation) = resolve(false, true);
    assert_eq!(
        dragon_added_after_offer.state.objects[&battlefield].zone,
        Zone::Battlefield,
        "a Dragon entering after the ETB choice but before the library choice changes the destination"
    );
    assert!(dragon_added_after_offer.state.objects[&battlefield].tapped);
    assert_eq!(
        dragon_added_after_offer
            .state
            .zone_change_generation
            .get(&battlefield)
            .copied()
            .unwrap_or(0),
        before_generation + 1
    );
}

#[test]
fn embermouth_declining_the_optional_search_does_not_shuffle_or_reveal() {
    let decks = Some(vec![
        deck_with("mountain", &["embermouth_sentinel"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        11030,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "embermouth_sentinel");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let sentinel = hand_index_for_card(&engine, 0, "embermouth_sentinel");
    engine
        .apply_scenario_command(0, &cast_spell(sentinel, vec![]))
        .expect("cast Sentinel");
    pass_both_players(&mut engine);
    engine
        .apply_scenario_command(0, &pass())
        .expect("controller passes");
    let optional = engine
        .apply_scenario_command(1, &pass())
        .expect("open optional ETB choice");
    assert_eq!(
        find_resolution_choice(&optional)
            .expect("optional search branch")
            .choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    let library_before = engine.state.players[0].library.clone();
    let declined = engine
        .apply_scenario_command(0, &decline_resolution_choice())
        .expect("decline the optional search");

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].library, library_before);
    assert!(declined.events.iter().all(|event| {
        !matches!(&event.ev, Some(Ev::CardsRevealed(_)))
            && !matches!(
                &event.ev,
                Some(Ev::Log(log))
                    if log.text == "P0 shuffles their library."
            )
    }));
}

#[test]
fn altanak_hand_ability_discards_itself_and_returns_the_exact_land_tapped() {
    let decks = Some(vec![
        deck_with("forest", &["altanak,_the_thrice-called"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        11004,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let altanak = inject_card_into_hand(&mut engine, 0, "altanak,_the_thrice-called");
    let land = inject_graveyard_card(&mut engine, 0, "mountain");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(
            0,
            &zone_ability(
                &engine,
                altanak,
                AbilitySourceZone::Hand,
                target_object(land),
            ),
        )
        .expect("activate Altanak from hand");
    assert_eq!(engine.state.objects[&altanak].zone, Zone::Graveyard);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert!(engine.state.objects[&land].tapped);

    let snapshot = engine.initial_response_batch();
    let returned_land = snapshot
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .find(|player| player.player_id == 0)
                .and_then(|player| {
                    player
                        .battlefield_objects
                        .iter()
                        .find(|object| object.object_id == land)
                }),
            _ => None,
        })
        .expect("returned Mountain in battlefield snapshot");
    assert!(
        returned_land.is_land,
        "Altanak's returned Mountain must retain authoritative land identity"
    );
}

#[test]
fn altanak_triggers_only_for_an_opponent_controlled_spell_or_ability_target() {
    let decks = Some(vec![
        deck_with("forest", &["altanak,_the_thrice-called"]),
        deck_with("mountain", &["lightning_bolt"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        11005,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let altanak = relocate_to_battlefield(&mut engine, 0, "altanak,_the_thrice-called", false);
    ensure_in_hand(&mut engine, 1, "lightning_bolt");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let before = engine.state.players[0].hand.len();

    engine
        .apply_scenario_command(0, &pass())
        .expect("give opponent priority");
    let bolt = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_scenario_command(1, &cast_spell(bolt, target_object(altanak)))
        .expect("opponent targets Altanak");
    assert_eq!(engine.state.stack.len(), 2, "Altanak trigger is above Bolt");
    assert_eq!(engine.state.stack.last().expect("trigger").controller, 0);
    let first = engine.state.priority_player_id();
    let second = 1 - first;
    engine
        .apply_scenario_command(first, &pass())
        .expect("first pass");
    let resolved = engine
        .apply_scenario_command(second, &pass())
        .expect("second pass");
    assert_eq!(
        engine.state.players[0].hand.len(),
        before + 1,
        "events: {:?}",
        resolved.events
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Bolt remains below the trigger"
    );
}

#[test]
fn tempest_hawk_combat_damage_trigger_searches_an_exact_copy_into_hand() {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        110_006,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_declare_attackers(&mut engine);
    let hawk = inject_creature_on_battlefield(&mut engine, 0, "tempest_hawk");
    let searched_hawk = inject_library_card(&mut engine, 0, "tempest_hawk");
    let opponent_life = engine.state.players[1].life;

    engine
        .apply_scenario_command(0, &declare_attackers(vec![hawk]))
        .expect("Tempest Hawk attacks");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(1, &declare_blockers(vec![]))
        .expect("opponent leaves Tempest Hawk unblocked");
    for _ in 0..5 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        pass_priority_round(&mut engine);
    }

    assert_eq!(engine.state.players[1].life, opponent_life - 2);
    let branch = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("combat-damage trigger offers its optional branch");
    assert_eq!(
        branch.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    assert_eq!(branch.deciding_player, 0);
    engine
        .apply_scenario_command(0, &select_branch(0))
        .expect("choose the search branch");
    let search = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("exact-name library search")
        .presentation
        .clone();
    assert_eq!(search.choice_kind, ChoiceKind::LibrarySearch);
    assert_eq!(search.candidates, [searched_hawk]);
    let completion = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![searched_hawk]))
        .expect("choose the exact Tempest Hawk copy");

    assert_eq!(engine.state.objects[&searched_hawk].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&searched_hawk));
    assert!(completion.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::CardsRevealed(reveal)) if reveal.cards.iter().any(|card| card.object_id == searched_hawk)
    )));
    assert!(completion.events.iter().any(
        |event| matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
    ));
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}
