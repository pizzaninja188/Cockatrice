//! Exact Garruk's Uprising clauses and its intervening-if versus entry-event distinction.
use super::helpers::*;
use tricerules_cards::{CardRegistry, CounterKind, Keyword};
use tricerules_core::{GameEngine, Zone};

fn game() -> GameEngine {
    assert!(
        CardRegistry::global().get("garruks_uprising").is_some(),
        "exact scoped card must be registered"
    );
    let mut engine = GameEngine::new(
        2026093009,
        &[0, 1],
        20,
        Some(vec![forest_only_deck(), forest_only_deck()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn seed_creature(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    // The combat helper overrides all creatures to 2/2. This fixture uses printed stats.
    assert!(CardRegistry::global().get(card_id).is_some());
    inject_permanent_on_battlefield(engine, player, card_id)
}

fn resolve_top(engine: &mut GameEngine) {
    answer_trigger_order_in_engine_order(engine);
    let first = engine.state.priority_player_id();
    engine.apply_command(first, &pass()).unwrap();
    let second = engine.state.priority_player_id();
    engine.apply_command(second, &pass()).unwrap();
}

fn cast(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
    x: u32,
) -> u32 {
    let object = inject_card_into_hand(engine, player, card_id);
    grant_pool(engine, player);
    let slot = hand_index_for_card(engine, player, card_id);
    engine
        .apply_command(player as i32, &cast_spell_x(slot, targets, x))
        .unwrap();
    resolve_top(engine);
    object
}

fn move_control(engine: &mut GameEngine, object: u32, from: usize, to: usize) {
    engine.state.players[from]
        .battlefield
        .retain(|id| *id != object);
    engine.state.players[to].battlefield.push(object);
    let permanent = engine.state.objects.get_mut(&object).unwrap();
    permanent.controller = to as i32;
    permanent.base_controller = to as i32;
}

fn advance_to_main(engine: &mut GameEngine, player: i32) {
    for _ in 0..60 {
        if engine.state.turn_step == tricerules_core::TurnStep::Main1
            && engine.state.active_player_id() == player
        {
            return;
        }
        let (actor, command) = match engine.state.cleanup_discard_player {
            Some(actor) => {
                let index = engine.state.player_idx(actor).unwrap();
                let excess = engine.state.players[index].hand.len() - 7;
                (actor, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (engine.state.priority_player_id(), pass()),
        };
        engine.apply_command(actor, &command).unwrap();
    }
    panic!("did not reach requested main phase");
}

#[test]
fn exact_characteristics_and_three_printed_clauses() {
    let card = CardRegistry::global()
        .get("garruks_uprising")
        .expect("Garruk's Uprising registered");
    assert_eq!(card.name, "Garruk's Uprising");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.types, ["Enchantment"]);
    assert_eq!(face.triggered_abilities.len(), 2);
    assert_eq!(face.static_abilities.len(), 1);
    assert!(face.triggered_abilities[0].intervening_if.is_some());
    assert!(face.triggered_abilities[1].intervening_if.is_none());
    assert!(!face.triggered_abilities[0].may);
    assert!(!face.triggered_abilities[1].may);
}

#[test]
fn entry_draw_checks_current_controlled_power_twice_and_draws_only_once() {
    for mode in 0..7 {
        let mut engine = game();
        let first = match mode {
            0 => None,
            1 => Some(seed_creature(&mut engine, 0, "centaur_courser")),
            6 => Some(seed_creature(&mut engine, 0, "grizzly_bears")),
            _ => Some(seed_creature(&mut engine, 0, "air_elemental")),
        };
        if mode == 5 {
            seed_creature(&mut engine, 0, "air_elemental");
        }
        if mode == 6 {
            cast(
                &mut engine,
                0,
                "giant_growth",
                target_object(first.unwrap()),
                0,
            );
            assert_eq!(engine.effective_power(first.unwrap()), Some(5));
        }
        let library = engine.state.players[0].library.len();
        let uprising = cast(&mut engine, 0, "garruks_uprising", vec![], 0);
        assert_eq!(engine.state.objects[&uprising].zone, Zone::Battlefield);
        assert_eq!(
            engine.state.stack.len(),
            usize::from(mode >= 2),
            "entry mode {mode}"
        );
        if mode == 3 || mode == 4 {
            cast(&mut engine, 0, "unsummon", target_object(first.unwrap()), 0);
            assert_eq!(engine.state.objects[&first.unwrap()].zone, Zone::Hand);
            if mode == 4 {
                // Isolate the self-entry recheck from the separate creature-entry ability.
                seed_creature(&mut engine, 0, "air_elemental");
            }
        }
        resolve_entire_stack_two_player(&mut engine);
        let draws = usize::from(mode >= 2 && mode != 3);
        assert_eq!(engine.state.players[0].library.len(), library - draws);
        assert!(
            engine.state.pending_resolution.is_none(),
            "both draws are mandatory"
        );
    }
    let mut engine = game();
    seed_creature(&mut engine, 1, "air_elemental");
    cast(&mut engine, 0, "garruks_uprising", vec![], 0);
    assert!(
        engine.state.stack.is_empty(),
        "an opponent's creature does not satisfy your condition"
    );
}

#[test]
fn trample_tracks_current_control_and_source_lifetime() {
    let mut engine = game();
    let own = seed_creature(&mut engine, 0, "grizzly_bears");
    let foreign_owned = inject_creature_under_foreign_control(&mut engine, 1, 0, "grizzly_bears");
    let enemy = seed_creature(&mut engine, 1, "grizzly_bears");
    let uprising = cast(&mut engine, 0, "garruks_uprising", vec![], 0);
    assert!(engine.state.stack.is_empty());
    let later = cast(&mut engine, 0, "grizzly_bears", vec![], 0);
    for creature in [own, foreign_owned, later] {
        assert!(engine.effective_has_keyword(creature, Keyword::Trample));
    }
    assert!(!engine.effective_has_keyword(enemy, Keyword::Trample));
    move_control(&mut engine, own, 0, 1);
    assert!(!engine.effective_has_keyword(own, Keyword::Trample));
    move_control(&mut engine, enemy, 1, 0);
    assert!(engine.effective_has_keyword(enemy, Keyword::Trample));
    move_control(&mut engine, uprising, 0, 1);
    assert!(engine.effective_has_keyword(own, Keyword::Trample));
    for creature in [foreign_owned, later, enemy] {
        assert!(!engine.effective_has_keyword(creature, Keyword::Trample));
    }
    cast(&mut engine, 0, "disenchant", target_object(uprising), 0);
    assert_eq!(engine.state.objects[&uprising].zone, Zone::Graveyard);
    for creature in [own, foreign_owned, later, enemy] {
        assert!(!engine.effective_has_keyword(creature, Keyword::Trample));
    }
}

#[test]
fn creature_entry_uses_event_power_and_controller_without_resolution_recheck() {
    for (card_id, remove, expected) in [
        ("centaur_courser", false, 0),
        ("air_elemental", false, 1),
        ("air_elemental", true, 1),
    ] {
        let mut engine = game();
        cast(&mut engine, 0, "garruks_uprising", vec![], 0);
        let library = engine.state.players[0].library.len();
        let creature = cast(&mut engine, 0, card_id, vec![], 0);
        assert_eq!(engine.state.stack.len(), expected);
        if remove {
            cast(&mut engine, 0, "unsummon", target_object(creature), 0);
        }
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.players[0].library.len(), library - expected);
    }
    let mut engine = game();
    cast(&mut engine, 0, "garruks_uprising", vec![], 0);
    advance_to_main(&mut engine, 1);
    let library = engine.state.players[0].library.len();
    cast(&mut engine, 1, "air_elemental", vec![], 0);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].library.len(), library);
}

#[test]
fn creature_entry_sees_static_bonus_counters_and_copied_values() {
    for mode in 0..3 {
        let mut engine = game();
        cast(&mut engine, 0, "garruks_uprising", vec![], 0);
        let library = engine.state.players[0].library.len();
        let creature = match mode {
            0 => {
                cast(&mut engine, 0, "glorious_anthem", vec![], 0);
                cast(&mut engine, 0, "centaur_courser", vec![], 0)
            }
            1 => {
                let creature = cast(&mut engine, 0, "endless_one", vec![], 4);
                assert_eq!(
                    engine.state.objects[&creature].counter_count(CounterKind::PlusOnePlusOne),
                    4
                );
                creature
            }
            _ => {
                let source = seed_creature(&mut engine, 1, "air_elemental");
                let creature = cast(&mut engine, 0, "clone", vec![], 0);
                assert!(
                    engine.state.pending_resolution.is_some(),
                    "copy source precedes entry"
                );
                engine
                    .apply_command(0, &submit_resolution_choice(vec![source]))
                    .unwrap();
                creature
            }
        };
        assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);
        assert_eq!(engine.effective_power(creature), Some(4));
        assert_eq!(
            engine.state.stack.len(),
            1,
            "post-replacement derived power creates one mandatory draw"
        );
        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(engine.state.players[0].library.len(), library - 1);
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn triggered_draw_survives_uprising_leaving_the_battlefield() {
    let mut engine = game();
    let uprising = cast(&mut engine, 0, "garruks_uprising", vec![], 0);
    let library = engine.state.players[0].library.len();
    let creature = cast(&mut engine, 0, "air_elemental", vec![], 0);
    assert_eq!(engine.state.stack.len(), 1);
    cast(&mut engine, 0, "disenchant", target_object(uprising), 0);
    assert_eq!(engine.state.objects[&uprising].zone, Zone::Graveyard);
    assert!(!engine.effective_has_keyword(creature, Keyword::Trample));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].library.len(), library - 1);
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn casting_requires_green_and_rejection_preserves_the_card_and_resources() {
    let mut engine = game();
    let uprising = inject_card_into_hand(&mut engine, 0, "garruks_uprising");
    engine.state.players[0].mana_pool.colorless = 3;
    let hand = engine.state.players[0].hand.clone();
    let library = engine.state.players[0].library.clone();
    let command_index = engine.state.command_index;
    let slot = hand_index_for_card(&engine, 0, "garruks_uprising");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect_err("three generic mana cannot pay the green pip");
    assert_eq!(engine.state.players[0].hand, hand);
    assert_eq!(engine.state.players[0].library, library);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 3);
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.state.objects[&uprising].zone, Zone::Hand);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
