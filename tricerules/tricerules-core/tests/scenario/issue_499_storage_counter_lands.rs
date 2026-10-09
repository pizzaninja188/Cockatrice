//! Issue #499's three pinned storage-counter lands and bounded X/split mana activation.
//!
//! Exact Scryfall Oracle identities checked by the issue owner: Dreadship Reef
//! `130a8cf5-1354-4d17-91c8-c073642eb3db`, Calciform Pools
//! `2031bc31-81cc-407a-8615-29832f586bbc`, and Saltcrusted Steppe
//! `021e4165-2f02-4bd4-86ca-cb7bf4c9e23d`. CR 107.1b and 107.3a govern the bounded X choice;
//! CR 605.1a, 605.2, and 605.3a-b govern the mana ability and payment timing; CR 614.16 and
//! 106.6a cover the independently tested counter-placement and mana-output replacement hooks.

use super::helpers::*;
use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{
    AbilityCost, AbilitySourceZone, Color, CounterKind, IdentifiedAbility, Layout, ManaCost,
    SpellEffectKind,
};
use tricerules_core::state::CopiableValues;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    BeginSpellCast, CommitSpellCast, PaymentMana, PaymentSelection, PreviewPayment,
    SpellCastAnnouncement,
};

type ManaPool = (u32, u32, u32, u32, u32, u32);

#[derive(Clone, Copy)]
struct StorageLand {
    id: &'static str,
    name: &'static str,
    oracle_id: &'static str,
    first_color: Color,
    second_color: Color,
}

const STORAGE_LANDS: [StorageLand; 3] = [
    StorageLand {
        id: "dreadship_reef",
        name: "Dreadship Reef",
        oracle_id: "130a8cf5-1354-4d17-91c8-c073642eb3db",
        first_color: Color::Blue,
        second_color: Color::Black,
    },
    StorageLand {
        id: "calciform_pools",
        name: "Calciform Pools",
        oracle_id: "2031bc31-81cc-407a-8615-29832f586bbc",
        first_color: Color::White,
        second_color: Color::Blue,
    },
    StorageLand {
        id: "saltcrusted_steppe",
        name: "Saltcrusted Steppe",
        oracle_id: "021e4165-2f02-4bd4-86ca-cb7bf4c9e23d",
        first_color: Color::Green,
        second_color: Color::White,
    },
];

fn mana_pool(engine: &GameEngine) -> ManaPool {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn storage_engine(seed: u64, card_id: &str, storage: u32) -> (GameEngine, u32) {
    let mut engine = semantic::main_phase(seed);
    let source = inject_permanent_on_battlefield(&mut engine, 0, card_id);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("storage land")
        .set_counter(CounterKind::Storage, storage);
    (engine, source)
}

fn activate_storage_mana(
    engine: &GameEngine,
    source: u32,
    x_value: u32,
    first_color_count: u32,
) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 2, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.x_value = x_value;
    activation.mana_split_first_color_count = first_color_count;
    command
}

fn ability_info(
    engine: &mut GameEngine,
    source: u32,
    index: usize,
) -> tricerules_proto::ruled::v1::AbilityInfo {
    let batch = engine.initial_response_batch();
    batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source)
                .and_then(|object| object.activated_abilities.get(index))
                .cloned(),
            _ => None,
        })
        .expect("land publishes its activated ability")
}

fn color_count(pool: &mut [u32; 6], color: Color, count: u32) {
    let slot = match color {
        Color::White => 0,
        Color::Blue => 1,
        Color::Black => 2,
        Color::Red => 3,
        Color::Green => 4,
    };
    pool[slot] += count;
}

fn expected_split(first: Color, second: Color, x: u32, first_count: u32) -> ManaPool {
    let mut pool = [0; 6];
    color_count(&mut pool, first, first_count);
    color_count(&mut pool, second, x - first_count);
    (pool[0], pool[1], pool[2], pool[3], pool[4], pool[5])
}

