use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;

/// Demonic Tutor: successful search — the chosen library card arrives in hand and the library
/// is shuffled afterward.
#[test]
fn demonic_tutor_puts_chosen_card_in_hand() {
    let registry = tricerules_cards::registry::global();
    let tutor = registry
        .get("demonic_tutor")
        .expect("Demonic Tutor is registered");
    assert!(tutor.primary_face().spell_effect.is_empty());
    assert!(tutor.primary_face().spell_program.is_some());
    let owner = tricerules_cards::ProgramOwnerId::face(
        "demonic_tutor",
        tricerules_cards::CardFaceId::new("demonic_tutor").expect("stable face ID"),
    );
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("Demonic Tutor's exact face owns its search Program");
    let [search_step] = program.body.steps.as_slice() else {
        panic!("Demonic Tutor has one direct search instruction");
    };
    let tricerules_cards::CompiledInstructionKind::Perform(effect) = &search_step.kind else {
        panic!("Demonic Tutor directly performs SearchLibrary");
    };
    assert!(matches!(
        effect.as_ref(),
        tricerules_cards::primitives::SpellEffectKind::SearchLibrary {
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            optional: false,
            count: 1,
            filter: None,
            destination: tricerules_cards::primitives::SearchDestination::Hand,
            shuffle: true,
            reveal: false,
            selection_bounds: Some(tricerules_cards::primitives::SearchSelectionBounds {
                min: 1,
                max: 1,
            }),
            ..
        }
    ));

    // P0 deck: Demonic Tutor + swamps for mana + a mountain (the search target).
    let decks = Some(vec![
        {
            let mut d = vec!["demonic_tutor".to_string(), "mountain".to_string()];
            while d.len() < 20 {
                d.push("swamp".to_string());
            }
            d
        },
        vec!["island".into(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        1,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "demonic_tutor");

    // Make sure there's at least one card in the library (the mountain target).
    let mountain_oid = inject_library_card(&mut e, 0, "mountain");
    let lib_size_before = e.state.players[0].library.len();
    let hand_size_before = e.state.players[0].hand.len();

    // Cast Demonic Tutor ({1}{B}).
    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "demonic_tutor",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    // Should be parked on a library search choice (ChoiceKind::LibrarySearch).
    let req = find_resolution_choice(&batch).expect("resolution choice required");
    assert_eq!(req.deciding_player_id, 0);
    assert_eq!(
        req.choice_kind(),
        ChoiceKind::LibrarySearch,
        "LibrarySearch (private to searching player)"
    );
    assert_eq!((req.min, req.max), (1, 1));
    assert!(!req.ordered);
    assert!(req.candidate_object_ids.contains(&mountain_oid));
    assert!(e.state.pending_resolution.is_some());
    // Casting moved Demonic Tutor to graveyard; hand should be one less.
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_size_before - 1,
        "demonic tutor moved to graveyard"
    );

    // The compiled mandatory min=1 search must reject an empty answer atomically while the
    // library has a legal candidate.
    let before_empty_answer = serde_json::to_value(&e.state).expect("serialize pending search");
    assert!(e
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .is_err());
    assert_eq!(
        serde_json::to_value(&e.state).expect("serialize state after rejected empty answer"),
        before_empty_answer,
        "rejecting an empty mandatory search answer preserves the complete pending state"
    );

    // Submit the search choice: pick the mountain.
    let completion = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![mountain_oid]))
        .expect("submit search choice");
    assert!(
        !completion.events.iter().any(|event| matches!(
            event.ev,
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(
                _
            ))
        )),
        "Demonic Tutor does not reveal its found card"
    );

    assert!(e.state.pending_resolution.is_none(), "resolution completed");
    // Mountain is now in hand.
    assert!(
        e.state.players[0].hand.contains(&mountain_oid),
        "chosen card is in hand"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_size_before - 1 + 1,
        "net hand size: tutor gone, mountain gained"
    );
    // Library shrinks by one (the mountain was removed from it).
    assert_eq!(
        e.state.players[0].library.len(),
        lib_size_before - 1,
        "library one smaller"
    );
    assert_eq!(
        tricerules_core::state::Zone::Hand,
        e.state
            .objects
            .get(&mountain_oid)
            .expect("mountain obj")
            .zone
    );
    // Demonic Tutor itself is in the graveyard.
    assert_eq!(
        count_card_id_in_graveyard(&e, 0, "demonic_tutor"),
        1,
        "demonic tutor in graveyard"
    );
}

