//! Actual-card scenarios for the queue-v19 Equipment vigilance cohort.
//!
//! Expectations are printed Oracle values. CR 301.5/702.6 define Equipment and equip;
//! CR 613.4c applies the attached pump and vigilance; CR 704.5n unattaches an Equipment
//! whose equipped creature leaves the battlefield.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    equip: ManaGift,
    delta_power: u32,
    delta_toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "accorders_shield",
            cast: ManaGift::default(),
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            delta_power: 0,
            delta_toughness: 3,
        },
        Case {
            id: "cathars_shield",
            cast: ManaGift::default(),
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            delta_power: 0,
            delta_toughness: 3,
        },
        Case {
            id: "kor_halberd",
            cast: ManaGift {
                w: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 1,
                ..Default::default()
            },
            delta_power: 1,
            delta_toughness: 1,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[]),
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

fn cast_equipment(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.cast));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast cost", case.id);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{}: on the battlefield",
        case.id
    );
    slot
}

fn equip(e: &mut GameEngine, equipment: u32, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, equipment, 0, target_object(creature))
        .unwrap_or_else(|error| panic!("{id}: equip: {error:?}"));
    pass_both_players(e);
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{id}: exact equip cost consumed"
    );
    assert_eq!(
        e.state.objects[&equipment].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(creature)),
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

#[test]
fn equipment_vigilance_batch_cards_cast_and_equip_with_exact_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_990_000 + index as u64);
        let equipment = cast_equipment(&mut e, &case);
        assert!(e.state.objects[&equipment].attached_to.is_none());
        assert_eq!(
            e.state.objects[&equipment].controller, e.state.players[0].id,
            "{}: controller",
            case.id
        );
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        equip(&mut e, equipment, creature, gift(&case.equip), case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn equipment_vigilance_batch_cards_grant_printed_pump_and_vigilance() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_991_000 + index as u64);
        let equipment = cast_equipment(&mut e, &case);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        equip(&mut e, equipment, creature, gift(&case.equip), case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(2 + case.delta_power), Some(2 + case.delta_toughness)),
            "{}: printed attached pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::Vigilance),
            "{}: vigilance",
            case.id
        );
        assert!(
            !e.effective_has_keyword(equipment, Keyword::Vigilance),
            "{}: the Equipment itself does not have vigilance",
            case.id
        );
    }
}

#[test]
fn equipment_vigilance_batch_cards_unattached_equipment_grants_nothing() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_992_000 + index as u64);
        let equipment = cast_equipment(&mut e, &case);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(2), Some(2)),
            "{}: no pump while unattached",
            case.id
        );
        assert!(
            !e.effective_has_keyword(creature, Keyword::Vigilance),
            "{}: no vigilance while unattached",
            case.id
        );
        assert_eq!(
            e.state.objects[&equipment].zone,
            Zone::Battlefield,
            "{}: the Equipment remains",
            case.id
        );
    }
}

#[test]
fn equipment_vigilance_batch_cards_equip_only_targets_creatures_you_control() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_993_000 + index as u64);
        let equipment = cast_equipment(&mut e, &case);
        let opponent_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_mana(&mut e, 0, gift(&case.equip));
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
fn equipment_vigilance_batch_cards_remain_attached_when_the_creature_dies() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_994_000 + index as u64);
        let equipment = cast_equipment(&mut e, &case);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        equip(&mut e, equipment, creature, gift(&case.equip), case.id);
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
