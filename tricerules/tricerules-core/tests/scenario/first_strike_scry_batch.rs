//! Actual-card scenarios for the reviewed pump / first strike / scry cohort.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c choose the target as the
//! spell is cast; CR 613.4c applies the pump; CR 702.7a-b grants first strike damage;
//! CR 701.22a defines Scry 1; CR 514.2 ends both temporary effects; CR 608.2b makes
//! an illegal only target fizzle the spell with no scry.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: i32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "coming_in_hot",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
        },
        Case {
            id: "daring_escape",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
        },
        Case {
            id: "furious_bellow",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
        },
        Case {
            id: "storm_strike",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card, "grizzly_bears"]),
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

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first pass");
    engine
        .apply_scenario_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn cast_at_target(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    let cast_slot = e.state.players[0].hand[hand_slot];
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    cast_slot
}

#[test]
fn first_strike_scry_batch_cards_pump_grant_first_strike_and_park_the_scry_choice() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_660_000 + index as u64, case.id);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let top = seat_on_top(&mut e, 0, &["storm_crow", "hill_giant"]);
        let slot = cast_at_target(&mut e, &case, bear);
        let parked = resolve_top_stack(&mut e);
        assert_eq!(
            e.effective_power(bear),
            Some((2 + case.power) as u32),
            "{}: printed pump",
            case.id
        );
        assert_eq!(e.effective_toughness(bear), Some(2), "{}", case.id);
        assert!(
            e.effective_has_keyword(bear, Keyword::FirstStrike),
            "{}: gained first strike",
            case.id
        );
        let choice = find_resolution_choice(&parked).expect("scry parks a choice");
        assert_eq!(choice.choice_kind(), ChoiceKind::LibraryTop, "{}", case.id);
        assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
        assert_eq!((choice.min, choice.max), (0, 1), "{}", case.id);
        assert_eq!(
            choice.candidate_object_ids,
            vec![top[0]],
            "{}: only the top card is offered",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![top[0]]))
            .expect("bottom the scried card");
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        assert_eq!(
            *e.state.players[0].library.back().expect("library"),
            top[0],
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn first_strike_scry_batch_cards_deal_first_strike_damage_before_the_regular_step() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_661_000 + index as u64, case.id);
        let attacker = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_at_target(&mut e, &case, attacker);
        let parked = resolve_top_stack(&mut e);
        if let Some(choice) = find_resolution_choice(&parked) {
            let bottom = choice.candidate_object_ids.clone();
            e.apply_scenario_command(0, &submit_resolution_choice(bottom))
                .expect("complete the scry choice");
        }
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the pumped attacker");
        e.apply_scenario_command(0, &pass()).expect("active pass");
        e.apply_scenario_command(1, &pass()).expect("defender pass");
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare the vanilla blocker");
        e.apply_scenario_command(0, &pass()).expect("active pass");
        e.apply_scenario_command(1, &pass()).expect("defender pass");
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::FirstStrikeDamage,
            "{}: first-strike step",
            case.id
        );
        assert_ne!(
            e.state.objects.get(&blocker).map(|object| object.zone),
            Some(Zone::Battlefield),
            "{}: the blocker dies to first-strike damage",
            case.id
        );
        assert_eq!(
            e.state.objects[&attacker].damage, 0,
            "{}: the first-strike attacker takes no return damage",
            case.id
        );
    }
}

#[test]
fn first_strike_scry_batch_cards_expire_the_pump_and_grant_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_662_000 + index as u64, case.id);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_at_target(&mut e, &case, bear);
        let parked = resolve_top_stack(&mut e);
        if let Some(choice) = find_resolution_choice(&parked) {
            let bottom = choice.candidate_object_ids.clone();
            e.apply_scenario_command(0, &submit_resolution_choice(bottom))
                .expect("complete the scry choice");
        }
        assert_eq!(
            e.effective_power(bear),
            Some((2 + case.power) as u32),
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
        assert!(
            !e.effective_has_keyword(bear, Keyword::FirstStrike),
            "{}: first strike expires",
            case.id
        );
    }
}

#[test]
fn first_strike_scry_batch_cards_fizzle_a_stale_target_without_scrying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_663_000 + index as u64, case.id);
        let bear = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let slot = cast_at_target(&mut e, &case, bear);
        e.state.players[0]
            .battlefield
            .retain(|object| *object != bear);
        e.state.players[0].graveyard.push(bear);
        e.state.objects.get_mut(&bear).expect("bear").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(bear).or_default() += 1;
        let library_before = e.state.players[0].library.clone();
        let resolution = resolve_top_stack(&mut e);
        assert!(
            find_resolution_choice(&resolution).is_none(),
            "{}: a fizzled spell offers no scry",
            case.id
        );
        assert_eq!(
            e.state.players[0].library, library_before,
            "{}: no card leaves the library",
            case.id
        );
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: the spell goes to its owner's graveyard",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn first_strike_scry_batch_cards_reject_a_noncreature_target_without_paying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_664_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let pool_before = pool(&e);
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal target",
            case.id
        );
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert_eq!(pool(&e), pool_before, "{}: no mana spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
