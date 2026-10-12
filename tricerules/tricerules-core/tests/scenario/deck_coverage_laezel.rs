//! Lae'zel's counter-placement replacement behavior.
use super::helpers::*;
use prost::Message;
use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{AbilityPresentation, CounterKind, Keyword, Layout};
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1 as rv1;

const LAEZEL: &str = "laezel,_vlaakiths_champion";

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let objects: Vec<_> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|object_id| !objects.contains(object_id));
    for object_id in objects.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    objects
}

fn choose_resolution_branch(index: u32) -> rv1::RuledCommand {
    rv1::RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
            rv1::SubmitResolutionChoice {
                decision: rv1::ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: index,
                ..Default::default()
            },
        )),
    }
}

fn blight_cost_selection(
    cost_index: u32,
    object_id: u32,
    zone_change_generation: u64,
) -> rv1::CostSelection {
    rv1::CostSelection {
        cost_index,
        selection: Some(rv1::cost_selection::Selection::BattlefieldObjects(
            rv1::CostObjectRefs {
                objects: vec![rv1::CostObjectRef {
                    object_id,
                    zone_change_generation,
                }],
            },
        )),
    }
}

fn setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("plains", &[]), deck_with("island", &[])]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn laezel_registers_exact_identity_and_counter_replacement() {
    let card = tricerules_cards::registry::global()
        .get(LAEZEL)
        .expect("complete Lae'zel definition");
    assert_eq!(card.name, "Lae'zel, Vlaakith's Champion");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "laezel_vlaakiths_champion");
    assert_eq!(face.mana_cost.to_string(), "{2}{W}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Gith", "Warrior"]);
    assert_eq!(face.keywords, [Keyword::ChooseABackground]);
    assert_eq!(face.keywords[0].as_str(), "Choose a Background");
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        face.static_abilities[0].definition,
        StaticAbilityDef::AddOneCounterOfEachKindToSelfOrControlledCreaturePlaneswalker
    );
}

#[test]
fn laezel_and_doubling_season_order_replacements_for_entry_counters_before_commit() {
    for (replacement_choice, expected_count) in [(0, 6), (1, 5)] {
        let mut engine = setup(20_261_021 + replacement_choice as u64);
        inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        inject_card_into_hand(&mut engine, 0, "endless_one");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );

        let endless_one = hand_index_for_card(&engine, 0, "endless_one");
        engine
            .apply_command(0, &cast_spell_x(endless_one, vec![], 2))
            .expect("cast Endless One with X=2");
        pass_both_players(&mut engine);

        let choice = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("counter replacement order before the entrant commits");
        assert_eq!(
            choice.presentation.choice_kind,
            tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect
        );
        assert_eq!(choice.presentation.candidates.len(), 2);
        let object_id = engine
            .state
            .objects
            .values()
            .find(|object| object.card_id == "endless_one")
            .expect("cast Endless One object")
            .id;
        assert_eq!(
            engine.state.objects[&object_id].zone,
            tricerules_core::state::Zone::Stack
        );
        assert_eq!(
            engine.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "entry counters remain staged until both replacement effects are ordered"
        );

        let selected = choice.presentation.candidates[replacement_choice];
        engine
            .apply_command(0, &submit_resolution_choice(vec![selected]))
            .expect("choose the first entry-counter replacement");

        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.objects[&object_id].zone,
            tricerules_core::state::Zone::Battlefield
        );
        assert_eq!(
            engine.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne),
            expected_count,
            "Lae'zel first yields six counters; Doubling Season first yields five"
        );
    }
}

#[test]
fn laezel_adds_one_counter_to_a_positive_loyalty_cost() {
    let mut engine = setup(20_261_012);
    inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    let chandra = inject_permanent_on_battlefield(&mut engine, 0, "chandra,_novice_pyromancer");
    engine
        .state
        .objects
        .get_mut(&chandra)
        .expect("Chandra")
        .counters
        .insert(CounterKind::Loyalty, 5);

    engine
        .apply_command(0, &activate_ability(chandra, 0, vec![]))
        .expect("activate Chandra's +1 loyalty ability");

    assert_eq!(
        engine.state.objects[&chandra].counter_count(CounterKind::Loyalty),
        7,
        "the +1 cost puts one loyalty counter plus Lae'zel's additional one"
    );
}

