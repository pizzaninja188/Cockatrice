//! Issue #114: recursive OR filters use the same authoritative predicates at publication,
//! submission, and resolution.

use super::helpers::*;
use tricerules_cards::primitives::{
    CardTypeFilter, ContinuousEffectKind, CounterKind, EffectDuration, GraveyardFilter,
    SpellEffectKind, TriggerCondition, TriggeredAbilityDef,
};
use tricerules_cards::CardRegistry;
use tricerules_core::state::PendingTrigger;
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};

fn issue_176_engine(cards: &[&str]) -> GameEngine {
    let mut engine = GameEngine::new(
        176_100,
        &[0, 1],
        20,
        Some(vec![deck_with("swamp", cards), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn issue_176_target(oid: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: oid,
        ..Default::default()
    }]
}

fn issue_176_enter(engine: &mut GameEngine, card: &str, targets: Vec<TargetRef>) -> u32 {
    ensure_in_hand(engine, 0, card);
    give_mana(
        engine,
        0,
        ManaGift {
            w: 10,
            u: 10,
            b: 10,
            r: 10,
            g: 10,
            c: 10,
        },
    );
    let slot = hand_index_for_card(engine, 0, card);
    let oid = engine.state.players[0].hand[slot];
    engine.apply_command(0, &cast_spell(slot, targets)).unwrap();
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    oid
}

fn issue_176_choose(mode: Option<u32>, targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: if mode.is_none() {
                targets.clone()
            } else {
                vec![]
            },
            selected_modes: mode
                .map(|mode_index| {
                    vec![SelectedSpellMode {
                        mode_index,
                        targets,
                    }]
                })
                .unwrap_or_default(),
            decline: false,
        })),
    }
}

