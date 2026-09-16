//! Issue #315 — the reviewed three-card creature activated-tapper cohort.
//!
//! These scenarios exercise the generated battlefield abilities through the authoritative command
//! path. CR 602.1/602.2 and 118.1-118.3 govern activation and atomic mana payment; CR 302.6
//! governs the creature tap cost and summoning sickness; CR 113.7a makes each ability independent
//! after activation; CR 608.2b revalidates its target on resolution.

use super::helpers::*;
use prost::Message;
use tricerules_cards::{CardFaceId, CardRegistry, ManaCost};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::CanonicalGameplayCommand;

fn creature_engine(seed: u64, source_card: &str, target_card: &str) -> (GameEngine, u32, u32) {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, source_card);
    let target = inject_creature_on_battlefield(&mut engine, 1, target_card);
    (engine, source, target)
}

/// Brightfield Mustang is a real pinned-Standard Mount, but it is intentionally not part of the
/// three-card generated cohort. Give the battlefield fixture its exact relevant printed
/// characteristics through the engine's CR 707 copiable-values path so this test exercises a real
/// Mount characteristic rather than relying on Three Tree Mascot's Changeling CDA.
fn install_brightfield_mustang_face(engine: &mut GameEngine, object_id: u32) {
    let mut face = CardRegistry::global()
        .get("grizzly_bears")
        .expect("fixture source face")
        .primary_face()
        .clone();
    face.face_id = CardFaceId::new("brightfield_mustang").expect("canonical Mount face id");
    face.name = "Brightfield Mustang".into();
    face.mana_cost = ManaCost::parse("{3}{W}").expect("Mount mana cost");
    face.types = vec!["Creature".into(), "Horse".into(), "Mount".into()];
    face.power = Some(3);
    face.toughness = Some(3);
    let object = engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("Mount fixture object");
    object.card_id = "brightfield_mustang".into();
    object.power = face.power;
    object.toughness = face.toughness;
    object.copiable_values = Some(tricerules_core::state::CopiableValues {
        source_card_id: "brightfield_mustang".into(),
        source_face_index: 0,
        display_name: face.name.clone(),
        face,
        room_faces: None,
    });
}

fn resolve_two_player(engine: &mut GameEngine) {
    resolve_entire_stack_two_player(engine);
}

fn three_player_main1(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));
    while engine.state.turn_step != TurnStep::Main1 {
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("advance three-player turn");
    }
    engine
}

fn resolve_three_player(engine: &mut GameEngine) {
    while !engine.state.stack.is_empty() {
        for _ in 0..engine.state.players.len() {
            if engine.state.stack.is_empty() {
                break;
            }
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("three-player priority pass");
        }
    }
}

fn canonical_command(inner: RuledCommand) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CanonicalGameplay(CanonicalGameplayCommand {
            command: inner.encode_to_vec(),
            auto_pass_policies: Vec::new(),
        })),
    }
}