fn install_static_fixture(engine: &mut GameEngine, source: u32, definition: StaticAbilityDef) {
    let mut face = tricerules_cards::registry::global()
        .get("island")
        .expect("Island fixture face")
        .primary_face()
        .clone();
    face.static_abilities =
        vec![IdentifiedAbility::fallback("static_01", definition).expect("fixture ability id")];
    let display_name = face.name.clone();
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("replacement fixture")
        .copiable_values = Some(CopiableValues {
        source_card_id: "island".into(),
        source_face_index: 0,
        face,
        room_faces: None,
        display_name,
    });
}

#[test]
fn issue_499_registers_complete_typed_storage_counter_lands() {
    let registry = tricerules_cards::registry::global();
    for card_data in STORAGE_LANDS {
        assert_eq!(
            card_data.oracle_id.len(),
            36,
            "exact Oracle identity constant"
        );
        let card = registry
            .get(card_data.id)
            .unwrap_or_else(|| panic!("missing complete definition for {}", card_data.name));
        assert_eq!(card.id, card_data.id);
        assert_eq!(card.name, card_data.name);
        assert_eq!(card.layout, Layout::Normal);
        assert_eq!(card.face_count(), 1);

        let face = card.primary_face();
        assert_eq!(face.face_id.as_str(), card_data.id);
        assert_eq!(face.name, card_data.name);
        assert!(face.mana_cost.is_empty());
        assert_eq!(face.types, vec!["Land".to_string()]);
        assert!(face.colors().is_empty());
        assert_eq!(face.activated_abilities.len(), 3);

        let colorless = &face.activated_abilities[0];
        assert_eq!(colorless.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(colorless.costs, vec![AbilityCost::Tap]);
        assert!(colorless.is_mana_ability());
        assert_eq!(
            colorless.mana_options().unwrap(),
            &[tricerules_cards::ManaAmount {
                c: 1,
                ..Default::default()
            }]
        );

        let add_storage = &face.activated_abilities[1];
        assert_eq!(
            add_storage.costs,
            vec![
                AbilityCost::Mana(ManaCost::parse("{1}").unwrap()),
                AbilityCost::Tap,
            ]
        );
        assert!(matches!(
            add_storage.effect.as_slice(),
            [SpellEffectKind::PutCounters {
                subject: tricerules_cards::primitives::EffectSubject::Source,
                counter: CounterKind::Storage,
                count: tricerules_cards::Amount::Fixed(1),
            }]
        ));
        assert!(!add_storage.is_mana_ability());

        let x_mana = &face.activated_abilities[2];
        assert_eq!(
            x_mana.costs,
            vec![
                AbilityCost::Mana(ManaCost::parse("{1}").unwrap()),
                AbilityCost::RemoveXStorageCountersFromSource,
            ]
        );
        assert_eq!(
            x_mana.storage_counter_split_mana(),
            Some((card_data.first_color, card_data.second_color))
        );
        assert!(x_mana.mana_options().is_none());
        assert!(x_mana.is_mana_ability());
    }
}

#[test]
fn issue_499_publishes_engine_authoritative_bounded_x_choice() {
    let (mut engine, source) = storage_engine(499_001, "dreadship_reef", 4);
    let info = ability_info(&mut engine, source, 2);
    let choice = info
        .x_counter_mana_choice
        .expect("storage land publishes X/split prompt metadata");
    assert_eq!(choice.counter_label, "storage");
    assert_eq!(choice.max_x, 4);
    assert_eq!(choice.first_color, "U");
    assert_eq!(choice.second_color, "B");
    assert_eq!(info.mana_cost, "{1}");
}

#[test]
fn issue_499_all_storage_land_pairs_support_zero_midpoint_and_endpoint_splits() {
    for (land_index, card_data) in STORAGE_LANDS.into_iter().enumerate() {
        for (choice_index, (x_value, first_count)) in
            [(0, 0), (2, 1), (4, 0), (4, 4)].into_iter().enumerate()
        {
            let seed = 499_010 + land_index as u64 * 10 + choice_index as u64;
            let (mut engine, source) = storage_engine(seed, card_data.id, 4);
            engine.state.players[0].mana_pool.colorless = 1;
            let info = ability_info(&mut engine, source, 2);
            let choice = info
                .x_counter_mana_choice
                .expect("the engine publishes the current maximum");
            assert_eq!(choice.max_x, 4);
            let command = activate_storage_mana(&engine, source, x_value, first_count);
            semantic::accepted(&mut engine, 0, &command);
            assert_eq!(
                mana_pool(&engine),
                expected_split(
                    card_data.first_color,
                    card_data.second_color,
                    x_value,
                    first_count,
                ),
                "{} X={x_value}, first color={first_count}",
                card_data.name
            );
            assert_eq!(
                engine.state.objects[&source].counter_count(CounterKind::Storage),
                4 - x_value,
                "exactly X Storage counters are removed"
            );
            assert!(
                !engine.state.objects[&source].tapped,
                "line 3 has no tap cost"
            );
            assert!(engine.state.stack.is_empty(), "mana ability uses no stack");
            assert!(engine.state.pending_resolution.is_none());
        }
    }
}

#[test]
fn issue_499_rejects_invalid_x_split_and_stale_generation_atomically() {
    for (x_value, first_count) in [(5, 0), (1, 2)] {
        let (mut engine, source) =
            storage_engine(499_050 + u64::from(x_value), "dreadship_reef", 4);
        engine.state.players[0].mana_pool.colorless = 1;
        let before_pool = mana_pool(&engine);
        let before_counters = engine.state.objects[&source].counter_count(CounterKind::Storage);
        let before_revision = engine.state.command_index;
        let command = activate_storage_mana(&engine, source, x_value, first_count);
        engine
            .apply_command(0, &command)
            .expect_err("X above the live maximum or a split above X is forged input");
        assert_eq!(mana_pool(&engine), before_pool);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Storage),
            before_counters
        );
        assert_eq!(engine.state.command_index, before_revision);
        assert!(engine.state.stack.is_empty());
    }

    let (mut engine, source) = storage_engine(499_053, "dreadship_reef", 4);
    engine.state.players[0].mana_pool.colorless = 1;
    let stale = activate_storage_mana(&engine, source, 4, 2);
    let generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or_default();
    engine
        .state
        .zone_change_generation
        .insert(source, generation + 1);
    let before_pool = mana_pool(&engine);
    let before_counters = engine.state.objects[&source].counter_count(CounterKind::Storage);
    let before_revision = engine.state.command_index;
    engine
        .apply_command(0, &stale)
        .expect_err("activation choice is bound to the observed source generation");
    assert_eq!(mana_pool(&engine), before_pool);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Storage),
        before_counters
    );
    assert_eq!(engine.state.command_index, before_revision);
}

