use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

fn counter_count(engine: &GameEngine, object_id: u32) -> u32 {
    engine.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne)
}

fn engine_with_wan(seed: u64, controller: usize, search_cards: &[&str]) -> (GameEngine, u32) {
    let decks = if controller == 0 {
        let mut p0_specials = vec!["wan_shi_tong,_librarian"];
        p0_specials.extend_from_slice(search_cards);
        Some(vec![
            deck_with("island", &p0_specials),
            deck_with("swamp", &[]),
        ])
    } else {
        Some(vec![
            deck_with("swamp", search_cards),
            deck_with("island", &["wan_shi_tong,_librarian"]),
        ])
    };
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let wan = move_ready_to_battlefield(&mut engine, controller, "wan_shi_tong,_librarian");
    assert_eq!(engine.state.stack.len(), 1, "Wan's X=0 ETB trigger");
    pass_both_players(&mut engine);
    assert!(engine.state.stack.is_empty());
    (engine, wan)
}

fn select_search_zones(branch_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
            decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: branch_index,
            ..Default::default()
        })),
    }
}

fn activate_say_its_name_search(
    engine: &GameEngine,
    source: u32,
    other_copies: [u32; 2],
) -> RuledCommand {
    let object_ref = |object_id| rv1::CostObjectRef {
        object_id,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
    };
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(rv1::ActivateAbility {
            source_object_id: source,
            ability_index: 0,
            cost_selections: vec![rv1::CostSelection {
                cost_index: 1,
                selection: Some(rv1::cost_selection::Selection::GraveyardObjects(
                    rv1::CostObjectRefs {
                        objects: other_copies.into_iter().map(object_ref).collect(),
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
    }
}

#[test]
fn wan_etb_retains_cast_x_until_its_trigger_resolves() {
    let mut engine = GameEngine::new(
        208_001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["wan_shi_tong,_librarian"]),
            deck_with("swamp", &[]),
        ]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "wan_shi_tong,_librarian");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 5,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "wan_shi_tong,_librarian");
    let wan = engine.state.players[0].hand[hand_index];
    engine
        .apply_command(0, &cast_spell_x(hand_index, Vec::new(), 5))
        .expect("cast Wan with X=5");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&wan].zone, Zone::Battlefield);
    assert_eq!(counter_count(&engine, wan), 0, "ETB is not a replacement");
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is waiting");
    assert_eq!(
        engine.state.stack[0].chosen_x, 5,
        "trigger snapshots cast X"
    );
    let hand_before_trigger = engine.state.players[0].hand.len();

    pass_both_players(&mut engine);
    assert_eq!(counter_count(&engine, wan), 5);
    assert_eq!(engine.state.players[0].hand.len(), hand_before_trigger + 2);
}

#[test]
fn wan_etb_draw_keeps_x_if_the_source_leaves_before_resolution() {
    let mut engine = GameEngine::new(
        208_002,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["wan_shi_tong,_librarian"]),
            deck_with("swamp", &[]),
        ]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "wan_shi_tong,_librarian");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 5,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "wan_shi_tong,_librarian");
    let wan = engine.state.players[0].hand[hand_index];
    engine
        .apply_command(0, &cast_spell_x(hand_index, Vec::new(), 5))
        .expect("cast Wan with X=5");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack[0].chosen_x, 5);

    engine.state.players[0]
        .battlefield
        .retain(|object| *object != wan);
    engine.state.players[0].graveyard.push(wan);
    engine.state.objects.get_mut(&wan).unwrap().zone = Zone::Graveyard;
    *engine.state.zone_change_generation.entry(wan).or_default() += 1;
    let hand_before_trigger = engine.state.players[0].hand.len();

    pass_both_players(&mut engine);
    assert_eq!(
        counter_count(&engine, wan),
        0,
        "departed source gets no counters"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before_trigger + 2);
}

#[test]
fn opponent_own_library_search_triggers_once_including_failure_to_find() {
    for (seed, tutor, mana) in [
        (
            208_010,
            "demonic_tutor",
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            208_011,
            "mystical_tutor",
            ManaGift {
                u: 1,
                ..Default::default()
            },
        ),
    ] {
        let (mut engine, wan) = engine_with_wan(seed, 1, &[tutor]);
        ensure_card_in_hand(&mut engine, 0, tutor);
        give_mana(&mut engine, 0, mana);
        let tutor_index = hand_index_for_card(&engine, 0, tutor);
        engine
            .apply_command(0, &cast_spell(tutor_index, Vec::new()))
            .expect("cast tutor");
        pass_both_players(&mut engine);
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("search choice");
        let chosen = if tutor == "demonic_tutor" {
            vec![pending.presentation.candidates[0]]
        } else {
            Vec::new()
        };
        let opponent_hand_before = engine.state.players[1].hand.len();
        engine
            .apply_command(0, &submit_resolution_choice(chosen))
            .expect("complete search");

        assert_eq!(engine.state.stack.len(), 1, "one search trigger");
        assert_eq!(counter_count(&engine, wan), 0, "trigger has not resolved");
        pass_both_players(&mut engine);
        assert_eq!(counter_count(&engine, wan), 1);
        assert_eq!(engine.state.players[1].hand.len(), opponent_hand_before + 1);
    }
}

