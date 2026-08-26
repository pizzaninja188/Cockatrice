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
    engine.state.continuous_effects.push(ContinuousEffect {
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
fn sacrificing_another_permanent_fires_controller_observer() {
    let mut engine = GameEngine::new(156_003, &[0, 1], 20, None, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let observer = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_counter_trigger(
        &mut engine,
        observer,
        TriggerCondition::WheneverPlayerSacrificesPermanent {
            player: CastTriggerPlayer::Controller,
            exclude_self: true,
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
            exclude_self: true,
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
