//! Actual-card scenarios for the reviewed "enters, deal 1 damage to any target"
//! family.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the entry trigger,
//! CR 115 makes the target chosen as the trigger is put on the stack, and
//! CR 608.2b leaves a departed target without damage.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "akoum_boulderfoot",
            power: 4,
            toughness: 5,
        },
        Case {
            id: "blisterstick_shaman",
            power: 2,
            toughness: 1,
        },
        Case {
            id: "sparkmage_apprentice",
            power: 1,
            toughness: 1,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card, "savannah_lions"]),
        deck_with("forest", &["grizzly_bears", "unsummon"]),
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

fn choose_trigger_target(targets: Vec<tricerules_proto::ruled::v1::TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets,
        })),
    }
}

#[test]
fn etb_damage_any_target_batch_cards_deal_one_damage_to_any_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_210_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert_eq!(
            e.state.pending_triggers.len(),
            1,
            "{}: the entry trigger is staged",
            case.id
        );
        e.apply_scenario_command(0, &choose_trigger_target(target_player(1)))
            .expect("target the opponent");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, 19,
            "{}: exactly one damage to the opponent",
            case.id
        );

        let mut creature_e = engine(9_211_000 + index as u64, case.id);
        let bear = move_ready_to_battlefield(&mut creature_e, 1, "grizzly_bears");
        let creature_source = move_ready_to_battlefield(&mut creature_e, 0, case.id);
        assert_eq!(creature_e.state.pending_triggers.len(), 1, "{}", case.id);
        creature_e
            .apply_scenario_command(0, &choose_trigger_target(target_object(bear)))
            .expect("target the creature");
        resolve_entire_stack_two_player(&mut creature_e);
        assert_eq!(
            creature_e.state.objects[&bear].damage, 1,
            "{}: exactly one damage marked on the creature",
            case.id
        );
        assert!(
            creature_e.state.players[1].battlefield.contains(&bear),
            "{}: the 2/2 survives one damage",
            case.id
        );
        assert_eq!(
            creature_e.state.players[1].life, 20,
            "{}: the creature's controller takes no damage",
            case.id
        );
        assert!(
            !creature_e.state.objects[&creature_source].tapped,
            "{}: the source is not tapped",
            case.id
        );
    }
}

#[test]
fn etb_damage_any_target_batch_cards_choose_the_target_when_the_trigger_resolves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_212_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        // "Any target" includes the source's own controller.
        e.apply_scenario_command(0, &choose_trigger_target(target_player(0)))
            .expect("target the source's controller");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].life, 19,
            "{}: the chosen player is the only player damaged",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: the other player is untouched",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: the source is not tapped",
            case.id
        );
    }
}

#[test]
fn etb_damage_any_target_batch_cards_do_no_damage_when_the_target_is_gone() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_213_000 + index as u64, case.id);
        let bystander = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
        let bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.pending_triggers.len(), 1, "{}", case.id);
        e.apply_scenario_command(0, &choose_trigger_target(target_object(bear)))
            .expect("target the bear");
        e.apply_scenario_command(0, &pass())
            .expect("the trigger's controller passes priority");
        ensure_in_hand(&mut e, 1, "unsummon");
        give_mana(
            &mut e,
            1,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let unsummon = hand_index_for_card(&e, 1, "unsummon");
        e.apply_scenario_command(1, &cast_spell(unsummon, target_object(bear)))
            .expect("return the target to its owner's hand in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&bear].zone,
            Zone::Hand,
            "{}: the target left the battlefield",
            case.id
        );
        assert!(
            e.state.players[1].hand.contains(&bear),
            "{}: the target is in its owner's hand",
            case.id
        );
        assert_eq!(
            e.state.objects[&bear].damage, 0,
            "{}: the stale trigger deals no damage",
            case.id
        );
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: the trigger's controller takes no damage",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: the opponent takes no damage",
            case.id
        );
        assert_eq!(
            e.state.objects[&bystander].damage, 0,
            "{}: an unrelated creature takes no damage",
            case.id
        );
    }
}
