//! Actual-card coverage for Ugin, the Spirit Dragon.

use tricerules_cards::primitives::{
    AbilityCost, Amount, ManaValueLimit, PlayerRecipient, SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Color, Layout};

const UGIN: &str = "ugin,_the_spirit_dragon";

#[test]
fn ugin_has_its_exact_identity_and_three_primitive_abilities() {
    let card = tricerules_cards::registry::global()
        .get(UGIN)
        .expect("complete Ugin, the Spirit Dragon definition is registered");
    assert_eq!(card.id, UGIN);
    assert_eq!(card.name, "Ugin, the Spirit Dragon");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "ugin_the_spirit_dragon");
    assert_eq!(face.mana_cost.to_string(), "{8}");
    assert_eq!(face.colors(), Vec::<Color>::new());
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Ugin"]);
    assert_eq!(face.loyalty, Some(7));
    assert_eq!(face.activated_abilities.len(), 3);

    let plus_two = &face.activated_abilities[0];
    assert_eq!(plus_two.ability_id.as_str(), "activated_01");
    assert_eq!(
        plus_two.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(plus_two.costs, [AbilityCost::Loyalty(2)]);
    assert!(plus_two.program.is_none());
    assert!(matches!(
        plus_two.effect.as_slice(),
        [SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(3),
            target,
        }] if target.kind == TargetKind::AnyTarget
    ));
    let plus_two_targets = &plus_two
        .targeting
        .as_ref()
        .expect("+2 targets exactly one any target")
        .groups;
    assert_eq!(plus_two_targets.len(), 1);
    assert_eq!(plus_two_targets[0].min, 1);
    assert_eq!(plus_two_targets[0].max, 1);
    assert_eq!(plus_two_targets[0].effect_indices, [0]);

    let minus_x = &face.activated_abilities[1];
    assert_eq!(minus_x.ability_id.as_str(), "activated_02");
    assert_eq!(
        minus_x.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(minus_x.costs, [AbilityCost::LoyaltyX]);
    assert!(minus_x.program.is_none());
    assert!(minus_x.targeting.is_none());
    let [SpellEffectKind::ExileAll {
        kind,
        max_mana_value_limit: Some(ManaValueLimit::ActivatedAbilityX),
        until_source_leaves: false,
    }] = minus_x.effect.as_slice()
    else {
        panic!("-X is an untargeted primitive ExileAll with its announced-X ceiling");
    };
    let color_branches = kind
        .any_of
        .as_ref()
        .expect("-X uses an explicit five-color union");
    assert_eq!(
        color_branches
            .iter()
            .map(|branch| branch.is_color)
            .collect::<Vec<_>>(),
        [
            Some(Color::White),
            Some(Color::Blue),
            Some(Color::Black),
            Some(Color::Red),
            Some(Color::Green),
        ]
    );

    let minus_ten = &face.activated_abilities[2];
    assert_eq!(minus_ten.ability_id.as_str(), "activated_03");
    assert_eq!(
        minus_ten.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(minus_ten.costs, [AbilityCost::Loyalty(-10)]);
    assert!(minus_ten.program.is_none());
    assert!(minus_ten.targeting.is_none());
    assert!(matches!(
        minus_ten.effect.as_slice(),
        [
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(7),
                who: PlayerRecipient::Controller,
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(7),
            },
            SpellEffectKind::PutPermanentFromHand {
                filter: None,
                max_mana_value: None,
                up_to_count: 7,
            },
        ]
    ));
}

use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{ContinuousEffectKind, CounterKind, EffectDuration};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::{EngineError, TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChoiceKind, DevCommand, DevMoveCard, DevZone,
    ResolutionChoiceRequired, RuledCommand, RuledEventBatch, TargetRef, TargetRefKind,
};

const CARTH: &str = "carth_the_lion";
const SORIN: &str = "sorin,_grim_nemesis";
const PLUS_TWO: &str = "activated_01";
const MINUS_X: &str = "activated_02";
const ULTIMATE: &str = "activated_03";
const ULGROTHA_BATTLE: &str = "invasion_of_ulgrotha_grandmother_ravi_sengir";
const VAMPIRE_KNIGHT_TOKEN: &str = "vampire_knight_b_1_1_lifelink";