#[test]
fn laezel_stages_positive_loyalty_cost_until_counter_replacements_finish() {
    let mut engine = setup(20_261_018);
    inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    for _ in 0..3 {
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    }
    let chandra = inject_permanent_on_battlefield(&mut engine, 0, "chandra,_novice_pyromancer");
    engine
        .state
        .objects
        .get_mut(&chandra)
        .expect("Chandra")
        .counters
        .insert(CounterKind::Loyalty, 5);
    let stack_len = engine.state.stack.len();
    let next_object_id = engine.state.next_object_id;

    engine
        .apply_command(0, &activate_ability(chandra, 0, vec![]))
        .expect("begin Chandra's +1 loyalty activation");

    for expected_choices in [3, 2] {
        let choice = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("replacement-order choice before cost commitment");
        assert_eq!(
            choice.presentation.choice_kind,
            tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect
        );
        assert_eq!(choice.presentation.candidates.len(), expected_choices);
        assert_eq!(
            engine.state.objects[&chandra].counter_count(CounterKind::Loyalty),
            5,
            "the entire cost remains unpaid while replacement choices are pending"
        );
        assert_eq!(engine.state.stack.len(), stack_len);
        assert_eq!(engine.state.next_object_id, next_object_id);
        let selected = choice.presentation.candidates[0];
        engine
            .apply_command(0, &submit_resolution_choice(vec![selected]))
            .expect("choose the next counter replacement");
    }

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&chandra].counter_count(CounterKind::Loyalty),
        21,
        "Lae'zel changes the one-counter cost to two, then three Doubling Seasons apply"
    );
    assert_eq!(engine.state.stack.len(), stack_len + 1);
    assert_eq!(engine.state.next_object_id, next_object_id + 1);
}

#[test]
fn laezel_and_doubling_season_order_effect_counter_replacements_before_commit() {
    let mut engine = setup(20_261_013);
    inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "battlegrowth");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "battlegrowth");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Battlegrowth");
    pass_both_players(&mut engine);

    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("replacement order choice");
    assert_eq!(
        choice.presentation.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect
    );
    assert_eq!(choice.presentation.candidates.len(), 2);
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "the simultaneous counter placement stays uncommitted while replacements are ordered"
    );

    engine
        .apply_command(
            0,
            &submit_resolution_choice(vec![choice.presentation.candidates[0]]),
        )
        .expect("choose the first counter replacement");
    assert!(engine.state.pending_resolution.is_none());
    assert!(matches!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        3 | 4
    ));
}

#[test]
fn laezel_and_doubling_season_order_prevention_counters_before_damage_commit() {
    let mut engine = setup(20_261_031);
    inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_card_into_hand(&mut engine, 0, "anti-venom,_horrifying_healer");
    inject_card_into_hand(&mut engine, 0, "shock");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            w: 5,
            ..Default::default()
        },
    );

    let anti_venom_slot = hand_index_for_card(&engine, 0, "anti-venom,_horrifying_healer");
    engine
        .apply_command(0, &cast_spell(anti_venom_slot, vec![]))
        .expect("cast Anti-Venom");
    pass_both_players(&mut engine);
    let anti_venom = engine
        .state
        .objects
        .values()
        .find(|object| {
            object.card_id == "anti-venom,_horrifying_healer"
                && object.zone == tricerules_core::state::Zone::Battlefield
        })
        .expect("Anti-Venom entered")
        .id;
    assert!(engine
        .state
        .damage_prevention_effects
        .iter()
        .any(|effect| effect.source_id == Some(anti_venom)));

    let shock = hand_index_for_card(&engine, 0, "shock");
    engine
        .apply_command(0, &cast_spell(shock, target_object(anti_venom)))
        .expect("cast Shock at Anti-Venom");
    pass_both_players(&mut engine);

    let mut replacement_choices = 0;
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::ReplacementEffect
        );
        assert_eq!(
            engine.state.objects[&anti_venom].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "prevention counters remain staged while their replacements are ordered"
        );
        assert_eq!(engine.state.objects[&anti_venom].damage, 0);
        assert_eq!(
            engine.state.stack.len(),
            0,
            "the resolving Shock is held by its continuation, not on the stack"
        );
        replacement_choices += 1;
        let selected = pending.presentation.candidates[0];
        engine
            .apply_command(0, &submit_resolution_choice(vec![selected]))
            .expect("choose the next prevention-counter replacement");
    }
    assert!(replacement_choices > 0);
    assert_eq!(engine.state.objects[&anti_venom].damage, 0);
    assert!(
        matches!(
            engine.state.objects[&anti_venom].counter_count(CounterKind::PlusOnePlusOne),
            9 | 10 | 12
        ),
        "ordered replacements produced {} counters",
        engine.state.objects[&anti_venom].counter_count(CounterKind::PlusOnePlusOne)
    );
}

