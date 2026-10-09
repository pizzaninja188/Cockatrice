//! Exact pinned-deck coverage for Flusterstorm.
//!
//! Oracle and rulings checked 2026-09-30. CR 702.40a governs storm and its copies; CR 608.2b
//! rechecks targets, CR 701.6a governs countering, and CR 118.12 governs the conditional payment.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

const FLUSTERSTORM: &str = "flusterstorm";

fn flusterstorm_engine(seed: u64, p0_cards: &[&str]) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", p0_cards),
            deck_with("island", &[FLUSTERSTORM]),
        ]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

fn assert_rejected_without_mutation(engine: &mut GameEngine, player: i32, command: RuledCommand) {
    let before = serde_json::to_value(&engine.state).expect("serialize pre-command state");
    assert!(engine.apply_command(player, &command).is_err());
    assert_eq!(
        serde_json::to_value(&engine.state).expect("serialize post-command state"),
        before
    );
}

#[test]
fn flusterstorm_copies_and_each_copy_resolves_its_own_one_mana_payment() {
    let mut engine = flusterstorm_engine(862_001, &["divination"]);
    ensure_in_hand(&mut engine, 0, "divination");
    ensure_in_hand(&mut engine, 1, FLUSTERSTORM);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let divination_slot = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination_slot, Vec::new()))
        .expect("cast Sorcery spell to target");
    let divination = engine.state.stack.last().expect("Divination on stack").id;
    engine.apply_command(0, &pass()).expect("yield priority");

    let flusterstorm_slot = hand_index_for_card(&engine, 1, FLUSTERSTORM);
    engine
        .apply_command(1, &cast_spell(flusterstorm_slot, stack_target(divination)))
        .expect("cast Flusterstorm at the Sorcery");
    pass_both_players(&mut engine);

    let copy_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Storm copy target choice");
    assert_eq!(copy_choice.deciding_player, 1);
    assert!(copy_choice.presentation.candidates.contains(&divination));
    engine
        .apply_command(
            1,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    chosen_object_ids: vec![divination],
                    ..Default::default()
                })),
            },
        )
        .expect("choose a legal target for the Storm copy");
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .filter(|item| item.is_copy)
            .count(),
        1
    );

    pass_both_players(&mut engine);
    let first_payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("first copy's conditional payment");
    assert_eq!(first_payment.deciding_player, 0);
    assert!(first_payment.presentation.prompt.contains("{1}"));
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay {1} for the copy");
    assert_eq!(engine.state.objects[&divination].zone, Zone::Stack);

    pass_both_players(&mut engine);
    let second_payment = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("original Flusterstorm's separate conditional payment");
    assert_eq!(second_payment.deciding_player, 0);
    assert!(second_payment.presentation.prompt.contains("{1}"));
    submit_mana_resolution_decision(&mut engine, 0, ResolutionChoiceDecision::Decline)
        .expect("decline payment for the original");

    assert_eq!(engine.state.objects[&divination].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.players[1]
            .graveyard
            .iter()
            .filter(|&&object_id| engine.state.objects[&object_id].card_id == FLUSTERSTORM)
            .count(),
        1
    );
}

#[test]
fn flusterstorm_rejects_a_creature_spell_as_its_target() {
    let mut engine = flusterstorm_engine(862_002, &["grizzly_bears"]);
    ensure_in_hand(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 1, FLUSTERSTORM);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    let bears_slot = hand_index_for_card(&engine, 0, "grizzly_bears");
    engine
        .apply_command(0, &cast_spell(bears_slot, Vec::new()))
        .expect("cast creature spell");
    let bears = engine.state.stack.last().expect("Bears on stack").id;
    engine.apply_command(0, &pass()).expect("yield priority");

    let flusterstorm_slot = hand_index_for_card(&engine, 1, FLUSTERSTORM);
    assert_rejected_without_mutation(
        &mut engine,
        1,
        cast_spell(flusterstorm_slot, stack_target(bears)),
    );
}
