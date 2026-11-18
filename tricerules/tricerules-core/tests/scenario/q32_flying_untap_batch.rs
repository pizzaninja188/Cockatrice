//! Actual-card scenarios for the queue-v32 flying self-untap cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b grants flying;
//! CR 602.1/602.2 activate the ability; CR 118.3 requires its mana cost; and
//! CR 701.26 untaps the source.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    activation: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "blizzard_elemental",
            mana: ManaGift {
                u: 2,
                c: 5,
                ..Default::default()
            },
            body: (5, 5),
            activation: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "glimmerbell",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 3),
            activation: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "vigilant_drake",
            mana: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
            body: (3, 3),
            activation: ManaGift {
                u: 1,
                c: 2,
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "island",
            &["blizzard_elemental", "glimmerbell", "vigilant_drake"],
        ),
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

#[test]
fn q32_flying_untap_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(32_080_001);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.effective_has_keyword(source, Keyword::Flying));
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1))
    );
    e.state.objects.get_mut(&source).unwrap().tapped = true;
    give_mana(&mut e, 0, gift(&case.activation));
    apply_ability(&mut e, 0, source, 0, vec![]).expect("untap the creature");
    resolve_entire_stack_two_player(&mut e);
    assert!(!e.state.objects[&source].tapped);
    assert!(e.state.stack.is_empty());
}

#[test]
fn q32_flying_untap_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(32_080_002);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    e.state.objects.get_mut(&source).unwrap().tapped = true;
    give_mana(&mut e, 0, gift(&case.activation));
    apply_ability(&mut e, 0, source, 0, vec![]).expect("untap the elemental");
    resolve_entire_stack_two_player(&mut e);
    assert!(!e.state.objects[&source].tapped);
}

#[test]
fn q32_flying_untap_cards_cast_with_their_printed_cost_and_body() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_081_000 + index as u64);
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert!(
            e.effective_has_keyword(source, Keyword::Flying),
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}",
            case.id
        );
        e.state.objects.get_mut(&source).unwrap().tapped = true;
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, vec![]).expect("untap");
        resolve_entire_stack_two_player(&mut e);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
    }
}

#[test]
fn q32_flying_untap_cards_require_the_printed_mana_and_reject_foreign_controllers() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_082_000 + index as u64);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.state.objects.get_mut(&source).unwrap().tapped = true;
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the printed mana cost is required",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}", case.id);

        let mut foreign = engine(32_083_000 + index as u64);
        let foreign_source = move_ready_to_battlefield(&mut foreign, 0, case.id);
        foreign
            .state
            .objects
            .get_mut(&foreign_source)
            .unwrap()
            .tapped = true;
        give_mana(&mut foreign, 0, gift(&case.activation));
        foreign
            .apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut foreign, 1, foreign_source, 0, vec![]).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert!(foreign.state.objects[&foreign_source].tapped, "{}", case.id);
    }
}