#[test]
fn issue_499_generic_one_is_paid_atomically_with_x_storage_counters() {
    let (mut engine, source) = storage_engine(499_054, "dreadship_reef", 4);
    let before_counters = engine.state.objects[&source].counter_count(CounterKind::Storage);
    let command = activate_storage_mana(&engine, source, 4, 2);
    engine
        .apply_command(0, &command)
        .expect_err("the printed {1} cost is still required at X=4");
    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Storage),
        before_counters
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_499_each_land_taps_for_colorless_and_line_two_pays_and_places_storage() {
    for (index, card_data) in STORAGE_LANDS.into_iter().enumerate() {
        let (mut engine, source) = storage_engine(499_060 + index as u64 * 2, card_data.id, 0);
        let command = activate_ability_for(&engine, source, 0, vec![]);
        semantic::accepted(&mut engine, 0, &command);
        assert_eq!(
            mana_pool(&engine),
            (0, 0, 0, 0, 0, 1),
            "{} line 1",
            card_data.name
        );
        assert!(
            engine.state.objects[&source].tapped,
            "{} line 1 taps",
            card_data.name
        );
        assert!(
            engine.state.stack.is_empty(),
            "{} line 1 is a mana ability",
            card_data.name
        );

        let (mut engine, source) = storage_engine(499_061 + index as u64 * 2, card_data.id, 0);
        engine.state.players[0].mana_pool.colorless = 1;
        let command = activate_ability_for(&engine, source, 1, vec![]);
        semantic::accepted(&mut engine, 0, &command);
        assert!(
            engine.state.objects[&source].tapped,
            "{} line 2 taps",
            card_data.name
        );
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Storage),
            0
        );
        assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0));
        assert_eq!(
            engine.state.stack.len(),
            1,
            "counter placement is not a mana ability"
        );
        semantic::complete(&mut engine, 8, |_| None).require_exercised();
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Storage),
            1,
            "{} line 2 resolves exactly one Storage counter",
            card_data.name
        );
    }
}

