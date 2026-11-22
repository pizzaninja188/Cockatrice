//! Actual-card scenarios for the queue-v35 target can't-block cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 govern the mana
//! activation, CR 509.1b makes a later block declaration illegal, and
//! CR 611.2a/514.2 end the restriction at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    cost: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "chainwhip_cyclops",
            cost: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            body: (4, 4),
        },
        Case {
            id: "lambholt_harrier",
            cost: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "sower_of_chaos",
            cost: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            body: (4, 3),
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
        deck_with("mountain", &[card]),
        deck_with("forest", &["grizzly_bears", "savannah_lions"]),
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
    assert!(
        !e.state.objects[&source].tapped,
        "{}: the ability has no tap cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

fn attack_and_reach_blockers(e: &mut GameEngine, attacker: u32) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(e);
}

#[test]
fn q35_cant_block_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(35_050_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    activate(&mut e, case, source, target);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 1, target),
        vec!["Can't block"],
        "Sower of Chaos restricts the chosen creature"
    );
}

#[test]
fn q35_cant_block_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(35_050_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    activate(&mut e, case, source, target);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 1, target),
        vec!["Can't block"],
        "Chainwhip Cyclops restricts the chosen creature"
    );
}

#[test]
fn q35_cant_block_cards_forbid_only_the_chosen_blocker() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_051_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        let restricted = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let ordinary = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");
        let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        activate(&mut e, case, source, restricted);
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 1, restricted),
            vec!["Can't block"],
            "{}: the chosen creature is restricted",
            case.id
        );
        assert!(
            zone_view_rules_annotation_labels(&mut e, 1, ordinary).is_empty(),
            "{}: another creature is unaffected",
            case.id
        );
        attack_and_reach_blockers(&mut e, attacker);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.blocker_id == restricted),
            "{}: the restricted creature is not offered as a blocker",
            case.id
        );
        assert!(
            legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.blocker_id == ordinary),
            "{}: the ordinary creature may still block",
            case.id
        );
    }
}

#[test]
fn q35_cant_block_cards_expire_at_cleanup() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_052_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate(&mut e, case, source, target);
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 1, target),
            vec!["Can't block"],
            "{}: the restriction exists during the turn",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 1, target).is_empty(),
            "{}: the restriction ends with the turn",
            case.id
        );
    }
}

#[test]
fn q35_cant_block_cards_reject_illegal_targets_and_foreign_controllers() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_053_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
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

        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 1, gift(&case.cost));
        e.apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut e, 1, source, 0, target_object(target)).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert!(
            zone_view_rules_annotation_labels(&mut e, 1, target).is_empty(),
            "{}: no restriction was applied",
            case.id
        );
    }
}
