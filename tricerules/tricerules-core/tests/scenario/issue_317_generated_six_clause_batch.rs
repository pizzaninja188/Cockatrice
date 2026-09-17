//! Issue #317 — the six reviewed single-clause completion cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 111.10a and 111.10f govern the Treasure and Clue tokens and their own abilities; CR
//! 113.6/602 govern graveyard-zone activated abilities; CR 115.1 and 404.2 govern bounded
//! graveyard-card target groups; CR 508.1/603.2 govern attack triggers; CR 509.1b/208 govern the
//! power-bounded blocking restriction; CR 601.2h/605 govern the tap-creature mana cost and mana
//! ability resolution; CR 608.2c-f govern printed-order spell instructions.

use super::helpers::*;
use tricerules_cards::AbilitySourceZone;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ActivateAbility, BlockPair, CostChoiceKind, CostObjectRef,
    CostObjectRefs, CostSelection, TargetRef, TargetRefKind,
};

fn deck_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", own),
        deck_with("mountain", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn object_cost_selection(cost_index: u32, engine: &GameEngine, object_id: u32) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id,
                zone_change_generation: generation(engine, object_id),
            }],
        })),
    }
}

fn advance_main1_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes in beginning of combat");
    engine
        .apply_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) -> RuledEventBatch {
    engine
        .apply_command(0, &pass())
        .expect("active player passes after declaring attackers");
    engine
        .apply_command(1, &pass())
        .expect("defender passes after attackers are declared")
}

fn graveyard_target(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        group_index: 0,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }
}

fn pool_total(engine: &GameEngine, player: usize) -> u32 {
    let pool = &engine.state.players[player].mana_pool;
    pool.white + pool.blue + pool.black + pool.red + pool.green + pool.colorless
}

#[test]
fn issue_317_careening_mine_cart_creates_one_treasure_only_for_its_own_attack() {
    let mut engine = deck_engine(317_001, &["careening_mine_cart"], &[]);
    let cart = move_ready_to_battlefield(&mut engine, 0, "careening_mine_cart");
    let crewmate = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    let mut crew = activate_ability_for(&engine, cart, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = crew.cmd.as_mut() else {
        unreachable!("activate_ability_for emits ActivateAbility")
    };
    activation.cost_selections = vec![object_cost_selection(0, &engine, crewmate)];
    engine
        .apply_command(0, &crew)
        .expect("crew the Vehicle with the untapped bear");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.characteristics(cart).unwrap().is_creature());
    assert!(engine.state.objects[&crewmate].tapped);

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker, cart]))
        .expect("attack with the crewing Vehicle and the bear");
    assert!(
        battlefield_token_oids(&engine, 0, "treasure").is_empty(),
        "the trigger is only staged at declaration"
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "only the Vehicle's own attack trigger is on the stack"
    );

    let resolved = pass_to_declare_blockers(&mut engine);
    let treasure = battlefield_token_oids(&engine, 0, "treasure");
    assert_eq!(
        treasure.len(),
        1,
        "exactly the attacking Vehicle's trigger creates a Treasure"
    );
    let created = token_created_events(&resolved);
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].card_id, "treasure");
}

