//! Debug-only `DevCommand` cheats (`engine::dev`).
//!
//! Two things are under test here and they pull in opposite directions: the gate must hold shut
//! by default, and once open the cheats must produce state indistinguishable from the real thing
//! — a conjured permanent has to fire ETB triggers, register static abilities, and *not* be a
//! token, or the console silently lies to whoever is using it to test a card.

use crate::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    permanent_moved, DevAddMana, DevCommand, DevMoveCard, DevPutCardInZone, DevZone,
};

fn dev(target: i32, payload: Dev) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(payload),
        })),
    }
}

fn put(target: i32, zone: DevZone, card_name: &str) -> RuledCommand {
    put_ready(target, zone, card_name, false)
}

fn put_ready(target: i32, zone: DevZone, card_name: &str, ready: bool) -> RuledCommand {
    dev(
        target,
        Dev::PutCardInZone(DevPutCardInZone {
            card_name: card_name.to_string(),
            zone: zone as i32,
            ready,
        }),
    )
}

fn mv(target: i32, zone: DevZone, card_name: &str) -> RuledCommand {
    dev(
        target,
        Dev::MoveCard(DevMoveCard {
            card_name: card_name.to_string(),
            zone: zone as i32,
            ready: false,
        }),
    )
}

fn add_mana(target: i32, w: u32, u: u32, b: u32, r: u32, g: u32, c: u32) -> RuledCommand {
    dev(target, Dev::AddMana(DevAddMana { w, u, b, r, g, c }))
}

/// Two all-basic decks, so every card named in these tests is one no decklist contains and the
/// conjure path is what gets exercised.
fn basics_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![vec!["mountain".into(); 12], vec!["forest".into(); 12]]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    e.enable_dev_commands();
    advance_to_main1_from_game_start(&mut e);
    e
}

fn power_of(e: &GameEngine, oid: u32) -> u32 {
    e.characteristics(oid)
        .expect("object exists")
        .power
        .expect("creature has power")
}

#[test]
fn earthbend_logged_animation_and_return_replay_identically() {
    let mut engine = basics_engine(150_200);
    let mut commands = vec![
        (0, put(0, DevZone::Battlefield, "Plains")),
        (0, put(0, DevZone::Battlefield, "Rebellious Captives")),
        (0, add_mana(0, 0, 0, 0, 0, 0, 6)),
    ];
    let mut batches = Vec::new();
    for (actor, command) in &commands {
        batches.push(engine.apply_command(*actor, command).unwrap());
    }
    let battlefield = &engine.state.players[0].battlefield;
    let land = *battlefield
        .iter()
        .find(|oid| engine.state.objects[oid].card_id == "plains")
        .unwrap();
    let source = *battlefield
        .iter()
        .find(|oid| engine.state.objects[oid].card_id == "rebellious_captives")
        .unwrap();
    let command = activate_ability_for(&engine, source, 0, target_object(land));
    batches.push(engine.apply_command(0, &command).unwrap());
    commands.push((0, command));
    for returning in [false, true] {
        if returning {
            let command = mv(0, DevZone::Exile, "Plains");
            batches.push(engine.apply_command(0, &command).unwrap());
            commands.push((0, command));
            assert_eq!(engine.state.stack.len(), 1);
        }
        while !engine.state.stack.is_empty() {
            let actor = engine.state.priority_player_id();
            let command = pass();
            batches.push(engine.apply_command(actor, &command).unwrap());
            commands.push((actor, command));
        }
        assert_eq!(
            engine.characteristics(land).unwrap().is_creature(),
            !returning
        );
    }
    assert!(engine.state.objects[&land].tapped);
    let mut replay = basics_engine(150_200);
    for ((actor, command), expected) in commands.iter().zip(&batches) {
        assert_eq!(&replay.apply_command(*actor, command).unwrap(), expected);
    }
    assert_eq!(
        replay.state.zone_change_generation[&land],
        engine.state.zone_change_generation[&land]
    );
    assert!(replay.state.objects[&land].tapped);
    assert!(replay.state.active_event_observers.is_empty());
}

