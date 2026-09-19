//! Issue #412 — the generated modal-spell and modal-ETB cohort.
//!
//! Oracle and rulings were checked 2026-09-19 against the pinned Scryfall snapshot. CR 700.2
//! (mode announcement), CR 702.194 (Teamwork), CR 701.12 (fight), CR 701.7 (destroy), CR 701.5
//! (counter), CR 111 (tokens), CR 119/121 (life and draw), and CR 701.25 (surveil) govern the
//! exercised behavior.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    CastCostGroupSelection, CastMethod, CastSpell, ChoiceKind, ChooseTriggerTarget, CostObjectRefs,
    RuledCommand, SelectedSpellMode, TargetRef,
};

fn modal_engine(seed: u64) -> GameEngine {
    // Explicit land-only decks keep the default grizzly_bears/library fixtures out of the
    // opening hand so injected fixture names stay unique.
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn fund(engine: &mut GameEngine, player: i32) {
    give_mana(
        engine,
        player,
        ManaGift {
            w: 4,
            u: 4,
            b: 4,
            r: 4,
            g: 4,
            c: 4,
        },
    );
}

fn prepare_spell(engine: &mut GameEngine, card_id: &str) -> usize {
    inject_card_into_hand(engine, 0, card_id);
    fund(engine, 0);
    hand_index_for_card(engine, 0, card_id)
}

fn target_in_group(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        ..Default::default()
    }
}

fn object_ref(engine: &GameEngine, object_id: u32) -> tricerules_proto::ruled::v1::CostObjectRef {
    tricerules_proto::ruled::v1::CostObjectRef {
        object_id,
        zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0),
    }
}

fn object_cost(engine: &GameEngine, option_index: u32, objects: &[u32]) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        battlefield_objects: Some(CostObjectRefs {
            objects: objects.iter().map(|oid| object_ref(engine, *oid)).collect(),
        }),
        ..Default::default()
    }
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn choose_trigger_mode(mode_index: u32, targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: vec![SelectedSpellMode {
                mode_index,
                targets,
            }],
            targets: Vec::new(),
        })),
    }
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

#[test]
fn plow_through_fight_mode_damages_both_ways() {
    let mut engine = modal_engine(412_001);
    let fighter = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&opposing).unwrap().power = Some(4);
    engine.state.objects.get_mut(&opposing).unwrap().toughness = Some(4);
    let slot = prepare_spell(&mut engine, "plow_through");

    // Group 0 is the controlled creature and group 1 the opposing creature; crossing them fails.
    assert!(
        engine
            .apply_command(
                0,
                &cast_modal_spell(
                    slot,
                    vec![(
                        0,
                        vec![target_object(opposing)[0], target_in_group(fighter, 1)]
                    )],
                ),
            )
            .is_err(),
        "fight groups must keep their printed controller relationships"
    );

    engine
        .apply_command(
            0,
            &cast_modal_spell(
                slot,
                vec![(
                    0,
                    vec![target_object(fighter)[0], target_in_group(opposing, 1)],
                )],
            ),
        )
        .expect("both fighters are legal targets");
    resolve_entire_stack_two_player(&mut engine);

    // The 2/2 deals 2 to the 4/4 and the 4/4 deals 4 back: damage goes both ways, so the fighter
    // dies while the opposing creature survives with marked damage.
    assert_eq!(engine.state.objects[&opposing].damage, 2);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&fighter].zone, Zone::Graveyard);
}

#[test]
fn plow_through_vehicle_mode_destroys_only_vehicles() {
    let mut engine = modal_engine(412_002);
    let vehicle = inject_permanent_on_battlefield(&mut engine, 1, "cultivators_caravan");
    let sword = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let slot = prepare_spell(&mut engine, "plow_through");

    assert!(
        engine
            .apply_command(0, &cast_modal_spell(slot, vec![(1, target_object(sword))]))
            .is_err(),
        "a plain artifact is not a Vehicle"
    );
    engine
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, target_object(vehicle))]),
        )
        .expect("a Vehicle is a legal target");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&vehicle].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&sword].zone, Zone::Battlefield);
}

