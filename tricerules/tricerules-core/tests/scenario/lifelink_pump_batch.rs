//! Actual-card scenarios for the queue-v18 lifelink pump cohort.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c choose the target at
//! cast, CR 611.2a/514.2 bound the pump, CR 613.4c applies it, and CR 702.15a-b
//! makes damage dealt by the creature also gain its controller that much life.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "grotesque_mutation",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            pump: (3, 1),
        },
        Case {
            id: "moment_of_heroism",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            pump: (2, 2),
        },
        Case {
            id: "mortals_ardor",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            pump: (1, 1),
        },
    ]
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

fn cast_pump(e: &mut GameEngine, case: &Case, target: u32) {
    give_mana(e, 0, gift(&case.mana));
    let _slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
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
fn lifelink_pump_batch_cards_pump_and_grant_lifelink_to_the_chosen_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_870_000 + index as u64);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
        cast_pump(&mut e, &case, creature);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: exact printed pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::Lifelink),
            "{}: lifelink",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(3), Some(3)),
            "{}: only the chosen creature",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, Keyword::Lifelink),
            "{}: only the chosen creature",
            case.id
        );
    }
}

#[test]
fn lifelink_pump_batch_cards_may_target_an_opponents_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_871_000 + index as u64);
        let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        cast_pump(&mut e, &case, opposing);
        assert_eq!(
            (e.effective_power(opposing), e.effective_toughness(opposing)),
            (
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: the pump applies to the opponent's creature",
            case.id
        );
        assert!(
            e.effective_has_keyword(opposing, Keyword::Lifelink),
            "{}: the opponent's creature gains lifelink",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn lifelink_pump_batch_cards_gain_life_when_the_pumped_creature_deals_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_872_000 + index as u64);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        cast_pump(&mut e, &case, attacker);
        let damage = 2 + case.pump.0;
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        e.apply_scenario_command(0, &pass()).expect("active pass");
        e.apply_scenario_command(1, &pass()).expect("defender pass");
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the pumped attacker");
        e.apply_scenario_command(0, &pass()).expect("active pass");
        e.apply_scenario_command(1, &pass()).expect("defender pass");
        e.apply_scenario_command(0, &pass()).expect("active pass");
        e.apply_scenario_command(1, &pass())
            .expect("defender pass to combat damage");
        assert_eq!(
            e.state.players[1].life,
            20 - damage,
            "{}: the unblocked attacker deals its pumped power",
            case.id
        );
        assert_eq!(
            e.state.players[0].life,
            20 + damage,
            "{}: CR 702.15b gains that much life",
            case.id
        );
    }
}

#[test]
fn lifelink_pump_batch_cards_fizzle_when_the_target_left_and_reject_a_noncreature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_873_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let pool_before = pool(&e);
        let _slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal creature target",
            case.id
        );
        assert_eq!(pool(&e), pool_before, "{}: no mana spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
        give_mana(&mut e, 0, gift(&case.mana));
        let _slot = inject_card_into_hand(&mut e, 0, case.id);
        let fizzle_hand = hand_index_for_card(&e, 0, case.id);
        let fizzle_slot = e.state.players[0].hand[fizzle_hand];
        e.apply_scenario_command(0, &cast_spell(fizzle_hand, target_object(target)))
            .expect("cast at the creature");
        move_to_graveyard(&mut e, 1, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&fizzle_slot].zone,
            Zone::Graveyard,
            "{}: the spell fizzles",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: no lifelink without damage",
            case.id
        );
    }
}

#[test]
fn lifelink_pump_batch_cards_expire_the_pump_and_lifelink_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_874_000 + index as u64);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_pump(&mut e, &case, creature);
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(2), Some(2)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
        assert!(
            !e.effective_has_keyword(creature, Keyword::Lifelink),
            "{}: CR 514.2 removes lifelink",
            case.id
        );
    }
}
