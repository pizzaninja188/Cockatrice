//! Actual-card scenarios for the queue-v27 bounce cohort.
//!
//! Expectations are printed Oracle values. CR 115.1 chooses one permanent target;
//! CR 400.3 moves that card to its owner's hand; CR 110.2 keeps ownership separate
//! from control; CR 608.2b fizzles a spell whose only target is illegal.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    nonland: bool,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "eye_of_nowhere",
            mana: ManaGift {
                u: 2,
                ..Default::default()
            },
            nonland: false,
        },
        Case {
            id: "regress",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            nonland: false,
        },
        Case {
            id: "disperse",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            nonland: true,
        },
        Case {
            id: "void_snare",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            nonland: true,
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card, "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn cast_bounce(e: &mut GameEngine, case: &Case, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
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
        "{}: exact cast cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn bounce_spells_batch_cards_return_an_opponents_creature_to_its_owners_hand() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_050_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let opponent_hand = e.state.players[1].hand.len();
        cast_bounce(&mut e, &case, target);
        assert_eq!(e.state.objects[&target].zone, Zone::Hand, "{}", case.id);
        assert!(e.state.players[1].hand.contains(&target), "{}", case.id);
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand + 1,
            "{}: the owner's hand gained the creature",
            case.id
        );
        assert_eq!(
            count_card_id_in_graveyard(&e, 0, case.id),
            1,
            "{}: the spell is in its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn bounce_spells_batch_cards_return_their_controllers_own_permanent() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_051_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let hand = e.state.players[0].hand.len();
        cast_bounce(&mut e, &case, target);
        assert_eq!(e.state.objects[&target].zone, Zone::Hand, "{}", case.id);
        assert!(e.state.players[0].hand.contains(&target), "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand + 1, "{}", case.id);
    }
}

#[test]
fn bounce_spells_batch_cards_apply_the_printed_land_restriction() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_052_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        if case.nonland {
            let before = e.state.players[0].mana_pool.blue;
            assert!(
                e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                    .is_err(),
                "{}: a land is not a legal nonland-permanent target",
                case.id
            );
            assert_eq!(e.state.players[0].mana_pool.blue, before, "{}", case.id);
            assert_eq!(
                e.state.objects[&land].zone,
                Zone::Battlefield,
                "{}",
                case.id
            );
        } else {
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .unwrap_or_else(|error| panic!("{}: a land is a permanent: {error:?}", case.id));
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(e.state.objects[&land].zone, Zone::Hand, "{}", case.id);
            assert!(e.state.players[1].hand.contains(&land), "{}", case.id);
        }
    }
}

#[test]
fn bounce_spells_batch_cards_reject_a_missing_target_and_fizzle_a_stale_one() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(27_053_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: the permanent target is mandatory",
            case.id
        );
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("target before it leaves");
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        e.state.players[1].graveyard.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: the stale target stays in the graveyard",
            case.id
        );
        assert_eq!(
            count_card_id_in_graveyard(&e, 0, case.id),
            1,
            "{}: the spell fizzles to its owner's graveyard",
            case.id
        );
    }
}
