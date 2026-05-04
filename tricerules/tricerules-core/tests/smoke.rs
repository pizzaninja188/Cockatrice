use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::ruled_event::Ev;

#[test]
fn engine_new_two_players() {
    let eng = GameEngine::new(12345, &[0, 1], 20, None, true).expect("engine");
    assert_eq!(eng.state.players.len(), 2);
}

#[test]
fn initial_batch_includes_zone_view_for_cockatrice() {
    let eng = GameEngine::new(12345, &[0, 1], 20, None, true).expect("engine");
    let b = eng.initial_response_batch();
    let e0 = b
        .events
        .first()
        .expect("zone view is first so server can sync before game state");
    match e0.ev.as_ref() {
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
        _ => panic!("expected ZoneView, got {:?}", e0.ev),
    }
}
