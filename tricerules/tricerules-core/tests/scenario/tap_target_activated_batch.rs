//! Actual-card scenarios for the reviewed `{W}, {T}: Tap target creature.` family.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 govern the activated
//! ability and its cost, CR 118.3 the mana payment, CR 701.26 tapping, CR 608.2b
//! resolution revalidation, and CR 302.6 the tap-cost summoning restriction.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 9] {
    [
        Case {
            id: "gideons_lawkeeper",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "goldmeadow_harrier",
            power: 1,
            toughness: 1,
        },
        Case {
            id: "benalish_trapper",
            power: 1,
            toughness: 2,
        },
        Case {
            id: "blinding_mage",
            power: 1,
            toughness: 2,
        },
        Case {
            id: "master_decoy",
            power: 1,
            toughness: 2,
        },
        Case {
            id: "akroan_mastiff",
            power: 2,
            toughness: 2,
        },
        Case {
            id: "loxodon_mystic",
            power: 3,
            toughness: 3,
        },
        Case {
            id: "nebelgast_beguiler",
            power: 2,
            toughness: 5,
        },
        Case {
            id: "ostiary_thrull",
            power: 2,
            toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
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

fn white_pool(e: &GameEngine) -> u32 {
    e.state.players[0].mana_pool.white
}

fn give_white(e: &mut GameEngine) {
    give_mana(
        e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
}

#[test]
fn tap_target_activated_batch_cards_apply_the_printed_effect() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_030_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        assert_eq!(e.effective_power(source), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.toughness),
            "{}",
            case.id
        );
        give_white(&mut e);
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .expect("activate the tap ability");
        assert!(
            e.state.objects[&source].tapped,
            "{}: tapping the source is a cost",
            case.id
        );
        assert!(
            !e.state.objects[&target].tapped,
            "{}: the target is tapped only on resolution",
            case.id
        );
        assert_eq!(white_pool(&e), 0, "{}: the {{W}} cost was paid", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.objects[&target].tapped,
            "{}: the target creature is tapped",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tap_target_activated_batch_cards_reject_illegal_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_031_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        give_white(&mut e);
        let before = white_pool(&e);
        assert!(
            e.apply_scenario_command(
                0,
                &activate_ability_for(&e, source, 0, target_object(artifact))
            )
            .is_err(),
            "{}: an artifact is not a creature target",
            case.id
        );
        assert_eq!(white_pool(&e), before, "{}: rejected activation", case.id);
        assert!(
            !e.state.objects[&source].tapped,
            "{}: a rejected activation does not tap the source",
            case.id
        );
        assert!(
            !e.state.objects[&artifact].tapped,
            "{}: a rejected activation does not tap the artifact",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn tap_target_activated_batch_cards_handle_stale_or_expired_state() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(1_032_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_white(&mut e);
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .expect("activate the tap ability");
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Exile;
        e.state.players[1].exile.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: a stale target is not returned",
            case.id
        );
        assert!(
            !e.state.objects[&target].tapped,
            "{}: a stale target is not tapped",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the tap cost remains paid",
            case.id
        );
        assert_eq!(white_pool(&e), 0, "{}: the mana cost remains paid", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let mut sick = engine(1_032_500 + index as u64, case.id);
        let sick_source = move_ready_to_battlefield(&mut sick, 0, case.id);
        sick.state
            .objects
            .get_mut(&sick_source)
            .expect("source")
            .summoning_sick = true;
        let legal_target = inject_creature_on_battlefield(&mut sick, 1, "grizzly_bears");
        give_white(&mut sick);
        assert!(
            sick.apply_scenario_command(
                0,
                &activate_ability_for(&sick, sick_source, 0, target_object(legal_target)),
            )
            .is_err(),
            "{}: a summoning-sick creature cannot pay a tap cost",
            case.id
        );
        assert!(!sick.state.objects[&sick_source].tapped, "{}", case.id);
        assert_eq!(white_pool(&sick), 1, "{}: no mana is spent", case.id);
        assert!(sick.state.stack.is_empty(), "{}", case.id);
    }
}
