//! Actual-card scenarios for five pinned Standard token and graveyard identities.
//! Scryfall Oracle and rulings checked 2026-09-22; no card-specific rulings.
//! CR 111.2–111.4, 602, 603.6a, and 702.122 govern the selected behavior.

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, AbilitySourceZone, ActivateAbility,
    CostObjectRef, CostObjectRefs, CostSelection, RuledCommand,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast(e: &mut GameEngine, card_id: &str) -> u32 {
    inject_card_into_hand(e, 0, card_id);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card_id);
    semantic::accepted(e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(e);
    *e.state.players[0]
        .battlefield
        .iter()
        .find(|&&id| e.state.objects[&id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on battlefield"))
}

fn generation(e: &GameEngine, id: u32) -> u64 {
    e.state
        .zone_change_generation
        .get(&id)
        .copied()
        .unwrap_or(0)
}

fn activate_on(e: &GameEngine, id: u32, costs: Vec<CostSelection>) -> RuledCommand {
    let mut cmd = activate_ability_with_costs(id, 0, vec![], costs);
    let Some(Cmd::ActivateAbility(activation)) = cmd.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(e, id);
    cmd
}

fn activate_graveyard(e: &GameEngine, id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: id,
            source_zone: AbilitySourceZone::Graveyard as i32,
            expected_zone_change_generation: generation(e, id),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_misc28_graveyard_sources_pay_exile_before_tokens_and_enforce_timing() {
    let mut student = engine(828_001);
    let source = inject_graveyard_card(&mut student, 0, "eternal_student");
    semantic::accepted(&mut student, 0, &primitive_yield());
    grant_pool(&mut student, 0);
    let activate = activate_graveyard(&student, source);
    semantic::accepted(&mut student, 0, &activate);
    assert_eq!(
        student.state.objects[&source].zone,
        Zone::Exile,
        "exile is a cost"
    );
    assert!(battlefield_token_oids(&student, 0, "inkling_wb_1_1_flying").is_empty());
    resolve_entire_stack_two_player(&mut student);
    assert_eq!(
        battlefield_token_oids(&student, 0, "inkling_wb_1_1_flying").len(),
        2
    );
    assert!(battlefield_token_oids(&student, 1, "inkling_wb_1_1_flying").is_empty());

    let mut late = engine(828_002);
    let source = inject_graveyard_card(&mut late, 0, "suspicious_shambler");
    semantic::accepted(&mut late, 0, &primitive_yield());
    grant_pool(&mut late, 0);
    assert!(late
        .apply_command(0, &activate_graveyard(&late, source))
        .is_err());
    assert_eq!(late.state.objects[&source].zone, Zone::Graveyard);
    assert!(battlefield_token_oids(&late, 0, "zombie_b_2_2").is_empty());

    let mut timely = engine(828_003);
    let source = inject_graveyard_card(&mut timely, 0, "suspicious_shambler");
    grant_pool(&mut timely, 0);
    let activate = activate_graveyard(&timely, source);
    semantic::accepted(&mut timely, 0, &activate);
    assert_eq!(
        timely.state.objects[&source].zone,
        Zone::Exile,
        "exile is a cost"
    );
    resolve_entire_stack_two_player(&mut timely);
    assert_eq!(battlefield_token_oids(&timely, 0, "zombie_b_2_2").len(), 2);
    assert!(battlefield_token_oids(&timely, 1, "zombie_b_2_2").is_empty());
}

#[test]
fn issue_misc28_envoy_and_tote_activation_semantics() {
    let mut envoy = engine(828_004);
    let source = cast(&mut envoy, "envoy_of_okinec_ahau");
    for expected in 1..=2 {
        grant_pool(&mut envoy, 0);
        let activate = activate_on(&envoy, source, vec![]);
        semantic::accepted(&mut envoy, 0, &activate);
        resolve_entire_stack_two_player(&mut envoy);
        assert_eq!(
            battlefield_token_oids(&envoy, 0, "gnome_c_1_1").len(),
            expected
        );
    }

    let mut tote = engine(828_005);
    let source = cast(&mut tote, "tinkers_tote");
    assert_eq!(battlefield_token_oids(&tote, 0, "gnome_c_1_1").len(), 2);
    let before_life = tote.state.players[0].life;
    grant_pool(&mut tote, 0);
    let activate = activate_on(&tote, source, vec![]);
    semantic::accepted(&mut tote, 0, &activate);
    assert_eq!(
        tote.state.objects[&source].zone,
        Zone::Graveyard,
        "sacrifice is a cost"
    );
    assert_eq!(
        tote.state.players[0].life, before_life,
        "life gain resolves later"
    );
    resolve_entire_stack_two_player(&mut tote);
    assert_eq!(tote.state.players[0].life, before_life + 3);
    assert_eq!(battlefield_token_oids(&tote, 0, "gnome_c_1_1").len(), 2);
}

#[test]
fn issue_misc28_rambler_uses_own_thopter_to_crew() {
    let mut e = engine(828_006);
    let vehicle = cast(&mut e, "broadcast_rambler");
    assert!(!e.characteristics(vehicle).unwrap().is_creature());
    let tokens = battlefield_token_oids(&e, 0, "thopter_c_1_1_flying");
    assert_eq!(tokens.len(), 1);
    let thopter = tokens[0];
    assert!(!e.state.objects[&thopter].tapped);
    let payment = CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id: thopter,
                zone_change_generation: generation(&e, thopter),
            }],
        })),
    };
    let activate = activate_on(&e, vehicle, vec![payment]);
    semantic::accepted(&mut e, 0, &activate);
    assert!(
        e.state.objects[&thopter].tapped,
        "crew taps the fresh Thopter as cost"
    );
    resolve_entire_stack_two_player(&mut e);
    let ch = e.characteristics(vehicle).unwrap();
    assert!(ch.is_artifact() && ch.is_creature());
    assert_eq!((ch.power, ch.toughness), (Some(5), Some(4)));
    assert_eq!(
        battlefield_token_oids(&e, 0, "thopter_c_1_1_flying").len(),
        1
    );
}
