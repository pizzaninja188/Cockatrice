//! Actual-card scenarios for the queue-v18 Aura enters-draw pump cohort.
//!
//! Expectations are printed Oracle values. CR 303.4a attaches the Aura; CR 603.6a
//! triggers the enters ability; CR 121.1 draws one card; CR 613.4c applies the static
//! pump; CR 704.5m cleans up an unattached Aura. Saved Aura rulings require no draw
//! when the target is illegal on resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone, RuledCommand};

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "chosen_by_heliod",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            pump: (0, 2),
        },
        Case {
            id: "scavenged_weaponry",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            pump: (1, 1),
        },
        Case {
            id: "scourgemark",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            pump: (1, 0),
        },
        Case {
            id: "sheltering_boughs",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            pump: (1, 3),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_aura(e: &mut GameEngine, card_id: &str, mana: ManaGift, target: u32) -> u32 {
    give_mana(e, 0, mana);
    let _slot = inject_card_into_hand(e, 0, card_id);
    let hand_slot = hand_index_for_card(e, 0, card_id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{card_id}: exact printed cost");
    battlefield_object_for_card(e, 0, card_id)
}

fn cast_until_trigger(e: &mut GameEngine, card_id: &str, mana: ManaGift, target: u32) -> u32 {
    give_mana(e, 0, mana);
    let _slot = inject_card_into_hand(e, 0, card_id);
    let hand_slot = hand_index_for_card(e, 0, card_id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error:?}"));
    e.apply_scenario_command(0, &pass()).expect("caster pass");
    e.apply_scenario_command(1, &pass()).expect("opponent pass");
    let aura = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| e.state.objects[oid].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} entered"));
    assert_eq!(e.state.stack.len(), 1, "{card_id}: entry trigger waits");
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

#[test]
fn aura_etb_draw_pump_batch_cards_attach_draw_one_and_apply_the_printed_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_830_000 + index as u64);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let top = e.state.players[0].library[0];
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        let aura = cast_aura(&mut e, case.id, gift(&case.mana), creature);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: the Aura left hand and its controller drew one",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: exactly one card was drawn",
            case.id
        );
        assert!(
            e.state.players[0].hand.contains(&top),
            "{}: the drawn card is the former top card",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: the opponent did not draw",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: the opponent's library is unchanged",
            case.id
        );
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(creature)),
            "{}: attached",
            case.id
        );
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: exact printed pump",
            case.id
        );
    }
}

#[test]
fn aura_etb_draw_pump_batch_cards_do_not_draw_when_the_aura_fizzles() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_831_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        give_mana(&mut e, 0, gift(&case.mana));
        let _slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let aura_slot = e.state.players[0].hand[hand_slot];
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(creature)))
            .expect("cast at the creature");
        move_to_graveyard(&mut e, 1, creature);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&aura_slot].zone,
            Zone::Graveyard,
            "{}: the Aura spell fizzles",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: no draw without an entering Aura",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}: the library is unchanged",
            case.id
        );
    }
}

#[test]
fn aura_etb_draw_pump_batch_cards_still_draw_when_the_aura_leaves_before_the_trigger_resolves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_832_000 + index as u64);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let _aura = cast_until_trigger(&mut e, case.id, gift(&case.mana), creature);
        let card_name = tricerules_cards::registry::global()
            .get(case.id)
            .expect("known card")
            .name
            .clone();
        dev_move_card(&mut e, 0, &card_name, DevZone::Graveyard);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: the entering trigger still draws",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: exactly one card was drawn",
            case.id
        );
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(2), Some(2)),
            "{}: the static pump left with the Aura",
            case.id
        );
    }
}

#[test]
fn aura_etb_draw_pump_batch_cards_keep_the_static_pump_after_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_833_000 + index as u64);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let aura = cast_aura(&mut e, case.id, gift(&case.mana), creature);
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: the static Aura pump remains",
            case.id
        );
    }
}
