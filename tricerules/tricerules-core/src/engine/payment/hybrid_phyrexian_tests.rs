//! Prepared payment probes, not Ajani or Tamiyo whole-card admission.
use super::*;

fn payment_engine() -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        13_110_301,
        &[0, 1],
        20,
        None,
        true,
    )
    .unwrap();
    engine.state.players[0].mana_pool.green = 2;
    engine.state.players[0].mana_pool.white = 2;
    engine.state.players[0].mana_pool.colorless = 1;
    engine
}
fn choice(color: rv1::ManaColor, life: bool) -> rv1::FlexPipPayment {
    rv1::FlexPipPayment {
        pip_index: 2,
        pay_life: life,
        mana_color: color as i32,
    }
}
#[test]
fn ajani_hybrid_choice_locks_exact_green_white_or_life_demands_and_committed_debits() {
    let cost = ManaCost::parse("{1}{G}{G/W/P}{W}").unwrap();
    for (selection, w, g, life) in [
        (choice(rv1::ManaColor::Green, false), 1, 2, 0),
        (choice(rv1::ManaColor::White, false), 2, 1, 0),
        (choice(rv1::ManaColor::Unspecified, true), 1, 1, 2),
    ] {
        let mut e = payment_engine();
        let demands = demand::normalize(&cost, 0, 0, 0, &[selection]).unwrap();
        assert_eq!(demands.len(), 1);
        assert_eq!(demands[0].amounts, [w, 0, 0, 0, g, 0, 1]);
        assert_eq!(demands[0].life, life);
        let plan = mana::plan_mana_payment_with_any_color(
            &e.state,
            0,
            &cost,
            0,
            0,
            0,
            &[selection],
            &[],
            &[],
            false,
        )
        .unwrap();
        assert_eq!(plan.phyrexian_symbols_paid_with_life, life / 2);
        assert_eq!(e.state.players[0].life, 20);
        mana::commit_mana_payment(&mut e.state, 0, plan);
        assert_eq!(e.state.players[0].life, 20 - life as i32);
        assert_eq!(e.state.players[0].mana_pool.white, 2 - w);
        assert_eq!(e.state.players[0].mana_pool.green, 2 - g);
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    }
}

#[test]
fn hybrid_choices_reject_missing_duplicate_wrong_color_and_conflicting_fields_without_state_change()
{
    let e = payment_engine();
    let cost = ManaCost::parse("{1}{G}{G/W/P}{W}").unwrap();
    let before = e.diagnostic_snapshot().unwrap();
    let good = choice(rv1::ManaColor::Green, false);
    let mut bad_index = good;
    bad_index.pip_index = 999;
    let mut wrong_pip = good;
    wrong_pip.pip_index = 0;
    let mut unknown = good;
    unknown.mana_color = 99;
    for choices in [
        vec![],
        vec![choice(rv1::ManaColor::Unspecified, false)],
        vec![good, choice(rv1::ManaColor::White, false)],
        vec![good, good],
        vec![choice(rv1::ManaColor::Blue, false)],
        vec![choice(rv1::ManaColor::Green, true)],
        vec![bad_index],
        vec![wrong_pip],
        vec![unknown],
    ] {
        assert!(
            demand::normalize(&cost, 0, 0, 0, &choices).is_err(),
            "{choices:?}"
        );
        assert!(
            mana::plan_mana_payment_with_any_color(
                &e.state,
                0,
                &cost,
                0,
                0,
                0,
                &choices,
                &[],
                &[],
                false
            )
            .is_err(),
            "{choices:?}"
        );
        assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    }
    let mono = ManaCost::parse("{G/P}").unwrap();
    for choices in [
        vec![],
        vec![rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: false,
            mana_color: 0,
        }],
    ] {
        let demands = demand::normalize(&mono, 0, 0, 0, &choices).unwrap();
        assert_eq!(demands[0].amounts, [0, 0, 0, 0, 1, 0, 0]);
        assert_eq!(demands[0].life, 0);
    }
    assert!(demand::normalize(
        &mono,
        0,
        0,
        0,
        &[rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: false,
            mana_color: rv1::ManaColor::Green as i32
        }]
    )
    .is_err());
}

#[test]
fn hybrid_choice_keeps_pip_index_under_x_reduction_and_rejects_removed_alternative_cost_pips() {
    let cost = ManaCost::parse("{X}{G}{G/W/P}{W}").unwrap();
    let selected = choice(rv1::ManaColor::White, false);
    let demand = demand::normalize(&cost, 3, 2, 4, &[selected]).unwrap();
    assert_eq!(demand[0].amounts, [2, 0, 0, 0, 1, 0, 1]);
    assert_eq!(demand[0].life, 0);
    for alternate in [ManaCost::default(), ManaCost::parse("{G}{W}").unwrap()] {
        assert!(demand::normalize(&alternate, 0, 0, 0, &[selected]).is_err());
        assert!(demand::normalize(&alternate, 0, 0, 0, &[]).is_ok());
    }
    let reduced = apply_activated_mana_reduction(
        &ManaCost::parse("{1}{G}{G/W/P}{W}").unwrap(),
        0,
        ActivatedManaReduction {
            generic: 1,
            ..Default::default()
        },
    )
    .unwrap()
    .0;
    assert_eq!(reduced.to_string(), "{0}{G}{G/W/P}{W}");
    assert!(demand::normalize(&reduced, 0, 0, 0, &[selected]).is_ok());
}