#[test]
fn issue_176_kraul_whipcracker_targets_only_opponents_tokens() {
    let mut engine = issue_176_engine(&["kraul_whipcracker"]);
    let token = inject_permanent_on_battlefield(&mut engine, 1, "treasure");
    let own = inject_permanent_on_battlefield(&mut engine, 0, "treasure");
    let nontoken = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let source = issue_176_enter(&mut engine, "kraul_whipcracker", vec![]);
    let batch = engine.initial_response_batch();
    assert_eq!(
        batch.legal_by_player[&0].valid_targets_by_ability[&(u64::from(source) << 32)].groups[0]
            .valid_permanent_ids,
        vec![token]
    );
    for illegal in [own, nontoken] {
        assert!(engine
            .apply_command(0, &issue_176_choose(None, issue_176_target(illegal)))
            .is_err());
    }
    engine
        .apply_command(0, &issue_176_choose(None, issue_176_target(token)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert!(!engine.state.objects.contains_key(&token));
    assert!(engine.state.objects.contains_key(&own));
}

#[test]
fn issue_176_due_diligence_rechecks_enchanted_creature_on_resolution() {
    for retarget in [false, true] {
        let mut engine = issue_176_engine(&["due_diligence"]);
        let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let aura = issue_176_enter(&mut engine, "due_diligence", issue_176_target(first));
        assert_eq!(engine.effective_power(first), Some(4));
        assert!(engine
            .apply_command(0, &issue_176_choose(None, issue_176_target(first)))
            .is_err());
        engine
            .apply_command(0, &issue_176_choose(None, issue_176_target(second)))
            .unwrap();
        if retarget {
            engine.state.objects.get_mut(&aura).unwrap().attached_to =
                Some(AttachmentRecipient::Object(second));
        }
        let before = engine.effective_power(second).unwrap();
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.effective_power(second),
            Some(before + if retarget { 0 } else { 2 })
        );
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn issue_176_hivespine_wolverine_modes_use_their_own_filters() {
    for mode in 0..3 {
        let mut engine = issue_176_engine(&["hivespine_wolverine"]);
        let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        let token = inject_permanent_on_battlefield(&mut engine, 1, "mercenary_r_1_1");
        let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
        let wolverine = issue_176_enter(&mut engine, "hivespine_wolverine", vec![]);
        if mode == 1 {
            assert!(engine
                .apply_command(0, &issue_176_choose(Some(mode), issue_176_target(creature)))
                .is_err());
        }
        let chosen = [creature, token, artifact][mode as usize];
        engine
            .apply_command(0, &issue_176_choose(Some(mode), issue_176_target(chosen)))
            .unwrap();
        resolve_entire_stack_two_player(&mut engine);
        match mode {
            0 => assert_eq!(
                engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
                1
            ),
            1 => {
                assert!(!engine.state.objects.contains_key(&token));
                assert_eq!(engine.state.objects[&wolverine].damage, 1);
            }
            _ => assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard),
        }
    }
}

#[test]
fn issue_176_rooftop_and_downwind_use_positive_damage_history() {
    for (card, mode) in [
        ("rooftop_assassin", None),
        ("downwind_ambusher", Some(1)),
        ("downwind_ambusher", Some(0)),
    ] {
        let mut engine = issue_176_engine(&[card, "shock"]);
        let damaged = inject_permanent_on_battlefield(&mut engine, 1, "serra_angel");
        let untouched = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        ensure_in_hand(&mut engine, 0, "shock");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "shock");
        engine
            .apply_command(0, &cast_spell(slot, issue_176_target(damaged)))
            .unwrap();
        resolve_entire_stack_two_player(&mut engine);
        // History is independent of remaining marked damage.
        engine.state.objects.get_mut(&damaged).unwrap().damage = 0;
        issue_176_enter(&mut engine, card, vec![]);
        if mode != Some(0) {
            assert!(engine
                .apply_command(0, &issue_176_choose(mode, issue_176_target(untouched)))
                .is_err());
        }
        let chosen = if mode == Some(0) { untouched } else { damaged };
        engine
            .apply_command(0, &issue_176_choose(mode, issue_176_target(chosen)))
            .unwrap();
        resolve_entire_stack_two_player(&mut engine);
        if mode == Some(0) {
            assert_eq!(engine.effective_power(untouched), Some(1));
            assert_eq!(engine.effective_toughness(untouched), Some(1));
        } else {
            assert_eq!(engine.state.objects[&damaged].zone, Zone::Graveyard);
        }
    }
}

#[test]
fn issue_176_hoarding_recluse_excludes_its_death_incarnation() {
    for skip in [false, true] {
        let mut engine = issue_176_engine(&["hoarding_recluse", "murder"]);
        let recluse = inject_creature_on_battlefield(&mut engine, 0, "hoarding_recluse");
        let other = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        engine.state.players[1]
            .battlefield
            .retain(|oid| *oid != other);
        engine.state.players[1].graveyard.push(other);
        engine.state.objects.get_mut(&other).unwrap().zone = Zone::Graveyard;
        ensure_in_hand(&mut engine, 0, "murder");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "murder");
        engine
            .apply_command(0, &cast_spell(slot, issue_176_target(recluse)))
            .unwrap();
        pass_both_players(&mut engine);
        let batch = engine.initial_response_batch();
        let candidates = &batch.legal_by_player[&0].valid_targets_by_ability
            [&(u64::from(recluse) << 32)]
            .groups[0]
            .valid_graveyard_ids;
        assert!(candidates.contains(&other));
        assert!(!candidates.contains(&recluse));
        let grave = |oid| {
            vec![TargetRef {
                kind: TargetRefKind::Graveyard as i32,
                object_id: oid,
                ..Default::default()
            }]
        };
        assert!(engine
            .apply_command(0, &issue_176_choose(None, grave(recluse)))
            .is_err());
        engine
            .apply_command(
                0,
                &issue_176_choose(None, if skip { vec![] } else { grave(other) }),
            )
            .unwrap();
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.objects[&recluse].zone, Zone::Graveyard);
        assert_eq!(
            engine.state.objects[&other].zone,
            if skip { Zone::Graveyard } else { Zone::Library }
        );
        if !skip {
            assert_eq!(engine.state.players[1].library.back(), Some(&other));
        }
    }
}

