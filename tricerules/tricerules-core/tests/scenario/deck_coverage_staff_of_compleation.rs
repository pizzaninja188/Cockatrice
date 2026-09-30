use super::helpers::*;
use tricerules_cards::{CardRegistry, CounterKind};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::SubmitResolutionChoice;

const STAFF: &str = "staff_of_compleation";

fn game(seed: u64, seats: usize) -> (GameEngine, u32) {
    let ids: Vec<i32> = (0..seats as i32).collect();
    let decks = Some(ids.iter().map(|_| deck_with("forest", &[])).collect());
    let mut e = GameEngine::new(seed, &ids, 20, decks, true).unwrap();
    for _ in 0..16 {
        if e.state.turn_step == tricerules_core::TurnStep::Main1 {
            break;
        }
        let actor = e.state.priority_player_id();
        e.apply_command(actor, &pass()).unwrap();
    }
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    let source = inject_card_into_hand(&mut e, 0, STAFF);
    e.state.players[0].mana_pool.colorless = 3;
    let slot = hand_index_for_card(&e, 0, STAFF);
    e.apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast exact Staff");
    for _ in 0..16 {
        if e.state.stack.is_empty() {
            break;
        }
        let actor = e.state.priority_player_id();
        e.apply_command(actor, &pass()).unwrap();
    }
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    e.state.players[0].mana_pool = Default::default();
    (e, source)
}

fn reject(e: &mut GameEngine, actor: i32, cmd: &RuledCommand) {
    let before = format!("{:?}", e.state);
    e.apply_command(actor, cmd)
        .expect_err("illegal command rejects");
    assert_eq!(format!("{:?}", e.state), before);
}

fn choice(objects: Vec<u32>, players: Vec<i32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: objects,
            chosen_player_ids: players,
            ..Default::default()
        })),
    }
}

fn move_control(e: &mut GameEngine, id: u32, from: usize, to: usize) {
    e.state.players[from].battlefield.retain(|x| *x != id);
    e.state.players[to].battlefield.push(id);
    let object = e.state.objects.get_mut(&id).unwrap();
    object.controller = to as i32;
    object.base_controller = to as i32;
}

