//! The zone view's concealed-zone omission (`RuledPerPlayerView::private_zones_unchanged`).
//!
//! Most commands — priority passes, mana taps, phase rolls — move nothing into or out of a hand or
//! library, so re-sending ~60 library card ids per player per batch was pure repetition. The engine
//! now omits both zones for a player whose hand and library are unchanged since their last
//! broadcast view, and Servatrice keeps what it already reconciled.
//!
//! That makes the emission a *stateful protocol*, so the tests that matter most are not "is this
//! one batch omitted" but "does a receiver that follows the contract end up with the truth". The
//! mirror below is exactly the consumer Servatrice is: apply present, keep absent — then compare
//! against the engine's own zones after every single command.

use crate::helpers::*;
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    BattlefieldObject, DevCommand, DevPutCardInZone, DevZone, RuledPerPlayerView, ZoneViewSync,
};

/// Every zone view in a batch, in order. A batch can carry two (the untap-step roll emits one
/// mid-batch so tap state reaches Cockatrice while the untap phase is still in the batch).
fn zone_views(b: &RuledEventBatch) -> Vec<&ZoneViewSync> {
    b.events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .collect()
}

/// The last zone view in a batch — the one that reflects the batch's final state.
fn last_zone_view(b: &RuledEventBatch) -> &ZoneViewSync {
    zone_views(b).pop().expect("batch carries a zone view")
}

fn view_for(view: &ZoneViewSync, player: i32) -> &RuledPerPlayerView {
    view.per_player
        .iter()
        .find(|p| p.player_id == player)
        .expect("view covers the player")
}

/// The contract in one assertion: an omission carries nothing, and a full view carries everything.
fn assert_omission_is_total(view: &ZoneViewSync) {
    if view.battlefields_unchanged {
        assert!(
            view.per_player
                .iter()
                .all(|player| player.battlefield_objects.is_empty()),
            "battlefields marked unchanged but replacement objects were still shipped"
        );
    }
    for p in &view.per_player {
        if p.private_zones_unchanged {
            assert!(
                p.hand_cards.is_empty() && p.library_cards.is_empty(),
                "P{} marked unchanged but still shipped {} hand / {} library entries",
                p.player_id,
                p.hand_cards.len(),
                p.library_cards.len()
            );
        }
    }
}

fn hand_card_ids(e: &GameEngine, player: usize) -> Vec<String> {
    e.state.players[player]
        .hand
        .iter()
        .map(|oid| e.state.objects[oid].card_id.clone())
        .collect()
}

fn library_card_ids(e: &GameEngine, player: usize) -> Vec<String> {
    e.state.players[player]
        .library
        .iter()
        .map(|oid| e.state.objects[oid].card_id.clone())
        .collect()
}

/// A receiver that consumes zone views the way Servatrice does: a present hand/library replaces
/// what it holds, an absent one leaves it alone.
#[derive(Default)]
struct ZoneMirror {
    hands: Vec<Vec<String>>,
    libraries: Vec<Vec<String>>,
    battlefields: Vec<Vec<BattlefieldObject>>,
}

impl ZoneMirror {
    fn apply(&mut self, view: &ZoneViewSync) {
        assert_omission_is_total(view);
        if self.hands.len() < view.per_player.len() {
            self.hands.resize(view.per_player.len(), Vec::new());
            self.libraries.resize(view.per_player.len(), Vec::new());
            self.battlefields.resize(view.per_player.len(), Vec::new());
        }
        for (i, p) in view.per_player.iter().enumerate() {
            if !p.private_zones_unchanged {
                self.hands[i] = p.hand_cards.iter().map(|c| c.card_id.clone()).collect();
                self.libraries[i] = p
                    .library_cards
                    .iter()
                    .map(|card| card.card_id.clone())
                    .collect();
            }
            if !view.battlefields_unchanged {
                self.battlefields[i] = p.battlefield_objects.clone();
            }
        }
    }

    fn apply_batch(&mut self, b: &RuledEventBatch) {
        for view in zone_views(b) {
            self.apply(view);
        }
    }

    fn assert_matches(&self, e: &mut GameEngine, context: &str) {
        for player in 0..e.state.players.len() {
            assert_eq!(
                self.hands[player],
                hand_card_ids(e, player),
                "{context}: mirrored hand for P{player} drifted from the engine"
            );
            assert_eq!(
                self.libraries[player],
                library_card_ids(e, player),
                "{context}: mirrored library for P{player} drifted from the engine"
            );
        }
        let full = e.initial_response_batch();
        let expected = last_zone_view(&full);
        assert!(!expected.battlefields_unchanged);
        for (player, expected_player) in expected.per_player.iter().enumerate() {
            assert_eq!(
                self.battlefields[player], expected_player.battlefield_objects,
                "{context}: mirrored battlefield for P{player} drifted from a fresh full view"
            );
        }
    }
}

