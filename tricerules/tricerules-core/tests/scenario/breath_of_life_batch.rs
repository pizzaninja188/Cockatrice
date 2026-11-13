//! Actual-card scenarios for the reviewed creature-reanimation sorcery family.
//!
//! Expectations are printed Oracle values. CR 601.2c requires one creature-card target in the
//! caster's graveyard; CR 110.5b puts it onto the battlefield untapped under its owner's control
//! (110.2/110.2a); CR 608.2b fizzles the spell if that target left before resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "breath_of_life",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "false_defeat",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "resurrection",
            mana: ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "rise_again",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
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

fn engine(seed: u64, case: &Case) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[case.id]),
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

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) {
    ensure_in_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast the reanimation sorcery");
}

#[test]
fn breath_of_life_batch_cards_return_your_creature_untapped() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(910_000 + index as u64, &case);
        let creature = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        cast_at(&mut e, &case, creature);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&creature].controller, 0, "{}", case.id);
        assert_eq!(e.state.objects[&creature].owner, 0, "{}", case.id);
        assert!(!e.state.objects[&creature].tapped, "{}", case.id);
        assert!(
            !e.state.players[0].graveyard.contains(&creature),
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn breath_of_life_batch_cards_reject_noncreature_and_opponent_graveyard_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(911_000 + index as u64, &case);
        let spell = inject_graveyard_card(&mut e, 0, "divination");
        let theirs = inject_graveyard_card(&mut e, 1, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let before = {
            let p = &e.state.players[0].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        };
        for target in [spell, theirs] {
            assert!(
                e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
                    .is_err(),
                "{}: only your creature cards are legal targets",
                case.id
            );
            let p = &e.state.players[0].mana_pool;
            assert_eq!(
                (p.white, p.blue, p.black, p.red, p.green, p.colorless),
                before,
                "{}: the rejected cast spends no mana",
                case.id
            );
            assert!(e.state.stack.is_empty(), "{}", case.id);
        }
        assert!(e.state.players[0]
            .hand
            .iter()
            .any(|oid| e.state.objects[oid].card_id == case.id));
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(
            e.state.objects[&theirs].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
    }
}

#[test]
fn breath_of_life_batch_cards_fizzle_when_the_target_already_left() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(912_000 + index as u64, &case);
        let creature = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        cast_at(&mut e, &case, creature);
        e.state.players[0].graveyard.retain(|oid| *oid != creature);
        e.state.objects.get_mut(&creature).expect("creature").zone = Zone::Exile;
        e.state.players[0].exile.push(creature);
        *e.state.zone_change_generation.entry(creature).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&creature].zone, Zone::Exile, "{}", case.id);
        assert!(
            !e.state.players[0].battlefield.contains(&creature),
            "{}: a stale target is not returned",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
