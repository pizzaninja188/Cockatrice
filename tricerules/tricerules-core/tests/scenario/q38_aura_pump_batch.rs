//! Actual-card scenarios for the queue-v38 Aura pump cohort.
//!
//! Expectations are printed Oracle values. CR 303.4a attaches the Aura to its creature target;
//! CR 613.4c applies both the static and temporary layer-7c pump; CR 611.2a/514.2 ends the
//! activated pump at cleanup; CR 602.1/118.3 pays the printed activation mana.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    static_pump: (i32, i32),
    activation: ManaGift,
    activated_pump: (i32, i32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "armor_of_faith",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            static_pump: (1, 1),
            activation: ManaGift {
                w: 1,
                ..Default::default()
            },
            activated_pump: (0, 1),
        },
        Case {
            id: "holy_armor",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            static_pump: (0, 2),
            activation: ManaGift {
                w: 1,
                ..Default::default()
            },
            activated_pump: (0, 1),
        },
        Case {
            id: "stonehands",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            static_pump: (0, 2),
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            activated_pump: (1, 0),
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

fn cast_aura(e: &mut GameEngine, case: &Case, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        {
            let p = &e.state.players[0].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        },
        (0, 0, 0, 0, 0, 0),
        "{}: exact cost",
        case.id
    );
}

fn activate_pump(e: &mut GameEngine, case: &Case, aura: u32) {
    give_mana(e, 0, gift(&case.activation));
    apply_ability(e, 0, aura, 0, vec![])
        .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q38_aura_pump_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(38_090_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast_aura(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(3)),
        "the static +1/+1 applies"
    );
    let aura = battlefield_object_for_card(&e, 0, case.id);
    activate_pump(&mut e, case, aura);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(4)),
        "the activated +0/+1 applies"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(3)),
        "only the static pump remains"
    );
}

#[test]
fn q38_aura_pump_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(38_090_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_aura(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(4)),
        "Stonehands may enchant an opponent's creature"
    );
    let aura = battlefield_object_for_card(&e, 0, case.id);
    activate_pump(&mut e, case, aura);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(4))
    );
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(4))
    );
}

#[test]
fn q38_aura_pump_cards_apply_their_printed_static_and_activated_values_to_the_attached_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_091_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        cast_aura(&mut e, case, target);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.static_pump.0) as u32),
                Some((2 + case.static_pump.1) as u32)
            ),
            "{}: static pump",
            case.id
        );
        let aura = battlefield_object_for_card(&e, 0, case.id);
        activate_pump(&mut e, case, aura);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.static_pump.0 + case.activated_pump.0) as u32),
                Some((2 + case.static_pump.1 + case.activated_pump.1) as u32)
            ),
            "{}: static plus activated pump",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: only the attached creature is pumped",
            case.id
        );
    }
}

#[test]
fn q38_aura_pump_cards_require_the_printed_activation_mana() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_092_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_aura(&mut e, case, target);
        let aura = battlefield_object_for_card(&e, 0, case.id);
        assert!(
            apply_ability(&mut e, 0, aura, 0, vec![]).is_err(),
            "{}: the activation requires its printed mana",
            case.id
        );
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.static_pump.0) as u32),
                Some((2 + case.static_pump.1) as u32)
            ),
            "{}: the pump did not resolve without mana",
            case.id
        );
    }
}

#[test]
fn q38_aura_pump_cards_require_a_creature_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_093_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                .is_err(),
            "{}: an Aura cannot enchant a player",
            case.id
        );
        assert!(!e.state.players[0].hand.is_empty(), "{}", case.id);
    }
}
