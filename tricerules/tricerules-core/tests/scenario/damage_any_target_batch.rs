//! Actual-card scenarios for the exact-text "deals damage to any target" family.
//!
//! Expectations are printed Oracle values, independent of the registry definitions:
//! player damage is life loss (CR 120.3a), creature damage is marked damage destroyed by
//! state-based actions only when lethal (CR 120.3e, 120.5-120.6), planeswalker damage removes
//! loyalty counters (CR 120.3c), battle damage removes defense counters (CR 120.3h), and an
//! illegal target at resolution means no damage is dealt (CR 608.2b, 115.4). The single target
//! is chosen as the spell is cast (CR 115.1a). Gut Shot's Phyrexian symbol is paid with red mana
//! or 2 life, and a player at 1 life cannot pay 2 life (CR 107.4f, 119.4).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::FlexPipPayment;

const STALE_CARD: &str = "grizzly_bears";
const NON_DAMAGEABLE: &str = "swiftfoot_boots";
const WALKER: &str = "jace_beleren";
const BATTLE: &str = "invasion_of_ulgrotha_grandmother_ravi_sengir";

struct Case {
    id: &'static str,
    amount: u32,
    mana: ManaGift,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "bee_sting",
            amount: 2,
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "cinder_storm",
            amount: 7,
            mana: ManaGift {
                r: 1,
                c: 6,
                ..Default::default()
            },
        },
        Case {
            id: "cleansing_screech",
            amount: 4,
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
        },
        Case {
            id: "explosive_impact",
            amount: 5,
            mana: ManaGift {
                r: 1,
                c: 5,
                ..Default::default()
            },
        },
        Case {
            id: "fire_ambush",
            amount: 3,
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "gintak_charge",
            amount: 2,
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
        },
        Case {
            id: "gut_shot",
            amount: 1,
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
        },
        Case {
            id: "hornet_sting",
            amount: 1,
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
        },
        Case {
            id: "lightning_blast",
            amount: 4,
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "open_fire",
            amount: 3,
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "precision_bolt",
            amount: 3,
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "scorching_spear",
            amount: 1,
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
        },
        Case {
            id: "searing_spear",
            amount: 3,
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "searing_wind",
            amount: 10,
            mana: ManaGift {
                r: 1,
                c: 8,
                ..Default::default()
            },
        },
        Case {
            id: "tarfire",
            amount: 2,
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
        },
        Case {
            id: "unfriendly_fire",
            amount: 4,
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
        },
        Case {
            id: "unyaro_bee_sting",
            amount: 2,
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "volcanic_hammer",
            amount: 3,
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "zukos_offense",
            amount: 2,
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast_and_resolve(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

fn assert_pool_empty(e: &GameEngine, id: &str) {
    let pool = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "{id}: the cast consumed exactly the supplied cost"
    );
}

fn assert_card_moved_to_graveyard(e: &GameEngine, id: &str, object: u32) {
    assert_eq!(
        e.state.objects[&object].zone,
        Zone::Graveyard,
        "{id}: the resolving spell leaves the stack for its owner's graveyard"
    );
    assert!(
        e.state.players[0].graveyard.contains(&object),
        "{id}: the spell card is in its owner's graveyard"
    );
}

#[test]
fn damage_any_target_cards_deal_exact_damage_and_use_exact_costs() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let id = case.id;
        let amount = case.amount;
        let mut e = engine(750_000 + case_index as u64);
        let p0_life = e.state.players[0].life;
        let p1_life = e.state.players[1].life;
        let p0_library = e.state.players[0].library.len();
        let p0_hand = e.state.players[0].hand.len();
        let p1_hand = e.state.players[1].hand.len();
        let p1_library = e.state.players[1].library.len();
        give_mana(&mut e, 0, case.mana);

        let slot = inject_card_into_hand(&mut e, 0, id);
        let hand_slot = hand_index_for_card(&e, 0, id);
        semantic::accepted(&mut e, 0, &cast_spell(hand_slot, target_player(1)));
        assert_pool_empty(&e, id);
        assert_eq!(
            e.state.stack.len(),
            1,
            "{id}: the spell is on the stack before resolution"
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            p0_hand,
            "{id}: casting moves the card out of hand"
        );
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.players[1].life,
            p1_life - amount as i32,
            "{id}: exact damage is dealt to the targeted player"
        );
        assert_eq!(
            e.state.players[0].life, p0_life,
            "{id}: the caster's life is unchanged"
        );
        assert_eq!(
            e.state.players[0].library.len(),
            p0_library,
            "{id}: no library movement for the caster"
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            p1_hand,
            "{id}: the opponent's hand is private and unchanged"
        );
        assert_eq!(
            e.state.players[1].library.len(),
            p1_library,
            "{id}: the opponent's library is unchanged"
        );
        assert_card_moved_to_graveyard(&e, id, slot);
    }
}

