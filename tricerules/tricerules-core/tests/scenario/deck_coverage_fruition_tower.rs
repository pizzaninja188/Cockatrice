//! Reviewed actual-card reuse of cast/upkeep triggers and resolving amounts.
use super::helpers::*;
use tricerules_cards::{AbilityPresentation, Color, Layout};
use tricerules_core::{GameEngine, TurnStep, Zone};

fn pass_all_players(engine: &mut GameEngine) {
    let count = engine.state.players.iter().filter(|p| !p.has_lost).count();
    for _ in 0..count {
        answer_trigger_order_in_engine_order(engine);
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("pass priority");
    }
}

fn set_hand_size(engine: &mut GameEngine, player: usize, count: usize) {
    while engine.state.players[player].hand.len() > count {
        let object = engine.state.players[player].hand.pop().unwrap();
        engine.state.objects.get_mut(&object).unwrap().zone = Zone::Graveyard;
        engine.state.players[player].graveyard.push(object);
    }
    while engine.state.players[player].hand.len() < count {
        inject_card_into_hand(engine, player, "forest");
    }
}

#[test]
fn exact_characteristics_and_presentation() {
    let registry = tricerules_cards::registry::global();
    for (id, name, cost, types, colors) in [
        (
            "forced_fruition",
            "Forced Fruition",
            "{4}{U}{U}",
            vec!["Enchantment"],
            vec![Color::Blue],
        ),
        (
            "ivory_tower",
            "Ivory Tower",
            "{1}",
            vec!["Artifact"],
            vec![],
        ),
    ] {
        let card = registry.get(id).expect("actual card registered");
        assert_eq!(card.id, id);
        assert_eq!(card.name, name);
        assert_eq!(card.layout, Layout::Normal);
        assert_eq!(card.face_count(), 1);
        let face = card.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.name, name);
        assert_eq!(face.mana_cost.to_string(), cost);
        assert_eq!(face.types, types);
        assert_eq!(face.colors(), colors);
        assert_eq!(face.power, None);
        assert_eq!(face.toughness, None);
        assert!(face.keywords.is_empty());
        assert!(face.static_abilities.is_empty());
        assert!(face.activated_abilities.is_empty());
        assert_eq!(face.triggered_abilities.len(), 1);
        assert_eq!(
            face.triggered_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
    }
}

#[test]
fn ivory_tower_uses_live_controller_hand_and_clamps_below_four() {
    for (at_trigger, at_resolution, gain) in [(7, 6, 2), (3, 7, 3), (7, 4, 0), (7, 2, 0)] {
        let decks = Some(vec![island_only_deck(), island_only_deck()]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            202_609_310,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut engine);
        let source = inject_permanent_on_battlefield(&mut engine, 1, "ivory_tower");
        let generation = engine.state.zone_change_generation.get(&source).copied();
        set_hand_size(&mut engine, 1, at_trigger);
        set_hand_size(&mut engine, 0, 7);
        end_active_turn(&mut engine, 0);
        assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
        assert_eq!(engine.state.stack.len(), 1, "no intervening hand threshold");
        assert_eq!(
            engine.state.players[1].life, 20,
            "trigger does not gain life early"
        );
        set_hand_size(&mut engine, 1, at_resolution);
        let hands: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|p| p.hand.clone())
            .collect();
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.players[1].life, 20 + gain);
        assert_eq!(engine.state.players[0].life, 20);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.zone_change_generation.get(&source).copied(),
            generation
        );
        assert_eq!(
            engine
                .state
                .players
                .iter()
                .map(|p| p.hand.clone())
                .collect::<Vec<_>>(),
            hands
        );
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn ivory_tower_does_not_trigger_on_opponent_upkeep() {
    let decks = Some(vec![island_only_deck(), island_only_deck()]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_609_311,
        &[0, 1],
        20,
        decks,
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "ivory_tower");
    set_hand_size(&mut engine, 0, 7);
    end_active_turn(&mut engine, 0);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].life, 20);
}

#[test]
fn forced_fruition_draws_seven_for_each_opponent_before_their_spell_resolves() {
    let decks = Some(vec![
        island_only_deck(),
        island_only_deck(),
        island_only_deck(),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        202_609_312,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .unwrap();
    pass_all_players(&mut engine);
    pass_all_players(&mut engine);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "forced_fruition");
    for caster in 0..3 {
        if caster != 0 {
            end_active_turn(&mut engine, caster - 1);
            pass_all_players(&mut engine);
            pass_all_players(&mut engine);
        }
        assert_eq!(engine.state.active_player_id(), caster);
        assert_eq!(engine.state.turn_step, TurnStep::Main1);
        inject_card_into_hand(&mut engine, caster as usize, "grizzly_bears");
        give_mana(
            &mut engine,
            caster,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        let hand = hand_index_for_card(&engine, caster as usize, "grizzly_bears");
        engine
            .apply_command(caster, &cast_spell(hand, vec![]))
            .unwrap();
        let spell = engine.state.stack[0].id;
        let before_hands: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|p| p.hand.clone())
            .collect();
        let before_libraries: Vec<_> = engine
            .state
            .players
            .iter()
            .map(|p| p.library.clone())
            .collect();
        assert_eq!(engine.state.stack.len(), if caster == 0 { 1 } else { 2 });
        if caster != 0 {
            let expected: Vec<_> = before_libraries[caster as usize]
                .iter()
                .take(7)
                .copied()
                .collect();
            pass_all_players(&mut engine);
            assert_eq!(engine.state.stack.len(), 1);
            assert_eq!(
                engine.state.stack[0].id, spell,
                "cast trigger resolves first"
            );
            assert_eq!(
                engine.state.players[caster as usize].hand.len(),
                before_hands[caster as usize].len() + 7
            );
            for object in expected {
                assert!(engine.state.players[caster as usize].hand.contains(&object));
                assert_eq!(engine.state.objects[&object].zone, Zone::Hand);
            }
            assert_eq!(
                engine.state.players[caster as usize].library.len(),
                before_libraries[caster as usize].len() - 7
            );
            for other in 0..3 {
                if other != caster as usize {
                    assert_eq!(engine.state.players[other].hand, before_hands[other]);
                    assert_eq!(engine.state.players[other].library, before_libraries[other]);
                }
            }
            assert!(
                engine.state.pending_resolution.is_none(),
                "mandatory untargeted draw"
            );
        }
        pass_all_players(&mut engine);
        assert!(engine.state.stack.is_empty());
        assert_eq!(engine.state.objects[&spell].zone, Zone::Battlefield);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    }
}
