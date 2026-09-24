//! Red Guardian uses the engine-owned source-damage history, not damage received.
//!
//! Pinned Scryfall `oracle_cards` SHA-256: 9611b5d93b20478a0ee46bae8b20a9eb39ee980f0ef4f5f6f6aaa8f7ab010ab2.
//! Card-specific ruling and Oracle text: official Marvel Super Heroes Release Notes.
//! Governed concepts: CR 120.2b/120.4 (damage source), 400.7 (new object after a zone
//! change), 603.6a (ETB trigger), 608.2b (target revalidation), and 608.2i (historical facts).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, DevCommand, DevMoveCard, DevZone, RuledCommand,
};

const RED_GUARDIAN: &str = "red_guardian,_super-soldier";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn dev_move(engine: &mut GameEngine, actor: i32, owner: i32, zone: DevZone, name: &str) {
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .expect("development move the named card");
}

fn cast_red_guardian_and_park_entry_trigger(engine: &mut GameEngine) -> u32 {
    inject_card_into_hand(engine, 0, RED_GUARDIAN);
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, RED_GUARDIAN);
    let guardian = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Red Guardian with flash");
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&guardian].zone, Zone::Battlefield);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    guardian
}

#[test]
fn red_guardian_destroys_a_damage_source_not_a_damage_recipient() {
    let mut engine = engine(487_001);
    let damage_source = inject_creature_on_battlefield(&mut engine, 1, "prodigal_sorcerer");
    let own_sorcerer = inject_creature_on_battlefield(&mut engine, 0, "prodigal_sorcerer");
    let departed_recipient = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 1, 1);
    let recipient_only = inject_creature_with_stats(&mut engine, 1, "hill_giant", 3, 3);

    // The opponent's Sorcerer deals positive damage to a creature that then leaves the
    // battlefield. The damage-source history remains attached to the Sorcerer's incarnation.
    engine
        .apply_command(0, &pass())
        .expect("pass to the opponent");
    apply_ability(
        &mut engine,
        1,
        damage_source,
        0,
        target_object(departed_recipient),
    )
    .expect("opponent activates Prodigal Sorcerer");
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&departed_recipient].zone,
        Zone::Graveyard
    );

    // The controller's creature deals damage to the opponent's other creature. That creature
    // received damage but is not a damage source and must not be offered as Red Guardian's target.
    apply_ability(
        &mut engine,
        0,
        own_sorcerer,
        0,
        target_object(recipient_only),
    )
    .expect("controller activates Prodigal Sorcerer");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&recipient_only].damage, 1);

    let guardian = cast_red_guardian_and_park_entry_trigger(&mut engine);
    let ability_key = u64::from(guardian) << 32;
    let published = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
        [&ability_key]
        .groups[0]
        .valid_permanent_ids;
    assert_eq!(published, &[damage_source]);
    assert!(!published.contains(&recipient_only));
    assert!(!published.contains(&own_sorcerer));

    engine
        .apply_command(0, &choose_trigger_target(damage_source))
        .expect("choose the opponent creature that dealt damage");

    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&damage_source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&departed_recipient].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&recipient_only].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&guardian].zone, Zone::Battlefield);
}

#[test]
fn red_guardian_does_not_offer_a_returned_damage_source_before_targeting() {
    let mut engine = engine(487_002);
    let damage_source = inject_creature_on_battlefield(&mut engine, 1, "prodigal_sorcerer");
    let recipient = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 1, 1);

    engine
        .apply_command(0, &pass())
        .expect("pass to the opponent");
    apply_ability(&mut engine, 1, damage_source, 0, target_object(recipient))
        .expect("opponent activates Prodigal Sorcerer");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&recipient].zone, Zone::Graveyard);

    engine.enable_dev_commands();
    dev_move(&mut engine, 0, 1, DevZone::Exile, "Prodigal Sorcerer");
    dev_move(&mut engine, 0, 1, DevZone::Battlefield, "Prodigal Sorcerer");
    let returned_generation = engine.state.zone_change_generation[&damage_source];

    inject_card_into_hand(&mut engine, 0, RED_GUARDIAN);
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, RED_GUARDIAN);
    let guardian = engine.state.players[0].hand[slot];
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Red Guardian with flash");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&guardian].zone, Zone::Battlefield);

    // With no legal target, CR 603.3d keeps the triggered ability off the stack. The returned
    // incarnation cannot supply its old source-damage event as a target.
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
    let ability_key = u64::from(guardian) << 32;
    assert!(
        !engine.initial_response_batch().legal_by_player[&0]
            .valid_targets_by_ability
            .contains_key(&ability_key),
        "a returned source incarnation cannot be offered for its old damage event"
    );
    assert_eq!(
        engine.state.zone_change_generation[&damage_source],
        returned_generation
    );
}

#[test]
fn red_guardian_revalidates_a_selected_source_after_its_generation_changes() {
    let mut engine = engine(487_003);
    let damage_source = inject_creature_on_battlefield(&mut engine, 1, "prodigal_sorcerer");
    let recipient = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 1, 1);

    engine
        .apply_command(0, &pass())
        .expect("pass to the opponent");
    apply_ability(&mut engine, 1, damage_source, 0, target_object(recipient))
        .expect("opponent activates Prodigal Sorcerer");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&recipient].zone, Zone::Graveyard);

    let guardian = cast_red_guardian_and_park_entry_trigger(&mut engine);
    let ability_key = u64::from(guardian) << 32;
    let published = &engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
        [&ability_key]
        .groups[0]
        .valid_permanent_ids;
    assert_eq!(published, &[damage_source]);
    engine
        .apply_command(0, &choose_trigger_target(damage_source))
        .expect("choose the opponent creature that dealt damage");

    // CR 400.7 makes the returned card a new object; CR 608.2b revalidates the chosen target.
    let generation = engine
        .state
        .zone_change_generation
        .get(&damage_source)
        .copied()
        .unwrap_or(0);
    engine.enable_dev_commands();
    dev_move(&mut engine, 0, 1, DevZone::Exile, "Prodigal Sorcerer");
    dev_move(&mut engine, 0, 1, DevZone::Battlefield, "Prodigal Sorcerer");
    assert!(engine.state.zone_change_generation[&damage_source] > generation);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&damage_source].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&guardian].zone, Zone::Battlefield);
}