#[test]
fn issue_317_fight_on_returns_up_to_two_own_creature_cards() {
    let mut engine = deck_engine(317_010, &["fight_on!"], &[]);
    let first = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let second = inject_graveyard_card(&mut engine, 0, "storm_crow");
    let noncreature = inject_graveyard_card(&mut engine, 0, "bonesplitter");
    let opposing = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "fight_on!");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "fight_on!");

    let published = engine.initial_response_batch();
    let legal = &published.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    let mut offered = legal.groups[0].valid_graveyard_ids.clone();
    offered.sort_unstable();
    let mut expected = vec![first, second];
    expected.sort_unstable();
    assert_eq!(offered, expected, "only own creature cards are offered");

    for illegal in [noncreature, opposing] {
        assert!(
            engine
                .apply_command(0, &cast_spell(slot, vec![graveyard_target(illegal)]))
                .is_err(),
            "a noncreature or opposing-graveyard card is not a legal target"
        );
    }
    assert!(
        engine
            .apply_command(
                0,
                &cast_spell(
                    slot,
                    vec![
                        graveyard_target(first),
                        graveyard_target(second),
                        graveyard_target(noncreature),
                    ],
                ),
            )
            .is_err(),
        "three targets exceed the group's maximum of two"
    );

    engine
        .apply_command(
            0,
            &cast_spell(
                slot,
                vec![graveyard_target(first), graveyard_target(second)],
            ),
        )
        .expect("return two creature cards");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&first].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&second].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&first));
    assert_eq!(engine.state.objects[&noncreature].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Graveyard);

    let mut decline = deck_engine(317_011, &["fight_on!"], &[]);
    let untouched = inject_graveyard_card(&mut decline, 0, "grizzly_bears");
    ensure_in_hand(&mut decline, 0, "fight_on!");
    grant_pool(&mut decline, 0);
    let slot = hand_index_for_card(&decline, 0, "fight_on!");
    decline
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("decline every optional target");
    resolve_entire_stack_two_player(&mut decline);
    assert_eq!(decline.state.objects[&untouched].zone, Zone::Graveyard);
}

#[test]
fn issue_317_springleaf_drum_requires_an_untapped_creature_you_control() {
    let mut engine = deck_engine(317_020, &["springleaf_drum"], &[]);
    let drum = relocate_to_battlefield(&mut engine, 0, "springleaf_drum", false);

    assert!(
        apply_ability(&mut engine, 0, drum, 0, vec![]).is_err(),
        "no creature is available to tap"
    );
    assert!(!engine.state.objects[&drum].tapped);
    assert_eq!(pool_total(&engine, 0), 0);

    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.state.objects.get_mut(&own).expect("fixture").tapped = true;
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    assert!(
        apply_ability(&mut engine, 0, drum, 0, vec![]).is_err(),
        "a tapped own creature and an untapped opposing creature cannot pay"
    );
    assert!(!engine.state.objects[&drum].tapped);
    assert_eq!(pool_total(&engine, 0), 0);

    engine.state.objects.get_mut(&own).expect("fixture").tapped = false;
    let published = engine.initial_response_batch();
    let key = u64::from(drum) << 32;
    let choices = &published.legal_by_player[&0].cost_choices_by_ability[&key];
    assert!(choices.non_mana_costs_payable, "{choices:?}");
    let tap = choices
        .choices
        .iter()
        .find(|choice| choice.cost_index == 1)
        .expect("creature-tap cost choice");
    assert_eq!(tap.kind(), CostChoiceKind::Tap);
    assert_eq!(tap.candidate_ids, [own]);
    assert!(!tap.candidate_ids.contains(&opposing));

    let mut command = activate_ability_for(&engine, drum, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for emits ActivateAbility")
    };
    activation.cost_selections = vec![object_cost_selection(1, &engine, own)];
    activation.mana_option_index = 2; // black
    engine
        .apply_command(0, &command)
        .expect("tap the creature for one black mana");
    assert!(engine.state.objects[&drum].tapped);
    assert!(engine.state.objects[&own].tapped);
    assert_eq!(engine.state.players[0].mana_pool.black, 1);
    assert_eq!(pool_total(&engine, 0), 1, "exactly one mana is produced");

    assert!(
        apply_ability(&mut engine, 0, drum, 0, vec![]).is_err(),
        "the tapped Drum cannot activate again"
    );
}