#[test]
fn hybrid_chosen_color_needs_as_any_color_permission_and_insufficient_life_stays_unpayable() {
    let mut e = payment_engine();
    let cost = ManaCost::parse("{G/W/P}").unwrap();
    let white = rv1::FlexPipPayment {
        pip_index: 0,
        pay_life: false,
        mana_color: rv1::ManaColor::White as i32,
    };
    e.state.players[0].mana_pool.white = 0;
    e.state.players[0].mana_pool.colorless = 0;
    e.state.players[0].mana_pool.green = 1;
    assert!(mana::plan_mana_payment_with_any_color(
        &e.state,
        0,
        &cost,
        0,
        0,
        0,
        &[white],
        &[],
        &[],
        false
    )
    .is_err());
    let plan = mana::plan_mana_payment_with_any_color(
        &e.state,
        0,
        &cost,
        0,
        0,
        0,
        &[white],
        &[],
        &[],
        true,
    )
    .unwrap();
    // The transaction layer records the permission dependency; this low-level planner
    // validates the exact announced color and actual pool debit.
    assert_eq!(plan.unrestricted_spent().g, 1);
    assert_eq!(plan.phyrexian_symbols_paid_with_life, 0);
    e.state.players[0].life = 1;
    let life = rv1::FlexPipPayment {
        pip_index: 0,
        pay_life: true,
        mana_color: 0,
    };
    assert!(mana::plan_mana_payment_with_any_color(
        &e.state,
        0,
        &cost,
        0,
        0,
        0,
        &[life],
        &[],
        &[],
        false
    )
    .is_err());
}

#[test]
fn hybrid_resolution_payment_offers_mana_or_life_but_requires_an_explicit_branch() {
    let e = payment_engine();
    let cost = ManaCost::parse("{G/W/P}").unwrap();
    assert!(e.can_pay_resolution_mana(0, &cost));
    let payment = crate::state::PendingManaPayment::from_cost(0, cost, 0);
    assert!(e
        .prepare_resolution_payment_costs(0, &payment, &[], &[])
        .is_err());
    assert!(e.can_pay_resolution_mana(0, &ManaCost::parse("{G/P}").unwrap()));
}

const HYBRID_PLANESWALKER: &str = r#"(id:"hybrid_compleated_probe",name:"Hybrid Compleated Probe",face_id:"hybrid_compleated_probe",
    mana_cost:"{1}{G}{G/W/P}{W}",supertypes:["Legendary"],types:["Planeswalker","Ajani"],loyalty:Some(4),
    static_abilities:[(ability_id:"compleated",presentation:Fallback,definition:Compleated)])"#;
fn cast_fixture(extra: &[&str]) -> GameEngine {
    let mut chunks = vec![HYBRID_PLANESWALKER];
    chunks.extend(extra);
    let registry = CardRegistry::from_chunks_and_tokens(&chunks, &[]).unwrap();
    let mut e = GameEngine::new_with_registry(
        13_110_302,
        &[0, 1],
        20,
        Some(vec![
            EngineDeck {
                mainboard: vec!["hybrid_compleated_probe".into(); 40],
                commanders: vec![]
            };
            2
        ]),
        true,
        Box::leak(Box::new(registry)),
    )
    .unwrap();
    e.state.turn_step = TurnStep::Main1;
    e.state.priority_idx = 0;
    e.state.players[0].mana_pool.white = 2;
    e.state.players[0].mana_pool.green = 2;
    e.state.players[0].mana_pool.colorless = 1;
    e
}
fn begin_cast(e: &mut GameEngine, selection: rv1::FlexPipPayment) -> u64 {
    let oid = e.state.players[0].hand[0];
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
            rv1::BeginSpellCast {
                announcement: Some(rv1::SpellCastAnnouncement {
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(0)),
                        expected_zone_change_generation: Some(
                            e.payment_object_ref(oid).zone_change_generation,
                        ),
                    }),
                    cast_method: rv1::CastMethod::Normal as i32,
                    flex_payments: vec![selection],
                    ..Default::default()
                }),
            },
        )),
    };
    // Exercise the actual typed command's wire serialization, not a reconstructed choice.
    let decoded = <RuledCommand as prost::Message>::decode(
        prost::Message::encode_to_vec(&command).as_slice(),
    )
    .unwrap();
    e.apply_command(0, &decoded).unwrap();
    let pending = e.state.pending_spell_cast.as_ref().unwrap();
    assert_eq!(pending.announcement.flex_payments, vec![selection]);
    pending.transaction_id
}
#[test]
fn ajani_shaped_staged_cast_preserves_branch_through_preview_cancel_retry_and_compleated_entry() {
    for (selection, expected_loyalty, w, g) in [
        (choice(rv1::ManaColor::Green, false), 4, 1, 2),
        (choice(rv1::ManaColor::White, false), 4, 2, 1),
        (choice(rv1::ManaColor::Unspecified, true), 2, 1, 1),
    ] {
        let mut e = cast_fixture(&[]);
        let original_hand = e.state.players[0].hand.clone();
        let source = original_hand[0];
        let transaction = begin_cast(&mut e, selection);
        assert_eq!(e.state.players[0].life, 20);
        assert_eq!(e.state.players[0].mana_pool.green, 2);
        let commit = rv1::CommitSpellCast {
            transaction_id: transaction,
            ..Default::default()
        };
        let before = e.diagnostic_snapshot().unwrap();
        let preview = e.preview_payment(
            0,
            &rv1::PreviewPayment {
                transaction_id: transaction,
                commit_spell_cast: Some(commit.clone()),
                ..Default::default()
            },
        );
        assert!(preview.error.is_empty(), "{}", preview.error);
        assert_eq!(e.diagnostic_snapshot().unwrap(), before);
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                    rv1::CancelSpellCast {
                        transaction_id: transaction,
                    },
                )),
            },
        )
        .unwrap();
        assert_eq!(e.state.players[0].hand, original_hand);
        assert_eq!(e.state.players[0].life, 20);
        assert_eq!(e.state.players[0].mana_pool.white, 2);
        let retry = begin_cast(&mut e, selection);
        assert_ne!(retry, transaction);
        let before = e.diagnostic_snapshot().unwrap();
        assert!(e
            .apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(commit))
                }
            )
            .is_err());
        assert_eq!(e.diagnostic_snapshot().unwrap(), before);
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id: retry,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
        assert_eq!(e.state.stack.len(), 1);
        assert_eq!(
            e.state.stack[0].phyrexian_symbols_paid_with_life,
            u32::from(selection.pay_life)
        );
        assert_eq!(
            e.state.players[0].life,
            if selection.pay_life { 18 } else { 20 }
        );
        assert_eq!(e.state.players[0].mana_pool.white, 2 - w);
        assert_eq!(e.state.players[0].mana_pool.green, 2 - g);
        for _ in 0..2 {
            let actor = e.state.priority_player_id();
            e.apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
        }
        assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(
            e.state.objects[&source]
                .counters
                .get(&CounterKind::Loyalty)
                .copied()
                .unwrap_or(0),
            expected_loyalty
        );
        assert_eq!(e.characteristics(source).unwrap().mana_value, 4);
    }
}