#[test]
fn laezel_and_doubling_season_order_earthbend_counters_before_commit() {
    for (replacement_choice, expected_count) in [(0, 6), (1, 5)] {
        let mut engine = setup(20_261_024 + replacement_choice as u64);
        inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
        inject_card_into_hand(&mut engine, 0, "badgermole");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 4,
                g: 1,
                ..Default::default()
            },
        );

        let badgermole = hand_index_for_card(&engine, 0, "badgermole");
        engine
            .apply_command(0, &cast_spell(badgermole, vec![]))
            .expect("cast Badgermole");
        pass_both_players(&mut engine);
        engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::ChooseTriggerTarget(
                        rv1::ChooseTriggerTarget {
                            targets: target_object(land),
                            ..Default::default()
                        },
                    )),
                },
            )
            .expect("choose the Forest for Badgermole's Earthbend trigger");
        pass_both_players(&mut engine);

        let counter_choice = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("counter replacements are ordered before Earthbend commits");
        assert_eq!(
            counter_choice.presentation.choice_kind,
            rv1::ChoiceKind::ReplacementEffect
        );
        assert_eq!(counter_choice.presentation.candidates.len(), 2);
        assert_eq!(
            engine.state.objects[&land].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "Earthbend counters stay staged while Lae'zel and Doubling Season are ordered"
        );
        let characteristics = engine
            .characteristics(land)
            .expect("Earthbent land remains a battlefield permanent during the choice");
        assert!(characteristics.has_type("Land"));
        assert!(characteristics.is_creature());

        engine
            .apply_command(
                0,
                &submit_resolution_choice(vec![
                    counter_choice.presentation.candidates[replacement_choice],
                ]),
            )
            .expect("choose the Earthbend counter replacement order");

        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.objects[&land].zone,
            tricerules_core::state::Zone::Battlefield
        );
        assert_eq!(
            engine.state.objects[&land].counter_count(CounterKind::PlusOnePlusOne),
            expected_count
        );
        assert!(engine.state.active_event_observers.iter().any(|observer| {
            observer
                .watched
                .is_some_and(|watched| watched.object_id == land)
                && matches!(
                    observer.matcher,
                    tricerules_core::state::EventObserverMatcher::WhenWatchedObjectDiesOrIsExiled
                )
        }));
    }
}