#[test]
fn heritage_reclamation_exiles_an_optional_graveyard_card_and_draws() {
    let mut engine = modal_engine(412_003);
    let buried = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    seat_on_top(&mut engine, 0, &["forest"]);
    let hand_before = engine.state.players[0].hand.len();
    let slot = prepare_spell(&mut engine, "heritage_reclamation");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(2, target_object(buried))]))
        .expect("exile up to one graveyard card");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&buried].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);

    // The instruction is optional: choosing no target still draws.
    let mut optional = modal_engine(412_004);
    seat_on_top(&mut optional, 0, &["forest"]);
    let hand_before = optional.state.players[0].hand.len();
    let slot = prepare_spell(&mut optional, "heritage_reclamation");
    optional
        .apply_command(0, &cast_modal_spell(slot, vec![(2, Vec::new())]))
        .expect("up to one target permits no target");
    resolve_entire_stack_two_player(&mut optional);
    assert_eq!(optional.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(
        optional.state.players[0].graveyard.len(),
        1,
        "no graveyard card was exiled; only the resolved spell is there"
    );
}

#[test]
fn pawpatch_formation_draws_and_creates_a_food_token() {
    let mut engine = modal_engine(412_005);
    seat_on_top(&mut engine, 0, &["forest"]);
    let hand_before = engine.state.players[0].hand.len();
    let slot = prepare_spell(&mut engine, "pawpatch_formation");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(2, Vec::new())]))
        .expect("draw and create a Food");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(battlefield_token_oids(&engine, 0, "food").len(), 1);
}

#[test]
fn unforgiving_aim_creates_the_black_green_elf_token() {
    let mut engine = modal_engine(412_006);
    let slot = prepare_spell(&mut engine, "unforgiving_aim");

    engine
        .apply_command(0, &cast_modal_spell(slot, vec![(2, Vec::new())]))
        .expect("create the Elf token");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(battlefield_token_oids(&engine, 0, "elf_bg_2_2").len(), 1);
}

#[test]
fn go_nuts_teamwork_unlocks_both_modes() {
    let mut engine = modal_engine(412_010);
    let slot = prepare_spell(&mut engine, "go_nuts!");

    // Without an untapped Teamwork cohort the published allowance stays at one mode; the fight
    // target exists but is tapped, so it cannot contribute power.
    let fighter = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let counted = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.objects.get_mut(&fighter).unwrap().tapped = true;
    let published = engine.initial_response_batch();
    let action = published.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == slot as u32)
        .expect("Go Nuts! is castable");
    assert_eq!(action.max_modes, 1);
    assert!(action.all_modes_cast_cost.is_none());

    let first_teammate = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_teammate = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&fighter).unwrap().tapped = false;

    let published = engine.initial_response_batch();
    let action = published.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == slot as u32)
        .expect("Go Nuts! is castable with Teamwork");
    assert_eq!(action.max_modes, 2);
    assert!(action.all_modes_cast_cost.is_some());

    let modes = vec![
        SelectedSpellMode {
            mode_index: 0,
            targets: target_object(counted),
        },
        SelectedSpellMode {
            mode_index: 1,
            targets: vec![target_object(fighter)[0], target_in_group(opposing, 1)],
        },
    ];
    let without_teamwork = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(hand_cast_source(slot)),
            cast_method: CastMethod::Normal as i32,
            selected_modes: modes.clone(),
            ..Default::default()
        })),
    };
    assert!(
        engine.apply_command(0, &without_teamwork).is_err(),
        "both modes require the announced Teamwork payment"
    );
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    source: Some(hand_cast_source(slot)),
                    cast_method: CastMethod::Normal as i32,
                    selected_modes: modes,
                    cast_cost_group_selections: vec![object_cost(
                        &engine,
                        0,
                        &[first_teammate, second_teammate],
                    )],
                    ..Default::default()
                })),
            },
        )
        .expect("Teamwork 3 is paid with two power-two creatures");
    assert!(engine.state.objects[&first_teammate].tapped);
    assert!(engine.state.objects[&second_teammate].tapped);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&counted]
            .counters
            .get(&tricerules_cards::CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert_eq!(engine.state.objects[&counted].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&fighter].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Graveyard);
}

#[test]
fn hulk_smash_teamwork_destroys_the_artifact_and_deals_power_damage() {
    let mut engine = modal_engine(412_011);
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let source = inject_creature_on_battlefield(&mut engine, 0, "colossal_dreadmaw");
    engine.state.objects.get_mut(&source).unwrap().power = Some(6);
    engine.state.objects.get_mut(&source).unwrap().toughness = Some(6);
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let slot = prepare_spell(&mut engine, "hulk_smash!");

    let first_teammate = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second_teammate = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let modes = vec![
        SelectedSpellMode {
            mode_index: 0,
            targets: target_object(artifact),
        },
        SelectedSpellMode {
            mode_index: 1,
            targets: vec![target_object(source)[0], target_in_group(opposing, 1)],
        },
    ];
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    source: Some(hand_cast_source(slot)),
                    cast_method: CastMethod::Normal as i32,
                    selected_modes: modes,
                    cast_cost_group_selections: vec![object_cost(
                        &engine,
                        0,
                        &[first_teammate, second_teammate],
                    )],
                    ..Default::default()
                })),
            },
        )
        .expect("Teamwork 4 is paid with two power-two creatures");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&source].damage, 0,
        "power-based damage does not damage the source back"
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
}