#[test]
fn hybrid_staged_cast_rechecks_live_any_color_permission_without_changing_the_announced_white_branch(
) {
    const LATTICE: &str = r#"(id:"hybrid_lattice_probe",name:"Hybrid Lattice Probe",face_id:"hybrid_lattice_probe",types:["Artifact"],
        static_abilities:[(ability_id:"spend",presentation:Fallback,definition:AllPlayersMaySpendManaAsThoughAnyColor)])"#;
    let mut e = cast_fixture(&[LATTICE]);
    e.state.players[0].mana_pool.white = 0;
    e.state.players[0].mana_pool.green = 4;
    e.state.players[0].mana_pool.colorless = 0;
    let lattice = e.state.next_object_id;
    e.state.next_object_id += 1;
    let object = new_object_from_card(
        lattice,
        0,
        "hybrid_lattice_probe",
        Zone::Battlefield,
        e.registry
            .get("hybrid_lattice_probe")
            .unwrap()
            .primary_face(),
    );
    e.state.objects.insert(lattice, object);
    e.state.zone_change_generation.insert(lattice, 0);
    e.state.players[0].battlefield.push(lattice);
    let selected = choice(rv1::ManaColor::White, false);
    let transaction = begin_cast(&mut e, selected);
    let commit = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
            rv1::CommitSpellCast {
                transaction_id: transaction,
                ..Default::default()
            },
        )),
    };
    let before = e.diagnostic_snapshot().unwrap();
    assert!(e.apply_command(1, &commit).is_err());
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    super::super::resolution::move_object_to_zone(
        &mut e.state,
        e.registry,
        lattice,
        Zone::Graveyard,
        None,
    )
    .unwrap();
    let before = e.diagnostic_snapshot().unwrap();
    assert!(
        e.apply_command(0, &commit).is_err(),
        "Green cannot silently replace the locked White branch after permission disappears"
    );
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    assert_eq!(
        e.state
            .pending_spell_cast
            .as_ref()
            .unwrap()
            .announcement
            .flex_payments,
        vec![selected]
    );
    super::super::resolution::move_object_to_zone(
        &mut e.state,
        e.registry,
        lattice,
        Zone::Battlefield,
        None,
    )
    .unwrap();
    e.apply_command(0, &commit).unwrap();
    assert_eq!(e.state.players[0].mana_pool.green, 0);
    assert_eq!(e.state.stack[0].phyrexian_symbols_paid_with_life, 0);
}

fn put_probe(
    e: &mut GameEngine,
    card: &str,
    zone: Zone,
    owner: PlayerId,
    tapped: bool,
) -> ObjectId {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let mut object = new_object_from_card(
        id,
        owner,
        card,
        zone,
        e.registry.get(card).unwrap().primary_face(),
    );
    object.tapped = tapped;
    e.state.objects.insert(id, object);
    e.state.zone_change_generation.insert(id, 0);
    let player = &mut e.state.players[owner as usize];
    match zone {
        Zone::Hand => player.hand.push(id),
        Zone::Battlefield => player.battlefield.push(id),
        _ => panic!("fixture zone"),
    }
    e.reconcile_activated_ability_slots();
    id
}
const HYBRID_DEFERRED_OPPONENT: &str = r#"(id:"hybrid_opponent_probe",name:"Hybrid Opponent Probe",face_id:"hybrid_opponent_probe",types:["Artifact"],
 activated_abilities:[(ability_id:"activate",presentation:Fallback,costs:[Mana("{G/W/P}"),Tap],
 effect:[Tap(subject:Chosen((kind:Creature,controller:You))),Tap(subject:Chosen((kind:Creature,controller:Opponent))),
 Fight(first:Chosen((kind:Creature,controller:You)),second:Chosen((kind:Creature,controller:Opponent)))],
 targeting:Some((groups:[(min:1,max:1,prompt:"Own",effect_indices:[0,2]),
 (min:1,max:1,prompt:"Opponent",effect_indices:[1,2],chooser:ChosenOpponent)])))])"#;
const HYBRID_HAND_RETURN: &str = r#"(id:"hybrid_hand_return_probe",name:"Hybrid Hand Return Probe",face_id:"hybrid_hand_return_probe",types:["Land"],
 activated_abilities:[(ability_id:"activate",presentation:Fallback,source_zone:Hand,timing:SorcerySpeed,
 costs:[Mana("{G/W/P}"),ReturnTappedCreature],effect:[PutAbilitySourceOntoBattlefield])])"#;
const HYBRID_PAYMENT_MANA: &str = r#"(id:"hybrid_payment_mana",name:"Hybrid Payment Mana",face_id:"hybrid_payment_mana",types:["Land"],
 activated_abilities:[(ability_id:"mana",presentation:Fallback,costs:[Tap],effect:[ProduceMana(options:[(c:1)]),DamagePlayer(amount:1,who:Controller)])])"#;

fn hybrid_nested_mana(e: &mut GameEngine, source: ObjectId) {
    e.state.add_damage_prevention_shield(0, 1);
    e.state.add_damage_prevention_shield(0, 1);
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                rv1::ActivateAbility {
                    source_object_id: source,
                    ..Default::default()
                },
            )),
        },
    )
    .unwrap();
    let nested = e.state.pending_resolution.as_ref().unwrap();
    let answer = rv1::SubmitResolutionChoice {
        resolution_prompt_id: nested.resolution_prompt_id.get(),
        chosen_object_ids: vec![nested.presentation.candidates[0]],
        ..Default::default()
    };
    if e.state.pending_ability_activation.is_some() {
        for viewer in [0, 1, -1] {
            assert!(e
                .pending_ability_activation_for(viewer)
                .unwrap()
                .flex_payments
                .is_empty());
        }
    }
    let before = e.diagnostic_snapshot().unwrap();
    let unrelated = rv1::SubmitResolutionChoice {
        flex_payments: vec![rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
            mana_color: 0,
        }],
        ..answer.clone()
    };
    assert!(e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(unrelated))
            }
        )
        .is_err());
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
        },
    )
    .unwrap();
}