#[test]
fn issue_176_graveyard_payment_and_stale_target_replay() {
    for (card, name, generic, stale) in [
        ("grizzly_bears", "Grizzly Bears", 1, false),
        ("serra_angel", "Serra Angel", 4, false),
        ("grizzly_bears", "Grizzly Bears", 1, true),
    ] {
        let mut engine = basics_engine(176_200);
        let mut commands = vec![
            (0, put(0, DevZone::Hand, "No One Left Behind")),
            (0, put(0, DevZone::Hand, name)),
            (0, mv(0, DevZone::Graveyard, name)),
            (0, add_mana(0, 0, 0, 1, 0, 0, generic)),
        ];
        let mut batches = Vec::new();
        for (actor, command) in &commands {
            batches.push(engine.apply_command(*actor, command).unwrap());
        }
        let target = *engine.state.players[0]
            .graveyard
            .iter()
            .find(|oid| engine.state.objects[oid].card_id == card)
            .unwrap();
        let command = cast_spell(
            hand_index_for_card(&engine, 0, "no_one_left_behind"),
            vec![TargetRef {
                object_id: target,
                kind: TargetRefKind::Graveyard as i32,
                ..Default::default()
            }],
        );
        batches.push(engine.apply_command(0, &command).unwrap());
        commands.push((0, command));
        if stale {
            for zone in [DevZone::Hand, DevZone::Graveyard] {
                let command = mv(0, zone, name);
                batches.push(engine.apply_command(0, &command).unwrap());
                commands.push((0, command));
            }
        }
        while !engine.state.stack.is_empty() {
            let actor = engine.state.priority_player_id();
            let command = pass();
            batches.push(engine.apply_command(actor, &command).unwrap());
            commands.push((actor, command));
        }
        assert_eq!(
            engine.state.objects[&target].zone,
            if stale {
                Zone::Graveyard
            } else {
                Zone::Battlefield
            }
        );
        assert_eq!(engine.state.players[0].mana_pool.black, 0);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        let mut replay = basics_engine(176_200);
        for ((actor, command), expected) in commands.iter().zip(&batches) {
            assert_eq!(&replay.apply_command(*actor, command).unwrap(), expected);
        }
        assert_eq!(
            replay.state.objects[&target].zone,
            engine.state.objects[&target].zone
        );
    }
}

// ---------------------------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------------------------

/// The whole safety story is this test: a session that did not opt in refuses dev commands, and
/// refuses them *before* `command_index` moves. That is what keeps a rejected cheat out of the
/// replay log on the Servatrice side (it appends only on an ok response).
#[test]
fn dev_command_rejected_when_gate_is_off() {
    let decks = Some(vec![vec!["mountain".into(); 12], vec!["forest".into(); 12]]);
    let mut e = GameEngine::new(900, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let before = e.state.command_index;

    let err = e
        .apply_command(0, &put(0, DevZone::Hand, "Grizzly Bears"))
        .expect_err("dev commands are off by default");
    assert!(
        err.to_string().contains("dev commands are not enabled"),
        "unexpected error: {err}"
    );
    assert_eq!(
        e.state.command_index, before,
        "a rejected dev command must not advance command_index"
    );
    assert!(e.state.players[0]
        .hand
        .iter()
        .all(|oid| e.state.objects[oid].card_id != "grizzly_bears"));
}

/// Mid-mulligan zone moves would desync the opening bookkeeping, so the gate stays shut until the
/// opening procedure is over — even in a dev-enabled session.
#[test]
fn dev_command_rejected_during_opening() {
    let decks = Some(vec![vec!["mountain".into(); 12], vec!["forest".into(); 12]]);
    // skip_opening_sequence = false, so the engine starts inside the opening procedure.
    let mut e = GameEngine::new(901, &[0, 1], 20, decks, false).expect("new");
    e.enable_dev_commands();
    assert!(e.state.opening.is_some(), "still in the opening procedure");

    let err = e
        .apply_command(0, &put(0, DevZone::Hand, "Grizzly Bears"))
        .expect_err("dev commands are refused during the opening");
    assert!(
        err.to_string().contains("during opening"),
        "unexpected error: {err}"
    );
}

// ---------------------------------------------------------------------------------------------
// put: move vs conjure
// ---------------------------------------------------------------------------------------------

/// `put` always conjures, even when the seat already owns a copy. Assembling a board means
/// asking for two Serra Angels and getting two; the earlier move-first behaviour made that
/// impossible to express and silently yanked the existing one out of play instead.
#[test]
fn dev_put_always_conjures_so_repeating_it_builds_multiples() {
    let mut e = basics_engine(902);
    e.apply_command(0, &put(0, DevZone::Battlefield, "Serra Angel"))
        .expect("first conjure");
    e.apply_command(0, &put(0, DevZone::Battlefield, "Serra Angel"))
        .expect("second conjure");

    let on_battlefield = e.state.players[0]
        .battlefield
        .iter()
        .filter(|oid| e.state.objects[oid].card_id == "serra_angel")
        .count();
    assert_eq!(on_battlefield, 2, "two distinct Serra Angels");

    // And a copy in the library is left alone rather than being tutored out.
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        vec!["forest".into(); 12],
    ]);
    let mut e2 = GameEngine::new(9021, &[0, 1], 20, decks, true).expect("new");
    e2.enable_dev_commands();
    advance_to_main1_from_game_start(&mut e2);
    e2.apply_command(0, &put(0, DevZone::Hand, "Lightning Bolt"))
        .expect("conjure a Bolt");
    let total = e2
        .state
        .objects
        .values()
        .filter(|o| o.card_id == "lightning_bolt")
        .count();
    assert_eq!(total, 2, "the deck's copy plus the conjured one");
}