#[test]
fn issue_499_counter_placement_replacement_doubles_effect_placement_only() {
    let (mut engine, source) = storage_engine(499_062, "dreadship_reef", 0);
    let fixture = inject_permanent_on_battlefield(&mut engine, 0, "island");
    install_static_fixture(
        &mut engine,
        fixture,
        StaticAbilityDef::DoubleEffectCountersPlacedOnPermanentsYouControl,
    );
    engine.state.players[0].mana_pool.colorless = 1;

    let command = activate_ability_for(&engine, source, 1, vec![]);
    semantic::accepted(&mut engine, 0, &command);
    semantic::complete(&mut engine, 8, |_| None).require_exercised();
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Storage),
        2,
        "the resolving line-two effect doubles one Storage counter"
    );
}

#[test]
fn issue_499_tap_for_mana_replacement_applies_to_line_one_not_line_three() {
    let (mut engine, source) = storage_engine(499_063, "dreadship_reef", 2);
    let fixture = inject_permanent_on_battlefield(&mut engine, 0, "island");
    install_static_fixture(
        &mut engine,
        fixture,
        StaticAbilityDef::MultiplyManaFromTappedPermanents { multiplier: 2 },
    );
    let command = activate_ability_for(&engine, source, 0, vec![]);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 2));
    assert!(engine.state.objects[&source].tapped);

    let (mut engine, source) = storage_engine(499_064, "dreadship_reef", 2);
    let fixture = inject_permanent_on_battlefield(&mut engine, 1, "island");
    install_static_fixture(
        &mut engine,
        fixture,
        StaticAbilityDef::MultiplyManaFromTappedPermanents { multiplier: 3 },
    );
    let command = activate_ability_for(&engine, source, 0, vec![]);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(
        mana_pool(&engine),
        (0, 0, 0, 0, 0, 1),
        "an opponent's multiplier does not modify the activating player's output"
    );

    let (mut engine, source) = storage_engine(499_065, "dreadship_reef", 2);
    let own_fixture = inject_permanent_on_battlefield(&mut engine, 0, "island");
    let opponent_fixture = inject_permanent_on_battlefield(&mut engine, 1, "island");
    install_static_fixture(
        &mut engine,
        own_fixture,
        StaticAbilityDef::MultiplyManaFromTappedPermanents { multiplier: 2 },
    );
    install_static_fixture(
        &mut engine,
        opponent_fixture,
        StaticAbilityDef::MultiplyManaFromTappedPermanents { multiplier: 3 },
    );
    let command = activate_ability_for(&engine, source, 0, vec![]);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(
        mana_pool(&engine),
        (0, 0, 0, 0, 0, 2),
        "only the activating player's static multiplier applies in mixed control"
    );

    let (mut engine, source) = storage_engine(499_066, "dreadship_reef", 2);
    let fixture = inject_permanent_on_battlefield(&mut engine, 0, "island");
    install_static_fixture(
        &mut engine,
        fixture,
        StaticAbilityDef::MultiplyManaFromTappedPermanents { multiplier: 2 },
    );
    engine.state.players[0].mana_pool.colorless = 1;
    let command = activate_storage_mana(&engine, source, 2, 2);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(mana_pool(&engine), (0, 2, 0, 0, 0, 0));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Storage),
        0
    );
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_499_storage_mana_preview_and_commit_share_x_split_validation() {
    let (mut engine, source) = storage_engine(499_067, "dreadship_reef", 3);
    engine.state.players[0].mana_pool.colorless = 1;
    let mut command = activate_storage_mana(&engine, source, 2, 1);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.payment = Some(PaymentSelection {
        mana: Some(PaymentMana {
            c: 1,
            ..Default::default()
        }),
        ..Default::default()
    });
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_ref() else {
        unreachable!()
    };
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            revision: engine.state.command_index,
            activate_ability: Some(activation.clone()),
            ..Default::default()
        },
    );
    assert!(preview.valid && preview.complete, "{preview:?}");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Storage),
        3,
        "previewing does not debit storage counters"
    );
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.payment = preview.selection;
    activation.restricted_mana = preview.restricted_mana;
    engine
        .apply_command(0, &command)
        .expect("commit accepts the same choice accepted by preview");
    assert_eq!(mana_pool(&engine), (0, 1, 1, 0, 0, 0));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Storage),
        1
    );

    for (x_value, first_count) in [(4, 0), (2, 3)] {
        let (mut engine, source) =
            storage_engine(499_068 + u64::from(x_value), "dreadship_reef", 3);
        engine.state.players[0].mana_pool.colorless = 1;
        let before_pool = mana_pool(&engine);
        let before_counters = engine.state.objects[&source].counter_count(CounterKind::Storage);
        let before_revision = engine.state.command_index;
        let command = activate_storage_mana(&engine, source, x_value, first_count);
        let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_ref() else {
            unreachable!()
        };
        let preview = engine.preview_payment(
            0,
            &PreviewPayment {
                revision: engine.state.command_index,
                activate_ability: Some(activation.clone()),
                ..Default::default()
            },
        );
        assert!(
            !preview.valid,
            "forged X/split should fail preview: {preview:?}"
        );
        assert!(
            preview
                .error
                .contains("invalid storage mana X or color split"),
            "preview should report the shared choice-validation error: {preview:?}"
        );
        let error = engine
            .apply_command(0, &command)
            .expect_err("the same forged X/split is rejected on commit");
        assert!(
            error
                .to_string()
                .contains("invalid storage mana X or color split"),
            "commit should report the same choice-validation error: {error}"
        );
        assert_eq!(mana_pool(&engine), before_pool);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::Storage),
            before_counters
        );
        assert_eq!(engine.state.command_index, before_revision);
        assert!(engine.state.stack.is_empty());
    }
}

