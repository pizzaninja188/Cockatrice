//! Actual-card coverage for Sorin, Grim Nemesis.

use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{
    AbilityCost, Amount, Color, CountExpression, CounterKind, InstructionKind, Keyword, LifeAmount,
    PermanentTypeFilter, PlayerLifeAggregate, PlayerRecipient, RelativePlayerSet, SpellEffectKind,
    TargetKind,
};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ruled_event::Ev, ChoiceKind, DevCommand, DevMoveCard, DevZone,
    RuledEventBatch, TargetRef, TargetRefKind,
};

const SORIN: &str = "sorin,_grim_nemesis";
const VAMPIRE_KNIGHT: &str = "vampire_knight_b_1_1_lifelink";

fn setup(players: &[i32]) -> GameEngine {
    let decks = (0..players.len())
        .map(|seat| {
            if seat == 0 {
                deck_with("swamp", &[SORIN])
            } else {
                deck_with("forest", &[])
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
    .expect("registered Sorin scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    for _ in 0..players.len().saturating_mul(8) {
        if engine.state.turn_step == TurnStep::Main1 && engine.state.active_player_idx == 0 {
            return engine;
        }
        pass_priority_round(&mut engine);
    }
    panic!("Sorin controller did not reach a prepared main phase");
}

fn put_sorin(engine: &mut GameEngine, seat: usize, loyalty: u32) -> u32 {
    let source = inject_permanent_on_battlefield(engine, seat, SORIN);
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("injected Sorin object")
        .set_counter(CounterKind::Loyalty, loyalty);
    engine.initial_response_batch();
    source
}

fn ability_index(ability_id: &str) -> u32 {
    tricerules_cards::registry::global()
        .get(SORIN)
        .expect("Sorin is registered")
        .primary_face()
        .activated_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == ability_id)
        .unwrap_or_else(|| panic!("Sorin ability {ability_id} is present")) as u32
}

fn permanent_target(engine: &GameEngine, object_id: u32) -> TargetRef {
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

fn player_target(player_id: i32) -> TargetRef {
    TargetRef {
        object_id: player_id as u32,
        kind: TargetRefKind::Player as i32,
        ..Default::default()
    }
}

fn sorin_activation(
    engine: &GameEngine,
    source: u32,
    ability_id: &str,
    x_value: u32,
    targets: Vec<TargetRef>,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(engine, source, ability_index(ability_id), targets);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.x_value = x_value;
    command
}

fn activate_sorin(
    engine: &mut GameEngine,
    actor: i32,
    source: u32,
    ability_id: &str,
    x_value: u32,
    targets: Vec<TargetRef>,
) -> Result<RuledEventBatch, tricerules_core::EngineError> {
    let command = sorin_activation(engine, source, ability_id, x_value, targets);
    engine.apply_scenario_command(actor, &command)
}

fn set_library_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
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

fn pass_to_resolution(engine: &mut GameEngine) -> Vec<RuledEventBatch> {
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
            let order_answers = answer_simultaneous_entry_order_in_engine_order(engine);
            assert!(
                !order_answers.is_empty(),
                "Sorin resolution helper cannot answer pending choice {choice_kind:?}"
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
                .expect("current priority holder passes to resolve Sorin"),
        );
    }
    assert!(engine.state.stack.is_empty(), "Sorin ability resolved");
    assert!(
        engine.state.pending_resolution.is_none(),
        "Sorin resolution helper resolves the token entry-order choice"
    );
    batches
}

fn pass_to_sorin_token_entry_order(engine: &mut GameEngine) -> Vec<RuledEventBatch> {
    let mut batches = Vec::new();
    for _ in 0..engine.state.players.len().saturating_mul(2) {
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            assert_eq!(
                pending.presentation.choice_kind,
                ChoiceKind::SimultaneousEntryOrder,
                "the prepared Sorin -9 case parks on its token entry-order offer"
            );
            return batches;
        }
        if engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty() {
            break;
        }
        let actor = engine.state.priority_player_id();
        batches.push(
            engine
                .apply_scenario_command(actor, &pass())
                .expect("current priority holder passes Sorin -9 to its token entry-order offer"),
        );
    }
    panic!(
        "Sorin -9 did not park on token entry order; pending choice: {:?}",
        engine.state.pending_resolution
    );
}

fn battlefield_token_ids(engine: &GameEngine) -> Vec<u32> {
    engine
        .state
        .objects
        .iter()
        .filter_map(|(object_id, object)| {
            (object.is_token() && object.zone == Zone::Battlefield).then_some(*object_id)
        })
        .collect()
}

fn last_zone_view(batch: &RuledEventBatch) -> &tricerules_proto::ruled::v1::ZoneViewSync {
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

fn dev_move(
    player_id: i32,
    zone: DevZone,
    card_name: &str,
) -> tricerules_proto::ruled::v1::RuledCommand {
    tricerules_proto::ruled::v1::RuledCommand {
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

#[test]
fn sorin_grim_nemesis_has_exact_identity_and_three_abilities() {
    let card = tricerules_cards::registry::global()
        .get(SORIN)
        .expect("Sorin, Grim Nemesis must have a complete ruled definition");
    assert_eq!(card.name, "Sorin, Grim Nemesis");
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{W}{B}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Sorin"]);
    assert_eq!(face.loyalty, Some(6));
    assert_eq!(face.activated_abilities.len(), 3);

    let plus_one = &face.activated_abilities[0];
    assert_eq!(plus_one.ability_id.as_str(), "activated_01");
    assert_eq!(plus_one.costs, [AbilityCost::Loyalty(1)]);
    assert!(plus_one.effect.is_empty());
    let plus_program = plus_one
        .program
        .as_ref()
        .expect("+1 uses its typed receipt program");
    assert_eq!(plus_program.body.steps.len(), 2);
    assert_eq!(plus_program.body.steps[0].bind, ["revealed_card"]);
    assert!(matches!(
        &plus_program.body.steps[0].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::RevealTopCardToHand)
    ));
    assert_eq!(
        plus_program.body.steps[1].input.as_deref(),
        Some("revealed_card")
    );
    assert!(matches!(
        &plus_program.body.steps[1].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::LoseLife {
                amount: LifeAmount::RevealedCardManaValue,
                who: PlayerRecipient::EachOpponent,
            })
    ));

    let minus_x = &face.activated_abilities[1];
    assert_eq!(minus_x.ability_id.as_str(), "activated_02");
    assert_eq!(minus_x.costs, [AbilityCost::LoyaltyX]);
    assert!(
        minus_x.program.is_none(),
        "-X uses the reviewed primitive effect-list tier"
    );
    assert!(matches!(
        minus_x.effect.as_slice(),
        [
            SpellEffectKind::DamageTarget { amount: Amount::X, target },
            SpellEffectKind::GainLife { amount: Amount::X, who: PlayerRecipient::Controller },
        ] if target.kind == TargetKind::AnyPermanent
            && target.permanent_types == [PermanentTypeFilter::Creature, PermanentTypeFilter::Planeswalker]
    ));
    let target_group = &minus_x
        .targeting
        .as_ref()
        .expect("-X has one target group")
        .groups[0];
    assert_eq!(target_group.min, 1);
    assert_eq!(target_group.max, 1);
    assert_eq!(target_group.effect_indices, [0]);

    let minus_nine = &face.activated_abilities[2];
    assert_eq!(minus_nine.ability_id.as_str(), "activated_03");
    assert_eq!(minus_nine.costs, [AbilityCost::Loyalty(-9)]);
    assert!(minus_nine.effect.is_empty());
    let ultimate_program = minus_nine
        .program
        .as_ref()
        .expect("-9 uses its resolving life-count program");
    assert_eq!(ultimate_program.body.steps.len(), 1);
    assert!(matches!(
        &ultimate_program.body.steps[0].kind,
        InstructionKind::Perform(effect)
            if matches!(effect.as_ref(), SpellEffectKind::CreateTokens {
                token,
                count: Amount::Count(CountExpression::PlayerLifeAggregate {
                    players: RelativePlayerSet::All,
                    aggregate: PlayerLifeAggregate::Maximum,
                }),
                who: PlayerRecipient::Controller,
                ..
            } if token == VAMPIRE_KNIGHT)
    ));

    let token = tricerules_cards::registry::global()
        .get(VAMPIRE_KNIGHT)
        .expect("Sorin's exact token definition is registered");
    let token_face = token.primary_face();
    assert_eq!(token_face.name, "Vampire Knight");
    assert_eq!(token_face.types, ["Creature", "Vampire", "Knight"]);
    assert_eq!(token_face.colors(), [Color::Black]);
    assert_eq!(token_face.power, Some(1));
    assert_eq!(token_face.toughness, Some(1));
    assert_eq!(token_face.keywords, [Keyword::Lifelink]);
}

#[test]
fn sorin_plus_one_reveals_the_same_card_then_each_opponent_loses_its_mana_value() {
    let mut engine = setup(&[10, 20, 30]);
    let source = put_sorin(&mut engine, 0, 6);
    let top = set_library_top(&mut engine, 0, &["grizzly_bears"])[0];
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&top)
        .copied()
        .unwrap_or(0);

    let before = engine.initial_response_batch();
    let before_view = last_zone_view(&before);
    let controller_view = before_view
        .per_player
        .iter()
        .find(|view| view.player_id == 10)
        .expect("controller has a private-zone view");
    assert!(
        controller_view
            .library_cards
            .iter()
            .any(|card| card.card_id == "grizzly_bears"),
        "controller sees the arranged top card in their private library view"
    );
    for opponent_id in [20, 30] {
        let opponent_view = before_view
            .per_player
            .iter()
            .find(|view| view.player_id == opponent_id)
            .expect("opponent has a private-zone view");
        assert!(
            opponent_view
                .library_cards
                .iter()
                .all(|card| card.card_id != "grizzly_bears"),
            "the library top is not exposed in another player's private view"
        );
    }

    let life_before = engine
        .state
        .players
        .iter()
        .map(|player| player.life)
        .collect::<Vec<_>>();
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;
    activate_sorin(&mut engine, 10, source, "activated_01", 0, vec![])
        .expect("activate Sorin's targetless +1");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        7
    );
    let batches = pass_to_resolution(&mut engine);

    let reveal = batches
        .iter()
        .flat_map(|batch| &batch.events)
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .expect("+1 publicly reveals its top card before the move to hand");
    assert_eq!(reveal.zone_owner_player_id, 10);
    assert_eq!(reveal.cards.len(), 1);
    assert_eq!(reveal.cards[0].object_id, top);
    assert_eq!(reveal.cards[0].zone_change_generation, original_generation);
    assert_eq!(reveal.cards[0].card_id, "grizzly_bears");
    assert_eq!(engine.state.objects[&top].zone, Zone::Hand);
    assert_eq!(
        engine.state.zone_change_generation[&top],
        original_generation + 1
    );
    assert!(engine.state.players[0].hand.contains(&top));
    assert!(!engine.state.players[0].library.contains(&top));
    assert_eq!(engine.state.players[0].life, life_before[0]);
    assert_eq!(engine.state.players[1].life, life_before[1] - 2);
    assert_eq!(engine.state.players[2].life, life_before[2] - 2);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before,
        "putting the revealed card into hand is not a draw"
    );

    let final_batch = batches.last().expect("resolution produced a final batch");
    let final_view = last_zone_view(final_batch);
    let controller_hand = final_view
        .per_player
        .iter()
        .find(|view| view.player_id == 10)
        .expect("controller has the updated private hand view");
    assert!(controller_hand
        .hand_cards
        .iter()
        .any(|card| card.object_id == top));
    for opponent_id in [20, 30] {
        let opponent_hand = final_view
            .per_player
            .iter()
            .find(|view| view.player_id == opponent_id)
            .expect("opponent has its own private hand view");
        assert!(
            opponent_hand
                .hand_cards
                .iter()
                .all(|card| card.object_id != top),
            "the moved card identity is present only in its owner's hand view"
        );
    }
}