#[test]
fn issue_317_stormkeld_vanguard_is_unblockable_only_by_small_creatures() {
    let mut engine = deck_engine(317_030, &["stormkeld_vanguard_bear_down"], &[]);
    let vanguard = move_ready_to_battlefield(&mut engine, 0, "stormkeld_vanguard_bear_down");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let small = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    let large = inject_creature_with_stats(&mut engine, 1, "hill_giant", 3, 3);

    advance_main1_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![vanguard, attacker]))
        .expect("attack with the Vanguard and an ordinary creature");
    let legal = pass_to_declare_blockers(&mut engine);
    let pairs = &legal.legal_by_player[&1].legal_block_pairs;
    assert!(
        pairs.contains(&BlockPair {
            attacker_id: vanguard,
            blocker_id: large,
        }),
        "a power-3 creature may block"
    );
    assert!(
        !pairs.contains(&BlockPair {
            attacker_id: vanguard,
            blocker_id: small,
        }),
        "a power-2 creature may not block"
    );
    assert!(
        pairs.contains(&BlockPair {
            attacker_id: attacker,
            blocker_id: small,
        }),
        "the restriction applies only to its own source"
    );

    assert!(
        engine
            .apply_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: vanguard,
                    blocker_id: small,
                }]),
            )
            .is_err(),
        "a declared power-2 block is rejected"
    );
    engine
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: vanguard,
                    blocker_id: large,
                },
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: small,
                },
            ]),
        )
        .expect("power-3 and ordinary blocks are legal");
}

#[test]
fn issue_317_cunning_maneuver_pumps_then_creates_a_functional_clue() {
    let mut engine = deck_engine(317_040, &["cunning_maneuver"], &[]);
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "cunning_maneuver");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "cunning_maneuver");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast at the bear");
    assert_eq!(
        engine.effective_power(target),
        Some(2),
        "the pump applies only on resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(target), Some(5));
    assert_eq!(engine.effective_toughness(target), Some(3));

    let clue = battlefield_token_oids(&engine, 0, "clue");
    assert_eq!(clue.len(), 1);
    let characteristics = engine
        .characteristics(clue[0])
        .expect("Clue characteristics");
    assert!(characteristics.is_artifact());
    assert!(characteristics.has_type("Clue"));
    assert!(!characteristics.is_creature());

    let hand_before = engine.state.players[0].hand.len();
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, clue[0], 0, vec![]).expect("pay {2} and sacrifice the Clue");
    assert!(
        !engine.state.objects.contains_key(&clue[0]),
        "the sacrificed token ceases to exist"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before + 1,
        "the Clue's own ability draws a card"
    );

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(target),
        Some(2),
        "the pump expires at cleanup"
    );
}

#[test]
fn issue_317_project_deathlok_soldier_activates_only_from_the_graveyard() {
    let mut engine = deck_engine(317_050, &["project_deathlok_soldier"], &[]);

    let in_hand = inject_card_into_hand(&mut engine, 0, "project_deathlok_soldier");
    assert!(
        apply_ability(&mut engine, 0, in_hand, 0, vec![]).is_err(),
        "the ability does not function from hand"
    );

    let on_battlefield = move_ready_to_battlefield(&mut engine, 0, "project_deathlok_soldier");
    assert!(
        apply_ability(&mut engine, 0, on_battlefield, 0, vec![]).is_err(),
        "the ability does not function from the battlefield"
    );
    assert_eq!(
        engine.state.objects[&on_battlefield].zone,
        Zone::Battlefield
    );

    let soldier = inject_graveyard_card(&mut engine, 0, "project_deathlok_soldier");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 2,
            ..Default::default()
        },
    );
    let command = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: soldier,
            source_zone: AbilitySourceZone::Graveyard as i32,
            expected_zone_change_generation: generation(&engine, soldier),
            ability_index: 0,
            ..Default::default()
        })),
    };
    engine
        .apply_command(0, &command)
        .expect("activate from the graveyard");
    assert_eq!(
        pool_total(&engine, 0),
        1,
        "exactly the printed three mana was paid from four available"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&soldier].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&soldier));
}
