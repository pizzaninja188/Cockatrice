//! Actual-card coverage for Esika's Chariot and its green 2/2 Cat token.
//!
//! Oracle and WotC rulings verified 2026-10-06. The 2026-09-25 Comprehensive Rules govern
//! Vehicle power/toughness (208.3, 301.7), Crew (702.122a), ETB triggers (603.6a), target
//! announcement and legality (115.1d, 608.2b), and copiable values (707.2). Chariot's WotC
//! ruling confirms copied tokens do not copy counters or tapped status.

use super::helpers::*;
use tricerules_cards::primitives::{Color, CounterKind};
use tricerules_cards::AbilityPresentation;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, ruled_event::Ev, ChooseTriggerTarget,
    CostObjectRef, CostObjectRefs, CostSelection, RuledCommand, TargetRef, TargetRefKind,
};

const CHARIOT: &str = "esikas_chariot";
const CAT_TOKEN: &str = "cat_g_2_2";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn crew_command(engine: &GameEngine, chariot: u32, creatures: &[u32]) -> RuledCommand {
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
    let mut command = activate_ability_with_costs(chariot, 0, Vec::new(), vec![selection]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(engine, chariot);
    command
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                kind: TargetRefKind::Permanent as i32,
                object_id,
                group_index: 0,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

#[test]
fn esikas_chariot_and_its_cat_token_have_the_complete_definitions() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(CHARIOT)
        .expect("Esika's Chariot registry definition");
    assert_eq!(card.name, "Esika's Chariot");
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), CHARIOT);
    assert_eq!(face.mana_cost.to_string(), "{3}{G}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Artifact", "Vehicle"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(4)));
    assert_eq!(face.triggered_abilities.len(), 2);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(
        face.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        face.triggered_abilities[1].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        face.activated_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![3])
    );

    let token = registry
        .get(CAT_TOKEN)
        .expect("green 2/2 Cat token definition");
    assert!(registry.is_token(CAT_TOKEN));
    assert_eq!(token.name, "Cat");
    assert_eq!(token.primary_face().types, ["Creature", "Cat"]);
    assert_eq!(token.primary_face().colors(), [Color::Green]);
    assert_eq!(
        (token.primary_face().power, token.primary_face().toughness),
        (Some(2), Some(2))
    );
}

