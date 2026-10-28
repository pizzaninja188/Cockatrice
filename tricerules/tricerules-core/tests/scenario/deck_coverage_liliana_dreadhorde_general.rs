//! Actual Liliana, Dreadhorde General clauses and ordered type-choice resolution.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::{AbilityCost, CounterKind};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_event::Ev, ChoiceKind};

const LILIANA: &str = "liliana,_dreadhorde_general";

fn setup(players: &[i32]) -> GameEngine {
    let decks = (0..players.len())
        .map(|seat| {
            if seat == 0 {
                deck_with("swamp", &[LILIANA])
            } else {
                deck_with("forest", &[])
            }
        })
        .collect();
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        281_028,
        players,
        30,
        Some(decks),
        true,
    )
    .expect("registered Liliana scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put_liliana(engine: &mut GameEngine, seat: usize, loyalty: u32) -> u32 {
    let source = inject_permanent_on_battlefield(engine, seat, LILIANA);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, loyalty);
    engine.initial_response_batch();
    source
}

fn ability_index(ability_id: &str) -> u32 {
    tricerules_cards::registry::global()
        .get(LILIANA)
        .expect("Liliana is registered")
        .primary_face()
        .activated_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == ability_id)
        .unwrap_or_else(|| panic!("Liliana ability {ability_id} is present")) as u32
}

fn resolve_until_idle(engine: &mut GameEngine) {
    for _ in 0..64 {
        if let Some(pending) = engine.state.pending_resolution.as_ref() {
            let actor = pending.deciding_player;
            let order = pending.presentation.candidates.clone();
            engine
                .apply_scenario_command(actor, &submit_resolution_choice(order))
                .expect("complete the engine-offered ordering choice");
            continue;
        }
        if engine.state.stack.is_empty() && engine.state.pending_triggers.is_empty() {
            return;
        }
        pass_priority_round(engine);
    }
    panic!(
        "Liliana scenario did not finish: stack={}, pending={:?}",
        engine.state.stack.len(),
        engine.state.pending_resolution
    );
}

#[test]
fn liliana_dreadhorde_general_has_exact_identity_and_four_clauses() {
    let card = tricerules_cards::registry::global()
        .get(LILIANA)
        .expect("Liliana, Dreadhorde General needs a complete ruled definition");
    assert_eq!(card.name, "Liliana, Dreadhorde General");
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{4}{B}{B}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Liliana"]);
    assert_eq!(face.loyalty, Some(6));
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(face.activated_abilities.len(), 3);
    let loyalty_costs = face
        .activated_abilities
        .iter()
        .map(|ability| match ability.costs.as_slice() {
            [AbilityCost::Loyalty(amount)] => *amount,
            costs => panic!("expected one loyalty cost, found {costs:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(loyalty_costs, [1, -4, -9]);
}

#[test]
fn liliana_passive_draws_for_its_controller_when_source_and_creatures_die_together() {
    let mut engine = setup(&[10, 20]);
    let source = put_liliana(&mut engine, 0, 6);
    let own_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    let library_before = engine.state.players[0].library.len();

    inject_card_into_hand(&mut engine, 0, "all_is_dust");
    give_mana(
        &mut engine,
        10,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "all_is_dust");
    engine
        .apply_scenario_command(10, &cast_spell(slot, vec![]))
        .expect("cast All Is Dust to sacrifice Liliana and both creatures together");
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&own_creature].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&opponent_creature].zone,
        Zone::Graveyard
    );
    resolve_until_idle(&mut engine);

    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 1,
        "the own-creature death draws once; the simultaneous opponent death does not"
    );
}

#[test]
fn liliana_plus_one_creates_one_black_two_two_zombie() {
    let mut engine = setup(&[10, 20]);
    let source = put_liliana(&mut engine, 0, 6);
    apply_ability(
        &mut engine,
        10,
        source,
        ability_index("activated_01"),
        vec![],
    )
    .expect("activate Liliana's +1");
    pass_priority_round(&mut engine);

    let zombies = battlefield_token_oids(&engine, 0, "zombie_b_2_2");
    assert_eq!(zombies.len(), 1);
    let stats = engine.characteristics(zombies[0]).unwrap();
    assert!(stats.has_type("Creature"));
    assert!(stats.has_type("Zombie"));
    assert_eq!(stats.colors, [tricerules_cards::primitives::Color::Black]);
    assert_eq!((stats.power, stats.toughness), (Some(2), Some(2)));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        7
    );
}

