use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};
use tricerules_core::state::{AffectedScope, ContinuousEffect, SpellCastMethod};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cast_cost_group_selection::SelectedObject, CastCostGroupSelection, CastCostOptionKind,
    CastMethod, CastSpell, RuledCommand,
};

fn setup_harmonize(seed: u64) -> (GameEngine, u32, u32, u64) {
    let decks = Some(vec![
        deck_with("island", &["unending_whisper", "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let whisper = take_oid_from_library_or_hand(&mut engine, 0, "unending_whisper");
    engine.state.players[0].graveyard.push(whisper);
    engine.state.objects.get_mut(&whisper).unwrap().zone = Zone::Graveyard;
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.state.objects.get_mut(&bear).unwrap().summoning_sick = true;
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);
    (engine, whisper, bear, generation)
}

fn harmonize_cast(whisper: u32, bear: Option<(u32, u64)>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(graveyard_cast_source(whisper)),
            cast_method: CastMethod::Harmonize as i32,
            cast_cost_group_selections: bear
                .map(|(object_id, generation)| {
                    vec![CastCostGroupSelection {
                        group_index: 0,
                        option_index: 0,
                        selected_object: Some(SelectedObject::PermanentId(object_id)),
                        expected_zone_change_generation: generation,
                    }]
                })
                .unwrap_or_default(),
            ..Default::default()
        })),
    }
}

#[test]
fn harmonize_publishes_owner_only_candidates_and_commits_tap_with_reduced_payment() {
    let (mut engine, whisper, bear, generation) = setup_harmonize(105_001);
    let island = relocate_to_battlefield(&mut engine, 0, "island", false);
    let tapped = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&tapped).unwrap().tapped = true;
    let zero_power = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&zero_power).unwrap().power = Some(0);
    let negative_power = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(negative_power),
        kind: ContinuousEffectKind::PtModify {
            delta_power: -4,
            delta_toughness: 0,
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    let opponent_bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.players[0].mana_pool.colorless = 3;
    let legal_batch = engine
        .apply_command(0, &activate_ability(island, 0, vec![]))
        .expect("activate Island for the blue pip");
    let action = legal_batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| {
            action.object_id == whisper && action.cast_method == CastMethod::Harmonize as i32
        })
        .expect("owner Harmonize action");
    assert_eq!(action.cost, "{5}{U}");
    let group = &action.cost_choices.as_ref().unwrap().cast_cost_groups[0];
    assert_eq!(group.skip_label, "Pay full Harmonize cost");
    assert_eq!(
        group.options[0].kind,
        CastCostOptionKind::TapPermanentForGenericReduction as i32
    );
    let option = &group.options[0];
    assert_eq!(
        option.valid_permanent_ids,
        [bear, zero_power, negative_power]
    );
    assert_eq!(option.valid_permanent_generations[0], generation);
    assert_eq!(option.valid_permanent_generic_reductions, [2, 0, 0]);
    assert!(!option.valid_permanent_ids.contains(&island));
    assert!(!option.valid_permanent_ids.contains(&tapped));
    assert!(!option.valid_permanent_ids.contains(&opponent_bear));
    assert!(!legal_batch.legal_by_player[&1]
        .zone_cast_actions
        .iter()
        .any(|action| action.object_id == whisper));

    let before_hand = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &harmonize_cast(whisper, Some((bear, generation))))
        .expect("pay {3}{U} and tap the summoning-sick Bear");
    assert!(engine.state.objects[&bear].tapped);
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Stack);
    assert_eq!(
        engine.state.stack.last().unwrap().cast_method,
        SpellCastMethod::Harmonize
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].hand.len(), before_hand + 1);
}

#[test]
fn full_cost_harmonize_does_not_tap_a_candidate() {
    let (mut engine, whisper, bear, _) = setup_harmonize(105_004);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 5,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &harmonize_cast(whisper, None))
        .expect("pay the full {5}{U}");
    assert!(!engine.state.objects[&bear].tapped);
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Stack);
}