/// `move` is the counterpart: it relocates what already exists and creates nothing.
#[test]
fn dev_move_relocates_an_owned_card_without_duplicating_it() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        vec!["forest".into(); 12],
    ]);
    let mut e = GameEngine::new(9022, &[0, 1], 20, decks, true).expect("new");
    e.enable_dev_commands();
    advance_to_main1_from_game_start(&mut e);

    let batch = e
        .apply_command(0, &mv(0, DevZone::Hand, "Lightning Bolt"))
        .expect("move bolt to hand");

    let total = e
        .state
        .objects
        .values()
        .filter(|o| o.card_id == "lightning_bolt")
        .count();
    assert_eq!(total, 1, "moved, not duplicated");
    assert!(e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects[oid].card_id == "lightning_bolt"));
    // A move reports itself so the relay relocates the physical card.
    let moved = permanents_moved_in(&batch);
    assert_eq!(moved.len(), 1, "one PermanentMoved for the move path");
    assert_eq!(moved[0].card_id, "lightning_bolt");
}

/// Repeating a move has to keep finding the *next* copy. The search runs library, graveyard,
/// exile, hand, battlefield, so once one copy is in the graveyard a second `move gy` used to find
/// that one, move it to the graveyard it was already in, and log success while changing nothing —
/// leaving the copy on the battlefield untouched.
#[test]
fn dev_move_skips_copies_already_in_the_destination() {
    let mut e = basics_engine(9024);
    e.apply_command(0, &put(0, DevZone::Battlefield, "Serra Angel"))
        .expect("first conjure");
    e.apply_command(0, &put(0, DevZone::Battlefield, "Serra Angel"))
        .expect("second conjure");

    let on_battlefield = |e: &GameEngine| {
        e.state.players[0]
            .battlefield
            .iter()
            .filter(|oid| e.state.objects[oid].card_id == "serra_angel")
            .count()
    };
    assert_eq!(on_battlefield(&e), 2);

    e.apply_command(0, &mv(0, DevZone::Graveyard, "Serra Angel"))
        .expect("move the first");
    assert_eq!(count_card_id_in_graveyard(&e, 0, "serra_angel"), 1);
    assert_eq!(on_battlefield(&e), 1);

    e.apply_command(0, &mv(0, DevZone::Graveyard, "Serra Angel"))
        .expect("move the second");
    assert_eq!(
        count_card_id_in_graveyard(&e, 0, "serra_angel"),
        2,
        "the second move must find the copy still on the battlefield"
    );
    assert_eq!(on_battlefield(&e), 0);

    // With nothing left to move, say so distinctly rather than claiming the card does not exist.
    let err = e
        .apply_command(0, &mv(0, DevZone::Graveyard, "Serra Angel"))
        .expect_err("every copy is already there");
    assert!(
        err.to_string().contains("already in that zone"),
        "unexpected error: {err}"
    );
}

