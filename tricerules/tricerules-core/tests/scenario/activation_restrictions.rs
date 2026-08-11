use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::{ruled_event::Ev, TargetRef};

fn activation_engine(seed: u64) -> GameEngine {
    anthem_engine(seed, "mountain")
}

fn target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        damage_amount: 0,
    }]
}

#[test]
fn celestial_enforcer_requires_a_flying_creature_controlled_by_its_controller() {
    let mut e = activation_engine(5401);
    let enforcer = inject_creature_on_battlefield(&mut e, 0, "celestial_enforcer");
    let target_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    assert_eq!(zone_view_ability_flags(&mut e, 0, enforcer), [false]);
    let err = e
        .apply_command(0, &activate_ability(enforcer, 0, target(target_creature)))
        .expect_err("the engine must reject activation while its condition is false");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));

    inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    assert_eq!(
        zone_view_ability_flags(&mut e, 0, enforcer),
        [false],
        "an opponent's flying creature does not satisfy 'you control'"
    );

    inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    assert_eq!(zone_view_ability_flags(&mut e, 0, enforcer), [true]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    e.apply_command(0, &activate_ability(enforcer, 0, target(target_creature)))
        .expect("the activation becomes legal after its condition is true");
    assert!(e.state.objects.get(&enforcer).expect("enforcer").tapped);
}

#[test]
fn goblin_bird_grabber_uses_the_same_condition_and_never_opens_target_selection() {
    let mut e = activation_engine(5402);
    let goblin = inject_creature_on_battlefield(&mut e, 0, "goblin_bird-grabber");
    assert_eq!(zone_view_ability_flags(&mut e, 0, goblin), [false]);

    inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    assert_eq!(zone_view_ability_flags(&mut e, 0, goblin), [true]);
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    e.apply_command(0, &activate_ability(goblin, 0, vec![]))
        .expect("source-bound keyword grants take no targets");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.characteristics(goblin)
            .expect("goblin characteristics")
            .has_keyword(Keyword::Flying),
        "the resolving ability grants flying to its physical source"
    );
}

#[test]
fn caged_zombie_tracks_committed_creature_deaths_for_ui_and_command_legality() {
    let mut e = activation_engine(5403);
    let zombie = inject_creature_on_battlefield(&mut e, 0, "caged_zombie");
    let gnomes = inject_creature_on_battlefield(&mut e, 0, "bottle_gnomes");
    assert_eq!(zone_view_ability_flags(&mut e, 0, zombie), [false]);

    e.apply_command(0, &activate_ability(gnomes, 0, vec![]))
        .expect("sacrifice Bottle Gnomes");
    assert_eq!(e.state.turn_history.current.creatures_died, 1);
    assert_eq!(zone_view_ability_flags(&mut e, 0, zombie), [true]);

    resolve_entire_stack_two_player(&mut e);
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    e.apply_command(0, &activate_ability(zombie, 0, vec![]))
        .expect("Caged Zombie is legal after a committed creature death");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 18);

    e.state.turn_history.finish_turn();
    assert_eq!(zone_view_ability_flags(&mut e, 0, zombie), [false]);
}

#[test]
fn activation_history_changes_invalidate_the_battlefield_view_cache() {
    let mut e = activation_engine(5404);
    let zombie = inject_creature_on_battlefield(&mut e, 0, "caged_zombie");
    e.initial_response_batch();

    // Isolate the public history input from an accompanying battlefield move. The next ordinary
    // command must still publish fresh ability flags instead of claiming the battlefield view is
    // unchanged.
    e.state.turn_history.current.creatures_died = 1;
    let batch = e.apply_command(0, &pass()).expect("pass priority");
    let view = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .expect("every settled batch publishes a zone view");
    assert!(!view.battlefields_unchanged);
    let zombie_view = view.per_player[0]
        .battlefield_objects
        .iter()
        .find(|object| object.object_id == zombie)
        .expect("Caged Zombie in refreshed battlefield view");
    assert_eq!(
        zombie_view
            .activated_abilities
            .iter()
            .map(|ability| ability.activatable)
            .collect::<Vec<_>>(),
        [true]
    );
}
