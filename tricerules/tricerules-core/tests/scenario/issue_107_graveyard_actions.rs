use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("island".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn grouped(ids: &[u32]) -> Vec<TargetRef> {
    ids.iter()
        .copied()
        .map(|object_id| TargetRef {
            object_id,
            group_index: 0,
            ..Default::default()
        })
        .collect()
}

fn choose_trigger_targets(ids: &[u32]) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: grouped(ids),
        })),
    }
}

fn cast_creature(engine: &mut GameEngine, card_id: &str) -> u32 {
    let object_id = inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    let index = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(index, Vec::new()))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    pass_both_players(engine);
    object_id
}

#[test]
fn arashin_sunshield_requires_one_graveyard_and_revalidates_each_target() {
    let mut engine = engine(107_001);
    let own_first = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let own_second = inject_graveyard_card(&mut engine, 0, "storm_crow");
    let opposing = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    cast_creature(&mut engine, "arashin_sunshield");

    let published = engine.initial_response_batch();
    let targets = published.legal_by_player[&0]
        .valid_targets_by_ability
        .values()
        .next()
        .expect("pending trigger targets");
    assert!(targets.groups[0].same_graveyard);
    assert_eq!(targets.groups[0].min, 0);
    assert_eq!(targets.groups[0].max, 2);
    assert!(engine
        .apply_command(0, &choose_trigger_targets(&[own_first, opposing]))
        .is_err());
    engine
        .apply_command(0, &choose_trigger_targets(&[own_first, own_second]))
        .expect("choose two cards from one graveyard");

    engine.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != own_second);
    engine.state.players[0].hand.push(own_second);
    engine
        .state
        .objects
        .get_mut(&own_second)
        .expect("card")
        .zone = Zone::Hand;
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&own_first].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&own_second].zone, Zone::Hand);
}

#[test]
fn graveyard_actions_place_cards_at_the_requested_library_end() {
    let mut top = engine(107_010);
    let top_card = inject_graveyard_card(&mut top, 0, "lightning_bolt");
    cast_creature(&mut top, "monastery_messenger");
    top.apply_command(0, &choose_trigger_targets(&[top_card]))
        .expect("choose top card");
    pass_both_players(&mut top);
    assert_eq!(top.state.players[0].library.front(), Some(&top_card));

    let mut bottom = engine(107_011);
    let bottom_card = inject_graveyard_card(&mut bottom, 1, "grizzly_bears");
    let chandelier = inject_creature_on_battlefield(&mut bottom, 0, "malevolent_chandelier");
    grant_pool(&mut bottom, 0);
    bottom
        .apply_command(0, &activate_ability(chandelier, 0, grouped(&[bottom_card])))
        .expect("activate Chandelier");
    pass_both_players(&mut bottom);
    assert_eq!(bottom.state.players[1].library.back(), Some(&bottom_card));
}

#[test]
fn soul_shackled_zombie_drains_only_when_a_creature_was_actually_exiled() {
    let mut drain = engine(107_020);
    let creature = inject_graveyard_card(&mut drain, 1, "grizzly_bears");
    let noncreature = inject_graveyard_card(&mut drain, 1, "forest");
    cast_creature(&mut drain, "soul-shackled_zombie");
    drain
        .apply_command(0, &choose_trigger_targets(&[creature, noncreature]))
        .expect("choose graveyard cohort");
    pass_both_players(&mut drain);
    assert_eq!(drain.state.players[0].life, 22);
    assert_eq!(drain.state.players[1].life, 18);

    let mut no_drain = engine(107_021);
    let stale_creature = inject_graveyard_card(&mut no_drain, 1, "grizzly_bears");
    let surviving_noncreature = inject_graveyard_card(&mut no_drain, 1, "forest");
    cast_creature(&mut no_drain, "soul-shackled_zombie");
    no_drain
        .apply_command(
            0,
            &choose_trigger_targets(&[stale_creature, surviving_noncreature]),
        )
        .expect("choose graveyard cohort");
    no_drain.state.players[1]
        .graveyard
        .retain(|object_id| *object_id != stale_creature);
    no_drain.state.players[1].hand.push(stale_creature);
    no_drain
        .state
        .objects
        .get_mut(&stale_creature)
        .expect("card")
        .zone = Zone::Hand;
    pass_both_players(&mut no_drain);
    assert_eq!(
        no_drain.state.objects[&surviving_noncreature].zone,
        Zone::Exile
    );
    assert_eq!(no_drain.state.players[0].life, 20);
    assert_eq!(no_drain.state.players[1].life, 20);
}
