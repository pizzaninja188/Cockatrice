//! Actual-card scenarios for the queue-v38 Mountain-count damage cohort.
//!
//! Expectations are printed Oracle values. CR 205.3i defines Mountain as a land subtype; CR 120.3
//! reads the damage amount on resolution; CR 115.1/601.2c target a creature; CR 608.2b fizzles a
//! stale target.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "rockslide_ambush",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "seismic_strike",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "spitting_earth",
            mana: ManaGift {
                r: 1,
                c: 1,
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("mountain", &[])]);
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

fn add_mountains(e: &mut GameEngine, player: usize, count: usize) {
    for _ in 0..count {
        inject_permanent_on_battlefield(e, player, "mountain");
    }
}

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
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
fn q38_mountain_damage_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(38_080_001);
    add_mountains(&mut e, 0, 1);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 1, 1);
    cast_at(&mut e, case, target);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Graveyard,
        "one Mountain deals one damage"
    );
}

#[test]
fn q38_mountain_damage_canary_hardest() {
    let case = &cases()[2];
    let mut e = engine(38_080_002);
    add_mountains(&mut e, 0, 3);
    add_mountains(&mut e, 1, 2);
    inject_permanent_on_battlefield(&mut e, 0, "forest");
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
    cast_at(&mut e, case, target);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Graveyard,
        "only the controller's three Mountains count"
    );
}

#[test]
fn q38_mountain_damage_cards_count_only_their_controllers_mountains() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_081_000 + index as u64);
        add_mountains(&mut e, 0, 2);
        add_mountains(&mut e, 1, 4);
        inject_permanent_on_battlefield(&mut e, 0, "forest");
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 10, 10);
        cast_at(&mut e, case, target);
        assert_eq!(
            e.state.objects[&target].damage, 2,
            "{}: exactly two Mountains you control; opponents' Mountains and a Forest do not count",
            case.id
        );
    }
}

#[test]
fn q38_mountain_damage_cards_deal_zero_without_mountains_and_fizzle_on_stale_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_082_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        cast_at(&mut e, case, target);
        assert_eq!(e.state.objects[&target].damage, 0, "{}", case.id);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );

        let mut stale = engine(38_083_000 + index as u64);
        add_mountains(&mut stale, 0, 2);
        let target = inject_creature_with_stats(&mut stale, 1, "grizzly_bears", 2, 2);
        inject_card_into_hand(&mut stale, 0, case.id);
        give_mana(&mut stale, 0, gift(&case.mana));
        let slot = hand_index_for_card(&stale, 0, case.id);
        stale
            .apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast at the creature");
        move_to_graveyard(&mut stale, 1, target);
        resolve_entire_stack_two_player(&mut stale);
        assert!(stale.state.stack.is_empty(), "{}", case.id);
    }
}
