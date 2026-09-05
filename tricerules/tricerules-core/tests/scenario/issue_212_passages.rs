use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_event::Ev, ChoiceCandidateSourceZone, ChoiceKind};

fn setup_passage(card_id: &str, extra_cards: &[&str]) -> (GameEngine, u32) {
    let mut specials = vec![card_id];
    specials.extend_from_slice(extra_cards);
    let decks = Some(vec![
        deck_with("forest", &specials),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(21200, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let passage = relocate_to_battlefield(&mut engine, 0, card_id, false);
    (engine, passage)
}

fn resolve_to_search(engine: &mut GameEngine, passage: u32) -> u32 {
    engine
        .apply_command(0, &activate_ability(passage, 0, vec![]))
        .expect("activate Passage");
    assert_eq!(engine.state.objects[&passage].zone, Zone::Graveyard);
    pass_both_players(engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("library search");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::LibrarySearch);
    *pending
        .presentation
        .candidates
        .first()
        .expect("basic land search candidate")
}

#[test]
fn fabled_passage_untaps_only_the_found_land_when_four_lands_are_controlled() {
    let (mut engine, passage) = setup_passage("fabled_passage", &[]);
    for _ in 0..3 {
        relocate_to_battlefield(&mut engine, 0, "forest", false);
    }
    let found = resolve_to_search(&mut engine, passage);
    engine
        .apply_command(0, &submit_resolution_choice(vec![found]))
        .expect("find basic land");

    assert_eq!(engine.state.objects[&found].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&found].tapped);
}

#[test]
fn fabled_passage_leaves_the_found_land_tapped_below_four_lands() {
    let (mut engine, passage) = setup_passage("fabled_passage", &[]);
    for _ in 0..2 {
        relocate_to_battlefield(&mut engine, 0, "forest", false);
    }
    let found = resolve_to_search(&mut engine, passage);
    engine
        .apply_command(0, &submit_resolution_choice(vec![found]))
        .expect("find basic land");

    assert!(engine.state.objects[&found].tapped);
}

#[test]
fn elven_passage_pays_life_and_can_behold_an_elf_card_from_hand() {
    let (mut engine, passage) = setup_passage("elven_passage", &["safewright_cavalry"]);
    let elf = relocate_to_hand(&mut engine, 0, "safewright_cavalry");

    let found = resolve_to_search(&mut engine, passage);
    assert_eq!(engine.state.players[0].life, 19);
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![found]))
        .expect("find basic land");
    let choice = find_resolution_choice(&batch).expect("Behold choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::Behold);
    let elf_index = choice
        .candidate_object_ids
        .iter()
        .position(|candidate| *candidate == elf)
        .expect("Elf card candidate");
    assert_eq!(
        choice.candidate_source_zones[elf_index],
        ChoiceCandidateSourceZone::Hand as i32
    );
    assert!(engine.state.objects[&found].tapped);

    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![elf]))
        .expect("behold Elf from hand");
    assert_eq!(engine.state.objects[&elf].zone, Zone::Hand);
    assert!(!engine.state.objects[&found].tapped);
    let reveal = batch
        .events
        .iter()
        .find_map(|event| match event.ev.as_ref() {
            Some(Ev::CardsRevealed(reveal)) => Some(reveal),
            _ => None,
        })
        .expect("public hand reveal event");
    assert_eq!(reveal.zone_owner_player_id, 0);
    assert_eq!(reveal.source_zone(), ChoiceCandidateSourceZone::Hand);
    assert_eq!(reveal.cards.len(), 1);
    assert_eq!(reveal.cards[0].object_id, elf);
    assert_eq!(reveal.cards[0].card_id, "safewright_cavalry");
}

#[test]
fn elven_passage_can_decline_behold_and_leave_the_found_land_tapped() {
    let (mut engine, passage) = setup_passage("elven_passage", &["safewright_cavalry"]);
    relocate_to_hand(&mut engine, 0, "safewright_cavalry");
    let found = resolve_to_search(&mut engine, passage);
    engine
        .apply_command(0, &submit_resolution_choice(vec![found]))
        .expect("find basic land");
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("decline Behold");

    assert!(engine.state.objects[&found].tapped);
}

#[test]
fn elven_passage_can_behold_an_elf_permanent() {
    let (mut engine, passage) = setup_passage("elven_passage", &[]);
    let elf = inject_permanent_on_battlefield(&mut engine, 0, "safewright_cavalry");
    let found = resolve_to_search(&mut engine, passage);
    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![found]))
        .expect("find basic land");
    let choice = find_resolution_choice(&batch).expect("Behold choice");
    let elf_index = choice
        .candidate_object_ids
        .iter()
        .position(|candidate| *candidate == elf)
        .expect("Elf permanent candidate");
    assert_eq!(
        choice.candidate_source_zones[elf_index],
        ChoiceCandidateSourceZone::Battlefield as i32
    );

    let batch = engine
        .apply_command(0, &submit_resolution_choice(vec![elf]))
        .expect("behold Elf permanent");
    assert!(!engine.state.objects[&found].tapped);
    assert!(!batch
        .events
        .iter()
        .any(|event| matches!(event.ev, Some(Ev::CardsRevealed(_)))));
}

#[test]
fn behold_rejects_a_stale_generation_and_keeps_the_choice_pending() {
    let (mut engine, passage) = setup_passage("elven_passage", &["safewright_cavalry"]);
    let elf = relocate_to_hand(&mut engine, 0, "safewright_cavalry");
    let found = resolve_to_search(&mut engine, passage);
    engine
        .apply_command(0, &submit_resolution_choice(vec![found]))
        .expect("find basic land");
    engine.state.zone_change_generation.insert(elf, 1);

    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![elf]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.state.objects[&found].tapped);
}

#[test]
fn elven_passage_cannot_activate_without_enough_life() {
    let (mut engine, passage) = setup_passage("elven_passage", &[]);
    engine.state.players[0].life = 0;

    assert!(engine
        .apply_command(0, &activate_ability(passage, 0, vec![]))
        .is_err());
    assert_eq!(engine.state.objects[&passage].zone, Zone::Battlefield);
    assert!(!engine.state.objects[&passage].tapped);
}