#[test]
fn damage_any_target_cards_mark_exact_nonlethal_damage_on_creatures() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let id = case.id;
        let amount = case.amount;
        let mut e = engine(751_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, STALE_CARD, 20, 20);
        let p1_life = e.state.players[1].life;
        give_mana(&mut e, 0, case.mana);

        let slot = inject_card_into_hand(&mut e, 0, id);
        let hand_slot = hand_index_for_card(&e, 0, id);
        semantic::accepted(&mut e, 0, &cast_spell(hand_slot, target_object(target)));
        assert_pool_empty(&e, id);
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Battlefield,
            "{id}: nonlethal damage does not move the creature"
        );
        assert_eq!(
            e.state.objects[&target].damage, amount,
            "{id}: exact nonlethal damage is marked"
        );
        assert_eq!(
            e.state.players[1].life, p1_life,
            "{id}: damaging a creature does not damage its controller"
        );
        assert_card_moved_to_graveyard(&e, id, slot);
    }
}

#[test]
fn damage_any_target_cards_accept_planeswalkers_and_battles() {
    let mut walker_engine = engine(752_001);
    let walker = inject_permanent_on_battlefield(&mut walker_engine, 1, WALKER);
    walker_engine
        .state
        .objects
        .get_mut(&walker)
        .expect("planeswalker")
        .set_counter(CounterKind::Loyalty, 20);
    give_mana(
        &mut walker_engine,
        0,
        ManaGift {
            r: 1,
            c: 8,
            ..Default::default()
        },
    );
    cast_and_resolve(&mut walker_engine, "searing_wind", target_object(walker));
    assert_eq!(
        walker_engine.state.objects[&walker].counter_count(CounterKind::Loyalty),
        10,
        "ten damage removes ten loyalty counters"
    );
    assert_eq!(walker_engine.state.players[1].life, 20);

    let mut battle_engine = engine(752_002);
    let battle = inject_permanent_on_battlefield(&mut battle_engine, 1, BATTLE);
    battle_engine
        .state
        .objects
        .get_mut(&battle)
        .expect("battle")
        .set_counter(CounterKind::Defense, 20);
    give_mana(
        &mut battle_engine,
        0,
        ManaGift {
            r: 1,
            c: 8,
            ..Default::default()
        },
    );
    cast_and_resolve(&mut battle_engine, "searing_wind", target_object(battle));
    assert_eq!(
        battle_engine.state.objects[&battle].counter_count(CounterKind::Defense),
        10,
        "ten damage removes ten defense counters"
    );
    assert_eq!(battle_engine.state.players[1].life, 20);
}

#[test]
fn damage_any_target_cards_reject_non_damageable_targets_without_paying() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let id = case.id;
        let mut e = engine(753_000 + case_index as u64);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, NON_DAMAGEABLE);
        give_mana(&mut e, 0, case.mana);
        let hand_before = e.state.players[0].hand.len();
        let card = inject_card_into_hand(&mut e, 0, id);
        let hand_slot = hand_index_for_card(&e, 0, id);

        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(artifact)))
                .is_err(),
            "{id}: a noncreature artifact is not a legal damage target"
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{id}: rejected cast keeps the card in hand"
        );
        assert!(
            e.state.players[0].hand.contains(&card),
            "{id}: rejected cast keeps the injected card"
        );
        assert_eq!(
            e.state.players[0].mana_pool.red + e.state.players[0].mana_pool.green,
            1,
            "{id}: rejected cast does not spend the supplied colored mana"
        );
        assert!(
            e.state.stack.is_empty(),
            "{id}: rejected cast does not put a spell on the stack"
        );
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{id}: rejected cast leaves the artifact untouched"
        );
    }
}

