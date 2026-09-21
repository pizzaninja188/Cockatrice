//! Issue #338/#376 — command-boundary scenarios for the reviewed direct-RON batch:
//! Drag to the Roots and My Precious // Allure of Power.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21. Every expectation is the
//! reviewed printed Oracle behavior.
//!
//! Governing CR concepts: 601.2b/f-h (announced additional costs and cost determination), 701.7
//! (destroy), 700.2/608.2h (delirium card types), 301.5/702.6 (Equipment and equip), 702.18
//! (hexproof), 509.1b (combat restrictions), 601.3e/715 (adventure alternative characteristics),
//! and 400.7 (zone identity).

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, CastCostGroupSelection, CastMethod, CastSpell, SelectedSpellMode,
};

fn battlefield_cost(
    engine: &GameEngine,
    option_index: u32,
    objects: &[u32],
) -> CastCostGroupSelection {
    CastCostGroupSelection {
        group_index: 0,
        option_index,
        battlefield_objects: Some(tricerules_proto::ruled::v1::CostObjectRefs {
            objects: objects
                .iter()
                .map(|oid| tricerules_proto::ruled::v1::CostObjectRef {
                    object_id: *oid,
                    zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(oid)
                        .copied()
                        .unwrap_or(0),
                })
                .collect(),
        }),
        ..Default::default()
    }
}

fn cast_adventure(
    hand_card_index: usize,
    cast_cost_group_selections: Vec<CastCostGroupSelection>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(hand_cast_source(hand_card_index)),
            face_index: 1,
            selected_modes: Vec::<SelectedSpellMode>::new(),
            cast_cost_group_selections,
            ..Default::default()
        })),
    }
}

fn engine(seed: u64, p0_extra: &[&str], p1_extra: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", p0_extra),
        deck_with("forest", p1_extra),
    ]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

#[test]
fn issue_338_drag_to_the_roots_delirium_reduction_and_nonland_target() {
    // Four card types in the graveyard reduce the generic cost by {2}.
    let mut delirium = engine(338_711, &["drag_to_the_roots"], &[]);
    let target = inject_creature_on_battlefield(&mut delirium, 1, "grizzly_bears");
    ensure_card_in_hand(&mut delirium, 0, "drag_to_the_roots");
    for card in ["grizzly_bears", "forest", "lightning_bolt", "divination"] {
        inject_graveyard_card(&mut delirium, 0, card);
    }
    let slot = hand_index_for_card(&delirium, 0, "drag_to_the_roots");
    semantic::accepted(&mut delirium, 0, &cast_spell(slot, target_object(target)));
    // {2}{B}{G} reduced by {2}: only one black and one green are spent.
    assert_eq!(delirium.state.players[0].mana_pool.black, 8);
    assert_eq!(delirium.state.players[0].mana_pool.green, 8);
    assert_eq!(delirium.state.players[0].mana_pool.colorless, 9);
    semantic::complete(&mut delirium, 12, |_| None).require_exercised();
    assert_eq!(delirium.state.objects[&target].zone, Zone::Graveyard);

    // Three card types pay the full {2}{B}{G}.
    let mut plain = engine(338_712, &["drag_to_the_roots"], &[]);
    let target = inject_creature_on_battlefield(&mut plain, 1, "grizzly_bears");
    ensure_card_in_hand(&mut plain, 0, "drag_to_the_roots");
    for card in ["grizzly_bears", "forest", "lightning_bolt"] {
        inject_graveyard_card(&mut plain, 0, card);
    }
    let slot = hand_index_for_card(&plain, 0, "drag_to_the_roots");
    semantic::accepted(&mut plain, 0, &cast_spell(slot, target_object(target)));
    assert_eq!(plain.state.players[0].mana_pool.colorless, 7);
    semantic::complete(&mut plain, 12, |_| None).require_exercised();
    assert_eq!(plain.state.objects[&target].zone, Zone::Graveyard);

    // A land is not a legal target.
    let mut land = engine(338_713, &["drag_to_the_roots"], &[]);
    let forest = inject_permanent_on_battlefield(&mut land, 1, "forest");
    ensure_card_in_hand(&mut land, 0, "drag_to_the_roots");
    let slot = hand_index_for_card(&land, 0, "drag_to_the_roots");
    assert!(
        land.apply_command(0, &cast_spell(slot, target_object(forest)))
            .is_err(),
        "a land is not a nonland permanent"
    );
}

