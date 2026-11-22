//! Actual-card scenarios for the queue-v38 flying-only damage cohort.
//!
//! Expectations are printed Oracle values. CR 115.1/601.2c choose the target while casting;
//! CR 702.9 supplies the flying filter; CR 120.3 deals the printed damage; CR 608.2b fizzles a
//! stale target and rejects a nonflying target at cast time.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "leaf_arrow",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            amount: 3,
        },
        Case {
            id: "pierce_the_sky",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            amount: 7,
        },
        Case {
            id: "shredding_winds",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            amount: 7,
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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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

fn cast_at(
    e: &mut GameEngine,
    case: &Case,
    target: Vec<tricerules_proto::ruled::v1::TargetRef>,
) -> Result<(), String> {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target))
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))?;
    resolve_entire_stack_two_player(e);
    Ok(())
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
fn q38_flyer_damage_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(38_030_001);
    let flier = inject_creature_with_stats(&mut e, 1, "wind_drake", 2, 2);
    let ground = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_at(&mut e, case, target_object(flier)).expect("cast at the flier");
    assert_eq!(e.state.objects[&flier].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&ground].zone, Zone::Battlefield);
    assert_eq!(
        e.state.objects[&ground].damage, 0,
        "the ground creature is not a legal target"
    );
}

#[test]
fn q38_flyer_damage_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(38_030_002);
    let flier = inject_creature_with_stats(&mut e, 1, "serra_angel", 8, 8);
    cast_at(&mut e, case, target_object(flier)).expect("cast at the flier");
    assert_eq!(e.state.objects[&flier].zone, Zone::Battlefield);
    assert_eq!(
        e.state.objects[&flier].damage, 7,
        "the printed seven damage"
    );
    assert_eq!(e.state.players[1].life, 20, "only the creature is damaged");
}

#[test]
fn q38_flyer_damage_cards_deal_their_printed_damage_only_to_flying_creatures() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_031_000 + index as u64);
        let flier = inject_creature_with_stats(&mut e, 1, "serra_angel", 10, 10);
        let ground = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        cast_at(&mut e, case, target_object(flier)).expect("cast at the flier");
        assert_eq!(e.state.objects[&flier].damage, case.amount, "{}", case.id);
        assert_eq!(e.state.objects[&ground].damage, 0, "{}", case.id);

        let mut rejected = engine(38_032_000 + index as u64);
        let ground = inject_creature_with_stats(&mut rejected, 1, "grizzly_bears", 2, 2);
        assert!(
            cast_at(&mut rejected, case, target_object(ground)).is_err(),
            "{}: a nonflying creature is not a legal target",
            case.id
        );
        assert!(
            !rejected.state.players[0].hand.is_empty(),
            "{}: the card stays in hand",
            case.id
        );
    }
}

#[test]
fn q38_flyer_damage_cards_reject_player_targets_and_fizzle_on_stale_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(38_033_000 + index as u64);
        assert!(
            cast_at(&mut e, case, target_player(1)).is_err(),
            "{}: only a flying creature is a legal target",
            case.id
        );

        let mut stale = engine(38_034_000 + index as u64);
        let flier = inject_creature_with_stats(&mut stale, 1, "serra_angel", 6, 6);
        inject_card_into_hand(&mut stale, 0, case.id);
        give_mana(&mut stale, 0, gift(&case.mana));
        let slot = hand_index_for_card(&stale, 0, case.id);
        stale
            .apply_scenario_command(0, &cast_spell(slot, target_object(flier)))
            .expect("cast at the flier");
        move_to_graveyard(&mut stale, 1, flier);
        resolve_entire_stack_two_player(&mut stale);
        assert!(stale.state.stack.is_empty(), "{}", case.id);
        assert_eq!(stale.state.players[1].life, 20, "{}", case.id);
    }
}