#[test]
fn issue_176_druid_counts_any_owned_token_and_tycoon_can_sacrifice_it() {
    let mut engine = issue_176_engine(&["druid_of_the_spade", "prosperity_tycoon"]);
    let druid = issue_176_enter(&mut engine, "druid_of_the_spade", vec![]);
    assert_eq!(engine.effective_power(druid), Some(2));
    let opponent_token = inject_permanent_on_battlefield(&mut engine, 1, "treasure");
    assert_eq!(engine.effective_power(druid), Some(2));
    let tycoon = issue_176_enter(&mut engine, "prosperity_tycoon", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    let mercenaries = battlefield_token_oids(&engine, 0, "mercenary_r_1_1");
    assert_eq!(mercenaries.len(), 1);
    assert_eq!(engine.effective_power(druid), Some(4));
    assert!(engine.effective_has_keyword(druid, tricerules_cards::Keyword::Trample));
    // Already tapped sources can activate: tapping is an effect, not a cost.
    engine.state.objects.get_mut(&tycoon).unwrap().tapped = true;
    let command = |oid| {
        let mut cmd = activate_ability_for(&engine, tycoon, 0, vec![]);
        let Some(Cmd::ActivateAbility(ability)) = cmd.cmd.as_mut() else {
            unreachable!()
        };
        ability.cost_selections = vec![permanent_cost_selection(1, oid)];
        cmd
    };
    let wrong_owner = command(opponent_token);
    let nontoken = command(druid);
    let valid = command(mercenaries[0]);
    assert!(engine.apply_command(0, &wrong_owner).is_err());
    assert!(engine.apply_command(0, &nontoken).is_err());
    engine.apply_command(0, &valid).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.effective_has_keyword(tycoon, tricerules_cards::Keyword::Indestructible));
    assert_eq!(engine.effective_power(druid), Some(2));
    let food = inject_permanent_on_battlefield(&mut engine, 0, "food");
    assert_eq!(engine.effective_power(druid), Some(4));
    apply_ability(&mut engine, 0, food, 0, vec![]).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 23);
    assert_eq!(engine.effective_power(druid), Some(2));
}

#[test]
fn issue_176_feed_checks_resolution_turn_and_sole_target_legality() {
    for (opponent_turn, leaves) in [(true, false), (false, false), (false, true)] {
        let mut engine = issue_176_engine(&["feed_the_cauldron"]);
        ensure_in_hand(&mut engine, 0, "feed_the_cauldron");
        let target = inject_permanent_on_battlefield(&mut engine, 1, "darksteel_myr");
        if opponent_turn {
            engine.state.active_player_idx = 1;
        }
        give_mana(
            &mut engine,
            0,
            ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, "feed_the_cauldron");
        engine
            .apply_command(0, &cast_spell(slot, issue_176_target(target)))
            .unwrap();
        if leaves {
            engine.state.players[1]
                .battlefield
                .retain(|oid| *oid != target);
            engine.state.players[1].hand.push(target);
            engine.state.objects.get_mut(&target).unwrap().zone = Zone::Hand;
            *engine
                .state
                .zone_change_generation
                .entry(target)
                .or_default() += 1;
        }
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            battlefield_token_oids(&engine, 0, "food").len(),
            usize::from(!opponent_turn && !leaves)
        );
        assert_eq!(
            engine.state.objects[&target].zone,
            if leaves {
                Zone::Hand
            } else {
                Zone::Battlefield
            },
            "indestructible does not make the target illegal"
        );
    }
}

#[test]
fn issue_176_feed_the_cauldron_checks_mana_value_and_creates_food() {
    let mut engine = issue_176_engine(&["feed_the_cauldron"]);
    ensure_in_hand(&mut engine, 0, "feed_the_cauldron");
    let small = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let large = inject_creature_on_battlefield(&mut engine, 1, "serra_angel");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(
        published_hand_targets(&mut engine, 0, "feed_the_cauldron"),
        vec![small]
    );
    let slot = hand_index_for_card(&engine, 0, "feed_the_cauldron");
    let index = engine.state.command_index;
    assert!(engine
        .apply_command(0, &cast_spell(slot, issue_176_target(large)))
        .is_err());
    assert_eq!(engine.state.command_index, index);
    engine
        .apply_command(0, &cast_spell(slot, issue_176_target(small)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&small].zone, Zone::Graveyard);
    assert!(engine
        .state
        .objects
        .values()
        .any(|object| object.zone == Zone::Battlefield
            && object.is_token()
            && object.controller == 0));
}