const HYBRID_CREATURE: &str = r#"(id:"hybrid_creature_probe",name:"Hybrid Creature Probe",face_id:"hybrid_creature_probe",types:["Creature"],power:2,toughness:2)"#;

#[test]
fn hybrid_deferred_activation_branches_are_announced_locked_and_paid_once() {
    for (card, zone) in [
        ("hybrid_opponent_probe", Zone::Battlefield),
        ("hybrid_hand_return_probe", Zone::Hand),
    ] {
        for (color, life) in [
            (rv1::ManaColor::Green, false),
            (rv1::ManaColor::White, false),
            (rv1::ManaColor::Unspecified, true),
        ] {
            let mut e = cast_fixture(&[
                HYBRID_DEFERRED_OPPONENT,
                HYBRID_HAND_RETURN,
                HYBRID_CREATURE,
                HYBRID_PAYMENT_MANA,
            ]);
            let mana_source = put_probe(&mut e, "hybrid_payment_mana", Zone::Battlefield, 0, false);
            let own = put_probe(&mut e, "hybrid_creature_probe", Zone::Battlefield, 0, true);
            let opponent = put_probe(&mut e, "hybrid_creature_probe", Zone::Battlefield, 1, false);
            let source = put_probe(&mut e, card, zone, 0, false);
            let ability = e
                .registry
                .get(card)
                .unwrap()
                .primary_face()
                .activated_abilities[0]
                .clone();
            assert!(e.ability_activatable(source, 0, &ability));
            let flex = rv1::FlexPipPayment {
                pip_index: 0,
                pay_life: life,
                mana_color: color as i32,
            };
            let begin = rv1::BeginAbilityActivation {
                source_object_id: source,
                source_zone: if zone == Zone::Hand {
                    rv1::AbilitySourceZone::Hand as i32
                } else {
                    rv1::AbilitySourceZone::Battlefield as i32
                },
                own_target: (zone == Zone::Battlefield).then_some(rv1::AbilityActivationTarget {
                    object_id: own,
                    ..Default::default()
                }),
                flex_payments: vec![flex],
                ..Default::default()
            };
            for invalid in [
                vec![],
                vec![rv1::FlexPipPayment {
                    mana_color: rv1::ManaColor::Blue as i32,
                    pay_life: false,
                    ..flex
                }],
                vec![flex, flex],
            ] {
                let before = e.diagnostic_snapshot().unwrap();
                assert!(e
                    .apply_command(
                        0,
                        &RuledCommand {
                            cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
                                rv1::BeginAbilityActivation {
                                    flex_payments: invalid,
                                    ..begin.clone()
                                }
                            ))
                        }
                    )
                    .is_err());
                assert_eq!(e.diagnostic_snapshot().unwrap(), before);
            }
            e.apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
                        begin.clone(),
                    )),
                },
            )
            .unwrap();
            let pending = e.state.pending_ability_activation.clone().unwrap();
            assert_eq!(pending.flex_payments, vec![flex]);
            assert_eq!(e.state.players[0].life, 20);
            assert_eq!(e.state.players[0].mana_pool.white, 2);
            // Cancelling an announcement neither pays nor strands its branch choice.
            e.apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CancelAbilityActivation(
                        rv1::CancelAbilityActivation {
                            transaction_id: pending.transaction_id,
                            expected_revision: pending.revision,
                        },
                    )),
                },
            )
            .unwrap();
            e.apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(begin)),
                },
            )
            .unwrap();
            if zone == Zone::Battlefield {
                let pending = e.state.pending_ability_activation.clone().unwrap();
                e.apply_command(
                    1,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitAbilityActivationChoice(
                            rv1::SubmitAbilityActivationChoice {
                                transaction_id: pending.transaction_id,
                                expected_revision: pending.revision,
                                target: Some(rv1::AbilityActivationTarget {
                                    object_id: opponent,
                                    group_index: 1,
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        )),
                    },
                )
                .unwrap();
            }
            let current = e.state.pending_ability_activation.clone().unwrap();
            assert_eq!(
                e.pending_ability_activation_for(0).unwrap().flex_payments,
                vec![flex]
            );
            for viewer in [1, -1] {
                assert!(e
                    .pending_ability_activation_for(viewer)
                    .unwrap()
                    .flex_payments
                    .is_empty());
            }
            hybrid_nested_mana(&mut e, mana_source);
            assert_eq!(
                e.state
                    .pending_ability_activation
                    .as_ref()
                    .unwrap()
                    .flex_payments,
                vec![flex]
            );
            assert_eq!(
                e.pending_ability_activation_for(0).unwrap().flex_payments,
                vec![flex]
            );
            let commit = rv1::CommitAbilityActivation {
                transaction_id: current.transaction_id,
                expected_revision: current.revision,
                return_tapped_creature: (zone == Zone::Hand).then_some(e.payment_object_ref(own)),
                ..Default::default()
            };
            for invalid in [
                rv1::CommitAbilityActivation {
                    flex_payments: vec![flex],
                    ..commit.clone()
                },
                rv1::CommitAbilityActivation {
                    transaction_id: pending.transaction_id,
                    ..commit.clone()
                },
            ] {
                let before = e.diagnostic_snapshot().unwrap();
                assert!(e
                    .apply_command(
                        0,
                        &RuledCommand {
                            cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(invalid))
                        }
                    )
                    .is_err());
                assert_eq!(e.diagnostic_snapshot().unwrap(), before);
            }
            e.apply_command(
                0,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(commit)),
                },
            )
            .unwrap();
            assert_eq!(e.state.players[0].life, if life { 18 } else { 20 });
            assert_eq!(
                e.state.players[0].mana_pool.white,
                if color == rv1::ManaColor::White { 1 } else { 2 }
            );
            assert_eq!(
                e.state.players[0].mana_pool.green,
                if color == rv1::ManaColor::Green { 1 } else { 2 }
            );
            assert_eq!(e.state.stack.len(), 1);
            if zone == Zone::Hand {
                assert_eq!(e.state.objects[&own].zone, Zone::Hand);
            } else {
                assert!(e.state.objects[&source].tapped);
            }
        }
    }
}

