//! A recharging depletion land's counter rider under two counter replacements (#539).
//!
//! CR 605.3b resolves a mana ability immediately, including its trailing "Put a depletion counter
//! on this land." When two Doubling Seasons each modify that placement, CR 616.1 has the land's
//! controller choose their order; the choice parks the rest of the unstacked mana ability, the
//! mana already added stays in the pool, and any interrupted payment resumes afterwards.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::ruled_command::Cmd;

const RECHARGING_LANDS: [(&str, char); 5] = [
    ("veldt", 'G'),
    ("lava_tubes", 'B'),
    ("river_delta", 'U'),
    ("timberline_ridge", 'R'),
    ("land_cap", 'W'),
];

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn add_two_doubling_seasons(engine: &mut GameEngine) {
    for _ in 0..2 {
        inject_permanent_on_battlefield(engine, 0, "doubling_season");
    }
}

fn depletion(engine: &GameEngine, land: u32) -> u32 {
    engine.state.objects[&land].counter_count(CounterKind::Depletion)
}

fn pool_amount(engine: &GameEngine, symbol: char) -> u32 {
    let pool = &engine.state.players[0].mana_pool;
    match symbol {
        'W' => pool.white,
        'U' => pool.blue,
        'B' => pool.black,
        'R' => pool.red,
        'G' => pool.green,
        _ => unreachable!(),
    }
}

/// Activate the land's first mana option (no extra cost selections).
fn tap_first_option(engine: &mut GameEngine, land: u32) -> rv1::RuledEventBatch {
    let mut command = activate_ability_for(engine, land, 0, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for constructs an ActivateAbility command");
    };
    ability.mana_option_index = 0;
    engine
        .apply_scenario_command(0, &command)
        .expect("tap the depletion land for mana")
}

/// The pending CR 616.1 order choice: its prompt kind, decider and first candidate.
fn replacement_order_choice(engine: &GameEngine) -> u32 {
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("two applicable counter replacements need an order choice");
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(
        pending.presentation.choice_kind,
        rv1::ChoiceKind::ReplacementEffect
    );
    assert_eq!(pending.presentation.candidates.len(), 2);
    pending.presentation.candidates[0]
}

#[test]
fn recharging_land_parks_the_counter_replacement_order_and_keeps_its_mana() {
    for (index, (card_id, color)) in RECHARGING_LANDS.into_iter().enumerate() {
        let mut engine = engine(539_100 + index as u64);
        add_two_doubling_seasons(&mut engine);
        let land = inject_permanent_on_battlefield(&mut engine, 0, card_id);

        let activation = tap_first_option(&mut engine, land);
        assert!(activation
            .events
            .iter()
            .all(|event| !matches!(event.ev, Some(rv1::ruled_event::Ev::StackPushed(_)))));
        assert!(engine.state.stack.is_empty(), "{card_id}: no stack object");
        assert!(engine.state.objects[&land].tapped);
        assert_eq!(pool_amount(&engine, color), 1, "{card_id}: mana is added");
        assert_eq!(depletion(&engine, land), 0, "{card_id}: placement waits");
        let choice = replacement_order_choice(&engine);

        engine
            .apply_scenario_command(0, &pass())
            .expect_err("the mana ability's choice must be answered first");
        engine
            .apply_scenario_command(0, &submit_resolution_choice(vec![choice]))
            .expect("order the two Doubling Season replacements");
        assert!(engine.state.pending_resolution.is_none(), "{card_id}");
        assert_eq!(
            depletion(&engine, land),
            4,
            "{card_id}: both doublers modify the one depletion counter"
        );
        assert_eq!(pool_amount(&engine, color), 1, "{card_id}: mana is kept");
        engine
            .apply_scenario_command(0, &undo_mana_ability())
            .expect_err("a source rider ability is not float-undoable");
    }
}