/// Moving something the seat does not own is an error rather than a silent conjure — otherwise
/// the two verbs would collapse back into one.
#[test]
fn dev_move_without_an_owned_copy_is_rejected() {
    let mut e = basics_engine(9023);
    let err = e
        .apply_command(0, &mv(0, DevZone::Hand, "Serra Angel"))
        .expect_err("nothing to move");
    assert!(
        err.to_string().contains("no copy of that card"),
        "unexpected error: {err}"
    );
}

/// The headline capability: a card in nobody's decklist can be put into hand and then cast.
#[test]
fn dev_put_conjures_a_card_no_deck_contains_and_it_is_castable() {
    let mut e = basics_engine(903);
    assert!(
        !e.state
            .objects
            .values()
            .any(|o| o.card_id == "lightning_bolt"),
        "precondition: no Bolt anywhere"
    );

    e.apply_command(0, &put(0, DevZone::Hand, "Lightning Bolt"))
        .expect("conjure bolt into hand");

    let idx = hand_index_for_card(&e, 0, "lightning_bolt");
    grant_pool(&mut e, 0);
    e.apply_command(0, &cast_spell(idx, target_player(1)))
        .expect("conjured card is castable");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 17, "Bolt dealt 3");
}

/// Conjuring emits the two events Servatrice needs: the refreshed catalog (so it can resolve the
/// new name at all) and the conjure notice itself (so it mints the backing Server_Card).
#[test]
fn dev_conjure_emits_catalog_and_conjure_events() {
    let mut e = basics_engine(904);
    let batch = e
        .apply_command(0, &put(0, DevZone::Hand, "Serra Angel"))
        .expect("conjure");

    let conjured: Vec<_> = batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::DevCardConjured(d)) => Some(d),
            _ => None,
        })
        .collect();
    assert_eq!(conjured.len(), 1);
    assert_eq!(conjured[0].card_name, "Serra Angel");
    assert_eq!(conjured[0].owner_player_id, 0);
    assert_eq!(conjured[0].zone, DevZone::Hand as i32);
    assert!(conjured[0].is_creature);

    let catalog_has_serra = batch.events.iter().any(|ev| match &ev.ev {
        Some(Ev::CardCatalog(c)) => c.entries.iter().any(|entry| entry.name == "Serra Angel"),
        _ => false,
    });
    assert!(
        catalog_has_serra,
        "the refreshed catalog must carry the conjured card, or the relay's zone reconcile \
         cannot resolve its name and abandons the sync"
    );
}

#[test]
fn dev_conjured_land_is_classified_in_the_battlefield_view() {
    let mut e = basics_engine(940);
    let batch = e
        .apply_command(0, &put(0, DevZone::Battlefield, "Forest"))
        .expect("conjure Forest");
    let forest = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .find(|player| player.player_id == 0)
                .and_then(|player| {
                    player
                        .battlefield_objects
                        .iter()
                        .find(|object| object.card_id == "forest")
                }),
            _ => None,
        })
        .expect("conjured Forest in battlefield view");
    assert!(forest.is_land);
    assert!(!forest.is_creature);

    e.apply_command(0, &put(0, DevZone::Battlefield, "Grizzly Bears"))
        .expect("conjure creature");
    e.apply_command(0, &put(0, DevZone::Battlefield, "Glorious Anthem"))
        .expect("conjure noncreature nonland permanent");
    let snapshot = e.initial_response_batch();
    let view = snapshot
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view.per_player.iter().find(|player| player.player_id == 0),
            _ => None,
        })
        .expect("player battlefield snapshot");
    let creature = view
        .battlefield_objects
        .iter()
        .find(|object| object.card_id == "grizzly_bears")
        .expect("conjured creature in battlefield view");
    assert!(creature.is_creature);
    assert!(!creature.is_land);
    let other = view
        .battlefield_objects
        .iter()
        .find(|object| object.card_id == "glorious_anthem")
        .expect("conjured noncreature nonland permanent in battlefield view");
    assert!(!other.is_creature);
    assert!(!other.is_land);
}