/// Mystical Tutor: only instant and sorcery cards appear as candidates; non-matching cards
/// (e.g., a creature) are not offered.
#[test]
fn mystical_tutor_filters_to_instant_or_sorcery() {
    let decks = Some(vec![
        {
            let mut d = vec!["mystical_tutor".to_string()];
            while d.len() < 20 {
                d.push("island".to_string());
            }
            d
        },
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        2,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "mystical_tutor");

    // Inject a creature (not a valid Mystical Tutor target) and a counterspell (valid).
    let bear_oid = inject_library_card(&mut e, 0, "grizzly_bears");
    let counter_oid = inject_library_card(&mut e, 0, "counterspell");

    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "mystical_tutor",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let req = find_resolution_choice(&batch).expect("resolution choice required");
    assert_eq!(req.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!(
        req.min, 0,
        "a search constrained by card characteristics may fail to find"
    );
    // The creature must NOT appear in candidates.
    assert!(
        !req.candidate_object_ids.contains(&bear_oid),
        "grizzly bears should not be a valid search target"
    );
    // Counterspell (Instant) must appear.
    assert!(
        req.candidate_object_ids.contains(&counter_oid),
        "counterspell must be a valid search target"
    );

    // Choose the counterspell — it should go on top of the library.
    let lib_top_before = e.state.players[0].library.front().copied();
    let completion = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![counter_oid]))
        .expect("submit");
    let reveal = completion
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal)) => {
                Some(reveal)
            }
            _ => None,
        })
        .expect("Mystical Tutor reveals the found card despite shuffling in the same command");
    assert_eq!(reveal.cards.len(), 1);
    assert_eq!(reveal.cards[0].object_id, counter_oid);
    assert_eq!(reveal.cards[0].card_name, "Counterspell");
    assert!(!reveal.reveal_id.is_empty());

    assert!(e.state.pending_resolution.is_none());
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(counter_oid),
        "counterspell is now on top of library"
    );
    assert_ne!(
        lib_top_before,
        Some(counter_oid),
        "top of library changed (was not counterspell before)"
    );
    assert_eq!(
        e.state.objects.get(&counter_oid).expect("counter obj").zone,
        tricerules_core::state::Zone::Library,
        "counterspell zone is Library"
    );
}

