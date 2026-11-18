//! Actual-card scenarios for the queue-v32 untap-and-pump instant cohort.
//!
//! Expectations are printed Oracle values. CR 115.1b targets one creature;
//! CR 701.26 untaps it; CR 611.2a/613.4c apply the pump until end of turn; and
//! CR 608.2b fizzles a spell whose only target left.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "gerrards_command",
            mana: ManaGift {
                g: 1,
                w: 1,
                ..Default::default()
            },
            pump: (3, 3),
        },
        Case {
            id: "inspirit",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            pump: (2, 4),
        },
        Case {
            id: "ornamental_courage",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            pump: (1, 3),
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
    let decks = Some(vec![
        deck_with("forest", &[]),
        deck_with("island", &["swords_to_plowshares"]),
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

fn cast_at(e: &mut GameEngine, case: &Case, target: u32, player: i32) {
    give_mana(e, player, gift(&case.mana));
    inject_card_into_hand(e, player as usize, case.id);
    let slot = hand_index_for_card(e, player as usize, case.id);
    e.apply_scenario_command(player, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q32_untap_pump_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(32_090_001);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.state.objects.get_mut(&bear).unwrap().tapped = true;
    cast_at(&mut e, case, bear, 0);
    assert!(!e.state.objects[&bear].tapped);
    assert_eq!(e.effective_power(bear), Some(3));
    assert_eq!(e.effective_toughness(bear), Some(5));
}

#[test]
fn q32_untap_pump_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(32_090_002);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.state.objects.get_mut(&bear).unwrap().tapped = true;
    cast_at(&mut e, case, bear, 0);
    assert!(!e.state.objects[&bear].tapped);
    assert_eq!(e.effective_power(bear), Some(5));
    assert_eq!(e.effective_toughness(bear), Some(5));
}

#[test]
fn q32_untap_pump_cards_untap_and_apply_the_printed_pump_until_cleanup() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_091_000 + index as u64);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.state.objects.get_mut(&bear).unwrap().tapped = true;
        cast_at(&mut e, case, bear, 0);
        assert!(!e.state.objects[&bear].tapped, "{}", case.id);
        assert_eq!(
            e.effective_power(bear),
            Some((2 + case.pump.0) as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bear),
            Some((2 + case.pump.1) as u32),
            "{}",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(bear),
            Some(2),
            "{}: pump expires",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bear),
            Some(2),
            "{}: pump expires",
            case.id
        );
    }
}

#[test]
fn q32_untap_pump_cards_may_target_an_already_untapped_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_092_500 + index as u64);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert!(
            !e.state.objects[&bear].tapped,
            "{}: starts untapped",
            case.id
        );
        cast_at(&mut e, case, bear, 0);
        assert!(
            !e.state.objects[&bear].tapped,
            "{}: remains untapped",
            case.id
        );
        assert_eq!(
            e.effective_power(bear),
            Some((2 + case.pump.0) as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bear),
            Some((2 + case.pump.1) as u32),
            "{}",
            case.id
        );
    }
}

#[test]
fn q32_untap_pump_cards_may_target_an_opponents_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_092_000 + index as u64);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.state.objects.get_mut(&bear).unwrap().tapped = true;
        cast_at(&mut e, case, bear, 0);
        assert!(!e.state.objects[&bear].tapped, "{}", case.id);
        assert_eq!(
            e.effective_power(bear),
            Some((2 + case.pump.0) as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bear),
            Some((2 + case.pump.1) as u32),
            "{}",
            case.id
        );
    }
}

#[test]
fn q32_untap_pump_cards_reject_a_noncreature_and_fizzle_a_stale_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(32_093_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 0, "sky_diamond");
        let artifact_life = e.state.players[1].life;
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
                .is_err(),
            "{}: only a creature is legal",
            case.id
        );
        assert_eq!(e.state.players[1].life, artifact_life, "{}", case.id);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );

        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.state.objects.get_mut(&bear).unwrap().tapped = true;
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
            .expect("cast at the creature");
        e.apply_scenario_command(0, &pass()).expect("caster passes");
        give_mana(
            &mut e,
            1,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        ensure_in_hand(&mut e, 1, "swords_to_plowshares");
        let swords = hand_index_for_card(&e, 1, "swords_to_plowshares");
        e.apply_scenario_command(1, &cast_spell(swords, target_object(bear)))
            .expect("exile the target in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&bear].zone, Zone::Exile, "{}", case.id);
        assert_eq!(
            e.state.players[0]
                .graveyard
                .iter()
                .filter(|oid| e.state.objects[oid].card_id == case.id)
                .count(),
            1,
            "{}: the fizzled spell goes to the graveyard",
            case.id
        );
    }
}
