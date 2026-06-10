use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::ruled_event::Ev;

#[test]
fn engine_new_two_players() {
    let eng = GameEngine::new(12345, &[0, 1], 20, None, true).expect("engine");
    assert_eq!(eng.state.players.len(), 2);
}

#[test]
fn initial_batch_includes_card_catalog_then_zone_view_for_cockatrice() {
    let eng = GameEngine::new(12345, &[0, 1], 20, None, true).expect("engine");
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
                assert_eq!(p.hand.len(), 7, "opening hand");
                assert_eq!(
                    p.lib_ids_csv.split(',').count(),
                    60 - 7,
                    "rest in library (csv)"
                );
                assert_eq!(p.battlefield_power.len(), p.battlefield.len());
                assert_eq!(p.battlefield_is_creature.len(), p.battlefield.len());
            }
        }
        _ => panic!("expected ZoneView, got {:?}", e1.ev),
    }
}