#[test]
fn issue_176_haywire_mite_exiles_noncreatures_and_gains_life() {
    let mut engine = issue_176_engine(&["haywire_mite"]);
    let mite = inject_creature_on_battlefield(&mut engine, 0, "haywire_mite");
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "ornithopter");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    assert!(apply_ability(&mut engine, 0, mite, 0, issue_176_target(creature)).is_err());
    assert_eq!(engine.state.objects[&mite].zone, Zone::Battlefield);
    apply_ability(&mut engine, 0, mite, 0, issue_176_target(artifact)).unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&mite].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 22);
}

fn published_hand_targets(engine: &mut GameEngine, player: i32, card_id: &str) -> Vec<u32> {
    let slot = hand_index_for_card(engine, player as usize, card_id);
    engine.initial_response_batch().legal_by_player[&player].valid_targets_by_hand_slot
        [&((slot as u32) << 8)]
        .groups[0]
        .valid_permanent_ids
        .clone()
}

fn graveyard_fixture_ability(filter: GraveyardFilter) -> TriggeredAbilityDef {
    let mut ability = CardRegistry::global()
        .get("gravedigger")
        .expect("Gravedigger definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::WhenSelfEntersBattlefield;
    let SpellEffectKind::MoveGraveyardCards {
        filter: ability_filter,
        ..
    } = &mut ability.effect[0]
    else {
        panic!("Gravedigger fixture must return a graveyard card")
    };
    *ability_filter = filter;
    ability
}

fn publish_graveyard_fixture(engine: &mut GameEngine, filter: GraveyardFilter) -> (u32, Vec<u32>) {
    let source = inject_creature_on_battlefield(engine, 0, "grizzly_bears");
    let ability = graveyard_fixture_ability(filter);
    let trigger_id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    engine.state.pending_triggers.push_back(PendingTrigger {
        object_id: trigger_id,
        source_permanent_id: source,
        source_face_index: 0,
        source_zone_change: engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0),
        source_face_change: 0,
        ability_index: 0,
        ability: ability.clone(),
        ability_text: ability.text.clone(),
        card_id: "grizzly_bears".into(),
        controller: 0,
        may: ability.may,
        trigger_context: Default::default(),
    });
    let key = u64::from(source) << 32;
    let candidates = engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
        [&key]
        .groups[0]
        .valid_graveyard_ids
        .clone();
    (source, candidates)
}