#[test]
fn harmonize_obeys_sorcery_timing_and_current_priority() {
    let (mut engine, whisper, _, _) = setup_harmonize(105_009);
    let island = inject_permanent_on_battlefield(&mut engine, 0, "island");
    engine.state.turn_step = tricerules_core::TurnStep::Upkeep;
    let upkeep = engine
        .apply_command(0, &activate_ability(island, 0, vec![]))
        .expect("activate mana ability in upkeep");
    assert!(!upkeep.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .any(|action| action.object_id == whisper));

    engine.state.turn_step = tricerules_core::TurnStep::Main1;
    let passed = engine.apply_command(0, &pass()).expect("pass priority");
    assert!(!passed.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .any(|action| action.object_id == whisper));
}

#[test]
fn missing_or_wrong_cast_method_is_rejected_for_the_source_zone() {
    let (mut engine, whisper, _, _) = setup_harmonize(105_005);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 5,
            ..Default::default()
        },
    );
    let mut missing = harmonize_cast(whisper, None);
    let Some(Cmd::CastSpell(cast)) = missing.cmd.as_mut() else {
        panic!("cast command")
    };
    cast.cast_method = CastMethod::Unspecified as i32;
    assert!(engine.apply_command(0, &missing).is_err());
    let mut normal = harmonize_cast(whisper, None);
    let Some(Cmd::CastSpell(cast)) = normal.cmd.as_mut() else {
        panic!("cast command")
    };
    cast.cast_method = CastMethod::Normal as i32;
    assert!(engine.apply_command(0, &normal).is_err());
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Graveyard);
}

#[test]
fn forged_or_unaffordable_harmonize_is_atomic() {
    let (mut engine, whisper, bear, generation) = setup_harmonize(105_006);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "island");
    let opponent_bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    assert!(engine
        .apply_command(0, &harmonize_cast(whisper, Some((land, 0))))
        .is_err());
    assert!(!engine.state.objects[&bear].tapped);
    assert!(!engine.state.objects[&land].tapped);
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Graveyard);

    assert!(engine
        .apply_command(0, &harmonize_cast(whisper, Some((opponent_bear, 0))))
        .is_err());
    assert!(!engine.state.objects[&opponent_bear].tapped);
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Graveyard);

    engine.state.objects.get_mut(&bear).unwrap().tapped = true;
    assert!(engine
        .apply_command(0, &harmonize_cast(whisper, Some((bear, generation))))
        .is_err());
    assert!(engine.state.objects[&bear].tapped);
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Graveyard);
    engine.state.objects.get_mut(&bear).unwrap().tapped = false;

    engine.state.players[0].mana_pool.colorless = 2;
    let before_pool = engine.state.players[0].mana_pool;
    assert!(engine
        .apply_command(0, &harmonize_cast(whisper, Some((bear, generation))))
        .is_err());
    assert!(!engine.state.objects[&bear].tapped);
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Graveyard);
    let after_pool = engine.state.players[0].mana_pool;
    assert_eq!(
        (
            after_pool.white,
            after_pool.blue,
            after_pool.black,
            after_pool.red,
            after_pool.green,
            after_pool.colorless,
        ),
        (
            before_pool.white,
            before_pool.blue,
            before_pool.black,
            before_pool.red,
            before_pool.green,
            before_pool.colorless,
        )
    );
}

#[test]
fn stale_harmonize_selection_rejects_without_tapping_spending_or_moving() {
    let (mut engine, whisper, bear, generation) = setup_harmonize(105_002);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    engine
        .state
        .zone_change_generation
        .insert(bear, generation + 1);
    let before_pool = engine.state.players[0].mana_pool;
    let err = engine
        .apply_command(0, &harmonize_cast(whisper, Some((bear, generation))))
        .expect_err("stale creature generation");
    assert!(format!("{err:?}").contains("stale harmonize"));
    assert!(!engine.state.objects[&bear].tapped);
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Graveyard);
    let after_pool = &engine.state.players[0].mana_pool;
    assert_eq!(
        (
            after_pool.white,
            after_pool.blue,
            after_pool.black,
            after_pool.red,
            after_pool.green,
            after_pool.colorless,
        ),
        (
            before_pool.white,
            before_pool.blue,
            before_pool.black,
            before_pool.red,
            before_pool.green,
            before_pool.colorless,
        )
    );
}

