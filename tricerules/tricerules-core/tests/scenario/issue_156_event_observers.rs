//! Issue #156: tapped, leaves-the-battlefield, and sacrifice observers use committed engine
//! events and preserve event-time identity.

use crate::helpers::*;
use tricerules_cards::primitives::{
    CastTriggerPlayer, ContinuousEffectKind, EffectDuration, TriggerCondition,
};
use tricerules_cards::CardRegistry;
use tricerules_core::state::ActiveDeathReplacement;
use tricerules_core::Zone;
use tricerules_core::{AffectedScope, ContinuousEffect};

fn grant_counter_trigger(engine: &mut GameEngine, source: u32, trigger: TriggerCondition) {
    let mut ability = CardRegistry::global()
        .get("ajanis_pridemate")
        .expect("Ajani's Pridemate definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = trigger;
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn attacking_fires_a_self_becomes_tapped_trigger() {
    let mut engine = GameEngine::new(156_001, &[0, 1], 20, None, true).expect("new engine");
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_counter_trigger(
        &mut engine,
        source,
        TriggerCondition::WheneverSelfBecomesTapped,
    );

    advance_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![source]))
        .expect("declare tapping attacker");

    assert_eq!(engine.state.stack.len(), 1, "tap trigger reaches the stack");
}

