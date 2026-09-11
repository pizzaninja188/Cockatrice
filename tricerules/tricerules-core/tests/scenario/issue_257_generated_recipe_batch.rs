//! Issue #257: generated Crew, mana Ward, Changeling, and self-uncounterability cards.
//!
//! Oracle verified 2026-09-11. Governing rules: CR 702.122, 702.21, 702.73,
//! 604.3, 113.6g, 613, and 701.6.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CostObjectRef, CostObjectRefs, CostSelection,
    ResolutionChoiceDecision,
};

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn crew_command(engine: &GameEngine, vehicle: u32, creatures: &[u32]) -> RuledCommand {
    let selection = CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: creatures
                .iter()
                .map(|object_id| CostObjectRef {
                    object_id: *object_id,
                    zone_change_generation: generation(engine, *object_id),
                })
                .collect(),
        })),
    };
    let mut command = activate_ability_with_costs(vehicle, 1, vec![], vec![selection]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(engine, vehicle);
    command
}

fn engine_with_caravan(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &["cultivators_caravan"]),
            deck_with("island", &[]),
        ]),
        true,
    )
    .expect("generated Cultivator's Caravan must be registered");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn issue_257_generated_crew_uses_current_power_and_generation_bound_atomic_payment() {
    let mut engine = engine_with_caravan(257_001);
    let caravan = relocate_to_battlefield(&mut engine, 0, "cultivators_caravan", false);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");

    let legal = engine.initial_response_batch();
    let choices =
        &legal.legal_by_player[&0].cost_choices_by_ability[&((u64::from(caravan) << 32) | 1)];
    let crew = &choices.choices[0];
    assert_eq!(crew.aggregate_minimum.as_ref().unwrap().minimum, 3);
    let candidates = crew
        .candidate_objects
        .iter()
        .map(|candidate| {
            (
                candidate.object.as_ref().unwrap().object_id,
                candidate.contribution,
            )
        })
        .collect::<Vec<_>>();
    assert!(candidates.contains(&(first, 2)) && candidates.contains(&(second, 2)));
    assert!(!candidates
        .iter()
        .any(|(id, _)| [caravan, land].contains(id)));

    engine
        .apply_command(0, &crew_command(&engine, caravan, &[first]))
        .expect_err("one unmodified Bear has insufficient power for Crew 3");
    assert!(!engine.state.objects[&first].tapped);
    assert!(engine.state.stack.is_empty());

    engine.state.objects.get_mut(&first).unwrap().add_counters(
        CounterKind::PlusOnePlusOne,
        1,
        engine.state.command_index,
    );
    let stale = crew_command(&engine, caravan, &[first]);
    *engine
        .state
        .zone_change_generation
        .entry(first)
        .or_default() += 1;
    engine
        .apply_command(0, &stale)
        .expect_err("a stale physical Crew contributor must be rejected");
    assert!(!engine.state.objects[&first].tapped);
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(0, &crew_command(&engine, caravan, &[first]))
        .expect("the Bear's current power of 3 pays Crew 3");
    assert!(engine.state.objects[&first].tapped);
    assert!(!engine.characteristics(caravan).unwrap().is_creature());
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.characteristics(caravan).unwrap().is_creature());
    assert_eq!(
        (
            engine.characteristics(caravan).unwrap().power,
            engine.characteristics(caravan).unwrap().toughness,
        ),
        (Some(5), Some(5))
    );

    end_active_turn(&mut engine, 0);
    assert!(!engine.characteristics(caravan).unwrap().is_creature());
}

fn ward_engine(seed: u64) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["unsummon", "prodigal_sorcerer"]),
            deck_with("forest", &["spider-rex,_daring_dino"]),
        ]),
        true,
    )
    .expect("generated Spider-Rex must be registered");
    advance_to_main1_from_game_start(&mut engine);
    let spider = relocate_to_battlefield(&mut engine, 1, "spider-rex,_daring_dino", false);
    (engine, spider)
}

