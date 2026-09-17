//! Issue #336 — the eight reviewed pump, cost, activation, and optional-ETB Standard cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 611.2a/514.2 govern the asymmetric -4/-0 Adventure pump and its cleanup expiry; CR 601.2f/118.7a
//! the Wizard-gated generic reduction; CR 602.2/601.2h the printed-cost activated draw; CR 404/701.13
//! the {2}, {T} graveyard exile; CR 603.5/701.14 the optional fight; CR 603.5/404.2 the optional
//! graveyard return; CR 603.5/701.8 the optional artifact-or-enchantment destroy; and CR 111.10a/603.6
//! the Equipment entry Treasure.

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
};

fn deck_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn choose_permanent_targets(ids: &[u32], decline: bool) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline,
            selected_modes: Vec::new(),
            targets: ids
                .iter()
                .copied()
                .map(|object_id| TargetRef {
                    object_id,
                    kind: TargetRefKind::Permanent as i32,
                    ..Default::default()
                })
                .collect(),
        })),
    }
}

fn choose_graveyard_targets(ids: &[u32], decline: bool) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline,
            selected_modes: Vec::new(),
            targets: ids
                .iter()
                .copied()
                .map(|object_id| TargetRef {
                    object_id,
                    kind: TargetRefKind::Graveyard as i32,
                    ..Default::default()
                })
                .collect(),
        })),
    }
}

fn graveyard_target(id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: id,
        kind: TargetRefKind::Graveyard as i32,
        ..Default::default()
    }]
}

fn hand_generic_reduction(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let hand_index = hand_index_for_card(engine, player, card_id) as u32;
    engine.initial_response_batch().legal_by_player[&(player as i32)]
        .hand_actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| panic!("missing cast action for {card_id}"))
        .generic_cost_reduction
}

#[test]
fn issue_336_desperate_parry_pumps_minus_four_zero_and_expires() {
    let mut engine = deck_engine(
        336_001,
        &["obyras_attendants_desperate_parry"],
        &["grizzly_bears"],
    );
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    assert_eq!(engine.effective_power(bear), Some(2));
    assert_eq!(engine.effective_toughness(bear), Some(2));

    ensure_in_hand(&mut engine, 0, "obyras_attendants_desperate_parry");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "obyras_attendants_desperate_parry");
    engine
        .apply_command(0, &cast_spell_face(slot, target_object(bear), 1))
        .expect("cast the Desperate Parry Adventure face");
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "the pump applies only on resolution"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.effective_power(bear),
        Some(0),
        "CR 611.2a/107.1b: -4/-0 subtracts power only and clamps at zero"
    );
    assert_eq!(engine.effective_toughness(bear), Some(2));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(bear),
        Some(2),
        "CR 514.2: the pump expires at cleanup"
    );
}