#[test]
fn hybrid_announcement_reduced_ability_offer_preserves_zero_pips_for_wire_indices() {
    const SOURCE: &str = r#"(id:"hybrid_reduced_probe",name:"Hybrid Reduced Probe",face_id:"hybrid_reduced_probe",types:["Artifact"],
     activated_abilities:[(ability_id:"activate",presentation:Fallback,costs:[Mana("{1}{G}{G/W/P}{W}")],
      cost_modifiers:[GenericReduction(amount:1)],effect:[GainLife(amount:1)])])"#;
    let mut e = cast_fixture(&[SOURCE]);
    let source = put_probe(&mut e, "hybrid_reduced_probe", Zone::Battlefield, 0, false);
    let effective = e.effective_activated_abilities(source).remove(0);
    assert_eq!(
        e.effective_ability_mana_cost(0, source, &effective.definition),
        "{0}{G}{G/W/P}{W}"
    );
}

#[test]
fn hybrid_direct_activation_uses_the_reduced_indexed_cost_and_exact_color_branch() {
    const SOURCE: &str = r#"(id:"hybrid_direct_probe",name:"Hybrid Direct Probe",face_id:"hybrid_direct_probe",types:["Artifact"],
     activated_abilities:[(ability_id:"activate",presentation:Fallback,costs:[Mana("{1}{G}{G/W/P}{W}")],
      cost_modifiers:[GenericReduction(amount:1)],effect:[GainLife(amount:1)])])"#;
    for selected in [
        choice(rv1::ManaColor::White, false),
        choice(rv1::ManaColor::Unspecified, true),
    ] {
        let mut e = cast_fixture(&[SOURCE]);
        let source = put_probe(&mut e, "hybrid_direct_probe", Zone::Battlefield, 0, false);
        let ability = e.effective_activated_abilities(source).remove(0);
        assert!(e.ability_activatable(source, ability.slot as usize, &ability.definition));
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                    rv1::ActivateAbility {
                        source_object_id: source,
                        expected_zone_change_generation: 0,
                        source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                        ability_index: ability.slot,
                        flex_payments: vec![selected],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
        assert_eq!(
            e.state.players[0].mana_pool.colorless, 1,
            "generic reduction removed the debit, not its pip index"
        );
        assert_eq!(
            e.state.players[0].mana_pool.white,
            if selected.pay_life { 1 } else { 0 }
        );
        assert_eq!(e.state.players[0].mana_pool.green, 1);
        assert_eq!(
            e.state.players[0].life,
            if selected.pay_life { 18 } else { 20 }
        );
        assert_eq!(
            e.state.stack[0].phyrexian_symbols_paid_with_life, 0,
            "an ability is not a cast spell"
        );
    }
}

#[test]
fn hybrid_turn_face_up_special_action_pays_each_branch_without_creating_cast_history() {
    const CREATURE: &str = r#"(id:"hybrid_face_up_probe",name:"Hybrid Face Up Probe",face_id:"hybrid_face_up_probe",
        mana_cost:"{G/W/P}",types:["Creature"],power:2,toughness:2)"#;
    for color in [
        rv1::ManaColor::Green,
        rv1::ManaColor::White,
        rv1::ManaColor::Unspecified,
    ] {
        let mut e = cast_fixture(&[CREATURE]);
        let source = put_probe(&mut e, "hybrid_face_up_probe", Zone::Battlefield, 0, false);
        e.state.objects.get_mut(&source).unwrap().face_down = true;
        let selected = rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: color == rv1::ManaColor::Unspecified,
            mana_color: color as i32,
        };
        let mut action = rv1::ExecutePermanentAction {
            object_id: source,
            expected_zone_change_generation: 0,
            kind: rv1::PermanentActionKind::TurnFaceUp as i32,
            flex_payments: vec![selected],
            payment: Some(rv1::PaymentSelection {
                expected_state_revision: e.state.command_index,
                source: Some(e.payment_object_ref(source)),
                mana: Some(rv1::PaymentMana {
                    w: u32::from(color == rv1::ManaColor::White),
                    g: u32::from(color == rv1::ManaColor::Green),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let preview = e.preview_payment(
            0,
            &rv1::PreviewPayment {
                transaction_id: 71,
                execute_permanent_action: Some(action.clone()),
                ..Default::default()
            },
        );
        assert!(preview.valid && preview.complete, "{}", preview.error);
        action.payment = preview.selection;
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::ExecutePermanentAction(action)),
            },
        )
        .unwrap();
        assert!(!e.state.objects[&source].face_down);
        assert!(e.state.stack.is_empty());
        assert_eq!(
            e.state.players[0].life,
            if selected.pay_life { 18 } else { 20 }
        );
        assert_eq!(
            e.state.players[0].mana_pool.white,
            if color == rv1::ManaColor::White { 1 } else { 2 }
        );
        assert_eq!(
            e.state.players[0].mana_pool.green,
            if color == rv1::ManaColor::Green { 1 } else { 2 }
        );
    }
}

