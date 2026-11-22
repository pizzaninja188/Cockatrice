//! Actual-card scenarios for the queue-v19 combat-damage prevention cycling cohort.
//!
//! Expectations are printed Oracle values. CR 614.1a and 615.1a replace combat damage
//! with zero for the turn; CR 611.2a/514.2 end the effect; CR 702.29a makes cycling a
//! hand-zone activated ability that discards the card as a cost.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::AbilitySourceZone;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ActivateAbility, RuledCommand};

struct Case {
    id: &'static str,
    mana: ManaGift,
    cycling: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "angelsong",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            cycling: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "haze_of_pollen",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            cycling: ManaGift {
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "lull",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            cycling: ManaGift {
                c: 2,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        vec!["forest".into(); 10],
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
    advance_to_declare_attackers(&mut e);
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

fn cast_before_damage(e: &mut GameEngine, case: &Case) {
    let attacker = *e.state.players[0]
        .battlefield
        .last()
        .expect("attacker on battlefield");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_scenario_command(0, &pass())
        .expect("p0 passes in declare attackers");
    inject_card_into_hand(e, 1, case.id);
    give_mana(e, 1, gift(&case.mana));
    let slot = hand_index_for_card(e, 1, case.id);
    e.apply_scenario_command(1, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    e.apply_scenario_command(1, &pass())
        .expect("p1 pass on stack");
    e.apply_scenario_command(0, &pass())
        .expect("p0 pass resolves the prevention");
}

fn hand_cycling(e: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

#[test]
fn fog_cycling_batch_cards_prevent_all_combat_damage_this_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_020_000 + index as u64);
        cast_before_damage(&mut e, &case);
        e.apply_scenario_command(0, &pass())
            .expect("p0 passes after prevention");
        e.apply_scenario_command(1, &pass())
            .expect("p1 passes to declare blockers");
        e.apply_scenario_command(0, &pass())
            .expect("p0 passes in declare blockers");
        let batch = e
            .apply_scenario_command(1, &pass())
            .expect("combat damage resolves");
        assert!(
            batch.events.iter().all(|event| {
                !matches!(
                    &event.ev,
                    Some(tricerules_proto::ruled::v1::ruled_event::Ev::LifeChanged(change))
                        if change.delta < 0
                )
            }),
            "{}: no combat life loss",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn fog_cycling_batch_cards_do_not_prevent_noncombat_loss() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_021_000 + index as u64);
        cast_before_damage(&mut e, &case);
        e.apply_scenario_command(0, &pass())
            .expect("p0 passes after prevention");
        inject_card_into_hand(&mut e, 1, "shock");
        give_mana(
            &mut e,
            1,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 1, "shock");
        e.apply_scenario_command(1, &cast_spell(slot, target_player(0)))
            .expect("shock the active player");
        e.apply_scenario_command(1, &pass()).expect("p1 pass");
        e.apply_scenario_command(0, &pass())
            .expect("p0 pass resolves shock");
        assert_eq!(
            e.state.players[0].life, 18,
            "{}: noncombat damage is not prevented",
            case.id
        );
    }
}

#[test]
fn fog_cycling_batch_cards_cycling_discards_self_and_draws_one() {
    for (index, case) in cases().into_iter().enumerate() {
        let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            10_022_000 + index as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        let source = inject_card_into_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        give_mana(&mut e, 0, gift(&case.cycling));
        e.apply_scenario_command(0, &hand_cycling(&e, source))
            .unwrap_or_else(|error| panic!("{}: cycling: {error:?}", case.id));
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: cycling discards the card as a cost",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: one card replaces the discarded card",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: one private draw",
            case.id
        );
    }
}

#[test]
fn fog_cycling_batch_cards_cycling_rejects_when_cost_unavailable() {
    for (index, case) in cases().into_iter().enumerate() {
        let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            10_023_000 + index as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        let source = inject_card_into_hand(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        assert!(
            e.apply_scenario_command(0, &hand_cycling(&e, source))
                .is_err(),
            "{}: cycling needs its exact mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Hand,
            "{}: the card stays in hand",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}",
            case.id
        );
    }
}

#[test]
fn fog_cycling_batch_cards_prevention_expires_at_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            10_024_000 + index as u64,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut e);
        give_mana(&mut e, 0, gift(&case.mana));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        e.apply_scenario_command(0, &pass()).expect("p0 pass");
        e.apply_scenario_command(1, &pass())
            .expect("p1 pass resolves the prevention");
        assert!(
            !e.state.damage_prevention_effects.is_empty(),
            "{}: prevention is active this turn",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            e.state.damage_prevention_effects.is_empty(),
            "{}: prevention expires at end of turn",
            case.id
        );
    }
}
