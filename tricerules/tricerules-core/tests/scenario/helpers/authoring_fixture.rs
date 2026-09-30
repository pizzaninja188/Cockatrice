//! Shared setup for scenarios and registry conformance; resources are not semantic proof.
use super::*;
use tricerules_core::Zone;

pub(crate) fn game(seed: u64, players: &[i32], card: &str, ability: Option<usize>) -> GameEngine {
    let mut cards = vec![
        card,
        "grizzly_bears",
        "grizzly_bears",
        "grizzly_bears",
        "island",
        "explosive_apparatus",
    ];
    cards.extend(match card {
        "annul" => Some("short_sword"),
        "flashfreeze" => Some("hill_giant"),
        "flusterstorm" => Some("divination"),
        _ => None,
    });
    if card == "decimate" {
        cards.push("ominous_seas");
    }
    if card == "inventors_fair" {
        cards.extend(["sol_ring", "sol_ring"]);
    }
    if card == "trash_for_treasure" {
        cards.push("mind_stone");
    }
    if card == "trading_post" && ability == Some(2) {
        // This ability targets an artifact already in the graveyard before paying its cost.
        cards.push("sol_ring");
    }
    if card == "fanatic_of_rhonas" && ability == Some(1) {
        cards.push("air_elemental");
    }
    let deck = super::deck_with("forest", &cards);
    let mut e = GameEngine::new(seed, players, 20, Some(vec![deck; players.len()]), true).unwrap();
    super::advance_to_main1_from_game_start(&mut e);
    for player in 0..e.state.players.len() {
        super::relocate_to_battlefield(&mut e, player, "grizzly_bears", false);
        super::relocate_to_battlefield(&mut e, player, "explosive_apparatus", false);
        if card == "decimate" {
            super::relocate_to_battlefield(&mut e, player, "ominous_seas", false);
        }
        if card == "inventors_fair" {
            // The search activation requires three artifacts before its costs are paid.
            // Explosive Apparatus is already present; add the two missing fixture resources.
            super::relocate_to_battlefield(&mut e, player, "sol_ring", false);
            super::relocate_to_battlefield(&mut e, player, "sol_ring", false);
        }
        super::relocate_to_hand(&mut e, player, "grizzly_bears");
        super::relocate_to_battlefield(&mut e, player, "forest", false);
        super::relocate_to_battlefield(&mut e, player, "island", false);
        let dead = super::take_oid_from_library_or_hand(&mut e, player, "grizzly_bears");
        e.state.players[player].graveyard.push(dead);
        e.state.objects.get_mut(&dead).unwrap().zone = tricerules_core::Zone::Graveyard;
        if card == "trash_for_treasure" {
            // A separate graveyard artifact is required before paying the sacrifice cost.
            let dead = super::take_oid_from_library_or_hand(&mut e, player, "mind_stone");
            e.state.players[player].graveyard.push(dead);
            e.state.objects.get_mut(&dead).unwrap().zone = tricerules_core::Zone::Graveyard;
        }
        if card == "trading_post" && ability == Some(2) && player == 0 {
            let dead = super::take_oid_from_library_or_hand(&mut e, player, "sol_ring");
            e.state.players[player].graveyard.push(dead);
            e.state.objects.get_mut(&dead).unwrap().zone = tricerules_core::Zone::Graveyard;
        }
        super::grant_pool(&mut e, player);
    }
    if card == "fanatic_of_rhonas" && ability == Some(1) {
        // Ferocious needs a controlled creature with current power at least four.
        super::relocate_to_battlefield(&mut e, 0, "air_elemental", false);
    }
    e
}

/// Existing reviewed conformance source setup. Other zone abilities need explicit fixtures.
pub(crate) fn ability_source(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    face: usize,
    ability: usize,
) -> u32 {
    let oid = if card == "fanatic_of_rhonas" && ability == 2 {
        let oid = super::take_oid_from_library_or_hand(e, player, card);
        e.state.players[player].graveyard.push(oid);
        e.state.objects.get_mut(&oid).unwrap().zone = Zone::Graveyard;
        oid
    } else {
        super::relocate_to_battlefield(e, player, card, false)
    };
    e.state.objects.get_mut(&oid).unwrap().face_up_index = face;
    if card == "chandra,_novice_pyromancer" {
        // Direct relocation skips entry. This is the printed starting loyalty, not an expectation.
        e.state
            .objects
            .get_mut(&oid)
            .unwrap()
            .set_counter(tricerules_cards::CounterKind::Loyalty, 5);
    }
    oid
}