#[test]
fn issue_338_my_precious_equip_and_adventure() {
    // Main face: cast the Equipment, equip for {2} and 2 life.
    let mut equip = engine(338_721, &["my_precious_allure_of_power"], &[]);
    let creature = inject_creature_with_stats(&mut equip, 0, "grizzly_bears", 2, 2);
    ensure_card_in_hand(&mut equip, 0, "my_precious_allure_of_power");
    let slot = hand_index_for_card(&equip, 0, "my_precious_allure_of_power");
    semantic::accepted(&mut equip, 0, &cast_spell(slot, vec![]));
    semantic::complete(&mut equip, 12, |_| None).require_exercised();
    let artifact = battlefield_object_for_card(&equip, 0, "my_precious_allure_of_power");
    let life_before = equip.state.players[0].life;
    let command = activate_ability_for(&equip, artifact, 0, target_object(creature));
    semantic::accepted(&mut equip, 0, &command);
    semantic::complete(&mut equip, 12, |_| None).require_exercised();
    assert_eq!(equip.state.players[0].life, life_before - 2);
    assert!(equip.effective_has_keyword(creature, tricerules_cards::Keyword::Hexproof));
    assert!(
        equip.state.continuous_effects.iter().any(|effect| {
            effect.affected == tricerules_core::AffectedScope::AttachedTo(artifact)
                && matches!(
                    &effect.kind,
                    tricerules_cards::primitives::ContinuousEffectKind::CombatRestriction(
                        restriction
                    ) if restriction.cant_be_blocked
                )
        }),
        "the unblockable restriction is scoped to the creature this Equipment is attached to"
    );

    // Adventure face: sacrifice a creature to draw two, then exile for later casting.
    let mut adventure = engine(338_722, &["my_precious_allure_of_power"], &[]);
    let fodder = inject_creature_on_battlefield(&mut adventure, 0, "grizzly_bears");
    ensure_card_in_hand(&mut adventure, 0, "my_precious_allure_of_power");
    let slot = hand_index_for_card(&adventure, 0, "my_precious_allure_of_power");
    let hand_before = adventure.state.players[0].hand.len();
    let command = cast_adventure(slot, vec![battlefield_cost(&adventure, 0, &[fodder])]);
    semantic::accepted(&mut adventure, 0, &command);
    semantic::complete(&mut adventure, 12, |_| None).require_exercised();
    assert_eq!(adventure.state.objects[&fodder].zone, Zone::Graveyard);
    assert_eq!(adventure.state.players[0].hand.len(), hand_before - 1 + 2);
    // The resolved Adventure spell is exiled and may be cast later as the permanent.
    let exiled = adventure.state.players[0]
        .exile
        .iter()
        .copied()
        .find(|oid| adventure.state.objects[oid].card_id == "my_precious_allure_of_power")
        .expect("the Adventure card is exiled on resolution");
    assert_eq!(adventure.state.objects[&exiled].zone, Zone::Exile);

    // The exile play permission lets the controller cast the same card as its permanent face.
    let legal = adventure.initial_response_batch();
    let action = legal.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == exiled)
        .expect("an Adventure exile play permission");
    assert_eq!(
        action.face_index, 0,
        "the permission casts the permanent face"
    );
    let permission_id = action.casting_permission_id;
    let cast = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Normal as i32,
            source: Some(exile_cast_source(
                exiled,
                adventure.state.zone_change_generation[&exiled],
            )),
            face_index: 0,
            casting_permission_id: permission_id,
            ..Default::default()
        })),
    };
    semantic::accepted(&mut adventure, 0, &cast);
    semantic::complete(&mut adventure, 12, |_| None).require_exercised();
    assert_eq!(
        adventure.state.objects[&exiled].zone,
        Zone::Battlefield,
        "the exiled Adventure card resolves as its permanent face"
    );
}