fn ugin_test_setup(players: &[i32]) -> GameEngine {
    let decks = (0..players.len())
        .map(|seat| {
            if seat == 0 {
                deck_with("forest", &[UGIN])
            } else {
                deck_with("island", &[])
            }
        })
        .collect();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        281_029,
        players,
        30,
        Some(decks),
        true,
    )
    .expect("registered Ugin scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    for _ in 0..players.len().saturating_mul(8) {
        if engine.state.turn_step == TurnStep::Main1 && engine.state.active_player_idx == 0 {
            return engine;
        }
        pass_priority_round(&mut engine);
    }
    panic!("Ugin controller did not reach a prepared main phase");
}

fn ugin_test_put(engine: &mut GameEngine, seat: usize, loyalty: u32) -> u32 {
    let source = inject_permanent_on_battlefield(engine, seat, UGIN);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("injected Ugin object")
        .set_counter(CounterKind::Loyalty, loyalty);
    engine.initial_response_batch();
    source
}

fn ugin_test_ability_index(ability_id: &str) -> u32 {
    tricerules_cards::registry::global()
        .get(UGIN)
        .expect("Ugin is registered")
        .primary_face()
        .activated_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == ability_id)
        .unwrap_or_else(|| panic!("Ugin ability {ability_id} is present")) as u32
}

fn ugin_test_permanent_target(engine: &GameEngine, object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        kind: TargetRefKind::Permanent as i32,
        expected_zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied(),
        ..Default::default()
    }
}

fn ugin_test_player_target(player_id: i32) -> TargetRef {
    TargetRef {
        object_id: player_id as u32,
        kind: TargetRefKind::Player as i32,
        ..Default::default()
    }
}

fn ugin_test_activation(
    engine: &GameEngine,
    source: u32,
    ability_id: &str,
    x_value: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    let mut command =
        activate_ability_for(engine, source, ugin_test_ability_index(ability_id), targets);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.x_value = x_value;
    command
}

fn ugin_test_activate(
    engine: &mut GameEngine,
    actor: i32,
    source: u32,
    ability_id: &str,
    x_value: u32,
    targets: Vec<TargetRef>,
) -> Result<RuledEventBatch, EngineError> {
    let command = ugin_test_activation(engine, source, ability_id, x_value, targets);
    engine.apply_scenario_command(actor, &command)
}

fn ugin_test_dev_move(player_id: i32, zone: DevZone, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player_id,
            dev: Some(tricerules_proto::ruled::v1::dev_command::Dev::MoveCard(
                DevMoveCard {
                    card_name: card_name.to_string(),
                    zone: zone as i32,
                    ready: false,
                },
            )),
        })),
    }
}

fn ugin_test_set_library_top(
    engine: &mut GameEngine,
    player: usize,
    card_ids: &[&str],
) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn ugin_test_last_zone_view(batch: &RuledEventBatch) -> &tricerules_proto::ruled::v1::ZoneViewSync {
    batch
        .events
        .iter()
        .rev()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("scenario batch contains its final per-player zone view")
}

fn ugin_test_selectable(choice: &ResolutionChoiceRequired, object_id: u32) -> bool {
    let Some(index) = choice
        .candidate_object_ids
        .iter()
        .position(|candidate| *candidate == object_id)
    else {
        return false;
    };
    choice.candidate_selectable[index]
}

fn ugin_test_submit_choice(
    engine: &mut GameEngine,
    chosen: Vec<u32>,
) -> Result<RuledEventBatch, EngineError> {
    let actor = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("resolution choice is pending")
        .deciding_player;
    engine.apply_scenario_command(actor, &submit_resolution_choice(chosen))
}

