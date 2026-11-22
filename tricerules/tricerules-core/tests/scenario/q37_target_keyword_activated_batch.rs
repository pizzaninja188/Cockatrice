//! Actual-card scenarios for the queue-v37 target-keyword activated cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the mana activation;
//! CR 115.1 requires a target creature; CR 611.2a/514.2 end the grant at cleanup.
use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    cost: ManaGift,
    keyword: Keyword,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "whip_sergeant",
            cost: ManaGift {
                r: 1,
                ..Default::default()
            },
            keyword: Keyword::Haste,
        },
        Case {
            id: "zephyr_charge",
            cost: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            keyword: Keyword::Flying,
        },
        Case {
            id: "elvish_herder",
            cost: ManaGift {
                g: 1,
                ..Default::default()
            },
            keyword: Keyword::Trample,
        },
        Case {
            id: "keen_glidemaster",
            cost: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            keyword: Keyword::Flying,
        },
        Case {
            id: "noble_steeds",
            cost: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            keyword: Keyword::FirstStrike,
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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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
fn q37_target_keyword_activated_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(37_050_001);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.cost));
    apply_ability(&mut e, 0, source, 0, target_object(target)).expect("grant trample");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact cost");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(target, case.keyword));
    assert!(
        !e.effective_has_keyword(source, case.keyword),
        "only the target gains it"
    );
    end_active_turn(&mut e, 0);
    assert!(!e.effective_has_keyword(target, case.keyword));
}

#[test]
fn q37_target_keyword_activated_canary_hardest() {
    let case = &cases()[4];
    let mut e = engine(37_050_002);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    give_mana(&mut e, 0, gift(&case.cost));
    apply_ability(&mut e, 0, source, 0, target_object(target)).expect("grant first strike");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(target, case.keyword));
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(2)),
        "the grant does not change the body"
    );
}

#[test]
fn q37_target_keyword_activated_cards_grant_only_the_chosen_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_051_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");
        give_mana(&mut e, 0, gift(&case.cost));
        apply_ability(&mut e, 0, source, 0, target_object(opponent)).expect("grant keyword");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.effective_has_keyword(opponent, case.keyword),
            "{}",
            case.id
        );
        assert!(!e.effective_has_keyword(own, case.keyword), "{}", case.id);
        assert!(
            !e.effective_has_keyword(source, case.keyword),
            "{}",
            case.id
        );
    }
}

#[test]
fn q37_target_keyword_activated_cards_require_a_target_and_exact_mana() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_052_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.cost));
        let before = pool(&e);
        apply_ability(&mut e, 0, source, 0, vec![]).expect_err("a target is required");
        assert_eq!(
            pool(&e),
            before,
            "{}: failed activation pays nothing",
            case.id
        );

        let mut short = engine(37_053_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut short, 0, case.id);
        let target = inject_creature_on_battlefield(&mut short, 0, "grizzly_bears");
        apply_ability(&mut short, 0, source, 0, target_object(target))
            .expect_err("the activation needs its printed mana");
        assert!(
            !short.effective_has_keyword(target, case.keyword),
            "{}",
            case.id
        );
    }
}
