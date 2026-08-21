use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand, TargetRef,
};

fn zone_ability(
    engine: &GameEngine,
    source: u32,
    source_zone: AbilitySourceZone,
    ability_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: source_zone as i32,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index,
            targets,
            ..Default::default()
        })),
    }
}

#[test]
fn zone_actions_are_stable_owner_only_and_generation_bound() {
    let mut engine = anthem_engine(10_101, "forest");
    let spirits = inject_card_into_hand(&mut engine, 0, "shepherding_spirits");
    let pummeler = inject_graveyard_card(&mut engine, 0, "sagu_pummeler");

    let batch = engine.initial_response_batch();
    let owner_actions = &batch.legal_by_player[&0].zone_ability_actions;
    assert_eq!(owner_actions.len(), 2);
    let hand = owner_actions
        .iter()
        .find(|action| action.object_id == spirits)
        .expect("hand typecycling action");
    assert_eq!(hand.source_zone(), AbilitySourceZone::Hand);
    assert_eq!(hand.hand_index, Some(7));
    assert_eq!(hand.zone_change_generation, 0);
    assert!(hand
        .ability
        .as_ref()
        .is_some_and(|ability| ability.activatable));
    let graveyard = owner_actions
        .iter()
        .find(|action| action.object_id == pummeler)
        .expect("graveyard renew action");
    assert_eq!(graveyard.source_zone(), AbilitySourceZone::Graveyard);
    assert_eq!(graveyard.hand_index, None);
    assert!(batch.legal_by_player[&1].zone_ability_actions.is_empty());

    let mut stale = zone_ability(&engine, spirits, AbilitySourceZone::Hand, 0, vec![]);
    let Some(Cmd::ActivateAbility(command)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    command.expected_zone_change_generation = 99;
    engine
        .apply_command(0, &stale)
        .expect_err("a stale published action is rejected");
    assert_eq!(engine.state.objects[&spirits].zone, Zone::Hand);
}

#[test]
fn typecycling_discards_then_privately_searches_for_the_land_subtype() {
    let mut engine = anthem_engine(10_102, "forest");
    let spirits = inject_card_into_hand(&mut engine, 0, "shepherding_spirits");
    let plains = inject_library_card(&mut engine, 0, "plains");
    let forest = inject_library_card(&mut engine, 0, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = zone_ability(&engine, spirits, AbilitySourceZone::Hand, 0, vec![]);

    engine
        .apply_command(0, &command)
        .expect("activate Plainscycling");
    assert_eq!(engine.state.objects[&spirits].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1);
    engine
        .apply_command(0, &pass())
        .expect("controller passes priority");
    let search = engine
        .apply_command(1, &pass())
        .expect("opponent passes and typecycling resolves");
    let choice = find_resolution_choice(&search).expect("private library search");
    assert_eq!(choice.candidate_object_ids, [plains]);
    assert!(!choice.candidate_object_ids.contains(&forest));

    engine
        .apply_command(0, &submit_resolution_choice(vec![plains]))
        .expect("choose and reveal Plains");
    assert_eq!(engine.state.objects[&plains].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&plains));
    engine
        .apply_command(0, &command)
        .expect_err("the old hand generation cannot be replayed");
}

#[test]
fn typecycling_with_no_matching_subtype_parks_for_an_explicit_fail_to_find() {
    let mut engine = anthem_engine(10_104, "forest");
    let spirits = inject_card_into_hand(&mut engine, 0, "shepherding_spirits");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    engine
        .apply_command(
            0,
            &zone_ability(&engine, spirits, AbilitySourceZone::Hand, 0, vec![]),
        )
        .expect("activate Plainscycling");
    engine.apply_command(0, &pass()).expect("controller passes");
    let search = engine
        .apply_command(1, &pass())
        .expect("opponent passes and opens the empty search");
    let choice = find_resolution_choice(&search).expect("empty library search still published");
    assert!(choice.candidate_object_ids.is_empty());
    assert_eq!(choice.min, 0);
    assert_eq!(choice.max, 1);
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("submit fail to find");
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&spirits].zone, Zone::Graveyard);
}

#[test]
fn renew_exiles_atomically_and_applies_all_counters_to_one_target() {
    let mut engine = anthem_engine(10_103, "forest");
    let pummeler = inject_graveyard_card(&mut engine, 0, "sagu_pummeler");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 5,
            ..Default::default()
        },
    );

    let invalid = zone_ability(
        &engine,
        pummeler,
        AbilitySourceZone::Graveyard,
        0,
        target_object(999_999),
    );
    engine
        .apply_command(0, &invalid)
        .expect_err("invalid target rejects before costs");
    assert_eq!(engine.state.objects[&pummeler].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.green, 5);

    let activate = zone_ability(
        &engine,
        pummeler,
        AbilitySourceZone::Graveyard,
        0,
        target_object(target),
    );
    engine.apply_command(0, &activate).expect("activate renew");
    assert_eq!(engine.state.objects[&pummeler].zone, Zone::Exile);
    pass_both_players(&mut engine);
    let object = &engine.state.objects[&target];
    assert_eq!(object.counter_count(CounterKind::PlusOnePlusOne), 2);
    assert_eq!(
        object.counter_count(CounterKind::Keyword(Keyword::Reach)),
        1
    );
}