#[test]
fn normal_hand_cast_does_not_receive_harmonize_stack_exit_replacement() {
    let decks = Some(vec![
        deck_with("island", &["unending_whisper"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(105_003, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "unending_whisper");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unending_whisper");
    let oid = engine.state.players[0].hand[slot];
    let command = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(ref cast)) = command.cmd else {
        panic!("cast helper")
    };
    assert_eq!(cast.cast_method, CastMethod::Normal as i32);
    engine.apply_command(0, &command).expect("normal cast");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
}

#[test]
fn countered_harmonize_spell_is_exiled() {
    let decks = Some(vec![
        deck_with("island", &["unending_whisper", "grizzly_bears"]),
        deck_with("island", &["counterspell"]),
    ]);
    let mut engine = GameEngine::new(105_007, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let whisper = take_oid_from_library_or_hand(&mut engine, 0, "unending_whisper");
    engine.state.players[0].graveyard.push(whisper);
    engine.state.objects.get_mut(&whisper).unwrap().zone = Zone::Graveyard;
    let bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let generation = engine
        .state
        .zone_change_generation
        .get(&bear)
        .copied()
        .unwrap_or(0);
    relocate_to_hand(&mut engine, 1, "counterspell");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &harmonize_cast(whisper, Some((bear, generation))))
        .expect("cast with Harmonize");
    engine.apply_command(0, &pass()).expect("pass to counterer");
    let counter_slot = hand_index_for_card(&engine, 1, "counterspell");
    engine
        .apply_command(1, &cast_spell(counter_slot, target_object(whisper)))
        .expect("counter Harmonize spell");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&whisper].zone, Zone::Exile);
}

#[test]
fn mammoth_bellow_reduces_only_generic_and_creates_one_five_five_elephant() {
    let decks = Some(vec![
        deck_with("island", &["mammoth_bellow", "serra_angel"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(105_008, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let bellow = take_oid_from_library_or_hand(&mut engine, 0, "mammoth_bellow");
    engine.state.players[0].graveyard.push(bellow);
    engine.state.objects.get_mut(&bellow).unwrap().zone = Zone::Graveyard;
    let angel = relocate_to_battlefield(&mut engine, 0, "serra_angel", false);
    let generation = engine
        .state
        .zone_change_generation
        .get(&angel)
        .copied()
        .unwrap_or(0);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            r: 1,
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &harmonize_cast(bellow, Some((angel, generation))))
        .expect("power four leaves {1}{G}{U}{R}, including every colored pip");
    resolve_entire_stack_two_player(&mut engine);
    let elephants = engine.state.players[0]
        .battlefield
        .iter()
        .filter(|oid| engine.state.objects[oid].card_id == "elephant_g_5_5")
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(elephants.len(), 1);
    assert_eq!(engine.effective_power(elephants[0]), Some(5));
    assert_eq!(engine.effective_toughness(elephants[0]), Some(5));
    assert_eq!(engine.state.objects[&bellow].zone, Zone::Exile);
}

#[test]
fn harmonize_command_replays_with_method_and_selected_permanent() {
    fn replay() -> (Vec<u32>, Vec<u32>, bool, SpellCastMethod) {
        let (mut engine, whisper, bear, generation) = setup_harmonize(105_010);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        );
        let command = harmonize_cast(whisper, Some((bear, generation)));
        let Some(Cmd::CastSpell(cast)) = command.cmd.as_ref() else {
            panic!("cast command")
        };
        assert_eq!(cast.cast_method, CastMethod::Harmonize as i32);
        assert!(matches!(
            cast.cast_cost_group_selections[0].selected_object,
            Some(SelectedObject::PermanentId(oid)) if oid == bear
        ));
        engine.apply_command(0, &command).expect("replay cast");
        let method = engine.state.stack.last().unwrap().cast_method;
        resolve_entire_stack_two_player(&mut engine);
        (
            engine.state.players[0].hand.clone(),
            engine.state.players[0].exile.clone(),
            engine.state.objects[&bear].tapped,
            method,
        )
    }

    assert_eq!(replay(), replay());
}
