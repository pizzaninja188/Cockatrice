//! Actual-card scenarios for the reviewed trample / landfall cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a makes a land entering under the
//! controller's control trigger landfall; CR 611.2c/613.4c apply the pump to the
//! source only; CR 702.19b assigns lethal damage to blockers then excess to the
//! defending player; CR 514.2 ends the pump at cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    pump: (u32, u32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "grove_rumbler",
            mana: ManaGift {
                r: 1,
                g: 1,
                c: 2,
                ..Default::default()
            },
            body: (3, 3),
            pump: (2, 2),
        },
        Case {
            id: "makindi_sliderunner",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 1),
            pump: (1, 1),
        },
        Case {
            id: "skyclave_geopede",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            body: (3, 1),
            pump: (2, 2),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("mountain", &["llanowar_elves"]),
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

fn cast_creature(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let _slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn landfall_trample_batch_cards_apply_the_printed_landfall_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_680_000 + index as u64, case.id);
        let source = cast_creature(&mut e, &case);
        assert!(
            e.effective_has_keyword(source, Keyword::Trample),
            "{}: printed trample",
            case.id
        );
        let bystander = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        let land = move_ready_to_battlefield(&mut e, 0, "mountain");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.players[0].battlefield.contains(&land),
            "{}: the land entered",
            case.id
        );
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0 + case.pump.0),
            "{}: landfall pumps power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1 + case.pump.1),
            "{}: landfall pumps toughness",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: source only",
            case.id
        );
    }
}

#[test]
fn landfall_trample_batch_cards_scope_landfall_to_their_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_681_000 + index as u64, case.id);
        let source = cast_creature(&mut e, &case);
        let opponent_land = move_ready_to_battlefield(&mut e, 1, "mountain");
        resolve_entire_stack_two_player(&mut e);
        assert!(
            e.state.players[1].battlefield.contains(&opponent_land),
            "{}: the opponent's land entered",
            case.id
        );
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0),
            "{}: an opponent's land does not trigger this landfall",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1),
            "{}",
            case.id
        );
    }
}

#[test]
fn landfall_trample_batch_cards_stack_multiple_landfall_triggers_and_expire() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_682_000 + index as u64, case.id);
        let source = cast_creature(&mut e, &case);
        move_ready_to_battlefield(&mut e, 0, "mountain");
        resolve_entire_stack_two_player(&mut e);
        move_ready_to_battlefield(&mut e, 0, "mountain");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0 + 2 * case.pump.0),
            "{}: each land adds a separate trigger",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1 + 2 * case.pump.1),
            "{}: each land adds a separate trigger",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(source),
            Some(case.body.0),
            "{}: CR 514.2 restores power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1),
            "{}: CR 514.2 restores toughness",
            case.id
        );
    }
}

#[test]
fn landfall_trample_batch_cards_trample_excess_damage_over_a_blocker() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_683_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let blocker = inject_creature_with_stats(&mut e, 1, "llanowar_elves", 1, 1);
        assert_eq!(
            (e.effective_power(blocker), e.effective_toughness(blocker)),
            (Some(1), Some(1)),
            "{}: printed blocker body",
            case.id
        );
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the trample attacker");
        e.apply_scenario_command(0, &pass()).expect("active pass");
        e.apply_scenario_command(1, &pass()).expect("defender pass");
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare the 1/1 blocker");
        e.apply_scenario_command(0, &pass())
            .expect("active pass blockers");
        e.apply_scenario_command(1, &pass())
            .expect("defender pass blockers");
        assert!(
            e.state.combat.as_ref().unwrap().assign_combat_damage_phase,
            "{}: trample opens damage assignment",
            case.id
        );
        let life_before = e.state.players[1].life;
        let excess = case.body.0 - 1;
        e.apply_scenario_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 1)], excess),
        )
        .expect("assign lethal and trample excess");
        assert_eq!(
            e.state.objects[&blocker].zone,
            Zone::Graveyard,
            "{}: the blocker dies",
            case.id
        );
        assert_eq!(
            e.state.players[1].life,
            life_before - i32::try_from(excess).expect("trample damage fits"),
            "{}: excess trample damage",
            case.id
        );
    }
}