fn ugin_test_pass_to_resolution(engine: &mut GameEngine) -> Vec<RuledEventBatch> {
    let mut batches = Vec::new();
    let max_steps = engine
        .state
        .players
        .len()
        .saturating_mul(2)
        .saturating_mul(8);
    for _ in 0..max_steps {
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            let choice_kind = pending.presentation.choice_kind;
            assert_eq!(
                choice_kind,
                ChoiceKind::SimultaneousEntryOrder,
                "only the known simultaneous-entry order may interrupt ordinary Ugin resolution"
            );
            let order_answers = answer_simultaneous_entry_order_in_engine_order(engine);
            assert!(
                !order_answers.is_empty(),
                "Ugin resolution helper must answer an offered entry order"
            );
            batches.extend(order_answers.into_iter().map(|(_, _, batch)| batch));
            continue;
        }
        if engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty() {
            break;
        }
        let actor = engine.state.priority_player_id();
        batches.push(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("current priority holder passes Ugin resolution"),
        );
    }
    assert!(engine.state.stack.is_empty(), "Ugin ability resolved");
    assert!(
        engine.state.pending_resolution.is_none(),
        "Ugin resolution helper finishes the known entry-order choice"
    );
    batches
}

fn ugin_test_pass_to_choice(
    engine: &mut GameEngine,
) -> (ResolutionChoiceRequired, Vec<RuledEventBatch>) {
    let mut batches = Vec::new();
    let max_steps = engine
        .state
        .players
        .len()
        .saturating_mul(2)
        .saturating_mul(12);
    for _ in 0..max_steps {
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            if pending.presentation.choice_kind == ChoiceKind::SimultaneousEntryOrder {
                let order_answers = answer_simultaneous_entry_order_in_engine_order(engine);
                assert!(
                    !order_answers.is_empty(),
                    "Ugin scenario must answer the offered simultaneous-entry order"
                );
                batches.extend(order_answers.into_iter().map(|(_, _, batch)| batch));
                continue;
            }
            let choice = batches
                .iter()
                .rev()
                .find_map(find_resolution_choice)
                .filter(|choice| choice.choice_kind() == pending.presentation.choice_kind)
                .expect("retain the actual wire offer for an already-pending Ugin choice");
            return (choice, batches);
        }
        if engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty() {
            panic!("Ugin scenario reached quiescence without the expected choice");
        }
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("current priority holder passes to the expected Ugin choice");
        let choice = find_resolution_choice(&batch);
        batches.push(batch);
        if let Some(choice) = choice {
            if choice.choice_kind() != ChoiceKind::SimultaneousEntryOrder {
                return (choice, batches);
            }
        }
    }
    panic!(
        "Ugin scenario did not reach an expected choice; pending={:?}",
        engine.state.pending_resolution
    );
}

fn ugin_test_finish_hand_entries(
    engine: &mut GameEngine,
    cohort_creature: u32,
    existing_creature: Option<u32>,
) -> (bool, bool, usize) {
    let mut saw_aura_choice = false;
    let mut saw_copy_choice = false;
    let mut replacement_choices = 0;
    let max_steps = 48;
    for _ in 0..max_steps {
        if let Some(pending) = engine.state.pending_resolution.as_ref().cloned() {
            let deciding_player = pending.deciding_player;
            match pending.presentation.choice_kind {
                ChoiceKind::SimultaneousEntryOrder => {
                    let order_answers = answer_simultaneous_entry_order_in_engine_order(engine);
                    assert!(
                        !order_answers.is_empty(),
                        "answer the actual engine-offered simultaneous-entry order"
                    );
                }
                ChoiceKind::AuraPermanent | ChoiceKind::CopySource => {
                    assert!(
                        !pending.presentation.candidates.contains(&cohort_creature),
                        "a same-cohort creature is not a preexisting Aura or Clone recipient"
                    );
                    let recipient = existing_creature
                        .expect("these Ugin cases prepared an established creature recipient");
                    assert!(
                        pending.presentation.candidates.contains(&recipient),
                        "an established creature remains selectable"
                    );
                    saw_aura_choice |=
                        pending.presentation.choice_kind == ChoiceKind::AuraPermanent;
                    saw_copy_choice |= pending.presentation.choice_kind == ChoiceKind::CopySource;
                    engine
                        .apply_scenario_command(
                            deciding_player,
                            &submit_resolution_choice(vec![recipient]),
                        )
                        .expect("choose the established Aura or Clone recipient");
                }
                ChoiceKind::ReplacementEffect => {
                    let chosen_application = *pending
                        .presentation
                        .candidates
                        .first()
                        .expect("the shared entry-counter planner offers an application");
                    replacement_choices += 1;
                    engine
                        .apply_scenario_command(
                            deciding_player,
                            &submit_resolution_choice(vec![chosen_application]),
                        )
                        .expect("apply the offered shared entry-counter replacement");
                }
                other => panic!("unknown Ugin hand-entry prompt: {other:?}"),
            }
            continue;
        }
        if engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty() {
            break;
        }
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("pass current priority while Ugin's hand-entry cohort completes");
    }
    assert!(engine.state.stack.is_empty(), "Ugin entry cohort finished");
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    (saw_aura_choice, saw_copy_choice, replacement_choices)
}