#[test]
fn hybrid_aggregate_begin_locks_color_then_cancel_can_choose_life_without_early_debits() {
    const SOURCE: &str = r#"(id:"hybrid_aggregate_probe",name:"Hybrid Aggregate Probe",face_id:"hybrid_aggregate_probe",types:["Artifact"],
      activated_abilities:[(ability_id:"activate",presentation:Fallback,costs:[Mana("{G/W/P}"),
       RemoveCountersFromPermanents(count:1,counter:Some(Charge),filter:(kind:AnyPermanent,controller:You))],effect:[GainLife(amount:1)])])"#;
    let mut e = cast_fixture(&[SOURCE]);
    let source = put_probe(
        &mut e,
        "hybrid_aggregate_probe",
        Zone::Battlefield,
        0,
        false,
    );
    e.state
        .objects
        .get_mut(&source)
        .unwrap()
        .counters
        .insert(CounterKind::Charge, 1);
    let ability = e.effective_activated_abilities(source).remove(0);
    assert!(e.ability_activatable(source, ability.slot as usize, &ability.definition));
    let selection = rv1::CostSelection {
        cost_index: 1,
        selection: Some(rv1::cost_selection::Selection::CounterRemovalGroup(
            rv1::CounterRemovalGroupSelection {
                contributions: vec![rv1::CounterRemovalContribution {
                    object: Some(e.payment_object_ref(source)),
                    option_id: super::super::counters::counter_option_id(CounterKind::Charge),
                    count: 1,
                }],
            },
        )),
    };
    let white = rv1::FlexPipPayment {
        pip_index: 0,
        pay_life: false,
        mana_color: rv1::ManaColor::White as i32,
    };
    let begin = |payment| RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginAbilityActivation(
            rv1::BeginAbilityActivation {
                source_object_id: source,
                source_zone: rv1::AbilitySourceZone::Battlefield as i32,
                expected_zone_change_generation: 0,
                ability_index: ability.slot,
                cost_selections: vec![selection.clone()],
                flex_payments: vec![payment],
                ..Default::default()
            },
        )),
    };
    e.apply_command(0, &begin(white)).unwrap();
    let pending = e.state.pending_ability_activation.as_ref().unwrap().clone();
    assert_eq!(pending.flex_payments, vec![white]);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Charge),
        1
    );
    assert_eq!(e.state.players[0].mana_pool.white, 2);
    let before = e.diagnostic_snapshot().unwrap();
    assert!(e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                    rv1::CommitAbilityActivation {
                        transaction_id: pending.transaction_id,
                        expected_revision: pending.revision,
                        flex_payments: vec![rv1::FlexPipPayment {
                            mana_color: rv1::ManaColor::Green as i32,
                            ..white
                        }],
                        ..Default::default()
                    }
                ))
            }
        )
        .is_err());
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CancelAbilityActivation(
                rv1::CancelAbilityActivation {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                },
            )),
        },
    )
    .unwrap();
    assert_eq!(e.state.players[0].life, 20);
    let life = rv1::FlexPipPayment {
        pip_index: 0,
        pay_life: true,
        mana_color: 0,
    };
    e.apply_command(0, &begin(life)).unwrap();
    let retry = e.state.pending_ability_activation.as_ref().unwrap().clone();
    assert_ne!(retry.transaction_id, pending.transaction_id);
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CommitAbilityActivation(
                rv1::CommitAbilityActivation {
                    transaction_id: retry.transaction_id,
                    expected_revision: retry.revision,
                    ..Default::default()
                },
            )),
        },
    )
    .unwrap();
    assert_eq!(e.state.players[0].life, 18);
    assert_eq!(e.state.players[0].mana_pool.white, 2);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Charge),
        0
    );
    assert_eq!(e.state.stack.len(), 1);
}

#[test]
fn ajani_shaped_life_cast_and_doubling_season_preserve_both_compleated_entry_orders() {
    const SEASON: &str = r#"(id:"hybrid_season_probe",name:"Hybrid Season Probe",face_id:"hybrid_season_probe",types:["Enchantment"],
       static_abilities:[(ability_id:"double",presentation:Fallback,definition:DoubleEffectCountersPlacedOnPermanentsYouControl)])"#;
    for (compleated_first, loyalty) in [(true, 4), (false, 6)] {
        let mut e = cast_fixture(&[SEASON]);
        put_probe(&mut e, "hybrid_season_probe", Zone::Battlefield, 0, false);
        let source = e.state.players[0].hand[0];
        let transaction = begin_cast(&mut e, choice(rv1::ManaColor::Unspecified, true));
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id: transaction,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
        let actor = e.state.priority_player_id();
        e.apply_command(
            actor,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
            },
        )
        .unwrap();
        let actor = e.state.priority_player_id();
        let batch = e
            .apply_command(
                actor,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
                },
            )
            .unwrap();
        let choice = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(choice)) => Some(choice),
                _ => None,
            })
            .unwrap();
        assert_eq!(choice.replacement_options.len(), 2);
        let option = choice
            .replacement_options
            .iter()
            .find(|option| {
                if compleated_first {
                    option
                        .effect_summary
                        .to_ascii_lowercase()
                        .contains("loyalty")
                } else {
                    option.effect_summary.contains("doubles effect counters")
                }
            })
            .unwrap();
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: choice.resolution_prompt_id,
                        chosen_object_ids: vec![option.application_id],
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Loyalty),
            loyalty
        );
        assert_eq!(e.state.players[0].life, 18);
        assert!(e.state.pending_resolution.is_none());
    }
}

#[test]
fn hybrid_free_cast_omits_the_printed_pip_and_rejects_a_stale_printed_cost_choice() {
    let mut e = cast_fixture(&[]);
    let marker = e.state.next_object_id;
    e.state.next_object_id += 1;
    e.state
        .static_emblems
        .push(crate::state::StaticEmblemInstance {
            object_id: marker,
            controller: 0,
            emblem_id: "prepared_free_cast".into(),
            display_name: "Prepared Free Cast".into(),
            effects: vec![StaticEmblemEffect::CastSpellsFromHandWithoutPayingManaCost],
            triggered_ability: None,
        });
    let source = e.state.players[0].hand[0];
    let announcement = rv1::SpellCastAnnouncement {
        source: Some(rv1::CastSource {
            location: Some(rv1::cast_source::Location::HandIndex(0)),
            expected_zone_change_generation: Some(0),
        }),
        cast_method: rv1::CastMethod::WithoutManaCost as i32,
        flex_payments: vec![choice(rv1::ManaColor::White, false)],
        ..Default::default()
    };
    let before = e.diagnostic_snapshot().unwrap();
    let begin = |announcement| RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
            rv1::BeginSpellCast {
                announcement: Some(announcement),
            },
        )),
    };
    assert!(e.apply_command(0, &begin(announcement.clone())).is_err());
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    let mut free = announcement;
    free.flex_payments.clear();
    e.apply_command(0, &begin(free)).unwrap();
    let transaction = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                rv1::CommitSpellCast {
                    transaction_id: transaction,
                    ..Default::default()
                },
            )),
        },
    )
    .unwrap();
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[0].mana_pool.green, 2);
    assert_eq!(e.state.players[0].mana_pool.white, 2);
    assert_eq!(e.state.stack[0].phyrexian_symbols_paid_with_life, 0);
    for _ in 0..2 {
        let actor = e.state.priority_player_id();
        e.apply_command(
            actor,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
            },
        )
        .unwrap();
    }
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Loyalty),
        4
    );
}