#[test]
fn chariot_enters_crews_with_aggregate_power_then_copies_a_legal_token_on_attack() {
    let mut engine = engine(202_610_701);
    let chariot = inject_card_into_hand(&mut engine, 0, CHARIOT);
    let slot = hand_index_for_card(&engine, 0, CHARIOT);
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Esika's Chariot");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&chariot].zone, Zone::Battlefield);

    let original_cats = battlefield_token_oids(&engine, 0, CAT_TOKEN);
    assert_eq!(original_cats.len(), 2, "Chariot creates two Cat tokens");
    for cat in &original_cats {
        let object = &engine.state.objects[cat];
        assert!(object.is_token());
        assert!(object.summoning_sick);
        let characteristics = engine.characteristics(*cat).expect("Cat characteristics");
        assert!(characteristics.has_name("Cat"));
        assert_eq!(characteristics.colors, [Color::Green]);
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(2), Some(2))
        );
    }

    let uncrewed = engine
        .characteristics(chariot)
        .expect("Vehicle characteristics");
    assert!(!uncrewed.is_creature());
    assert_eq!((uncrewed.power, uncrewed.toughness), (None, None));

    let before_short_crew = format!("{:?}", engine.state);
    assert!(
        engine
            .apply_command(0, &crew_command(&engine, chariot, &original_cats[..1]))
            .is_err(),
        "one 2/2 Cat is below Crew 4"
    );
    assert_eq!(format!("{:?}", engine.state), before_short_crew);

    engine
        .apply_command(0, &crew_command(&engine, chariot, &original_cats))
        .expect("two Cats pay Crew 4 together, despite summoning sickness");
    assert!(original_cats
        .iter()
        .all(|cat| engine.state.objects[cat].tapped));
    assert!(!engine.characteristics(chariot).unwrap().is_creature());
    resolve_entire_stack_two_player(&mut engine);
    let crewed = engine.characteristics(chariot).unwrap();
    assert!(crewed.has_type("Artifact") && crewed.has_type("Vehicle") && crewed.is_creature());
    assert_eq!((crewed.power, crewed.toughness), (Some(4), Some(4)));

    end_active_turn(&mut engine, 0);
    assert!(!engine.characteristics(chariot).unwrap().is_creature());
    assert_eq!(
        (
            engine.characteristics(chariot).unwrap().power,
            engine.characteristics(chariot).unwrap().toughness
        ),
        (None, None),
        "Crew's type addition expires at cleanup"
    );
    advance_to_main1_from_game_start(&mut engine); // opponent's turn
    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine); // Chariot controller's next turn

    let opponent_cat = inject_permanent_on_battlefield(&mut engine, 1, CAT_TOKEN);
    assert!(engine.state.objects[&opponent_cat].is_token());
    let nontoken_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let refreshed_cats = battlefield_token_oids(&engine, 0, CAT_TOKEN);
    assert_eq!(refreshed_cats.len(), 2);
    assert!(refreshed_cats
        .iter()
        .all(|cat| !engine.state.objects[cat].tapped));

    engine
        .apply_command(0, &crew_command(&engine, chariot, &refreshed_cats))
        .expect("crew Chariot for the next turn's attack");
    resolve_entire_stack_two_player(&mut engine);
    let target_cat = refreshed_cats[0];
    engine
        .state
        .objects
        .get_mut(&target_cat)
        .expect("target Cat")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    assert_eq!(engine.effective_power(target_cat), Some(3));

    engine
        .apply_command(0, &primitive_yield())
        .expect("advance to beginning of combat");
    pass_both_players(&mut engine);
    let attack_batch = engine
        .apply_command(0, &declare_attackers(vec![chariot]))
        .expect("attack with the crewed Vehicle");
    let prompt = attack_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::TriggerNeedsTarget(prompt)) => Some(prompt),
            _ => None,
        })
        .expect("attack trigger asks for a token target");
    assert_eq!(prompt.source_permanent_id, chariot);
    assert_eq!(prompt.controller_player_id, 0);

    for illegal_target in [nontoken_creature, opponent_cat] {
        let before_illegal_target = format!("{:?}", engine.state);
        assert!(
            engine
                .apply_command(0, &choose_trigger_target(illegal_target))
                .is_err(),
            "only a token you control is legal"
        );
        assert_eq!(format!("{:?}", engine.state), before_illegal_target);
    }

    engine
        .apply_command(0, &choose_trigger_target(target_cat))
        .expect("target the tapped Cat token you control");
    resolve_entire_stack_two_player(&mut engine);
    let cats_after_copy = battlefield_token_oids(&engine, 0, CAT_TOKEN);
    assert_eq!(
        cats_after_copy.len(),
        3,
        "the attack trigger creates one copy"
    );
    let copy = *cats_after_copy
        .iter()
        .find(|cat| !refreshed_cats.contains(cat))
        .expect("new copied Cat token");
    assert!(engine.state.objects[&copy].is_token());
    assert!(
        !engine.state.objects[&copy].tapped,
        "copy does not copy tapped status"
    );
    assert_eq!(
        engine.state.objects[&copy].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "copy does not copy the target's counter"
    );
    let original = engine.characteristics(target_cat).unwrap();
    let copied = engine.characteristics(copy).unwrap();
    assert_eq!((original.power, original.toughness), (Some(3), Some(3)));
    assert!(copied.has_name("Cat"));
    assert_eq!(copied.types, ["Creature", "Cat"]);
    assert_eq!(copied.colors, [Color::Green]);
    assert_eq!((copied.power, copied.toughness), (Some(2), Some(2)));
}