#[test]
fn ugin_test_plus_two_uses_engine_any_target_for_player_creature_planeswalker_and_battle() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 7);
    let player_life = engine.state.players[1].life;
    ugin_test_activate(
        &mut engine,
        10,
        source,
        PLUS_TWO,
        0,
        vec![ugin_test_player_target(20)],
    )
    .expect("+2 can target an opponent player");
    ugin_test_pass_to_resolution(&mut engine);
    assert_eq!(engine.state.players[1].life, player_life - 3);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        9
    );

    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 7);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let creature_target = ugin_test_permanent_target(&engine, creature);
    ugin_test_activate(&mut engine, 10, source, PLUS_TWO, 0, vec![creature_target])
        .expect("+2 can target a creature permanent");
    ugin_test_pass_to_resolution(&mut engine);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);

    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 7);
    let planeswalker = inject_permanent_on_battlefield(&mut engine, 1, UGIN);
    let planeswalker_target = ugin_test_permanent_target(&engine, planeswalker);
    engine
        .state
        .objects
        .get_mut(&planeswalker)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    engine.initial_response_batch();
    ugin_test_activate(
        &mut engine,
        10,
        source,
        PLUS_TWO,
        0,
        vec![planeswalker_target],
    )
    .expect("+2 can target a planeswalker permanent");
    ugin_test_pass_to_resolution(&mut engine);
    assert_eq!(
        engine.state.objects[&planeswalker].counter_count(CounterKind::Loyalty),
        2
    );

    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 7);
    let battle = inject_permanent_on_battlefield(&mut engine, 1, ULGROTHA_BATTLE);
    let battle_target = ugin_test_permanent_target(&engine, battle);
    engine
        .state
        .objects
        .get_mut(&battle)
        .unwrap()
        .set_counter(CounterKind::Defense, 5);
    engine.initial_response_batch();
    ugin_test_activate(&mut engine, 10, source, PLUS_TWO, 0, vec![battle_target])
        .expect("+2 can target the admitted Invasion of Ulgrotha Battle");
    ugin_test_pass_to_resolution(&mut engine);
    assert_eq!(engine.state.objects[&battle].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&battle].counter_count(CounterKind::Defense),
        2
    );
}

#[test]
fn ugin_test_plus_two_rejects_a_land_and_fizzles_when_its_only_target_leaves() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 7);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let land_target = ugin_test_permanent_target(&engine, land);
    let stack_before = engine.state.stack.len();
    assert!(ugin_test_activate(&mut engine, 10, source, PLUS_TWO, 0, vec![land_target],).is_err());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        7
    );
    assert_eq!(engine.state.stack.len(), stack_before);

    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let creature_target = ugin_test_permanent_target(&engine, creature);
    let life_before = engine.state.players[1].life;
    ugin_test_activate(&mut engine, 10, source, PLUS_TWO, 0, vec![creature_target])
        .expect("+2 is legal while its creature target is on the battlefield");
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            10,
            &ugin_test_dev_move(20, DevZone::Graveyard, "Grizzly Bears"),
        )
        .expect("move the sole target before resolution");
    ugin_test_pass_to_resolution(&mut engine);
    assert_eq!(engine.state.players[1].life, life_before);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        9
    );
}

