use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{GameEngine, Zone};

#[test]
fn exact_single_face_characteristics_and_complete_ability_counts() {
    let registry = tricerules_cards::CardRegistry::global();
    let beacon = registry.get("interplanar_beacon").unwrap();
    assert_eq!(beacon.name, "Interplanar Beacon");
    assert_eq!(beacon.face_count(), 1);
    let face = beacon.primary_face();
    assert_eq!(face.types, ["Land"]);
    assert_eq!(face.mana_cost.to_string(), "");
    assert_eq!(face.triggered_abilities.len(), 1);
    assert!(!face.triggered_abilities[0].may);
    assert_eq!(face.activated_abilities.len(), 2);
    let chandra = registry.get(CHANDRA).unwrap();
    assert_eq!(chandra.name, "Chandra, Novice Pyromancer");
    assert_eq!(chandra.face_count(), 1);
    let face = chandra.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{R}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Chandra"]);
    assert_eq!(face.loyalty, Some(5));
    assert_eq!(face.activated_abilities.len(), 3);
    assert!(face.triggered_abilities.is_empty());
}

const CHANDRA: &str = "chandra,_novice_pyromancer";
type Mana = (u32, u32, u32, u32, u32, u32);
const PAIRS: [Mana; 10] = [
    (1, 1, 0, 0, 0, 0),
    (1, 0, 1, 0, 0, 0),
    (1, 0, 0, 1, 0, 0),
    (1, 0, 0, 0, 1, 0),
    (0, 1, 1, 0, 0, 0),
    (0, 1, 0, 1, 0, 0),
    (0, 1, 0, 0, 1, 0),
    (0, 0, 1, 1, 0, 0),
    (0, 0, 1, 0, 1, 0),
    (0, 0, 0, 1, 1, 0),
];

fn engine(seed: u64) -> GameEngine {
    let mut e = GameEngine::new(
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    e
}

fn beacon(e: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(e, 0, "interplanar_beacon");
    let slot = hand_index_for_card(e, 0, "interplanar_beacon");
    e.apply_command(0, &play_land(slot))
        .expect("play actual Beacon");
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    assert!(!e.state.objects[&source].tapped);
    source
}

fn chandra(e: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(e, 0, CHANDRA);
    e.state.players[0].mana_pool.colorless = 3;
    e.state.players[0].mana_pool.red = 1;
    let slot = hand_index_for_card(e, 0, CHANDRA);
    e.apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Chandra");
    resolve_entire_stack_two_player(e);
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Loyalty),
        5
    );
    e.state.players[0].mana_pool = Default::default();
    source
}

fn option(e: &GameEngine, source: u32, index: u32) -> RuledCommand {
    let mut cmd = activate_ability_for(e, source, 1, vec![]);
    let Some(Cmd::ActivateAbility(a)) = cmd.cmd.as_mut() else {
        unreachable!()
    };
    a.mana_option_index = index;
    cmd
}

fn spend(group: u32, mana: Mana) -> ManaSpendSelection {
    ManaSpendSelection {
        restriction_group_id: group,
        w: mana.0,
        u: mana.1,
        b: mana.2,
        r: mana.3,
        g: mana.4,
        c: mana.5,
    }
}

fn cast_with(e: &GameEngine, card: &str, payment: ManaSpendSelection) -> RuledCommand {
    let mut cmd = cast_spell(hand_index_for_card(e, 0, card), vec![]);
    let Some(Cmd::CastSpell(cast)) = cmd.cmd.as_mut() else {
        unreachable!()
    };
    cast.restricted_mana.push(payment);
    cmd
}

fn reject_unchanged(e: &mut GameEngine, player: i32, cmd: &RuledCommand) {
    let before = format!("{:?}", e.state);
    e.apply_command(player, cmd)
        .expect_err("invalid command must reject");
    assert_eq!(format!("{:?}", e.state), before);
}

