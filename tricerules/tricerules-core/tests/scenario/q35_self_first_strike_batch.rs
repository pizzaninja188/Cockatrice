//! Actual-card scenarios for the queue-v35 self first-strike cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 and 118.3 govern the
//! mana activation; CR 611.2a/514.2 give the keyword grant its duration and
//! CR 613 layer 6 applies it to the source only.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    cost: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "kessig_wolf",
            cost: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (3, 1),
        },
        Case {
            id: "titanium_golem",
            cost: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (3, 3),
        },
        Case {
            id: "unyielding_krumar",
            cost: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (3, 3),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card]),
        deck_with("forest", &[card]),
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

fn activate(e: &mut GameEngine, case: &Case, player: i32, source: u32) {
    give_mana(e, player, gift(&case.cost));
    apply_ability(e, player, source, 0, vec![])
        .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn q35_self_first_strike_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(35_030_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(3), Some(3)),
        "printed body"
    );
    activate(&mut e, case, 0, source);
    assert!(
        e.effective_has_keyword(source, Keyword::FirstStrike),
        "Unyielding Krumar gains first strike"
    );
}

#[test]
fn q35_self_first_strike_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(35_030_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    activate(&mut e, case, 0, source);
    assert!(
        e.effective_has_keyword(source, Keyword::FirstStrike),
        "Kessig Wolf gains first strike"
    );
}

#[test]
fn q35_self_first_strike_cards_grant_first_strike_to_the_source_only() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_031_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let bystander = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert!(
            !e.effective_has_keyword(source, Keyword::FirstStrike),
            "{}: no first strike before activation",
            case.id
        );
        activate(&mut e, case, 0, source);
        assert!(
            e.effective_has_keyword(source, Keyword::FirstStrike),
            "{}: the source gains first strike",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::FirstStrike),
            "{}: only the source is affected",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(source, Keyword::FirstStrike),
            "{}: the grant ends with the turn",
            case.id
        );
    }
}

#[test]
fn q35_self_first_strike_cards_reject_foreign_controllers() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_032_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 1, gift(&case.cost));
        e.apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut e, 1, source, 0, vec![]).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert!(
            !e.effective_has_keyword(source, Keyword::FirstStrike),
            "{}: no keyword was granted",
            case.id
        );
    }
}