#[test]
fn sorin_plus_one_empty_library_contributes_zero_without_a_draw() {
    let mut engine = setup(&[10, 20, 30]);
    let source = put_sorin(&mut engine, 0, 6);
    engine.state.players[0].library.clear();
    let life_before = engine
        .state
        .players
        .iter()
        .map(|player| player.life)
        .collect::<Vec<_>>();
    let draws_before = engine.state.turn_history.current.player(0).cards_drawn;

    activate_sorin(&mut engine, 10, source, "activated_01", 0, vec![])
        .expect("empty-library +1 is still a legal activation");
    let batches = pass_to_resolution(&mut engine);
    assert!(batches
        .iter()
        .flat_map(|batch| &batch.events)
        .all(|event| { !matches!(event.ev, Some(Ev::CardsRevealed(_))) }));
    assert_eq!(engine.state.players[0].life, life_before[0]);
    assert_eq!(engine.state.players[1].life, life_before[1]);
    assert_eq!(engine.state.players[2].life, life_before[2]);
    assert_eq!(
        engine.state.turn_history.current.player(0).cards_drawn,
        draws_before
    );
    assert!(engine.state.players[0].library.is_empty());
}

#[test]
fn sorin_plus_one_uses_library_mana_value_with_x_rules() {
    for (card_id, expected_mana_value) in [("endless_one", 0), ("blaze", 1)] {
        let mut engine = setup(&[10, 20, 30]);
        let source = put_sorin(&mut engine, 0, 6);
        let top = set_library_top(&mut engine, 0, &[card_id])[0];
        let before = engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>();
        let definition = tricerules_cards::registry::global()
            .get(card_id)
            .expect("the existing X-cost sample is admitted");
        assert_eq!(
            definition.primary_face().mana_cost.mana_value(),
            expected_mana_value
        );

        activate_sorin(&mut engine, 10, source, "activated_01", 0, vec![])
            .expect("activate +1 with a library card containing X");
        let batches = pass_to_resolution(&mut engine);
        let reveal = batches
            .iter()
            .flat_map(|batch| &batch.events)
            .find_map(|event| match event.ev.as_ref() {
                Some(Ev::CardsRevealed(reveal)) => reveal.cards.first(),
                _ => None,
            })
            .expect("the X-cost card is publicly revealed");
        assert_eq!(reveal.object_id, top);
        assert_eq!(reveal.card_id, card_id);
        assert_eq!(
            engine.state.players[1].life,
            before[1] - expected_mana_value as i32
        );
        assert_eq!(
            engine.state.players[2].life,
            before[2] - expected_mana_value as i32
        );
    }
}