#[test]
fn coliseum_behemoth_etb_chooses_destruction_or_draw() {
    let mut destroy_engine = modal_engine(412_020);
    let artifact = inject_permanent_on_battlefield(&mut destroy_engine, 1, "short_sword");
    let enchantment = inject_permanent_on_battlefield(&mut destroy_engine, 1, "glorious_anthem");
    let hand_before = destroy_engine.state.players[0].hand.len();
    let behemoth = inject_card_into_hand(&mut destroy_engine, 0, "coliseum_behemoth");
    fund(&mut destroy_engine, 0);
    let slot = hand_index_for_card(&destroy_engine, 0, "coliseum_behemoth");
    destroy_engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Coliseum Behemoth");
    pass_both_players(&mut destroy_engine);
    destroy_engine
        .apply_command(0, &choose_trigger_mode(0, target_object(enchantment)))
        .expect("destroy target artifact or enchantment");
    pass_both_players(&mut destroy_engine);
    assert_eq!(
        destroy_engine.state.objects[&enchantment].zone,
        Zone::Graveyard
    );
    assert_eq!(
        destroy_engine.state.objects[&artifact].zone,
        Zone::Battlefield
    );
    assert_eq!(destroy_engine.state.players[0].hand.len(), hand_before);
    assert_eq!(
        destroy_engine.state.objects[&behemoth].zone,
        Zone::Battlefield
    );

    let mut draw_engine = modal_engine(412_021);
    seat_on_top(&mut draw_engine, 0, &["forest"]);
    let hand_before = draw_engine.state.players[0].hand.len();
    inject_card_into_hand(&mut draw_engine, 0, "coliseum_behemoth");
    fund(&mut draw_engine, 0);
    let slot = hand_index_for_card(&draw_engine, 0, "coliseum_behemoth");
    draw_engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Coliseum Behemoth");
    pass_both_players(&mut draw_engine);
    draw_engine
        .apply_command(0, &choose_trigger_mode(1, Vec::new()))
        .expect("draw a card");
    pass_both_players(&mut draw_engine);
    assert_eq!(draw_engine.state.players[0].hand.len(), hand_before + 1);
}

#[test]
fn fangkeepers_familiar_etb_gains_life_and_surveils() {
    let mut engine = modal_engine(412_030);
    let top = seat_on_top(&mut engine, 0, &["grizzly_bears", "storm_crow", "island"]);
    inject_card_into_hand(&mut engine, 0, "fangkeepers_familiar");
    fund(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "fangkeepers_familiar");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Fangkeeper's Familiar");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_mode(0, Vec::new()))
        .expect("gain 3 life and surveil 3");
    let batch = resolve_top_stack(&mut engine);
    assert_eq!(engine.state.players[0].life, 23);
    let choice = find_resolution_choice(&batch).expect("surveil choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 3));
    assert_eq!(choice.candidate_object_ids, top);

    engine
        .apply_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
        .expect("put two surveilled cards into the graveyard");
    assert_eq!(engine.state.objects[&top[0]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&top[1]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&top[2]].zone, Zone::Library);
    assert_eq!(
        engine.state.players[0].library.front().copied(),
        Some(top[2])
    );
}

#[test]
fn fangkeepers_familiar_etb_destroys_an_enchantment() {
    let mut engine = modal_engine(412_031);
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "glorious_anthem");
    inject_card_into_hand(&mut engine, 0, "fangkeepers_familiar");
    fund(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "fangkeepers_familiar");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Fangkeeper's Familiar");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_trigger_mode(1, target_object(enchantment)))
        .expect("destroy target enchantment");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&enchantment].zone, Zone::Graveyard);
}

