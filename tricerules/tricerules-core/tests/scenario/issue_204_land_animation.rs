//! Issue #204: duration-aware land self-animation.
//!
//! Oracle and rulings were verified 2026-09-03 for Restless Reef and Soulstone Sanctuary.
//! Governing rules: CR 205.1b, 205.3m, 302.6, 611.2a, 613.1d-f, 613.4b, and 613.7.

use crate::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
};
use tricerules_cards::{CardRegistry, Color, Keyword};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    BattlefieldObject, ChooseTriggerTarget, DevCommand, DevMoveCard, DevZone,
};

fn engine_with_card(seed: u64, card_id: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card_id]), island_only_deck()]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn published_object(engine: &mut GameEngine, player: usize, oid: u32) -> BattlefieldObject {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .get(player)
                .and_then(|view| {
                    view.battlefield_objects
                        .iter()
                        .find(|object| object.object_id == oid)
                })
                .cloned(),
            _ => None,
        })
        .expect("published battlefield object")
}

fn add_effect(
    engine: &mut GameEngine,
    oid: u32,
    kind: ContinuousEffectKind,
    duration: EffectDuration,
) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration,
        timestamp: engine.state.command_index,
    });
}

fn add_external_creature_form(engine: &mut GameEngine, oid: u32) {
    add_effect(
        engine,
        oid,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec!["Shark".into()],
        }),
        EffectDuration::Indefinite,
    );
    add_effect(
        engine,
        oid,
        ContinuousEffectKind::Layer7bSetPt {
            power: 4,
            toughness: 4,
        },
        EffectDuration::Indefinite,
    );
}

fn move_same_object_through_hand(engine: &mut GameEngine, player: usize, oid: u32) {
    let player_id = engine.state.players[player].id;
    let card_name = CardRegistry::global()
        .get(&engine.state.objects[&oid].card_id)
        .expect("registered card")
        .name
        .clone();
    let move_to = |zone, ready| RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player_id,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: card_name.clone(),
                zone: zone as i32,
                ready,
            })),
        })),
    };
    engine.enable_dev_commands();
    engine
        .apply_command(player_id, &move_to(DevZone::Hand, false))
        .expect("move animated land to hand");
    assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
    engine
        .apply_command(player_id, &move_to(DevZone::Battlefield, true))
        .expect("return the same land to the battlefield");
    assert!(engine.state.players[player].battlefield.contains(&oid));
}

#[test]
fn land_self_animation_cards_are_registered() {
    let registry = CardRegistry::global();
    for card_id in ["restless_reef", "soulstone_sanctuary"] {
        assert!(
            registry.get(card_id).is_some(),
            "issue #204 card {card_id} must be registered"
        );
    }
}

