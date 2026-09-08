use super::helpers::*;
use tricerules_cards::{Color, ContinuousEffectKind, EffectDuration};
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::Zone;

fn fund(e: &mut GameEngine, player: i32) {
    give_mana(
        e,
        player,
        ManaGift {
            w: 10,
            u: 10,
            b: 10,
            r: 10,
            g: 10,
            ..Default::default()
        },
    );
}

fn stack_target(oid: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: oid,
        kind: TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

fn setup(counter: &str, spell: &str) -> (GameEngine, u32, usize) {
    let mut e = GameEngine::new(
        231010,
        &[0, 1],
        20,
        Some(vec![
            deck_with("forest", &[spell]),
            deck_with("island", &[counter]),
        ]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, spell);
    ensure_in_hand(&mut e, 1, counter);
    fund(&mut e, 0);
    fund(&mut e, 1);
    let slot = hand_index_for_card(&e, 0, spell);
    e.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    let target = e.state.stack.last().unwrap().id;
    e.apply_command(0, &pass()).unwrap();
    let counter_slot = hand_index_for_card(&e, 1, counter);
    (e, target, counter_slot)
}

fn assert_rejected_without_mutation(e: &mut GameEngine, player: i32, command: RuledCommand) {
    let before = serde_json::to_value(&e.state).unwrap();
    assert!(e.apply_command(player, &command).is_err());
    assert_eq!(serde_json::to_value(&e.state).unwrap(), before);
}

#[test]
fn issue_231_counter_offers_submission_and_resolution_agree() {
    for (counter, spell, legal) in [
        ("annul", "short_sword", true),
        ("annul", "glorious_anthem", true),
        ("annul", "ornithopter", true),
        ("annul", "grizzly_bears", false),
        ("flashfreeze", "hill_giant", true),
        ("flashfreeze", "grizzly_bears", true),
        ("flashfreeze", "watchwolf", true),
        ("flashfreeze", "ornithopter", false),
        ("flashfreeze", "silvercoat_lion", false),
        ("get_out", "grizzly_bears", true),
        ("get_out", "glorious_anthem", true),
        ("get_out", "short_sword", false),
    ] {
        let (mut e, target, slot) = setup(counter, spell);
        let batch = e.initial_response_batch();
        let targets = if counter == "get_out" {
            batch.legal_by_player[&1]
                .hand_actions
                .iter()
                .find(|action| action.hand_index == slot as u32)
                .and_then(|action| action.modes[0].targets.as_ref())
                .map(|targets| targets.groups[0].valid_stack_ids.clone())
                .unwrap_or_default()
        } else {
            batch.legal_by_player[&1].valid_targets_by_hand_slot[&((slot as u32) << 8)].groups[0]
                .valid_stack_ids
                .clone()
        };
        assert_eq!(
            targets.contains(&target),
            legal,
            "{counter} against {spell}"
        );
        let command = if counter == "get_out" {
            cast_modal_spell(slot, vec![(0, stack_target(target))])
        } else {
            cast_spell(slot, stack_target(target))
        };
        if legal {
            e.apply_command(1, &command).unwrap();
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(
                e.state.objects[&target].zone,
                Zone::Graveyard,
                "{counter} against {spell}"
            );
        } else {
            assert_rejected_without_mutation(&mut e, 1, command);
        }
    }
}

fn set_colors(e: &mut GameEngine, oid: u32, colors: Vec<Color>) {
    e.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind: ContinuousEffectKind::Layer5SetColors(colors),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: e.state.command_index,
    });
}

#[test]
fn issue_231_flashfreeze_rechecks_color_and_exact_stack_generation() {
    for change_generation in [false, true] {
        let (mut e, target, slot) = setup("flashfreeze", "hill_giant");
        e.apply_command(1, &cast_spell(slot, stack_target(target)))
            .unwrap();
        if change_generation {
            *e.state.zone_change_generation.entry(target).or_default() += 1;
        } else {
            set_colors(&mut e, target, vec![Color::Blue]);
        }
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Battlefield);
        assert_eq!(
            e.characteristics(target).unwrap().colors,
            vec![Color::Red],
            "a stack color effect must end when the spell becomes a new battlefield object"
        );
    }
    let (mut e, target, slot) = setup("flashfreeze", "silvercoat_lion");
    set_colors(&mut e, target, vec![Color::Green]);
    let batch = e.initial_response_batch();
    assert!(
        batch.legal_by_player[&1].valid_targets_by_hand_slot[&((slot as u32) << 8)].groups[0]
            .valid_stack_ids
            .contains(&target)
    );
    e.apply_command(1, &cast_spell(slot, stack_target(target)))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
}

fn get_out_setup() -> (GameEngine, u32, u32, u32) {
    let mut e = GameEngine::new(
        231020,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &["get_out"]), forest_only_deck()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    ensure_in_hand(&mut e, 0, "get_out");
    fund(&mut e, 0);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let stolen = inject_permanent_on_battlefield(&mut e, 1, "glorious_anthem");
    e.state.objects.get_mut(&stolen).unwrap().owner = 0;
    let borrowed = inject_creature_on_battlefield(&mut e, 0, "hill_giant");
    e.state.objects.get_mut(&borrowed).unwrap().owner = 1;
    (e, own, stolen, borrowed)
}

fn return_targets(ids: &[u32]) -> Vec<TargetRef> {
    ids.iter()
        .map(|&oid| TargetRef {
            object_id: oid,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        })
        .collect()
}

