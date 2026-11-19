//! Actual-card scenarios for the queue-v37 tap-and-grant activated cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 and 118.3 define the mana and
//! tap costs; CR 115.1 requires a target creature; CR 611.2a/514.2 end the grant.
use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    mana: ManaGift,
    keyword: Keyword,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "flying_carpet",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            keyword: Keyword::Flying,
        },
        Case {
            id: "caller_of_gales",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            keyword: Keyword::Flying,
        },
        Case {
            id: "war_chariot",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            keyword: Keyword::Trample,
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

fn activate(e: &mut GameEngine, case: &Case, source: u32, target: u32) {
    give_mana(e, 0, gift(&case.mana));
    apply_ability(e, 0, source, 0, target_object(target))
        .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    assert!(
        e.state.objects[&source].tapped,
        "{}: the tap cost is paid",
        case.id
    );
    assert!(
        !e.effective_has_keyword(target, case.keyword),
        "{}: the grant waits for resolution",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn q37_tap_keyword_activated_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(37_090_001);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    activate(&mut e, case, source, target);
    assert!(e.effective_has_keyword(target, case.keyword));
    assert!(!e.effective_has_keyword(source, case.keyword));
    end_active_turn(&mut e, 0);
    assert!(!e.effective_has_keyword(target, case.keyword));
}

#[test]
fn q37_tap_keyword_activated_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(37_090_002);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    activate(&mut e, case, source, target);
    assert!(e.effective_has_keyword(target, case.keyword));
    assert!(
        !e.effective_has_keyword(source, case.keyword),
        "the source does not grant itself"
    );
}

#[test]
fn q37_tap_keyword_activated_cards_grant_only_the_chosen_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_091_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");
        activate(&mut e, case, source, own);
        assert!(e.effective_has_keyword(own, case.keyword), "{}", case.id);
        assert!(
            !e.effective_has_keyword(opponent, case.keyword),
            "{}",
            case.id
        );
        assert!(
            !e.effective_has_keyword(source, case.keyword),
            "{}",
            case.id
        );
    }
}

#[test]
fn q37_tap_keyword_activated_cards_require_a_target_mana_and_an_untapped_source() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_092_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let _target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, 0, vec![]).expect_err("a target is required");
        assert!(!e.state.objects[&source].tapped, "{}", case.id);

        let mut no_mana = engine(37_093_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut no_mana, 0, case.id);
        let target = inject_creature_on_battlefield(&mut no_mana, 0, "grizzly_bears");
        apply_ability(&mut no_mana, 0, source, 0, target_object(target))
            .expect_err("the activation needs its printed mana");
        assert!(!no_mana.state.objects[&source].tapped, "{}", case.id);

        let mut tapped = engine(37_094_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut tapped, 0, case.id);
        let target = inject_creature_on_battlefield(&mut tapped, 0, "grizzly_bears");
        tapped
            .state
            .objects
            .get_mut(&source)
            .expect("source")
            .tapped = true;
        give_mana(&mut tapped, 0, gift(&case.mana));
        apply_ability(&mut tapped, 0, source, 0, target_object(target))
            .expect_err("a tapped source cannot pay {T}");
        assert!(
            !tapped.effective_has_keyword(target, case.keyword),
            "{}",
            case.id
        );
    }
}