#[test]
fn ugin_test_minus_x_exiles_all_five_colors_including_multicolor_once_at_the_limit() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 10);
    let white = inject_permanent_on_battlefield(&mut engine, 1, "white_knight");
    let blue = inject_permanent_on_battlefield(&mut engine, 1, "prodigal_sorcerer");
    let black = inject_permanent_on_battlefield(&mut engine, 1, "black_knight");
    let red = inject_permanent_on_battlefield(&mut engine, 1, "goblin_arsonist");
    let green = inject_permanent_on_battlefield(&mut engine, 1, "grizzly_bears");
    let multicolor = inject_permanent_on_battlefield(&mut engine, 1, CARTH);
    let above_limit = inject_permanent_on_battlefield(&mut engine, 1, "doubling_season");
    let colorless_land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let colorless_zero = inject_permanent_on_battlefield(&mut engine, 1, "ornithopter");
    let multicolor_generation = engine
        .state
        .zone_change_generation
        .get(&multicolor)
        .copied()
        .unwrap_or(0);

    // Carth's {2}{B}{G} cost is exactly four. Doubling Season is green but has mana value five.
    // The -X ability has no targets and checks these registered objects when it resolves.
    ugin_test_activate(&mut engine, 10, source, MINUS_X, 4, vec![])
        .expect("-X is targetless and accepts its announced X");
    ugin_test_pass_to_resolution(&mut engine);

    for object_id in [white, blue, black, red, green, multicolor] {
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Exile);
    }
    assert_eq!(
        engine.state.zone_change_generation[&multicolor],
        multicolor_generation + 1
    );
    assert_eq!(engine.state.objects[&above_limit].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&colorless_land].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&colorless_zero].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6
    );
}

#[test]
fn ugin_test_minus_x_uses_live_colors_when_the_ability_resolves() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 10);
    let becomes_colorless = inject_permanent_on_battlefield(&mut engine, 1, "white_knight");
    let becomes_green = inject_permanent_on_battlefield(&mut engine, 1, "forest");

    ugin_test_activate(&mut engine, 10, source, MINUS_X, 2, vec![])
        .expect("activate targetless -X before the intervening color change");
    // Fixture an intervening layer-five color change while the actual Ugin ability is on the
    // stack. This changes only the targets' prepared live characteristics; Ugin's effect still
    // resolves through the registered ability and ordinary priority commands below.
    for (object_id, colors) in [
        (becomes_colorless, Vec::new()),
        (becomes_green, vec![Color::Green]),
    ] {
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(object_id),
            kind: ContinuousEffectKind::Layer5SetColors(colors),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
    }
    assert!(engine
        .characteristics(becomes_colorless)
        .unwrap()
        .colors
        .is_empty());
    assert_eq!(
        engine.characteristics(becomes_green).unwrap().colors,
        [Color::Green]
    );

    ugin_test_pass_to_resolution(&mut engine);
    assert_eq!(
        engine.state.objects[&becomes_colorless].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&becomes_green].zone, Zone::Exile);
}

#[test]
fn ugin_test_minus_x_zero_exiles_a_colored_zero_mana_token_but_keeps_colorless_ornithopter() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 7);
    let colored_zero = inject_permanent_on_battlefield(&mut engine, 1, VAMPIRE_KNIGHT_TOKEN);
    let colorless_zero = inject_permanent_on_battlefield(&mut engine, 1, "ornithopter");
    assert!(engine.state.objects[&colored_zero].is_token());

    ugin_test_activate(&mut engine, 10, source, MINUS_X, 0, vec![])
        .expect("X=0 is a legal targetless loyalty activation");
    let batches = ugin_test_pass_to_resolution(&mut engine);

    assert_eq!(
        batches.iter().flat_map(|batch| &batch.events).filter(|event| matches!(
            event.ev.as_ref(),
            Some(Ev::PermanentMoved(moved))
                if moved.object_id == colored_zero
                    && moved.destination() == tricerules_proto::ruled::v1::permanent_moved::Destination::Exile
        )).count(),
        1,
        "the colored zero-mana token is exiled exactly once before it ceases to exist"
    );
    assert!(!engine.state.objects.contains_key(&colored_zero));
    assert_eq!(
        engine.state.objects[&colorless_zero].zone,
        Zone::Battlefield
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        7
    );
}

