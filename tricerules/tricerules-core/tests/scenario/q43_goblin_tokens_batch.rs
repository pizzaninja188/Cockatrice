//! Actual-card scenarios for the queue-v43 Goblin token sorcery cohort.
//!
//! Expectations are printed Oracle values. CR 601.2a casts the sorcery; CR 608.2 resolves it;
//! CR 111.1/111.4 create the named 1/1 red Goblin creature tokens under the controller.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    count: usize,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "krenkos_command",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            count: 2,
        },
        Case {
            id: "hordeling_outburst",
            mana: ManaGift {
                r: 2,
                c: 1,
                ..Default::default()
            },
            count: 3,
        },
        Case {
            id: "goblin_rally",
            mana: ManaGift {
                r: 2,
                c: 3,
                ..Default::default()
            },
            count: 4,
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
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

fn cast_tokens(e: &mut GameEngine, case: &Case) -> RuledEventBatch {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    let batch = pass_both_players_with_batch(e);
    assert_eq!(
        {
            let p = &e.state.players[0].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        },
        (0, 0, 0, 0, 0, 0),
        "{}: exact cost",
        case.id
    );
    batch
}

fn assert_goblin_tokens(e: &GameEngine, case: &Case, batch: &RuledEventBatch) {
    let events = token_created_events(batch);
    assert_eq!(
        events.len(),
        case.count,
        "{}: token-created events",
        case.id
    );
    for event in events {
        let identity = event.identity.as_ref().expect("token identity");
        assert_eq!(&*identity.name, "Goblin", "{}: token name", case.id);
        assert_eq!(&*identity.pt, "1/1", "{}: token body", case.id);
        assert_eq!(&*identity.color, "r", "{}: token color", case.id);
    }
    let created = battlefield_token_oids(e, 0, "goblin_r_1_1");
    assert_eq!(
        created.len(),
        case.count,
        "{}: printed token count",
        case.id
    );
    for token in created {
        assert_eq!(e.effective_power(token), Some(1), "{}", case.id);
        assert_eq!(e.effective_toughness(token), Some(1), "{}", case.id);
        assert_eq!(e.state.objects[&token].controller, 0, "{}", case.id);
    }
    assert!(
        battlefield_token_oids(e, 1, "goblin_r_1_1").is_empty(),
        "{}: tokens belong to the caster",
        case.id
    );
}

#[test]
fn q43_goblin_tokens_canary_creates_the_printed_count() {
    let case = &cases()[0];
    let mut e = engine(43_020_001);
    let batch = cast_tokens(&mut e, case);
    assert_goblin_tokens(&e, case, &batch);
}

#[test]
fn q43_goblin_tokens_cards_create_exact_goblin_tokens_for_their_printed_cost() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_021_000 + index as u64);
        let batch = cast_tokens(&mut e, case);
        assert_goblin_tokens(&e, case, &batch);
    }
}

#[test]
fn q43_goblin_tokens_cards_do_not_create_tokens_without_mana() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_022_000 + index as u64);
        let card = inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: the spell requires its printed mana",
            case.id
        );
        assert!(
            battlefield_token_oids(&e, 0, "goblin_r_1_1").is_empty(),
            "{}: no token is created",
            case.id
        );
        assert!(
            e.state.players[0].hand.contains(&card),
            "{}: the uncast card stays in hand",
            case.id
        );
    }
}
