//! Actual-card scenarios for the queue-v23 Equipment haste cohort.
//!
//! Expectations are printed Oracle values. CR 301.5/702.6 define Equipment and
//! equip; CR 613.4c applies the attached pump; CR 702.10 grants haste; CR 704.5n
//! unattaches an Equipment whose equipped creature leaves the battlefield.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    delta_power: u32,
    delta_toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "boots_of_speed",
            cast: ManaGift {
                r: 1,
                ..Default::default()
            },
            delta_power: 1,
            delta_toughness: 0,
        },
        Case {
            id: "crystal_slipper",
            cast: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            delta_power: 1,
            delta_toughness: 0,
        },
        Case {
            id: "strider_harness",
            cast: ManaGift {
                c: 3,
                ..Default::default()
            },
            delta_power: 1,
            delta_toughness: 1,
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
        deck_with("mountain", &[]),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_equipment(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.cast));
    let equipment = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast cost", case.id);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&equipment].zone,
        Zone::Battlefield,
        "{}: on the battlefield",
        case.id
    );
    equipment
}

fn equip(e: &mut GameEngine, equipment: u32, target: u32, id: &str) {
    give_mana(
        e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(e, 0, equipment, 0, target_object(target))
        .unwrap_or_else(|error| panic!("{id}: equip: {error:?}"));
    pass_both_players(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact equip cost");
    assert_eq!(
        e.state.objects[&equipment].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(target)),
        "{id}: attached"
    );
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn cast_bear(e: &mut GameEngine, seed_card: &str) -> u32 {
    assert_eq!(seed_card, "grizzly_bears");
    give_mana(
        e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = inject_card_into_hand(e, 0, "grizzly_bears");
    let hand_slot = hand_index_for_card(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast the creature");
    resolve_entire_stack_two_player(e);
    assert_eq!(e.state.objects[&slot].zone, Zone::Battlefield);
    slot
}

#[test]
fn equipment_haste_batch_canary_cards_cast_equip_and_grant_haste() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "boots_of_speed" | "strider_harness"))
    {
        let mut e = engine(12_400_000);
        let equipment = cast_equipment(&mut e, &case);
        assert!(e.state.objects[&equipment].attached_to.is_none());
        let creature = cast_bear(&mut e, "grizzly_bears");
        assert!(e.state.objects[&creature].summoning_sick, "{}", case.id);
        equip(&mut e, equipment, creature, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(2 + case.delta_power), Some(2 + case.delta_toughness)),
            "{}: exact printed pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::Haste),
            "{}: haste",
            case.id
        );
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![creature]))
            .expect("haste lets the equipped summoning-sick creature attack");
    }
}

#[test]
fn equipment_haste_batch_cards_cast_and_equip_with_exact_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_410_000 + index as u64);
        let equipment = cast_equipment(&mut e, &case);
        assert_eq!(
            e.state.objects[&equipment].controller, e.state.players[0].id,
            "{}: controller",
            case.id
        );
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        equip(&mut e, equipment, creature, case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn equipment_haste_batch_cards_grant_printed_pump_and_haste() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_415_000 + index as u64);
        let equipment = cast_equipment(&mut e, &case);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        equip(&mut e, equipment, creature, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(2 + case.delta_power), Some(2 + case.delta_toughness)),
            "{}: exact printed pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::Haste),
            "{}: haste",
            case.id
        );
    }
}

#[test]
fn equipment_haste_batch_cards_unattached_equipment_grants_nothing() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_420_000 + index as u64);
        let _equipment = cast_equipment(&mut e, &case);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(2), Some(2)),
            "{}: no pump while unattached",
            case.id
        );
        assert!(
            !e.effective_has_keyword(creature, Keyword::Haste),
            "{}: no haste while unattached",
            case.id
        );
    }
}

#[test]
fn equipment_haste_batch_cards_equip_only_creatures_you_control() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_430_000 + index as u64);
        let equipment = cast_equipment(&mut e, &case);
        let opponent_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, equipment, 0, target_object(opponent_creature)).is_err(),
            "{}: an opponent's creature is not a legal equip target",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert!(
            e.state.objects[&equipment].attached_to.is_none(),
            "{}: the Equipment stays unattached",
            case.id
        );
    }
}

#[test]
fn equipment_haste_batch_cards_remain_and_unattach_when_the_creature_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_440_000 + index as u64);
        let equipment = cast_equipment(&mut e, &case);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        equip(&mut e, equipment, creature, case.id);
        move_to_graveyard(&mut e, 0, creature);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&equipment].zone,
            Zone::Battlefield,
            "{}: the Equipment stays on the battlefield",
            case.id
        );
        assert!(
            e.state.objects[&equipment].attached_to.is_none(),
            "{}: the Equipment becomes unattached",
            case.id
        );
    }
}