#[test]
fn controller_search_does_not_trigger_wan() {
    let (mut engine, wan) = engine_with_wan(208_020, 0, &["demonic_tutor"]);
    ensure_card_in_hand(&mut engine, 0, "demonic_tutor");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let tutor_index = hand_index_for_card(&engine, 0, "demonic_tutor");
    engine
        .apply_command(0, &cast_spell(tutor_index, Vec::new()))
        .expect("cast tutor");
    pass_both_players(&mut engine);
    let chosen = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
        .expect("complete own search");
    assert!(engine.state.stack.is_empty());
    assert_eq!(counter_count(&engine, wan), 0);
}

#[test]
fn gifts_search_trigger_waits_until_the_whole_spell_finishes() {
    let (mut engine, wan) = engine_with_wan(208_030, 1, &["gifts_ungiven"]);
    ensure_card_in_hand(&mut engine, 0, "gifts_ungiven");
    let revealed = [
        inject_library_card(&mut engine, 0, "mountain"),
        inject_library_card(&mut engine, 0, "island"),
        inject_library_card(&mut engine, 0, "grizzly_bears"),
        inject_library_card(&mut engine, 0, "hill_giant"),
    ];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let gifts = hand_index_for_card(&engine, 0, "gifts_ungiven");
    engine
        .apply_command(0, &cast_spell(gifts, Vec::new()))
        .expect("cast Gifts Ungiven");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(revealed.to_vec()))
        .expect("complete the library-search portion");

    assert!(
        engine.state.stack.is_empty(),
        "trigger is staged during Gifts"
    );
    assert!(
        engine.state.pending_resolution.is_some(),
        "opponent still chooses"
    );
    assert_eq!(engine.state.staged_trigger_groups.len(), 1);
    engine
        .apply_command(1, &submit_resolution_choice(revealed[..2].to_vec()))
        .expect("finish Gifts");

    assert_eq!(
        engine.state.stack.len(),
        1,
        "staged search trigger is now on stack"
    );
    let hand_before_trigger = engine.state.players[1].hand.len();
    pass_both_players(&mut engine);
    assert_eq!(counter_count(&engine, wan), 1);
    assert_eq!(engine.state.players[1].hand.len(), hand_before_trigger + 1);
}

#[test]
fn multi_zone_search_triggers_only_when_library_is_in_the_chosen_scope() {
    for (seed, branch_index, include_library) in [(208_040, 0, false), (208_041, 2, true)] {
        let (mut engine, wan) = engine_with_wan(seed, 1, &[]);
        let source = inject_graveyard_card(&mut engine, 0, "say_its_name");
        let costs = [
            inject_graveyard_card(&mut engine, 0, "say_its_name"),
            inject_graveyard_card(&mut engine, 0, "say_its_name"),
        ];
        let altanak = if include_library {
            inject_library_card(&mut engine, 0, "altanak,_the_thrice-called")
        } else {
            inject_card_into_hand(&mut engine, 0, "altanak,_the_thrice-called")
        };
        engine
            .apply_command(0, &activate_say_its_name_search(&engine, source, costs))
            .expect("activate Say Its Name from the graveyard");
        pass_both_players(&mut engine);
        engine
            .apply_command(0, &select_search_zones(branch_index))
            .expect("choose search zones");
        engine
            .apply_command(
                0,
                &submit_resolution_choice(if include_library {
                    vec![altanak]
                } else {
                    Vec::new()
                }),
            )
            .expect("finish multi-zone search");

        if include_library {
            assert_eq!(engine.state.stack.len(), 1, "library scope triggers Wan");
            pass_both_players(&mut engine);
            assert_eq!(counter_count(&engine, wan), 1);
        } else {
            assert!(
                engine.state.stack.is_empty(),
                "hand-only scope is not a library search"
            );
            assert_eq!(counter_count(&engine, wan), 0);
        }
    }
}