#[test]
fn issue_315_each_variant_pays_exact_mana_and_taps_source_then_target() {
    for (seed, card_id, mana) in [
        (
            315_001,
            "coeurl",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            315_002,
            "frostbridge_guard",
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            315_003,
            "sterling_keykeeper",
            ManaGift {
                c: 2,
                ..Default::default()
            },
        ),
    ] {
        let (mut engine, source, target) = creature_engine(seed, card_id, "grizzly_bears");
        give_mana(&mut engine, 0, mana);
        engine
            .apply_command(
                0,
                &activate_ability_for(&engine, source, 0, target_object(target)),
            )
            .unwrap_or_else(|error| panic!("{card_id} should activate: {error}"));

        assert!(
            engine.state.objects[&source].tapped,
            "{card_id} pays its tap cost"
        );
        assert!(
            !engine.state.objects[&target].tapped,
            "effect waits for resolution"
        );
        assert_eq!(
            engine.state.players[0].mana_pool.white, 0,
            "{card_id} white payment"
        );
        assert_eq!(
            engine.state.players[0].mana_pool.colorless, 0,
            "{card_id} generic payment"
        );
        assert_eq!(engine.state.stack.len(), 1);

        resolve_two_player(&mut engine);
        assert!(
            engine.state.objects[&target].tapped,
            "{card_id} resolves Tap"
        );
        assert!(engine.state.objects[&source].tapped);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn issue_315_mana_and_tap_costs_are_atomic_and_cover_summoning_sickness() {
    let (mut wrong_color, source, target) =
        creature_engine(315_004, "frostbridge_guard", "grizzly_bears");
    give_mana(
        &mut wrong_color,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let pool_before = wrong_color.state.players[0].mana_pool;
    let command_index_before = wrong_color.state.command_index;
    wrong_color
        .apply_command(
            0,
            &activate_ability_for(&wrong_color, source, 0, target_object(target)),
        )
        .expect_err("generic mana cannot pay Frostbridge Guard's white pip");
    assert_eq!(wrong_color.state.players[0].mana_pool, pool_before);
    assert_eq!(wrong_color.state.command_index, command_index_before);
    assert!(!wrong_color.state.objects[&source].tapped);
    assert!(!wrong_color.state.objects[&target].tapped);
    assert!(wrong_color.state.stack.is_empty());

    let (mut short, source, target) = creature_engine(315_005, "coeurl", "grizzly_bears");
    give_mana(
        &mut short,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let pool_before = short.state.players[0].mana_pool;
    short
        .apply_command(
            0,
            &activate_ability_for(&short, source, 0, target_object(target)),
        )
        .expect_err("one white mana cannot pay Coeurl's generic pip");
    assert_eq!(short.state.players[0].mana_pool, pool_before);
    assert!(!short.state.objects[&source].tapped);
    assert!(short.state.stack.is_empty());

    let (mut sick, source, target) =
        creature_engine(315_006, "sterling_keykeeper", "grizzly_bears");
    sick.state.objects.get_mut(&source).unwrap().summoning_sick = true;
    give_mana(
        &mut sick,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let pool_before = sick.state.players[0].mana_pool;
    let command_index_before = sick.state.command_index;
    sick.apply_command(
        0,
        &activate_ability_for(&sick, source, 0, target_object(target)),
    )
    .expect_err("a summoning-sick creature cannot pay its tap cost");
    assert_eq!(sick.state.players[0].mana_pool, pool_before);
    assert_eq!(sick.state.command_index, command_index_before);
    assert!(!sick.state.objects[&source].tapped);
    assert!(sick.state.stack.is_empty());

    let (mut tapped, source, target) =
        creature_engine(315_007, "sterling_keykeeper", "grizzly_bears");
    tapped.state.objects.get_mut(&source).unwrap().tapped = true;
    give_mana(
        &mut tapped,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let pool_before = tapped.state.players[0].mana_pool;
    tapped
        .apply_command(
            0,
            &activate_ability_for(&tapped, source, 0, target_object(target)),
        )
        .expect_err("an already-tapped source cannot pay its tap cost");
    assert_eq!(tapped.state.players[0].mana_pool, pool_before);
    assert!(tapped.state.stack.is_empty());
    assert!(!tapped.state.objects[&target].tapped);
}

#[test]
fn issue_315_target_filter_is_checked_at_activation_and_revalidated_on_resolution() {
    let (mut coeurl, source, enchantment_creature) =
        creature_engine(315_008, "coeurl", "lucent_liminid");
    give_mana(
        &mut coeurl,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    coeurl
        .apply_command(
            0,
            &activate_ability_for(&coeurl, source, 0, target_object(enchantment_creature)),
        )
        .expect_err("Coeurl excludes an Enchantment creature at activation");
    assert!(!coeurl.state.objects[&source].tapped);
    assert!(!coeurl.state.objects[&enchantment_creature].tapped);
    assert!(coeurl.state.stack.is_empty());

    let (mut keykeeper, source, changeling) =
        creature_engine(315_009, "sterling_keykeeper", "three_tree_mascot");
    give_mana(
        &mut keykeeper,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    keykeeper
        .apply_command(
            0,
            &activate_ability_for(&keykeeper, source, 0, target_object(changeling)),
        )
        .expect_err("a changeling has the current Mount subtype and is excluded");
    assert!(!keykeeper.state.objects[&source].tapped);
    assert!(keykeeper.state.stack.is_empty());

    let (mut frostbridge, source, changeling) =
        creature_engine(315_010, "frostbridge_guard", "three_tree_mascot");
    give_mana(
        &mut frostbridge,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    frostbridge
        .apply_command(
            0,
            &activate_ability_for(&frostbridge, source, 0, target_object(changeling)),
        )
        .expect("Frostbridge Guard accepts any creature, including a changeling");
    resolve_two_player(&mut frostbridge);
    assert!(frostbridge.state.objects[&changeling].tapped);
}

#[test]
fn issue_315_target_filters_cover_real_mount_artifact_creature_and_noncreature() {
    let (mut keykeeper, source, mount) =
        creature_engine(315_011, "sterling_keykeeper", "brightfield_mustang");
    install_brightfield_mustang_face(&mut keykeeper, mount);
    assert!(
        keykeeper
            .characteristics(mount)
            .is_some_and(|characteristics| {
                characteristics.has_type("Mount") && !characteristics.all_creature_types
            }),
        "the target fixture must be a real Mount, not only a changeling"
    );
    give_mana(
        &mut keykeeper,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let pool_before = keykeeper.state.players[0].mana_pool;
    keykeeper
        .apply_command(
            0,
            &activate_ability_for(&keykeeper, source, 0, target_object(mount)),
        )
        .expect_err("Sterling Keykeeper excludes a real Mount");
    assert_eq!(keykeeper.state.players[0].mana_pool, pool_before);
    assert!(!keykeeper.state.objects[&source].tapped);
    assert!(!keykeeper.state.objects[&mount].tapped);
    assert!(keykeeper.state.stack.is_empty());

    for (seed, card_id, mana, artifact_id) in [
        (
            3_150_111,
            "coeurl",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            "iron_myr",
        ),
        (
            3_150_112,
            "frostbridge_guard",
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            "yotian_soldier",
        ),
    ] {
        let (mut engine, source, artifact_creature) = creature_engine(seed, card_id, artifact_id);
        assert!(
            engine
                .characteristics(artifact_creature)
                .is_some_and(|characteristics| {
                    characteristics.is_creature() && characteristics.is_artifact()
                }),
            "{artifact_id} must be an artifact creature"
        );
        give_mana(&mut engine, 0, mana);
        engine
            .apply_command(
                0,
                &activate_ability_for(&engine, source, 0, target_object(artifact_creature)),
            )
            .expect("artifact creatures are legal for the unrestricted creature filters");
        resolve_two_player(&mut engine);
        assert!(engine.state.objects[&source].tapped);
        assert!(engine.state.objects[&artifact_creature].tapped);
    }

    for (seed, card_id, mana) in [
        (
            3_150_113,
            "coeurl",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            3_150_114,
            "frostbridge_guard",
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            3_150_115,
            "sterling_keykeeper",
            ManaGift {
                c: 2,
                ..Default::default()
            },
        ),
    ] {
        let (mut engine, source, noncreature) = creature_engine(seed, card_id, "forest");
        assert!(
            engine
                .characteristics(noncreature)
                .is_some_and(|characteristics| !characteristics.is_creature()),
            "forest must remain a noncreature permanent"
        );
        give_mana(&mut engine, 0, mana);
        let pool_before = engine.state.players[0].mana_pool;
        let command_index_before = engine.state.command_index;
        engine
            .apply_command(
                0,
                &activate_ability_for(&engine, source, 0, target_object(noncreature)),
            )
            .expect_err("all creature tappers reject a noncreature target atomically");
        assert_eq!(engine.state.players[0].mana_pool, pool_before);
        assert_eq!(engine.state.command_index, command_index_before);
        assert!(!engine.state.objects[&source].tapped);
        assert!(!engine.state.objects[&noncreature].tapped);
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn issue_315_unrestricted_target_remains_legal_after_real_control_change() {
    let mut engine = GameEngine::new(3_150_116, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "frostbridge_guard");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 1, "ray_of_command");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, source, 0, target_object(target)),
        )
        .expect("activate before the target changes controller");
    assert!(engine.state.objects[&source].tapped);
    assert!(!engine.state.objects[&target].tapped);

    engine
        .apply_command(0, &pass())
        .expect("activator passes so the opponent can change control");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let ray_of_command = hand_index_for_card(&engine, 1, "ray_of_command");
    engine
        .apply_command(1, &cast_spell(ray_of_command, target_object(target)))
        .expect("cast Ray of Command on the pending ability target");
    resolve_two_player(&mut engine);

    assert_eq!(engine.state.objects[&target].controller, 1);
    assert!(engine.state.players[1].battlefield.contains(&target));
    assert!(engine.state.objects[&target].tapped);
    assert!(engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_315_accepts_already_tapped_targets_and_a_legal_self_target() {
    let (mut already_tapped, source, target) =
        creature_engine(315_012, "frostbridge_guard", "grizzly_bears");
    already_tapped
        .state
        .objects
        .get_mut(&target)
        .unwrap()
        .tapped = true;
    give_mana(
        &mut already_tapped,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    already_tapped
        .apply_command(
            0,
            &activate_ability_for(&already_tapped, source, 0, target_object(target)),
        )
        .expect("Tap can target an already-tapped creature");
    resolve_two_player(&mut already_tapped);
    assert!(already_tapped.state.objects[&target].tapped);

    let (mut self_target, source, _) = creature_engine(315_013, "coeurl", "grizzly_bears");
    give_mana(
        &mut self_target,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    self_target
        .apply_command(
            0,
            &activate_ability_for(&self_target, source, 0, target_object(source)),
        )
        .expect("Coeurl is a legal nonenchantment creature target for itself");
    assert!(
        self_target.state.objects[&source].tapped,
        "the source pays {{T}}"
    );
    resolve_two_player(&mut self_target);
    assert!(self_target.state.objects[&source].tapped);
    assert!(self_target.state.stack.is_empty());
}

#[test]
fn issue_315_controller_priority_and_multiplayer_control_are_engine_authoritative() {
    let (mut priority, source, target) =
        creature_engine(315_014, "sterling_keykeeper", "grizzly_bears");
    priority
        .apply_command(0, &pass())
        .expect("active controller passes priority");
    assert_eq!(priority.state.priority_player_id(), 1);
    give_mana(
        &mut priority,
        1,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let opponent_pool = priority.state.players[1].mana_pool;
    let command_index_before = priority.state.command_index;
    priority
        .apply_command(
            1,
            &activate_ability_for(&priority, source, 0, target_object(target)),
        )
        .expect_err("the opponent cannot activate another player's source");
    assert_eq!(priority.state.players[1].mana_pool, opponent_pool);
    assert_eq!(priority.state.command_index, command_index_before);
    assert!(priority.state.stack.is_empty());
    assert!(!priority.state.objects[&source].tapped);
    assert_eq!(priority.state.priority_player_id(), 1);

    priority
        .apply_command(1, &pass())
        .expect("opponent passes priority back");
    give_mana(
        &mut priority,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    priority
        .apply_command(
            0,
            &activate_ability_for(&priority, source, 0, target_object(target)),
        )
        .expect("the controlling player can activate at priority");
    resolve_two_player(&mut priority);
    assert!(priority.state.objects[&target].tapped);

    let mut multiplayer = three_player_main1(315_015);
    let source =
        inject_creature_under_foreign_control(&mut multiplayer, 0, 2, "sterling_keykeeper");
    let target = inject_creature_on_battlefield(&mut multiplayer, 1, "grizzly_bears");
    let owner_pool = multiplayer.state.players[0].mana_pool;
    multiplayer
        .apply_command(
            0,
            &activate_ability_for(&multiplayer, source, 0, target_object(target)),
        )
        .expect_err("the owner is not the controller and cannot activate");
    assert_eq!(multiplayer.state.players[0].mana_pool, owner_pool);
    assert!(multiplayer.state.stack.is_empty());

    multiplayer
        .apply_command(0, &pass())
        .expect("active player passes");
    multiplayer
        .apply_command(1, &pass())
        .expect("next player passes to the controller");
    give_mana(
        &mut multiplayer,
        2,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    multiplayer
        .apply_command(
            2,
            &activate_ability_for(&multiplayer, source, 0, target_object(target)),
        )
        .expect("the third player controls and activates the source");
    assert_eq!(multiplayer.state.objects[&source].owner, 0);
    assert_eq!(multiplayer.state.objects[&source].controller, 2);
    resolve_three_player(&mut multiplayer);
    assert!(multiplayer.state.objects[&target].tapped);
    assert!(multiplayer.state.objects[&source].tapped);
}

#[test]
fn issue_315_source_zone_is_battlefield_only_and_actual_zone_changes_keep_summoning_sickness() {
    let (mut engine, source, target) = creature_engine(315_016, "coeurl", "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, source, 0, target_object(target)),
        )
        .expect("activate before moving the source");
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(source)))
        .expect("cast Unsummon targeting the ability source");
    resolve_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert!(engine.state.objects[&target].tapped);

    let generation = engine.state.zone_change_generation[&source];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let pool_before = engine.state.players[0].mana_pool;
    let command_index_before = engine.state.command_index;
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, source, 0, target_object(target)),
        )
        .expect_err("the generated ability is unavailable from its hand");
    assert_eq!(engine.state.players[0].mana_pool, pool_before);
    assert_eq!(engine.state.command_index, command_index_before);
    assert_eq!(engine.state.zone_change_generation[&source], generation);
    assert!(engine.state.stack.is_empty());

    let hand_index = hand_index_for_card(&engine, 0, "coeurl");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(hand_index, Vec::new()))
        .expect("cast the same physical source from hand");
    resolve_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(engine.state.objects[&source].summoning_sick);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    let pool_before = engine.state.players[0].mana_pool;
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, source, 0, target_object(target)),
        )
        .expect_err("the recast creature still has summoning sickness");
    assert_eq!(engine.state.players[0].mana_pool, pool_before);
    assert!(engine.state.objects[&source].summoning_sick);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_315_ability_is_independent_after_source_leaves_and_target_revalidation_uses_generations() {
    let (mut source_leaves, source, target) =
        creature_engine(315_017, "frostbridge_guard", "grizzly_bears");
    inject_card_into_hand(&mut source_leaves, 0, "unsummon");
    give_mana(
        &mut source_leaves,
        0,
        ManaGift {
            w: 1,
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    source_leaves
        .apply_command(
            0,
            &activate_ability_for(&source_leaves, source, 0, target_object(target)),
        )
        .expect("activate before the source leaves");
    let unsummon = hand_index_for_card(&source_leaves, 0, "unsummon");
    source_leaves
        .apply_command(0, &cast_spell(unsummon, target_object(source)))
        .expect("return the source while its ability is on the stack");
    resolve_two_player(&mut source_leaves);
    assert_eq!(source_leaves.state.objects[&source].zone, Zone::Hand);
    assert!(source_leaves.state.objects[&target].tapped);

    let (mut target_leaves, source, target) =
        creature_engine(315_018, "frostbridge_guard", "grizzly_bears");
    inject_card_into_hand(&mut target_leaves, 1, "unsummon");
    give_mana(
        &mut target_leaves,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    target_leaves
        .apply_command(
            0,
            &activate_ability_for(&target_leaves, source, 0, target_object(target)),
        )
        .expect("target is legal when the ability is activated");
    target_leaves
        .apply_command(0, &pass())
        .expect("the activator passes so the target controller can respond");
    let old_generation = target_leaves
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    give_mana(
        &mut target_leaves,
        1,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&target_leaves, 1, "unsummon");
    target_leaves
        .apply_command(1, &cast_spell(unsummon, target_object(target)))
        .expect("move the target to a new real zone generation");
    resolve_two_player(&mut target_leaves);
    assert_eq!(target_leaves.state.objects[&target].zone, Zone::Hand);
    assert!(target_leaves.state.zone_change_generation[&target] > old_generation);
    assert!(!target_leaves.state.objects[&target].tapped);
    assert!(target_leaves.state.objects[&source].tapped);
    assert!(target_leaves.state.stack.is_empty());
}

#[test]
fn issue_315_stale_source_generation_is_rejected_before_payment_after_recast() {
    let (mut engine, source, target) =
        creature_engine(315_020, "sterling_keykeeper", "grizzly_bears");
    let stale_command = activate_ability_for(&engine, source, 0, target_object(target));

    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(source)))
        .expect("cast Unsummon to create the first source zone change");
    resolve_two_player(&mut engine);
    let hand_generation = engine.state.zone_change_generation[&source];
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);

    let source_hand_index = hand_index_for_card(&engine, 0, "sterling_keykeeper");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &cast_spell(source_hand_index, Vec::new()))
        .expect("cast the same physical source for a new battlefield object generation");
    resolve_two_player(&mut engine);
    let fresh_generation = engine.state.zone_change_generation[&source];
    assert_ne!(fresh_generation, hand_generation);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let pool_before = engine.state.players[0].mana_pool;
    let command_index_before = engine.state.command_index;
    engine
        .apply_command(0, &stale_command)
        .expect_err("the old battlefield generation cannot be replayed");
    assert_eq!(engine.state.players[0].mana_pool, pool_before);
    assert_eq!(engine.state.command_index, command_index_before);
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, source, 0, target_object(target)),
        )
        .expect("a fresh generation can activate");
    assert!(engine.state.objects[&source].tapped);
    resolve_two_player(&mut engine);
    assert!(engine.state.objects[&target].tapped);
}

#[test]
fn issue_315_canonical_replay_produces_identical_tap_state_and_responses() {
    fn replay() -> (Vec<Vec<u8>>, bool, bool, u64) {
        let (mut engine, source, target) =
            creature_engine(315_019, "sterling_keykeeper", "grizzly_bears");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let first = engine.state.priority_player_id();
        let second = if first == 0 { 1 } else { 0 };
        let commands = [
            (
                0,
                canonical_command(activate_ability_for(
                    &engine,
                    source,
                    0,
                    target_object(target),
                )),
            ),
            (first, canonical_command(pass())),
            (second, canonical_command(pass())),
        ];
        let mut responses = Vec::new();
        for (player, command) in commands {
            responses.push(
                engine
                    .apply_command(player, &command)
                    .expect("replay command accepted")
                    .encode_to_vec(),
            );
        }
        (
            responses,
            engine.state.objects[&source].tapped,
            engine.state.objects[&target].tapped,
            engine.state.command_index,
        )
    }

    assert_eq!(replay(), replay());
    let (responses, source_tapped, target_tapped, command_index) = replay();
    assert!(source_tapped);
    assert!(target_tapped);
    assert_eq!(responses.len(), 3);
    assert!(command_index >= 3);
}
