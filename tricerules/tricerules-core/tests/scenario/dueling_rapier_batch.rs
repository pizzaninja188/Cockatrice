//! Actual-card scenarios for the reviewed Dueling Rapier flash Equipment family.
//!
//! Expectations are printed Oracle values. CR 301.5/702.6 govern Equipment and equip;
//! CR 603.6a puts the entry trigger on the stack; CR 701.3 attaches; CR 704.5n detaches
//! an Equipment whose host left; and CR 608.2b removes a trigger whose only target is
//! illegal. The saved ruling sets are empty for all three cards.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    mana: ManaGift,
    equip: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "dueling_rapier",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 4,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
        },
        Case {
            id: "mirran_banesplitter",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 0,
        },
        Case {
            id: "paladins_shield",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 0,
            toughness: 2,
        },
    ]
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

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("forest", &[]),
    ]);
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

fn cast_and_attach(e: &mut GameEngine, case: &Case, creature: u32) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the Equipment");
    pass_both_players(e);
    e.apply_scenario_command(0, &choose_trigger_target(creature))
        .expect("choose the entry attachment target");
    pass_both_players(e);
    battlefield_object_for_card(e, 0, case.id)
}

fn move_battlefield_object_to_graveyard(e: &mut GameEngine, player: usize, object_id: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    e.state.players[player].graveyard.push(object_id);
    e.state.objects.get_mut(&object_id).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object_id).or_insert(0) += 1;
}

#[test]
fn dueling_rapier_batch_cards_attach_grant_and_move_the_printed_bonus() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(840_000 + index as u64, case.id);
        let first = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let equipment = cast_and_attach(&mut e, &case, first);
        assert_eq!(
            e.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(first)),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_power(first),
            Some(2 + case.power),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(first),
            Some(2 + case.toughness),
            "{}",
            case.id
        );

        let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.equip));
        apply_ability(&mut e, 0, equipment, 0, target_object(second))
            .expect("activate equip for the second creature");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(second)),
            "{}",
            case.id
        );
        assert_eq!(e.effective_power(first), Some(2), "{}", case.id);
        assert_eq!(e.effective_toughness(first), Some(2), "{}", case.id);
        assert_eq!(
            e.effective_power(second),
            Some(2 + case.power),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(second),
            Some(2 + case.toughness),
            "{}",
            case.id
        );
    }
}

#[test]
fn dueling_rapier_batch_cards_reject_an_opponents_creature_as_an_equip_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(841_000 + index as u64, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let equipment = cast_and_attach(&mut e, &case, own);
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.equip));
        let before = &e.state.players[0].mana_pool;
        let before_pool = (
            before.white,
            before.blue,
            before.black,
            before.red,
            before.green,
            before.colorless,
        );
        assert!(
            apply_ability(&mut e, 0, equipment, 0, target_object(opponent)).is_err(),
            "{}: an opponent's creature is not a legal equip target",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(own)),
            "{}",
            case.id
        );
        let after = &e.state.players[0].mana_pool;
        assert_eq!(
            (
                after.white,
                after.blue,
                after.black,
                after.red,
                after.green,
                after.colorless,
            ),
            before_pool,
            "{}: the rejected activation spends no mana",
            case.id
        );
    }
}

#[test]
fn dueling_rapier_batch_cards_fizzle_the_entry_attach_when_the_target_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(842_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the Equipment");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &choose_trigger_target(target))
            .expect("choose the entry attachment target");
        move_battlefield_object_to_graveyard(&mut e, 0, target);
        pass_both_players(&mut e);
        let equipment = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&equipment].attached_to, None,
            "{}: a stale entry target leaves the Equipment unattached",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn dueling_rapier_batch_cards_remove_the_entry_trigger_with_no_legal_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(843_000 + index as u64, case.id);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the Equipment");
        pass_both_players(&mut e);
        let equipment = battlefield_object_for_card(&e, 0, case.id);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: no entry trigger remains without a legal creature",
            case.id
        );
        assert_eq!(e.state.objects[&equipment].attached_to, None, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn dueling_rapier_batch_cards_flash_on_an_opponents_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(844_000 + index as u64, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        end_active_turn(&mut e, 0);
        assert_eq!(e.state.active_player_id(), 1, "{}", case.id);
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::Upkeep,
            "{}",
            case.id
        );
        e.apply_scenario_command(1, &pass())
            .expect("opponent passes in upkeep");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast the flash Equipment on the opponent's turn");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &choose_trigger_target(own))
            .expect("choose the entry attachment target");
        pass_both_players(&mut e);
        let equipment = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(own)),
            "{}",
            case.id
        );
    }
}
