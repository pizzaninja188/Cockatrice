//! Exact deck-corpus coverage for Emrakul's Messenger and From Beyond.
//!
//! Oracle and rulings were checked against their exact Scryfall records on 2026-09-28.
//! CR 702.114 and 613.1e govern Devoid and color layers; CR 121.2 the second draw; CR 503.1 the
//! upkeep trigger; CR 701.23 the subtype search; and CR 605.1a/605.3b each Spawn/Scion mana ability.

use super::helpers::*;
use tricerules_cards::{CardRegistry, Color};
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::{permanent_moved::Destination, ChoiceKind};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &["divination"]),
        deck_with("forest", &["divination"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn shuffle_count(batch: &RuledEventBatch) -> usize {
    batch
        .events
        .iter()
        .filter(|event| {
            matches!(&event.ev, Some(Ev::Log(log)) if log.text == "P0 shuffles their library.")
        })
        .count()
}

fn assert_and_sacrifice_eldrazi_token(
    engine: &mut GameEngine,
    token_id: &str,
    subtype: &str,
    power: u32,
) {
    let token = battlefield_token_oids(engine, 0, token_id);
    assert_eq!(token.len(), 1, "exactly one {subtype} token was created");
    let token = token[0];
    let characteristics = engine
        .characteristics(token)
        .expect("token characteristics");
    assert!(characteristics.is_creature());
    assert!(characteristics.has_type("Eldrazi"));
    assert!(characteristics.has_type(subtype));
    assert!(characteristics.colors.is_empty());
    assert_eq!(characteristics.power, Some(power));
    assert_eq!(characteristics.toughness, Some(1));

    let definition = CardRegistry::global()
        .get(token_id)
        .expect("Eldrazi token definition");
    let ability = definition
        .primary_face()
        .activated_abilities
        .first()
        .expect("sacrifice-for-colorless ability");
    assert!(ability.is_mana_ability());

    let mana_before = engine.state.players[0].mana_pool.colorless;
    apply_ability(engine, 0, token, 0, vec![]).expect("sacrifice token for {C}");
    assert_eq!(engine.state.players[0].mana_pool.colorless, mana_before + 1);
    assert!(
        engine.state.stack.is_empty(),
        "mana ability resolves immediately"
    );
    assert!(battlefield_token_oids(engine, 0, token_id).is_empty());
}

fn resolve_search_ability(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_command(first, &pass())
        .expect("first player passes priority");
    engine
        .apply_command(second, &pass())
        .expect("second player resolves the ability")
}

#[test]
fn devoid_cards_are_colorless_but_keep_their_mana_cost_color_identity() {
    let registry = CardRegistry::global();
    let messenger = registry
        .get("emrakuls_messenger")
        .expect("Emrakul's Messenger is registered");
    assert_eq!(messenger.name, "Emrakul's Messenger");
    assert_eq!(messenger.primary_face().mana_cost.to_string(), "{1}{U}");
    assert!(messenger.primary_face().colors().is_empty());
    assert_eq!(messenger.color_identity(), vec![Color::Blue]);

    let from_beyond = registry
        .get("from_beyond")
        .expect("From Beyond is registered");
    assert_eq!(from_beyond.primary_face().mana_cost.to_string(), "{3}{G}");
    assert!(from_beyond.primary_face().colors().is_empty());
    assert_eq!(from_beyond.color_identity(), vec![Color::Green]);
}

#[test]
fn emrakuls_messenger_creates_a_spawn_on_its_controllers_second_draw() {
    let mut engine = engine(202_609_283);
    inject_permanent_on_battlefield(&mut engine, 0, "emrakuls_messenger");
    ensure_in_hand(&mut engine, 0, "divination");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    let divination = hand_index_for_card(&engine, 0, "divination");
    engine
        .apply_command(0, &cast_spell(divination, vec![]))
        .expect("cast Divination to draw two cards");
    resolve_entire_stack_two_player(&mut engine);

    assert_and_sacrifice_eldrazi_token(&mut engine, "eldrazi_spawn_c_0_1", "Spawn", 0);
}

#[test]
fn emrakuls_messenger_does_not_trigger_for_an_opponents_second_draw() {
    let mut engine = engine(202_609_284);
    inject_permanent_on_battlefield(&mut engine, 0, "emrakuls_messenger");
    end_active_turn(&mut engine, 0);
    pass_both_players(&mut engine); // opponent upkeep to draw
    pass_both_players(&mut engine); // opponent draw to main1
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
    ensure_in_hand(&mut engine, 1, "divination");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );

    let divination = hand_index_for_card(&engine, 1, "divination");
    engine
        .apply_command(1, &cast_spell(divination, vec![]))
        .expect("opponent casts Divination");
    resolve_entire_stack_two_player(&mut engine);

    assert!(engine.state.stack.is_empty());
    assert!(battlefield_token_oids(&engine, 0, "eldrazi_spawn_c_0_1").is_empty());
}

#[test]
fn from_beyond_creates_a_scion_at_its_controllers_upkeep() {
    let mut engine = engine(202_609_285);
    inject_permanent_on_battlefield(&mut engine, 0, "from_beyond");
    end_active_turn(&mut engine, 0);
    pass_both_players(&mut engine); // opponent upkeep to draw
    pass_both_players(&mut engine); // opponent draw to main1
    end_active_turn(&mut engine, 1);

    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    assert_eq!(engine.state.stack.len(), 1, "upkeep trigger is waiting");
    resolve_entire_stack_two_player(&mut engine);
    assert_and_sacrifice_eldrazi_token(&mut engine, "eldrazi_scion_c_1_1", "Scion", 1);
}

#[test]
fn from_beyond_sacrifices_to_search_reveal_and_shuffle_an_eldrazi_card() {
    let mut engine = engine(202_609_286);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "from_beyond");
    let eldrazi = inject_library_card(&mut engine, 0, "eldrazi_devastator");
    let name_only_match = inject_library_card(&mut engine, 0, "eldrazi_monument");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );

    apply_ability(&mut engine, 0, source, 0, vec![]).expect("pay {1}{G} and sacrifice From Beyond");
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    let search = resolve_search_ability(&mut engine);
    let choice = find_resolution_choice(&search).expect("Eldrazi library search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(choice.candidate_object_ids.contains(&eldrazi));
    assert!(
        !choice.candidate_object_ids.contains(&name_only_match),
        "Eldrazi in a card's name does not satisfy the Eldrazi subtype filter"
    );

    assert!(
        engine
            .apply_command(0, &submit_resolution_choice(vec![name_only_match]))
            .is_err(),
        "a non-Eldrazi choice is rejected"
    );
    assert_eq!(engine.state.objects[&eldrazi].zone, Zone::Library);
    assert_eq!(engine.state.objects[&name_only_match].zone, Zone::Library);

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![eldrazi]))
        .expect("choose the Eldrazi card");
    assert_eq!(engine.state.objects[&eldrazi].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&eldrazi));
    assert_eq!(engine.state.objects[&name_only_match].zone, Zone::Library);
    assert!(completion.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::CardsRevealed(reveal))
            if reveal.cards.iter().any(|card| card.object_id == eldrazi))
    }));
    assert_eq!(shuffle_count(&completion), 1);
    assert!(completion.events.iter().any(|event| {
        matches!(&event.ev, Some(Ev::PermanentMoved(moved))
            if moved.object_id == eldrazi && moved.destination == Destination::Hand as i32)
    }));
}

#[test]
fn from_beyond_shuffles_after_searching_without_an_eligible_eldrazi() {
    let mut engine = engine(202_609_287);
    let source = inject_permanent_on_battlefield(&mut engine, 0, "from_beyond");
    let name_only_match = inject_library_card(&mut engine, 0, "eldrazi_monument");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, source, 0, vec![]).expect("pay {1}{G} and sacrifice From Beyond");

    let search = resolve_search_ability(&mut engine);
    let choice = find_resolution_choice(&search).expect("empty Eldrazi library search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(choice.candidate_object_ids.is_empty());
    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("fail to find an Eldrazi card");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&name_only_match].zone, Zone::Library);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(shuffle_count(&completion), 1);
    assert!(!completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::CardsRevealed(_)))));
}