#[test]
fn beacon_colorless_and_all_ten_restricted_pairs_pay_only_planeswalker_spells() {
    let mut e = engine(26_110_000);
    let source = beacon(&mut e);
    let cmd = activate_ability_for(&e, source, 0, vec![]);
    e.apply_command(0, &cmd).unwrap();
    assert_eq!(e.state.players[0].mana_pool.colorless, 1);
    assert!(e.state.players[0].restricted_mana.is_empty());
    assert!(e.state.objects[&source].tapped);
    assert!(e.state.stack.is_empty());
    reject_unchanged(&mut e, 0, &cmd);

    for (index, expected) in PAIRS.iter().enumerate() {
        let mut e = engine(26_110_010 + index as u64);
        let source = beacon(&mut e);
        let cmd = option(&e, source, index as u32);
        reject_unchanged(&mut e, 0, &cmd);
        e.state.players[0].mana_pool.colorless = 1;
        reject_unchanged(&mut e, 1, &cmd);
        let invalid = option(&e, source, 10);
        reject_unchanged(&mut e, 0, &invalid);
        let batch = e
            .apply_command(0, &cmd)
            .expect("pay one and tap for two colors");
        assert!(e.state.objects[&source].tapped);
        assert_eq!(e.state.players[0].mana_pool, Default::default());
        assert!(e.state.stack.is_empty());
        assert_eq!(e.state.players[0].restricted_mana.len(), 1);
        let contribution = &e.state.players[0].restricted_mana[0];
        let a = &contribution.amount;
        assert_eq!((a.w, a.u, a.b, a.r, a.g, a.c), *expected);
        let group = contribution.restriction_group_id;
        let published = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::ManaPoolUpdated(pool)) if pool.player_id == 0 => {
                    pool.restricted_groups.first()
                }
                _ => None,
            })
            .expect("publish restriction group");
        let presentation = published.presentation.as_ref().unwrap();
        assert_eq!(presentation.card_id, "interplanar_beacon");
        assert_eq!(presentation.oracle_line_indices, [3]);
        let mut one = (0, 0, 0, 0, 0, 0);
        if expected.0 != 0 {
            one.0 = 1;
        } else if expected.1 != 0 {
            one.1 = 1;
        } else if expected.2 != 0 {
            one.2 = 1;
        } else {
            one.3 = 1;
        }
        inject_card_into_hand(&mut e, 0, "sol_ring");
        let illegal = cast_with(&e, "sol_ring", spend(group, one));
        reject_unchanged(&mut e, 0, &illegal);
        let stone = inject_permanent_on_battlefield(&mut e, 0, "mind_stone");
        let mut activation = activate_ability_for(&e, stone, 1, vec![]);
        let Some(Cmd::ActivateAbility(a)) = activation.cmd.as_mut() else {
            unreachable!()
        };
        a.restricted_mana.push(spend(group, one));
        reject_unchanged(&mut e, 0, &activation);
        let walker = inject_card_into_hand(&mut e, 0, CHANDRA);
        e.state.players[0].mana_pool.red = u32::from(expected.3 == 0);
        e.state.players[0].mana_pool.colorless = if expected.3 == 0 { 1 } else { 2 };
        let cast = cast_with(&e, CHANDRA, spend(group, *expected));
        e.apply_command(0, &cast)
            .expect("both colors pay one planeswalker spell");
        assert!(e.state.players[0].restricted_mana.is_empty());
        assert_eq!(e.state.players[0].mana_pool, Default::default());
        assert_eq!(e.state.players[0].life, 20);
        assert_eq!(
            e.state.stack.len(),
            2,
            "life trigger is above the planeswalker"
        );
        pass_both_players(&mut e);
        assert_eq!(e.state.players[0].life, 21);
        assert_eq!(e.state.objects[&walker].zone, Zone::Stack);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&walker].counter_count(CounterKind::Loyalty),
            5
        );
    }
}

#[test]
fn beacon_two_mana_can_pay_different_planeswalkers_and_unspent_mana_expires() {
    let mut e = engine(26_110_100);
    let source = beacon(&mut e);
    e.state.players[0].mana_pool.colorless = 1;
    let cmd = option(&e, source, 0);
    e.apply_command(0, &cmd).unwrap();
    let group = e.state.players[0].restricted_mana[0].restriction_group_id;
    inject_card_into_hand(&mut e, 0, "jace_beleren");
    e.state.players[0].mana_pool.blue = 1;
    e.state.players[0].mana_pool.colorless = 1;
    let cast = cast_with(&e, "jace_beleren", spend(group, (0, 1, 0, 0, 0, 0)));
    e.apply_command(0, &cast).unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].restricted_mana[0].amount.w, 1);
    assert_eq!(e.state.players[0].restricted_mana[0].amount.u, 0);
    inject_card_into_hand(&mut e, 0, CHANDRA);
    e.state.players[0].mana_pool.red = 1;
    e.state.players[0].mana_pool.colorless = 2;
    let cast = cast_with(&e, CHANDRA, spend(group, (1, 0, 0, 0, 0, 0)));
    e.apply_command(0, &cast).unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.players[0].restricted_mana.is_empty());
    assert_eq!(e.state.players[0].life, 22);

    let mut e = engine(26_110_101);
    let source = beacon(&mut e);
    e.state.players[0].mana_pool.colorless = 1;
    let cmd = option(&e, source, 0);
    e.apply_command(0, &cmd).unwrap();
    pass_both_players(&mut e);
    assert!(e.state.players[0].restricted_mana.is_empty());
}

