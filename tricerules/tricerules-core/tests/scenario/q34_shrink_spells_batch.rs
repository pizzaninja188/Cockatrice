//! Actual-card scenarios for the queue-v34 +P/-T shrink cohort.
//!
//! Expectations are printed Oracle values. CR 611.2a and 514.2 give the pump
//! its until-end-of-turn duration; CR 613.4c applies the negative toughness;
//! CR 115.1 makes the chosen creature the only target.
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
            id: "auger_spree",
            mana: ManaGift {
                b: 1,
                r: 1,
                c: 1,
                ..Default::default()
            },
            pump: (4, -4),
        },
        Case {
            id: "flowstone_infusion",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            pump: (2, -2),
        },
        Case {
            id: "lash_of_malice",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            pump: (2, -2),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn q34_shrink_spells_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(34_010_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    cast_at(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(7), Some(3)),
        "Flowstone Infusion gives +2/-2"
    );
}

#[test]
fn q34_shrink_spells_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(34_010_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
    cast_at(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(9), Some(1)),
        "Auger Spree gives +4/-4"
    );
}

#[test]
fn q34_shrink_spells_cards_apply_the_printed_pump_to_exactly_one_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_011_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        cast_at(&mut e, case, target);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((5 + case.pump.0) as u32),
                Some((5 + case.pump.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: only the chosen creature is affected",
            case.id
        );
    }
}

#[test]
fn q34_shrink_spells_cards_reject_noncreatures_and_fizzle_stale_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_012_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal creature target",
            case.id
        );
        assert_eq!(
            pool(&e),
            (
                case.mana.w,
                case.mana.u,
                case.mana.b,
                case.mana.r,
                case.mana.g,
                case.mana.c
            )
        );
        assert!(e.state.stack.is_empty(), "{}: nothing was cast", case.id);

        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 5, 5);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast at the creature");
        move_to_graveyard(&mut e, 1, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: the target left",
            case.id
        );
        assert!(
            e.state.players[0]
                .graveyard
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the fizzled spell is in the graveyard",
            case.id
        );
    }
}