fn land_deck() -> Option<Vec<Vec<String>>> {
    Some(vec![
        std::iter::repeat_n("mountain".to_string(), 20).collect(),
        std::iter::repeat_n("forest".to_string(), 20).collect(),
    ])
}

/// The session's first view seeds Servatrice's physical deck and hand, so it can never be an
/// omission — the cache starts empty precisely to guarantee that.
#[test]
fn first_zone_view_of_a_session_is_full() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None, true).expect("new");
    let initial = e.initial_response_batch();
    let view = last_zone_view(&initial);
    for player in [0, 1] {
        let p = view_for(view, player);
        assert!(
            !p.private_zones_unchanged,
            "P{player}'s first view must carry hand and library in full"
        );
        assert_eq!(p.hand_cards.len(), 7);
        assert_eq!(p.library_cards.len(), 53);
    }
}

/// The optimization actually firing: a priority pass in main1 moves no card into or out of a
/// concealed zone, so neither player's hand or library is re-sent.
#[test]
fn priority_pass_omits_both_players_concealed_zones() {
    let mut e = GameEngine::new(99, &[0, 1], 20, land_deck(), true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let b = e.apply_command(0, &pass()).expect("p0 passes in main1");
    let view = last_zone_view(&b);
    assert_omission_is_total(view);
    for player in [0, 1] {
        let p = view_for(view, player);
        assert!(
            p.private_zones_unchanged,
            "P{player} changed neither hand nor library, so the view must omit them"
        );
    }
    assert!(
        view.battlefields_unchanged,
        "a priority handoff with no battlefield-visible change must omit every battlefield"
    );
    assert!(
        view.per_player
            .iter()
            .all(|player| player.battlefield_objects.is_empty()),
        "a battlefield omission must carry no replacement objects"
    );
}

/// The decision is per player, not per batch: playing a land empties one hand slot and leaves the
/// opponent untouched, so only the player who acted is re-sent.
#[test]
fn playing_a_land_resends_only_that_player() {
    let mut e = GameEngine::new(99, &[0, 1], 20, land_deck(), true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let idx = hand_index_for_card(&e, 0, "mountain");

    let b = e
        .apply_command(0, &play_land(idx))
        .expect("p0 plays a mountain");

    let view = last_zone_view(&b);
    assert_omission_is_total(view);
    let p0 = view_for(view, 0);
    assert!(
        !p0.private_zones_unchanged,
        "P0's hand shrank by the land, so their view must be full"
    );
    assert_eq!(p0.hand_cards.len(), e.state.players[0].hand.len());
    // The library did not move, but it rides along: the two zones are omitted jointly because
    // Servatrice reconciles them against one pool of physical cards and cannot apply half a view.
    assert_eq!(p0.library_cards.len(), e.state.players[0].library.len());
    assert!(
        view_for(view, 1).private_zones_unchanged,
        "P1 did nothing; their concealed zones must stay omitted"
    );
}

/// A draw is the other half of that: the drawing player is re-sent, their opponent is not.
#[test]
fn a_turn_roll_resends_only_the_player_who_drew() {
    let mut e = GameEngine::new(99, &[0, 1], 20, land_deck(), true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Roll to P1's turn: P1 draws for the turn (CR 103.8 only exempts the starting player's
    // first draw step), P0 does not. `state.turn` stays 1 here — it bumps when the active seat
    // wraps back to seat 0, not on every active-player change — so key on the active player.
    let mut p0_hand_before = hand_card_ids(&e, 0);
    let mut p0_library_before = library_card_ids(&e, 0);
    let p1_hand_before = e.state.players[1].hand.len();
    let mut rolled = None;
    for _ in 0..40 {
        p0_hand_before = hand_card_ids(&e, 0);
        p0_library_before = library_card_ids(&e, 0);
        let actor = e.state.priority_player_id();
        let b = e.apply_command(actor, &pass()).expect("pass");
        if e.state.active_player_id() == 1 {
            rolled = Some(b);
            break;
        }
    }
    let b = rolled.expect("the turn rolled to P1 within the pass budget");

    // The roll batch carries two views (untap-step sync, then the command tail); both must obey
    // the contract, and the second must not contradict the first.
    let views = zone_views(&b);
    assert!(
        views.len() >= 2,
        "the turn roll emits a mid-batch zone view"
    );
    for view in &views {
        assert_omission_is_total(view);
        assert!(
            view_for(view, 0).private_zones_unchanged,
            "P0 neither drew nor played; every view in the roll must omit them"
        );
    }
    assert_eq!(hand_card_ids(&e, 0), p0_hand_before);
    assert_eq!(library_card_ids(&e, 0), p0_library_before);
    // The roll batch stops at upkeep; keep passing until P1's draw step actually fires, then
    // check the batch that drew.
    let mut drew = None;
    for _ in 0..10 {
        let actor = e.state.priority_player_id();
        let b = e.apply_command(actor, &pass()).expect("pass");
        if e.state.players[1].hand.len() > p1_hand_before {
            drew = Some(b);
            break;
        }
    }
    let draw_batch = drew.expect("P1 reached their draw step");
    let view = last_zone_view(&draw_batch);
    assert_omission_is_total(view);
    assert!(
        !view_for(view, 1).private_zones_unchanged,
        "P1 drew, so their concealed zones had to be re-sent in full"
    );
    assert_eq!(
        view_for(view, 1).hand_cards.len(),
        p1_hand_before + 1,
        "the re-sent hand must be the post-draw one"
    );
    assert!(
        view_for(view, 0).private_zones_unchanged,
        "P0 did not draw; the same batch must still omit their zones"
    );
}

/// Conjuring a card into hand is a zone change like any other and must break the omission —
/// otherwise the dev console would hand a player a card Servatrice never learns about.
#[test]
fn dev_conjure_into_hand_resends_that_player() {
    let mut e = GameEngine::new(99, &[0, 1], 20, land_deck(), true).expect("new");
    e.enable_dev_commands();
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &pass()).expect("prime the cache");

    let conjure = RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 0,
            dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                card_name: "Grizzly Bears".to_string(),
                zone: DevZone::Hand as i32,
                ready: false,
            })),
        })),
    };
    let b = e.apply_command(0, &conjure).expect("conjure into hand");

    let view = last_zone_view(&b);
    assert_omission_is_total(view);
    let p0 = view_for(view, 0);
    assert!(!p0.private_zones_unchanged);
    assert_eq!(p0.hand_cards.len(), e.state.players[0].hand.len());
    assert!(
        p0.hand_cards.iter().any(|c| c.card_id == "grizzly_bears"),
        "the conjured card must appear in the re-sent hand"
    );
}