#[test]
fn laezel_and_doubling_season_order_explore_counters_before_the_library_choice() {
    let mut engine = setup(20_261_022);
    inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    seat_on_top(&mut engine, 0, &["grizzly_bears"]);
    inject_card_into_hand(&mut engine, 0, "cenote_scout");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let scout_slot = hand_index_for_card(&engine, 0, "cenote_scout");

    engine
        .apply_command(0, &cast_spell(scout_slot, vec![]))
        .expect("cast Cenote Scout");
    pass_both_players(&mut engine);
    let scout = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "cenote_scout")
        .expect("Cenote Scout");
    let scout_id = scout.id;
    assert_eq!(scout.zone, tricerules_core::state::Zone::Battlefield);
    pass_both_players(&mut engine);

    let choice =
        engine.state.pending_resolution.as_ref().expect(
            "Explore's counter replacements are ordered before its optional library choice",
        );
    assert_eq!(
        choice.presentation.choice_kind,
        rv1::ChoiceKind::ReplacementEffect
    );
    assert_eq!(choice.presentation.candidates.len(), 2);
    assert_eq!(
        engine.state.objects[&scout_id].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "Explore counters stay staged while Lae'zel and Doubling Season are ordered"
    );
    let selected = choice.presentation.candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![selected]))
        .expect("choose the first Explore counter replacement");

    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Explore continues to its library choice")
            .presentation
            .choice_kind,
        rv1::ChoiceKind::LibraryLook
    );
    assert!(
        matches!(
            engine.state.objects[&scout_id].counter_count(CounterKind::PlusOnePlusOne),
            3 | 4
        ),
        "ordering Doubling Season and Lae'zel on one Explore counter yields three or four"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("leave the revealed nonland on top");
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn laezel_and_doubling_season_order_blight_effect_counters_before_commit() {
    let mut engine = setup(20_261_023);
    inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let bear = inject_creature_with_stats(&mut engine, 0, "ancient_brontodon", 9, 9);
    inject_card_into_hand(&mut engine, 0, "chaos_spewer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "chaos_spewer");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Chaos Spewer");
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &choose_resolution_branch(1))
        .expect("choose the Blight branch");
    engine
        .apply_command(0, &submit_resolution_choice(vec![bear]))
        .expect("choose a creature to blight");

    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Blight's counter replacements are ordered before placement");
    assert_eq!(
        choice.presentation.choice_kind,
        rv1::ChoiceKind::ReplacementEffect
    );
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::MinusOneMinusOne),
        0,
        "Blight counters remain staged while replacement effects are ordered"
    );
    let selected = choice.presentation.candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![selected]))
        .expect("choose the first Blight counter replacement");
    assert!(engine.state.pending_resolution.is_none());
    assert!(matches!(
        engine.state.objects[&bear].counter_count(CounterKind::MinusOneMinusOne),
        5 | 6
    ));
}

#[test]
fn laezel_stages_blight_activation_cost_until_replacement_order_finishes() {
    let mut engine = setup(20_261_024);
    inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let glutton = inject_creature_on_battlefield(&mut engine, 0, "gristle_glutton");
    let recipient = inject_creature_with_stats(&mut engine, 0, "ancient_brontodon", 20, 20);
    let generation = engine
        .state
        .zone_change_generation
        .get(&recipient)
        .copied()
        .unwrap_or(0);
    let stack_len = engine.state.stack.len();
    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                glutton,
                0,
                vec![],
                vec![blight_cost_selection(1, recipient, generation)],
            ),
        )
        .expect("activate Gristle Glutton with its Blight cost");

    let mut replacement_choices = 0;
    while let Some(pending) = engine.state.pending_resolution.as_ref() {
        assert_eq!(
            pending.presentation.choice_kind,
            rv1::ChoiceKind::ReplacementEffect
        );
        assert!(!engine.state.objects[&glutton].tapped);
        assert_eq!(
            engine.state.objects[&recipient].counter_count(CounterKind::MinusOneMinusOne),
            0,
            "Blight cost counters are not committed while replacement order is selected"
        );
        assert_eq!(engine.state.stack.len(), stack_len);
        replacement_choices += 1;
        let selected = pending.presentation.candidates[0];
        engine
            .apply_command(0, &submit_resolution_choice(vec![selected]))
            .expect("finish the next Blight counter replacement choice");
    }
    assert!(replacement_choices > 0);

    assert!(engine.state.objects[&glutton].tapped);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.stack.len(), stack_len + 1);
    assert!(matches!(
        engine.state.objects[&recipient].counter_count(CounterKind::MinusOneMinusOne),
        16
    ));
}

#[test]
fn laezel_and_doubling_season_apply_each_order_to_two_counter_effect() {
    for (replacement_choice, expected_count) in [(0, 6), (1, 5)] {
        let mut engine = setup(20_261_014 + replacement_choice as u64);
        inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
        inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
        let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        inject_card_into_hand(&mut engine, 0, "efflorescence");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                g: 3,
                ..Default::default()
            },
        );

        let slot = hand_index_for_card(&engine, 0, "efflorescence");
        engine
            .apply_command(0, &cast_spell(slot, target_object(bear)))
            .expect("cast Efflorescence");
        pass_both_players(&mut engine);

        let choice = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("replacement order choice");
        assert_eq!(choice.presentation.candidates.len(), 2);
        assert_eq!(
            engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "the whole counter event remains staged until replacement ordering is complete"
        );
        let selected = choice.presentation.candidates[replacement_choice];
        let resolution = engine
            .apply_command(0, &submit_resolution_choice(vec![selected]))
            .expect("choose a counter replacement order");

        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            resolution
                .events
                .iter()
                .filter(|event| matches!(
                    event.ev.as_ref(),
                    Some(rv1::ruled_event::Ev::Log(log))
                        if log.text.ends_with("counter replacement effect is applied.")
                ))
                .count(),
            2,
            "the selected order logs Lae'zel and Doubling Season exactly once"
        );
        assert_eq!(
            engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
            expected_count,
            "Lae'zel first yields six; Doubling Season first yields five"
        );
    }
}