#[test]
fn counter_order_choice_during_spell_payment_keeps_the_payment_open() {
    let mut engine = engine(539_200);
    add_two_doubling_seasons(&mut engine);
    let veldt = inject_permanent_on_battlefield(&mut engine, 0, "veldt");
    let transaction_id = begin_spell_payment(&mut engine, "elvish_mystic");

    tap_first_option(&mut engine, veldt);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert!(engine.state.pending_spell_cast.is_some());
    let choice = replacement_order_choice(&engine);

    let pending_choice = format!("{:?}", engine.state.pending_resolution);
    let cancel = rv1::RuledCommand {
        cmd: Some(Cmd::CancelSpellCast(rv1::CancelSpellCast {
            transaction_id,
        })),
    };
    engine
        .apply_scenario_command(0, &cancel)
        .expect_err("payment can't close while the mana ability is unfinished");
    assert_eq!(
        format!("{:?}", engine.state.pending_resolution),
        pending_choice
    );

    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![choice]))
        .expect("order the replacements without closing spell payment");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(depletion(&engine, veldt), 4);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert_eq!(
        engine
            .state
            .pending_spell_cast
            .as_ref()
            .expect("spell payment is still open")
            .transaction_id,
        transaction_id
    );

    commit_green_spell(&mut engine, transaction_id);
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].card_id, "elvish_mystic");
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
}

#[test]
fn counter_order_choice_during_attack_payment_resumes_the_declaration() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        539_300,
        &[0, 1],
        20,
        None,
        true,
    )
    .expect("new game");
    advance_to_declare_attackers(&mut engine);
    let attacker = engine.state.players[0].battlefield[0];
    add_two_doubling_seasons(&mut engine);
    let veldt = inject_permanent_on_battlefield(&mut engine, 0, "veldt");
    inject_permanent_on_battlefield(&mut engine, 1, "propaganda");
    engine
        .apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("begin Propaganda payment");
    assert!(engine.state.pending_attack_declaration.is_some());

    tap_first_option(&mut engine, veldt);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    let choice = replacement_order_choice(&engine);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![choice]))
        .expect("order the replacements during attack payment");
    assert_eq!(depletion(&engine, veldt), 4);
    assert_eq!(engine.state.players[0].mana_pool.green, 1);

    let pending = engine
        .state
        .pending_attack_declaration
        .as_ref()
        .expect("attack payment is still open")
        .clone();
    engine.state.players[0].mana_pool.colorless = 1;
    engine
        .apply_scenario_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(Cmd::CommitAttackDeclaration(rv1::CommitAttackDeclaration {
                    transaction_id: pending.transaction_id,
                    expected_revision: pending.revision,
                    payment: Some(rv1::PaymentSelection {
                        expected_state_revision: engine.state.command_index,
                        mana: Some(rv1::PaymentMana {
                            g: 1,
                            c: 1,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
        )
        .expect("pay Propaganda with the land's kept mana");
    assert!(engine.state.pending_attack_declaration.is_none());
    assert!(
        engine
            .state
            .combat
            .as_ref()
            .expect("combat")
            .attackers_declared
    );
}

fn begin_spell_payment(engine: &mut GameEngine, card_id: &str) -> u64 {
    inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    let Some(Cmd::CastSpell(cast)) = cast_spell(slot, vec![]).cmd else {
        unreachable!("cast announcement")
    };
    let announcement = rv1::SpellCastAnnouncement {
        as_foretold_source: None,
        targets: cast.targets,
        x_value: cast.x_value,
        flex_payments: cast.flex_payments,
        face_index: cast.face_index,
        selected_modes: cast.selected_modes,
        source: cast.source,
        prototyped: cast.prototyped,
        cost_selections: cast.cost_selections,
        cast_cost_group_selections: cast.cast_cost_group_selections,
        cast_method: cast.cast_method,
        casting_permission_id: cast.casting_permission_id,
    };
    engine
        .apply_scenario_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(rv1::BeginSpellCast {
                    announcement: Some(announcement),
                })),
            },
        )
        .expect("begin spell payment");
    engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("spell payment remains open")
        .transaction_id
}

fn commit_green_spell(engine: &mut GameEngine, transaction_id: u64) {
    let preview = engine.preview_payment_scenario(
        0,
        &rv1::PreviewPayment {
            transaction_id,
            revision: engine.state.command_index,
            commit_spell_cast: Some(rv1::CommitSpellCast {
                transaction_id,
                payment: Some(rv1::PaymentSelection {
                    mana: Some(rv1::PaymentMana {
                        g: 1,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    engine
        .apply_scenario_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(rv1::CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        )
        .expect("commit spell payment");
}
