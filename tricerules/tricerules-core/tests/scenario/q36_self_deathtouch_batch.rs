//! Actual-card scenarios for the queue-v36 self-deathtouch cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the mana activation;
//! CR 611.2a/514.2 end the keyword grant; CR 613.1f applies it in layer 6 and CR 702.2
//! defines deathtouch.
use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    cost: ManaGift,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "mardu_hateblade",
            cost: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        Case {
            id: "narnam_cobra",
            cost: ManaGift {
                g: 1,
                ..Default::default()
            },
        },
        Case {
            id: "skittering_heartstopper",
            cost: ManaGift {
                b: 1,
                ..Default::default()
            },
        },
        Case {
            id: "twilight_panther",
            cost: ManaGift {
                b: 1,
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
fn q36_self_deathtouch_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(36_060_001);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    let other = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    give_mana(&mut e, 0, gift(&case.cost));
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate deathtouch");
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "exact printed activation cost"
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.effective_has_keyword(source, Keyword::Deathtouch),
        "Mardu Hateblade gains deathtouch"
    );
    assert!(
        !e.effective_has_keyword(other, Keyword::Deathtouch),
        "the activation affects only the source"
    );
    end_active_turn(&mut e, 0);
    assert!(
        !e.effective_has_keyword(source, Keyword::Deathtouch),
        "the grant ends with the turn"
    );
}

#[test]
fn q36_self_deathtouch_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(36_060_002);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    let other = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    give_mana(&mut e, 0, gift(&case.cost));
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate deathtouch");
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "exact printed activation cost"
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.effective_has_keyword(source, Keyword::Deathtouch),
        "Narnam Cobra gains deathtouch"
    );
    assert!(
        !e.effective_has_keyword(other, Keyword::Deathtouch),
        "another creature does not gain deathtouch"
    );
}

#[test]
fn q36_self_deathtouch_cards_grant_the_keyword_only_to_the_source() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_061_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let own_other = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let opponent = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        give_mana(&mut e, 0, gift(&case.cost));
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate deathtouch");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.effective_has_keyword(source, Keyword::Deathtouch),
            "{}: source gains deathtouch",
            case.id
        );
        assert!(
            !e.effective_has_keyword(own_other, Keyword::Deathtouch),
            "{}: another controlled creature is unaffected",
            case.id
        );
        assert!(
            !e.effective_has_keyword(opponent, Keyword::Deathtouch),
            "{}: an opponent's creature is unaffected",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(source, Keyword::Deathtouch),
            "{}: the grant ends with the turn",
            case.id
        );
    }
}

#[test]
fn q36_self_deathtouch_cards_require_mana_and_the_source_controller() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_062_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: activation without mana is rejected",
            case.id
        );
        assert!(
            !e.effective_has_keyword(source, Keyword::Deathtouch),
            "{}: the rejected activation grants nothing",
            case.id
        );
        assert!(
            apply_ability(&mut e, 1, source, 0, vec![]).is_err(),
            "{}: a foreign player cannot activate the source",
            case.id
        );
    }
}