#[test]
fn ugin_test_minus_x_keeps_announced_x_after_its_source_leaves() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 7);
    let target = inject_permanent_on_battlefield(&mut engine, 1, "doubling_season");

    ugin_test_activate(&mut engine, 10, source, MINUS_X, 7, vec![])
        .expect("pay all seven loyalty counters for X=7");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    let activation = engine
        .state
        .stack
        .iter()
        .find(|item| !item.is_triggered && item.source_permanent_id == Some(source))
        .expect("Ugin's -X remains on the stack after source-based actions");
    assert_eq!(activation.chosen_x, 7);

    ugin_test_pass_to_resolution(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn ugin_test_ultimate_draws_seven_before_private_zero_to_seven_hand_choice() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 10);
    let permanent_in_hand = inject_card_into_hand(&mut engine, 0, "pacifism");
    let nonpermanent_in_hand = inject_card_into_hand(&mut engine, 0, "counterspell");
    let initial_hand_count = engine.state.players[0].hand.len();
    let draws_before = engine.state.turn_history.current.player(10).cards_drawn;
    let library_forest = engine.state.players[0]
        .library
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == "forest")
        .expect("prepared deck has a Forest in its library");

    ugin_test_activate(&mut engine, 10, source, ULTIMATE, 0, vec![])
        .expect("pay ten loyalty for Ugin's ultimate");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    let (choice, batches) = ugin_test_pass_to_choice(&mut engine);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (0, 7));
    assert_eq!(choice.deciding_player_id, 10);
    assert!(ugin_test_selectable(&choice, permanent_in_hand));
    assert!(!ugin_test_selectable(&choice, nonpermanent_in_hand));
    assert!(
        engine.state.players[0].hand.contains(&library_forest),
        "Draw 7 resolves before the hand selection is offered"
    );
    assert!(ugin_test_selectable(&choice, library_forest));
    assert_eq!(engine.state.players[0].hand.len(), initial_hand_count + 7);
    assert_eq!(
        engine.state.turn_history.current.player(10).cards_drawn,
        draws_before + 7
    );
    assert_eq!(engine.state.players[0].life, 37);

    let last_batch = batches
        .last()
        .expect("the hand-choice offer has an event batch");
    let view = ugin_test_last_zone_view(last_batch);
    let controller_view = view
        .per_player
        .iter()
        .find(|player_view| player_view.player_id == 10)
        .expect("controller's private hand view");
    assert!(controller_view
        .hand_cards
        .iter()
        .any(|card| card.object_id == permanent_in_hand));
    assert!(controller_view
        .hand_cards
        .iter()
        .any(|card| card.object_id == nonpermanent_in_hand));
    let opponent_view = view
        .per_player
        .iter()
        .find(|player_view| player_view.player_id == 20)
        .expect("opponent's private hand view");
    assert!(opponent_view
        .hand_cards
        .iter()
        .all(|card| card.object_id != permanent_in_hand && card.object_id != nonpermanent_in_hand));

    ugin_test_submit_choice(&mut engine, vec![]).expect("zero is a legal optional selection");
    assert_eq!(engine.state.objects[&permanent_in_hand].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn ugin_test_ultimate_rejects_eight_and_puts_exactly_seven_selected_permanents() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 10);
    let cards = (0..8)
        .map(|_| inject_card_into_hand(&mut engine, 0, "grizzly_bears"))
        .collect::<Vec<_>>();
    let instant = inject_card_into_hand(&mut engine, 0, "counterspell");
    let initial_hand_count = engine.state.players[0].hand.len();
    let draws_before = engine.state.turn_history.current.player(10).cards_drawn;

    ugin_test_activate(&mut engine, 10, source, ULTIMATE, 0, vec![])
        .expect("activate -10 with ten loyalty");
    let (choice, _) = ugin_test_pass_to_choice(&mut engine);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (0, 7));
    assert!(cards
        .iter()
        .all(|object_id| ugin_test_selectable(&choice, *object_id)));
    assert!(!ugin_test_selectable(&choice, instant));

    assert!(ugin_test_submit_choice(&mut engine, cards.clone()).is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert!(cards
        .iter()
        .all(|object_id| engine.state.players[0].hand.contains(object_id)));
    ugin_test_submit_choice(&mut engine, cards[..7].to_vec())
        .expect("seven is the printed maximum and the choice is all-or-none");
    ugin_test_finish_hand_entries(&mut engine, cards[0], None);

    assert!(cards[..7]
        .iter()
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Battlefield));
    assert_eq!(engine.state.objects[&cards[7]].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&instant].zone, Zone::Hand);
    assert_eq!(
        engine.state.players[0].hand.len(),
        initial_hand_count + 7 - 7
    );
    assert_eq!(
        engine.state.turn_history.current.player(10).cards_drawn,
        draws_before + 7
    );
    assert_eq!(engine.state.players[0].life, 37);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn ugin_test_ultimate_hand_cohort_uses_shared_entry_counters_aura_and_clone_choices() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 10);
    let _laezel = inject_permanent_on_battlefield(&mut engine, 0, "laezel,_vlaakiths_champion");
    let _season = inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let established = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let counter_entrant = inject_card_into_hand(&mut engine, 0, "dockworker_drone");
    let coentrant = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let aura = inject_card_into_hand(&mut engine, 0, "pacifism");
    let clone = inject_card_into_hand(&mut engine, 0, "clone");
    let counter_generation = engine
        .state
        .zone_change_generation
        .get(&counter_entrant)
        .copied()
        .unwrap_or(0);
    let coentrant_generation = engine
        .state
        .zone_change_generation
        .get(&coentrant)
        .copied()
        .unwrap_or(0);

    ugin_test_activate(&mut engine, 10, source, ULTIMATE, 0, vec![])
        .expect("activate Ugin's registered -10 ability");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    let (choice, _) = ugin_test_pass_to_choice(&mut engine);
    for object_id in [counter_entrant, coentrant, aura, clone] {
        assert!(ugin_test_selectable(&choice, object_id));
    }
    ugin_test_submit_choice(&mut engine, vec![counter_entrant, coentrant, aura, clone])
        .expect("select one actual Ugin hand cohort");
    let (saw_aura, saw_clone, replacement_choices) =
        ugin_test_finish_hand_entries(&mut engine, coentrant, Some(established));

    assert!(saw_aura, "Ugin's Aura recipient offer was exercised");
    assert!(saw_clone, "Ugin's Clone copy-source offer was exercised");
    assert!(
        replacement_choices > 0,
        "Lae'zel and Doubling Season use the shared counter planner"
    );
    assert_eq!(
        engine.state.objects[&counter_entrant].zone,
        Zone::Battlefield
    );
    // The affected player chooses replacement order: (1 + 1) * 2 = 4,
    // or (1 * 2) + 1 = 3. Either engine-offered order must apply both effects.
    assert!([3, 4].contains(
        &engine.state.objects[&counter_entrant].counter_count(CounterKind::PlusOnePlusOne)
    ));
    assert_eq!(
        engine.state.zone_change_generation[&counter_entrant],
        counter_generation + 1
    );
    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&coentrant],
        coentrant_generation + 1
    );
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::state::AttachmentRecipient::Object(
            established
        ))
    );
    assert_eq!(engine.state.objects[&clone].zone, Zone::Battlefield);
    assert!(engine.state.objects[&clone].copiable_values.is_some());
}