#[test]
fn liliana_minus_four_with_four_loyalty_leaves_before_each_player_sacrifices() {
    let mut engine = setup(&[10, 20, 30]);
    let source = put_liliana(&mut engine, 0, 4);
    let p1_creatures = (0..3)
        .map(|_| inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears"))
        .collect::<Vec<_>>();
    let p2_creature = inject_creature_on_battlefield(&mut engine, 2, "storm_crow");
    let library_before = engine.state.players[0].library.len();

    engine
        .apply_scenario_command(
            10,
            &activate_ability_for(&engine, source, ability_index("activated_02"), vec![]),
        )
        .expect("activate Liliana's -4 at exactly four loyalty");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    pass_priority_round(&mut engine);

    let first = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P1 chooses which two creatures to sacrifice");
    assert_eq!(first.deciding_player, 20);
    assert_eq!(first.presentation.choice_kind, ChoiceKind::PermanentObjects);
    assert_eq!(first.presentation.candidates, p1_creatures);
    engine
        .apply_scenario_command(20, &submit_resolution_choice(p1_creatures[..2].to_vec()))
        .expect("P1 chooses two of three creatures");

    assert_eq!(engine.state.objects[&p1_creatures[0]].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&p1_creatures[1]].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&p1_creatures[2]].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&p2_creature].zone, Zone::Graveyard);
    resolve_until_idle(&mut engine);
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before,
        "the source left for zero loyalty before opponents' creatures died"
    );
}

#[test]
fn liliana_minus_nine_uses_all_six_types_and_resolves_after_exact_nine_source_leaves() {
    let mut engine = setup(&[10, 20]);
    let source = put_liliana(&mut engine, 0, 8);
    assert!(engine
        .apply_scenario_command(
            10,
            &activate_ability_for(&engine, source, ability_index("activated_03"), vec![]),
        )
        .is_err());
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        8,
        "an unpaid -9 leaves the source unchanged"
    );
    assert!(engine.state.stack.is_empty());
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 9);
    engine.initial_response_batch();

    let artifact_creature = inject_permanent_on_battlefield(&mut engine, 1, "darksteel_myr");
    let unchosen_indestructible = inject_permanent_on_battlefield(&mut engine, 1, "darksteel_myr");
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let enchantment_a = inject_permanent_on_battlefield(&mut engine, 1, "abundance");
    let enchantment_b = inject_permanent_on_battlefield(&mut engine, 1, "phyrexian_arena");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let planeswalker = put_liliana(&mut engine, 1, 6);
    let opponent_library_before = engine.state.players[1].library.len();
    let battle = inject_permanent_on_battlefield(
        &mut engine,
        1,
        "invasion_of_ulgrotha_grandmother_ravi_sengir",
    );
    engine
        .state
        .objects
        .get_mut(&battle)
        .unwrap()
        .set_counter(CounterKind::Defense, 5);
    engine
        .state
        .objects
        .get_mut(&unchosen_indestructible)
        .unwrap()
        .set_counter(CounterKind::Finality, 1);
    engine.initial_response_batch();

    engine
        .apply_scenario_command(
            10,
            &activate_ability_for(&engine, source, ability_index("activated_03"), vec![]),
        )
        .expect("activate Liliana's -9 at exactly nine loyalty");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Loyalty),
        0
    );
    pass_priority_round(&mut engine);

    let artifact_choice = engine.state.pending_resolution.as_ref().unwrap();
    assert_eq!(artifact_choice.deciding_player, 20);
    assert!(artifact_choice.presentation.prompt.contains("artifact"));
    assert_eq!(
        artifact_choice.presentation.candidates,
        [artifact_creature, unchosen_indestructible, artifact]
    );
    engine
        .apply_scenario_command(20, &submit_resolution_choice(vec![artifact_creature]))
        .expect("keep the artifact creature for its artifact type");

    let creature_choice = engine.state.pending_resolution.as_ref().unwrap();
    assert!(creature_choice.presentation.prompt.contains("creature"));
    assert!(creature_choice
        .presentation
        .candidates
        .contains(&artifact_creature));
    let next_prompt = engine
        .apply_scenario_command(20, &submit_resolution_choice(vec![artifact_creature]))
        .expect("the same multitype permanent remains eligible as a creature");
    let enchantment_choice = next_prompt
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref()? {
            Ev::ResolutionChoiceRequired(choice) => Some(choice),
            _ => None,
        })
        .expect("the next type prompt is an enchantment choice");
    assert!(enchantment_choice.prompt_text.contains("enchantment"));
    assert_eq!(
        enchantment_choice.accepted_permanent_selections.len(),
        1,
        "the artifact creature appears once in the public keep union"
    );
    assert_eq!(
        enchantment_choice.accepted_permanent_selections[0].object_id,
        artifact_creature
    );
    engine
        .apply_scenario_command(20, &submit_resolution_choice(vec![enchantment_b]))
        .expect("keep one enchantment");
    resolve_until_idle(&mut engine);

    for kept in [artifact_creature, enchantment_b, land, planeswalker, battle] {
        assert_eq!(engine.state.objects[&kept].zone, Zone::Battlefield);
    }
    assert_eq!(
        engine.state.objects[&unchosen_indestructible].zone,
        Zone::Exile,
        "the shared sacrifice mover applies the finality replacement"
    );
    for sacrificed in [artifact, creature, enchantment_a] {
        assert_eq!(engine.state.objects[&sacrificed].zone, Zone::Graveyard);
    }
    assert_eq!(
        engine.state.players[1].library.len(),
        opponent_library_before - 1,
        "the surviving opponent Liliana sees the creature's death once after the simultaneous sacrifice"
    );
}
