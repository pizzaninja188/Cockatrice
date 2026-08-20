use crate::helpers::*;

/// CR 704.5j: when a player controls two or more legendary permanents with the same name,
/// an SBA fires asking the controller which one to keep; the rest go to the graveyard.
/// This test verifies that (a) the SBA pauses for player choice instead of auto-resolving,
/// (b) the player's chosen legend stays on the battlefield, and
/// (c) the unchosen legend ends up in the graveyard.
#[test]
fn legend_rule_controller_chooses_which_to_keep() {
    // Build a small engine; Isamaru (legendary 2/2 Dog, no triggers) is in both libraries.
    let decks = Some(vec![
        std::iter::repeat_n("isamaru,_hound_of_konda".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("forest".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(42, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut e);

    // Put two copies of Isamaru on P0's battlefield directly.
    let oid_a = deploy_to_battlefield(&mut e, 0, "isamaru,_hound_of_konda", false);
    let oid_b = deploy_to_battlefield(&mut e, 0, "isamaru,_hound_of_konda", false);
    assert!(oid_a != oid_b, "two distinct objects");

    // Pass priority so SBAs run; the legend rule should pause for a choice.
    let batch = e
        .apply_command(0, &pass())
        .expect("pass priority triggers legend SBA");

    // The engine must emit a ResolutionChoiceRequired with ChoiceKind::LegendKeep.
    let choice_req = find_resolution_choice(&batch);
    assert!(
        choice_req.is_some(),
        "legend rule must emit ResolutionChoiceRequired"
    );
    let req = choice_req.unwrap();
    assert_eq!(
        req.choice_kind(),
        ChoiceKind::LegendKeep,
        "choice_kind must be LegendKeep"
    );
    assert_eq!(req.deciding_player_id, 0, "P0 owns the legends");
    // Candidates contain both duplicate legends.
    assert!(
        req.candidate_object_ids.contains(&oid_a) && req.candidate_object_ids.contains(&oid_b),
        "both duplicate legends must be candidates"
    );
    assert_eq!(req.min, 1);
    assert_eq!(req.max, 1);
    assert!(matches!(
        &e.state
            .pending_resolution
            .as_ref()
            .expect("legend continuation")
            .continuation,
        ResolutionContinuation::LegendKeep
    ));

    // P0 chooses to keep oid_b.
    let keep_batch = e
        .apply_command(0, &submit_resolution_choice(vec![oid_b]))
        .expect("submit legend keep choice");

    // oid_a must be in P0's graveyard; oid_b must still be on P0's battlefield.
    let p0_idx = e.state.player_idx(0).unwrap();
    assert!(
        e.state.players[p0_idx].graveyard.contains(&oid_a),
        "unchosen legend must be in graveyard"
    );
    assert!(
        e.state.players[p0_idx].battlefield.contains(&oid_b),
        "chosen legend must remain on battlefield"
    );

    // The batch must include a PermanentMoved event for oid_a → graveyard.
    let moved = permanents_moved_in(&keep_batch);
    assert!(
        moved.iter().any(|pm| pm.object_id == oid_a
            && pm.destination()
                == tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard),
        "PermanentMoved event for removed legend required"
    );
}

/// CR 704.5j + LTB triggers: when the legend rule removes a legend, its dies-triggers fire.
/// Kokusho, the Evening Star has a WhenSelfDies trigger that makes each opponent lose 5 life
/// and the controller gain equal life. After choosing which legend to keep, the removed
/// Kokusho's trigger must be placed on the stack.
#[test]
fn legend_rule_ltb_trigger_fires_on_removed_legend() {
    let decks = Some(vec![
        std::iter::repeat_n("kokusho,_the_evening_star".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("forest".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(43, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut e);

    let oid_a = deploy_to_battlefield(&mut e, 0, "kokusho,_the_evening_star", false);
    let _oid_b = deploy_to_battlefield(&mut e, 0, "kokusho,_the_evening_star", false);

    // Pass priority; legend SBA pauses for choice.
    let batch = e.apply_command(0, &pass()).expect("pass");
    let req = find_resolution_choice(&batch).expect("ResolutionChoiceRequired expected");
    assert_eq!(req.choice_kind(), ChoiceKind::LegendKeep);

    // P0 keeps oid_a; oid_b is sacrificed and its WhenSelfDies trigger must fire.
    let stack_before = e.state.stack.len();
    e.apply_command(0, &submit_resolution_choice(vec![oid_a]))
        .expect("submit legend keep");

    // Kokusho's death trigger (EachOpponentLosesLifeYouGainEqual) goes straight to the stack
    // (no target required). There should now be one item on the stack.
    assert_eq!(
        e.state.stack.len(),
        stack_before + 1,
        "Kokusho WhenSelfDies trigger must be on the stack"
    );
    let trigger_item = e.state.stack.last().expect("stack non-empty");
    assert_eq!(
        trigger_item.card_id, "kokusho,_the_evening_star",
        "trigger source must be kokusho"
    );
    assert!(trigger_item.is_triggered, "must be a triggered ability");

    // Let the trigger resolve: both players pass → P1 (opponent) loses 5, P0 gains 5.
    let p0_life_before = e.state.players[e.state.player_idx(0).unwrap()].life;
    let p1_life_before = e.state.players[e.state.player_idx(1).unwrap()].life;
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass → trigger resolves");
    let p0_life_after = e.state.players[e.state.player_idx(0).unwrap()].life;
    let p1_life_after = e.state.players[e.state.player_idx(1).unwrap()].life;
    assert_eq!(p1_life_after, p1_life_before - 5, "opponent loses 5 life");
    assert_eq!(p0_life_after, p0_life_before + 5, "controller gains 5 life");
}

/// When a player controls THREE or more copies of the same legendary permanent, the legend
/// SBA runs repeatedly (one conflict per SBA pass) until only one remains.
#[test]
fn legend_rule_reduces_to_one_with_multiple_copies() {
    let decks = Some(vec![
        std::iter::repeat_n("isamaru,_hound_of_konda".to_string(), 20).collect::<Vec<_>>(),
        std::iter::repeat_n("forest".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(44, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut e);

    let oid_a = deploy_to_battlefield(&mut e, 0, "isamaru,_hound_of_konda", false);
    let oid_b = deploy_to_battlefield(&mut e, 0, "isamaru,_hound_of_konda", false);
    let oid_c = deploy_to_battlefield(&mut e, 0, "isamaru,_hound_of_konda", false);

    // First legend SBA: three copies → pause for first choice (candidates = [a, b, c]).
    let batch = e.apply_command(0, &pass()).expect("pass");
    let req = find_resolution_choice(&batch).expect("first ResolutionChoiceRequired");
    assert_eq!(req.candidate_object_ids.len(), 3);

    // P0 keeps oid_c, removing oid_a and oid_b in ONE choice? No — CR 704.5j says
    // "choose one to keep" meaning the rest all go. Let's verify: candidates include all 3,
    // min=max=1 → keep exactly one, the other two go to graveyard.
    let keep_batch = e
        .apply_command(0, &submit_resolution_choice(vec![oid_c]))
        .expect("keep oid_c");
    let _ = keep_batch;

    let p0_idx = e.state.player_idx(0).unwrap();
    // After choosing oid_c, the other two (oid_a and oid_b) must be gone.
    let isamaru_on_bf: Vec<u32> = e.state.players[p0_idx]
        .battlefield
        .iter()
        .copied()
        .filter(|&oid| {
            e.state
                .objects
                .get(&oid)
                .map(|o| o.card_id == "isamaru,_hound_of_konda")
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        isamaru_on_bf,
        vec![oid_c],
        "exactly one Isamaru must remain after all duplicates removed"
    );
    assert!(
        e.state.players[p0_idx].graveyard.contains(&oid_a),
        "oid_a must be in graveyard"
    );
    assert!(
        e.state.players[p0_idx].graveyard.contains(&oid_b),
        "oid_b must be in graveyard"
    );
}

/// The legend rule does not apply to non-legendary permanents.
#[test]
fn legend_rule_does_not_apply_to_non_legendary() {
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("forest".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(45, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut e);

    // Two Grizzly Bears on the battlefield — not legendary, no legend SBA.
    let _oid_a = deploy_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let _oid_b = deploy_to_battlefield(&mut e, 0, "grizzly_bears", false);

    // Passing priority must NOT emit ResolutionChoiceRequired.
    let batch = e.apply_command(0, &pass()).expect("pass");
    let req = find_resolution_choice(&batch);
    assert!(
        req.is_none(),
        "non-legendary duplicates must not trigger legend SBA"
    );
    // Both must stay on the battlefield.
    assert_eq!(
        e.state.players[e.state.player_idx(0).unwrap()]
            .battlefield
            .len(),
        2
    );
}
