//! Actual-card scenarios for the reviewed Recollect graveyard-to-hand family.
//!
//! Expectations are printed Oracle values. CR 115.1 and 601.2c choose one graveyard-card
//! target; CR 400.7 gives a card that changes zones a new object identity; CR 608.2b
//! fizzles the spell when its only target is illegal on resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::TargetRef;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "elven_cache",
            mana: ManaGift {
                g: 2,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "recollect",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "regrowth",
            mana: ManaGift {
                g: 1,
                c: 1,
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

fn graveyard_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Graveyard as i32,
        ..Default::default()
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn cast_recollect(e: &mut GameEngine, case: &Case, target: u32) -> Result<(), String> {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(target)]))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn recollect_batch_cards_return_the_exact_chosen_graveyard_card() {
    for (index, case) in cases().into_iter().enumerate() {
        for card in ["grizzly_bears", "forest", "lightning_bolt"] {
            let mut e = engine(850_000 + index as u64 * 10 + card.len() as u64, case.id);
            let target = inject_graveyard_card(&mut e, 0, card);
            let other = inject_graveyard_card(&mut e, 0, "hill_giant");
            let opponent = inject_graveyard_card(&mut e, 1, "grizzly_bears");
            assert!(cast_recollect(&mut e, &case, target).is_ok(), "{}", case.id);
            pass_both_players(&mut e);
            assert_eq!(e.state.objects[&target].zone, Zone::Hand, "{}", case.id);
            assert!(e.state.players[0].hand.contains(&target), "{}", case.id);
            assert_eq!(e.state.objects[&other].zone, Zone::Graveyard, "{}", case.id);
            assert_eq!(
                e.state.objects[&opponent].zone,
                Zone::Graveyard,
                "{}",
                case.id
            );
            assert!(
                e.state.players[1].graveyard.contains(&opponent),
                "{}",
                case.id
            );
            assert_eq!(
                count_card_id_in_graveyard(&e, 0, case.id),
                1,
                "{}: the resolved spell is in its owner's graveyard",
                case.id
            );
        }
    }
}

#[test]
fn recollect_batch_cards_fizzle_when_the_chosen_card_leaves_the_graveyard() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(851_000 + index as u64, case.id);
        let target = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        let other = inject_graveyard_card(&mut e, 0, "forest");
        let hand_before = e.state.players[0].hand.len();
        assert!(cast_recollect(&mut e, &case, target).is_ok(), "{}", case.id);
        e.state.players[0].graveyard.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Exile;
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{}", case.id);
        assert_eq!(e.state.objects[&other].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(count_card_id_in_graveyard(&e, 0, case.id), 1, "{}", case.id);
    }
}

#[test]
fn recollect_batch_cards_reject_an_opponents_graveyard_and_a_missing_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(852_000 + index as u64, case.id);
        let opponent = inject_graveyard_card(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let hand_before = e.state.players[0].hand.len();
        let pool_before = {
            let p = &e.state.players[0].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        };
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(opponent)]))
                .is_err(),
            "{}: only the caster's graveyard is legal",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: the graveyard target is mandatory",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(
            e.state.objects[&opponent].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        let p = &e.state.players[0].mana_pool;
        assert_eq!(
            (p.white, p.blue, p.black, p.red, p.green, p.colorless),
            pool_before,
            "{}: rejected casts spend no mana",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