#[test]
fn restless_reef_publishes_land_creature_land_creature_row_contract() {
    let mut engine = engine_with_card(204_001, "restless_reef");
    let reef = move_ready_to_battlefield(&mut engine, 0, "restless_reef");
    assert!(engine.state.objects[&reef].tapped, "the land enters tapped");

    let land = published_object(&mut engine, 0, reef);
    assert!(land.is_land);
    assert!(!land.is_creature);

    let command_index = engine.state.command_index;
    let unfunded = activate_ability_for(&engine, reef, 1, vec![]);
    assert!(engine.apply_command(0, &unfunded).is_err());
    assert_eq!(engine.state.command_index, command_index);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, reef, 1, vec![]).expect("activate Restless Reef");
    resolve_entire_stack_two_player(&mut engine);

    let animated = engine
        .characteristics(reef)
        .expect("animated characteristics");
    assert!(animated.has_type("Land") && animated.is_creature());
    assert!(animated.has_type("Shark"));
    assert_eq!(animated.colors, vec![Color::Blue, Color::Black]);
    assert_eq!((animated.power, animated.toughness), (Some(4), Some(4)));
    assert!(animated.keywords.contains(&Keyword::Deathtouch));
    let creature_row = published_object(&mut engine, 0, reef);
    assert!(creature_row.is_land && creature_row.is_creature);
    assert_eq!((creature_row.power, creature_row.toughness), (4, 4));
    assert!(creature_row.keywords.contains(&"Deathtouch".into()));

    end_active_turn(&mut engine, 0);
    let expired = engine
        .characteristics(reef)
        .expect("expired characteristics");
    assert!(expired.has_type("Land"));
    assert!(!expired.is_creature());
    assert_eq!((expired.power, expired.toughness), (None, None));
    let land_row_again = published_object(&mut engine, 0, reef);
    assert!(land_row_again.is_land);
    assert!(!land_row_again.is_creature);
    assert_eq!((land_row_again.power, land_row_again.toughness), (0, 0));
    assert!(!land_row_again.keywords.contains(&"Deathtouch".into()));

    advance_to_main1_from_game_start(&mut engine);
    engine
        .apply_command(1, &pass())
        .expect("active opponent passes priority");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, reef, 1, vec![]).expect("reactivate Restless Reef");
    resolve_entire_stack_two_player(&mut engine);
    let creature_row_again = published_object(&mut engine, 0, reef);
    assert!(creature_row_again.is_land && creature_row_again.is_creature);
    assert_eq!(
        (creature_row_again.power, creature_row_again.toughness),
        (4, 4)
    );
}

#[test]
fn restless_reef_attack_trigger_works_after_external_animation() {
    let mut engine = engine_with_card(204_002, "restless_reef");
    let reef = relocate_to_battlefield(&mut engine, 0, "restless_reef", false);
    add_external_creature_form(&mut engine, reef);

    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    let library_before = engine.state.players[1].library.len();
    engine
        .apply_command(0, &declare_attackers(vec![reef]))
        .expect("declare externally animated Reef");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: target_player(1),
                    ..Default::default()
                })),
            },
        )
        .expect("target opponent with Reef trigger");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].library.len(), library_before - 4);
}

#[test]
fn soulstone_animation_is_indefinite_all_types_and_resets_on_zone_change() {
    let mut engine = engine_with_card(204_003, "soulstone_sanctuary");
    let sanctuary = relocate_to_battlefield(&mut engine, 0, "soulstone_sanctuary", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, sanctuary, 1, vec![]).expect("activate Soulstone");
    resolve_entire_stack_two_player(&mut engine);

    let animated = engine
        .characteristics(sanctuary)
        .expect("animated characteristics");
    assert!(animated.has_type("Land") && animated.is_creature());
    assert!(animated.all_creature_types);
    assert!(animated.has_type("Shark") && animated.has_type("Ninja"));
    assert_eq!((animated.power, animated.toughness), (Some(3), Some(3)));
    assert!(animated.keywords.contains(&Keyword::Vigilance));

    end_active_turn(&mut engine, 0);
    assert!(engine
        .characteristics(sanctuary)
        .expect("persistent characteristics")
        .is_creature());

    move_same_object_through_hand(&mut engine, 0, sanctuary);
    let reset = engine
        .characteristics(sanctuary)
        .expect("reset characteristics");
    assert!(reset.has_type("Land"));
    assert!(!reset.is_creature());
    assert!(!reset.all_creature_types);
    assert_eq!((reset.power, reset.toughness), (None, None));
    let land_row = published_object(&mut engine, 0, sanctuary);
    assert!(land_row.is_land && !land_row.is_creature);
}

