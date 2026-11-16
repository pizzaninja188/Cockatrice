//! Actual-card scenarios for the queue-v23 damage-a-creature-and-gain-life cohort.
//!
//! Expectations are printed Oracle values. CR 115.1 fixes the target, CR 120.3
//! deals damage, CR 119.3 gains life, and CR 608.2b makes an illegal target
//! fizzle the whole spell so no life is gained.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

const ANTI_VENOM: &str = "anti-venom,_horrifying_healer";

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: i32,
    sorcery: bool,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "pharikas_cure",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
            amount: 2,
            sorcery: false,
        },
        Case {
            id: "essence_extraction",
            mana: ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            amount: 3,
            sorcery: false,
        },
        Case {
            id: "douse_in_gloom",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            amount: 2,
            sorcery: false,
        },
        Case {
            id: "vicious_hunger",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
            amount: 2,
            sorcery: true,
        },
        Case {
            id: "last_kiss",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            amount: 2,
            sorcery: false,
        },
        Case {
            id: "sorins_thirst",
            mana: ManaGift {
                b: 2,
                ..Default::default()
            },
            amount: 2,
            sorcery: false,
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
        deck_with("swamp", &["grizzly_bears", ANTI_VENOM]),
        deck_with("island", &["grizzly_bears"]),
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
fn damage_creature_lifegain_batch_canary_cards_deal_exact_damage_and_gain_life() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "pharikas_cure" | "essence_extraction"))
    {
        let mut e = engine(12_800_000);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let life = e.state.players[0].life;
        cast_at(&mut e, &case, bear);
        assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(e.state.players[0].life, life + case.amount, "{}", case.id);
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: no damage to the player",
            case.id
        );
    }
}

#[test]
fn damage_creature_lifegain_batch_cards_mark_nonlethal_damage_and_gain_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_810_000 + index as u64);
        let giant = inject_creature_with_stats(&mut e, 1, "hill_giant", 4, 4);
        let life = e.state.players[0].life;
        cast_at(&mut e, &case, giant);
        assert_eq!(
            e.state.objects[&giant].damage,
            u32::try_from(case.amount).unwrap(),
            "{}: exact damage marked",
            case.id
        );
        assert_eq!(
            e.state.objects[&giant].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[0].life, life + case.amount, "{}", case.id);
    }
}

#[test]
fn damage_creature_lifegain_batch_cards_still_gain_life_when_damage_is_prevented() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_820_000 + index as u64);
        ensure_in_hand(&mut e, 0, ANTI_VENOM);
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 5,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, ANTI_VENOM);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast Anti-Venom");
        resolve_entire_stack_two_player(&mut e);
        let anti = battlefield_object_for_card(&e, 0, ANTI_VENOM);
        let life = e.state.players[0].life;
        cast_at(&mut e, &case, anti);
        assert_eq!(e.state.players[0].life, life + case.amount, "{}", case.id);
        assert_eq!(
            e.state.objects[&anti].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&anti].damage, 0,
            "{}: damage prevented",
            case.id
        );
    }
}

#[test]
fn damage_creature_lifegain_batch_cards_fizzle_a_stale_target_without_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_830_000 + index as u64);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
            .expect("cast at the bear");
        e.state.players[1].battlefield.retain(|id| *id != bear);
        e.state.players[1].graveyard.push(bear);
        e.state.objects.get_mut(&bear).expect("bear").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(bear).or_default() += 1;
        let life = e.state.players[0].life;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life, life,
            "{}: no life on a fizzle",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn damage_creature_lifegain_batch_cards_respect_instant_and_sorcery_timing() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_845_000 + index as u64);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let life = e.state.players[0].life;
        end_active_turn(&mut e, 0);
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.priority_player_id(),
            e.state.players[1].id,
            "{}: opponent has priority",
            case.id
        );
        e.apply_scenario_command(e.state.players[1].id, &pass())
            .expect("opponent passes priority");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        let command = cast_spell(slot, target_object(bear));
        if case.sorcery {
            assert!(
                e.apply_scenario_command(0, &command).is_err(),
                "{}: Vicious Hunger cannot be cast on the opponent's turn",
                case.id
            );
            assert_eq!(
                pool(&e),
                before,
                "{}: rejected cast spends no mana",
                case.id
            );
            assert!(!e.state.players[0].hand.is_empty(), "{}", case.id);
            assert!(e.state.stack.is_empty(), "{}", case.id);
            assert_eq!(
                e.state.objects[&bear].zone,
                Zone::Battlefield,
                "{}",
                case.id
            );
        } else {
            e.apply_scenario_command(0, &command)
                .unwrap_or_else(|error| panic!("{}: instant cast: {error:?}", case.id));
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard, "{}", case.id);
            assert_eq!(e.state.players[0].life, life + case.amount, "{}", case.id);
        }
    }
}

#[test]
fn damage_creature_lifegain_batch_cards_reject_a_noncreature_target_without_paying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_840_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "accorders_shield");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let before = pool(&e);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
                .is_err(),
            "{}: a noncreature is not a legal target",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert!(!e.state.players[0].hand.is_empty(), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
