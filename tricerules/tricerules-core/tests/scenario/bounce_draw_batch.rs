//! Actual-card scenarios for the reviewed bounce-and-draw spell cohort.
//!
//! Expectations are printed Oracle values. CR 400.3 returns the target creature
//! to its owner's hand, CR 121.1 draws one card for the spell's controller, and
//! CR 608.2b makes a spell with an illegal sole target fail to resolve, so its
//! draw does not happen.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "drag_under",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "repulse",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "symbol_of_unsummoning",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card, "grizzly_bears"]),
        deck_with("forest", &["grizzly_bears", "unsummon"]),
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

fn cast_named(e: &mut GameEngine, card_id: &str, mana: ManaGift, target: u32) {
    ensure_in_hand(e, 0, card_id);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card_id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
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
        "{card_id}: exact cast cost"
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn bounce_draw_batch_cards_return_the_target_and_draw() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_310_000 + index as u64, case.id);
        let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
        ensure_in_hand(&mut e, 0, case.id);
        let caster_hand = e.state.players[0].hand.len();
        let caster_library = e.state.players[0].library.len();
        let opponent_hand = e.state.players[1].hand.len();
        cast_named(&mut e, case.id, case.mana, target);

        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Hand,
            "{}: returned to hand",
            case.id
        );
        assert!(
            e.state.players[1].hand.contains(&target),
            "{}: returned to its owner's hand",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand + 1,
            "{}: the opponent's hand gained the creature",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            caster_library - 1,
            "{}: the controller drew one card",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            caster_hand,
            "{}: the spell left hand and the draw replaced it",
            case.id
        );
        let spell = e.state.players[0]
            .graveyard
            .iter()
            .copied()
            .find(|oid| e.state.objects[oid].card_id == case.id)
            .expect("the spell is in the graveyard");
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn bounce_draw_batch_cards_return_their_controllers_own_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_311_000 + index as u64, case.id);
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        let caster_hand = e.state.players[0].hand.len();
        let caster_library = e.state.players[0].library.len();
        cast_named(&mut e, case.id, case.mana, target);

        assert_eq!(e.state.objects[&target].zone, Zone::Hand, "{}", case.id);
        assert!(
            e.state.players[0].hand.contains(&target),
            "{}: the controller's own creature returns to their hand",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            caster_hand + 1,
            "{}: the return adds the creature and the draw adds a card",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            caster_library - 1,
            "{}: one draw",
            case.id
        );
    }
}

#[test]
fn bounce_draw_batch_cards_require_a_creature_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_312_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal creature target",
            case.id
        );
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}: the rejected spell leaves the land alone",
            case.id
        );
        assert_eq!(
            e.state.objects[&e.state.players[0].hand[slot]].zone,
            Zone::Hand,
            "{}: the rejected spell stays in hand",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn bounce_draw_batch_cards_fizzle_stale_targets_without_drawing() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_313_000 + index as u64, case.id);
        let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast the bounce spell");
        e.apply_scenario_command(0, &pass())
            .expect("the caster passes priority");

        ensure_in_hand(&mut e, 1, "unsummon");
        give_mana(
            &mut e,
            1,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let unsummon = hand_index_for_card(&e, 1, "unsummon");
        e.apply_scenario_command(1, &cast_spell(unsummon, target_object(target)))
            .expect("return the target in response");
        let caster_hand = e.state.players[0].hand.len();
        let caster_library = e.state.players[0].library.len();
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Hand,
            "{}: the response returned the target",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            caster_library,
            "{}: the fizzled spell draws nothing",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            caster_hand,
            "{}: the fizzled spell adds no card",
            case.id
        );
        let spell = e.state.players[0]
            .graveyard
            .iter()
            .copied()
            .find(|oid| e.state.objects[oid].card_id == case.id)
            .expect("the fizzled spell is in the graveyard");
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
    }
}