#[test]
fn sacrificing_a_source_fires_its_leaves_trigger() {
    let mut engine = GameEngine::new(156_002, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let relic = inject_creature_on_battlefield(&mut engine, 0, "bottle_gnomes");
    grant_counter_trigger(
        &mut engine,
        relic,
        TriggerCondition::WhenSelfLeavesBattlefield,
    );

    engine
        .apply_command(0, &activate_ability(relic, 0, vec![]))
        .expect("sacrifice Bottle Gnomes");

    assert_eq!(
        engine.state.stack.len(),
        2,
        "the leaves trigger is stacked above the activated ability"
    );
}

#[test]
fn issue_168_filtered_departure_observes_a_committed_sacrifice() {
    let mut engine = GameEngine::new(168010, &[0, 1], 20, None, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let relic = inject_creature_on_battlefield(&mut engine, 0, "bottle_gnomes");
    grant_counter_trigger(
        &mut engine,
        relic,
        TriggerCondition::WheneverPermanentLeavesBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: Default::default(),
            destination: Default::default(),
            cardinality: Default::default(),
        },
    );
    engine
        .apply_command(0, &activate_ability(relic, 0, vec![]))
        .unwrap();
    assert_eq!(
        engine.state.stack.len(),
        2,
        "the departed source still observes itself"
    );
}

#[test]
fn issue_168_soul_salvage_is_one_graveyard_departure_batch() {
    use tricerules_cards::primitives::{
        PermanentEventFilter, PermanentTypeFilter, ZoneEventCardinality,
    };
    let decks = Some(vec![
        deck_with("swamp", &["soul_salvage"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(168011, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let observer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_counter_trigger(
        &mut engine,
        observer,
        TriggerCondition::WheneverCardsLeaveGraveyard {
            owner: CastTriggerPlayer::Controller,
            filter: PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Creature),
                ..Default::default()
            },
            cardinality: ZoneEventCardinality::OneOrMore,
        },
    );
    let first = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let second = inject_graveyard_card(&mut engine, 0, "hill_giant");
    ensure_in_hand(&mut engine, 0, "soul_salvage");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 3,
            ..Default::default()
        },
    );
    let targets = [first, second]
        .into_iter()
        .map(|object_id| tricerules_proto::ruled::v1::TargetRef {
            kind: tricerules_proto::ruled::v1::TargetRefKind::Graveyard as i32,
            object_id,
            ..Default::default()
        })
        .collect();
    let slot = hand_index_for_card(&engine, 0, "soul_salvage");
    engine.apply_command(0, &cast_spell(slot, targets)).unwrap();
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "two returned cards produce one Mortipede-style trigger"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&observer]
            .counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn issue_168_reanimation_group_waits_for_all_replacement_choices() {
    use tricerules_cards::primitives::*;
    use tricerules_proto::ruled::v1::{
        ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
    };
    let mut engine = GameEngine::new(168012, &[0, 1], 20, None, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "llanowar_elves");
    let observer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let mut ability = CardRegistry::global()
        .get("ajanis_pridemate")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::WheneverSelfBecomesTapped;
    ability.effect = vec![
        SpellEffectKind::MoveGraveyardCards {
            filter: Default::default(),
            destination: GraveyardDestination::Battlefield { tapped: false },
        },
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(1),
        },
    ];
    ability.targeting = CardRegistry::global()
        .get("soul_salvage")
        .unwrap()
        .primary_face()
        .targeting
        .clone();
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: 0,
    });
    grant_counter_trigger(
        &mut engine,
        observer,
        TriggerCondition::WheneverCardsLeaveGraveyard {
            owner: CastTriggerPlayer::Controller,
            filter: Default::default(),
            cardinality: ZoneEventCardinality::OneOrMore,
        },
    );
    let first = inject_graveyard_card(&mut engine, 0, "hill_giant");
    let second = inject_graveyard_card(&mut engine, 0, "clone");
    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .unwrap();
    let targets = [first, second]
        .into_iter()
        .map(|object_id| TargetRef {
            object_id,
            kind: TargetRefKind::Graveyard as i32,
            ..Default::default()
        })
        .collect();
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets,
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    pass_both_players(&mut engine);
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(
        engine.state.objects[&first].zone,
        Zone::Graveyard,
        "the first member cannot enter before the last choice"
    );
    assert_eq!(engine.state.objects[&second].zone, Zone::Graveyard);
    let state_before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![observer]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), state_before);
    let generation = engine.state.zone_change_generation.get(&first).copied();
    engine
        .state
        .zone_change_generation
        .insert(first, generation.unwrap_or(0) + 1);
    let stale_state = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![observer]))
        .is_err());
    assert_eq!(
        format!("{:?}", engine.state),
        stale_state,
        "a stale member cannot partially commit its cohort"
    );
    if let Some(generation) = generation {
        engine
            .state
            .zone_change_generation
            .insert(first, generation);
    } else {
        engine.state.zone_change_generation.remove(&first);
    }
    engine
        .apply_command(0, &submit_resolution_choice(vec![observer]))
        .unwrap();
    assert_eq!(engine.state.objects[&first].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&second].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.players[0].life, 21,
        "the effect tail resumes exactly once after commitment"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&observer]
            .counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn sacrificing_another_permanent_fires_controller_observer() {
    let mut engine = GameEngine::new(156_003, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let observer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_counter_trigger(
        &mut engine,
        observer,
        TriggerCondition::WheneverPlayerSacrificesPermanent {
            player: CastTriggerPlayer::Controller,
            filter: tricerules_cards::primitives::PermanentEventFilter {
                exclude_source: true,
                ..Default::default()
            },
        },
    );
    let sacrifice = inject_creature_on_battlefield(&mut engine, 0, "bottle_gnomes");

    engine
        .apply_command(0, &activate_ability(sacrifice, 0, vec![]))
        .expect("sacrifice Bottle Gnomes");

    assert_eq!(
        engine.state.stack.len(),
        2,
        "the sacrifice observer is stacked above the activated ability"
    );
}

#[test]
fn another_permanent_observer_excludes_the_sacrificed_source_generation() {
    let mut engine = GameEngine::new(156_007, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "bottle_gnomes");
    grant_counter_trigger(
        &mut engine,
        source,
        TriggerCondition::WheneverPlayerSacrificesPermanent {
            player: CastTriggerPlayer::Controller,
            filter: tricerules_cards::primitives::PermanentEventFilter {
                exclude_source: true,
                ..Default::default()
            },
        },
    );

    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .expect("sacrifice the observer itself");

    assert_eq!(
        engine.state.stack.len(),
        1,
        "the sacrificed generation does not satisfy 'another permanent'"
    );
}

#[test]
fn chrome_companion_attacking_uses_its_authored_tap_trigger() {
    let mut engine = GameEngine::new(156_004, &[0, 1], 20, None, true).expect("new engine");
    let companion = inject_creature_on_battlefield(&mut engine, 0, "chrome_companion");

    advance_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![companion]))
        .expect("declare Chrome Companion as an attacker");

    assert_eq!(
        engine.state.stack.len(),
        1,
        "authored tap trigger is stacked"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 21);
}

