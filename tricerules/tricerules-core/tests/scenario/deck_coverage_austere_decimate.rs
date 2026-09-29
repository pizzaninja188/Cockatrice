//! Actual-card coverage for Austere Command's choose-two destruction modes and Decimate's four
//! required target instances, including one artifact creature chosen for both relevant targets.
//!
//! CR 601.2b-c, 608.2b, and 700.2 govern mode/target announcement, target rechecks, and modal
//! resolution order. WotC's Austere Command ruling specifies sequential modes; its Decimate ruling
//! confirms that a permanent with multiple card types can fill multiple target instances.

use super::helpers::*;
use tricerules_cards::CardRegistry;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::TargetRef;

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn target_in_group(group_index: u32, object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        ..Default::default()
    }
}

#[test]
fn both_card_definitions_have_exact_identity_and_choice_shapes() {
    let registry = CardRegistry::global();

    assert_eq!(
        registry.id_for_name("Austere Command"),
        Some("austere_command")
    );
    let austere = registry
        .get("austere_command")
        .expect("Austere Command definition");
    let austere_face = &austere.faces[0];
    assert_eq!(austere_face.mana_cost.to_string(), "{4}{W}{W}");
    assert_eq!(
        austere_face
            .types
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Sorcery"]
    );
    let modal = austere_face.modal_spell.as_ref().expect("choose two modes");
    assert_eq!((modal.min_modes, modal.max_modes), (2, 2));
    assert_eq!(modal.modes.len(), 4);
    assert_eq!(
        modal
            .modes
            .iter()
            .map(|mode| mode.mode_id.as_str())
            .collect::<Vec<_>>(),
        [
            "artifacts",
            "enchantments",
            "creatures_mv_3_or_less",
            "creatures_mv_4_or_greater"
        ]
    );

    assert_eq!(registry.id_for_name("Decimate"), Some("decimate"));
    let decimate = registry.get("decimate").expect("Decimate definition");
    let decimate_face = &decimate.faces[0];
    assert_eq!(decimate_face.mana_cost.to_string(), "{2}{R}{G}");
    assert_eq!(
        decimate_face
            .types
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Sorcery"]
    );
    assert_eq!(decimate_face.spell_effect.len(), 4);
    assert_eq!(
        decimate_face
            .targeting
            .as_ref()
            .expect("four required target groups")
            .groups
            .len(),
        4
    );
}

#[test]
fn austere_command_destroys_artifacts_and_creatures_with_mana_value_three_or_less() {
    let mut engine = engine(940_101);
    let card = inject_card_into_hand(&mut engine, 0, "austere_command");
    let own_artifact = inject_permanent_on_battlefield(&mut engine, 0, "short_sword");
    let opposing_artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let low_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing_low_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let low_artifact_creature = inject_creature_on_battlefield(&mut engine, 0, "yotian_soldier");
    let high_creature = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    let opposing_high_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "ominous_seas");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    grant_pool(&mut engine, 0);

    let hand_slot = hand_index_for_card(&engine, 0, "austere_command");
    engine
        .apply_command(
            0,
            &cast_modal_spell(hand_slot, vec![(0, vec![]), (2, vec![])]),
        )
        .expect("choose artifact and mana-value-three-or-less modes");
    resolve_entire_stack_two_player(&mut engine);

    for destroyed in [
        card,
        own_artifact,
        opposing_artifact,
        low_creature,
        opposing_low_creature,
        low_artifact_creature,
    ] {
        assert_eq!(engine.state.objects[&destroyed].zone, Zone::Graveyard);
    }
    for preserved in [high_creature, opposing_high_creature, enchantment, land] {
        assert_eq!(engine.state.objects[&preserved].zone, Zone::Battlefield);
    }
}

