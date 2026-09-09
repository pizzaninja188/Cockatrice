use super::helpers::*;
use tricerules_cards::{
    Color, ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineReplacement,
};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::{TurnStep, Zone};

const HYDRO: &str = "hydro-man,_fluid_felon";

fn engine(players: &[i32]) -> GameEngine {
    let decks = players
        .iter()
        .map(|_| vec!["island".to_string(); 20])
        .collect();
    let mut engine = GameEngine::new(236_001, players, 20, Some(decks), true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_from_injected_hand(engine: &mut GameEngine, player: usize, card_id: &str) {
    inject_card_into_hand(engine, player, card_id);
    let slot = hand_index_for_card(engine, player, card_id);
    engine
        .apply_command(player as i32, &cast_spell(slot, vec![]))
        .expect("cast spell");
}

fn cast_from_injected_hand_with_targets(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    targets: Vec<TargetRef>,
) {
    inject_card_into_hand(engine, player, card_id);
    let slot = hand_index_for_card(engine, player, card_id);
    engine
        .apply_command(player as i32, &cast_spell(slot, targets))
        .expect("cast spell");
}

fn resolve_stack(engine: &mut GameEngine) {
    while !engine.state.stack.is_empty() {
        answer_trigger_order_in_engine_order(engine);
        let player = engine.state.priority_player_id();
        engine
            .apply_command(player, &pass())
            .expect("priority pass");
    }
}

fn advance_to_end_step(engine: &mut GameEngine, active: i32) {
    engine.apply_command(active, &primitive_yield()).unwrap();
    engine.apply_command(active, &primitive_yield()).unwrap();
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine.apply_command(active, &primitive_yield()).unwrap();
    }
    engine.apply_command(active, &primitive_yield()).unwrap();
    engine.apply_command(active, &primitive_yield()).unwrap();
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

fn roll_from_end_step(engine: &mut GameEngine, active: i32) {
    engine.apply_command(active, &primitive_yield()).unwrap();
    resolve_cleanup_discards_if_any(engine);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
}

fn battlefield_view(
    engine: &mut GameEngine,
    player: usize,
    oid: u32,
) -> tricerules_proto::ruled::v1::BattlefieldObject {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view.per_player.get(player),
            _ => None,
        })
        .and_then(|view| {
            view.battlefield_objects
                .iter()
                .find(|object| object.object_id == oid)
        })
        .cloned()
        .expect("battlefield object view")
}

#[test]
fn issue_236_blue_and_multicolor_spells_pump_only_the_casters_creature() {
    for (spell, mana) in [
        (
            "fugitive_wizard",
            ManaGift {
                u: 1,
                ..Default::default()
            },
        ),
        (
            "mantis_rider",
            ManaGift {
                w: 1,
                u: 1,
                r: 1,
                ..Default::default()
            },
        ),
    ] {
        let mut engine = engine(&[0, 1]);
        let hydro = inject_creature_on_battlefield(&mut engine, 0, HYDRO);
        give_mana(&mut engine, 0, mana);
        cast_from_injected_hand(&mut engine, 0, spell);
        resolve_stack(&mut engine);
        assert_eq!(
            engine.characteristics(hydro).unwrap().power,
            Some(3),
            "{spell}"
        );
    }

    for (caster, spell) in [(0, "ornithopter"), (1, "reach_through_mists")] {
        let mut engine = engine(&[0, 1]);
        let hydro = inject_creature_on_battlefield(&mut engine, 0, HYDRO);
        give_mana(
            &mut engine,
            caster,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        if caster == 0 {
            cast_from_injected_hand(&mut engine, 0, spell);
        } else {
            engine
                .apply_command(0, &pass())
                .expect("pass priority to opponent");
            cast_from_injected_hand_with_targets(&mut engine, 1, spell, vec![]);
        }
        resolve_stack(&mut engine);
        assert_eq!(
            engine.characteristics(hydro).unwrap().power,
            Some(2),
            "P{caster} casts {spell}"
        );
    }
}

#[test]
fn issue_236_uncast_blue_spell_copy_does_not_trigger_and_pump_expires_at_cleanup() {
    let mut engine = engine(&[0, 1]);
    let hydro = inject_creature_on_battlefield(&mut engine, 0, HYDRO);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 5,
            ..Default::default()
        },
    );

    cast_from_injected_hand(&mut engine, 0, "divination");
    let divination = engine
        .state
        .stack
        .iter()
        .find(|item| item.card_id == "divination")
        .expect("Divination on the stack")
        .id;
    cast_from_injected_hand_with_targets(&mut engine, 0, "twincast", target_object(divination));
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.characteristics(hydro).unwrap().power,
        Some(4),
        "the two casts pump Hydro-Man, but the uncast Divination copy does not"
    );
    advance_to_end_step(&mut engine, 0);
    resolve_stack(&mut engine);
    roll_from_end_step(&mut engine, 0);
    assert!(
        engine.state.continuous_effects.iter().all(|effect| {
            !matches!(
                (&effect.affected, &effect.kind),
                (AffectedScope::Single(affected), ContinuousEffectKind::PtModify { .. })
                    if *affected == hydro
            )
        }),
        "both cast-trigger pumps expire during cleanup"
    );
}

