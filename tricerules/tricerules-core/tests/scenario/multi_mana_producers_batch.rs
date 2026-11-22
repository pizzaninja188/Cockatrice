//! Actual-card scenarios for the queue-v28 multi-mana producer cohort.
//!
//! Expectations are printed Oracle values. CR 605.1a/605.3a-b make each targetless
//! activated mana ability resolve immediately; CR 118.3 taps the source as its cost;
//! CR 106.1a-b define the produced colors and types.
use super::helpers::*;

struct Case {
    id: &'static str,
    pool: (u32, u32, u32, u32, u32, u32),
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "fyndhorn_elder",
            pool: (0, 0, 0, 0, 2, 0),
        },
        Case {
            id: "greenweaver_druid",
            pool: (0, 0, 0, 0, 2, 0),
        },
        Case {
            id: "gyre_engineer",
            pool: (0, 1, 0, 0, 1, 0),
        },
        Case {
            id: "nantuko_elder",
            pool: (0, 0, 0, 0, 1, 1),
        },
        Case {
            id: "llanowar_tribe",
            pool: (0, 0, 0, 0, 3, 0),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "grizzly_bears"]),
        deck_with("island", &[]),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn multi_mana_producers_batch_cards_produce_the_printed_mana_and_tap() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_020_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(!e.state.objects[&source].tapped);
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
        assert_eq!(pool(&e), case.pool, "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: a mana ability resolves immediately",
            case.id
        );
    }
}

#[test]
fn multi_mana_producers_batch_cards_require_an_untapped_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_021_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.state.objects.get_mut(&source).expect("source").tapped = true;
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the {{T}} cost cannot be paid",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn multi_mana_producers_batch_creatures_respect_summoning_sickness() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_022_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = true;
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: a summoning-sick creature cannot activate {{T}}",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);

        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = false;
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
        assert_eq!(pool(&e), case.pool, "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);
    }
}