#[test]
fn hybrid_chosen_color_constrains_explicit_restricted_mana_contributions() {
    let mut e = payment_engine();
    e.state.players[0].mana_pool.white = 0;
    e.state.players[0].mana_pool.green = 0;
    e.state.players[0].mana_pool.colorless = 0;
    e.state.players[0]
        .restricted_mana
        .push(crate::state::RestrictedManaContribution {
            restriction_group_id: 71,
            amount: ManaAmount {
                g: 1,
                ..Default::default()
            },
        });
    let spend = rv1::ManaSpendSelection {
        restriction_group_id: 71,
        g: 1,
        ..Default::default()
    };
    let cost = ManaCost::parse("{G/W/P}").unwrap();
    let green = rv1::FlexPipPayment {
        pip_index: 0,
        pay_life: false,
        mana_color: rv1::ManaColor::Green as i32,
    };
    let white = rv1::FlexPipPayment {
        mana_color: rv1::ManaColor::White as i32,
        ..green
    };
    assert!(mana::plan_mana_payment_with_any_color(
        &e.state,
        0,
        &cost,
        0,
        0,
        0,
        &[green],
        &[spend],
        &[],
        false
    )
    .is_err());
    let plan = mana::plan_mana_payment_with_any_color(
        &e.state,
        0,
        &cost,
        0,
        0,
        0,
        &[green],
        &[spend],
        &[71],
        false,
    )
    .unwrap();
    assert_eq!(
        plan.restricted_spent(),
        &[(
            71,
            ManaAmount {
                g: 1,
                ..Default::default()
            }
        )]
    );
    assert!(mana::plan_mana_payment_with_any_color(
        &e.state,
        0,
        &cost,
        0,
        0,
        0,
        &[white],
        &[spend],
        &[71],
        false
    )
    .is_err());
}

#[test]
fn hybrid_prepared_command_log_replays_exact_color_life_cancel_and_compleated_state() {
    use prost::Message;
    let mut e = cast_fixture(&[]);
    let mut log = Vec::new();
    let selection = choice(rv1::ManaColor::White, false);
    let transaction = begin_cast(&mut e, selection);
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
            rv1::BeginSpellCast {
                announcement: Some(
                    e.state
                        .pending_spell_cast
                        .as_ref()
                        .unwrap()
                        .announcement
                        .clone(),
                ),
            },
        )),
    };
    log.push((
        0,
        command.encode_to_vec(),
        serde_json::to_value(e.diagnostic_snapshot().unwrap()).unwrap(),
    ));
    let cancel = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
            rv1::CancelSpellCast {
                transaction_id: transaction,
            },
        )),
    };
    e.apply_command(0, &cancel).unwrap();
    log.push((
        0,
        cancel.encode_to_vec(),
        serde_json::to_value(e.diagnostic_snapshot().unwrap()).unwrap(),
    ));
    let retry = begin_cast(&mut e, choice(rv1::ManaColor::Unspecified, true));
    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::BeginSpellCast(
            rv1::BeginSpellCast {
                announcement: Some(
                    e.state
                        .pending_spell_cast
                        .as_ref()
                        .unwrap()
                        .announcement
                        .clone(),
                ),
            },
        )),
    };
    log.push((
        0,
        command.encode_to_vec(),
        serde_json::to_value(e.diagnostic_snapshot().unwrap()).unwrap(),
    ));
    let commit = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
            rv1::CommitSpellCast {
                transaction_id: retry,
                ..Default::default()
            },
        )),
    };
    e.apply_command(0, &commit).unwrap();
    log.push((
        0,
        commit.encode_to_vec(),
        serde_json::to_value(e.diagnostic_snapshot().unwrap()).unwrap(),
    ));
    for _ in 0..2 {
        let actor = e.state.priority_player_id();
        let pass = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
        };
        e.apply_command(actor, &pass).unwrap();
        log.push((
            actor,
            pass.encode_to_vec(),
            serde_json::to_value(e.diagnostic_snapshot().unwrap()).unwrap(),
        ));
    }
    let mut replay = cast_fixture(&[]);
    for (actor, bytes, expected) in log {
        let decoded = RuledCommand::decode(bytes.as_slice()).unwrap();
        replay.apply_command(actor, &decoded).unwrap();
        assert_eq!(
            serde_json::to_value(replay.diagnostic_snapshot().unwrap()).unwrap(),
            expected
        );
    }
    assert_eq!(replay.state.players[0].life, 18);
}

const HYBRID_RESOLUTION: &str = r#"(id:"hybrid_resolution_probe",name:"Hybrid Resolution Probe",face_id:"hybrid_resolution_probe",types:["Artifact"],
 activated_abilities:[(ability_id:"activate",presentation:Fallback,costs:[],effect:[],program:Some((body:(steps:[
 (id:"choose",kind:Choose(choice:"pay",chooser:Controller,optional:true,branches:[(
 branch_id:"paid",presentation:Fallback,cost:Mana("{G/W/P}"),body:(steps:[
 (id:"gain",kind:Perform(GainLife(amount:4))) ]))],otherwise:())) ]))))])"#;

