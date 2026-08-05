use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::ruled_event::Ev;

#[test]
fn engine_new_two_players() {
    let eng = GameEngine::new(12345, &[0, 1], 20, None, true).expect("engine");
    assert_eq!(eng.state.players.len(), 2);
}

#[test]
fn initial_batch_includes_card_catalog_then_zone_view_for_cockatrice() {
    let mut eng = GameEngine::new(12345, &[0, 1], 20, None, true).expect("engine");
    let b = eng.initial_response_batch();
    // Catalog first: Servatrice resolves the zone-view card ids through it.
    let e0 = b.events.first().expect("catalog is first");
    match e0.ev.as_ref() {
        Some(Ev::CardCatalog(c)) => {
            assert!(!c.entries.is_empty());
            let bolt = c
                .entries
                .iter()
                .find(|e| e.card_id == "lightning_bolt")
                .expect("default deck card in catalog");
            assert_eq!(bolt.name, "Lightning Bolt");
            assert!(!bolt.is_permanent, "instants do not resolve to battlefield");
            let mountain = c
                .entries
                .iter()
                .find(|e| e.card_id == "mountain")
                .expect("default deck land in catalog");
            assert_eq!(mountain.name, "Mountain");
            assert!(mountain.is_permanent);
        }
        _ => panic!("expected CardCatalog, got {:?}", e0.ev),
    }
    let e1 = b
        .events
        .get(1)
        .expect("zone view follows so server can sync before game state");
    match e1.ev.as_ref() {
        Some(Ev::ZoneView(z)) => {
            assert_eq!(z.per_player.len(), 2);
            for p in &z.per_player {
                assert_eq!(p.hand_cards.len(), 7, "opening hand");
                assert_eq!(p.library_card_ids.len(), 60 - 7, "rest in library");
            }
        }
        _ => panic!("expected ZoneView, got {:?}", e1.ev),
    }
}

#[test]
fn library_ids_preserve_comma_bearing_card_ids() {
    // Regression: card ids keep commas verbatim (the slug convention preserves them, e.g.
    // "kokusho,_the_evening_star"). The zone-view library was once a comma-joined string, which
    // split such an id into two entries — that inflated Servatrice's startup library count, so the
    // zone sync mismatched and the ruled session was torn down, leaving the game stuck at start.
    let deck = vec![
        "kokusho,_the_evening_star".to_string(),
        "mountain".to_string(),
        "mountain".to_string(),
    ];
    let decks = Some(vec![deck.clone(), deck]);
    // skip_opening_sequence = false: opening draws happen only after the choose-first command, so
    // the whole deck is still in the library when the initial zone view is emitted.
    let mut eng = GameEngine::new(1, &[0, 1], 20, decks, false).expect("engine");
    let b = eng.initial_response_batch();
    let zv = b
        .events
        .iter()
        .find_map(|e| match e.ev.as_ref() {
            Some(Ev::ZoneView(z)) => Some(z),
            _ => None,
        })
        .expect("zone view present in initial batch");
    for p in &zv.per_player {
        assert_eq!(
            p.library_card_ids.len(),
            3,
            "3 library cards; a comma must not be miscounted"
        );
        assert!(
            p.library_card_ids
                .iter()
                .any(|id| id == "kokusho,_the_evening_star"),
            "comma-bearing id must survive as a single lib_ids entry, got {:?}",
            p.library_card_ids
        );
    }
}
