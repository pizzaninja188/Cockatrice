//! Actual-card scenarios for the queue-v35 target first-strike cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 and 118.3 govern the
//! mana-and-tap activation; CR 611.2a/514.2 give the keyword grant its duration
//! and CR 613 layer 6 applies it to the chosen creature only.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;

struct Case {
    id: &'static str,
    cost: ManaGift,
    body: Option<(u32, u32)>,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "fyndhorn_bow",
            cost: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: None,
        },
        Case {
            id: "icatian_scout",
            cost: ManaGift {
                c: 1,
                ..Default::default()
            },
            body: Some((1, 1)),
        },
        Case {
            id: "iron_lance",
            cost: ManaGift {
                c: 3,
                ..Default::default()
            },
            body: None,
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
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

fn activate(e: &mut GameEngine, case: &Case, source: u32, target: u32) {
    give_mana(e, 0, gift(&case.cost));
    apply_ability(e, 0, source, 0, target_object(target))
        .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    assert!(e.state.objects[&source].tapped, "{}: tap cost", case.id);
    resolve_entire_stack_two_player(e);
}

#[test]
fn q35_target_first_strike_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(35_040_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    activate(&mut e, case, source, target);
    assert!(
        e.effective_has_keyword(target, Keyword::FirstStrike),
        "Icatian Scout grants first strike"
    );
}

#[test]
fn q35_target_first_strike_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(35_040_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    activate(&mut e, case, source, target);
    assert!(
        e.effective_has_keyword(target, Keyword::FirstStrike),
        "Fyndhorn Bow grants first strike"
    );
}

#[test]
fn q35_target_first_strike_cards_grant_first_strike_to_the_chosen_creature_only() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_041_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let bystander = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        match case.body {
            Some((power, toughness)) => assert_eq!(
                (e.effective_power(source), e.effective_toughness(source)),
                (Some(power), Some(toughness)),
                "{}: printed body",
                case.id
            ),
            None => assert_eq!(
                (e.effective_power(source), e.effective_toughness(source)),
                (None, None),
                "{}: no body",
                case.id
            ),
        }
        assert!(
            !e.effective_has_keyword(target, Keyword::FirstStrike),
            "{}: no first strike before activation",
            case.id
        );
        activate(&mut e, case, source, target);
        assert!(
            e.effective_has_keyword(target, Keyword::FirstStrike),
            "{}: the chosen opponent's creature gains first strike",
            case.id
        );
        assert!(
            !e.effective_has_keyword(source, Keyword::FirstStrike),
            "{}: the source is not affected",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::FirstStrike),
            "{}: another creature is not affected",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(target, Keyword::FirstStrike),
            "{}: the grant ends with the turn",
            case.id
        );
    }
}

#[test]
fn q35_target_first_strike_cards_reject_illegal_targets_and_foreign_controllers() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_042_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(land)).is_err(),
            "{}: a land is not a legal target",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: a rejected target pays nothing",
            case.id
        );

        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 1, gift(&case.cost));
        e.apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut e, 1, source, 0, target_object(target)).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert!(
            !e.effective_has_keyword(target, Keyword::FirstStrike),
            "{}: no keyword was granted",
            case.id
        );
    }
}
