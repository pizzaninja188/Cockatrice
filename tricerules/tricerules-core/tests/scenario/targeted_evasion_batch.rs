//! Actual-card scenarios for the queue-v21 targeted evasion cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activates the ability and
//! pays its printed mana; CR 509.1b makes a declared block illegal while the
//! restriction applies; CR 611.2a ends the grant at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    activation: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "amphin_pathmage",
            activation: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 2,
        },
        Case {
            id: "coralhelm_guide",
            activation: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
        },
        Case {
            id: "passwall_adept",
            activation: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 3,
        },
        Case {
            id: "wormhole_serpent",
            activation: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 5,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card, "grizzly_bears", "savannah_lions"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn activate(e: &mut GameEngine, source: u32, target: u32, case: &Case) {
    give_mana(e, 0, gift(&case.activation));
    e.apply_scenario_command(
        0,
        &activate_ability_for(e, source, 0, target_object(target)),
    )
    .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    resolve_entire_stack_two_player(e);
    assert!(e.state.stack.is_empty(), "{}", case.id);
}

#[test]
fn targeted_evasion_batch_canary_cards_grant_unblockable() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "passwall_adept" | "coralhelm_guide"))
    {
        let mut e = engine(9_990_000, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        activate(&mut e, source, target, &case);
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 0, target),
            vec!["Can't be blocked"],
            "{}: the grant is published",
            case.id
        );
    }
}

#[test]
fn targeted_evasion_batch_cards_grant_unblockable_with_the_exact_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_991_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        activate(&mut e, source, target, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: the ability has no tap cost",
            case.id
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 0, target),
            vec!["Can't be blocked"],
            "{}: the chosen target is restricted",
            case.id
        );
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, bystander).is_empty(),
            "{}: an untargeted creature is unaffected",
            case.id
        );
    }
}

#[test]
fn targeted_evasion_batch_cards_make_the_target_unblockable_in_combat() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_992_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate(&mut e, source, attacker, &case);
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attackers");
        pass_both_players(&mut e);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker),
            "{}: no legal blocker is offered for the restricted attacker",
            case.id
        );
    }
}

#[test]
fn targeted_evasion_batch_cards_expire_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_993_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        activate(&mut e, source, target, &case);
        end_active_turn(&mut e, 0);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, target).is_empty(),
            "{}: CR 514.2 ends the grant",
            case.id
        );
    }
}

#[test]
fn targeted_evasion_batch_cards_reject_illegal_targets_and_missing_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_994_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, "bonesplitter");
        give_mana(&mut e, 0, gift(&case.activation));
        let before = pool(&e);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(artifact))
            )
            .is_err(),
            "{}: an artifact is not a creature target",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, artifact).is_empty(),
            "{}: no restriction is created",
            case.id
        );

        let mut e = engine(9_995_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(target))
            )
            .is_err(),
            "{}: the printed mana is required",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, target).is_empty(),
            "{}",
            case.id
        );
    }
}

#[test]
fn targeted_evasion_batch_cards_drop_a_stale_target_without_returning_it() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_996_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        give_mana(&mut e, 0, gift(&case.activation));
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .expect("activate the evasion grant");
        e.state.players[0].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Exile;
        e.state.players[0].exile.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: a stale target is not returned",
            case.id
        );
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: cost remains paid",
            case.id
        );
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, target).is_empty(),
            "{}: a left-battlefield object keeps no restriction",
            case.id
        );
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, bystander).is_empty(),
            "{}: the stale target does not redirect to another creature",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn targeted_evasion_batch_cards_may_target_an_opponents_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_997_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate(&mut e, source, opposing, &case);
        let from_caster = zone_view_rules_annotation_labels(&mut e, 0, opposing);
        let from_owner = zone_view_rules_annotation_labels(&mut e, 1, opposing);
        assert!(
            from_caster.contains(&"Can't be blocked".to_string())
                || from_owner.contains(&"Can't be blocked".to_string()),
            "{}: any creature is a legal target ({from_caster:?} / {from_owner:?})",
            case.id
        );
        assert_eq!(
            e.state.objects[&opposing].controller, 1,
            "{}: the grant does not change control",
            case.id
        );
    }
}
