//! Actual-card scenarios for the queue-v27 typed graveyard-recursion cohort.
//!
//! Expectations are printed Oracle values. CR 115.1 and 601.2c choose one
//! graveyard-card target of the printed card type; CR 400.7 gives a card that
//! changes zones a new object identity; CR 608.2b fizzles the spell when its only
//! target is illegal on resolution; CR 404.1 keeps each graveyard owner-scoped.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::TargetRef;

struct Case {
    id: &'static str,
    mana: ManaGift,
    legal: &'static str,
    illegal: &'static str,
}

fn cases() -> [Case; 7] {
    [
        Case {
            id: "return_to_battle",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "lightning_bolt",
        },
        Case {
            id: "wildwood_rebirth",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            legal: "grizzly_bears",
            illegal: "lightning_bolt",
        },
        Case {
            id: "call_to_mind",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            legal: "lightning_bolt",
            illegal: "grizzly_bears",
        },
        Case {
            id: "relearn",
            mana: ManaGift {
                u: 2,
                c: 1,
                ..Default::default()
            },
            legal: "divination",
            illegal: "grizzly_bears",
        },
        Case {
            id: "reconstruction",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            legal: "bonesplitter",
            illegal: "grizzly_bears",
        },
        Case {
            id: "ritual_of_restoration",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            legal: "bonesplitter",
            illegal: "grizzly_bears",
        },
        Case {
            id: "sages_knowledge",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            legal: "divination",
            illegal: "lightning_bolt",
        },
    ]
}

fn graveyard_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: tricerules_proto::ruled::v1::TargetRefKind::Graveyard as i32,
        ..Default::default()
    }
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

fn prepare(e: &mut GameEngine, case: &Case) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
}

fn cast_recursion(e: &mut GameEngine, case: &Case, target: u32) -> Result<(), String> {
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(target)]))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn graveyard_type_recursion_batch_cards_return_only_the_chosen_legal_card() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_040_000 + index as u64, case.id);
        let target = inject_graveyard_card(&mut e, 0, case.legal);
        let other = inject_graveyard_card(&mut e, 0, case.legal);
        let opponent = inject_graveyard_card(&mut e, 1, "grizzly_bears");
        prepare(&mut e, &case);
        assert!(cast_recursion(&mut e, &case, target).is_ok(), "{}", case.id);
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

#[test]
fn graveyard_type_recursion_batch_cards_fizzle_when_the_target_leaves_the_graveyard() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_041_000 + index as u64, case.id);
        let target = inject_graveyard_card(&mut e, 0, case.legal);
        let other = inject_graveyard_card(&mut e, 0, case.legal);
        let hand_before = e.state.players[0].hand.len();
        prepare(&mut e, &case);
        assert!(cast_recursion(&mut e, &case, target).is_ok(), "{}", case.id);
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
fn graveyard_type_recursion_batch_cards_reject_the_wrong_type_opponents_graveyard_and_missing_target(
) {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_042_000 + index as u64, case.id);
        let wrong = inject_graveyard_card(&mut e, 0, case.illegal);
        let opponent = inject_graveyard_card(&mut e, 1, case.legal);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let hand_before = e.state.players[0].hand.len();
        let pool_before = {
            let p = &e.state.players[0].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        };
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(wrong)]))
                .is_err(),
            "{}: the printed card type is required",
            case.id
        );
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
        assert_eq!(e.state.objects[&wrong].zone, Zone::Graveyard, "{}", case.id);
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
    }
}
