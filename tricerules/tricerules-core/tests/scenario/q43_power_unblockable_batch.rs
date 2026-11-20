//! Actual-card scenarios for the queue-v43 activated power-limited unblockable cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the activation; CR 118.3 pays
//! the printed mana and tap; CR 509.1b rejects an illegal block; CR 611.2a/514.2 end the
//! until-end-of-turn restriction at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    ability: u32,
    limit: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "tawnoss_wand",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            ability: 0,
            limit: 2,
        },
        Case {
            id: "sneaking_guide",
            mana: ManaGift {
                c: 2,
                ..Default::default()
            },
            ability: 0,
            limit: 2,
        },
        Case {
            id: "access_tunnel",
            mana: ManaGift {
                c: 3,
                ..Default::default()
            },
            ability: 1,
            limit: 3,
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card]),
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

fn activate(e: &mut GameEngine, source: u32, target: u32, mana: &ManaGift, index: u32) {
    give_mana(e, 0, gift(mana));
    apply_ability(e, 0, source, index, target_object(target))
        .unwrap_or_else(|error| panic!("activate {index}: {error:?}"));
    resolve_entire_stack_two_player(e);
    assert!(
        e.state.objects[&source].tapped,
        "the printed tap cost is paid"
    );
    let p = &e.state.players[0].mana_pool;
    assert_eq!(
        (p.white, p.blue, p.black, p.red, p.green, p.colorless),
        (0, 0, 0, 0, 0, 0),
        "the exact printed mana is spent"
    );
}

fn attack_and_reach_blockers(e: &mut GameEngine, attackers: &[u32]) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(e);
    e.apply_scenario_command(0, &declare_attackers(attackers.to_vec()))
        .expect("declare attackers");
    pass_both_players(e);
}

#[test]
fn q43_power_unblockable_canary_restricts_the_chosen_creature() {
    let case = &cases()[0];
    let mut e = engine(43_050_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    activate(&mut e, source, target, &case.mana, case.ability);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, target),
        vec!["Can't be blocked"],
        "the restriction is published on the chosen creature"
    );
    assert_eq!(
        {
            let p = &e.state.players[0].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        },
        (0, 0, 0, 0, 0, 0),
        "exact two generic"
    );
}

#[test]
fn q43_power_unblockable_cards_apply_the_printed_power_limit() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_051_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", case.limit, case.limit);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate(&mut e, source, target, &case.mana, case.ability);
        attack_and_reach_blockers(&mut e, &[target]);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == target && pair.blocker_id == blocker),
            "{}: no legal blocker is offered for the restricted attacker",
            case.id
        );
    }
}

#[test]
fn q43_power_unblockable_cards_require_the_printed_mana_and_respect_the_power_limit() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_052_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let over =
            inject_creature_with_stats(&mut e, 0, "grizzly_bears", case.limit + 1, case.limit + 1);
        assert!(
            apply_ability(&mut e, 0, source, case.ability, target_object(over)).is_err(),
            "{}: a creature above the printed power limit is not a target",
            case.id
        );
        let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert!(
            apply_ability(&mut e, 0, source, case.ability, target_object(target)).is_err(),
            "{}: the activation requires its printed mana",
            case.id
        );
        activate(&mut e, source, target, &case.mana, case.ability);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: the source stays",
            case.id
        );
        give_mana(&mut e, 0, gift(&case.mana));
        assert!(
            apply_ability(&mut e, 0, source, case.ability, target_object(target)).is_err(),
            "{}: the tapped source cannot activate again",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, target).is_empty(),
            "{}: CR 514.2 ends the restriction",
            case.id
        );
    }
}

#[test]
fn q43_power_unblockable_cards_recheck_the_power_limit_at_resolution() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_053_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", case.limit, case.limit);
        give_mana(&mut e, 0, gift(&case.mana));
        apply_ability(&mut e, 0, source, case.ability, target_object(target))
            .expect("activate the restriction");
        inject_card_into_hand(&mut e, 0, "giant_growth");
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "giant_growth");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("pump the target above the limit in response");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, target).is_empty(),
            "{}: a target above the limit at resolution is not restricted",
            case.id
        );

        let mut later = engine(43_054_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut later, 0, case.id);
        let target = inject_creature_on_battlefield(&mut later, 0, "grizzly_bears");
        activate(&mut later, source, target, &case.mana, case.ability);
        inject_card_into_hand(&mut later, 0, "giant_growth");
        give_mana(
            &mut later,
            0,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&later, 0, "giant_growth");
        later
            .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("pump the already-restricted creature");
        resolve_entire_stack_two_player(&mut later);
        assert_eq!(
            later.effective_power(target),
            Some(5),
            "{}: the pump actually raised the power",
            case.id
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut later, 0, target),
            vec!["Can't be blocked"],
            "{}: raising power after resolution does not remove the restriction",
            case.id
        );
    }
}

#[test]
fn q43_power_unblockable_access_tunnel_adds_colorless_mana() {
    let mut e = engine(43_055_001, "access_tunnel");
    let tunnel = inject_permanent_on_battlefield(&mut e, 0, "access_tunnel");
    apply_ability(&mut e, 0, tunnel, 0, vec![]).expect("activate the mana ability");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.objects[&tunnel].tapped,
        "the mana ability taps the land"
    );
    let p = &e.state.players[0].mana_pool;
    assert_eq!(
        (p.white, p.blue, p.black, p.red, p.green, p.colorless),
        (0, 0, 0, 0, 0, 1),
        "Access Tunnel adds one colorless mana"
    );
}