#[test]
fn issue_257_generated_ward_handles_opponent_spells_and_abilities_but_not_own_spells() {
    let (mut paid, spider) = ward_engine(257_002);
    ensure_in_hand(&mut paid, 0, "unsummon");
    give_mana(
        &mut paid,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&paid, 0, "unsummon");
    let spell = paid.state.players[0].hand[slot];
    paid.apply_command(0, &cast_spell(slot, target_object(spider)))
        .expect("opponent spell targets Spider-Rex");
    assert_eq!(paid.state.stack.len(), 2);
    assert_eq!(
        paid.state.stack.last().unwrap().ability_text.as_deref(),
        Some("Spider-Rex, Daring Dino — triggered ability (triggered_01)")
    );
    pass_both_players(&mut paid);
    assert_eq!(
        paid.state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation
            .mana_payment()
            .unwrap()
            .generic_mana_cost,
        2
    );
    submit_mana_resolution_decision(&mut paid, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay Ward {2}");
    assert!(paid.state.stack.iter().any(|item| item.id == spell));

    let (mut unpaid, spider) = ward_engine(257_003);
    let prodigal = relocate_to_battlefield(&mut unpaid, 0, "prodigal_sorcerer", false);
    unpaid
        .apply_command(0, &activate_ability(prodigal, 0, target_object(spider)))
        .expect("opponent ability targets Spider-Rex");
    let ability = unpaid.state.stack[0].id;
    pass_both_players(&mut unpaid);
    let batch = unpaid
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline Ward");
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == ability
    )));
    assert!(!batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == ability
    )));

    let mut own = GameEngine::new(
        257_004,
        &[0, 1],
        20,
        Some(vec![
            deck_with("island", &["unsummon", "spider-rex,_daring_dino"]),
            deck_with("forest", &[]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut own);
    let spider = relocate_to_battlefield(&mut own, 0, "spider-rex,_daring_dino", false);
    ensure_in_hand(&mut own, 0, "unsummon");
    give_mana(
        &mut own,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    own.apply_command(
        0,
        &cast_spell(
            hand_index_for_card(&own, 0, "unsummon"),
            target_object(spider),
        ),
    )
    .expect("controller targets their own Spider-Rex");
    assert_eq!(own.state.stack.len(), 1);
}

#[test]
fn issue_257_generated_changeling_supplies_all_creature_types_in_every_zone() {
    let mut engine = GameEngine::new(
        257_005,
        &[0, 1],
        20,
        Some(vec![deck_with("plains", &[]), deck_with("island", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let objects = [
        inject_library_card(&mut engine, 0, "prideful_feastling"),
        inject_card_into_hand(&mut engine, 0, "prideful_feastling"),
        inject_creature_on_battlefield(&mut engine, 0, "prideful_feastling"),
        inject_graveyard_card(&mut engine, 0, "prideful_feastling"),
    ];
    assert_eq!(
        objects.map(|id| engine.state.objects[&id].zone),
        [
            Zone::Library,
            Zone::Hand,
            Zone::Battlefield,
            Zone::Graveyard
        ]
    );
    for object in objects {
        let characteristics = engine.characteristics(object).expect("Prideful Feastling");
        assert!(characteristics.has_type("Goblin"));
        assert!(characteristics.has_type("Dragon"));
        assert!(characteristics.has_type("Sliver"));
    }
}

fn counter_creature_spell(
    seed: u64,
    card_id: &str,
    green: u32,
    colorless: u32,
) -> (GameEngine, u32) {
    let mut engine = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &[card_id]),
            deck_with("island", &["counterspell"]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, card_id);
    ensure_in_hand(&mut engine, 1, "counterspell");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: green,
            c: colorless,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, card_id);
    let spell = engine.state.players[0].hand[slot];
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    engine
        .apply_command(
            1,
            &cast_spell(
                hand_index_for_card(&engine, 1, "counterspell"),
                target_object(spell),
            ),
        )
        .unwrap();
    engine.apply_command(1, &pass()).unwrap();
    (engine, spell)
}

#[test]
fn issue_257_only_the_generated_bear_spell_resists_counterspell() {
    let (mut gigantic, spell) = counter_creature_spell(257_006, "gigantic_big_bear", 2, 5);
    let batch = gigantic.apply_command(0, &pass()).unwrap();
    assert!(!batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == spell
    )));
    assert!(gigantic.state.stack.iter().any(|item| item.id == spell));
    resolve_entire_stack_two_player(&mut gigantic);
    assert_eq!(gigantic.state.objects[&spell].zone, Zone::Battlefield);

    let (mut ordinary, spell) = counter_creature_spell(257_007, "grizzly_bears", 1, 1);
    let batch = ordinary.apply_command(0, &pass()).unwrap();
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackObjectCountered(countered)) if countered.object_id == spell
    )));
    assert_eq!(ordinary.state.objects[&spell].zone, Zone::Graveyard);
}
