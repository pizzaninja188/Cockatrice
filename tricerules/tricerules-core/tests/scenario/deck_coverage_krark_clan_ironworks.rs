//! Exact deck-corpus coverage for Krark-Clan Ironworks.
//!
//! Oracle and rulings checked against Scryfall on 2026-09-26; this Oracle identity has no rulings.
//! CR 602.2b and 701.21a govern paying the artifact sacrifice cost; CR 605.1a and 605.3b classify
//! and immediately resolve the ability; CR 118.1 governs carrying out the cost.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    BeginSpellCast, CommitSpellCast, PaymentMana, PaymentSelection, PreviewPayment,
    SpellCastAnnouncement,
};

fn ironworks_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let ironworks = inject_permanent_on_battlefield(&mut engine, 0, "krark-clan_ironworks");
    (engine, ironworks)
}

fn sacrifice_for_mana(engine: &GameEngine, ironworks: u32, permanent: u32) -> RuledCommand {
    let mut command = activate_ability_with_costs(
        ironworks,
        0,
        vec![],
        vec![permanent_cost_selection(0, permanent)],
    );
    if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
        activation.expected_zone_change_generation = engine
            .state
            .zone_change_generation
            .get(&ironworks)
            .copied()
            .unwrap_or(0);
    }
    command
}

fn begin_mind_stone_cast(engine: &mut GameEngine) -> u64 {
    let mind_stone = inject_card_into_hand(engine, 0, "mind_stone");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == mind_stone)
        .expect("Mind Stone is in hand");
    let command = cast_spell(slot, vec![]);
    let Some(Cmd::CastSpell(cast)) = command.cmd else {
        unreachable!()
    };
    let announcement = SpellCastAnnouncement {
        targets: cast.targets,
        x_value: cast.x_value,
        flex_payments: cast.flex_payments,
        face_index: cast.face_index,
        selected_modes: cast.selected_modes,
        source: cast.source,
        cost_selections: cast.cost_selections,
        cast_cost_group_selections: cast.cast_cost_group_selections,
        cast_method: cast.cast_method,
        casting_permission_id: cast.casting_permission_id,
    };
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                    announcement: Some(announcement),
                })),
            },
        )
        .expect("begin paying for Mind Stone");
    engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("spell payment remains open")
        .transaction_id
}

#[test]
fn ironworks_can_sacrifice_itself_and_immediately_add_two_colorless() {
    let (mut engine, ironworks) = ironworks_engine(20_260_926);
    let batch = engine
        .apply_command(0, &sacrifice_for_mana(&engine, ironworks, ironworks))
        .expect("sacrifice the source as the cost");

    assert_eq!(engine.state.objects[&ironworks].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(
        engine.state.stack.is_empty(),
        "the mana ability uses no stack"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(!batch.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::StackPushed(_)) | Some(Ev::StackResolved(_))
        )
    }));
}

#[test]
fn ironworks_sacrifices_another_artifact_and_rejects_uncontrolled_or_nonartifact_choices() {
    let (mut engine, ironworks) = ironworks_engine(20_260_927);
    let own_artifact = inject_permanent_on_battlefield(&mut engine, 0, "mind_stone");
    let opposing_artifact = inject_permanent_on_battlefield(&mut engine, 1, "mind_stone");
    let own_land = inject_permanent_on_battlefield(&mut engine, 0, "forest");

    engine
        .apply_command(0, &sacrifice_for_mana(&engine, ironworks, own_artifact))
        .expect("sacrifice a different controlled artifact");
    assert_eq!(engine.state.objects[&own_artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&ironworks].zone, Zone::Battlefield);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(engine.state.stack.is_empty());

    for invalid in [opposing_artifact, own_land] {
        let before_mana = engine.state.players[0].mana_pool;
        let before_index = engine.state.command_index;
        engine
            .apply_command(0, &sacrifice_for_mana(&engine, ironworks, invalid))
            .expect_err("only a controlled artifact can pay the cost");
        assert_eq!(engine.state.players[0].mana_pool, before_mana);
        assert_eq!(engine.state.command_index, before_index);
        assert_eq!(engine.state.objects[&invalid].zone, Zone::Battlefield);
    }
}

#[test]
fn ironworks_mana_can_pay_for_a_spell_before_priority_returns() {
    let (mut engine, ironworks) = ironworks_engine(20_260_928);
    let transaction_id = begin_mind_stone_cast(&mut engine);
    assert!(engine.state.pending_spell_cast.is_some());

    engine
        .apply_command(0, &sacrifice_for_mana(&engine, ironworks, ironworks))
        .expect("mana ability is available during spell payment");
    assert!(engine.state.pending_spell_cast.is_some());
    assert_eq!(engine.state.objects[&ironworks].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(
        engine.state.stack.is_empty(),
        "the pending spell is not committed yet"
    );

    let proposed = CommitSpellCast {
        transaction_id,
        payment: Some(PaymentSelection {
            mana: Some(PaymentMana {
                c: 2,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            transaction_id,
            revision: engine.state.command_index,
            commit_spell_cast: Some(proposed),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("use the produced mana to commit the spell");

    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.stack.len(), 1);
    assert!(engine.state.stack[0].card_id == "mind_stone");
}