#[test]
fn ugin_test_ultimate_aura_and_clone_cannot_use_a_same_cohort_creature() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 10);
    let coentrant = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let clone = inject_card_into_hand(&mut engine, 0, "clone");
    let aura = inject_card_into_hand(&mut engine, 0, "pacifism");

    ugin_test_activate(&mut engine, 10, source, ULTIMATE, 0, vec![]).expect("activate Ugin -10");
    let (choice, _) = ugin_test_pass_to_choice(&mut engine);
    assert!(ugin_test_selectable(&choice, coentrant));
    assert!(ugin_test_selectable(&choice, clone));
    assert!(ugin_test_selectable(&choice, aura));
    let selection = ugin_test_submit_choice(&mut engine, vec![coentrant, clone, aura])
        .expect("choose the creature and Clone as one hand cohort");
    assert!(!selection.events.iter().any(|event| matches!(
        event.ev.as_ref(),
        Some(Ev::ResolutionChoiceRequired(required))
            if matches!(required.choice_kind(), ChoiceKind::CopySource | ChoiceKind::AuraPermanent)
                && required.candidate_object_ids.contains(&coentrant)
    )));
    ugin_test_finish_hand_entries(&mut engine, coentrant, None);

    assert_eq!(engine.state.objects[&coentrant].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&clone].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Hand);
    assert!(engine.state.objects[&clone].copiable_values.is_none());
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn ugin_test_ultimate_affordability_is_ten_or_nine_with_carth_and_keeps_carth_callback_visible() {
    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 9);
    let stack_before = engine.state.stack.len();
    assert!(ugin_test_activate(&mut engine, 10, source, ULTIMATE, 0, vec![]).is_err());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        9
    );
    assert_eq!(engine.state.stack.len(), stack_before);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 10);
    engine.initial_response_batch();
    ugin_test_activate(&mut engine, 10, source, ULTIMATE, 0, vec![])
        .expect("unmodified Ugin can pay ten loyalty");

    let mut engine = ugin_test_setup(&[10, 20]);
    let source = ugin_test_put(&mut engine, 0, 8);
    let carth = inject_permanent_on_battlefield(&mut engine, 0, CARTH);
    let stack_before = engine.state.stack.len();
    assert!(ugin_test_activate(&mut engine, 10, source, ULTIMATE, 0, vec![]).is_err());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        8
    );
    assert_eq!(engine.state.stack.len(), stack_before);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 9);
    engine.initial_response_batch();
    let sorin_in_library =
        ugin_test_set_library_top(&mut engine, 0, &[SORIN, "forest", "forest"])[0];

    ugin_test_activate(&mut engine, 10, source, ULTIMATE, 0, vec![])
        .expect("Carth makes nine loyalty payable but eight was rejected");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        stack_before + 2,
        "Ugin's ultimate remains below Carth's planeswalker-death trigger"
    );
    assert!(engine
        .state
        .stack
        .last()
        .is_some_and(|item| { item.is_triggered && item.source_permanent_id == Some(carth) }));
    let ability = engine
        .state
        .stack
        .iter()
        .find(|item| !item.is_triggered && item.source_permanent_id == Some(source))
        .expect("the actual Ugin ultimate remains on the stack");
    assert_eq!(ability.ability_index, Some(2));

    let (carth_choice, _) = ugin_test_pass_to_choice(&mut engine);
    assert_eq!((carth_choice.min, carth_choice.max), (0, 1));
    assert!(ugin_test_selectable(&carth_choice, sorin_in_library));
    ugin_test_submit_choice(&mut engine, vec![])
        .expect("explicitly decline Carth's known optional top-three planeswalker choice");

    let (ugin_choice, _) = ugin_test_pass_to_choice(&mut engine);
    assert_eq!(ugin_choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((ugin_choice.min, ugin_choice.max), (0, 7));
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    ugin_test_submit_choice(&mut engine, vec![]).expect("decline Ugin's optional hand cohort");
    ugin_test_pass_to_resolution(&mut engine);
    assert!(engine.state.stack.is_empty());
}