fn hybrid_resolution_fixture() -> GameEngine {
    let mut e = cast_fixture(&[HYBRID_RESOLUTION, HYBRID_PAYMENT_MANA]);
    put_probe(&mut e, "hybrid_payment_mana", Zone::Battlefield, 0, false);
    let source = put_probe(
        &mut e,
        "hybrid_resolution_probe",
        Zone::Battlefield,
        0,
        false,
    );
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::ActivateAbility(
                rv1::ActivateAbility {
                    source_object_id: source,
                    ..Default::default()
                },
            )),
        },
    )
    .unwrap();
    for _ in 0..2 {
        let actor = e.state.priority_player_id();
        e.apply_command(
            actor,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::PassPriority(rv1::PassPriority {})),
            },
        )
        .unwrap();
    }
    let prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt,
                    decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                    ..Default::default()
                },
            )),
        },
    )
    .unwrap();
    assert!(e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .continuation
        .mana_payment()
        .is_some());
    e
}

#[test]
fn hybrid_resolution_preview_commit_decline_and_stale_receipts_preserve_exact_payment() {
    use prost::Message;
    for (color, life) in [
        (rv1::ManaColor::Green, false),
        (rv1::ManaColor::White, false),
        (rv1::ManaColor::Unspecified, true),
    ] {
        let mut e = hybrid_resolution_fixture();
        let prompt = e
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        let mut answer = rv1::SubmitResolutionChoice {
            resolution_prompt_id: prompt,
            decision: rv1::ResolutionChoiceDecision::PayMana as i32,
            flex_payments: vec![rv1::FlexPipPayment {
                pip_index: 0,
                mana_color: color as i32,
                pay_life: life,
            }],
            payment: Some(rv1::PaymentSelection {
                mana: Some(rv1::PaymentMana {
                    w: u32::from(color == rv1::ManaColor::White),
                    g: u32::from(color == rv1::ManaColor::Green),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        if life {
            e.state.players[0].mana_pool = Default::default();
        }
        let before = e.diagnostic_snapshot().unwrap();
        for invalid in [
            rv1::SubmitResolutionChoice {
                flex_payments: vec![],
                ..answer.clone()
            },
            rv1::SubmitResolutionChoice {
                resolution_prompt_id: prompt + 1,
                ..answer.clone()
            },
            rv1::SubmitResolutionChoice {
                decision: rv1::ResolutionChoiceDecision::Decline as i32,
                ..answer.clone()
            },
        ] {
            let view = e.preview_payment(
                0,
                &rv1::PreviewPayment {
                    resolution_choice: Some(invalid.clone()),
                    ..Default::default()
                },
            );
            assert!(!view.valid);
            assert!(e
                .apply_command(
                    0,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(invalid))
                    }
                )
                .is_err());
            assert_eq!(e.diagnostic_snapshot().unwrap(), before);
        }
        if life {
            e.state.players[0].life = 1;
            assert!(!e.can_pay_resolution_mana(0, &ManaCost::parse("{G/W/P}").unwrap()));
            assert!(
                !e.preview_payment(
                    0,
                    &rv1::PreviewPayment {
                        resolution_choice: Some(answer.clone()),
                        ..Default::default()
                    }
                )
                .valid
            );
            e.state.players[0].life = 20;
        }
        let preview = e.preview_payment(
            0,
            &rv1::PreviewPayment {
                resolution_choice: Some(answer.clone()),
                ..Default::default()
            },
        );
        assert!(preview.valid && preview.complete, "{preview:?}");
        answer.payment = preview.selection;
        let command = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
        };
        let bytes = command.encode_to_vec();
        e.apply_command(0, &command).unwrap();
        assert!(e.state.pending_resolution.is_none());
        assert_eq!(e.state.players[0].life, if life { 22 } else { 24 });
        assert_eq!(
            e.state.players[0].mana_pool.white,
            if life {
                0
            } else if color == rv1::ManaColor::White {
                1
            } else {
                2
            }
        );
        assert_eq!(
            e.state.players[0].mana_pool.green,
            if life {
                0
            } else if color == rv1::ManaColor::Green {
                1
            } else {
                2
            }
        );
        let mut replay = hybrid_resolution_fixture();
        if life {
            replay.state.players[0].mana_pool = Default::default();
        }
        replay
            .apply_command(0, &RuledCommand::decode(bytes.as_slice()).unwrap())
            .unwrap();
        assert_eq!(
            replay.diagnostic_snapshot().unwrap(),
            e.diagnostic_snapshot().unwrap()
        );
    }
    let mut e = hybrid_resolution_fixture();
    let prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt,
                    decision: rv1::ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                },
            )),
        },
    )
    .unwrap();
    assert_eq!(e.state.players[0].life, 20);
    assert_eq!(e.state.players[0].mana_pool.white, 2);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn hybrid_resolution_nested_mana_reissues_the_prompt_and_rejects_old_receipts() {
    let mut e = hybrid_resolution_fixture();
    let prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    let mana_source = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|id| e.state.objects[id].card_id == "hybrid_payment_mana")
        .unwrap();
    let mut answer = rv1::SubmitResolutionChoice {
        resolution_prompt_id: prompt,
        decision: rv1::ResolutionChoiceDecision::PayMana as i32,
        flex_payments: vec![rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
            mana_color: 0,
        }],
        ..Default::default()
    };
    let preview = e.preview_payment(
        0,
        &rv1::PreviewPayment {
            resolution_choice: Some(answer.clone()),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete);
    answer.payment = preview.selection;
    hybrid_nested_mana(&mut e, mana_source);
    let restored_prompt = e
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .resolution_prompt_id
        .get();
    assert_ne!(restored_prompt, prompt);
    let before = e.diagnostic_snapshot().unwrap();
    assert!(e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    answer.clone()
                ))
            }
        )
        .is_err());
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
    answer.resolution_prompt_id = restored_prompt;
    let refreshed = e.preview_payment(
        0,
        &rv1::PreviewPayment {
            resolution_choice: Some(answer.clone()),
            ..Default::default()
        },
    );
    assert!(refreshed.valid && refreshed.complete);
    answer.payment = refreshed.selection;
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(answer)),
        },
    )
    .unwrap();
    assert_eq!(e.state.players[0].life, 22);
    assert_eq!(e.state.players[0].mana_pool.colorless, 2);
}
