use tricerules_core::GameEngine;

#[test]
fn engine_new_two_players() {
    let eng = GameEngine::new(12345, &[0, 1], 20, None).expect("engine");
    assert_eq!(eng.state.players.len(), 2);
}
