//! Actual-card scenarios for the queue-v54 batch B cohorts.
//!
//! Expectations are printed Oracle values. CR 601.2c/115.1 govern targets, CR 601.2f cost
//! modifiers, CR 406.2 exile, CR 603.6a/d optional entry triggers, CR 701.8 destroy, CR 602.2
//! activation costs, CR 611.2a keyword grants and CR 514.2 until-end-of-turn expiry.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, ResolutionChoiceDecision, RuledCommand, TargetRef,
    TargetRefKind,
};

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

fn choose_permanent_targets(ids: &[u32], decline: bool) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline,
            selected_modes: Vec::new(),
            targets: ids
                .iter()
                .copied()
                .map(|object_id| TargetRef {
                    object_id,
                    kind: TargetRefKind::Permanent as i32,
                    ..Default::default()
                })
                .collect(),
        })),
    }
}

fn engine(seed: u64, p0: &[&str], p1: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("island", p0), deck_with("forest", p1)]);
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

fn cast_at(e: &mut GameEngine, card: &str, mana: ManaGift, target: u32, expect_ok: bool) {
    inject_card_into_hand(e, 0, card);
    give_mana(e, 0, gift(&mana));
    let slot = hand_index_for_card(e, 0, card);
    let result = e.apply_scenario_command(0, &cast_spell(slot, target_object(target)));
    if expect_ok {
        result.unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
        resolve_entire_stack_two_player(e);
    } else {
        assert!(result.is_err(), "{card}: illegal target must be rejected");
    }
}

// ---------------------------------------------------------------------------
// 04-white-exile-spells

#[test]
fn q54_canary_simplest_blessed_light_exiles_a_creature() {
    let mut e = engine(54_040_001, &["blessed_light"], &["grizzly_bears"]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_at(
        &mut e,
        "blessed_light",
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
        creature,
        true,
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{4}}{{W}} cost");
    assert_eq!(e.state.objects[&creature].zone, Zone::Exile);
}

#[test]
fn q54_canary_hardest_banish_from_edoras_discounts_a_tapped_target() {
    let mut e = engine(54_040_002, &["banish_from_edoras"], &["grizzly_bears"]);
    let creature = relocate_to_battlefield(&mut e, 1, "grizzly_bears", true);
    cast_at(
        &mut e,
        "banish_from_edoras",
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
        creature,
        true,
    );
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "the tapped target reduces the generic cost to {{2}}{{W}}"
    );
    assert_eq!(e.state.objects[&creature].zone, Zone::Exile);

    let mut e = engine(54_040_003, &["banish_from_edoras"], &["grizzly_bears"]);
    let creature = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
    inject_card_into_hand(&mut e, 0, "banish_from_edoras");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "banish_from_edoras");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .is_err(),
        "an untapped target does not receive the discount"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        8,
        "rejected cast keeps the card"
    );
}

#[test]
fn q54_white_exile_cards_reach_their_printed_targets() {
    let mut e = engine(54_041_001, &["blessed_light"], &["crusade"]);
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "crusade");
    cast_at(
        &mut e,
        "blessed_light",
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
        enchantment,
        true,
    );
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Exile);

    let mut e = engine(54_041_002, &["ionas_judgment"], &["crusade"]);
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "crusade");
    cast_at(
        &mut e,
        "ionas_judgment",
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
        enchantment,
        true,
    );
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Exile);

    let mut e = engine(54_041_004, &["ionas_judgment"], &["grizzly_bears"]);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_at(
        &mut e,
        "ionas_judgment",
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
        creature,
        true,
    );
    assert_eq!(e.state.objects[&creature].zone, Zone::Exile);

    let mut e = engine(54_041_003, &["hawkeyes_shot"], &["grizzly_bears"]);
    let creature = relocate_to_battlefield(&mut e, 1, "grizzly_bears", true);
    cast_at(
        &mut e,
        "hawkeyes_shot",
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
        creature,
        true,
    );
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "Hawkeye's Shot also discounts the tapped target"
    );
    assert_eq!(e.state.objects[&creature].zone, Zone::Exile);
}

