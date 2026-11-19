//! Actual-card scenarios for the queue-v36 self-unblockable cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the mana activation;
//! CR 509.1b makes a disobedient block illegal; CR 611.2a/514.2 end the restriction and
//! CR 701.26 / 702.10a-b are unaffected.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "agent_of_horizons",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 2,
        },
        Case {
            id: "ashioks_skulker",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 5,
        },
        Case {
            id: "blockade_runner",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
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
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn attack_and_reach_blockers(e: &mut GameEngine, attacker: u32) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    pass_both_players(e);
}

#[test]
fn q36_self_unblockable_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(36_090_001);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.mana));
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate unblockable");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, source),
        vec!["Can't be blocked"],
        "Blockade Runner publishes the restriction"
    );
    attack_and_reach_blockers(&mut e, source);
    let legal = &e.initial_response_batch().legal_by_player[&1];
    assert!(
        !legal
            .legal_block_pairs
            .iter()
            .any(|pair| pair.attacker_id == source && pair.blocker_id == blocker),
        "Blockade Runner cannot be blocked"
    );
}

#[test]
fn q36_self_unblockable_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(36_090_002);
    let source = inject_creature_with_stats(&mut e, 0, case.id, case.power, case.toughness);
    inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.mana));
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate unblockable");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed cost");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "Ashiok's Skulker keeps its printed body"
    );
    attack_and_reach_blockers(&mut e, source);
    let legal = &e.initial_response_batch().legal_by_player[&1];
    assert!(
        !legal
            .legal_block_pairs
            .iter()
            .any(|pair| pair.attacker_id == source),
        "Ashiok's Skulker cannot be blocked"
    );
}

#[test]
fn q36_self_unblockable_cards_restrict_only_the_source() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_091_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let ordinary = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate unblockable");
        resolve_entire_stack_two_player(&mut e);
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![source, ordinary]))
            .expect("declare both attackers");
        pass_both_players(&mut e);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == source),
            "{}: the restricted attacker has no legal blocker",
            case.id
        );
        assert!(
            legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == ordinary && pair.blocker_id == blocker),
            "{}: an ordinary attacker is still blockable",
            case.id
        );
    }
}

#[test]
fn q36_self_unblockable_cards_reject_declared_blocks() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_092_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate unblockable");
        resolve_entire_stack_two_player(&mut e);
        attack_and_reach_blockers(&mut e, source);
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: source,
                    blocker_id: blocker,
                }]),
            )
            .is_err(),
            "{}: the engine rejects the forbidden block",
            case.id
        );
        assert!(
            !e.state
                .combat
                .as_ref()
                .expect("combat")
                .attacking
                .is_empty(),
            "{}: the rejected declaration leaves the combat intact",
            case.id
        );
    }
}

#[test]
fn q36_self_unblockable_cards_reject_foreign_activation() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_094_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 1, gift(&case.mana));
        e.apply_scenario_command(0, &pass())
            .expect("pass priority to the foreign player");
        assert!(
            apply_ability(&mut e, 1, source, 0, vec![]).is_err(),
            "{}: a foreign player cannot activate the source",
            case.id
        );
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, source).is_empty(),
            "{}: the rejected foreign activation applies no restriction",
            case.id
        );
    }
}

#[test]
fn q36_self_unblockable_cards_expire_at_cleanup() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(36_093_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate unblockable");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 0, source),
            vec!["Can't be blocked"],
            "{}: restriction active this turn",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, source).is_empty(),
            "{}: the restriction expires at cleanup",
            case.id
        );
    }
}