#[test]
fn cards_publish_the_exact_union_of_recursive_branches_without_duplicates() {
    let decks = Some(vec![
        deck_with("plains", &["make_your_move", "broken_wings"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(114_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "glorious_anthem");
    let power_four = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&power_four)
        .expect("power target")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    let derived_flyer = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(derived_flyer),
        kind: ContinuousEffectKind::Layer6AddKeyword(tricerules_cards::primitives::Keyword::Flying),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    let overlapping_artifact_flyer = inject_creature_on_battlefield(&mut engine, 1, "ornithopter");
    let ground_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    ensure_in_hand(&mut engine, 0, "make_your_move");
    ensure_in_hand(&mut engine, 0, "broken_wings");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            g: 1,
            c: 4,
            ..Default::default()
        },
    );

    assert_eq!(
        published_hand_targets(&mut engine, 0, "make_your_move"),
        vec![
            artifact,
            enchantment,
            power_four,
            overlapping_artifact_flyer
        ]
    );
    assert_eq!(
        published_hand_targets(&mut engine, 0, "broken_wings"),
        vec![
            artifact,
            enchantment,
            derived_flyer,
            overlapping_artifact_flyer
        ]
    );
    assert_ne!(ground_creature, land);
}

#[test]
fn forged_nonmatching_target_is_rejected_before_mana_or_card_movement() {
    let decks = Some(vec![
        deck_with("plains", &["make_your_move"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(114_002, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let illegal = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "make_your_move");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "make_your_move");
    let hand_before = engine.state.players[0].hand.clone();
    let mana_before = engine.state.players[0].mana_pool;

    assert!(engine
        .apply_command(0, &cast_spell(slot, target_object(illegal)))
        .is_err());
    assert_eq!(engine.state.players[0].hand, hand_before);
    let mana_after = engine.state.players[0].mana_pool;
    assert_eq!(
        (
            mana_after.white,
            mana_after.blue,
            mana_after.black,
            mana_after.red,
            mana_after.green,
            mana_after.colorless,
        ),
        (
            mana_before.white,
            mana_before.blue,
            mana_before.black,
            mana_before.red,
            mana_before.green,
            mana_before.colorless,
        )
    );
    assert_eq!(engine.state.objects[&illegal].zone, Zone::Battlefield);
}

#[test]
fn power_branch_is_revalidated_against_current_derived_power() {
    let decks = Some(vec![
        deck_with("plains", &["make_your_move"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(114_003, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    ensure_in_hand(&mut engine, 0, "make_your_move");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "make_your_move");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast at a derived power-four creature");
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .counters
        .clear();

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&target].zone,
        Zone::Battlefield,
        "CR 608.2b: the now-illegal sole target makes the spell fail to resolve"
    );
}

#[test]
fn graveyard_or_and_exclusion_fixtures_publish_and_revalidate_exact_candidates() {
    let mut say_engine =
        GameEngine::new(114_004, &[0, 1], 20, None, true).expect("Say Its Name fixture");
    advance_to_main1_from_game_start(&mut say_engine);
    let creature = inject_graveyard_card(&mut say_engine, 0, "grizzly_bears");
    let land = inject_graveyard_card(&mut say_engine, 0, "forest");
    inject_graveyard_card(&mut say_engine, 0, "short_sword");
    let (_, candidates) = publish_graveyard_fixture(
        &mut say_engine,
        GraveyardFilter {
            any_of: Some(vec![
                GraveyardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    ..Default::default()
                },
                GraveyardFilter {
                    card_type: Some(CardTypeFilter::Land),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        },
    );
    assert_eq!(candidates, vec![creature, land]);

    let mut messenger_engine =
        GameEngine::new(114_005, &[0, 1], 20, None, true).expect("Messenger fixture");
    advance_to_main1_from_game_start(&mut messenger_engine);
    inject_graveyard_card(&mut messenger_engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut messenger_engine, 0, "forest");
    let artifact = inject_graveyard_card(&mut messenger_engine, 0, "short_sword");
    let instant = inject_graveyard_card(&mut messenger_engine, 0, "lightning_bolt");
    let (_, candidates) = publish_graveyard_fixture(
        &mut messenger_engine,
        GraveyardFilter {
            card_type: Some(CardTypeFilter::Noncreature),
            excluded_card_types: vec![CardTypeFilter::Land],
            ..Default::default()
        },
    );
    assert_eq!(candidates, vec![artifact, instant]);

    messenger_engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(instant),
                })),
            },
        )
        .expect("choose noncreature, nonland graveyard card");
    *messenger_engine
        .state
        .zone_change_generation
        .entry(instant)
        .or_default() += 1;
    pass_both_players(&mut messenger_engine);
    assert_eq!(
        messenger_engine.state.objects[&instant].zone,
        Zone::Graveyard
    );
}

#[test]
fn battlefield_target_generation_is_revalidated_through_the_recursive_filter() {
    let decks = Some(vec![
        deck_with("forest", &["broken_wings"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(114_006, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "wind_drake");
    ensure_in_hand(&mut engine, 0, "broken_wings");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "broken_wings");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Broken Wings");
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 2;

    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
}
