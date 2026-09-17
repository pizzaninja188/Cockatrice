//! Issue #350 — Harmonize alternative cast-method costs on generated faces.
//!
//! These scenarios drive the generated `harmonize_cost` definitions through the authoritative
//! command path. CR 702.180 governs Harmonize's graveyard alternative cost, the tap-a-creature
//! generic reduction by power, and the exile-instead-of-graveyard on stack exit; CR 601.2 / 202.3
//! govern the printed-cost normal cast. The shared engine semantics (owner-only candidates,
//! atomic forged-payment rejection, current-priority timing, and command replay) are already
//! covered by `issue_105_harmonize`; these cases only prove the generated definitions reach them.

use super::helpers::*;
use tricerules_core::state::SpellCastMethod;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cast_cost_group_selection::SelectedObject, CastCostGroupSelection, CastCostOptionKind,
    CastMethod, CastSpell, ChoiceKind, RuledCommand,
};

fn harmonize_cast(
    engine: &GameEngine,
    object_id: u32,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
    tap: Option<(u32, u64)>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(graveyard_cast_source(
                object_id,
                engine
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
            )),
            cast_method: CastMethod::Harmonize as i32,
            targets,
            cast_cost_group_selections: tap
                .map(|(object_id, generation)| {
                    vec![CastCostGroupSelection {
                        group_index: 0,
                        option_index: 0,
                        selected_object: Some(SelectedObject::PermanentId(object_id)),
                        expected_zone_change_generation: generation,
                        battlefield_objects: None,
                    }]
                })
                .unwrap_or_default(),
            ..Default::default()
        })),
    }
}

#[test]
fn issue_350_roamers_routine_harmonize_reduces_and_searches_then_exiles() {
    let decks = Some(vec![
        deck_with("forest", &["roamers_routine", "grizzly_bears"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(350_001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);

    let routine = take_oid_from_library_or_hand(&mut engine, 0, "roamers_routine");
    engine.state.players[0].graveyard.push(routine);
    engine.state.objects.get_mut(&routine).unwrap().zone = Zone::Graveyard;
    let basic = inject_library_card(&mut engine, 0, "forest");
    let tap_bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let tap_generation = engine
        .state
        .zone_change_generation
        .get(&tap_bear)
        .copied()
        .unwrap_or(0);

    // A mana-ability activation returns a legal batch containing the owner's Harmonize action.
    let untapped_forest = relocate_to_battlefield(&mut engine, 0, "forest", false);
    let batch = engine
        .apply_command(0, &activate_ability(untapped_forest, 0, vec![]))
        .expect("activate Forest for green");
    let action = batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| {
            action.object_id == routine && action.cast_method == CastMethod::Harmonize as i32
        })
        .expect("owner Harmonize action");
    assert_eq!(action.cost, "{4}{G}");
    let group = &action.cost_choices.as_ref().unwrap().cast_cost_groups[0];
    assert_eq!(
        group.options[0].kind,
        CastCostOptionKind::TapPermanentForGenericReduction as i32
    );
    assert!(group.options[0].valid_permanent_ids.contains(&tap_bear));
    assert_eq!(group.options[0].valid_permanent_generic_reductions, [2]);

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
            &harmonize_cast(&engine, routine, vec![], Some((tap_bear, tap_generation))),
        )
        .expect("pay {2}{G} and tap the summoning-sick Bear");
    assert!(engine.state.objects[&tap_bear].tapped);
    assert_eq!(
        engine.state.stack.last().unwrap().cast_method,
        SpellCastMethod::Harmonize
    );

    engine.apply_command(0, &pass()).expect("caster passes");
    let search_batch = engine
        .apply_command(1, &pass())
        .expect("opponent passes into resolution");
    let choice = find_resolution_choice(&search_batch).expect("basic-land search");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(choice.candidate_object_ids.contains(&basic));
    engine
        .apply_command(0, &submit_resolution_choice(vec![basic]))
        .expect("choose the basic land");
    assert_eq!(engine.state.objects[&basic].zone, Zone::Battlefield);
    assert!(
        engine.state.objects[&basic].tapped,
        "searched land enters tapped"
    );
    assert_eq!(engine.state.objects[&routine].zone, Zone::Exile);
    assert!(!engine.state.players[0].graveyard.contains(&routine));
}

#[test]
fn issue_350_urenis_rebuff_harmonize_returns_a_creature_then_exiles() {
    let decks = Some(vec![
        deck_with(
            "island",
            &["urenis_rebuff", "grizzly_bears", "grizzly_bears"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(350_002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);

    let rebuff = take_oid_from_library_or_hand(&mut engine, 0, "urenis_rebuff");
    engine.state.players[0].graveyard.push(rebuff);
    engine.state.objects.get_mut(&rebuff).unwrap().zone = Zone::Graveyard;
    let target_bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let tap_bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let tap_generation = engine
        .state
        .zone_change_generation
        .get(&tap_bear)
        .copied()
        .unwrap_or(0);

    let untapped_island = relocate_to_battlefield(&mut engine, 0, "island", false);
    engine
        .apply_command(0, &activate_ability(untapped_island, 0, vec![]))
        .expect("activate Island for blue");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    engine
        .apply_command(
            0,
            &harmonize_cast(
                &engine,
                rebuff,
                target_object(target_bear),
                Some((tap_bear, tap_generation)),
            ),
        )
        .expect("pay {3}{U} and tap the Bear");
    assert!(engine.state.objects[&tap_bear].tapped);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target_bear].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&rebuff].zone, Zone::Exile);
    assert!(!engine.state.players[0].graveyard.contains(&rebuff));
}

#[test]
fn issue_350_urenis_rebuff_normal_cast_uses_the_printed_cost_and_buries_the_spell() {
    let decks = Some(vec![
        deck_with("island", &["urenis_rebuff", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(350_003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);

    ensure_card_in_hand(&mut engine, 0, "urenis_rebuff");
    let rebuff = engine.state.players[0].hand[hand_index_for_card(&engine, 0, "urenis_rebuff")];
    let target_bear = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "urenis_rebuff");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target_bear)))
        .expect("normal cast at {1}{U}");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target_bear].zone, Zone::Hand);
    assert_eq!(
        engine.state.objects[&rebuff].zone,
        Zone::Graveyard,
        "a normal hand cast is not replaced by Harmonize's stack-exit exile (CR 702.180a)"
    );
}
