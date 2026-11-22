//! Actual-card scenarios for the queue-v43 Order protection/pump cohort.
//!
//! Expectations are printed Oracle values. CR 702.16 publishes protection; CR 602.1/602.2 define
//! each activation; CR 118.3 pays the printed mana; CR 613.1f grants first strike and
//! CR 611.2a/613.4c apply the pump until cleanup.
use super::helpers::*;

struct Case {
    id: &'static str,
    body: (u32, u32),
    protection: &'static str,
    first_cost: ManaGift,
    pump_cost: ManaGift,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "order_of_leitbur",
            body: (2, 1),
            protection: "Protection from black",
            first_cost: ManaGift {
                w: 1,
                ..Default::default()
            },
            pump_cost: ManaGift {
                w: 2,
                ..Default::default()
            },
        },
        Case {
            id: "order_of_the_white_shield",
            body: (2, 1),
            protection: "Protection from black",
            first_cost: ManaGift {
                w: 1,
                ..Default::default()
            },
            pump_cost: ManaGift {
                w: 2,
                ..Default::default()
            },
        },
        Case {
            id: "order_of_the_ebon_hand",
            body: (2, 1),
            protection: "Protection from white",
            first_cost: ManaGift {
                b: 1,
                ..Default::default()
            },
            pump_cost: ManaGift {
                b: 2,
                ..Default::default()
            },
        },
        Case {
            id: "knight_of_stromgald",
            body: (2, 1),
            protection: "Protection from white",
            first_cost: ManaGift {
                b: 1,
                ..Default::default()
            },
            pump_cost: ManaGift {
                b: 2,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("island", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn activate(e: &mut GameEngine, creature: u32, index: u32, mana: &ManaGift, label: &str) {
    give_mana(e, 0, gift(mana));
    apply_ability(e, 0, creature, index, vec![])
        .unwrap_or_else(|error| panic!("{label}: activate {index}: {error:?}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{label}: exact cost");
}

#[test]
fn q43_order_pump_canary_publishes_protection_and_pumps() {
    let case = &cases()[0];
    let mut e = engine(43_090_001, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(
        zone_view_rules_annotation_labels(&mut e, 0, creature)
            .iter()
            .any(|label| label == case.protection),
        "the printed protection is published"
    );
    activate(&mut e, creature, 0, &case.first_cost, case.id);
    assert!(e.effective_has_keyword(creature, tricerules_cards::Keyword::FirstStrike));
    activate(&mut e, creature, 1, &case.pump_cost, case.id);
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(3), Some(1)),
        "the printed +1/+0 applies"
    );
}

#[test]
fn q43_order_pump_cards_apply_their_printed_protection_and_two_activations() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_091_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, creature)
                .iter()
                .any(|label| label == case.protection),
            "{}: protection",
            case.id
        );
        activate(&mut e, creature, 0, &case.first_cost, case.id);
        assert!(
            e.effective_has_keyword(creature, tricerules_cards::Keyword::FirstStrike),
            "{}: first strike",
            case.id
        );
        activate(&mut e, creature, 1, &case.pump_cost, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0 + 1), Some(case.body.1)),
            "{}: printed pump",
            case.id
        );
        activate(&mut e, creature, 1, &case.pump_cost, case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.body.0 + 2),
            "{}: the mana-only pump activation is repeatable",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: CR 514.2 restores the body",
            case.id
        );
        assert!(
            !e.effective_has_keyword(creature, tricerules_cards::Keyword::FirstStrike),
            "{}: the granted first strike also expires",
            case.id
        );
    }
}

#[test]
fn q43_order_pump_cards_require_each_printed_cost() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_092_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: first-strike activation requires its mana",
            case.id
        );
        assert!(
            apply_ability(&mut e, 0, creature, 1, vec![]).is_err(),
            "{}: pump activation requires its mana",
            case.id
        );
        assert!(
            !e.effective_has_keyword(creature, tricerules_cards::Keyword::FirstStrike),
            "{}: no keyword without payment",
            case.id
        );
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: no pump without payment",
            case.id
        );
    }
}