/// Mystical Tutor: submitting a non-instant/sorcery card is rejected.
#[test]
fn mystical_tutor_rejects_non_instant_sorcery() {
    let decks = Some(vec![
        {
            let mut d = vec!["mystical_tutor".to_string()];
            while d.len() < 20 {
                d.push("island".to_string());
            }
            d
        },
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        3,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "mystical_tutor");

    // Only a creature in the library (no valid target).
    let bear_oid = inject_library_card(&mut e, 0, "grizzly_bears");

    cast_instant_and_resolve(
        &mut e,
        0,
        "mystical_tutor",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert!(e.state.pending_resolution.is_some());

    // Submitting the bear (not an instant/sorcery) must fail.
    let lib_snapshot: Vec<u32> = e.state.players[0].library.iter().copied().collect();
    let hand_snapshot = e.state.players[0].hand.clone();
    let res = e.apply_scenario_command(0, &submit_resolution_choice(vec![bear_oid]));
    assert!(
        res.is_err(),
        "choosing a non-instant/sorcery card should be illegal"
    );
    assert!(
        e.state.pending_resolution.is_some(),
        "still parked after rejection"
    );
    assert_eq!(
        e.state.players[0]
            .library
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        lib_snapshot,
        "library unchanged"
    );
    assert_eq!(e.state.players[0].hand, hand_snapshot, "hand unchanged");
}

/// Empty library: Demonic Tutor finishes its resultless search without offering a choice.
#[test]
fn demonic_tutor_empty_library_resolves_without_choice() {
    let decks = Some(vec![
        {
            let mut d = vec!["demonic_tutor".to_string()];
            while d.len() < 20 {
                d.push("swamp".to_string());
            }
            d
        },
        vec!["island".into(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "demonic_tutor");
    let tutor_oid = e.state.players[0]
        .hand
        .iter()
        .copied()
        .find(|oid| {
            e.state
                .objects
                .get(oid)
                .is_some_and(|object| object.card_id == "demonic_tutor")
        })
        .expect("Demonic Tutor is in hand");

    // Drain the library completely so there are no valid search targets.
    e.state.players[0].library.clear();
    let other_object_zones_before = e
        .state
        .objects
        .iter()
        .map(|(oid, object)| (*oid, object.zone))
        .filter(|(oid, _)| *oid != tutor_oid)
        .collect::<std::collections::BTreeMap<_, _>>();

    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "demonic_tutor",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    assert!(
        find_resolution_choice(&batch).is_none(),
        "an empty resultless Program search completes without a choice prompt"
    );
    assert!(e.state.pending_resolution.is_none(), "resolution completed");
    assert!(
        e.state.stack.is_empty(),
        "the Tutor spell finished resolving"
    );
    assert!(e.state.players[0].library.is_empty());
    for (oid, zone) in other_object_zones_before {
        assert_eq!(
            e.state.objects[&oid].zone, zone,
            "an empty search moved no other card"
        );
    }
    assert_eq!(
        count_card_id_in_graveyard(&e, 0, "demonic_tutor"),
        1,
        "the Tutor itself resolves to its graveyard"
    );
    for log in ["P0 finds no card.", "P0 shuffles their library."] {
        assert_eq!(
            batch
                .events
                .iter()
                .filter(|event| matches!(
                    &event.ev,
                    Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(message))
                        if message.text == log
                ))
                .count(),
            1,
            "the empty search emits '{log}' exactly once"
        );
    }
}

/// Library search choice_kind is LibrarySearch (private) — verifies relay redaction signal.
#[test]
fn search_library_choice_kind_is_library_search() {
    let decks = Some(vec![
        {
            let mut d = vec!["demonic_tutor".to_string()];
            while d.len() < 20 {
                d.push("swamp".to_string());
            }
            d
        },
        vec!["island".into(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "demonic_tutor");

    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "demonic_tutor",
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );

    let req = find_resolution_choice(&batch).expect("resolution choice required");
    // LibrarySearch signals the relay to strip candidate ids/names from all other players.
    assert_eq!(
        req.choice_kind(),
        ChoiceKind::LibrarySearch,
        "LibrarySearch choice_kind signals relay to redact candidates from opponents"
    );
}

/// CR 709.4: a split card has the combined characteristics of both halves everywhere but the
/// stack, so Fire // Ice is an instant card while it sits in the library and Mystical Tutor can
/// find it. The type filter reads the card's faces, not whole-card flags (which multi-face cards
/// do not carry).
#[test]
fn mystical_tutor_finds_a_split_card_by_face_type() {
    let decks = Some(vec![
        {
            let mut d = vec!["mystical_tutor".to_string()];
            while d.len() < 20 {
                d.push("island".to_string());
            }
            d
        },
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        4,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "mystical_tutor");

    let fire_ice_oid = inject_library_card(&mut e, 0, "fire_ice");

    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "mystical_tutor",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let req = find_resolution_choice(&batch).expect("resolution choice required");
    assert!(
        req.candidate_object_ids.contains(&fire_ice_oid),
        "Fire // Ice is an instant card in the library"
    );

    e.apply_scenario_command(0, &submit_resolution_choice(vec![fire_ice_oid]))
        .expect("submit");
    assert_eq!(
        e.state.players[0].library.front().copied(),
        Some(fire_ice_oid),
        "Fire // Ice is now on top of the library"
    );
}

/// CR 715.4: outside the stack an adventurer card has only its normal characteristics. Stomp is
/// an instant only while Bonecrusher Giant is cast as an Adventure, so Mystical Tutor cannot find
/// the physical card in a library.
#[test]
fn mystical_tutor_excludes_an_adventure_spell_face_in_library() {
    let decks = Some(vec![
        deck_with("island", &["mystical_tutor"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        5,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new");
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "mystical_tutor");

    let bonecrusher_oid = inject_library_card(&mut e, 0, "bonecrusher_giant_stomp");
    let batch = cast_instant_and_resolve(
        &mut e,
        0,
        "mystical_tutor",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let req = find_resolution_choice(&batch).expect("resolution choice required");
    assert!(
        !req.candidate_object_ids.contains(&bonecrusher_oid),
        "Stomp's instant type does not exist while Bonecrusher Giant is in the library"
    );
    e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("fail to find");
}
