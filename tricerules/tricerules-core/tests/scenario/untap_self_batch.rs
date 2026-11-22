//! Actual-card scenarios for the queue-v28 "{U}: Untap this creature" cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activate the ability;
//! CR 118.3 pays the {U} cost; CR 701.26 untaps this creature; 605.1a confirms the
//! ability is not a mana ability.
use super::helpers::*;

struct Case {
    id: &'static str,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "simic_ragworm",
        },
        Case {
            id: "horseshoe_crab",
        },
        Case { id: "soliton" },
    ]
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

#[test]
fn untap_self_batch_cards_spend_one_blue_mana_and_untap_the_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_080_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bystander = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&source).expect("source").tapped = true;
        e.state
            .objects
            .get_mut(&bystander)
            .expect("bystander")
            .tapped = true;
        give_mana(
            &mut e,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
        assert_eq!(
            e.state.players[0].mana_pool.blue, 0,
            "{}: the {{U}} cost was paid",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert!(
            !e.state.objects[&source].tapped,
            "{}: the source untaps",
            case.id
        );
        assert!(
            e.state.objects[&bystander].tapped,
            "{}: no other creature untaps",
            case.id
        );
    }
}

#[test]
fn untap_self_batch_cards_are_legal_on_an_untapped_source_and_reject_no_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_081_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(!e.state.objects[&source].tapped);
        give_mana(
            &mut e,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert!(
            !e.state.objects[&source].tapped,
            "{}: an untapped source stays untapped",
            case.id
        );

        let mut unpaid = engine(28_082_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut unpaid, 0, case.id);
        unpaid
            .state
            .objects
            .get_mut(&source)
            .expect("source")
            .tapped = true;
        assert!(
            apply_ability(&mut unpaid, 0, source, 0, vec![]).is_err(),
            "{}: the {{U}} cost is required",
            case.id
        );
        assert!(
            unpaid.state.objects[&source].tapped,
            "{}: a rejected activation leaves the source tapped",
            case.id
        );
    }
}
