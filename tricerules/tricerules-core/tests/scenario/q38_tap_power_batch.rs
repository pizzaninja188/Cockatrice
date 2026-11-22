//! Actual-card scenarios for the queue-v38 power-restricted tapper cohort.
//!
//! Expectations are printed Oracle values and the pinned rulings: the power of the target is
//! checked both as the ability is activated and as it resolves. CR 602.1/118.3 pay {T}; CR 115.1
//! targets a creature; CR 701.26 taps; CR 608.2b fizzles an illegal target on resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    body: (u32, u32),
    limit: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "aysen_bureaucrats",
            body: (1, 1),
            limit: 2,
        },
        Case {
            id: "epharas_warden",
            body: (1, 2),
            limit: 3,
        },
        Case {
            id: "kor_line-slinger",
            body: (0, 1),
            limit: 3,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
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

#[test]
fn q38_tap_power_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(38_070_001);
    let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
    let legal = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let too_big = inject_creature_with_stats(&mut e, 1, "hill_giant", 3, 3);
    assert!(apply_ability(&mut e, 0, source, 0, target_object(too_big)).is_err());
    apply_ability(&mut e, 0, source, 0, target_object(legal)).expect("tap the 2/2");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&legal].tapped);
    assert!(e.state.objects[&source].tapped, "the tap cost is paid");
}

#[test]
fn q38_tap_power_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(38_070_002);
    let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
    let legal = inject_creature_with_stats(&mut e, 1, "hill_giant", 3, 3);
    let too_big = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
    assert!(apply_ability(&mut e, 0, source, 0, target_object(too_big)).is_err());
    apply_ability(&mut e, 0, source, 0, target_object(legal)).expect("tap the 3/3");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&legal].tapped);
}

#[test]
fn q38_tap_power_cards_accept_power_at_the_limit_and_reject_power_above_it() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_071_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        let at_limit = inject_creature_with_stats(&mut e, 1, "grizzly_bears", case.limit, 10);
        let above = inject_creature_with_stats(&mut e, 1, "grizzly_bears", case.limit + 1, 10);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(above)).is_err(),
            "{}: above the printed limit",
            case.id
        );
        apply_ability(&mut e, 0, source, 0, target_object(at_limit))
            .unwrap_or_else(|error| panic!("{}: at the limit: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert!(e.state.objects[&at_limit].tapped, "{}", case.id);
        assert!(!e.state.objects[&above].tapped, "{}", case.id);
    }
}

#[test]
fn q38_tap_power_cards_recheck_power_on_resolution() {
    let case = &cases()[0];
    let mut e = engine(38_072_001);
    let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    apply_ability(&mut e, 0, source, 0, target_object(target)).expect("target the 2/2");
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
        .expect("cast Giant Growth in response");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.state.objects[&target].tapped),
        (Some(5), false),
        "the tapper fizzles when the target's power exceeds the printed limit"
    );
}

#[test]
fn q38_tap_power_cards_require_an_untapped_source_and_a_creature_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_073_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_err(),
            "{}: a player is not a creature target",
            case.id
        );
        e.state.objects.get_mut(&source).expect("source").tapped = true;
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(target)).is_err(),
            "{}: a tapped source cannot pay {{T}}",
            case.id
        );
    }
}