#[test]
fn issue_336_arcane_epiphany_reduction_needs_a_controlled_wizard() {
    let mut absent = deck_engine(336_002, &["arcane_epiphany"], &[]);
    ensure_in_hand(&mut absent, 0, "arcane_epiphany");
    assert_eq!(
        hand_generic_reduction(&mut absent, 0, "arcane_epiphany"),
        0,
        "no controlled Wizard means no reduction"
    );
    give_mana(
        &mut absent,
        0,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&absent, 0, "arcane_epiphany");
    assert!(
        absent.apply_command(0, &cast_spell(slot, vec![])).is_err(),
        "four mana cannot pay the unreduced {{3}}{{U}}{{U}}"
    );

    let mut non_wizard = deck_engine(336_003, &["arcane_epiphany"], &[]);
    inject_creature_on_battlefield(&mut non_wizard, 0, "grizzly_bears");
    ensure_in_hand(&mut non_wizard, 0, "arcane_epiphany");
    assert_eq!(
        hand_generic_reduction(&mut non_wizard, 0, "arcane_epiphany"),
        0,
        "a non-Wizard creature does not satisfy the condition"
    );
    give_mana(
        &mut non_wizard,
        0,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&non_wizard, 0, "arcane_epiphany");
    assert!(
        non_wizard
            .apply_command(0, &cast_spell(slot, vec![]))
            .is_err(),
        "a non-Wizard creature leaves the spell at full cost"
    );

    let mut wizard = deck_engine(336_004, &["arcane_epiphany"], &[]);
    inject_creature_on_battlefield(&mut wizard, 0, "mystic_archaeologist");
    ensure_in_hand(&mut wizard, 0, "arcane_epiphany");
    assert_eq!(
        hand_generic_reduction(&mut wizard, 0, "arcane_epiphany"),
        1,
        "CR 601.2f: one controlled Wizard reduces the generic component by one"
    );
    give_mana(
        &mut wizard,
        0,
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before = wizard.state.players[0].hand.len();
    let slot = hand_index_for_card(&wizard, 0, "arcane_epiphany");
    wizard
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("the reduced {2}{U}{U} is payable with four mana");
    assert_eq!(wizard.state.players[0].mana_pool.colorless, 0);
    assert_eq!(wizard.state.players[0].mana_pool.blue, 0);
    resolve_entire_stack_two_player(&mut wizard);
    assert_eq!(
        wizard.state.players[0].hand.len(),
        hand_before + 2,
        "the cast removes the spell and the three-card draw is unchanged"
    );
}

#[test]
fn issue_336_spectral_sailor_activation_draws_one_for_three_and_a_blue() {
    let mut short = deck_engine(336_005, &[], &[]);
    let short_sailor = inject_creature_on_battlefield(&mut short, 0, "spectral_sailor");
    give_mana(
        &mut short,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let pool_before = short.state.players[0].mana_pool;
    assert!(
        short
            .apply_command(0, &activate_ability_for(&short, short_sailor, 0, vec![]),)
            .is_err(),
        "{{3}}{{U}} is not payable with three mana"
    );
    assert_eq!(short.state.players[0].mana_pool, pool_before);
    assert!(short.state.stack.is_empty());

    let mut exact = deck_engine(336_006, &[], &[]);
    let sailor = inject_creature_on_battlefield(&mut exact, 0, "spectral_sailor");
    let library_before = exact.state.players[0].library.len();
    let hand_before = exact.state.players[0].hand.len();
    give_mana(
        &mut exact,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    exact
        .apply_command(0, &activate_ability_for(&exact, sailor, 0, vec![]))
        .expect("activate {3}{U}: Draw a card.");
    assert_eq!(exact.state.players[0].mana_pool.blue, 0);
    assert_eq!(exact.state.players[0].mana_pool.colorless, 0);
    assert_eq!(exact.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut exact);
    assert_eq!(exact.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(exact.state.players[0].library.len(), library_before - 1);
}

#[test]
fn issue_336_magic_pot_exiles_a_target_graveyard_card_for_two_and_tap() {
    let mut engine = deck_engine(336_006, &["magic_pot"], &[]);
    let pot = inject_creature_on_battlefield(&mut engine, 0, "magic_pot");
    let own_card = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opposing_card = inject_graveyard_card(&mut engine, 1, "forest");
    let battlefield_creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    assert!(
        engine
            .apply_command(
                0,
                &activate_ability_for(&engine, pot, 0, target_object(battlefield_creature)),
            )
            .is_err(),
        "a battlefield permanent is not a legal graveyard-card target"
    );

    engine
        .apply_command(
            0,
            &activate_ability_for(&engine, pot, 0, graveyard_target(opposing_card)),
        )
        .expect("exile an opponent's graveyard card");
    assert!(engine.state.objects[&pot].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&opposing_card].zone,
        Zone::Exile,
        "CR 404.2: any graveyard is legal"
    );
    assert_eq!(
        engine.state.objects[&own_card].zone,
        Zone::Graveyard,
        "only the chosen card is exiled"
    );
}

#[test]
fn issue_336_affectionate_indrik_may_fight_and_may_decline() {
    let mut decline = deck_engine(336_007, &["affectionate_indrik"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut decline, 1, "grizzly_bears");
    let own = inject_creature_on_battlefield(&mut decline, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut decline, 0, "affectionate_indrik");
    assert_eq!(decline.state.pending_triggers.len(), 1);
    assert!(
        decline
            .apply_command(0, &choose_permanent_targets(&[own], false))
            .is_err(),
        "CR 115.1: a creature you control is not a legal fight target"
    );
    assert_eq!(
        decline.state.pending_triggers.len(),
        1,
        "a rejected target leaves the optional trigger pending"
    );
    decline
        .apply_command(0, &choose_permanent_targets(&[], true))
        .expect("CR 603.5: decline the optional fight");
    resolve_entire_stack_two_player(&mut decline);
    assert_eq!(decline.state.objects[&bear].zone, Zone::Battlefield);
    assert_eq!(decline.state.objects[&bear].damage, 0);

    let mut fight = deck_engine(336_008, &["affectionate_indrik"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut fight, 1, "grizzly_bears");
    let indrik = move_ready_to_battlefield(&mut fight, 0, "affectionate_indrik");
    fight
        .apply_command(0, &choose_permanent_targets(&[bear], false))
        .expect("CR 701.14: fight the chosen creature");
    resolve_entire_stack_two_player(&mut fight);
    assert_eq!(
        fight.state.objects[&bear].zone,
        Zone::Graveyard,
        "the 4/4 Indrik deals four to the 2/2 bear"
    );
    assert_eq!(fight.state.objects[&indrik].damage, 2);
}

#[test]
fn issue_336_graveshifter_may_return_a_creature_card_and_may_decline() {
    let mut engine = deck_engine(336_009, &["graveshifter"], &[]);
    let creature_card = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let land_card = inject_graveyard_card(&mut engine, 0, "forest");
    move_ready_to_battlefield(&mut engine, 0, "graveshifter");
    assert!(
        engine
            .apply_command(0, &choose_graveyard_targets(&[land_card], false))
            .is_err(),
        "a land card is not a legal creature-card target"
    );
    engine
        .apply_command(0, &choose_graveyard_targets(&[creature_card], false))
        .expect("return the creature card");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&creature_card].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&land_card].zone, Zone::Graveyard);

    let mut decline = deck_engine(336_010, &["graveshifter"], &[]);
    let card = inject_graveyard_card(&mut decline, 0, "grizzly_bears");
    move_ready_to_battlefield(&mut decline, 0, "graveshifter");
    decline
        .apply_command(0, &choose_graveyard_targets(&[], true))
        .expect("decline the optional return");
    resolve_entire_stack_two_player(&mut decline);
    assert_eq!(decline.state.objects[&card].zone, Zone::Graveyard);
}

#[test]
fn issue_336_reclamation_sage_may_destroy_an_artifact_and_may_decline() {
    let mut engine = deck_engine(
        336_011,
        &["reclamation_sage"],
        &["bonesplitter", "grizzly_bears"],
    );
    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "bonesplitter");
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    move_ready_to_battlefield(&mut engine, 0, "reclamation_sage");
    assert!(
        engine
            .apply_command(0, &choose_permanent_targets(&[creature], false))
            .is_err(),
        "a creature is not an artifact or enchantment"
    );
    engine
        .apply_command(0, &choose_permanent_targets(&[artifact], false))
        .expect("destroy the chosen artifact");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Battlefield);

    let mut decline = deck_engine(336_012, &["reclamation_sage"], &["bonesplitter"]);
    let artifact = inject_permanent_on_battlefield(&mut decline, 1, "bonesplitter");
    move_ready_to_battlefield(&mut decline, 0, "reclamation_sage");
    decline
        .apply_command(0, &choose_permanent_targets(&[], true))
        .expect("decline the optional destroy");
    resolve_entire_stack_two_player(&mut decline);
    assert_eq!(decline.state.objects[&artifact].zone, Zone::Battlefield);
}

#[test]
fn issue_336_gold_pan_creates_a_treasure_on_entry() {
    let mut engine = deck_engine(336_013, &["gold_pan"], &[]);
    move_ready_to_battlefield(&mut engine, 0, "gold_pan");
    resolve_entire_stack_two_player(&mut engine);
    let treasures = battlefield_token_oids(&engine, 0, "treasure");
    assert_eq!(treasures.len(), 1, "CR 111.10a: one Treasure token");
    assert!(
        battlefield_token_oids(&engine, 1, "treasure").is_empty(),
        "the token is created under the Equipment's controller"
    );
}

#[test]
fn issue_336_scenarios_reach_main1() {
    let engine = deck_engine(336_014, &[], &[]);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}
