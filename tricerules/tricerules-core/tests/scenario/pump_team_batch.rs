//! Actual-card scenarios for the exact-text "creatures you control get +N/+N until end of turn"
//! family.
//!
//! Expectations are printed Oracle values. CR 611.2c and the card rulings fix the affected set
//! as the controller's creatures present when the spell resolves; creatures entering later are
//! unaffected. CR 613.4 applies the layer-7c P/T change, and CR 514.2 removes it during cleanup.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    mana: ManaGift,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "banners_raised",
            power: 1,
            toughness: 0,
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
        },
        Case {
            id: "bar_the_door",
            power: 0,
            toughness: 4,
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "burn_bright",
            power: 2,
            toughness: 0,
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "charge",
            power: 1,
            toughness: 1,
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
        },
        Case {
            id: "chorus_of_woe",
            power: 1,
            toughness: 0,
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        Case {
            id: "desperate_charge",
            power: 2,
            toughness: 0,
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "ethereal_guidance",
            power: 2,
            toughness: 1,
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "path_of_angers_flame",
            power: 2,
            toughness: 0,
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "righteous_charge",
            power: 2,
            toughness: 2,
            mana: ManaGift {
                w: 2,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "scare_tactics",
            power: 1,
            toughness: 0,
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        Case {
            id: "shield_wall",
            power: 0,
            toughness: 2,
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "solidarity",
            power: 0,
            toughness: 5,
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "steadfastness",
            power: 0,
            toughness: 3,
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "virtuous_charge",
            power: 1,
            toughness: 1,
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "vitalizing_wind",
            power: 7,
            toughness: 7,
            mana: ManaGift {
                g: 1,
                c: 8,
                ..Default::default()
            },
        },
        Case {
            id: "warriors_charge",
            power: 1,
            toughness: 1,
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "warriors_honor",
            power: 1,
            toughness: 1,
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
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

fn assert_pool_empty(e: &GameEngine, id: &str) {
    let pool = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "{id}: the cast consumed exactly the supplied cost"
    );
}

fn assert_card_moved_to_graveyard(e: &GameEngine, id: &str, object: u32) {
    assert_eq!(
        e.state.objects[&object].zone,
        Zone::Graveyard,
        "{id}: the resolving spell leaves the stack for its owner's graveyard"
    );
    assert!(
        e.state.players[0].graveyard.contains(&object),
        "{id}: the spell card is in its owner's graveyard"
    );
}

#[test]
fn pump_team_cards_apply_exact_printed_delta_to_controlled_creatures() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let id = case.id;
        let mut e = engine(760_000 + case_index as u64);
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let second = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let p0_hand = e.state.players[0].hand.len();
        let p1_hand = e.state.players[1].hand.len();
        let p1_library = e.state.players[1].library.len();
        give_mana(&mut e, 0, case.mana);
        let card = inject_card_into_hand(&mut e, 0, id);
        let hand_slot = hand_index_for_card(&e, 0, id);

        semantic::accepted(&mut e, 0, &cast_spell(hand_slot, vec![]));
        assert_pool_empty(&e, id);
        assert_eq!(
            e.state.stack.len(),
            1,
            "{id}: the spell is on the stack before resolution"
        );
        resolve_entire_stack_two_player(&mut e);

        for object in [first, second] {
            assert_eq!(
                e.effective_power(object),
                Some(2 + case.power),
                "{id}: exact power bonus on a controlled creature"
            );
            assert_eq!(
                e.effective_toughness(object),
                Some(2 + case.toughness),
                "{id}: exact toughness bonus on a controlled creature"
            );
        }
        assert_eq!(
            e.effective_power(opposing),
            Some(2),
            "{id}: an opponent's creature is unchanged"
        );
        assert_eq!(
            e.effective_toughness(opposing),
            Some(2),
            "{id}: an opponent's toughness is unchanged"
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            p0_hand,
            "{id}: casting moves the card out of hand"
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            p1_hand,
            "{id}: the opponent's hand is private and unchanged"
        );
        assert_eq!(
            e.state.players[1].library.len(),
            p1_library,
            "{id}: the opponent's library is unchanged"
        );
        assert_card_moved_to_graveyard(&e, id, card);
    }
}

#[test]
fn pump_team_cards_exclude_later_entrants_and_expire_at_cleanup() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let id = case.id;
        let mut e = engine(761_000 + case_index as u64);
        let existing = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        give_mana(&mut e, 0, case.mana);
        inject_card_into_hand(&mut e, 0, id);
        let hand_slot = hand_index_for_card(&e, 0, id);
        semantic::accepted(&mut e, 0, &cast_spell(hand_slot, vec![]));
        resolve_entire_stack_two_player(&mut e);

        let later = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        assert_eq!(
            e.effective_power(later),
            Some(2),
            "{id}: a creature entering after resolution is not pumped"
        );
        assert_eq!(
            e.effective_toughness(later),
            Some(2),
            "{id}: a later entrant's toughness is unchanged"
        );

        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(existing),
            Some(2),
            "{id}: the pump expires during cleanup"
        );
        assert_eq!(
            e.effective_toughness(existing),
            Some(2),
            "{id}: the toughness pump expires during cleanup"
        );
        assert_eq!(
            e.effective_power(later),
            Some(2),
            "{id}: the later entrant remains unchanged after cleanup"
        );
    }
}