#[test]
fn fangkeepers_familiar_etb_counters_a_creature_spell() {
    let mut engine = modal_engine(412_032);
    fund(&mut engine, 0);
    fund(&mut engine, 1);

    // Player 0's creature spell starts the stack; player 1 flashes a creature above it, then
    // player 0 flashes Fangkeeper's Familiar above both so its ETB trigger can target the
    // still-unresolved creature spell.
    let bears = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let bears_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(bears_slot, Vec::new()))
        .expect("cast Grizzly Bears");
    engine
        .apply_command(0, &pass())
        .expect("pass priority to the opponent");

    let wolf = inject_card_into_hand(&mut engine, 1, "bounding_wolf");
    let wolf_slot = hand_index_for_card(&engine, 1, "bounding_wolf");
    engine
        .apply_command(1, &cast_spell(wolf_slot, Vec::new()))
        .expect("flash Bounding Wolf");
    engine
        .apply_command(1, &pass())
        .expect("pass priority back");

    let fangkeeper = inject_card_into_hand(&mut engine, 0, "fangkeepers_familiar");
    let fang_slot = hand_index_for_card(&engine, 0, "fangkeepers_familiar");
    engine
        .apply_command(0, &cast_spell(fang_slot, Vec::new()))
        .expect("flash Fangkeeper's Familiar");
    engine
        .apply_command(0, &pass())
        .expect("pass priority to the opponent");
    engine
        .apply_command(1, &pass())
        .expect("Fangkeeper's Familiar resolves and the ETB trigger is announced");

    // A battlefield permanent is not a stack spell, so it cannot satisfy the creature-spell
    // filter.
    let permanent = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    assert!(
        engine
            .apply_command(0, &choose_trigger_mode(2, target_object(permanent)),)
            .is_err(),
        "a battlefield permanent is not a creature spell"
    );
    engine
        .apply_command(0, &choose_trigger_mode(2, target_object(wolf)))
        .expect("counter the flashed creature spell");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&wolf].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&fangkeeper].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&bears].zone, Zone::Stack);
}

#[test]
fn shipped_choose_one_and_teamwork_allowances_still_bind_exactly() {
    // Abrade's reviewed pair keeps its per-mode target filters.
    let mut abrade = modal_engine(412_040);
    let creature = inject_creature_on_battlefield(&mut abrade, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut abrade, 1, "short_sword");
    let slot = prepare_spell(&mut abrade, "abrade");
    assert!(
        abrade
            .apply_command(
                0,
                &cast_modal_spell(slot, vec![(0, target_object(artifact))])
            )
            .is_err(),
        "Abrade's damage mode still requires a creature"
    );
    abrade
        .apply_command(
            0,
            &cast_modal_spell(slot, vec![(0, target_object(creature))]),
        )
        .expect("Abrade's damage mode still accepts a creature");
    resolve_entire_stack_two_player(&mut abrade);
    assert_eq!(abrade.state.objects[&creature].zone, Zone::Graveyard);

    // Murdock's Crusade keeps its shipped Teamwork-to-all-modes link.
    let mut crusade = modal_engine(412_041);
    let target = inject_creature_on_battlefield(&mut crusade, 1, "colossal_dreadmaw");
    let enchantment = inject_permanent_on_battlefield(&mut crusade, 1, "burn,_burn,_tree_and_fern");
    let slot = prepare_spell(&mut crusade, "murdocks_crusade");
    let published = crusade.initial_response_batch();
    let action = published.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == slot as u32)
        .expect("Murdock's Crusade is castable");
    assert_eq!(action.max_modes, 1);
    assert!(action.all_modes_cast_cost.is_none());
    let first = inject_creature_on_battlefield(&mut crusade, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut crusade, 0, "grizzly_bears");
    let published = crusade.initial_response_batch();
    let action = published.legal_by_player[&0]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == slot as u32)
        .expect("Murdock's Crusade is castable with Teamwork");
    assert_eq!(action.max_modes, 2);
    assert!(action.all_modes_cast_cost.is_some());
    crusade
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    source: Some(hand_cast_source(slot)),
                    cast_method: CastMethod::Normal as i32,
                    selected_modes: vec![
                        SelectedSpellMode {
                            mode_index: 0,
                            targets: target_object(target),
                        },
                        SelectedSpellMode {
                            mode_index: 1,
                            targets: target_object(enchantment),
                        },
                    ],
                    cast_cost_group_selections: vec![object_cost(&crusade, 0, &[first, second])],
                    ..Default::default()
                })),
            },
        )
        .expect("the shipped teamwork allowance still pays and resolves");
    resolve_entire_stack_two_player(&mut crusade);
    assert_eq!(crusade.state.objects[&target].zone, Zone::Exile);
    assert_eq!(crusade.state.objects[&enchantment].zone, Zone::Exile);
}
