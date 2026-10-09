//! Wash Out chooses a color during resolution, then simultaneously returns that cohort.
use super::helpers::*;
use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::{Color, ContinuousEffectKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine, Zone};
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision, SubmitResolutionChoice};

fn branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn setup() -> GameEngine {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_801,
        &[10, 20, 30],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast(engine: &mut GameEngine) -> RuledEventBatch {
    inject_card_into_hand(engine, 0, "wash_out");
    give_mana(
        engine,
        10,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "wash_out");
    engine.apply_command(10, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].card_id, "wash_out");
    assert!(engine.state.stack[0].targets.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let mut batch = RuledEventBatch::default();
    for _ in 0..engine.state.players.len() {
        batch = engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    batch
}

fn modify(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn wash_out_each_color_returns_only_matching_permanents_across_all_players() {
    let cards = [
        "serra_angel",
        "merfolk_of_the_pearl_trident",
        "scathe_zombies",
        "hill_giant",
        "grizzly_bears",
    ];
    for selected in 0..5 {
        let mut engine = setup();
        let objects = cards
            .into_iter()
            .enumerate()
            .map(|(index, card)| inject_creature_on_battlefield(&mut engine, index % 3, card))
            .collect::<Vec<_>>();
        let colorless = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
        cast(&mut engine);
        engine.apply_command(10, &branch(selected as u32)).unwrap();
        for (index, oid) in objects.into_iter().enumerate() {
            assert_eq!(
                engine.state.objects[&oid].zone,
                if index == selected {
                    Zone::Hand
                } else {
                    Zone::Battlefield
                }
            );
            if index == selected {
                assert!(engine.state.players[index % 3].hand.contains(&oid));
            }
        }
        assert_eq!(engine.state.objects[&colorless].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn wash_out_uses_current_colors_and_returns_untargetable_borrowed_and_token_cohort() {
    let mut engine = setup();
    let own = inject_creature_on_battlefield(&mut engine, 0, "merfolk_of_the_pearl_trident");
    let protected = inject_creature_on_battlefield(&mut engine, 1, "merfolk_of_the_pearl_trident");
    let borrowed = inject_creature_on_battlefield(&mut engine, 0, "merfolk_of_the_pearl_trident");
    engine.state.objects.get_mut(&borrowed).unwrap().owner = 30;
    let multicolor = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
    let became_red = inject_creature_on_battlefield(&mut engine, 1, "merfolk_of_the_pearl_trident");
    let green = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let colorless = inject_permanent_on_battlefield(&mut engine, 1, "island");
    let token = inject_creature_on_battlefield(&mut engine, 2, "soldier_w_1_1");
    for kind in [
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Hexproof),
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Shroud),
        ContinuousEffectKind::Layer6AddProtection(ProtectionQuality::Color(Color::Blue)),
    ] {
        modify(&mut engine, protected, kind);
    }
    cast(&mut engine);
    // Color is read when the selected instruction executes, not when the spell is cast.
    modify(
        &mut engine,
        multicolor,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue, Color::Green]),
    );
    modify(
        &mut engine,
        became_red,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
    );
    modify(
        &mut engine,
        token,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
    );
    let batch = engine.apply_command(10, &branch(1)).unwrap();
    for (oid, owner) in [(own, 10), (protected, 20), (borrowed, 30), (multicolor, 30)] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
        assert!(engine
            .state
            .players
            .iter()
            .find(|p| p.id == owner)
            .unwrap()
            .hand
            .contains(&oid));
        assert!(batch.events.iter().any(|event| matches!(&event.ev,
            Some(Ev::PermanentMoved(moved)) if moved.object_id == oid && moved.controller_player_id == owner
        )));
    }
    assert!(!engine.state.players[0].hand.contains(&borrowed));
    for oid in [became_red, green, colorless] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    }
    assert!(engine
        .state
        .objects
        .get(&token)
        .is_none_or(|object| object.zone != Zone::Hand));
    assert!(!engine
        .state
        .players
        .iter()
        .any(|player| player.hand.contains(&token)));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn wash_out_snapshots_the_color_cohort_before_any_departure_changes_colors() {
    let mut engine = setup();
    let source = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let affected = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    modify(
        &mut engine,
        source,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
    );
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(source),
        affected: AffectedScope::Single(affected),
        kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    cast(&mut engine);
    engine.apply_command(10, &branch(1)).unwrap();
    for (index, oid) in [(0, source), (1, affected)] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
        assert!(engine.state.players[index].hand.contains(&oid));
    }
    assert!(engine.state.stack.is_empty());
}

#[test]
fn wash_out_paid_cast_and_color_choice_replay_deterministically() {
    let play = || {
        let mut engine = setup();
        inject_creature_on_battlefield(&mut engine, 1, "merfolk_of_the_pearl_trident");
        inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
        let first = cast(&mut engine);
        let second = engine.apply_command(10, &branch(1)).unwrap();
        assert!(engine.state.stack.is_empty());
        // GameState has tuple-keyed LKI maps, so JSON cannot serialize it as an object.
        // Compare public responses plus every player/object and the identity/history state.
        let objects = engine
            .state
            .objects
            .iter()
            .map(|(&oid, object)| (oid, serde_json::to_value(object).unwrap()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let generations = engine
            .state
            .zone_change_generation
            .iter()
            .map(|(&oid, &generation)| (oid, generation))
            .collect::<std::collections::BTreeMap<_, _>>();
        (
            first,
            second,
            engine.initial_response_batch(),
            serde_json::to_value(&engine.state.players).unwrap(),
            objects,
            generations,
            engine.state.command_index,
            engine.state.turn_history.clone(),
        )
    };
    assert_eq!(play(), play());
}

#[test]
fn wash_out_paid_cast_offers_all_five_colors_even_when_no_permanents_match() {
    let mut engine = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 1, "island");
    let batch = cast(&mut engine);
    let choice = find_resolution_choice(&batch).unwrap();
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(choice.deciding_player_id, 10);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(choice.resolution_branches.len(), 5);
    for (index, color) in ["white", "blue", "black", "red", "green"]
        .into_iter()
        .enumerate()
    {
        let option = &choice.resolution_branches[index];
        assert_eq!(option.branch_index, index as u32);
        assert!(option.selectable);
        assert_eq!(
            option.label,
            format!("Return all {color} permanents to their owners' hands.")
        );
    }
    for (actor, command) in [
        (20, branch(0)),
        (10, branch(5)),
        (
            10,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    decision: ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        ),
    ] {
        let before = format!("{:?}", engine.state);
        assert!(engine.apply_command(actor, &command).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
    }
    engine.apply_command(10, &branch(0)).unwrap();
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
