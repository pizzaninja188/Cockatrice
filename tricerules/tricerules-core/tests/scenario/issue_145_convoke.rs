use super::helpers::*;
use tricerules_proto::ruled::v1 as rv1;

fn object_ref(engine: &GameEngine, oid: u32) -> rv1::CostObjectRef {
    rv1::CostObjectRef {
        object_id: oid,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&oid)
            .copied()
            .unwrap_or(0),
    }
}

fn setup(card: &str) -> GameEngine {
    let mut engine = GameEngine::new(
        145002,
        &[0, 1],
        20,
        Some(vec![
            deck_with(
                "island",
                &[
                    card,
                    "grizzly_bears",
                    "merfolk_of_the_pearl_trident",
                    "ornithopter",
                    "merrow_skyswimmer",
                ],
            ),
            deck_with("forest", &["grizzly_bears"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, card);
    engine
}

fn draft(
    engine: &GameEngine,
    card: &str,
    contributions: &[(u32, rv1::ConvokePaymentKind)],
    mana: rv1::SpellPaymentMana,
) -> rv1::CastSpell {
    let hand = hand_index_for_card(engine, 0, card);
    rv1::CastSpell {
        cast_method: rv1::CastMethod::Normal as i32,
        source: Some(hand_cast_source(hand)),
        payment: Some(rv1::SpellPaymentSelection {
            source: Some(object_ref(engine, engine.state.players[0].hand[hand])),
            expected_state_revision: engine.state.command_index,
            convoke: contributions
                .iter()
                .map(|&(oid, kind)| rv1::ConvokeContribution {
                    object: Some(object_ref(engine, oid)),
                    kind: kind as i32,
                })
                .collect(),
            mana: Some(mana),
        }),
        ..Default::default()
    }
}

fn preview(engine: &GameEngine, cast: &rv1::CastSpell) -> rv1::SpellPaymentPreview {
    engine.preview_spell_payment(
        0,
        &rv1::PreviewSpellPayment {
            transaction_id: 13,
            revision: 2,
            cast_spell: Some(cast.clone()),
        },
    )
}

fn commit(
    engine: &mut GameEngine,
    cast: rv1::CastSpell,
) -> Result<rv1::RuledEventBatch, tricerules_core::EngineError> {
    engine.apply_command(
        0,
        &rv1::RuledCommand {
            cmd: Some(Cmd::CastSpell(cast)),
        },
    )
}

#[test]
fn convoke_serialized_command_replays_identically_without_preview_queries() {
    use prost::Message;
    fn run(with_previews: bool) -> (Vec<u8>, Vec<rv1::RuledEventBatch>) {
        let mut engine = setup("unexpected_assistance");
        let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        engine.state.players[0].mana_pool.blue = 2;
        engine.state.players[0].mana_pool.colorless = 2;
        let cast = draft(
            &engine,
            "unexpected_assistance",
            &[(bear, rv1::ConvokePaymentKind::Generic)],
            rv1::SpellPaymentMana {
                u: 2,
                c: 2,
                ..Default::default()
            },
        );
        if with_previews {
            for _ in 0..3 {
                assert!(preview(&engine, &cast).complete);
            }
        }
        let encoded = rv1::RuledCommand {
            cmd: Some(Cmd::CastSpell(cast)),
        }
        .encode_to_vec();
        let replayed = rv1::RuledCommand::decode(encoded.as_slice()).unwrap();
        let batches = vec![
            engine.apply_command(0, &replayed).unwrap(),
            engine.apply_command(0, &pass()).unwrap(),
            engine.apply_command(1, &pass()).unwrap(),
        ];
        (encoded, batches)
    }
    assert_eq!(run(true), run(false));
}

#[test]
fn convoke_preview_is_read_only_and_publishes_only_derived_legal_colors() {
    let mut engine = setup("unexpected_assistance");
    let colorless = relocate_to_battlefield(&mut engine, 0, "ornithopter", false);
    let multicolor = relocate_to_battlefield(&mut engine, 0, "merrow_skyswimmer", false);
    let opponent = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    let cast = draft(&engine, "unexpected_assistance", &[], Default::default());
    let before = format!("{:?}", engine.state);
    let response = preview(&engine, &cast);
    assert!(response.valid, "{}", response.error);
    assert!(!response.complete);
    assert_eq!((response.transaction_id, response.revision), (13, 2));
    assert_eq!(
        response
            .candidates
            .iter()
            .find(|c| c.object.unwrap().object_id == colorless)
            .unwrap()
            .options,
        [6]
    );
    assert_eq!(
        response
            .candidates
            .iter()
            .find(|c| c.object.unwrap().object_id == multicolor)
            .unwrap()
            .options,
        [2, 6]
    );
    assert!(!response
        .candidates
        .iter()
        .any(|c| c.object.unwrap().object_id == opponent));
    assert_eq!(format!("{:?}", engine.state), before);
    assert_eq!(preview(&engine, &cast), response);
    let query = rv1::RuledCommand {
        cmd: Some(Cmd::PreviewSpellPayment(rv1::PreviewSpellPayment {
            cast_spell: Some(cast),
            ..Default::default()
        })),
    };
    assert!(
        engine.apply_command(0, &query).is_err(),
        "queries never enter gameplay/replay"
    );
    assert_eq!(format!("{:?}", engine.state), before);
}

#[test]
fn convoke_stale_duplicate_excess_invalid_color_and_unavailable_mana_are_atomic() {
    for invalid in 0..7 {
        let mut engine = setup("unexpected_assistance");
        let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        engine.state.players[0].mana_pool.blue = 2;
        engine.state.players[0].mana_pool.colorless = 2;
        let mut cast = draft(
            &engine,
            "unexpected_assistance",
            &[(bear, rv1::ConvokePaymentKind::Generic)],
            rv1::SpellPaymentMana {
                u: 2,
                c: 2,
                ..Default::default()
            },
        );
        let selection = cast.payment.as_mut().unwrap();
        match invalid {
            0 => selection.convoke.push(selection.convoke[0]),
            1 => {
                selection.convoke[0]
                    .object
                    .as_mut()
                    .unwrap()
                    .zone_change_generation += 1
            }
            2 => selection.expected_state_revision += 1,
            3 => selection.convoke[0].kind = rv1::ConvokePaymentKind::Blue as i32,
            4 => engine.state.players[0].mana_pool.blue = 1,
            5 => selection.mana.as_mut().unwrap().c = 3,
            _ => engine.state.objects.get_mut(&bear).unwrap().tapped = true,
        }
        let before = format!("{:?}", engine.state);
        assert!(commit(&mut engine, cast).is_err(), "invalid case {invalid}");
        assert_eq!(
            format!("{:?}", engine.state),
            before,
            "atomic case {invalid}"
        );
    }
}

#[test]
fn convoke_refresh_keeps_valid_choices_and_removes_a_mana_ability_creature() {
    let mut engine = setup("unexpected_assistance");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let fish = relocate_to_battlefield(&mut engine, 0, "merfolk_of_the_pearl_trident", false);
    let mut cast = draft(
        &engine,
        "unexpected_assistance",
        &[
            (bear, rv1::ConvokePaymentKind::Generic),
            (fish, rv1::ConvokePaymentKind::Blue),
        ],
        Default::default(),
    );
    let first = preview(&engine, &cast);
    assert!(first.valid && !first.selection_changed);
    cast.payment = first.selection;
    engine.state.objects.get_mut(&bear).unwrap().tapped = true;
    engine.state.command_index += 1;
    let refreshed = preview(&engine, &cast);
    assert!(refreshed.valid && refreshed.selection_changed);
    let selection = refreshed.selection.unwrap();
    assert_eq!(selection.convoke.len(), 1);
    assert_eq!(selection.convoke[0].object.unwrap().object_id, fish);
    assert_eq!(
        selection.expected_state_revision,
        engine.state.command_index
    );
}

#[test]
fn convoke_preview_removes_only_excess_mana_and_retains_creature_selections() {
    let mut engine = setup("unexpected_assistance");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.players[0].mana_pool.blue = 2;
    engine.state.players[0].mana_pool.colorless = 3;
    let cast = draft(
        &engine,
        "unexpected_assistance",
        &[(bear, rv1::ConvokePaymentKind::Generic)],
        rv1::SpellPaymentMana {
            u: 2,
            c: 3,
            ..Default::default()
        },
    );
    let response = preview(&engine, &cast);
    assert!(response.valid && response.selection_changed);
    let selection = response.selection.unwrap();
    assert_eq!(selection.convoke.len(), 1);
    assert_eq!(selection.mana.unwrap().c, 2);
    assert_eq!(selection.mana.unwrap().u, 2);
}

#[test]
fn convoke_all_mana_resolves_assistance_with_private_discard() {
    let mut engine = setup("unexpected_assistance");
    engine.state.players[0].mana_pool.blue = 2;
    engine.state.players[0].mana_pool.colorless = 3;
    let cast = draft(
        &engine,
        "unexpected_assistance",
        &[],
        rv1::SpellPaymentMana {
            u: 2,
            c: 3,
            ..Default::default()
        },
    );
    assert!(preview(&engine, &cast).complete);
    commit(&mut engine, cast).unwrap();
    let hand_before = engine.state.players[0].hand.len();
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    let choice = find_resolution_choice(&batch).expect("draw then private discard");
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 3);
    assert_eq!(choice.deciding_player_id, 0);
    engine
        .apply_command(
            0,
            &submit_resolution_choice(vec![choice.candidate_object_ids[0]]),
        )
        .unwrap();
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 2);
}

#[test]
fn convoke_combines_restricted_mana_and_refreshes_unavailable_group_amounts() {
    use tricerules_cards::{
        primitives::CardTypeFilter, ManaAmount, ManaSpendFilter, ManaSpendingRestriction,
    };
    use tricerules_core::state::RestrictedManaContribution;
    let mut engine = setup("unexpected_assistance");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine
        .state
        .mana_restrictions
        .push(ManaSpendingRestriction {
            label: "Spend only on an instant or sorcery".into(),
            cast_spell: vec![ManaSpendFilter {
                card_type: Some(CardTypeFilter::InstantOrSorcery),
                subtype: None,
            }],
            activate_ability: vec![],
        });
    engine.state.players[0]
        .restricted_mana
        .push(RestrictedManaContribution {
            restriction_group_id: 1,
            amount: ManaAmount {
                c: 1,
                ..Default::default()
            },
        });
    engine.state.players[0].mana_pool.blue = 2;
    engine.state.players[0].mana_pool.colorless = 1;
    let mut cast = draft(
        &engine,
        "unexpected_assistance",
        &[(bear, rv1::ConvokePaymentKind::Generic)],
        rv1::SpellPaymentMana {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    cast.restricted_mana.push(rv1::ManaSpendSelection {
        restriction_group_id: 1,
        c: 2,
        ..Default::default()
    });
    let response = preview(&engine, &cast);
    assert!(
        response.valid && response.complete && response.selection_changed,
        "{response:?}"
    );
    assert_eq!(response.restricted_mana[0].c, 1);
    cast.payment = response.selection;
    cast.restricted_mana = response.restricted_mana;
    let confirmed = preview(&engine, &cast);
    assert!(confirmed.complete && !confirmed.selection_changed);
    commit(&mut engine, cast).unwrap();
    assert!(engine.state.objects[&bear].tapped);
    assert!(engine.state.players[0].restricted_mana.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
}

#[test]
fn convoke_hybrid_skyswimmer_creates_a_white_blue_token() {
    let mut engine = setup("merrow_skyswimmer");
    let fish = relocate_to_battlefield(&mut engine, 0, "merfolk_of_the_pearl_trident", false);
    engine.state.players[0].mana_pool.white = 1;
    engine.state.players[0].mana_pool.colorless = 3;
    let cast = draft(
        &engine,
        "merrow_skyswimmer",
        &[(fish, rv1::ConvokePaymentKind::Blue)],
        rv1::SpellPaymentMana {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    assert!(preview(&engine, &cast).complete);
    commit(&mut engine, cast).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    let token = engine.state.players[0]
        .battlefield
        .iter()
        .find(|&&oid| engine.state.objects[&oid].card_id == "merfolk_wu_1_1")
        .expect("token");
    let chars = engine.characteristics(*token).unwrap();
    assert_eq!(chars.colors.len(), 2);
    assert!(chars.colors.contains(&tricerules_cards::Color::White));
    assert!(chars.colors.contains(&tricerules_cards::Color::Blue));
}

#[test]
fn convoke_appeal_accepts_one_or_two_targets_and_control_not_ownership() {
    for target_count in 1..=2 {
        let mut engine = setup("appeal_to_eirdu");
        let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
        let fish = relocate_to_battlefield(&mut engine, 0, "merfolk_of_the_pearl_trident", false);
        engine.state.objects.get_mut(&bear).unwrap().owner = 1;
        engine.state.players[0].mana_pool.white = 1;
        engine.state.players[0].mana_pool.colorless = 2;
        let mut cast = draft(
            &engine,
            "appeal_to_eirdu",
            &[(bear, rv1::ConvokePaymentKind::Generic)],
            rv1::SpellPaymentMana {
                w: 1,
                c: 2,
                ..Default::default()
            },
        );
        cast.targets = [bear, fish]
            .into_iter()
            .take(target_count)
            .map(|oid| TargetRef {
                object_id: oid,
                ..Default::default()
            })
            .collect();
        assert!(preview(&engine, &cast).complete);
        commit(&mut engine, cast).unwrap();
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.characteristics(bear).unwrap().power, Some(4));
        assert_eq!(engine.characteristics(bear).unwrap().toughness, Some(3));
        assert_eq!(
            engine.characteristics(fish).unwrap().power,
            Some(if target_count == 2 { 3 } else { 1 })
        );
    }
}

#[test]
fn convoke_all_creature_celebrant_resolves_with_vigilance() {
    let mut engine = setup("sun-dappled_celebrant");
    engine.enable_dev_commands();
    let mut creatures = Vec::new();
    for _ in 0..6 {
        let command = rv1::RuledCommand {
            cmd: Some(Cmd::DevCommand(rv1::DevCommand {
                target_player_id: 0,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: "Serra Angel".into(),
                        zone: rv1::DevZone::Battlefield as i32,
                        ready: false,
                    },
                )),
            })),
        };
        engine.apply_command(0, &command).unwrap();
        creatures.push(*engine.state.players[0].battlefield.last().unwrap());
    }
    let contributions = creatures
        .iter()
        .enumerate()
        .map(|(i, &oid)| {
            (
                oid,
                if i < 2 {
                    rv1::ConvokePaymentKind::White
                } else {
                    rv1::ConvokePaymentKind::Generic
                },
            )
        })
        .collect::<Vec<_>>();
    let cast = draft(
        &engine,
        "sun-dappled_celebrant",
        &contributions,
        Default::default(),
    );
    assert!(preview(&engine, &cast).complete);
    commit(&mut engine, cast).unwrap();
    for oid in creatures {
        assert!(engine.state.objects[&oid].tapped);
    }
    resolve_entire_stack_two_player(&mut engine);
    let celebrant = engine.state.players[0]
        .battlefield
        .iter()
        .find(|&&oid| engine.state.objects[&oid].card_id == "sun-dappled_celebrant")
        .unwrap();
    let chars = engine.characteristics(*celebrant).unwrap();
    assert_eq!((chars.power, chars.toughness), (Some(5), Some(6)));
    assert!(chars.has_keyword(tricerules_cards::Keyword::Vigilance));
}

#[test]
fn convoke_tapping_an_attacker_or_blocker_does_not_remove_it_from_combat() {
    for pay_with_blocker in [false, true] {
        let mut engine = setup("unexpected_assistance");
        let attacker = relocate_to_battlefield(&mut engine, 0, "merrow_skyswimmer", false);
        let blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
        engine
            .state
            .objects
            .get_mut(&attacker)
            .unwrap()
            .summoning_sick = false;
        engine.state.objects.get_mut(&blocker).unwrap().card_id = "serra_angel".into();
        engine.apply_command(0, &primitive_yield()).unwrap();
        engine.apply_command(0, &pass()).unwrap();
        engine.apply_command(1, &pass()).unwrap();
        engine
            .apply_command(0, &declare_attackers(vec![attacker]))
            .unwrap();
        engine.apply_command(0, &pass()).unwrap();
        engine.apply_command(1, &pass()).unwrap();
        engine
            .apply_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker,
                }]),
            )
            .unwrap();
        let mut cast = draft(
            &engine,
            "unexpected_assistance",
            &[],
            rv1::SpellPaymentMana {
                u: 2,
                c: 2,
                ..Default::default()
            },
        );
        let payer = if pay_with_blocker { 1 } else { 0 };
        let oid = if pay_with_blocker { blocker } else { attacker };
        if pay_with_blocker {
            let source = cast.payment.as_ref().unwrap().source.unwrap().object_id;
            engine.state.players[0].hand.retain(|&id| id != source);
            engine.state.players[1].hand.push(source);
            engine.state.objects.get_mut(&source).unwrap().owner = 1;
            cast.source = Some(hand_cast_source(engine.state.players[1].hand.len() - 1));
            engine.apply_command(0, &pass()).unwrap();
        }
        cast.payment.as_mut().unwrap().expected_state_revision = engine.state.command_index;
        cast.payment
            .as_mut()
            .unwrap()
            .convoke
            .push(rv1::ConvokeContribution {
                object: Some(object_ref(&engine, oid)),
                kind: rv1::ConvokePaymentKind::Generic as i32,
            });
        engine.state.players[payer].mana_pool.blue = 2;
        engine.state.players[payer].mana_pool.colorless = 2;
        engine
            .apply_command(
                payer as i32,
                &rv1::RuledCommand {
                    cmd: Some(Cmd::CastSpell(cast)),
                },
            )
            .unwrap();
        assert!(engine.state.objects[&oid].tapped);
        let combat = engine.state.combat.as_ref().unwrap();
        assert!(combat.attacking.contains(&attacker));
        assert_eq!(combat.blockers[&attacker], [blocker]);
    }
}

