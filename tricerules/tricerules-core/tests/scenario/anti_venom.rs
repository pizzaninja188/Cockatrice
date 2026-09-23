use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

const ANTI_VENOM: &str = "anti-venom,_horrifying_healer";

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &["zombify"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

#[test]
fn anti_venom_cast_returns_a_creature_from_its_owners_graveyard() {
    let mut engine = engine(910_101);
    let own_creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opponent_creature = inject_graveyard_card(&mut engine, 1, "storm_crow");
    inject_card_into_hand(&mut engine, 0, ANTI_VENOM);
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, ANTI_VENOM);
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Anti-Venom");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert!(
        engine
            .apply_command(0, &choose_trigger_target(opponent_creature))
            .is_err(),
        "Anti-Venom may target only its controller's graveyard"
    );
    engine
        .apply_command(0, &choose_trigger_target(own_creature))
        .expect("choose own graveyard creature");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&own_creature].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&own_creature].controller, 0);
    assert_eq!(
        engine.state.objects[&opponent_creature].zone,
        Zone::Graveyard
    );
}

#[test]
fn anti_venom_returned_by_zombify_does_not_trigger_its_cast_ability() {
    let mut engine = engine(910_102);
    let anti_venom = inject_graveyard_card(&mut engine, 0, ANTI_VENOM);
    relocate_to_hand(&mut engine, 0, "zombify");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "zombify");
    engine
        .apply_command(0, &cast_spell(slot, target_object(anti_venom)))
        .expect("cast Zombify");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&anti_venom].zone, Zone::Battlefield);
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.stack.is_empty());
}
