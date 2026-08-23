use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, ChoiceCandidateSourceZone, ChoiceKind,
    ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice,
};

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
        })),
    }
}

fn decline_resolution_choice() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: ResolutionChoiceDecision::Decline as i32,
            selected_branch_index: 0,
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
        let mut engine = GameEngine::new(11000, &[0, 1], 20, decks, true).expect("new");
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
            .apply_command(0, &cast_spell(bolt, target_object(living)))
            .expect("cast Bolt at Living Phone");
        pass_both_players(&mut engine);
        assert_eq!(engine.state.objects[&living].zone, Zone::Graveyard);

        engine.apply_command(0, &pass()).expect("controller pass");
        let batch = engine
            .apply_command(1, &pass())
            .expect("resolve death trigger");
        let choice = find_resolution_choice(&batch).expect("Living Phone look choice");
        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
        assert_eq!(choice.candidate_object_ids, looked_at[..5]);
        assert_eq!(
            choice.candidate_selectable,
            [true, false, false, true, false]
        );

        engine
            .apply_command(0, &submit_resolution_choice(vec![looked_at[0]]))
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
    let mut engine = GameEngine::new(11001, &[0, 1], 20, decks, true).expect("new");
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
        .apply_command(0, &cast_spell(index, vec![]))
        .expect("cast");
    engine.apply_command(0, &pass()).expect("caster pass");
    let batch = engine.apply_command(1, &pass()).expect("resolve to choice");

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
        .apply_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline optional return");
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn uncharted_voyage_asks_the_target_owner_then_resumes_with_casters_surveil() {
    let decks = Some(vec![
        deck_with("island", &["uncharted_voyage"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(11002, &[0, 1], 20, decks, true).expect("new");
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
        .apply_command(0, &cast_spell(index, target_object(target)))
        .expect("cast");
    engine.apply_command(0, &pass()).expect("caster pass");
    let batch = engine
        .apply_command(1, &pass())
        .expect("resolve to owner choice");
    let choice = find_resolution_choice(&batch).expect("owner top/bottom choice");
    assert_eq!(choice.deciding_player_id, 1);
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(choice.resolution_branches.len(), 2);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert!(engine.state.players[1].battlefield.contains(&target));
    assert!(
        engine.apply_command(0, &select_branch(0)).is_err(),
        "caster is not the owner"
    );
    assert!(
        engine
            .apply_command(1, &decline_resolution_choice())
            .is_err(),
        "owner placement is mandatory"
    );

    let resumed = engine
        .apply_command(1, &select_branch(0))
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
        .apply_command(0, &submit_resolution_choice(Vec::new()))
        .expect("keep surveilled card on top");
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn embermouth_rechecks_the_dragon_condition_when_the_search_completes() {
    fn resolve(with_dragon: bool) -> (GameEngine, u32) {
        let decks = Some(vec![
            deck_with("mountain", &["embermouth_sentinel"]),
            deck_with("island", &[]),
        ]);
        let mut engine = GameEngine::new(11003, &[0, 1], 20, decks, true).expect("new");
        advance_to_main1_from_game_start(&mut engine);
        ensure_in_hand(&mut engine, 0, "embermouth_sentinel");
        if with_dragon {
            inject_creature_on_battlefield(&mut engine, 0, "dirgur_island_dragon_skimming_strike");
        }
        let basic = inject_library_card(&mut engine, 0, "forest");
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
            .apply_command(0, &cast_spell(sentinel, vec![]))
            .expect("cast Sentinel");
        pass_both_players(&mut engine);
        engine.apply_command(0, &pass()).expect("controller pass");
        let optional = engine
            .apply_command(1, &pass())
            .expect("open optional ETB choice");
        assert_eq!(
            find_resolution_choice(&optional)
                .expect("optional search branch")
                .choice_kind(),
            ChoiceKind::ResolutionBranch
        );
        let search = engine
            .apply_command(0, &select_branch(0))
            .expect("take search branch");
        let choice = find_resolution_choice(&search).expect("basic-land search");
        assert!(choice.candidate_object_ids.contains(&basic));
        engine
            .apply_command(0, &submit_resolution_choice(vec![basic]))
            .expect("choose exact basic");
        (engine, basic)
    }

    let (without_dragon, top) = resolve(false);
    assert_eq!(without_dragon.state.objects[&top].zone, Zone::Library);
    assert_eq!(without_dragon.state.players[0].library.front(), Some(&top));

    let (with_dragon, battlefield) = resolve(true);
    assert_eq!(
        with_dragon.state.objects[&battlefield].zone,
        Zone::Battlefield
    );
    assert!(with_dragon.state.objects[&battlefield].tapped);
}

#[test]
fn altanak_hand_ability_discards_itself_and_returns_the_exact_land_tapped() {
    let decks = Some(vec![
        deck_with("forest", &["altanak,_the_thrice-called"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(11004, &[0, 1], 20, decks, true).expect("new");
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
        .apply_command(
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
    let mut engine = GameEngine::new(11005, &[0, 1], 20, decks, true).expect("new");
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
        .apply_command(0, &pass())
        .expect("give opponent priority");
    let bolt = hand_index_for_card(&engine, 1, "lightning_bolt");
    engine
        .apply_command(1, &cast_spell(bolt, target_object(altanak)))
        .expect("opponent targets Altanak");
    assert_eq!(engine.state.stack.len(), 2, "Altanak trigger is above Bolt");
    assert_eq!(engine.state.stack.last().expect("trigger").controller, 0);
    let first = engine.state.priority_player_id();
    let second = 1 - first;
    engine.apply_command(first, &pass()).expect("first pass");
    let resolved = engine.apply_command(second, &pass()).expect("second pass");
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
