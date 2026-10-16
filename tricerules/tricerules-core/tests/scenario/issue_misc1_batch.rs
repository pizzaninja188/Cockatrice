//! Reviewed activated-ability and combat-trick scenarios: Riverguard's Reflexes, Exorcise, Rock
//! Soldiers, Aetherize and Intrepid Tenderfoot.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 115 (targets),
//! 508 (attacking), 602/605 (activated abilities and timing), 611.2c/613 layer 7c, 122.1
//! (counters), and 701.13 (exile).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{DeclareAttackers, TargetRefKind};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn combat_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_declare_attackers(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast_and_resolve(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

fn choose_trigger_target(object_id: u32, kind: TargetRefKind) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: kind as i32,
                ..Default::default()
            }],
        })),
    }
}

#[test]
fn issue_misc1_riverguards_reflexes() {
    let mut e = engine(725_001);
    let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    e.state.objects.get_mut(&creature).expect("object").tapped = true;
    cast_and_resolve(&mut e, "riverguards_reflexes", target_object(creature));
    assert_eq!(e.effective_power(creature), Some(4));
    assert_eq!(e.effective_toughness(creature), Some(4));
    assert!(e.effective_has_keyword(creature, tricerules_cards::Keyword::FirstStrike));
    assert!(!e.state.objects[&creature].tapped, "the target is untapped");
}

#[test]
fn issue_misc1_exorcise() {
    // An artifact is exiled.
    let mut artifact = engine(725_002);
    let boots = inject_permanent_on_battlefield(&mut artifact, 1, "swiftfoot_boots");
    cast_and_resolve(&mut artifact, "exorcise", target_object(boots));
    assert_eq!(artifact.state.objects[&boots].zone, Zone::Exile);

    // A creature with power 4 or greater is exiled.
    let mut big = engine(725_012);
    let angel = inject_creature_with_stats(&mut big, 1, "serra_angel", 4, 4);
    cast_and_resolve(&mut big, "exorcise", target_object(angel));
    assert_eq!(big.state.objects[&angel].zone, Zone::Exile);

    // A noncreature enchantment is exiled.
    let mut enchantment = engine(725_032);
    let light = inject_permanent_on_battlefield(&mut enchantment, 1, "banishing_light");
    cast_and_resolve(&mut enchantment, "exorcise", target_object(light));
    assert_eq!(enchantment.state.objects[&light].zone, Zone::Exile);

    // A small nonartifact, nonenchantment creature is not a legal target.
    let mut small = engine(725_022);
    let bear = inject_creature_with_stats(&mut small, 1, "grizzly_bears", 2, 2);
    inject_card_into_hand(&mut small, 0, "exorcise");
    let slot = hand_index_for_card(&small, 0, "exorcise");
    assert!(
        small
            .apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "a 2/2 nonartifact nonenchantment is not a legal target"
    );
}

#[test]
fn issue_misc1_rock_soldiers() {
    // The entry trigger destroys a noncreature artifact.
    let mut e = engine(725_003);
    let boots = inject_permanent_on_battlefield(&mut e, 1, "swiftfoot_boots");
    inject_card_into_hand(&mut e, 0, "rock_soldiers");
    let slot = hand_index_for_card(&e, 0, "rock_soldiers");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    pass_both_players(&mut e);
    assert_eq!(e.state.pending_triggers.len(), 1, "the entry trigger waits");
    e.apply_scenario_command(0, &choose_trigger_target(boots, TargetRefKind::Permanent))
        .expect("choose the artifact");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&boots].zone, Zone::Graveyard);

    // An artifact creature is not a legal target.
    let mut creature = engine(725_013);
    let juggernaut = inject_creature_on_battlefield(&mut creature, 1, "juggernaut");
    inject_card_into_hand(&mut creature, 0, "rock_soldiers");
    let slot = hand_index_for_card(&creature, 0, "rock_soldiers");
    semantic::accepted(&mut creature, 0, &cast_spell(slot, vec![]));
    pass_both_players(&mut creature);
    assert_eq!(creature.state.pending_triggers.len(), 1);
    assert!(
        creature
            .apply_scenario_command(
                0,
                &choose_trigger_target(juggernaut, TargetRefKind::Permanent)
            )
            .is_err(),
        "an artifact creature is excluded"
    );

    // Choosing no target is legal and does not stall.
    let mut decline = engine(725_023);
    inject_card_into_hand(&mut decline, 0, "rock_soldiers");
    let slot = hand_index_for_card(&decline, 0, "rock_soldiers");
    semantic::accepted(&mut decline, 0, &cast_spell(slot, vec![]));
    pass_both_players(&mut decline);
    assert_eq!(decline.state.pending_triggers.len(), 1);
    let no_target = RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: Vec::new(),
        })),
    };
    decline
        .apply_scenario_command(0, &no_target)
        .expect("choose no target");
    resolve_entire_stack_two_player(&mut decline);
}