#[test]
fn exact_artifact_and_five_complete_abilities() {
    let card = CardRegistry::global().get(STAFF).expect("Staff registered");
    assert_eq!(card.name, "Staff of Compleation");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(face.activated_abilities.len(), 5);
    assert!(face.triggered_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    let (_, source) = game(26_120_000, 2);
    assert!(source > 0);
}

#[test]
fn destroy_uses_physical_owner_independent_of_controller_and_can_target_staff() {
    for self_target in [false, true] {
        let (mut e, source) = game(26_120_100 + u64::from(self_target), 2);
        let foreign = inject_permanent_on_battlefield(&mut e, 1, "grizzly_bears");
        move_control(&mut e, foreign, 1, 0);
        let bad = activate_ability_for(&e, source, 0, target_object(foreign));
        reject(&mut e, 0, &bad);
        let target = if self_target {
            source
        } else {
            let own = inject_permanent_on_battlefield(&mut e, 0, "grizzly_bears");
            move_control(&mut e, own, 0, 1);
            own
        };
        let cmd = activate_ability_for(&e, source, 0, target_object(target));
        e.apply_command(0, &cmd).unwrap();
        assert_eq!(e.state.players[0].life, 19);
        assert!(e.state.objects[&source].tapped);
        assert_eq!(e.state.objects[&target].zone, Zone::Battlefield);
        assert_eq!(e.state.stack.len(), 1);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
        assert!(e.state.players[0].graveyard.contains(&target));
        assert_eq!(e.state.objects[&foreign].zone, Zone::Battlefield);
    }
}

#[test]
fn paid_life_mana_is_immediate_and_has_exactly_five_color_options() {
    for index in 0..5 {
        let (mut e, source) = game(26_120_200 + index as u64, 2);
        let mut cmd = activate_ability_for(&e, source, 1, vec![]);
        if let Some(Cmd::ActivateAbility(a)) = cmd.cmd.as_mut() {
            a.mana_option_index = index;
        }
        reject(&mut e, 1, &cmd);
        let mut bad = cmd.clone();
        if let Some(Cmd::ActivateAbility(a)) = bad.cmd.as_mut() {
            a.mana_option_index = 5;
        }
        reject(&mut e, 0, &bad);
        e.apply_command(0, &cmd).unwrap();
        assert_eq!(e.state.players[0].life, 18);
        assert!(e.state.objects[&source].tapped);
        assert!(e.state.stack.is_empty());
        let p = &e.state.players[0].mana_pool;
        let actual = [p.white, p.blue, p.black, p.red, p.green];
        let mut expected = [0; 5];
        expected[index as usize] = 1;
        assert_eq!(actual, expected);
        assert_eq!(p.colorless, 0);
        reject(&mut e, 0, &cmd);
    }
}

#[test]
fn all_four_life_costs_reject_unaffordable_payment_and_allow_exact_payment() {
    for ability in 0..4 {
        for exact in [false, true] {
            let (mut e, source) = game(26_120_300 + ability as u64 * 2 + u64::from(exact), 3);
            let target = inject_permanent_on_battlefield(&mut e, 0, "forest");
            let targets = if ability == 0 {
                target_object(target)
            } else {
                vec![]
            };
            let cmd = activate_ability_for(&e, source, ability, targets);
            let amount = ability as i32 + 1;
            e.state.players[0].life = amount - i32::from(!exact);
            if exact {
                e.apply_command(0, &cmd)
                    .expect("can pay exactly the current life total");
                assert_eq!(e.state.players[0].life, 0);
                assert!(
                    e.state.players[0].has_lost,
                    "zero life is handled by state-based actions"
                );
            } else {
                reject(&mut e, 0, &cmd);
                assert!(!e.state.objects[&source].tapped);
            }
        }
    }
}

#[test]
fn draw_and_generic_untap_use_the_stack_and_untap_has_no_tap_cost() {
    let (mut e, source) = game(26_120_400, 2);
    let hand = e.state.players[0].hand.len();
    let draw = activate_ability_for(&e, source, 3, vec![]);
    e.apply_command(0, &draw).unwrap();
    assert_eq!(e.state.players[0].life, 16);
    assert_eq!(e.state.players[0].hand.len(), hand);
    assert!(e.state.objects[&source].tapped);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand + 1);
    let untap = activate_ability_for(&e, source, 4, vec![]);
    e.state.players[0].mana_pool.colorless = 4;
    reject(&mut e, 0, &untap);
    e.state.players[0].mana_pool.colorless = 5;
    e.apply_command(0, &untap).unwrap();
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    assert!(e.state.objects[&source].tapped);
    resolve_entire_stack_two_player(&mut e);
    assert!(!e.state.objects[&source].tapped);
    e.state.players[0].mana_pool.red = 5;
    e.apply_command(0, &activate_ability_for(&e, source, 4, vec![]))
        .unwrap();
    assert!(!e.state.objects[&source].tapped);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].mana_pool.red, 0);
}

#[test]
fn staff_proliferates_mixed_recipients_or_none_and_rejects_invalid_choices() {
    for empty in [false, true] {
        let (mut e, source) = game(26_120_500 + u64::from(empty), 2);
        let bear = inject_permanent_on_battlefield(&mut e, 1, "grizzly_bears");
        e.state
            .objects
            .get_mut(&bear)
            .unwrap()
            .set_counter(CounterKind::PlusOnePlusOne, 1);
        e.state
            .objects
            .get_mut(&bear)
            .unwrap()
            .set_counter(CounterKind::Stun, 2);
        e.state.players[1].counters.insert(CounterKind::Poison, 1);
        let blank = inject_permanent_on_battlefield(&mut e, 0, "forest");
        e.apply_command(0, &activate_ability_for(&e, source, 2, vec![]))
            .unwrap();
        assert_eq!(e.state.players[0].life, 17);
        assert!(e.state.objects[&source].tapped);
        assert!(e.state.pending_resolution.is_none());
        pass_both_players(&mut e);
        assert_eq!(
            e.state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .choice_kind,
            ChoiceKind::Proliferate
        );
        reject(&mut e, 1, &choice(vec![bear], vec![1]));
        reject(&mut e, 0, &choice(vec![blank], vec![]));
        let cmd = if empty {
            choice(vec![], vec![])
        } else {
            choice(vec![bear], vec![1])
        };
        e.apply_command(0, &cmd).unwrap();
        assert_eq!(
            e.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
            if empty { 1 } else { 2 }
        );
        assert_eq!(
            e.state.objects[&bear].counter_count(CounterKind::Stun),
            if empty { 2 } else { 3 }
        );
        assert_eq!(
            e.state.players[1].counters[&CounterKind::Poison],
            if empty { 1 } else { 2 }
        );
        assert!(e.state.pending_resolution.is_none());
    }
}
