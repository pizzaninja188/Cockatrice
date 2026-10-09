//! Actual-card payment, private selection, and zone-incarnation coverage.
use super::helpers::*;
use tricerules_core::Zone;

fn game(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![island_only_deck(), island_only_deck()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn exact_characteristics_and_ability_counts() {
    let registry = tricerules_cards::registry::global();
    for (id, name, cost, types, power, toughness, abilities) in [
        (
            "terrain_generator",
            "Terrain Generator",
            "",
            vec!["Land"],
            None,
            None,
            2,
        ),
        (
            "trash_for_treasure",
            "Trash for Treasure",
            "{2}{R}",
            vec!["Sorcery"],
            None,
            None,
            0,
        ),
    ] {
        let card = registry.get(id).expect("exact card registered");
        assert_eq!(card.name, name);
        assert_eq!(card.face_count(), 1);
        let face = card.primary_face();
        assert_eq!(face.mana_cost.to_string(), cost);
        assert_eq!(face.types, types);
        assert_eq!((face.power, face.toughness), (power, toughness));
        assert_eq!(face.activated_abilities.len(), abilities);
    }
}

#[test]
fn terrain_puts_only_own_basic_hand_land_tapped_or_declines_without_land_play_or_shuffle() {
    for choose in [false, true] {
        let mut engine = game(202609330);
        let terrain = inject_permanent_on_battlefield(&mut engine, 0, "terrain_generator");
        let forest = inject_card_into_hand(&mut engine, 0, "forest");
        let taiga = inject_card_into_hand(&mut engine, 0, "taiga");
        let foreign = inject_card_into_hand(&mut engine, 1, "forest");
        let library = engine.state.players[0].library.clone();
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        apply_ability(&mut engine, 0, terrain, 1, vec![]).unwrap();
        assert!(engine.state.objects[&terrain].tapped);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        pass_both_players(&mut engine);
        let pending = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(pending.deciding_player, 0);
        assert_eq!((pending.presentation.min, pending.presentation.max), (0, 1));
        assert!(pending.presentation.candidates.contains(&forest));
        assert!(!pending.presentation.candidates.contains(&taiga));
        assert!(!pending.presentation.candidates.contains(&foreign));
        let before = format!("{:?}", engine.state.pending_resolution);
        for (actor, object) in [(1, forest), (0, taiga), (0, foreign)] {
            engine
                .apply_command(actor, &submit_resolution_choice(vec![object]))
                .expect_err("invalid actor/nonbasic/foreign hand");
            assert_eq!(format!("{:?}", engine.state.pending_resolution), before);
        }
        let completion = engine
            .apply_command(
                0,
                &submit_resolution_choice(if choose { vec![forest] } else { vec![] }),
            )
            .unwrap();
        assert_eq!(
            engine.state.objects[&forest].zone,
            if choose {
                Zone::Battlefield
            } else {
                Zone::Hand
            }
        );
        if choose {
            assert!(engine.state.objects[&forest].tapped);
        }
        assert_eq!(engine.state.players[0].library, library);
        assert_eq!(engine.state.lands_played_this_turn, 0);
        assert!(!completion
            .events
            .iter()
            .any(|e| matches!(&e.ev, Some(Ev::Log(log)) if log.text.contains("shuffles"))));
        assert!(engine.state.stack.is_empty());
        let slot = hand_index_for_card(&engine, 0, "island");
        engine
            .apply_command(0, &play_land(slot))
            .expect("normal land play remains available");
        assert_eq!(engine.state.lands_played_this_turn, 1);
    }
    let mut engine = game(202609331);
    let terrain = inject_permanent_on_battlefield(&mut engine, 0, "terrain_generator");
    apply_ability(&mut engine, 0, terrain, 0, vec![]).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&terrain].tapped);
    assert!(engine.state.stack.is_empty());
}

fn graveyard(object: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        kind: TargetRefKind::Graveyard as i32,
        object_id: object,
        group_index: 0,
        ..Default::default()
    }]
}

#[test]
fn trash_targets_before_sacrifice_pays_red_and_returns_exact_artifact_with_etb() {
    let mut engine = game(202609334);
    let source = inject_card_into_hand(&mut engine, 0, "trash_for_treasure");
    let fodder = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let target = inject_graveyard_card(&mut engine, 0, "ichor_wellspring");
    let foreign = inject_graveyard_card(&mut engine, 1, "sol_ring");
    let bear = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "trash_for_treasure");
    for illegal in [fodder, foreign, bear] {
        let command = cast_spell_with_costs(
            slot,
            graveyard(illegal),
            vec![permanent_cost_selection(0, fodder)],
        );
        engine
            .apply_command(0, &command)
            .expect_err("target must already be own graveyard artifact");
        assert_eq!(engine.state.players[0].mana_pool.red, 1);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
        assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
        assert_eq!(engine.state.objects[&fodder].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty());
    }
    let command = cast_spell_with_costs(
        slot,
        graveyard(target),
        vec![permanent_cost_selection(0, fodder)],
    );
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    let before = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].hand.len(), before + 1);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&foreign].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Graveyard);
}

#[test]
fn trash_rejects_missing_red_and_revalidates_target_incarnation() {
    let mut engine = game(202609335);
    inject_card_into_hand(&mut engine, 0, "trash_for_treasure");
    let fodder = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let target = inject_graveyard_card(&mut engine, 0, "mind_stone");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "trash_for_treasure");
    let command = cast_spell_with_costs(
        slot,
        graveyard(target),
        vec![permanent_cost_selection(0, fodder)],
    );
    engine
        .apply_command(0, &command)
        .expect_err("generic cannot pay red pip");
    assert_eq!(engine.state.objects[&fodder].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    engine.state.players[0].mana_pool.colorless = 2;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &command).unwrap();
    // Simulate the same physical card leaving and returning to the graveyard: new incarnation.
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 2;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&fodder].zone,
        Zone::Graveyard,
        "paid cost remains paid"
    );
}
