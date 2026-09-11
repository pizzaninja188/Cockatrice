use super::helpers::*;
use tricerules_core::Zone;

fn setup(seed: u64, source_card: &str, target_card: &str) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("forest", &[source_card]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, source_card, false);
    let target = inject_permanent_on_battlefield(&mut engine, 1, target_card);
    (engine, source, target)
}

#[test]
fn generated_naturalizers_destroy_artifacts_and_enchantments_after_sacrificing() {
    for (seed, source_card, target_card) in [
        (251_001, "cathar_commando", "short_sword"),
        (251_002, "thrashing_brontodon", "exploration"),
    ] {
        let (mut engine, source, target) = setup(seed, source_card, target_card);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );

        apply_ability(&mut engine, 0, source, 0, target_object(target))
            .expect("activate generated Naturalize ability");
        assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
        assert_eq!(engine.state.stack.len(), 1, "ability survives its source");

        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    }
}

#[test]
fn generated_naturalizer_rejects_wrong_targets_and_costs_atomically() {
    let (mut engine, source, artifact) = setup(251_003, "undergrowth_leopard", "short_sword");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    for target in [creature, land] {
        apply_ability(&mut engine, 0, source, 0, target_object(target))
            .expect_err("only artifacts and enchantments are legal targets");
        assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty());
    }

    engine.state.players[0].mana_pool.colorless = 0;
    apply_ability(&mut engine, 0, source, 0, target_object(artifact))
        .expect_err("insufficient mana cannot sacrifice the source");
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());

    engine.state.priority_idx = 1;
    give_mana(
        &mut engine,
        1,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 1, source, 0, target_object(artifact))
        .expect_err("a player cannot sacrifice a permanent they do not control");
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[1].mana_pool.colorless, 1);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn generated_naturalizer_revalidates_its_target_on_resolution() {
    let (mut engine, source, target) = setup(251_004, "voracious_varmint", "short_sword");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, source, 0, target_object(target)).expect("activate");

    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != target);
    engine.state.players[1].hand.push(target);
    engine.state.objects.get_mut(&target).expect("target").zone = Zone::Hand;

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Hand);
    assert!(engine.state.stack.is_empty());
}