#[test]
fn cryoshatter_remembers_the_creature_that_became_tapped() {
    let mut engine = GameEngine::new(156_005, &[0, 1], 20, None, true).expect("new engine");
    let creature = inject_creature_on_battlefield(&mut engine, 0, "colossal_dreadmaw");
    let aura = inject_permanent_on_battlefield(&mut engine, 1, "cryoshatter");
    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("Cryoshatter")
        .attached_to = Some(AttachmentRecipient::Object(creature));

    advance_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![creature]))
        .expect("declare enchanted creature as an attacker");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Cryoshatter trigger is stacked"
    );

    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("Cryoshatter")
        .attached_to = None;
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&creature].zone,
        Zone::Graveyard,
        "the trigger destroys the event-time enchanted object after the Aura detaches"
    );
}

#[test]
fn a_triggering_mana_ability_cannot_be_undone() {
    let mut engine = GameEngine::new(156_006, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let elf = inject_creature_on_battlefield(&mut engine, 0, "llanowar_elves");
    grant_counter_trigger(
        &mut engine,
        elf,
        TriggerCondition::WheneverSelfBecomesTapped,
    );

    let batch = engine
        .apply_command(0, &activate_ability(elf, 0, vec![]))
        .expect("activate Llanowar Elves");

    assert_eq!(engine.state.players[0].mana_pool.green, 1);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "tap trigger is stacked after mana is produced"
    );
    assert_eq!(batch.legal_by_player[&0].undoable_mana_abilities, 0);
    assert!(engine.state.undoable_mana_abilities.is_empty());
}

#[test]
fn sacrifice_observer_fires_when_a_replacement_exiles_the_permanent() {
    let mut engine = GameEngine::new(156_008, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let observer = inject_creature_on_battlefield(&mut engine, 0, "pirate_peddlers");
    let victim = inject_creature_on_battlefield(&mut engine, 0, "bottle_gnomes");
    engine
        .state
        .death_replacement_effects
        .push(ActiveDeathReplacement {
            object_id: victim,
            zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&victim)
                .copied()
                .unwrap_or(0),
        });

    engine
        .apply_command(0, &activate_ability(victim, 0, vec![]))
        .expect("sacrifice replaced by exile");

    assert_eq!(engine.state.objects[&victim].zone, Zone::Exile);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "the semantic sacrifice trigger is above Bottle Gnomes' ability"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&observer]
            .counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1,
    );
}

#[test]
fn cryogen_relic_leaves_trigger_fires_when_returned_to_hand() {
    let decks = Some(vec![
        deck_with("island", &["boomerang"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(156_009, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let relic = inject_permanent_on_battlefield(&mut engine, 0, "cryogen_relic");
    ensure_in_hand(&mut engine, 0, "boomerang");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "boomerang");

    engine
        .apply_command(0, &cast_spell(slot, target_object(relic)))
        .expect("cast Boomerang on Cryogen Relic");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&relic].zone, Zone::Hand);
    assert_eq!(engine.state.stack.len(), 1, "leave trigger is stacked");
    let hand_size = engine.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_size + 1);
}

fn issue_168_engine() -> GameEngine {
    let mut engine = GameEngine::new(168100, &[0, 1], 20, None, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn issue_168_cast(engine: &mut GameEngine, card: &str, targets: Vec<TargetRef>) -> u32 {
    let oid = inject_card_into_hand(engine, 0, card);
    grant_pool(engine, 0);
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|id| *id == oid)
        .unwrap();
    engine.apply_command(0, &cast_spell(slot, targets)).unwrap();
    oid
}

fn issue_168_choose(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_168_tabby_observes_simultaneous_enchantment_death_but_not_later_aura_sba() {
    let mut engine = issue_168_engine();
    let tabby = inject_creature_on_battlefield(&mut engine, 0, "warehouse_tabby");
    let enchantment = inject_creature_on_battlefield(&mut engine, 0, "appendage_amalgam");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "holy_strength");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(tabby));
    issue_168_cast(&mut engine, "wrath_of_god", vec![]);
    pass_both_players(&mut engine);
    for oid in [tabby, enchantment, aura] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Graveyard);
    }
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the simultaneous enchantment is seen; the subsequently orphaned Aura is not"
    );
    resolve_entire_stack_two_player(&mut engine);
    let rats = battlefield_token_oids(&engine, 0, "rat_b_1_1_cant_block");
    assert_eq!(rats.len(), 1);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, rats[0]),
        vec!["Can't block"]
    );
}