#[test]
fn q54_white_exile_cards_require_the_full_cost_for_an_untapped_target() {
    for (seed, card) in [
        (54_044_001, "banish_from_edoras"),
        (54_044_002, "hawkeyes_shot"),
    ] {
        let mut e = engine(seed, &[card], &["grizzly_bears"]);
        let creature = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
        inject_card_into_hand(&mut e, 0, card);
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, card);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{card}: the discounted payment is illegal for an untapped target"
        );
        assert_eq!(
            e.state.players[0].mana_pool.white, 1,
            "{card}: kept white mana"
        );
        assert_eq!(
            e.state.players[0].mana_pool.colorless, 2,
            "{card}: kept colorless mana"
        );
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, card);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .unwrap_or_else(|error| panic!("{card}: full-cost cast: {error:?}"));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{card}: full cost paid");
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Exile,
            "{card}: untapped creature exiled at full cost"
        );
    }
}

#[test]
fn q54_blessed_light_rejects_an_artifact_target() {
    let mut e = engine(54_042_001, &["blessed_light"], &["bonesplitter"]);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    cast_at(
        &mut e,
        "blessed_light",
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
        artifact,
        false,
    );
    assert_eq!(e.state.players[0].mana_pool.white, 1, "kept mana");
    assert_eq!(e.state.players[0].mana_pool.colorless, 4);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Battlefield);
}

#[test]
fn q54_banish_from_edoras_fizzles_when_its_target_left() {
    let mut e = engine(
        54_043_001,
        &["banish_from_edoras", "doom_blade"],
        &["grizzly_bears"],
    );
    let creature = relocate_to_battlefield(&mut e, 1, "grizzly_bears", true);
    inject_card_into_hand(&mut e, 0, "banish_from_edoras");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "banish_from_edoras");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Banish from Edoras");
    inject_card_into_hand(&mut e, 0, "doom_blade");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let doom = hand_index_for_card(&e, 0, "doom_blade");
    e.apply_scenario_command(0, &cast_spell(doom, target_object(creature)))
        .expect("destroy the target in response");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

// ---------------------------------------------------------------------------
// 05-artifact-enchantment-destruction

#[test]
fn q54_canary_simplest_inspired_insurgent_sacrifices_itself_for_an_artifact() {
    let mut e = engine(54_050_001, &["inspired_insurgent"], &["bonesplitter"]);
    let insurgent = relocate_to_battlefield(&mut e, 0, "inspired_insurgent", false);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, insurgent, 0, target_object(artifact)).expect("activate");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{1}} cost");
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&insurgent].zone, Zone::Graveyard);
}

#[test]
fn q54_canary_hardest_conclave_naturalists_may_destroy_and_may_decline() {
    let mut e = engine(
        54_050_002,
        &["conclave_naturalists"],
        &["bonesplitter", "grizzly_bears"],
    );
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut e, 0, "conclave_naturalists");
    assert!(
        e.apply_scenario_command(0, &choose_permanent_targets(&[creature], false))
            .is_err(),
        "a creature is not an artifact or enchantment"
    );
    e.apply_scenario_command(0, &choose_permanent_targets(&[artifact], false))
        .expect("choose the artifact");
    resolve_entire_stack_two_player(&mut e);
    answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::SelectBranch);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);

    let mut e = engine(54_050_003, &["conclave_naturalists"], &["bonesplitter"]);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
    move_ready_to_battlefield(&mut e, 0, "conclave_naturalists");
    e.apply_scenario_command(0, &choose_permanent_targets(&[artifact], false))
        .expect("choose the artifact before the may choice");
    resolve_entire_stack_two_player(&mut e);
    answer_optional_triggered_ability_choice(&mut e, ResolutionChoiceDecision::Decline);
    assert_eq!(
        e.state.objects[&artifact].zone,
        Zone::Battlefield,
        "the may ability can decline"
    );
}

#[test]
fn q54_sylvok_replica_destroys_an_enchantment_for_green() {
    let mut e = engine(54_051_001, &["sylvok_replica"], &["crusade"]);
    let replica = relocate_to_battlefield(&mut e, 0, "sylvok_replica", false);
    let enchantment = inject_permanent_on_battlefield(&mut e, 1, "crusade");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, replica, 0, target_object(enchantment)).expect("activate");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{G}} cost");
    assert_eq!(e.state.objects[&enchantment].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&replica].zone, Zone::Graveyard);
}

#[test]
fn q54_artifact_enchantment_effects_reject_a_creature_target() {
    let mut e = engine(54_052_001, &["sylvok_replica"], &["grizzly_bears"]);
    let replica = relocate_to_battlefield(&mut e, 0, "sylvok_replica", false);
    let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    assert!(
        apply_ability(&mut e, 0, replica, 0, target_object(creature)).is_err(),
        "Sylvok Replica targets only artifacts and enchantments"
    );
    assert_eq!(e.state.players[0].mana_pool.green, 1);
    assert_eq!(e.state.objects[&replica].zone, Zone::Battlefield);
}

