//! Exact deck-corpus coverage for Stump Stomp // Burnwillow Clearing.
//!
//! Scryfall Oracle and rulings checked 2026-09-25. CR 712.11b-c governs choosing the spell face;
//! CR 712.12 governs choosing the land face. CR 120.2b identifies the creature as the damage
//! source, CR 608.2b rechecks both targets at resolution, CR 614.1c governs entering tapped, and
//! CR 605.1a/605.3b govern the land's immediate mana ability.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;

const STUMP_STOMP: &str = "stump_stomp_burnwillow_clearing";

fn permanent_target(object_id: u32, group_index: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    }
}

fn stump_targets(source: u32, recipient: u32) -> Vec<TargetRef> {
    vec![permanent_target(source, 0), permanent_target(recipient, 1)]
}

fn stump_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn leave_battlefield_to_hand(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].hand.push(object_id);
    engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn mana_option(engine: &GameEngine, source: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.mana_option_index = option;
    command
}

fn advance_to_controller_next_main1(engine: &mut GameEngine) {
    end_active_turn(engine, 0);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 1);
    end_active_turn(engine, 1);
    advance_to_main1_from_game_start(engine);
    assert_eq!(engine.state.active_player_id(), 0);
}

#[test]
fn stump_stomp_uses_two_targets_and_only_the_source_deals_damage() {
    for (recipient_card, recipient_is_planeswalker, seed) in [
        ("jace_beleren", true, 20_260_928),
        ("colossal_dreadmaw", false, 20_260_933),
    ] {
        let mut engine = stump_engine(seed);
        let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let recipient = if recipient_is_planeswalker {
            inject_permanent_on_battlefield(&mut engine, 1, recipient_card)
        } else {
            inject_creature_with_stats(&mut engine, 1, recipient_card, 6, 6)
        };
        if recipient_is_planeswalker {
            engine
                .state
                .objects
                .get_mut(&recipient)
                .unwrap()
                .set_counter(CounterKind::Loyalty, 6);
        }
        inject_card_into_hand(&mut engine, 0, STUMP_STOMP);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, STUMP_STOMP);

        let response = engine.initial_response_batch();
        let legal = response.legal_by_player[&0]
            .valid_targets_by_hand_slot
            .get(&((slot as u32) << 8));

        semantic::accepted(
            &mut engine,
            0,
            &cast_spell_face(slot, stump_targets(source, recipient), 0),
        );

        let legal = legal.expect("the engine publishes both Stump Stomp target groups");
        assert_eq!(legal.groups.len(), 2);
        assert_eq!(legal.groups[0].valid_permanent_ids, [source]);
        assert_eq!(legal.groups[1].valid_permanent_ids, [recipient]);
        assert_eq!(legal.groups[1].distinct_from_group_indices, [0]);

        engine
            .state
            .objects
            .get_mut(&source)
            .unwrap()
            .set_counter(CounterKind::PlusOnePlusOne, 1);
        resolve_entire_stack_two_player(&mut engine);

        if recipient_is_planeswalker {
            assert_eq!(
                engine.state.objects[&recipient].counter_count(CounterKind::Loyalty),
                3,
                "the planeswalker loses loyalty equal to the source creature's current power"
            );
        } else {
            assert_eq!(engine.state.objects[&recipient].damage, 3);
            assert_eq!(engine.state.objects[&recipient].zone, Zone::Battlefield);
        }
        assert_eq!(
            engine.state.objects[&source].damage, 0,
            "Stump Stomp is not a fight"
        );
    }
}

#[test]
fn stump_stomp_deals_no_damage_if_either_target_leaves_before_resolution() {
    for (invalid_group, seed) in [(0, 20_260_929), (1, 20_260_930)] {
        let mut engine = stump_engine(seed);
        let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
        engine
            .state
            .objects
            .get_mut(&walker)
            .unwrap()
            .set_counter(CounterKind::Loyalty, 6);
        inject_card_into_hand(&mut engine, 0, STUMP_STOMP);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, STUMP_STOMP);
        semantic::accepted(
            &mut engine,
            0,
            &cast_spell_face(slot, stump_targets(source, walker), 0),
        );

        if invalid_group == 0 {
            leave_battlefield_to_hand(&mut engine, 0, source);
        } else {
            leave_battlefield_to_hand(&mut engine, 1, walker);
        }
        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(engine.state.objects[&source].damage, 0);
        assert_eq!(
            engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
            6,
            "neither target group can deliver partial damage"
        );
    }
}

#[test]
fn burnwillow_clearing_enters_tapped_and_produces_either_color_after_untapping() {
    for (option, red, green, seed) in [(0, 1, 0, 20_260_931), (1, 0, 1, 20_260_932)] {
        let mut engine = stump_engine(seed);
        let land = inject_card_into_hand(&mut engine, 0, STUMP_STOMP);
        let slot = hand_index_for_card(&engine, 0, STUMP_STOMP);
        semantic::accepted(&mut engine, 0, &play_land_face(slot, 1));

        assert_eq!(engine.state.objects[&land].face_up_index, 1);
        assert!(
            engine.state.objects[&land].tapped,
            "Burnwillow Clearing enters tapped"
        );
        engine
            .apply_command(0, &mana_option(&engine, land, option))
            .expect_err("a tapped land cannot pay the mana ability's tap cost");

        advance_to_controller_next_main1(&mut engine);
        assert!(
            !engine.state.objects[&land].tapped,
            "the land untaps normally"
        );
        let command = mana_option(&engine, land, option);
        semantic::accepted(&mut engine, 0, &command);
        assert_eq!(engine.state.players[0].mana_pool.red, red);
        assert_eq!(engine.state.players[0].mana_pool.green, green);
        assert!(
            engine.state.stack.is_empty(),
            "the mana ability resolves immediately"
        );
        assert!(engine.state.objects[&land].tapped);
    }
}
