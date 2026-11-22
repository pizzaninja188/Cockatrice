//! Actual-card scenarios for the reviewed flash / hexproof Aura cohort.
//!
//! Expectations are printed Oracle values. CR 702.8a permits flash timing; CR 303.4a
//! defines the Aura's controller-only target; CR 603.6a/611.2a grant hexproof until end of
//! turn; CR 702.18a-b stops opponents' targeting; CR 613.4c applies the attached pump;
//! CR 704.5m and 608.2b cover illegal attachment and fizzle; CR 514.2 ends the temporary
//! grant while the static pump remains. The saved Cradle of Safety and Starlit Mantle
//! rulings require the last-enchanted creature to keep hexproof when the Aura leaves
//! before its enters trigger resolves.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone, RuledCommand};

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump: (u32, u32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "accelerated_evolution",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            pump: (2, 2),
        },
        Case {
            id: "cradle_of_safety",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            pump: (1, 1),
        },
        Case {
            id: "starlit_mantle",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            pump: (1, 1),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn cast_and_resolve_aura(e: &mut GameEngine, id: &str, target: u32, mana: ManaGift) -> u32 {
    give_mana(e, 0, mana);
    let _slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {id}: {error:?}"));
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, id)
}

fn cast_until_trigger(e: &mut GameEngine, id: &str, target: u32, mana: ManaGift) -> u32 {
    give_mana(e, 0, mana);
    let _slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {id}: {error:?}"));
    e.apply_scenario_command(0, &pass()).expect("caster pass");
    e.apply_scenario_command(1, &pass()).expect("opponent pass");
    let aura = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == id)
        .unwrap_or_else(|| panic!("{id} entered"));
    assert_eq!(e.state.stack.len(), 1, "{id}: entry trigger waits");
    aura
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn dev_move_card(e: &mut GameEngine, player_id: i32, card_name: &str, zone: DevZone) {
    e.enable_dev_commands();
    e.apply_scenario_command(
        player_id,
        &RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: player_id,
                dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                    card_name: card_name.into(),
                    zone: zone as i32,
                    ready: true,
                })),
            })),
        },
    )
    .expect("dev move card");
}

fn mana_pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn flash_hexproof_aura_batch_cards_attach_grant_hexproof_and_keep_the_static_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_630_000 + index as u64);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
        let aura = cast_and_resolve_aura(&mut e, case.id, creature, gift(&case.mana));
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(creature)),
            "{}: attached",
            case.id
        );
        assert_eq!(
            e.effective_power(creature),
            Some(2 + case.pump.0),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(2 + case.pump.1),
            "{}",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::Hexproof),
            "{}",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(3), Some(3)),
            "{}: only the enchanted creature",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::Hexproof),
            "{}: only the enchanted creature",
            case.id
        );
        assert_eq!(
            mana_pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed mana cost",
            case.id
        );
    }
}

#[test]
fn flash_hexproof_aura_batch_cards_use_flash_timing_on_an_opponent_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_631_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        end_active_turn(&mut e, 0);
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.priority_player_id(),
            e.state.players[1].id,
            "{}: opponent has priority",
            case.id
        );
        e.apply_scenario_command(e.state.players[1].id, &pass())
            .expect("opponent passes priority");
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(creature)))
            .unwrap_or_else(|error| panic!("{}: flash cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Battlefield,
            "{}: flash resolved on the opponent's turn",
            case.id
        );
    }
}

#[test]
fn flash_hexproof_aura_batch_cards_require_a_creature_you_control_as_the_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_632_000 + index as u64);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let pool_before = mana_pool(&e);
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let legal = &e.initial_response_batch().legal_by_player[&0].valid_targets_by_hand_slot
            [&((hand_slot as u32) << 8)]
            .groups[0]
            .valid_permanent_ids;
        assert_eq!(legal, &[own], "{}: only the caster's creature", case.id);
        for illegal in [opponent, land] {
            assert!(
                e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(illegal)))
                    .is_err(),
                "{}: {illegal} is not a legal Aura target",
                case.id
            );
        }
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert_eq!(mana_pool(&e), pool_before, "{}: no mana spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn flash_hexproof_aura_batch_cards_stop_opponents_from_targeting_the_enchanted_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_633_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast_and_resolve_aura(&mut e, case.id, creature, gift(&case.mana));
        inject_card_into_hand(&mut e, 1, "moment_of_triumph");
        let removal_slot = hand_index_for_card(&e, 1, "moment_of_triumph");
        e.apply_scenario_command(0, &pass())
            .expect("active player passes");
        assert!(
            e.apply_scenario_command(1, &cast_spell(removal_slot, target_object(creature)))
                .is_err(),
            "{}: hexproof stops the opponent's targeted spell",
            case.id
        );
        e.apply_scenario_command(1, &pass())
            .expect("opponent returns priority");
    }
}

#[test]
fn flash_hexproof_aura_batch_cards_keep_hexproof_on_the_last_enchanted_creature_when_the_aura_leaves(
) {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_634_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let _aura = cast_until_trigger(&mut e, case.id, creature, gift(&case.mana));
        let card_name = tricerules_cards::registry::global()
            .get(case.id)
            .expect("known card")
            .name
            .clone();
        dev_move_card(&mut e, 0, &card_name, DevZone::Graveyard);
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.effective_has_keyword(creature, Keyword::Hexproof),
            "{}: the last-enchanted creature still gains hexproof",
            case.id
        );
        assert_eq!(
            e.effective_power(creature),
            Some(2),
            "{}: the pump is gone with the Aura",
            case.id
        );
    }
}

#[test]
fn flash_hexproof_aura_batch_cards_expire_hexproof_at_cleanup_but_keep_the_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_635_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast_and_resolve_aura(&mut e, case.id, creature, gift(&case.mana));
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Hexproof),
            "{}: hexproof expires at cleanup",
            case.id
        );
        assert_eq!(
            e.effective_power(creature),
            Some(2 + case.pump.0),
            "{}: static pump remains",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(2 + case.pump.1),
            "{}",
            case.id
        );
    }
}

#[test]
fn flash_hexproof_aura_batch_cards_fizzle_when_the_creature_left_before_the_aura_resolves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_636_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(creature)))
            .expect("cast");
        move_to_graveyard(&mut e, 0, creature);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: fizzled Aura goes to its owner's graveyard",
            case.id
        );
        assert_eq!(e.state.objects[&slot].attached_to, None, "{}", case.id);
        assert!(
            e.state.players[0].graveyard.contains(&slot),
            "{}: owner's graveyard",
            case.id
        );
        assert!(e.state.continuous_effects.is_empty(), "{}", case.id);
    }
}