#[test]
fn convoke_mixed_payment_taps_a_summoning_sick_creature_without_making_mana() {
    let mut engine = GameEngine::new(
        145001,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["unexpected_assistance", "grizzly_bears"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "unexpected_assistance");
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.objects.get_mut(&bear).unwrap().summoning_sick = true;
    engine.state.players[0].mana_pool.clear();
    engine.state.players[0].mana_pool.blue = 2;
    engine.state.players[0].mana_pool.colorless = 2;
    let hand = hand_index_for_card(&engine, 0, "unexpected_assistance");
    let source = engine.state.players[0].hand[hand];
    let mut command = cast_spell(hand, vec![]);
    let Some(rv1::ruled_command::Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        panic!("cast")
    };
    cast.payment = Some(rv1::SpellPaymentSelection {
        source: Some(object_ref(&engine, source)),
        expected_state_revision: engine.state.command_index,
        convoke: vec![rv1::ConvokeContribution {
            object: Some(object_ref(&engine, bear)),
            kind: rv1::ConvokePaymentKind::Generic as i32,
        }],
        mana: Some(rv1::SpellPaymentMana {
            u: 2,
            c: 2,
            ..Default::default()
        }),
    });
    engine
        .apply_command(0, &command)
        .expect("one creature pays the missing generic unit");
    assert!(engine.state.objects[&bear].tapped);
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.stack.last().unwrap().id, source);
}