#[test]
fn dev_conjure_waits_for_entry_replacement_choice_before_announcing_or_committing() {
    let mut e = basics_engine(933);
    e.apply_command(0, &put(0, DevZone::Battlefield, "Orb of Dreams"))
        .expect("conjure Orb");

    let prompt_batch = e
        .apply_command(0, &put(0, DevZone::Battlefield, "Diregraf Ghoul"))
        .expect("propose Ghoul entry");
    assert!(prompt_batch
        .events
        .iter()
        .all(|event| !matches!(&event.ev, Some(Ev::DevCardConjured(_)))));
    let ghoul = e
        .state
        .objects
        .values()
        .find(|object| object.card_id == "diregraf_ghoul")
        .expect("proposed Ghoul");
    assert_eq!(ghoul.zone, tricerules_core::Zone::Stack);
    assert!(!e.state.players[0].battlefield.contains(&ghoul.id));

    let pending = e.state.pending_resolution.as_ref().expect("CR 616 choice");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::ReplacementEffect
    );
    assert_eq!(pending.presentation.candidates.len(), 2);
    let application = pending.presentation.candidates[0];
    let completion = e
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .expect("choose entry replacement");

    let ghoul = e
        .state
        .objects
        .values()
        .find(|object| object.card_id == "diregraf_ghoul")
        .expect("committed Ghoul");
    assert_eq!(ghoul.zone, tricerules_core::Zone::Battlefield);
    assert!(ghoul.tapped);
    assert!(completion
        .events
        .iter()
        .any(|event| matches!(&event.ev, Some(Ev::DevCardConjured(_)))));
}

#[test]
fn dev_move_waits_for_entry_replacement_choice_before_leaving_the_source_zone() {
    let mut e = basics_engine(934);
    e.apply_command(0, &put(0, DevZone::Battlefield, "Orb of Dreams"))
        .expect("conjure Orb");
    e.apply_command(0, &put(0, DevZone::Hand, "Diregraf Ghoul"))
        .expect("conjure Ghoul into hand");
    let ghoul = *e.state.players[0]
        .hand
        .iter()
        .find(|oid| e.state.objects[oid].card_id == "diregraf_ghoul")
        .expect("Ghoul in hand");

    let prompt_batch = e
        .apply_command(0, &mv(0, DevZone::Battlefield, "Diregraf Ghoul"))
        .expect("propose move");
    assert_eq!(e.state.objects[&ghoul].zone, tricerules_core::Zone::Hand);
    assert!(e.state.players[0].hand.contains(&ghoul));
    assert!(prompt_batch.events.iter().all(|event| !matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved))
            if moved.destination == permanent_moved::Destination::Battlefield as i32
    )));

    let application = e
        .state
        .pending_resolution
        .as_ref()
        .expect("CR 616 choice")
        .presentation
        .candidates[0];
    let completion = e
        .apply_command(0, &submit_resolution_choice(vec![application]))
        .expect("choose entry replacement");
    assert_eq!(
        e.state.objects[&ghoul].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(e.state.objects[&ghoul].tapped);
    assert!(completion.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved))
            if moved.destination == permanent_moved::Destination::Battlefield as i32
    )));
}

/// A face name is accepted by the dev console as an alias for the physical card, but cards in
/// hand use the front face. The relay needs that front Oracle name to load art and hover text.
#[test]
fn dev_conjuring_transform_back_face_name_displays_the_front_face() {
    let mut e = basics_engine(930);
    let batch = e
        .apply_command(0, &put(0, DevZone::Hand, "Merciless Predator"))
        .expect("conjure by back-face alias");

    let conjured = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::DevCardConjured(conjured)) => Some(conjured),
            _ => None,
        })
        .expect("conjure event");
    assert_eq!(conjured.card_name, "Reckless Waif");

    let oid = *e.state.players[0].hand.last().expect("card in hand");
    assert_eq!(
        e.state.objects[&oid].card_id,
        "reckless_waif_merciless_predator"
    );
    assert_eq!(e.state.objects[&oid].face_up_index, 0);
}

/// Unlike a DFC, a split card has its combined characteristics outside the stack, so resolving a
/// face-name alias must keep the whole-card display rather than arbitrarily choosing face 0.
#[test]
fn dev_conjuring_split_face_name_keeps_the_combined_display() {
    let mut e = basics_engine(931);
    let batch = e
        .apply_command(0, &put(0, DevZone::Hand, "Ice"))
        .expect("conjure by split-face alias");

    let conjured = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::DevCardConjured(conjured)) => Some(conjured),
            _ => None,
        })
        .expect("conjure event");
    assert_eq!(conjured.card_name, "Fire // Ice");
}