#[test]
fn laezel_and_doubling_season_mass_counter_choices_commit_as_one_batch() {
    let mut engine = setup(20_261_016);
    let laezel = inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "the_crystals_chosen");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 5,
            w: 2,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "the_crystals_chosen");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast The Crystal's Chosen");
    pass_both_players(&mut engine);

    let mut affected = vec![laezel, bear];
    affected.extend(
        engine
            .state
            .objects
            .values()
            .filter(|object| object.zone == tricerules_core::state::Zone::Battlefield)
            .filter(|object| object.token_origin.is_some())
            .map(|object| object.id),
    );
    assert_eq!(
        affected.len(),
        10,
        "two existing creatures and eight tokens after Doubling Season"
    );
    assert_eq!(
        engine.state.objects[&laezel].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        0
    );

    let mut choices = 0;
    while let Some(choice) = engine.state.pending_resolution.as_ref() {
        assert_eq!(
            choice.presentation.choice_kind,
            tricerules_proto::ruled::v1::ChoiceKind::ReplacementEffect
        );
        let selected = choice.presentation.candidates[0];
        engine
            .apply_command(0, &submit_resolution_choice(vec![selected]))
            .expect("choose the next replacement order");
        choices += 1;
        if engine.state.pending_resolution.is_some() {
            for object_id in &affected {
                assert_eq!(
                    engine.state.objects[object_id].counter_count(CounterKind::PlusOnePlusOne),
                    0,
                    "no recipient commits while a sibling event still needs a choice"
                );
            }
        }
    }
    assert_eq!(choices, affected.len());
    for object_id in affected {
        assert_eq!(
            engine.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne),
            4,
            "each one-counter event applies Lae'zel before Doubling Season"
        );
    }
}

#[test]
fn laezel_and_proliferate_use_the_putting_player_for_permanent_and_player_counters() {
    let mut engine = setup(20_261_017);
    inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&bear)
        .expect("bear")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
    let self_index = engine.state.player_idx(0).expect("active player");
    engine.state.players[self_index]
        .counters
        .insert(CounterKind::Poison, 1);
    let opponent_index = engine.state.player_idx(1).expect("opponent");
    engine.state.players[opponent_index]
        .counters
        .insert(CounterKind::Poison, 1);
    grant_pool(&mut engine, 0);

    engine
        .apply_command(0, &activate_ability(bastion, 1, vec![]))
        .expect("activate Karn's Bastion");
    pass_both_players(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Proliferate choice")
            .presentation
            .choice_kind,
        rv1::ChoiceKind::Proliferate
    );
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![bear],
                    chosen_player_ids: vec![0, 1],
                    decision: rv1::ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
        )
        .expect("proliferate the creature and both players");

    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        3,
        "Lae'zel adds another counter when its controller proliferates their creature"
    );
    assert_eq!(
        engine.state.players[self_index].counters[&CounterKind::Poison],
        3,
        "Lae'zel adds another counter when its controller proliferates themself"
    );
    assert_eq!(
        engine.state.players[opponent_index].counters[&CounterKind::Poison],
        2,
        "Lae'zel does not modify an opponent's counter when the opponent is not the actor"
    );
}