#[test]
fn damage_any_target_cards_fizzle_when_the_chosen_creature_left() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let id = case.id;
        let mut e = engine(754_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, STALE_CARD, 20, 20);
        let p0_life = e.state.players[0].life;
        let p1_life = e.state.players[1].life;
        give_mana(&mut e, 0, case.mana);
        inject_card_into_hand(&mut e, 0, id);
        let hand_slot = hand_index_for_card(&e, 0, id);
        semantic::accepted(&mut e, 0, &cast_spell(hand_slot, target_object(target)));

        // The target leaves the battlefield before resolution (CR 608.2b).
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.players[0].life, p0_life,
            "{id}: a fizzled spell does not damage any player"
        );
        assert_eq!(
            e.state.players[1].life, p1_life,
            "{id}: the intended player target was not damaged"
        );
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{id}: the stale target remains where it moved"
        );
    }
}

#[test]
fn gut_shot_pays_red_or_two_life_and_rejects_life_payment_at_one() {
    let mut life_paid = engine(755_001);
    let p0_life = life_paid.state.players[0].life;
    let p1_life = life_paid.state.players[1].life;
    let slot = inject_card_into_hand(&mut life_paid, 0, "gut_shot");
    let hand_slot = hand_index_for_card(&life_paid, 0, "gut_shot");
    semantic::accepted(
        &mut life_paid,
        0,
        &cast_spell_flex(
            hand_slot,
            target_player(1),
            vec![FlexPipPayment {
                pip_index: 0,
                pay_life: true,
                mana_color: 0,
            }],
        ),
    );
    assert_pool_empty(&life_paid, "gut_shot");
    assert_eq!(
        life_paid.state.players[0].life,
        p0_life - 2,
        "the Phyrexian symbol is paid with 2 life"
    );
    resolve_entire_stack_two_player(&mut life_paid);
    assert_eq!(
        life_paid.state.players[1].life,
        p1_life - 1,
        "Gut Shot deals exactly one damage"
    );
    assert_card_moved_to_graveyard(&life_paid, "gut_shot", slot);

    let mut red_paid = engine(755_002);
    let p0_life = red_paid.state.players[0].life;
    let p1_life = red_paid.state.players[1].life;
    give_mana(
        &mut red_paid,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let red_card = inject_card_into_hand(&mut red_paid, 0, "gut_shot");
    let hand_slot = hand_index_for_card(&red_paid, 0, "gut_shot");
    semantic::accepted(&mut red_paid, 0, &cast_spell(hand_slot, target_player(1)));
    assert_pool_empty(&red_paid, "gut_shot");
    assert_eq!(
        red_paid.state.players[0].life, p0_life,
        "paying red mana does not cost life"
    );
    resolve_entire_stack_two_player(&mut red_paid);
    assert_eq!(red_paid.state.players[1].life, p1_life - 1);
    assert_card_moved_to_graveyard(&red_paid, "gut_shot", red_card);

    let mut one_life = engine(755_003);
    one_life.state.players[0].life = 1;
    let slot = inject_card_into_hand(&mut one_life, 0, "gut_shot");
    let hand_slot = hand_index_for_card(&one_life, 0, "gut_shot");
    assert!(
        one_life
            .apply_scenario_command(
                0,
                &cast_spell_flex(
                    hand_slot,
                    target_player(1),
                    vec![FlexPipPayment {
                        pip_index: 0,
                        pay_life: true,
                        mana_color: 0,
                    }],
                ),
            )
            .is_err(),
        "a player at 1 life cannot pay 2 life"
    );
    assert_eq!(one_life.state.players[0].life, 1);
    assert_eq!(
        one_life.state.players[0].hand.len(),
        8,
        "the rejected life payment keeps Gut Shot in the opening hand plus one"
    );
    assert!(one_life.state.stack.is_empty());

    // The red branch remains available at 1 life.
    give_mana(
        &mut one_life,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let hand_slot = hand_index_for_card(&one_life, 0, "gut_shot");
    semantic::accepted(&mut one_life, 0, &cast_spell(hand_slot, target_player(1)));
    assert_pool_empty(&one_life, "gut_shot");
    assert_eq!(one_life.state.players[0].life, 1);
    resolve_entire_stack_two_player(&mut one_life);
    assert_eq!(one_life.state.players[1].life, 19);
    assert_eq!(
        one_life.state.objects[&slot].zone,
        Zone::Graveyard,
        "the spell resolves after the red-mana payment"
    );
}
