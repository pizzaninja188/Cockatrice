//! Investigate keyword action (#339) on shipped cards.
//!
//! CR 701.16a: to investigate is to create a Clue token (CR 111.10f). Investigate is its own
//! keyword action, reported separately from the Clue creation it performs; token replacement
//! effects modify the Clue creation inside one action.

use super::helpers::*;
use tricerules_proto::ruled::v1::ruled_event::Ev;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn logs(batch: &RuledEventBatch) -> Vec<&str> {
    batch
        .events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::Log(log)) => Some(log.text.as_str()),
            _ => None,
        })
        .collect()
}

fn cast_deduce(e: &mut GameEngine) -> RuledEventBatch {
    inject_card_into_hand(e, 0, "deduce");
    let slot = hand_index_for_card(e, 0, "deduce");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Deduce");
    pass_both_players_with_batch(e)
}

#[test]
fn deduce_investigates_once_and_creates_one_clue() {
    let mut e = engine(339_001);
    let batch = cast_deduce(&mut e);
    let logs = logs(&batch);
    assert_eq!(
        logs.iter()
            .filter(|line| **line == "P0 investigates (Deduce).")
            .count(),
        1,
        "one investigate action: {logs:?}"
    );
    let clue_log = logs
        .iter()
        .position(|line| line.starts_with("P0 creates 1 Clue token"))
        .expect("Clue creation log");
    let investigate_log = logs
        .iter()
        .position(|line| *line == "P0 investigates (Deduce).")
        .unwrap();
    assert!(
        clue_log < investigate_log,
        "the action completes after its Clue is created: {logs:?}"
    );
    assert_eq!(token_created_events(&batch).len(), 1);
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 1);
    assert!(battlefield_token_oids(&e, 1, "clue").is_empty());
}

#[test]
fn token_doubling_applies_inside_one_investigate_action() {
    let mut e = engine(339_002);
    inject_permanent_on_battlefield(&mut e, 0, "doubling_season");
    let batch = cast_deduce(&mut e);
    let logs = logs(&batch);
    assert_eq!(
        logs.iter()
            .filter(|line| line.starts_with("P0 investigates"))
            .count(),
        1,
        "a doubled Clue is still one investigate action: {logs:?}"
    );
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 2);
}

#[test]
fn investigate_land_activation_reports_the_action() {
    let mut e = engine(339_003);
    let land = inject_permanent_on_battlefield(&mut e, 0, "study");
    e.state.objects.get_mut(&land).expect("Study").tapped = false;
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
        .expect("activate Study's investigate ability");
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(
        logs(&batch)
            .iter()
            .filter(|line| line.starts_with("P0 investigates ("))
            .count(),
        1
    );
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 1);
}