#[test]
fn issue_168_tabby_activation_grants_deathtouch() {
    let mut engine = issue_168_engine();
    let tabby = inject_creature_on_battlefield(&mut engine, 0, "warehouse_tabby");
    grant_pool(&mut engine, 0);
    engine
        .apply_command(0, &activate_ability(tabby, 0, vec![]))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine
        .characteristics(tabby)
        .unwrap()
        .keywords
        .contains(&tricerules_cards::Keyword::Deathtouch));
}

#[test]
fn issue_168_tracker_damages_the_sacrificing_opponent_in_three_seats() {
    let mut engine = issue_168_engine();
    let mut third = engine.state.players[1].clone();
    third.id = 2;
    third.hand.clear();
    third.library.clear();
    third.battlefield.clear();
    third.graveyard.clear();
    engine.state.players.push(third);
    inject_creature_on_battlefield(&mut engine, 0, "vengeful_tracker");
    let artifact = inject_creature_on_battlefield(&mut engine, 1, "bottle_gnomes");
    engine.apply_command(0, &pass()).unwrap();
    engine
        .apply_command(1, &activate_ability(artifact, 0, vec![]))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 2);
    while !engine.state.stack.is_empty() {
        let player = engine.state.priority_player_id();
        engine.apply_command(player, &pass()).unwrap();
    }
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        [20, 21, 20]
    );
}

#[test]
fn issue_168_rakish_crew_creates_a_functional_mercenary_and_drains_once_per_outlaw() {
    let mut engine = issue_168_engine();
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    issue_168_cast(&mut engine, "rakish_crew", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    let mercenaries = battlefield_token_oids(&engine, 0, "mercenary_r_1_1");
    assert_eq!(mercenaries.len(), 1);
    let mercenary = mercenaries[0];
    engine
        .state
        .objects
        .get_mut(&mercenary)
        .unwrap()
        .summoning_sick = false;
    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, mercenary, 0, target_object(bear)),
        )
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(3));
    // Changeling satisfies all five outlaw subtypes but is only one matching object.
    let changeling = inject_creature_on_battlefield(&mut engine, 0, "feisty_spikeling");
    issue_168_cast(&mut engine, "lightning_bolt", target_object(changeling));
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        (engine.state.players[0].life, engine.state.players[1].life),
        (21, 19)
    );
}

#[test]
fn issue_168_vial_smasher_outlaw_or_is_one_targeted_trigger_and_excludes_self() {
    let mut engine = issue_168_engine();
    issue_168_cast(&mut engine, "vial_smasher,_gleeful_grenadier", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.pending_triggers.is_empty(),
        "its own entry is excluded"
    );
    issue_168_cast(&mut engine, "feisty_spikeling", vec![]);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(0, &issue_168_choose(target_player(0)))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    engine
        .apply_command(0, &issue_168_choose(target_player(1)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 19);
}

#[test]
fn issue_168_carrot_cake_entry_and_sacrifice_each_create_then_scry() {
    let mut engine = issue_168_engine();
    let cake = issue_168_cast(&mut engine, "carrot_cake", vec![]);
    pass_both_players(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "rabbit_w_1_1").len(), 1);
    assert!(engine.state.pending_resolution.is_some());
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    grant_pool(&mut engine, 0);
    engine
        .apply_command(0, &activate_ability_for(&engine, cake, 0, vec![]))
        .unwrap();
    assert_eq!(
        engine.state.stack.len(),
        2,
        "sacrifice trigger above the gain-life activation"
    );
    pass_both_players(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "rabbit_w_1_1").len(), 2);
    assert_eq!(
        engine.state.players[0].life, 20,
        "life is not gained until the original activation resolves"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 23);
}

