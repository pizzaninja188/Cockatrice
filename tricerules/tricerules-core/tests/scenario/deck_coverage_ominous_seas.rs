//! Actual-card coverage for Ominous Seas' draw trigger, counter-removal cost, token, and Cycling.
//!
//! Oracle and rulings were checked against Scryfall on 2026-09-26. CR 121.2 and 603.2c cover one
//! trigger per actual card draw; CR 602.2 and 122.1 cover paying and removing its named counters;
//! CR 702.29a-b cover Cycling from hand; CR 111.3-4 define the token's characteristics and name.

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, AbilitySourceZone, ActivateAbility,
    CostChoiceKind, CostObjectRef, CostSelection, CounterRemovalSelection,
};

const OMINOUS_SEAS: &str = "ominous_seas";
const KRAKEN_TOKEN: &str = "kraken_token_8_8";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("island", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_ominous_seas(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, OMINOUS_SEAS);
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, OMINOUS_SEAS);
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Ominous Seas");
    resolve_entire_stack_two_player(engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    source
}

fn draw_two_with_vision_skeins(engine: &mut GameEngine) {
    inject_card_into_hand(engine, 0, "vision_skeins");
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "vision_skeins");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Vision Skeins");
    resolve_entire_stack_two_player(engine);
}

fn cycle_ominous_seas_from_hand(engine: &mut GameEngine, source: u32) {
    give_mana(
        engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 1,
            ..Default::default()
        })),
    };
    engine
        .apply_command(0, &command)
        .expect("activate Cycling from hand");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    let refreshed = engine.initial_response_batch();
    assert!(refreshed.events.iter().all(|event| !matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::ActivePublicRevealSnapshot(snapshot))
            if snapshot.reveals.iter().any(|reveal| reveal.cards.iter().any(|card| card.object_id == source))
    )), "DiscardSelf already moves the card to a public zone and needs no duplicate reveal window");
    resolve_entire_stack_two_player(engine);
}

fn counter_selection(source: u32, generation: u64, option_id: u32) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(Selection::CounterRemoval(CounterRemovalSelection {
            source: Some(CostObjectRef {
                object_id: source,
                zone_change_generation: generation,
            }),
            option_id,
        })),
    }
}

fn activate_kraken(
    engine: &GameEngine,
    source: u32,
    selection: CostSelection,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, Vec::new());
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!("constructed an activation command");
    };
    ability.cost_selections.push(selection);
    command
}

#[test]
fn ominous_seas_counts_controller_draws_cycles_from_hand_and_pays_eight_counters() {
    let mut engine = engine(202_609_280);
    let source = cast_ominous_seas(&mut engine);
    assert!(engine.state.objects[&source]
        .counter_annotation()
        .is_empty());

    for _ in 0..3 {
        draw_two_with_vision_skeins(&mut engine);
    }
    assert_eq!(
        engine.state.objects[&source].counter_annotation(),
        "6 foreshadow counter(s)",
        "Vision Skeins draws two cards for each player, but only its controller's six draws count"
    );

    let seventh_draw = inject_card_into_hand(&mut engine, 0, OMINOUS_SEAS);
    cycle_ominous_seas_from_hand(&mut engine, seventh_draw);
    assert_eq!(
        engine.state.objects[&source].counter_annotation(),
        "7 foreshadow counter(s)"
    );

    let generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    let command_index = engine.state.command_index;
    engine
        .apply_command(
            0,
            &activate_kraken(&engine, source, counter_selection(source, generation, 10)),
        )
        .expect_err("seven foreshadow counters cannot pay the eight-counter cost");
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&source].counter_annotation(),
        "7 foreshadow counter(s)"
    );
    assert!(engine.state.stack.is_empty());

    let eighth_draw = inject_card_into_hand(&mut engine, 0, OMINOUS_SEAS);
    cycle_ominous_seas_from_hand(&mut engine, eighth_draw);
    assert_eq!(
        engine.state.objects[&source].counter_annotation(),
        "8 foreshadow counter(s)"
    );

    let legal = engine.initial_response_batch();
    let ability_key = u64::from(source) << 32;
    let costs = &legal.legal_by_player[&0].cost_choices_by_ability[&ability_key];
    assert!(costs.non_mana_costs_payable);
    let choice = costs
        .choices
        .iter()
        .find(|choice| choice.kind() == CostChoiceKind::RemoveCounters)
        .expect("Ominous Seas publishes its counter cost");
    let removal = choice.counter_removal.as_ref().expect("counter options");
    assert_eq!(removal.count, 8);
    assert_eq!(removal.source.as_ref().unwrap().object_id, source);
    assert_eq!(removal.options.len(), 1);
    assert_eq!(removal.options[0].option_id, 10);
    assert_eq!(removal.options[0].label, "foreshadow");
    assert_eq!(removal.options[0].available_count, 8);

    let selection = counter_selection(source, generation, removal.options[0].option_id);
    engine
        .apply_command(0, &activate_kraken(&engine, source, selection))
        .expect("remove eight foreshadow counters as the activation cost");
    assert!(engine.state.objects[&source]
        .counter_annotation()
        .is_empty());
    assert!(
        battlefield_token_oids(&engine, 0, KRAKEN_TOKEN).is_empty(),
        "the Kraken is created only when the ability resolves"
    );
    resolve_entire_stack_two_player(&mut engine);

    let krakens = battlefield_token_oids(&engine, 0, KRAKEN_TOKEN);
    assert_eq!(krakens.len(), 1);
    let kraken = krakens[0];
    assert_eq!(engine.effective_power(kraken), Some(8));
    assert_eq!(engine.effective_toughness(kraken), Some(8));
    assert_eq!(
        engine.characteristics(kraken).unwrap().colors,
        vec![tricerules_cards::Color::Blue]
    );
    assert!(battlefield_token_oids(&engine, 1, KRAKEN_TOKEN).is_empty());
}
