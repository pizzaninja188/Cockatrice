//! Actual Kaya with prepared zones/mana/loyalty; strict normal and network evidence are separate.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1 as rv1;
const ID: &str = "kaya_the_inexorable";

fn game() -> GameEngine {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        26_110_313,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]); 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    e
}
fn kaya(e: &mut GameEngine, loyalty: u32) -> u32 {
    let id = inject_permanent_on_battlefield(e, 0, ID);
    e.state
        .objects
        .get_mut(&id)
        .unwrap()
        .set_counter(CounterKind::Loyalty, loyalty);
    id
}
fn activate(e: &mut GameEngine, source: u32, index: u32, targets: Vec<rv1::TargetRef>) {
    let cmd = activate_ability_for(e, source, index, targets);
    e.apply_scenario_command(0, &cmd).unwrap();
    pass_priority_round(e);
}
fn reject(e: &mut GameEngine, actor: i32, cmd: &RuledCommand) {
    let before = e.diagnostic_snapshot().unwrap();
    assert!(e.apply_scenario_command(actor, cmd).is_err());
    assert_eq!(e.diagnostic_snapshot().unwrap(), before);
}
fn advance(e: &mut GameEngine, step: TurnStep, after: u64) {
    for _ in 0..200 {
        if e.state.turn_instance > after
            && e.state.active_player_id() == 0
            && e.state.turn_step == step
        {
            return;
        }
        let (actor, cmd) = match e.state.cleanup_discard_player {
            Some(player) => {
                let index = e.state.player_idx(player).unwrap();
                let excess = e.state.players[index].hand.len() - 7;
                (player, discard_cleanup_batch((0..excess as u32).collect()))
            }
            None => (e.state.priority_player_id(), pass()),
        };
        e.apply_scenario_command(actor, &cmd).unwrap();
    }
    panic!("bounded actual cleanup/turn advance");
}
fn spirits(e: &GameEngine, player: usize) -> usize {
    e.state.players[player]
        .battlefield
        .iter()
        .filter(|id| e.state.objects[id].card_id == "spirit_w_1_1_flying")
        .count()
}
fn finish_departure(e: &mut GameEngine) {
    let actor = e.state.priority_player_id();
    e.apply_scenario_command(actor, &pass()).unwrap();
    for _ in 0..8 {
        answer_trigger_order_in_engine_order(e);
        if e.state.stack.is_empty() {
            return;
        }
        pass_priority_round(e);
    }
    panic!("bounded actual Ghostform departure triggers");
}

#[test]
fn kaya_actual_cast_enters_with_five_and_zero_target_plus_one_uses_loyalty_once() {
    let mut e = game();
    let id = inject_card_into_hand(&mut e, 0, ID);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 3,
            w: 1,
            b: 1,
            ..Default::default()
        },
    );
    let cmd = cast_spell(hand_index_for_card(&e, 0, ID), vec![]);
    e.apply_scenario_command(0, &cmd).unwrap();
    pass_priority_round(&mut e);
    assert_eq!(e.state.objects[&id].counter_count(CounterKind::Loyalty), 5);
    let ultimate = activate_ability_for(&e, id, 2, vec![]);
    reject(&mut e, 0, &ultimate);
    activate(&mut e, id, 0, vec![]);
    assert_eq!(e.state.objects[&id].counter_count(CounterKind::Loyalty), 6);
    let again = activate_ability_for(&e, id, 0, vec![]);
    reject(&mut e, 0, &again);
    assert_eq!(spirits(&e, 0), 0);
}

#[test]
fn kaya_actual_plus_one_accepts_opponents_nontoken_and_its_controller_gets_the_spirit() {
    let mut e = game();
    let source = kaya(&mut e, 5);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let token = inject_permanent_on_battlefield(&mut e, 0, "spirit_w_1_1_flying");
    let bad = activate_ability_for(&e, source, 0, target_object(token));
    reject(&mut e, 0, &bad);
    let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let bad = activate_ability_for(&e, source, 0, target_object(land));
    reject(&mut e, 0, &bad);
    activate(&mut e, source, 0, target_object(target));
    assert_eq!(
        e.state.objects[&target].counter_count(CounterKind::Ghostform),
        1
    );
    e.state.objects.get_mut(&target).unwrap().damage = 100;
    finish_departure(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Hand);
    assert!(e.state.players[1].hand.contains(&target));
    assert_eq!(spirits(&e, 1), 1);
    assert_eq!(spirits(&e, 0), 1, "only prepared token on Kaya's side");
}