#[test]
fn laezel_proliferate_batch_hides_face_down_recipient_identity_on_wire() {
    let mut engine = setup(20_261_025);
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    let hidden_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let hidden = engine
        .state
        .objects
        .get_mut(&hidden_creature)
        .expect("hidden creature");
    hidden.face_down = true;
    hidden.counters.insert(CounterKind::PlusOnePlusOne, 1);
    grant_pool(&mut engine, 0);

    engine
        .apply_command(0, &activate_ability(bastion, 1, vec![]))
        .expect("activate Karn's Bastion");
    pass_both_players(&mut engine);
    let batch = engine
        .apply_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![hidden_creature],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("proliferate the face-down creature");

    let bytes = batch.encode_to_vec();
    let decoded = rv1::RuledEventBatch::decode(bytes.as_slice()).expect("decode public batch");
    let public_text = decoded
        .events
        .iter()
        .filter_map(|event| match event.ev.as_ref()? {
            rv1::ruled_event::Ev::Log(log) => Some(log.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(public_text.contains("Face-down creature"), "{public_text}");
    assert!(!public_text.contains("Grizzly Bears"), "{public_text}");
}

fn prepare_mixed_proliferate_counter_replacement(seed: u64) -> (GameEngine, u32, usize, usize) {
    let mut engine = setup(seed);
    inject_permanent_on_battlefield(&mut engine, 0, LAEZEL);
    inject_permanent_on_battlefield(&mut engine, 0, "doubling_season");
    let bastion = inject_permanent_on_battlefield(&mut engine, 0, "karns_bastion");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&bear)
        .expect("bear")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
    let self_index = engine.state.player_idx(0).expect("active player");
    engine.state.players[self_index]
        .counters
        .insert(CounterKind::Poison, 1);
    let opponent_index = engine.state.player_idx(1).expect("opponent");
    engine.state.players[opponent_index]
        .counters
        .insert(CounterKind::Poison, 1);
    grant_pool(&mut engine, 0);

    engine
        .apply_command(0, &activate_ability(bastion, 1, vec![]))
        .expect("activate Karn's Bastion");
    pass_both_players(&mut engine);
    engine
        .apply_command(
            0,
            &rv1::RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        chosen_object_ids: vec![bear],
                        chosen_player_ids: vec![0],
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("proliferate the creature and controller");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("counter replacement choice")
            .presentation
            .choice_kind,
        rv1::ChoiceKind::ReplacementEffect
    );
    (engine, bear, self_index, opponent_index)
}

#[test]
fn laezel_and_doubling_season_stage_mixed_proliferate_recipients_together() {
    let (mut engine, bear, self_index, opponent_index) =
        prepare_mixed_proliferate_counter_replacement(20_261_016);
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the creature remains unchanged while its counter replacements are ordered"
    );
    assert_eq!(
        engine.state.players[self_index].counters[&CounterKind::Poison],
        1,
        "the player recipient remains unchanged while the object choice is pending"
    );
    assert_eq!(
        engine.state.players[opponent_index].counters[&CounterKind::Poison],
        1,
        "an unselected player is unchanged"
    );

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("order choice");
    let selected = pending.presentation.candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![selected]))
        .expect("choose Lae'zel before Doubling Season");
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        5,
        "Lae'zel then Doubling Season changes the one proliferated counter to four"
    );
    assert_eq!(
        engine.state.players[self_index].counters[&CounterKind::Poison],
        3,
        "the same parent action commits the controller's Lae'zel-modified poison event"
    );
    assert_eq!(
        engine.state.players[opponent_index].counters[&CounterKind::Poison],
        1,
        "the opponent was not part of the parent action"
    );
}

#[test]
fn stale_mixed_proliferate_counter_choice_commits_no_sibling_recipient() {
    let (mut engine, bear, self_index, opponent_index) =
        prepare_mixed_proliferate_counter_replacement(20_261_015);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("order choice");
    let selected = pending.presentation.candidates[0];
    let owner_index = engine.state.player_idx(0).expect("bear owner");
    engine.state.players[owner_index]
        .battlefield
        .retain(|object_id| *object_id != bear);
    engine.state.players[owner_index].graveyard.push(bear);
    engine.state.objects.get_mut(&bear).unwrap().zone = tricerules_core::state::Zone::Graveyard;
    *engine.state.zone_change_generation.entry(bear).or_default() += 1;

    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![selected]))
        .is_err());
    assert_eq!(
        engine.state.players[self_index].counters[&CounterKind::Poison],
        1,
        "a stale object recipient cannot leave its player sibling partially committed"
    );
    assert_eq!(
        engine.state.players[opponent_index].counters[&CounterKind::Poison],
        1
    );
    assert_eq!(
        engine.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the stale recipient keeps only its preexisting counter"
    );
}