#[test]
fn reanimation_uses_layer_timestamps_and_temporary_forms_restore_prior_effects() {
    let mut persistent = engine_with_card(204_006, "soulstone_sanctuary");
    let sanctuary = relocate_to_battlefield(&mut persistent, 0, "soulstone_sanctuary", false);
    give_mana(
        &mut persistent,
        0,
        ManaGift {
            c: 8,
            ..Default::default()
        },
    );
    apply_ability(&mut persistent, 0, sanctuary, 1, vec![]).expect("first animation");
    resolve_entire_stack_two_player(&mut persistent);
    add_effect(
        &mut persistent,
        sanctuary,
        ContinuousEffectKind::Layer4SetCreatureTypes(vec!["Frog".into()]),
        EffectDuration::Indefinite,
    );
    add_effect(
        &mut persistent,
        sanctuary,
        ContinuousEffectKind::Layer7bSetPt {
            power: 1,
            toughness: 1,
        },
        EffectDuration::Indefinite,
    );
    let later_form = persistent
        .characteristics(sanctuary)
        .expect("later characteristic effects");
    assert!(!later_form.all_creature_types);
    assert!(later_form.has_type("Frog"));
    assert_eq!((later_form.power, later_form.toughness), (Some(1), Some(1)));

    apply_ability(&mut persistent, 0, sanctuary, 1, vec![]).expect("reactivate Soulstone");
    resolve_entire_stack_two_player(&mut persistent);
    let reanimated = persistent
        .characteristics(sanctuary)
        .expect("newer animation");
    assert!(reanimated.all_creature_types);
    assert_eq!((reanimated.power, reanimated.toughness), (Some(3), Some(3)));

    let mut temporary = engine_with_card(204_007, "restless_reef");
    let reef = relocate_to_battlefield(&mut temporary, 0, "restless_reef", false);
    add_effect(
        &mut temporary,
        reef,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec!["Frog".into()],
        }),
        EffectDuration::Indefinite,
    );
    add_effect(
        &mut temporary,
        reef,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Green]),
        EffectDuration::Indefinite,
    );
    add_effect(
        &mut temporary,
        reef,
        ContinuousEffectKind::Layer7bSetPt {
            power: 2,
            toughness: 2,
        },
        EffectDuration::Indefinite,
    );
    give_mana(
        &mut temporary,
        0,
        ManaGift {
            u: 1,
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut temporary, 0, reef, 1, vec![]).expect("animate over prior form");
    resolve_entire_stack_two_player(&mut temporary);
    let reef_form = temporary.characteristics(reef).expect("Restless Reef form");
    assert!(reef_form.has_type("Shark") && !reef_form.has_type("Frog"));
    assert_eq!(reef_form.colors, vec![Color::Blue, Color::Black]);
    assert_eq!((reef_form.power, reef_form.toughness), (Some(4), Some(4)));

    end_active_turn(&mut temporary, 0);
    let restored = temporary
        .characteristics(reef)
        .expect("restored prior form");
    assert!(restored.has_type("Frog") && !restored.has_type("Shark"));
    assert_eq!(restored.colors, vec![Color::Green]);
    assert_eq!((restored.power, restored.toughness), (Some(2), Some(2)));
    assert!(!restored.keywords.contains(&Keyword::Deathtouch));
}

#[test]
fn animation_revalidates_source_generation_and_summoning_sickness() {
    let mut stale = engine_with_card(204_004, "restless_reef");
    let reef = relocate_to_battlefield(&mut stale, 0, "restless_reef", false);
    give_mana(
        &mut stale,
        0,
        ManaGift {
            u: 1,
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut stale, 0, reef, 1, vec![]).expect("put animation on stack");
    move_same_object_through_hand(&mut stale, 0, reef);
    resolve_entire_stack_two_player(&mut stale);
    assert!(!stale
        .characteristics(reef)
        .expect("new Reef object")
        .is_creature());

    let mut sick = engine_with_card(204_005, "soulstone_sanctuary");
    let sanctuary = relocate_to_battlefield(&mut sick, 0, "soulstone_sanctuary", false);
    sick.state
        .objects
        .get_mut(&sanctuary)
        .expect("sanctuary")
        .summoning_sick = true;
    give_mana(
        &mut sick,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut sick, 0, sanctuary, 1, vec![]).expect("animation has no tap cost");
    resolve_entire_stack_two_player(&mut sick);
    assert_eq!(
        zone_view_ability_flags(&mut sick, 0, sanctuary),
        vec![false, true]
    );
    assert!(apply_ability(&mut sick, 0, sanctuary, 0, vec![]).is_err());
}