#[test]
fn q54_sylvok_replica_may_target_itself_and_then_fizzles() {
    let mut e = engine(54_054_001, &["sylvok_replica"], &["grizzly_bears"]);
    let replica = relocate_to_battlefield(&mut e, 0, "sylvok_replica", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, replica, 0, target_object(replica))
        .expect("Sylvok Replica may target itself");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&replica].zone,
        Zone::Graveyard,
        "the sacrifice cost was paid"
    );
    assert_eq!(e.state.stack.len(), 0, "the ability fizzled cleanly");
}

#[test]
fn q54_sylvok_replica_fizzles_when_its_target_left() {
    let mut e = engine(
        54_053_001,
        &["sylvok_replica", "doom_blade"],
        &["ornithopter"],
    );
    let replica = relocate_to_battlefield(&mut e, 0, "sylvok_replica", false);
    let thopter = inject_creature_on_battlefield(&mut e, 1, "ornithopter");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, replica, 0, target_object(thopter)).expect("activate");
    inject_card_into_hand(&mut e, 0, "doom_blade");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let doom = hand_index_for_card(&e, 0, "doom_blade");
    e.apply_scenario_command(0, &cast_spell(doom, target_object(thopter)))
        .expect("destroy the target in response");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&thopter].zone, Zone::Graveyard);
    assert!(e.state.stack.is_empty());
}

// ---------------------------------------------------------------------------
// 06-keyword-grant-activated

#[test]
fn q54_canary_simplest_weldfast_monitor_gains_menace_for_red() {
    let mut e = engine(54_060_001, &["weldfast_monitor"], &["grizzly_bears"]);
    let source = relocate_to_battlefield(&mut e, 0, "weldfast_monitor", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate menace");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{R}} cost");
    assert!(e.effective_has_keyword(source, Keyword::Menace));
    end_active_turn(&mut e, 0);
    assert!(
        !e.effective_has_keyword(source, Keyword::Menace),
        "the menace grant expired"
    );
}

#[test]
fn q54_canary_hardest_riveteers_initiate_pays_either_hybrid_half() {
    for (seed, mana, label) in [
        (
            54_060_002,
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            "black half",
        ),
        (
            54_060_003,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            "green half",
        ),
    ] {
        let mut e = engine(seed, &["riveteers_initiate"], &["grizzly_bears"]);
        let source = relocate_to_battlefield(&mut e, 0, "riveteers_initiate", false);
        give_mana(&mut e, 0, mana);
        apply_ability(&mut e, 0, source, 0, vec![]).unwrap_or_else(|error| {
            panic!("activate Riveteers Initiate with the {label}: {error:?}")
        });
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{label}: exact hybrid cost");
        assert!(
            e.effective_has_keyword(source, Keyword::Deathtouch),
            "{label}: deathtouch"
        );
    }
}

#[test]
fn q54_vectis_silencers_gains_deathtouch_for_black() {
    let mut e = engine(54_061_001, &["vectis_silencers"], &["grizzly_bears"]);
    let source = relocate_to_battlefield(&mut e, 0, "vectis_silencers", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate deathtouch");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{2}}{{B}} cost");
    assert!(e.effective_has_keyword(source, Keyword::Deathtouch));
}

#[test]
fn q54_keyword_grants_reject_missing_mana() {
    let mut e = engine(54_062_001, &["vectis_silencers"], &["grizzly_bears"]);
    let source = relocate_to_battlefield(&mut e, 0, "vectis_silencers", false);
    assert!(apply_ability(&mut e, 0, source, 0, vec![]).is_err());
    assert!(!e.effective_has_keyword(source, Keyword::Deathtouch));
    assert!(!e.state.objects[&source].tapped);
}

#[test]
fn q54_keyword_grants_expire_at_cleanup() {
    let mut e = engine(54_063_001, &["vectis_silencers"], &["grizzly_bears"]);
    let source = relocate_to_battlefield(&mut e, 0, "vectis_silencers", false);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.effective_has_keyword(source, Keyword::Deathtouch));
    end_active_turn(&mut e, 0);
    assert!(!e.effective_has_keyword(source, Keyword::Deathtouch));
}