#[test]
fn issue_misc1_aetherize() {
    let mut e = combat_engine(725_004);
    let attacker_a = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let attacker_b = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let idle = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker_a, attacker_b]))
        .expect("declare attackers");
    e.apply_scenario_command(0, &pass())
        .expect("pass to the defender");
    grant_pool(&mut e, 1);
    inject_card_into_hand(&mut e, 1, "aetherize");
    let slot = hand_index_for_card(&e, 1, "aetherize");
    semantic::accepted(&mut e, 1, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.players[0].hand.contains(&attacker_a)
            && e.state.players[0].hand.contains(&attacker_b),
        "both attackers returned to their owner's hand"
    );
    assert_eq!(
        e.state.objects[&idle].zone,
        Zone::Battlefield,
        "a nonattacking creature stays"
    );
}

#[test]
fn issue_misc1_aetherize_returns_attackers_aimed_at_a_different_defender() {
    let deck = deck_with("island", &[]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        725_014,
        &[20, 30, 10],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .expect("three-player combat fixture with P20 active, P30 defending, and P10 casting");
    advance_to_main1_from_game_start(&mut e);
    // Start this focused combat fixture at the declaration boundary without passing through the
    // first player's cleanup step; combat behavior itself still goes through engine validation.
    e.state.turn_step = TurnStep::DeclareAttackers;

    let attacker_a = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let attacker_b = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let idle = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let legal = e.initial_response_batch().legal_by_player[&20]
        .legal_attack_assignments
        .clone();
    let assignments = [attacker_a, attacker_b]
        .into_iter()
        .map(|attacker| {
            legal
                .iter()
                .find(|assignment| {
                    assignment.attacker_object_id == attacker
                        && assignment.defending_player_id == 30
                })
                .cloned()
                .expect("P20 may attack P30 independently of P10")
        })
        .collect::<Vec<_>>();
    e.apply_scenario_command(
        20,
        &RuledCommand {
            cmd: Some(Cmd::DeclareAttackers(DeclareAttackers { assignments })),
        },
    )
    .expect("attack P30, not the Aetherize caster P10");
    assert_eq!(e.state.priority_player_id(), 20);
    e.apply_scenario_command(20, &pass())
        .expect("active player passes");
    e.apply_scenario_command(30, &pass())
        .expect("the attacked defender passes");
    assert_eq!(e.state.priority_player_id(), 10);

    inject_card_into_hand(&mut e, 2, "aetherize");
    e.state.players[2].mana_pool.blue = 1;
    e.state.players[2].mana_pool.colorless = 3;
    let slot = hand_index_for_card(&e, 2, "aetherize");
    semantic::accepted(&mut e, 10, &cast_spell(slot, vec![]));
    while !e.state.stack.is_empty() {
        pass_priority_round(&mut e);
    }

    for (attacker, owner_index) in [(attacker_a, 0), (attacker_b, 0)] {
        assert_eq!(e.state.objects[&attacker].zone, Zone::Hand);
        assert!(e.state.players[owner_index].hand.contains(&attacker));
    }
    assert_eq!(e.state.objects[&idle].zone, Zone::Battlefield);
}

#[test]
fn issue_misc1_intrepid_tenderfoot() {
    let mut e = engine(725_005);
    let tenderfoot = inject_creature_on_battlefield(&mut e, 0, "intrepid_tenderfoot");
    grant_pool(&mut e, 0);
    let counters_before = e.state.objects[&tenderfoot]
        .counter_count(tricerules_cards::primitives::CounterKind::PlusOnePlusOne);
    semantic::accepted(&mut e, 0, &activate_ability(tenderfoot, 0, vec![]));
    resolve_entire_stack_two_player(&mut e);
    let counters_after = e.state.objects[&tenderfoot]
        .counter_count(tricerules_cards::primitives::CounterKind::PlusOnePlusOne);
    assert_eq!(
        counters_after,
        counters_before + 1,
        "the activated ability adds one +1/+1 counter"
    );
}
