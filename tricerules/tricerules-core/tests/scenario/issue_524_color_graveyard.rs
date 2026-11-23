//! Actual-card scenarios for #524: Revive returns a green card and Reborn Hope a multicolored
//! card from the caster's graveyard.
//!
//! Expectations are printed Oracle values. CR 105.2 and 105.2b define the color requirement;
//! CR 115.1 and 601.2c choose one graveyard-card target; CR 400.7 gives a card that changes
//! zones a new object identity; CR 608.2b fizzles the spell when its only target is illegal.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::TargetRef;

struct Case {
    id: &'static str,
    mana: ManaGift,
    legal: &'static [&'static str],
    illegal: &'static [&'static str],
}

fn cases() -> [Case; 2] {
    [
        Case {
            id: "revive",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            // Mono-green creature and a green-white instant.
            legal: &["grizzly_bears", "advent_of_the_wurm"],
            // Red, red-white, colorless artifact and colorless land.
            illegal: &["lightning_bolt", "lightning_helix", "ornithopter", "forest"],
        },
        Case {
            id: "reborn_hope",
            mana: ManaGift {
                g: 1,
                w: 1,
                ..Default::default()
            },
            legal: &["advent_of_the_wurm", "lightning_helix"],
            illegal: &["grizzly_bears", "lightning_bolt", "ornithopter", "forest"],
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

fn cast(e: &mut GameEngine, case: &Case, targets: Vec<TargetRef>) -> Result<(), String> {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

fn mana_pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn issue_524_cards_return_only_matching_graveyard_cards() {
    for (index, case) in cases().into_iter().enumerate() {
        for (offset, card) in case.legal.iter().enumerate() {
            let mut e = engine(524_100 + index as u64 * 10 + offset as u64, case.id);
            let target = inject_graveyard_card(&mut e, 0, card);
            let others: Vec<_> = case
                .illegal
                .iter()
                .map(|other| inject_graveyard_card(&mut e, 0, other))
                .collect();
            assert!(
                cast(&mut e, &case, vec![graveyard_target(target)]).is_ok(),
                "{}: {card}",
                case.id
            );
            pass_both_players(&mut e);
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Hand,
                "{}: {card}",
                case.id
            );
            assert!(
                e.state.players[0].hand.contains(&target),
                "{}: {card}",
                case.id
            );
            for other in others {
                assert_eq!(e.state.objects[&other].zone, Zone::Graveyard, "{}", case.id);
            }
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
fn issue_524_cards_fizzle_when_the_chosen_card_leaves_the_graveyard() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(524_200 + index as u64, case.id);
        let target = inject_graveyard_card(&mut e, 0, case.legal[0]);
        let other = inject_graveyard_card(&mut e, 0, case.legal[1]);
        let hand_before = e.state.players[0].hand.len();
        assert!(
            cast(&mut e, &case, vec![graveyard_target(target)]).is_ok(),
            "{}",
            case.id
        );
        e.state.players[0].graveyard.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Exile;
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Exile, "{}", case.id);
        assert_eq!(
            e.state.objects[&other].zone,
            Zone::Graveyard,
            "{}: the other matching card is not a substitute target",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(count_card_id_in_graveyard(&e, 0, case.id), 1, "{}", case.id);
    }
}

#[test]
fn issue_524_cards_reject_nonmatching_and_missing_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(524_300 + index as u64, case.id);
        let nonmatching: Vec<_> = case
            .illegal
            .iter()
            .map(|card| (*card, inject_graveyard_card(&mut e, 0, card)))
            .collect();
        let opponent = inject_graveyard_card(&mut e, 1, case.legal[0]);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let hand_before = e.state.players[0].hand.len();
        let pool_before = mana_pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        for (card, oid) in &nonmatching {
            assert!(
                e.apply_scenario_command(0, &cast_spell(slot, vec![graveyard_target(*oid)]))
                    .is_err(),
                "{}: {card} does not match the color requirement",
                case.id
            );
        }
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
        for (card, oid) in nonmatching {
            assert_eq!(
                e.state.objects[&oid].zone,
                Zone::Graveyard,
                "{}: {card}",
                case.id
            );
        }
        assert_eq!(
            e.state.objects[&opponent].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            mana_pool(&e),
            pool_before,
            "{}: rejected casts spend no mana",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