#[test]
fn issue_231_get_out_returns_one_or_two_owned_permanents_under_either_controller() {
    for count in [1, 2] {
        let (mut e, own, stolen, borrowed) = get_out_setup();
        let slot = hand_index_for_card(&e, 0, "get_out");
        let batch = e.initial_response_batch();
        let action = batch.legal_by_player[&0]
            .hand_actions
            .iter()
            .find(|a| a.hand_index == slot as u32)
            .unwrap();
        let group = &action.modes[1].targets.as_ref().unwrap().groups[0];
        assert_eq!((group.min, group.max), (1, 2));
        assert!(group.valid_permanent_ids.contains(&own));
        assert!(group.valid_permanent_ids.contains(&stolen));
        assert!(!group.valid_permanent_ids.contains(&borrowed));
        let targets = if count == 1 {
            vec![stolen]
        } else {
            vec![own, stolen]
        };
        e.apply_command(
            0,
            &cast_modal_spell(slot, vec![(1, return_targets(&targets))]),
        )
        .unwrap();
        resolve_entire_stack_two_player(&mut e);
        for target in targets {
            assert_eq!(e.state.objects[&target].zone, Zone::Hand);
            assert!(e.state.players[0].hand.contains(&target));
            assert!(!e.state.players[1].hand.contains(&target));
        }
        assert_eq!(e.state.objects[&borrowed].zone, Zone::Battlefield);
    }
}

#[test]
fn issue_231_get_out_rejects_wrong_owner_duplicates_and_invalid_counts_atomically() {
    let (mut e, own, stolen, borrowed) = get_out_setup();
    let third = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let slot = hand_index_for_card(&e, 0, "get_out");
    for targets in [
        vec![],
        vec![borrowed],
        vec![own, own],
        vec![own, stolen, third],
    ] {
        assert_rejected_without_mutation(
            &mut e,
            0,
            cast_modal_spell(slot, vec![(1, return_targets(&targets))]),
        );
    }
}

#[test]
fn issue_231_get_out_rechecks_types_and_generations_for_each_target() {
    for both_illegal in [false, true] {
        for generation_change in [false, true] {
            let (mut e, own, stolen, _) = get_out_setup();
            let slot = hand_index_for_card(&e, 0, "get_out");
            e.apply_command(
                0,
                &cast_modal_spell(slot, vec![(1, return_targets(&[own, stolen]))]),
            )
            .unwrap();
            for oid in if both_illegal {
                vec![own, stolen]
            } else {
                vec![own]
            } {
                if generation_change {
                    *e.state.zone_change_generation.entry(oid).or_default() += 1;
                } else {
                    // A copy effect changes current types without changing this incarnation.
                    let definition = tricerules_cards::CardRegistry::global()
                        .get("forest")
                        .unwrap();
                    e.state.objects.get_mut(&oid).unwrap().copiable_values =
                        Some(tricerules_core::state::CopiableValues {
                            source_card_id: "forest".into(),
                            source_face_index: 0,
                            face: definition.primary_face().clone(),
                            room_faces: None,
                            display_name: "Forest".into(),
                        });
                }
            }
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(e.state.objects[&own].zone, Zone::Battlefield);
            assert_eq!(
                e.state.objects[&stolen].zone,
                if both_illegal {
                    Zone::Battlefield
                } else {
                    Zone::Hand
                }
            );
        }
    }
}

#[test]
fn issue_231_get_out_uses_one_snapshot_for_simultaneous_leave_triggers() {
    let (mut e, bear, _, _) = get_out_setup();
    let scribe = inject_creature_on_battlefield(&mut e, 0, "three_tree_scribe");
    let recipient = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let slot = hand_index_for_card(&e, 0, "get_out");
    // Put the observer first: sequential snapshots would lose its observation of the bear.
    e.apply_command(
        0,
        &cast_modal_spell(slot, vec![(1, return_targets(&[scribe, bear]))]),
    )
    .unwrap();
    pass_both_players(&mut e);
    assert_eq!(e.state.objects[&scribe].zone, Zone::Hand);
    assert_eq!(e.state.objects[&bear].zone, Zone::Hand);
    for _ in 0..2 {
        answer_trigger_order_in_engine_order(&mut e);
        assert!(
            !e.state.pending_triggers.is_empty(),
            "Scribe observes both departures"
        );
        e.apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: return_targets(&[recipient]),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
    }
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&recipient].counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        2
    );
}

#[test]
fn issue_231_flashfreeze_can_counter_a_virtual_spell_copy_without_touching_original() {
    let (mut e, original, slot) = setup("flashfreeze", "hill_giant");
    let mut copy = e.state.stack[0].clone();
    copy.id = e.state.next_object_id;
    e.state.next_object_id += 1;
    copy.is_copy = true;
    let copied = copy.id;
    e.state.stack.push(copy);
    assert!(!e.state.objects.contains_key(&copied));
    let batch = e.initial_response_batch();
    assert!(
        batch.legal_by_player[&1].valid_targets_by_hand_slot[&((slot as u32) << 8)].groups[0]
            .valid_stack_ids
            .contains(&copied)
    );
    e.apply_command(1, &cast_spell(slot, stack_target(copied)))
        .unwrap();
    pass_both_players(&mut e);
    assert_eq!(e.state.stack.len(), 1);
    assert_eq!(e.state.stack[0].id, original);
    assert!(!e
        .state
        .players
        .iter()
        .any(|player| player.graveyard.contains(&copied)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&original].zone, Zone::Battlefield);
}
