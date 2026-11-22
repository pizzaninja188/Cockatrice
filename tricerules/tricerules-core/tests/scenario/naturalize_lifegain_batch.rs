//! Actual-card scenarios for the queue-v23 artifact-or-enchantment destroy plus
//! life cohort.
//!
//! Expectations are printed Oracle values. CR 115.1 fixes the target, CR 701.8
//! destroys, CR 119.3 gains life, and CR 608.2b makes an illegal target fizzle
//! the whole spell so no life is gained.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    amount: i32,
    sorcery: bool,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "natural_end",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            amount: 3,
            sorcery: false,
        },
        Case {
            id: "springsage_ritual",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            amount: 4,
            sorcery: false,
        },
        Case {
            id: "appetite_for_the_unnatural",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            amount: 2,
            sorcery: false,
        },
        Case {
            id: "invoke_the_divine",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            amount: 4,
            sorcery: false,
        },
        Case {
            id: "sanctify",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            amount: 3,
            sorcery: true,
        },
        Case {
            id: "solemn_offering",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            amount: 4,
            sorcery: true,
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
        deck_with("plains", &["grizzly_bears"]),
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

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let life = e.state.players[0].life;
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    assert_eq!(e.state.stack.len(), 1, "{}: spell on the stack", case.id);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Battlefield,
        "{}: target is still legal before resolution",
        case.id
    );
    assert_eq!(
        e.state.players[0].life, life,
        "{}: no life before resolution",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn naturalize_lifegain_batch_canary_cards_destroy_an_artifact_and_gain_the_printed_life() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "appetite_for_the_unnatural" | "solemn_offering"))
    {
        let mut e = engine(12_700_000);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "accorders_shield");
        let life = e.state.players[0].life;
        cast_at(&mut e, &case, artifact);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[0].life, life + case.amount, "{}", case.id);
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: only the caster gains",
            case.id
        );
    }
}

#[test]
fn naturalize_lifegain_batch_cards_destroy_an_enchantment_and_gain_exact_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_710_000 + index as u64);
        let enchantment = inject_permanent_on_battlefield(&mut e, 1, "impact_tremors");
        let life = e.state.players[0].life;
        cast_at(&mut e, &case, enchantment);
        assert_eq!(
            e.state.objects[&enchantment].zone,
            Zone::Graveyard,
            "{}: enchantment destroyed",
            case.id
        );
        assert_eq!(e.state.players[0].life, life + case.amount, "{}", case.id);
    }
}

#[test]
fn naturalize_lifegain_batch_cards_gain_life_when_an_indestructible_target_survives() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_720_000 + index as u64);
        let myr = inject_permanent_on_battlefield(&mut e, 1, "darksteel_myr");
        let life = e.state.players[0].life;
        cast_at(&mut e, &case, myr);
        assert_eq!(
            e.state.objects[&myr].zone,
            Zone::Battlefield,
            "{}: indestructible target survives",
            case.id
        );
        assert_eq!(e.state.players[0].life, life + case.amount, "{}", case.id);
    }
}

#[test]
fn naturalize_lifegain_batch_cards_fizzle_a_stale_target_without_gaining_life() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_730_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "accorders_shield");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
            .expect("cast at the artifact");
        e.state.players[1].battlefield.retain(|id| *id != artifact);
        e.state.players[1].graveyard.push(artifact);
        e.state.objects.get_mut(&artifact).expect("artifact").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(artifact).or_default() += 1;
        let life = e.state.players[0].life;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life, life,
            "{}: no life on a fizzle",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(
            count_card_id_in_graveyard(&e, 0, case.id),
            1,
            "{}: the fizzled spell reaches its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn naturalize_lifegain_batch_cards_respect_instant_and_sorcery_timing() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_745_000 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "accorders_shield");
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
        let command = cast_spell(slot, target_object(artifact));
        if case.sorcery {
            assert!(
                e.apply_scenario_command(0, &command).is_err(),
                "{}: a sorcery cannot be cast on the opponent's turn",
                case.id
            );
            assert!(!e.state.players[0].hand.is_empty(), "{}", case.id);
            assert_eq!(
                pool(&e),
                before,
                "{}: rejected cast spends no mana",
                case.id
            );
            assert!(e.state.stack.is_empty(), "{}", case.id);
            assert_eq!(
                e.state.objects[&artifact].zone,
                Zone::Battlefield,
                "{}",
                case.id
            );
        } else {
            e.apply_scenario_command(0, &command)
                .unwrap_or_else(|error| panic!("{}: instant cast: {error:?}", case.id));
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(
                e.state.objects[&artifact].zone,
                Zone::Graveyard,
                "{}",
                case.id
            );
            assert_eq!(e.state.players[0].life, life + case.amount, "{}", case.id);
        }
    }
}

#[test]
fn naturalize_lifegain_batch_cards_reject_a_creature_target_without_paying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(12_740_000 + index as u64);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let before = pool(&e);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
                .is_err(),
            "{}: a creature is not an artifact or enchantment",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert!(!e.state.players[0].hand.is_empty(), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(
            e.state.objects[&bear].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
