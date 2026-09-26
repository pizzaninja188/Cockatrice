use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::permanent_moved::Destination;
use tricerules_proto::ruled::v1::ruled_event::Ev;

#[test]
fn sakura_tribe_elder_can_be_sacrificed_while_summoning_sick_to_find_a_tapped_basic_land() {
    let mut engine = GameEngine::new(906_101, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let elder = inject_card_into_hand(&mut engine, 0, "sakura-tribe_elder");
    grant_pool(&mut engine, 0);
    let hand_index = hand_index_for_card(&engine, 0, "sakura-tribe_elder");
    engine
        .apply_command(0, &cast_spell(hand_index, vec![]))
        .expect("cast Sakura-Tribe Elder");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&elder].zone, Zone::Battlefield);
    assert!(
        engine.state.objects[&elder].summoning_sick,
        "the creature entered this turn"
    );

    let forest = inject_library_card(&mut engine, 0, "forest");
    let taiga = inject_library_card(&mut engine, 0, "taiga");
    let generation_before = engine.state.zone_change_generation[&elder];
    engine
        .apply_command(0, &activate_ability_for(&engine, elder, 0, vec![]))
        .expect("the sacrifice-only ability can be activated while summoning sick");

    assert_eq!(engine.state.objects[&elder].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&elder],
        generation_before + 1
    );
    assert_eq!(engine.state.stack.len(), 1);

    engine.apply_command(0, &pass()).expect("controller passes");
    let search_batch = engine
        .apply_command(1, &pass())
        .expect("opponent passes and the ability resolves");
    let choice = find_resolution_choice(&search_batch).expect("basic-land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(choice.candidate_object_ids.contains(&forest));
    assert!(
        !choice.candidate_object_ids.contains(&taiga),
        "Taiga is a land but not a basic land"
    );

    let completion = engine
        .apply_command(0, &submit_resolution_choice(vec![forest]))
        .expect("choose Forest");
    let object = engine.state.objects.get(&forest).expect("Forest object");
    assert_eq!(object.zone, Zone::Battlefield);
    assert!(object.tapped, "the searched-for basic land enters tapped");
    assert_eq!(object.owner, 0);
    assert_eq!(object.controller, 0);
    assert!(engine.state.players[0].battlefield.contains(&forest));
    assert!(!engine.state.players[0].library.contains(&forest));
    assert!(engine.state.players[0].library.contains(&taiga));
    assert!(permanents_moved_in(&completion).iter().any(|moved| {
        moved.object_id == forest
            && moved.owner_player_id == 0
            && moved.destination == Destination::Battlefield as i32
    }));
    assert!(completion.events.iter().any(|event| {
        matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text == "P0 shuffles their library."
        )
    }));
    assert!(engine.state.pending_resolution.is_none());
}