#[test]
fn sorin_minus_x_zero_consumes_the_loyalty_use_and_stale_or_player_targets_reject() {
    let mut engine = setup(&[10, 20, 30]);
    let source = put_sorin(&mut engine, 0, 6);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let target_ref = permanent_target(&engine, target);
    let loyalty_before = engine.state.objects[&source].counter_count(CounterKind::Loyalty);
    let stack_before = engine.state.stack.len();

    let mut stale = sorin_activation(&engine, source, "activated_02", 1, vec![target_ref]);
    let Some(Cmd::ActivateAbility(ability)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    ability.expected_zone_change_generation = u64::MAX;
    assert!(engine.apply_scenario_command(10, &stale).is_err());
    assert!(activate_sorin(
        &mut engine,
        10,
        source,
        "activated_02",
        1,
        vec![player_target(20)]
    )
    .is_err());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        loyalty_before
    );
    assert_eq!(engine.state.stack.len(), stack_before);

    let life_before = engine.state.players[0].life;
    activate_sorin(&mut engine, 10, source, "activated_02", 0, vec![target_ref])
        .expect("X=0 remains a real, legal targeted activation");
    let batches = pass_to_resolution(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(engine.state.objects[&target].damage, 0);
    assert_eq!(engine.state.players[0].life, life_before);
    assert!(activate_sorin(&mut engine, 10, source, "activated_01", 0, vec![]).is_err());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(engine.state.stack.len(), 0);
    assert!(!batches.is_empty());
}

#[test]
fn sorin_minus_x_with_carth_keeps_effect_x_after_source_sba() {
    let mut engine = setup(&[10, 20]);
    let source = put_sorin(&mut engine, 0, 6);
    let carth = inject_permanent_on_battlefield(&mut engine, 0, "carth_the_lion");
    let target = put_sorin(&mut engine, 1, 15);
    let target_ref = permanent_target(&engine, target);
    let life_before = engine.state.players[0].life;
    let stack_before = engine.state.stack.len();
    assert!(
        engine.state.stack.is_empty(),
        "no stack item precedes the activation"
    );

    assert!(activate_sorin(&mut engine, 10, source, "activated_02", 8, vec![target_ref]).is_err());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(engine.state.stack.len(), stack_before);

    activate_sorin(&mut engine, 10, source, "activated_02", 7, vec![target_ref])
        .expect("Carth makes X=7 pay a final six loyalty counters");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        0
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.stack.len(),
        stack_before + 2,
        "Sorin's activation remains below Carth's trigger for its death"
    );
    let ability = engine
        .state
        .stack
        .iter()
        .find(|item| {
            !item.is_triggered
                && item.source_permanent_id == Some(source)
                && item.ability_index == Some(1)
        })
        .expect("Sorin's -X activation remains on the stack");
    assert_eq!(
        ability.chosen_x, 7,
        "the activation retains its announced X"
    );
    assert!(
        engine.state.stack.last().is_some_and(|item| {
            item.is_triggered
                && item.source_permanent_id == Some(carth)
                && item.ability_index == Some(1)
        }),
        "Carth's planeswalker-death trigger is on top"
    );
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Loyalty),
        15
    );

    let carth_look = pass_both_players_with_batch(&mut engine);
    assert!(find_resolution_choice(&carth_look).is_some());
    engine
        .apply_scenario_command(10, &submit_resolution_choice(vec![]))
        .expect("decline Carth's optional planeswalker selection");
    pass_to_resolution(&mut engine);
    assert_eq!(
        engine.state.objects[&target].counter_count(CounterKind::Loyalty),
        8
    );
    assert_eq!(engine.state.players[0].life, life_before + 7);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn sorin_minus_x_fizzles_when_its_sole_target_becomes_illegal() {
    let mut engine = setup(&[10, 20]);
    let source = put_sorin(&mut engine, 0, 6);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let life_before = engine.state.players[0].life;
    let target_ref = permanent_target(&engine, target);
    activate_sorin(&mut engine, 10, source, "activated_02", 2, vec![target_ref])
        .expect("activate -X with a legal creature target");
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        4
    );

    engine.enable_dev_commands();
    engine
        .apply_scenario_command(10, &dev_move(20, DevZone::Graveyard, "Grizzly Bears"))
        .expect("move the sole target before resolution");
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    pass_to_resolution(&mut engine);
    assert_eq!(engine.state.players[0].life, life_before);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        4
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn sorin_minus_x_prevention_does_not_reduce_the_separate_life_gain() {
    let mut engine = setup(&[10, 20]);
    let source = put_sorin(&mut engine, 0, 6);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let life_before = engine.state.players[0].life;
    engine.state.add_damage_prevention_shield(target, 3);
    let target_ref = permanent_target(&engine, target);

    activate_sorin(&mut engine, 10, source, "activated_02", 3, vec![target_ref])
        .expect("activate -X at a legal creature");
    pass_to_resolution(&mut engine);
    assert_eq!(engine.state.players[0].life, life_before + 3);
    assert_eq!(engine.state.objects[&target].damage, 0);
    assert_eq!(engine.state.remaining_damage_prevention(target), 0);
}