#[test]
fn beacon_cast_trigger_survives_counter_and_ignores_other_spells_and_players() {
    let mut e = engine(26_110_200);
    beacon(&mut e);
    let walker = inject_card_into_hand(&mut e, 0, CHANDRA);
    e.state.players[0].mana_pool.colorless = 3;
    e.state.players[0].mana_pool.red = 1;
    let slot = hand_index_for_card(&e, 0, CHANDRA);
    e.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(e.state.players[0].life, 20);
    e.apply_command(0, &pass()).unwrap();
    inject_card_into_hand(&mut e, 1, "counterspell");
    e.state.players[1].mana_pool.blue = 2;
    let slot = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(1, &cast_spell(slot, targets_with_damage(vec![(walker, 0)])))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&walker].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].life, 21);
    inject_card_into_hand(&mut e, 0, "sol_ring");
    e.state.players[0].mana_pool.colorless = 1;
    let slot = hand_index_for_card(&e, 0, "sol_ring");
    e.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 21);
    end_active_turn(&mut e, 0);
    advance_to_main1_from_game_start(&mut e);
    inject_card_into_hand(&mut e, 1, "jace_beleren");
    e.state.players[1].mana_pool.blue = 2;
    e.state.players[1].mana_pool.colorless = 1;
    let slot = hand_index_for_card(&e, 1, "jace_beleren");
    e.apply_command(1, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, 21);
    assert_eq!(e.state.players[1].life, 20);
}

#[test]
fn chandra_pump_is_a_controlled_elemental_snapshot_and_expires_at_cleanup() {
    let mut e = engine(26_110_300);
    let source = chandra(&mut e);
    let own = inject_permanent_on_battlefield(&mut e, 0, "fire_elemental");
    let opponent = inject_permanent_on_battlefield(&mut e, 1, "fire_elemental");
    let bear = inject_permanent_on_battlefield(&mut e, 0, "grizzly_bears");
    let cmd = activate_ability_for(&e, source, 0, vec![]);
    e.apply_command(0, &cmd).unwrap();
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Loyalty),
        6
    );
    assert_eq!(e.effective_power(own), Some(5));
    reject_unchanged(&mut e, 0, &cmd);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(own), Some(7));
    assert_eq!(e.effective_toughness(own), Some(4));
    assert_eq!(e.effective_power(opponent), Some(5));
    assert_eq!(e.effective_power(bear), Some(2));
    let later = inject_permanent_on_battlefield(&mut e, 0, "fire_elemental");
    assert_eq!(e.effective_power(later), Some(5));
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(own), Some(5));
}

#[test]
fn chandra_red_mana_uses_the_stack_and_expires_at_the_step_boundary() {
    let mut e = engine(26_110_400);
    let source = chandra(&mut e);
    let cmd = activate_ability_for(&e, source, 1, vec![]);
    e.apply_command(0, &cmd).unwrap();
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::Loyalty),
        4
    );
    assert_eq!(e.state.players[0].mana_pool.red, 0);
    assert_eq!(
        e.state.stack.len(),
        1,
        "loyalty ability is not an immediate mana ability"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].mana_pool.red, 2);
    reject_unchanged(&mut e, 0, &cmd);
    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(slot, target_player(1)))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, 17);
    assert_eq!(e.state.players[0].mana_pool.red, 1);
    pass_both_players(&mut e);
    assert_eq!(e.state.players[0].mana_pool.red, 0);
    for _ in 0..8 {
        if e.state.active_player_id() != 0 {
            break;
        }
        e.apply_command(0, &primitive_yield()).unwrap();
    }
    assert_eq!(e.state.active_player_id(), 1);
    advance_to_main1_from_game_start(&mut e);
    let cmd = activate_ability_for(&e, source, 1, vec![]);
    reject_unchanged(&mut e, 0, &cmd);
}

#[test]
fn chandra_two_damage_accepts_every_any_target_kind_and_rejects_a_land() {
    for kind in 0..4 {
        let mut e = engine(26_110_500 + kind);
        let source = chandra(&mut e);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        let bad = activate_ability_for(&e, source, 2, targets_with_damage(vec![(land, 0)]));
        reject_unchanged(&mut e, 0, &bad);
        let target = match kind {
            0 => 1,
            1 => inject_permanent_on_battlefield(&mut e, 1, "hill_giant"),
            2 => {
                let id = inject_permanent_on_battlefield(&mut e, 1, "jace_beleren");
                e.state
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .set_counter(CounterKind::Loyalty, 3);
                id
            }
            _ => {
                let id = inject_permanent_on_battlefield(
                    &mut e,
                    1,
                    "invasion_of_ulgrotha_grandmother_ravi_sengir",
                );
                e.state
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .set_counter(CounterKind::Defense, 5);
                e.state.battle_protectors.insert(id, 1);
                id
            }
        };
        let targets = if kind == 0 {
            target_player(1)
        } else {
            targets_with_damage(vec![(target, 0)])
        };
        let cmd = activate_ability_for(&e, source, 2, targets);
        e.apply_command(0, &cmd).unwrap();
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::Loyalty),
            3
        );
        resolve_entire_stack_two_player(&mut e);
        match kind {
            0 => assert_eq!(e.state.players[1].life, 18),
            1 => assert_eq!(e.state.objects[&target].damage, 2),
            2 => assert_eq!(
                e.state.objects[&target].counter_count(CounterKind::Loyalty),
                1
            ),
            _ => assert_eq!(
                e.state.objects[&target].counter_count(CounterKind::Defense),
                3
            ),
        }
    }
}