/// The contract end to end. A receiver applying present views and keeping absent ones must hold
/// the engine's exact hands and libraries after *every* command of a real game — draws, land
/// drops, casts, resolutions and all the priority passes between them.
#[test]
fn a_contract_following_receiver_never_drifts_from_the_engine() {
    let decks = Some(vec![
        {
            let mut d: Vec<String> = std::iter::repeat_n("mountain".to_string(), 10).collect();
            d.extend(std::iter::repeat_n("lightning_bolt".to_string(), 10));
            d
        },
        {
            let mut d: Vec<String> = std::iter::repeat_n("forest".to_string(), 10).collect();
            d.extend(std::iter::repeat_n("grizzly_bears".to_string(), 10));
            d
        },
    ]);
    let mut e = GameEngine::new(7, &[0, 1], 20, decks, true).expect("new");
    let mut mirror = ZoneMirror::default();
    mirror.apply(last_zone_view(&e.initial_response_batch()));
    mirror.assert_matches(&mut e, "initial batch");

    // Every command from here on is applied through the mirror: a batch the mirror never sees
    // would let it drift for reasons that have nothing to do with the omission contract.
    for command in [pass(), pass(), pass(), pass()] {
        let actor = e.state.priority_player_id();
        let b = e.apply_command(actor, &command).expect("pass to main1");
        mirror.apply_batch(&b);
    }
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    mirror.assert_matches(&mut e, "at main1");

    let idx = hand_index_for_card(&e, 0, "mountain");
    let b = e
        .apply_command(0, &play_land(idx))
        .expect("p0 plays a mountain");
    mirror.apply_batch(&b);
    mirror.assert_matches(&mut e, "after the land drop");

    // Several turns of passing, which walks draws, turn rolls and the CR 514.1 cleanup discard —
    // the three things that actually move cards into and out of the concealed zones.
    let mut drew = false;
    for step in 0..60 {
        if e.state.winner.is_some() {
            break;
        }
        let command = match e.state.cleanup_discard_player {
            Some(_) => discard_cleanup(0),
            None => pass(),
        };
        let actor = e
            .state
            .cleanup_discard_player
            .unwrap_or_else(|| e.state.priority_player_id());
        let b = e.apply_command(actor, &command).expect("command");
        mirror.apply_batch(&b);
        mirror.assert_matches(&mut e, &format!("after command {step}"));
        drew |= e.state.turn > 1;
    }
    assert!(
        drew,
        "the walk must cross at least one turn roll, or it proves nothing about draws"
    );
}
