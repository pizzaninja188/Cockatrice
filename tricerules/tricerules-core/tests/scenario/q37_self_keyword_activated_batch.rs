//! Actual-card scenarios for the queue-v37 self-keyword activated cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the mana activation;
//! CR 611.2a/514.2 end the keyword grant at cleanup; CR 613.1f applies it in layer 6.
//! CR 107.4f/119.4 allow the {G/P} activation to be paid with green mana or 2 life.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    cost: ManaGift,
    keywords: &'static [Keyword],
}

fn cases() -> [Case; 10] {
    [
        Case {
            id: "cabaretti_initiate",
            cost: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            keywords: &[Keyword::DoubleStrike],
        },
        Case {
            id: "fallaji_chaindancer",
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            keywords: &[Keyword::DoubleStrike],
        },
        Case {
            id: "giant_crab",
            cost: ManaGift {
                u: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Shroud],
        },
        Case {
            id: "gruul_nodorog",
            cost: ManaGift {
                r: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Menace],
        },
        Case {
            id: "horror_of_the_dim",
            cost: ManaGift {
                u: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Hexproof],
        },
        Case {
            id: "insatiable_souleater",
            cost: ManaGift {
                g: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Trample],
        },
        Case {
            id: "moorland_inquisitor",
            cost: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            keywords: &[Keyword::FirstStrike],
        },
        Case {
            id: "pestilent_wolf",
            cost: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            keywords: &[Keyword::Deathtouch],
        },
        Case {
            id: "prakhata_pillar-bug",
            cost: ManaGift {
                b: 1,
                ..Default::default()
            },
            keywords: &[Keyword::Lifelink],
        },
        Case {
            id: "stonefare_crocodile",
            cost: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            keywords: &[Keyword::Lifelink],
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

fn activate(e: &mut GameEngine, case: &Case, source: u32) {
    give_mana(e, 0, gift(&case.cost));
    apply_ability(e, 0, source, 0, vec![])
        .unwrap_or_else(|error| panic!("{}: activate the keyword grant: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    resolve_entire_stack_two_player(e);
}

fn check_keywords(e: &GameEngine, object: u32, keywords: &[Keyword], expect: bool, id: &str) {
    for keyword in keywords {
        assert_eq!(
            e.effective_has_keyword(object, *keyword),
            expect,
            "{id}: {keyword:?}"
        );
    }
}

#[test]
fn q37_self_keyword_activated_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(37_040_001);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    activate(&mut e, case, source);
    check_keywords(&e, source, case.keywords, true, case.id);
    check_keywords(&e, other, case.keywords, false, "grizzly_bears");
    end_active_turn(&mut e, 0);
    check_keywords(&e, source, case.keywords, false, case.id);
}

#[test]
fn q37_self_keyword_activated_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(37_040_002);
    let source = inject_creature_with_stats(&mut e, 0, case.id, 1, 2);
    activate(&mut e, case, source);
    check_keywords(&e, source, case.keywords, true, case.id);
    let body = (e.effective_power(source), e.effective_toughness(source));
    assert_eq!(
        body,
        (Some(1), Some(2)),
        "the activation does not change the body"
    );
}

#[test]
fn q37_self_keyword_activated_cards_grant_only_the_source() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_041_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let own_other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");
        activate(&mut e, case, source);
        check_keywords(&e, source, case.keywords, true, case.id);
        check_keywords(&e, own_other, case.keywords, false, "own other");
        check_keywords(&e, opponent, case.keywords, false, "opponent");
    }
}

#[test]
fn q37_self_keyword_activated_cards_expire_at_cleanup() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_042_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        activate(&mut e, case, source);
        check_keywords(&e, source, case.keywords, true, case.id);
        end_active_turn(&mut e, 0);
        check_keywords(&e, source, case.keywords, false, case.id);
    }
}

#[test]
fn q37_self_keyword_activated_cards_reject_missing_mana_and_foreign_activation() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_043_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        apply_ability(&mut e, 0, source, 0, vec![]).expect_err("the activation needs mana");
        check_keywords(&e, source, case.keywords, false, case.id);
        give_mana(&mut e, 1, gift(&case.cost));
        apply_ability(&mut e, 1, source, 0, vec![]).expect_err("only the controller activates");
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
    }
}

#[test]
fn q37_self_keyword_activated_phyrexian_activation_can_pay_two_life() {
    let case = &cases()[5];
    let mut e = engine(37_044_000);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    assert_eq!(e.state.players[0].life, 20);
    let command = {
        let mut command = activate_ability_with_costs(source, 0, vec![], vec![]);
        if let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(
            ref mut activate,
        )) = command.cmd
        {
            activate
                .flex_payments
                .push(tricerules_proto::ruled::v1::FlexPipPayment {
                    pip_index: 0,
                    pay_life: true,
                    mana_color: 0,
                });
        }
        command
    };
    e.apply_scenario_command(0, &command)
        .expect("pay {G/P} with 2 life");
    assert_eq!(
        e.state.players[0].life, 18,
        "the life payment replaces one green mana"
    );
    resolve_entire_stack_two_player(&mut e);
    check_keywords(&e, source, case.keywords, true, case.id);
}