#[test]
fn kaya_actual_two_plus_ones_persist_across_cleanup_and_create_two_spirits_on_exile() {
    let mut e = game();
    let source = kaya(&mut e, 5);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    activate(&mut e, source, 0, target_object(target));
    let turn = e.state.turn_instance;
    advance(&mut e, TurnStep::Main1, turn);
    activate(&mut e, source, 0, target_object(target));
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Loyalty),
        7
    );
    assert_eq!(
        e.state.objects[&target].counter_count(CounterKind::Ghostform),
        2
    );
    e.state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::Ghostform, 0);
    e.state
        .objects
        .get_mut(&target)
        .unwrap()
        .set_counter(CounterKind::Finality, 1);
    e.state.objects.get_mut(&target).unwrap().damage = 100;
    finish_departure(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Hand);
    assert_eq!(spirits(&e, 0), 2);
}

#[test]
fn kaya_actual_minus_three_exiles_each_nonland_type_and_rejects_land_stale_actor_and_insufficient_loyalty(
) {
    for card in [
        "grizzly_bears",
        "mind_stone",
        "doubling_season",
        "ajani_steadfast",
    ] {
        let mut e = game();
        let source = kaya(&mut e, 5);
        let target = inject_permanent_on_battlefield(&mut e, 1, card);
        if card == "ajani_steadfast" {
            e.state
                .objects
                .get_mut(&target)
                .unwrap()
                .set_counter(CounterKind::Loyalty, 4);
        }
        activate(&mut e, source, 1, target_object(target));
        assert_eq!(e.state.objects[&target].zone, Zone::Exile);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Loyalty),
            2
        );
    }
    let mut e = game();
    let source = kaya(&mut e, 2);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let bad = activate_ability_for(&e, source, 1, target_object(land));
    reject(&mut e, 0, &bad);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let cmd = activate_ability_for(&e, source, 1, target_object(target));
    reject(&mut e, 0, &cmd);
    e.state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Loyalty, 5);
    reject(&mut e, 1, &cmd);
    let mut bad = cmd.clone();
    let Some(rv1::ruled_command::Cmd::ActivateAbility(activation)) = bad.cmd.as_mut() else {
        unreachable!()
    };
    activation.targets[0].expected_zone_change_generation = Some(999);
    reject(&mut e, 0, &bad);
}

#[test]
fn kaya_actual_ultimate_survives_source_death_and_casts_exact_kaya_from_each_zone() {
    for zone in [Zone::Hand, Zone::Graveyard, Zone::Exile] {
        let mut e = game();
        let source = kaya(&mut e, 7);
        let candidate = match zone {
            Zone::Hand => inject_card_into_hand(&mut e, 0, ID),
            Zone::Graveyard => source,
            Zone::Exile => {
                let id = inject_graveyard_card(&mut e, 0, ID);
                e.state.players[0].graveyard.retain(|old| *old != id);
                e.state.players[0].exile.push(id);
                e.state.objects.get_mut(&id).unwrap().zone = Zone::Exile;
                id
            }
            _ => unreachable!(),
        };
        activate(&mut e, source, 2, vec![]);
        assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
        assert_eq!(e.state.static_emblems.len(), 1);
        let turn = e.state.turn_instance;
        advance(&mut e, TurnStep::Upkeep, turn);
        pass_priority_round(&mut e);
        assert!(e.state.pending_resolution.is_some());
        let location = match zone {
            Zone::Hand => rv1::cast_source::Location::HandIndex(
                e.state.players[0]
                    .hand
                    .iter()
                    .position(|id| *id == candidate)
                    .unwrap() as u32,
            ),
            Zone::Graveyard => rv1::cast_source::Location::GraveyardObjectId(candidate),
            _ => rv1::cast_source::Location::ExileObjectId(candidate),
        };
        let prompt = e
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .resolution_prompt_id
            .get();
        let cmd = RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    resolution_prompt_id: prompt,
                    decision: rv1::ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(rv1::SpellCastAnnouncement {
                        cast_method: rv1::CastMethod::WithoutManaCost as i32,
                        source: Some(rv1::CastSource {
                            location: Some(location),
                            expected_zone_change_generation: (zone != Zone::Hand).then(|| {
                                e.state
                                    .zone_change_generation
                                    .get(&candidate)
                                    .copied()
                                    .unwrap_or(0)
                            }),
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )),
        };
        e.apply_scenario_command(0, &cmd).unwrap();
        assert!(e.state.stack.is_empty());
        let tx = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
        e.apply_scenario_command(
            0,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id: tx,
                        ..Default::default()
                    },
                )),
            },
        )
        .unwrap();
        assert!(e.state.pending_resolution.is_none());
        pass_priority_round(&mut e);
        assert_eq!(e.state.objects[&candidate].zone, Zone::Battlefield);
        assert_eq!(
            e.state.objects[&candidate].counter_count(CounterKind::Loyalty),
            5
        );
        assert_eq!(e.state.static_emblems.len(), 1);
    }
}
