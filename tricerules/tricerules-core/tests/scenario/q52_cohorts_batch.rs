//! Actual-card scenarios for the queue-v52 destroy-artifact-or-land cohort.
//!
//! Expectations are printed Oracle values. CR 701.8 defines destruction,
//! CR 701.19c prevents regeneration, CR 119.3 covers the controller's life loss,
//! CR 702.29a-b covers cycling, and CR 608.2b fizzles a departed target.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{AbilitySourceZone, ActivateAbility, RuledCommand};

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "demolish",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "sunder_from_within",
            mana: ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "pillage",
            mana: ManaGift {
                r: 2,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "victorious_destruction",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
        },
        Case {
            id: "violent_impact",
            mana: ManaGift {
                r: 1,
                c: 3,
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

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("forest", &["grizzly_bears", "pewter_golem"]),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
}

fn hand_ability(e: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

#[test]
fn q52_canary_simplest_demolish_destroys_an_artifact() {
    let case = &cases()[0];
    let mut e = engine(52_010_001, case.id);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "sol_ring");
    cast_at(&mut e, case, artifact);
    assert_eq!(e.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 20, "no life loss rider");
}

#[test]
fn q52_canary_hardest_victorious_destruction_destroys_a_land_and_drains() {
    let case = &cases()[3];
    let mut e = engine(52_010_002, case.id);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    cast_at(&mut e, case, land);
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 19, "the land's controller loses 1");
    assert_eq!(e.state.players[0].life, 20, "the caster does not lose life");
}

#[test]
fn q52_cards_accept_each_branch_and_reject_other_types() {
    for (index, case) in cases().iter().enumerate() {
        for (branch, card) in [("artifact", "sol_ring"), ("land", "forest")] {
            let mut e = engine(52_011_000 + index as u64, case.id);
            let target = inject_permanent_on_battlefield(&mut e, 1, card);
            cast_at(&mut e, case, target);
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Graveyard,
                "{}: {branch} branch",
                case.id
            );
        }
        for (invalid, card) in [("creature", "grizzly_bears"), ("enchantment", "pacifism")] {
            let mut e = engine(52_012_000 + index as u64, case.id);
            let target = inject_permanent_on_battlefield(&mut e, 1, card);
            inject_card_into_hand(&mut e, 0, case.id);
            give_mana(&mut e, 0, gift(&case.mana));
            let slot = hand_index_for_card(&e, 0, case.id);
            assert!(
                e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
                    .is_err(),
                "{}: {invalid} is not a legal target",
                case.id
            );
        }
    }
}

#[test]
fn q52_cards_fizzle_a_departed_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(52_013_000 + index as u64, case.id);
        let target = inject_permanent_on_battlefield(&mut e, 1, "sol_ring");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast the destruction spell");
        inject_card_into_hand(&mut e, 0, "shatter");
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        );
        let shatter = hand_index_for_card(&e, 0, "shatter");
        e.apply_scenario_command(0, &cast_spell(shatter, target_object(target)))
            .expect("destroy the target in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
        assert!(
            e.state.players[1].graveyard.contains(&target),
            "{}: the target is in its owner's graveyard",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: a stale target produces no life loss",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}: stack is empty", case.id);
    }
}

#[test]
fn q52_pillage_prevents_regeneration() {
    let case = &cases()[2];
    let mut e = engine(52_020_001, case.id);
    let golem = inject_permanent_on_battlefield(&mut e, 1, "pewter_golem");
    e.state
        .objects
        .get_mut(&golem)
        .unwrap()
        .regeneration_shields = 1;
    cast_at(&mut e, case, golem);
    assert_eq!(e.state.objects[&golem].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&golem].regeneration_shields, 0);
}

#[test]
fn q52_violent_impact_cycles_for_two_and_draws() {
    let case = &cases()[4];
    let mut e = engine(52_030_001, case.id);
    ensure_in_hand(&mut e, 0, case.id);
    let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
    let drawn = inject_library_card(&mut e, 0, "grizzly_bears");
    e.state.players[0]
        .library
        .retain(|candidate| *candidate != drawn);
    e.state.players[0].library.push_front(drawn);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(&e, card))
        .expect("activate Cycling {2}");
    assert_eq!(
        e.state.objects[&card].zone,
        Zone::Graveyard,
        "cycling discards itself"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&drawn].zone,
        Zone::Hand,
        "cycling draws one"
    );
    assert!(e.state.players[0].hand.contains(&drawn));
}
