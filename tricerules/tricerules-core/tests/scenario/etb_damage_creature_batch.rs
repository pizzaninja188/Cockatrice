//! Actual-card scenarios for the reviewed enters-damage creature cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a makes the enters ability
//! trigger; CR 115.1a/601.2c chooses its creature target; CR 120.3 deals the
//! damage; CR 608.2b fizzles the trigger when that target left. The saved
//! Corrupt Eunuchs ruling requires the creature to target itself when it is the
//! only creature on the battlefield.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "corrupt_eunuchs",
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "fire_imp",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 1),
        },
        Case {
            id: "goblin_commando",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            body: (2, 2),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("forest", &["grizzly_bears", "grizzly_bears"]),
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

fn cast_until_trigger(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let _slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    pass_both_players(e);
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "{}: the damage trigger waits for a target",
        case.id
    );
    battlefield_object_for_card(e, 0, case.id)
}

fn choose_target(e: &mut GameEngine, target: u32) {
    e.apply_scenario_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                decline: false,
                selected_modes: Vec::new(),
                targets: target_object(target),
            })),
        },
    )
    .expect("choose the trigger target");
}

fn resolve_trigger(e: &mut GameEngine) {
    resolve_entire_stack_two_player(e);
}

#[test]
fn etb_damage_creature_batch_cards_deal_exactly_two_damage_to_a_chosen_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_690_000 + index as u64, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 4);
        let bystander = inject_creature_with_stats(&mut e, 1, "hill_giant", 3, 3);
        let source = cast_until_trigger(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        choose_target(&mut e, target);
        resolve_trigger(&mut e);
        assert_eq!(
            e.state.objects[&target].damage, 2,
            "{}: exactly two damage",
            case.id
        );
        assert_eq!(
            e.state.objects[&bystander].damage, 0,
            "{}: only the chosen creature",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: players are untouched",
            case.id
        );
    }
}

#[test]
fn etb_damage_creature_batch_cards_must_target_themselves_when_no_other_creature_exists() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_691_000 + index as u64, case.id);
        let source = cast_until_trigger(&mut e, &case);
        choose_target(&mut e, source);
        resolve_trigger(&mut e);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: the source is the only legal creature, takes its own two damage, and dies",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&source),
            "{}: owner's graveyard",
            case.id
        );
    }
}

#[test]
fn etb_damage_creature_batch_cards_fizzle_when_the_chosen_creature_left() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_692_000 + index as u64, case.id);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let source = cast_until_trigger(&mut e, &case);
        choose_target(&mut e, target);
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_trigger(&mut e);
        assert!(e.state.stack.is_empty(), "{}: the trigger is gone", case.id);
        assert_eq!(
            e.state.objects[&source].damage, 0,
            "{}: no damage is dealt",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn etb_damage_creature_batch_cards_offer_only_creature_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_693_000 + index as u64, case.id);
        let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 4);
        let source = cast_until_trigger(&mut e, &case);
        let ability_key = u64::from(source) << 32;
        let offered = &e.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
            [&ability_key]
            .groups[0]
            .valid_permanent_ids;
        assert!(
            offered.contains(&creature),
            "{}: a creature is offered",
            case.id
        );
        assert!(
            offered.contains(&source),
            "{}: the source is itself a legal creature target",
            case.id
        );
        assert_eq!(
            offered.len(),
            2,
            "{}: every creature on the battlefield is offered",
            case.id
        );
        choose_target(&mut e, creature);
        resolve_trigger(&mut e);
        assert_eq!(
            e.state.objects[&creature].damage, 2,
            "{}: the chosen creature is damaged",
            case.id
        );
    }
}