#[test]
fn sorin_minus_nine_counts_controller_in_all_player_maximum_and_batches_tokens() {
    let mut engine = setup(&[10, 20, 30]);
    let source = put_sorin(&mut engine, 0, 15);
    engine.state.players[0].life = 30;
    engine.state.players[1].life = 25;
    engine.state.players[2].life = 20;

    activate_sorin(&mut engine, 10, source, "activated_03", 0, vec![])
        .expect("activate Sorin's -9");
    let mut batches = pass_to_sorin_token_entry_order(&mut engine);
    let token_order = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the token batch is parked on its entry-order choice");
    assert_eq!(
        token_order.presentation.choice_kind,
        ChoiceKind::SimultaneousEntryOrder
    );
    assert_eq!(token_order.deciding_player, 10);
    // The resolving effect already captured 30. A later live-life change must not recalculate the
    // quantity while the simultaneously created tokens await their timestamp order.
    engine.state.players[0].life = 100;
    batches.extend(
        answer_simultaneous_entry_order_in_engine_order(&mut engine)
            .into_iter()
            .map(|(_, _, batch)| batch),
    );
    assert!(engine.state.pending_resolution.is_none());
    let token_ids = battlefield_token_ids(&engine)
        .into_iter()
        .filter(|object_id| engine.state.objects[object_id].card_id == VAMPIRE_KNIGHT)
        .collect::<Vec<_>>();
    assert_eq!(
        token_ids.len(),
        30,
        "All/Maximum includes Sorin's controller"
    );
    for object_id in &token_ids {
        assert_eq!(engine.state.objects[object_id].controller, 10);
    }
    let final_view = last_zone_view(
        batches
            .last()
            .expect("token ordering produces a final batch"),
    );
    let controller_view = final_view
        .per_player
        .iter()
        .find(|view| view.player_id == 10)
        .expect("Sorin's controller receives its battlefield view");
    let runtime_tokens = controller_view
        .battlefield_objects
        .iter()
        .filter(|object| object.card_id == VAMPIRE_KNIGHT)
        .collect::<Vec<_>>();
    assert_eq!(runtime_tokens.len(), 30);
    for token in runtime_tokens {
        assert!(token.is_creature, "the battlefield view reports a creature");
        assert_eq!((token.power, token.toughness), (1, 1));
        assert!(token.keywords.iter().any(|keyword| keyword == "Lifelink"));
        let identity = token
            .token_identity
            .as_ref()
            .expect("the live battlefield token carries its token identity");
        assert_eq!(identity.color, "b", "the token is black only");
        assert_eq!(identity.pt, "1/1");
        assert_eq!(identity.types, ["Creature", "Vampire", "Knight"]);
        assert!(identity.is_creature);
        assert_eq!(identity.keywords, ["Lifelink"]);
    }
    let token_creation_batches = batches
        .iter()
        .filter(|batch| {
            batch
                .events
                .iter()
                .any(|event| matches!(event.ev, Some(Ev::TokenCreated(_))))
        })
        .collect::<Vec<_>>();
    assert_eq!(token_creation_batches.len(), 1);
    assert_eq!(
        token_creation_batches[0]
            .events
            .iter()
            .filter(|event| matches!(event.ev, Some(Ev::TokenCreated(_))))
            .count(),
        30,
        "the computed cohort enters in one simultaneous token batch"
    );
}