#[test]
fn issue_168_knightfisher_accepts_nontoken_changeling_but_not_an_unrelated_creature() {
    let mut engine = issue_168_engine();
    issue_168_cast(&mut engine, "knightfisher", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    assert!(battlefield_token_oids(&engine, 0, "fish_u_1_1").is_empty());
    issue_168_cast(&mut engine, "grizzly_bears", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    assert!(battlefield_token_oids(&engine, 0, "fish_u_1_1").is_empty());
    issue_168_cast(&mut engine, "feisty_spikeling", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(battlefield_token_oids(&engine, 0, "fish_u_1_1").len(), 1);
}

#[test]
fn issue_168_scribe_bounce_uses_old_source_and_publishes_a_current_target() {
    let mut engine = issue_168_engine();
    let scribe = inject_creature_on_battlefield(&mut engine, 0, "three_tree_scribe");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    issue_168_cast(&mut engine, "boomerang", target_object(scribe));
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&scribe].zone, Zone::Hand);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let before = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(0, &issue_168_choose(target_object(scribe)))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    engine
        .apply_command(0, &issue_168_choose(target_object(bear)))
        .unwrap();
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(3));
}

#[test]
fn issue_168_celebration_cards_count_nonland_entry_history_even_after_departure() {
    let mut engine = issue_168_engine();
    let mice = issue_168_cast(&mut engine, "armory_mice", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    let wielder = issue_168_cast(&mut engine, "gallant_pie-wielder", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    // Begin this fixture with both static abilities installed, but no entry history.
    engine.state.turn_history.current.permanents_entered.clear();
    let land = inject_card_into_hand(&mut engine, 0, "plains");
    let slot = engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == land)
        .unwrap();
    engine.apply_command(0, &play_land(slot)).unwrap();
    let bear = issue_168_cast(&mut engine, "grizzly_bears", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_toughness(mice),
        Some(1),
        "land plus one creature is not Celebration"
    );
    assert!(!engine
        .characteristics(wielder)
        .unwrap()
        .keywords
        .contains(&tricerules_cards::Keyword::DoubleStrike));
    issue_168_cast(&mut engine, "boomerang", target_object(bear));
    resolve_entire_stack_two_player(&mut engine);
    issue_168_cast(&mut engine, "hill_giant", vec![]);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_toughness(mice), Some(3));
    assert!(engine
        .characteristics(wielder)
        .unwrap()
        .keywords
        .contains(&tricerules_cards::Keyword::DoubleStrike));
}

#[test]
fn issue_168_scribe_distinguishes_death_from_sacrifice_replaced_by_exile() {
    for replaced in [false, true] {
        let mut engine = issue_168_engine();
        let scribe = inject_creature_on_battlefield(&mut engine, 0, "three_tree_scribe");
        let gnomes = inject_creature_on_battlefield(&mut engine, 0, "bottle_gnomes");
        if replaced {
            engine
                .state
                .death_replacement_effects
                .push(ActiveDeathReplacement {
                    object_id: gnomes,
                    zone_change_generation: 0,
                });
        }
        engine
            .apply_command(0, &activate_ability(gnomes, 0, vec![]))
            .unwrap();
        assert_eq!(
            engine.state.objects[&gnomes].zone,
            if replaced {
                Zone::Exile
            } else {
                Zone::Graveyard
            }
        );
        assert_eq!(engine.state.pending_triggers.len(), usize::from(replaced));
        if replaced {
            engine
                .apply_command(0, &issue_168_choose(target_object(scribe)))
                .unwrap();
        }
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.objects[&scribe]
                .counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
            u32::from(replaced)
        );
    }
}

#[test]
fn issue_168_graveyard_cast_is_observed_only_after_successful_commit() {
    let mut engine = issue_168_engine();
    let observer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_counter_trigger(
        &mut engine,
        observer,
        TriggerCondition::WheneverCardsLeaveGraveyard {
            owner: CastTriggerPlayer::Controller,
            filter: Default::default(),
            cardinality: tricerules_cards::primitives::ZoneEventCardinality::OneOrMore,
        },
    );
    let spell = inject_graveyard_card(&mut engine, 0, "bump_in_the_night");
    let cast = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: tricerules_proto::ruled::v1::CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell)),
            targets: target_player(1),
            ..Default::default()
        })),
    };
    let before = format!("{:?}", engine.state);
    assert!(
        engine.apply_command(0, &cast).is_err(),
        "cannot pay the flashback cost"
    );
    assert_eq!(format!("{:?}", engine.state), before);
    grant_pool(&mut engine, 0);
    engine.apply_command(0, &cast).unwrap();
    assert_eq!(engine.state.objects[&spell].zone, Zone::Stack);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "one departure trigger above the committed spell"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&observer]
            .counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(engine.state.objects[&spell].zone, Zone::Exile);
}