/// Adventure uses one combined cards.xml entry in every physical zone. The face names remain
/// cast-choice labels only; minting the card under either alias must use the combined CardRef so
/// art and hover details work before and after it changes zones.
#[test]
fn dev_conjuring_adventure_face_name_keeps_the_combined_display() {
    let mut e = basics_engine(932);
    let batch = e
        .apply_command(0, &put(0, DevZone::Hand, "Bonecrusher Giant"))
        .expect("conjure by creature-face alias");

    let conjured = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::DevCardConjured(conjured)) => Some(conjured),
            _ => None,
        })
        .expect("conjure event");
    assert_eq!(conjured.card_name, "Bonecrusher Giant // Stomp");
}

/// Only hand and battlefield can be conjured into; the rest are move-only because Servatrice
/// keeps separate physical binding maps for them.
#[test]
fn dev_conjure_into_graveyard_exile_or_library_is_rejected() {
    let mut e = basics_engine(905);
    for zone in [DevZone::Graveyard, DevZone::Exile, DevZone::Library] {
        let err = e
            .apply_command(0, &put(0, zone, "Serra Angel"))
            .expect_err("conjuring is restricted to hand and battlefield");
        assert!(
            err.to_string().contains("hand and battlefield only"),
            "unexpected error for {zone:?}: {err}"
        );
    }
}

#[test]
fn dev_put_unknown_card_name_is_rejected() {
    let mut e = basics_engine(906);
    let err = e
        .apply_command(0, &put(0, DevZone::Hand, "Not A Real Card"))
        .expect_err("unknown Oracle name");
    assert!(
        err.to_string().contains("Not A Real Card"),
        "the error should name the card: {err}"
    );
}

/// The graveyard is reachable by moving even though conjuring into it is restricted — which is
/// the main reason `move` exists as its own verb rather than a flag on `put`.
#[test]
fn dev_move_reaches_zones_conjuring_cannot() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        vec!["forest".into(); 12],
    ]);
    let mut e = GameEngine::new(907, &[0, 1], 20, decks, true).expect("new");
    e.enable_dev_commands();
    advance_to_main1_from_game_start(&mut e);

    e.apply_command(0, &mv(0, DevZone::Graveyard, "Lightning Bolt"))
        .expect("move to graveyard");
    assert_eq!(count_card_id_in_graveyard(&e, 0, "lightning_bolt"), 1);

    e.apply_command(0, &mv(0, DevZone::Exile, "Lightning Bolt"))
        .expect("move on to exile");
    assert_eq!(count_card_id_in_graveyard(&e, 0, "lightning_bolt"), 0);
    assert_eq!(e.state.players[0].exile.len(), 1);
}

/// The documented two-step for a zone conjuring cannot reach: conjure to hand, then move it on.
#[test]
fn dev_conjure_then_move_reaches_the_graveyard() {
    let mut e = basics_engine(9071);
    e.apply_command(0, &put(0, DevZone::Hand, "Serra Angel"))
        .expect("conjure into hand");
    e.apply_command(0, &mv(0, DevZone::Graveyard, "Serra Angel"))
        .expect("then move it to the graveyard");
    assert_eq!(count_card_id_in_graveyard(&e, 0, "serra_angel"), 1);
}

/// The hand is the documented staging zone for a card that will be moved into a public zone.
/// Prefer that visible copy when the deck already contains another card with the same name.
#[test]
fn dev_conjure_then_move_prefers_the_staged_hand_copy_over_the_library() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        vec!["forest".into(); 12],
    ]);
    let mut e = GameEngine::new(9072, &[0, 1], 20, decks, true).expect("new");
    e.enable_dev_commands();
    advance_to_main1_from_game_start(&mut e);

    let library_oid = *e.state.players[0]
        .library
        .iter()
        .find(|oid| e.state.objects[oid].card_id == "lightning_bolt")
        .expect("deck copy");
    e.apply_command(0, &put(0, DevZone::Hand, "Lightning Bolt"))
        .expect("conjure staged copy");
    let staged_oid = *e.state.players[0]
        .hand
        .iter()
        .find(|oid| e.state.objects[oid].card_id == "lightning_bolt")
        .expect("staged hand copy");

    e.apply_command(0, &mv(0, DevZone::Graveyard, "Lightning Bolt"))
        .expect("move staged copy to graveyard");

    assert!(e.state.players[0].graveyard.contains(&staged_oid));
    assert!(
        e.state.players[0].library.contains(&library_oid),
        "the same-named deck copy must remain in the library"
    );
}