#[test]
fn sorin_minus_nine_reads_resolution_life_and_excludes_a_lost_player() {
    let mut engine = setup(&[10, 20, 30, 40]);
    let source = put_sorin(&mut engine, 0, 15);
    engine.state.players[0].life = 20;
    engine.state.players[1].life = 25;
    engine.state.players[2].life = 18;
    engine.state.players[3].life = 40;

    activate_sorin(&mut engine, 10, source, "activated_03", 0, vec![])
        .expect("activate -9 before changing the live life totals");
    engine.state.players[2].life = 35;
    engine.state.players[3].life = 100;
    engine.state.players[3].has_lost = true;
    let batches = pass_to_resolution(&mut engine);
    let token_ids = battlefield_token_ids(&engine)
        .into_iter()
        .filter(|object_id| engine.state.objects[object_id].card_id == VAMPIRE_KNIGHT)
        .collect::<Vec<_>>();
    assert_eq!(
        token_ids.len(),
        35,
        "life is read at resolution and the lost 100-life player is excluded"
    );
    assert!(token_ids
        .iter()
        .all(|object_id| engine.state.objects[object_id].controller == 10));
    assert_eq!(
        batches
            .iter()
            .flat_map(|batch| &batch.events)
            .filter(|event| matches!(event.ev, Some(Ev::TokenCreated(_))))
            .count(),
        35
    );
}