fn begin_opt_cast(engine: &mut GameEngine) -> u64 {
    let opt = inject_card_into_hand(engine, 0, "opt");
    let hand_index = engine.state.players[0]
        .hand
        .iter()
        .position(|candidate| *candidate == opt)
        .expect("Opt is in hand");
    let Some(Cmd::CastSpell(cast)) = cast_spell(hand_index, vec![]).cmd else {
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
        .expect("begin paying for Opt");
    engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("spell payment stays open")
        .transaction_id
}

#[test]
fn issue_499_mana_ability_is_usable_during_cast_payment_without_using_stack() {
    let (mut engine, source) = storage_engine(499_065, "dreadship_reef", 1);
    engine.state.players[0].mana_pool.colorless = 1;
    let transaction_id = begin_opt_cast(&mut engine);
    assert!(engine.state.pending_spell_cast.is_some());

    let command = activate_storage_mana(&engine, source, 1, 1);
    semantic::accepted(&mut engine, 0, &command);
    assert!(engine.state.pending_spell_cast.is_some());
    assert_eq!(mana_pool(&engine), (0, 1, 0, 0, 0, 0));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Storage),
        0
    );
    assert!(engine.state.stack.is_empty());

    let proposed = CommitSpellCast {
        transaction_id,
        payment: Some(PaymentSelection {
            mana: Some(PaymentMana {
                u: 1,
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
        .expect("use the storage-land mana to finish casting Opt");
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(mana_pool(&engine), (0, 0, 0, 0, 0, 0));
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].card_id, "opt");
}