#[test]
fn issue_236_blue_spell_pump_rechecks_that_hydro_man_is_a_creature() {
    let mut engine = engine(&[0, 1]);
    let hydro = inject_creature_on_battlefield(&mut engine, 0, HYDRO);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    cast_from_injected_hand(&mut engine, 0, "fugitive_wizard");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "the blue cast creates Hydro-Man's trigger"
    );

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(hydro),
        kind: ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            card_types: vec![PermanentTypeFilter::Land],
            creature_types: vec![],
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    resolve_stack(&mut engine);

    assert_eq!(
        engine.characteristics(hydro).unwrap().power,
        None,
        "the intervening-if condition suppresses the pump when Hydro-Man stops being a creature"
    );
}

#[test]
fn issue_236_end_step_form_is_public_activatable_and_expires_before_next_untap() {
    let mut engine = engine(&[0, 1]);
    let hydro = inject_creature_on_battlefield(&mut engine, 0, HYDRO);
    engine.state.objects.get_mut(&hydro).unwrap().tapped = true;
    engine.state.objects.get_mut(&hydro).unwrap().summoning_sick = true;

    advance_to_end_step(&mut engine, 0);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_stack(&mut engine);

    let characteristics = engine.characteristics(hydro).unwrap();
    assert!(characteristics.has_type("Land"));
    assert!(!characteristics.is_creature());
    assert!(characteristics.is_legendary());
    assert_eq!(characteristics.colors, vec![Color::Blue]);
    assert!(
        !engine.state.objects[&hydro].tapped,
        "the instruction untaps first"
    );
    let view = battlefield_view(&mut engine, 0, hydro);
    assert!(view.is_land && !view.is_creature);
    assert_eq!(view.activated_abilities.len(), 1);
    assert_eq!(view.activated_abilities[0].text, "{T}: Add {U}.");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    cast_from_injected_hand(&mut engine, 0, "reach_through_mists");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "a blue cast does not trigger while Hydro-Man is not a creature"
    );
    resolve_stack(&mut engine);

    let activate = activate_ability_for(&engine, hydro, 0, vec![]);
    engine
        .apply_command(0, &activate)
        .expect("a summoning-sick noncreature land may activate its tap ability");
    assert_eq!(engine.state.players[0].mana_pool.blue, 1);

    roll_from_end_step(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 1);

    assert_eq!(engine.state.active_player_id(), 0);
    assert_eq!(engine.state.turn_step, TurnStep::Upkeep);
    let characteristics = engine.characteristics(hydro).unwrap();
    assert!(characteristics.is_creature());
    assert!(!characteristics.has_type("Land"));
    assert!(battlefield_view(&mut engine, 0, hydro)
        .activated_abilities
        .is_empty());
}

#[test]
fn issue_236_expiry_controller_is_fixed_while_activation_follows_current_control() {
    let mut engine = engine(&[0, 1]);
    let hydro = inject_creature_on_battlefield(&mut engine, 0, HYDRO);
    advance_to_end_step(&mut engine, 0);
    resolve_stack(&mut engine);

    engine.state.players[0]
        .battlefield
        .retain(|candidate| *candidate != hydro);
    engine.state.players[1].battlefield.push(hydro);
    let object = engine.state.objects.get_mut(&hydro).unwrap();
    object.base_controller = 1;
    object.controller = 1;

    roll_from_end_step(&mut engine, 0);
    assert_eq!(engine.state.active_player_id(), 1);
    assert!(
        engine.characteristics(hydro).unwrap().has_type("Land"),
        "the form does not expire on the new controller's turn"
    );
    let activate = activate_ability_for(&engine, hydro, 0, vec![]);
    engine
        .apply_command(1, &activate)
        .expect("the current controller activates the granted mana ability");
    assert_eq!(engine.state.players[1].mana_pool.blue, 1);

    engine.state.players[1]
        .battlefield
        .retain(|candidate| *candidate != hydro);
    engine.state.players[0].battlefield.push(hydro);
    let object = engine.state.objects.get_mut(&hydro).unwrap();
    object.base_controller = 0;
    object.controller = 0;
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 1);
    assert_eq!(engine.state.active_player_id(), 0);
    assert!(engine.characteristics(hydro).unwrap().is_creature());
}

#[test]
fn issue_236_land_form_does_not_survive_a_zone_change_or_reentry() {
    let mut engine = engine(&[0, 1]);
    let hydro = inject_creature_on_battlefield(&mut engine, 0, HYDRO);
    advance_to_end_step(&mut engine, 0);
    resolve_stack(&mut engine);
    assert!(engine.characteristics(hydro).unwrap().has_type("Land"));

    inject_card_into_hand(&mut engine, 1, "boomerang");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    engine.apply_command(0, &pass()).expect("pass to opponent");
    let slot = hand_index_for_card(&engine, 1, "boomerang");
    engine
        .apply_command(1, &cast_spell(slot, target_object(hydro)))
        .expect("cast Boomerang");
    resolve_stack(&mut engine);
    assert_eq!(engine.state.objects[&hydro].zone, Zone::Hand);
    assert!(engine.state.continuous_effects.iter().all(|effect| {
        !matches!(effect.affected, AffectedScope::Single(affected) if affected == hydro)
    }));

    roll_from_end_step(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    end_active_turn(&mut engine, 1);
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, HYDRO);
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("recast Hydro-Man");
    resolve_stack(&mut engine);

    assert!(engine.characteristics(hydro).unwrap().is_creature());
    assert!(!engine.characteristics(hydro).unwrap().has_type("Land"));
    assert!(battlefield_view(&mut engine, 0, hydro)
        .activated_abilities
        .is_empty());
}