#[test]
fn austere_command_destroys_enchantments_and_creatures_with_mana_value_four_or_greater() {
    let mut engine = engine(940_102);
    let card = inject_card_into_hand(&mut engine, 0, "austere_command");
    let own_enchantment = inject_permanent_on_battlefield(&mut engine, 0, "ominous_seas");
    let opposing_enchantment = inject_permanent_on_battlefield(&mut engine, 1, "ominous_seas");
    let own_high_creature = inject_creature_on_battlefield(&mut engine, 0, "hill_giant");
    let opposing_high_creature = inject_creature_on_battlefield(&mut engine, 1, "hill_giant");
    let low_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let low_artifact_creature = inject_creature_on_battlefield(&mut engine, 1, "yotian_soldier");
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    grant_pool(&mut engine, 0);

    let hand_slot = hand_index_for_card(&engine, 0, "austere_command");
    engine
        .apply_command(
            0,
            // Modes resolve in printed order even when the command lists them in reverse.
            &cast_modal_spell(hand_slot, vec![(3, vec![]), (1, vec![])]),
        )
        .expect("choose enchantment and mana-value-four-or-greater modes");
    let chosen = &engine
        .state
        .stack
        .last()
        .expect("spell on stack")
        .chosen_modes;
    assert_eq!(
        chosen
            .iter()
            .map(|mode| mode.mode_id.as_str())
            .collect::<Vec<_>>(),
        ["enchantments", "creatures_mv_4_or_greater"]
    );
    resolve_entire_stack_two_player(&mut engine);

    for destroyed in [
        card,
        own_enchantment,
        opposing_enchantment,
        own_high_creature,
        opposing_high_creature,
    ] {
        assert_eq!(engine.state.objects[&destroyed].zone, Zone::Graveyard);
    }
    for preserved in [low_creature, low_artifact_creature, artifact, land] {
        assert_eq!(engine.state.objects[&preserved].zone, Zone::Battlefield);
    }
}

#[test]
fn austere_command_rejects_too_few_or_repeated_modes_without_consuming_the_card() {
    let mut engine = engine(940_103);
    let card = inject_card_into_hand(&mut engine, 0, "austere_command");
    grant_pool(&mut engine, 0);
    let hand_slot = hand_index_for_card(&engine, 0, "austere_command");

    assert!(engine
        .apply_command(0, &cast_modal_spell(hand_slot, vec![(0, vec![])]))
        .is_err());
    assert!(engine
        .apply_command(
            0,
            &cast_modal_spell(hand_slot, vec![(0, vec![]), (0, vec![])]),
        )
        .is_err());
    assert_eq!(engine.state.objects[&card].zone, Zone::Hand);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn decimate_rejects_a_wrong_type_then_allows_one_artifact_creature_for_two_targets() {
    let mut engine = engine(940_104);
    let card = inject_card_into_hand(&mut engine, 0, "decimate");
    let artifact_creature = inject_creature_on_battlefield(&mut engine, 0, "yotian_soldier");
    let wrong_artifact_target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "ominous_seas");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    grant_pool(&mut engine, 0);

    let hand_slot = hand_index_for_card(&engine, 0, "decimate");
    let missing_group_targets = vec![
        target_in_group(0, artifact_creature),
        target_in_group(1, artifact_creature),
        target_in_group(2, enchantment),
    ];
    assert!(engine
        .apply_command(0, &cast_spell(hand_slot, missing_group_targets))
        .is_err());
    assert_eq!(engine.state.objects[&card].zone, Zone::Hand);
    assert!(engine.state.stack.is_empty());

    let invalid_targets = vec![
        target_in_group(0, wrong_artifact_target),
        target_in_group(1, wrong_artifact_target),
        target_in_group(2, enchantment),
        target_in_group(3, land),
    ];
    assert!(engine
        .apply_command(0, &cast_spell(hand_slot, invalid_targets))
        .is_err());
    assert_eq!(engine.state.objects[&card].zone, Zone::Hand);
    assert!(engine.state.stack.is_empty());
    for permanent in [artifact_creature, wrong_artifact_target, enchantment, land] {
        assert_eq!(engine.state.objects[&permanent].zone, Zone::Battlefield);
    }

    let legal_targets = vec![
        target_in_group(0, artifact_creature),
        target_in_group(1, artifact_creature),
        target_in_group(2, enchantment),
        target_in_group(3, land),
    ];
    engine
        .apply_command(0, &cast_spell(hand_slot, legal_targets))
        .expect("artifact creature satisfies Decimate's artifact and creature targets");
    resolve_entire_stack_two_player(&mut engine);

    for destroyed in [card, artifact_creature, enchantment, land] {
        assert_eq!(engine.state.objects[&destroyed].zone, Zone::Graveyard);
    }
    assert_eq!(
        engine.state.objects[&wrong_artifact_target].zone,
        Zone::Battlefield
    );
}
