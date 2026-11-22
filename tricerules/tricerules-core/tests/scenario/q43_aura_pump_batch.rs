//! Actual-card scenarios for the queue-v43 Aura activation cohort.
//!
//! Expectations are printed Oracle values. CR 303.4a attaches the Aura to its creature target;
//! CR 602.1/602.2 define the activation; CR 118.3 pays the exact printed mana; CR 613.4c applies
//! the temporary layer-7c pump and CR 611.2a/514.2 end it at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    mana: ManaGift,
    activation: ManaGift,
    pump: (i32, i32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "blessing",
            mana: ManaGift {
                w: 2,
                ..Default::default()
            },
            activation: ManaGift {
                w: 1,
                ..Default::default()
            },
            pump: (1, 1),
        },
        Case {
            id: "flowstone_blade",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            pump: (1, -1),
        },
        Case {
            id: "firebreathing",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            pump: (1, 0),
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
    assert_eq!(
        {
            let p = &e.state.players[0].mana_pool;
            (p.white, p.blue, p.black, p.red, p.green, p.colorless)
        },
        (0, 0, 0, 0, 0, 0),
        "{}: exact activation cost",
        case.id
    );
}

#[test]
fn q43_aura_pump_canary_applies_the_printed_pump_to_the_enchanted_creature() {
    let case = &cases()[2];
    let mut e = engine(43_010_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast_aura(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(2)),
        "Firebreathing has no static pump"
    );
    let aura = battlefield_object_for_card(&e, 0, case.id);
    activate_pump(&mut e, case, aura);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(2)),
        "the printed +1/+0 applies to the enchanted creature"
    );
}

#[test]
fn q43_aura_pump_cards_apply_their_printed_values_and_expire_at_cleanup() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_011_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        cast_aura(&mut e, case, target);
        let aura = battlefield_object_for_card(&e, 0, case.id);
        activate_pump(&mut e, case, aura);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: printed activated pump",
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
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2), Some(2)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}

#[test]
fn q43_aura_pump_cards_require_the_printed_activation_mana_and_a_creature_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(43_012_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_aura(&mut e, case, target);
        let aura = battlefield_object_for_card(&e, 0, case.id);
        assert!(
            apply_ability(&mut e, 0, aura, 0, vec![]).is_err(),
            "{}: activation requires its printed mana",
            case.id
        );
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (Some(2), Some(2)),
            "{}: no pump without mana",
            case.id
        );

        let mut player_e = engine(43_013_000 + index as u64);
        inject_card_into_hand(&mut player_e, 0, case.id);
        give_mana(&mut player_e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&player_e, 0, case.id);
        assert!(
            player_e
                .apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                .is_err(),
            "{}: an Aura cannot enchant a player",
            case.id
        );
        assert!(!player_e.state.players[0].hand.is_empty(), "{}", case.id);
    }
}

#[test]
fn q43_aura_pump_cards_follow_the_attachment_at_resolution_and_keep_an_applied_bonus() {
    let case = &cases()[2];
    let mut e = engine(43_014_001);
    let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let second = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_aura(&mut e, case, first);
    let aura = battlefield_object_for_card(&e, 0, case.id);

    give_mana(&mut e, 0, gift(&case.activation));
    apply_ability(&mut e, 0, aura, 0, vec![]).expect("activate the pump");
    e.state.objects.get_mut(&aura).expect("Aura").attached_to =
        Some(AttachmentRecipient::Object(second));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(second), e.effective_toughness(second)),
        (Some(3), Some(2)),
        "the creature enchanted at resolution receives the pump"
    );
    assert_eq!(
        (e.effective_power(first), e.effective_toughness(first)),
        (Some(2), Some(2)),
        "the former recipient is not pumped"
    );

    activate_pump(&mut e, case, aura);
    assert_eq!(
        (e.effective_power(second), e.effective_toughness(second)),
        (Some(4), Some(2)),
        "the second activation stacks on the current recipient"
    );

    e.state.players[0].battlefield.retain(|id| *id != aura);
    e.state.players[0].graveyard.push(aura);
    e.state.objects.get_mut(&aura).expect("Aura").zone = tricerules_core::Zone::Graveyard;
    *e.state.zone_change_generation.entry(aura).or_default() += 1;
    assert_eq!(
        (e.effective_power(second), e.effective_toughness(second)),
        (Some(4), Some(2)),
        "the applied bonus remains after the Aura leaves"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(second), e.effective_toughness(second)),
        (Some(2), Some(2)),
        "both temporary pumps expire at cleanup"
    );
}