// ---------------------------------------------------------------------------------------------
// Battlefield entry: what put bf actually does
// ---------------------------------------------------------------------------------------------

/// CR 302.6 applies to a conjured permanent exactly as to a cast one.
#[test]
fn dev_put_onto_battlefield_is_summoning_sick_by_default() {
    let mut e = basics_engine(908);
    e.apply_command(0, &put(0, DevZone::Battlefield, "Grizzly Bears"))
        .expect("conjure onto battlefield");

    let oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    assert!(
        e.state.objects[&oid].summoning_sick,
        "a conjured creature is summoning sick like any other"
    );
}

/// `ready` is the whole reason combat can be tested without passing a turn cycle.
#[test]
fn dev_put_ready_clears_summoning_sickness_and_allows_attacking() {
    let mut e = basics_engine(909);
    e.apply_command(
        0,
        &put_ready(0, DevZone::Battlefield, "Grizzly Bears", true),
    )
    .expect("conjure ready");

    let oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    assert!(
        !e.state.objects[&oid].summoning_sick,
        "ready must survive move_object_to_zone re-asserting sickness on entry"
    );

    // And it can actually be declared as an attacker this turn.
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    e.apply_command(0, &declare_attackers(vec![oid]))
        .expect("a readied creature can attack the turn it arrives");
}

/// Without `ready`, the same creature is refused as an attacker — the flag is doing the work,
/// not some other quirk of the conjure path.
#[test]
fn dev_put_without_ready_cannot_attack_the_turn_it_arrives() {
    let mut e = basics_engine(910);
    e.apply_command(0, &put(0, DevZone::Battlefield, "Grizzly Bears"))
        .expect("conjure");
    let oid = battlefield_object_for_card(&e, 0, "grizzly_bears");

    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass");
    e.apply_command(1, &pass()).expect("nap pass");
    e.apply_command(0, &declare_attackers(vec![oid]))
        .expect_err("summoning sick creature cannot attack");
}

#[test]
fn dev_put_ready_into_hand_is_a_no_op_not_an_error() {
    let mut e = basics_engine(911);
    e.apply_command(0, &put_ready(0, DevZone::Hand, "Grizzly Bears", true))
        .expect("ready is ignored for non-battlefield zones");
    assert_eq!(hand_index_for_card(&e, 0, "grizzly_bears"), {
        e.state.players[0]
            .hand
            .iter()
            .position(|oid| e.state.objects[oid].card_id == "grizzly_bears")
            .unwrap()
    });
}

/// CR 603.6a: enters-the-battlefield abilities trigger however the permanent arrived.
#[test]
fn dev_put_onto_battlefield_fires_etb_triggers() {
    let mut e = basics_engine(912);
    e.apply_command(0, &put(0, DevZone::Battlefield, "Soul Warden"))
        .expect("conjure warden");
    let life_before = e.state.players[0].life;

    e.apply_command(0, &put(0, DevZone::Battlefield, "Grizzly Bears"))
        .expect("conjure a creature for the warden to see");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.players[0].life,
        life_before + 1,
        "Soul Warden must see a conjured creature enter"
    );
}

/// The load-bearing one. `fire_triggers`'s EntersBattlefield arm is the only path to
/// `emit_static_abilities_on_enter`, so skipping it would leave a conjured anthem on the
/// battlefield granting nothing at all — with no error and no log line to explain it.
#[test]
fn dev_put_onto_battlefield_registers_static_abilities() {
    let mut e = basics_engine(913);
    e.apply_command(
        0,
        &put_ready(0, DevZone::Battlefield, "Grizzly Bears", true),
    )
    .expect("conjure bears");
    let bears = battlefield_object_for_card(&e, 0, "grizzly_bears");
    assert_eq!(power_of(&e, bears), 2, "printed 2/2 before the anthem");

    e.apply_command(0, &put(0, DevZone::Battlefield, "Glorious Anthem"))
        .expect("conjure anthem");
    assert_eq!(
        power_of(&e, bears),
        3,
        "a conjured anthem must actually grant its bonus"
    );

    // And the reverse direction: leaving the battlefield drains it (CR 604.3/611.3).
    e.apply_command(0, &mv(0, DevZone::Hand, "Glorious Anthem"))
        .expect("move the anthem off the battlefield");
    assert_eq!(power_of(&e, bears), 2, "the anthem stops applying on leave");
}

/// CR 601: cast triggers key on *casting*. Putting a permanent onto the battlefield is not
/// casting, so Argothian Enchantress must not draw off a conjured enchantment — the same
/// behaviour as a real put-onto-the-battlefield effect.
#[test]
fn dev_put_onto_battlefield_fires_no_cast_trigger() {
    let mut e = basics_engine(914);
    e.apply_command(0, &put(0, DevZone::Battlefield, "Argothian Enchantress"))
        .expect("conjure enchantress");
    resolve_entire_stack_two_player(&mut e);
    let hand_before = e.state.players[0].hand.len();

    e.apply_command(0, &put(0, DevZone::Battlefield, "Glorious Anthem"))
        .expect("conjure an enchantment");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "no card was cast, so no cast trigger may fire"
    );
}

/// A conjured card is a real card, not a token: `is_token` keys off the card_id namespace, so
/// the CR 111.7 "tokens cease to exist" SBA must leave it alone when it changes zones.
#[test]
fn dev_conjured_permanent_is_not_a_token() {
    let mut e = basics_engine(915);
    e.apply_command(0, &put(0, DevZone::Battlefield, "Grizzly Bears"))
        .expect("conjure");
    let oid = battlefield_object_for_card(&e, 0, "grizzly_bears");

    e.apply_command(0, &mv(0, DevZone::Graveyard, "Grizzly Bears"))
        .expect("move it to the graveyard");

    assert_eq!(count_card_id_in_graveyard(&e, 0, "grizzly_bears"), 1);
    assert!(
        e.state.objects.contains_key(&oid),
        "a conjured card survives leaving the battlefield; only tokens cease to exist"
    );
}

// ---------------------------------------------------------------------------------------------
// mana
// ---------------------------------------------------------------------------------------------

/// The other half of the point: no lands, no turns — add the mana and cast.
#[test]
fn dev_add_mana_lets_a_spell_be_cast_with_no_lands() {
    let mut e = basics_engine(916);
    e.apply_command(0, &put(0, DevZone::Hand, "Lightning Bolt"))
        .expect("conjure bolt");
    assert!(
        e.state.players[0].battlefield.is_empty(),
        "precondition: no lands in play"
    );

    e.apply_command(0, &add_mana(0, 0, 0, 0, 1, 0, 0))
        .expect("add {R}");
    assert_eq!(e.state.players[0].mana_pool.red, 1);

    let idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(idx, target_player(1)))
        .expect("dev mana pays for the spell");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 17);
}

#[test]
fn dev_add_mana_is_additive_and_targets_the_named_seat() {
    let mut e = basics_engine(917);
    e.apply_command(0, &add_mana(1, 2, 0, 0, 0, 0, 3))
        .expect("add mana to the opponent");
    e.apply_command(0, &add_mana(1, 1, 0, 0, 0, 0, 0))
        .expect("add more");

    assert_eq!(e.state.players[1].mana_pool.white, 3, "2 + 1, additive");
    assert_eq!(e.state.players[1].mana_pool.colorless, 3);
    assert_eq!(
        e.state.players[0].mana_pool.white, 0,
        "the sender's own pool is untouched"
    );
}

/// Dev mana is real mana in every respect that matters, including emptying (CR 106.4).
#[test]
fn dev_add_mana_empties_at_the_next_step_change() {
    let mut e = basics_engine(918);
    e.apply_command(0, &add_mana(0, 0, 0, 0, 5, 0, 0))
        .expect("add {R}{R}{R}{R}{R}");
    assert_eq!(e.state.players[0].mana_pool.red, 5);

    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    assert_eq!(
        e.state.players[0].mana_pool.red, 0,
        "pools empty on a step/phase change like any other mana"
    );
}

#[test]
fn dev_command_for_an_unknown_seat_is_rejected() {
    let mut e = basics_engine(919);
    e.apply_command(0, &add_mana(42, 1, 0, 0, 0, 0, 0))
        .expect_err("unknown player id");
}
