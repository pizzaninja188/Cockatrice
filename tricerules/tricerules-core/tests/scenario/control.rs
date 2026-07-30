//! CR 110.2 control vs CR 108.3 ownership.
//!
//! A permanent carries both identities at once and they can differ — reanimation is the first
//! effect that separates them. These scenarios pin down which subsystem reads which, by building
//! the shape a reanimated permanent has (owned by one seat, controlled by the other) and asserting
//! that control drives the battlefield behaviours while ownership drives where the card goes home.
//!
//! The last group drives the whole thing through Reanimate, the card that motivated it: those
//! cases go through the real ETB path rather than injecting a board, which is the only way to
//! exercise static abilities (`emit_static_abilities_on_enter`) under a changed controller.

use crate::helpers::*;

/// CR 506.2: a creature is declared as an attacker by the player who controls it. The owner has
/// no say — the creature is not even on their battlefield list.
#[test]
fn foreign_controlled_creature_attacks_for_its_controller_not_its_owner() {
    let decks = Some(vec![forest_only_deck(), island_only_deck()]);
    let mut e = GameEngine::new(4000, &[0, 1], 20, decks, true).expect("new");

    // P1 owns the bear; P0 controls it — what Reanimate produces.
    let bear = inject_creature_under_foreign_control(&mut e, 1, 0, "grizzly_bears");
    assert!(
        e.state.players[0].battlefield.contains(&bear),
        "the control index holds it for the controller"
    );
    assert!(
        !e.state.players[1].battlefield.contains(&bear),
        "and not for the owner"
    );
    assert_eq!(e.state.objects.get(&bear).expect("obj").owner, 1);

    advance_to_declare_attackers(&mut e);

    // The owner cannot declare it — it is not their creature to attack with.
    assert!(
        e.apply_command(1, &declare_attackers(vec![bear])).is_err(),
        "the owner must not be able to attack with a creature they do not control"
    );
    // The controller can.
    e.apply_command(0, &declare_attackers(vec![bear]))
        .expect("the controller attacks with it");
    assert!(e
        .state
        .combat
        .as_ref()
        .expect("combat")
        .attacking
        .contains(&bear));
}

/// CR 509.1a: likewise for blocking — a creature blocks for its controller.
#[test]
fn foreign_controlled_creature_blocks_for_its_controller() {
    let decks = Some(vec![forest_only_deck(), island_only_deck()]);
    let mut e = GameEngine::new(4001, &[0, 1], 20, decks, true).expect("new");

    // P0 attacks with a creature of their own; P1 blocks with a creature P0 owns but P1 controls.
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = inject_creature_under_foreign_control(&mut e, 0, 1, "grizzly_bears");

    advance_to_declare_attackers(&mut e);
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attackers");
    // Both players pass priority in the declare-attackers step to reach declare blockers.
    e.apply_command(0, &pass()).expect("attacker passes");
    e.apply_command(1, &pass()).expect("defender passes");

    e.apply_command(
        1,
        &declare_blockers(vec![tricerules_proto::ruled::v1::BlockPair {
            blocker_id: blocker,
            attacker_id: attacker,
        }]),
    )
    .expect("the controller blocks with it even though the attacker's owner owns it");
}

/// CR 302.6 / 502.1: a permanent untaps and sheds summoning sickness during the untap step of the
/// player who **controls** it, not the one who owns it.
#[test]
fn foreign_controlled_permanent_untaps_on_its_controllers_turn() {
    let decks = Some(vec![forest_only_deck(), island_only_deck()]);
    let mut e = GameEngine::new(4002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // P1 owns it, P0 controls it; tapped and summoning sick, as if just reanimated.
    let bear = inject_creature_under_foreign_control(&mut e, 1, 0, "grizzly_bears");
    if let Some(o) = e.state.objects.get_mut(&bear) {
        o.tapped = true;
        o.summoning_sick = true;
    }

    // P0's turn ends and P1 takes theirs — the *owner's* untap step must not touch it.
    end_active_turn(&mut e, 0);
    assert!(
        e.state.objects.get(&bear).expect("obj").tapped,
        "the owner's untap step must leave a permanent they do not control alone"
    );

    // Back to P0, the controller: now it untaps and loses summoning sickness. Yield rather than
    // `end_active_turn` — that helper's step count assumes the active player has attackers to
    // skip, and P1 controls no creatures here.
    for _ in 0..12 {
        if e.state.active_player_id() == 0 {
            break;
        }
        e.apply_command(1, &primitive_yield()).expect("p1 yields");
        resolve_cleanup_discards_if_any(&mut e);
    }
    assert_eq!(
        e.state.active_player_id(),
        0,
        "back to the controller's turn"
    );
    let o = e.state.objects.get(&bear).expect("obj");
    assert!(!o.tapped, "the controller's untap step untaps it");
    assert!(
        !o.summoning_sick,
        "and it has now been controlled since its controller's turn began"
    );
}

/// CR 400.3: a permanent that leaves the battlefield goes to its **owner's** graveyard, not its
/// controller's — and the control index must let go of it. This is the regression test for the
/// ghost-permanent class of bug: an owner-scoped `retain` would leave the oid in the controller's
/// battlefield list forever, where it would keep blocking and keep being SBA-checked.
#[test]
fn foreign_controlled_creature_dies_to_its_owners_graveyard() {
    let decks = Some(vec![
        deck_with("mountain", &["lightning_bolt"]),
        island_only_deck(),
    ]);
    let mut e = GameEngine::new(4003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // P1 owns the bear; P0 controls it. P0 then bolts their own borrowed creature.
    let bear = inject_creature_under_foreign_control(&mut e, 1, 0, "grizzly_bears");
    relocate_to_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(
            bolt,
            vec![tricerules_proto::ruled::v1::TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast bolt at the borrowed creature");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass — bolt resolves");

    assert!(
        e.state.players[1].graveyard.contains(&bear),
        "CR 400.3: it goes to its OWNER's graveyard"
    );
    assert!(
        !e.state.players[0].graveyard.contains(&bear),
        "not the controller's"
    );
    assert!(
        !e.state.players[0].battlefield.contains(&bear),
        "and the controller's battlefield list must let go of it"
    );
    let o = e.state.objects.get(&bear).expect("obj");
    assert_eq!(
        o.controller, o.owner,
        "CR 400.7: a new object in a new zone is controlled by its owner again"
    );
}

// ---------------------------------------------------------------------------------------
// Reanimate — the card that separates the two identities through the real ETB path.

/// Reanimate takes a creature out of *any* graveyard and puts it onto the battlefield under the
/// caster's control, at the cost of life equal to its mana value (CR 202.3).
#[test]
fn reanimate_takes_an_opponents_creature_under_your_control() {
    let decks = Some(vec![
        deck_with("swamp", &["reanimate"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(4100, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Hill Giant ({3}{R}, mana value 4) sits in the OPPONENT's graveyard.
    let giant = inject_graveyard_card(&mut e, 1, "hill_giant");
    relocate_to_hand(&mut e, 0, "reanimate");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let life_before = e.state.players[0].life;

    let idx = hand_index_for_card(&e, 0, "reanimate");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![tricerules_proto::ruled::v1::TargetRef {
                object_id: giant,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast reanimate on the opponent's creature");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass())
        .expect("p1 pass — reanimate resolves");

    // Under YOUR control...
    assert!(
        e.state.players[0].battlefield.contains(&giant),
        "the creature is on the caster's battlefield"
    );
    assert!(
        !e.state.players[1].battlefield.contains(&giant),
        "and not the owner's"
    );
    let obj = e.state.objects.get(&giant).expect("obj");
    assert_eq!(obj.controller, 0, "CR 110.2: the caster controls it");
    // ...but still owned by its owner.
    assert_eq!(obj.owner, 1, "CR 108.3: ownership does not change");
    assert!(obj.summoning_sick, "CR 302.6: it is summoning sick");

    // You lose life equal to its mana value ({3}{R} = 4).
    assert_eq!(
        e.state.players[0].life,
        life_before - 4,
        "caster loses life equal to the reanimated card's mana value"
    );
    assert_eq!(e.state.players[1].life, 20, "the owner's life is untouched");
}

/// A reanimated permanent that later dies goes to its OWNER's graveyard (CR 400.3), where its
/// owner — not the player who borrowed it — can target it again.
#[test]
fn reanimated_creature_dies_back_to_its_owners_graveyard() {
    let decks = Some(vec![
        deck_with("swamp", &["reanimate", "lightning_bolt"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(4101, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    relocate_to_hand(&mut e, 0, "reanimate");
    relocate_to_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            r: 1,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "reanimate");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![tricerules_proto::ruled::v1::TargetRef {
                object_id: bears,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast reanimate");
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.players[0].battlefield.contains(&bears));

    // Burn it down.
    let bolt = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(
            bolt,
            vec![tricerules_proto::ruled::v1::TargetRef {
                object_id: bears,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast bolt");
    resolve_entire_stack_two_player(&mut e);

    assert!(
        e.state.players[1].graveyard.contains(&bears),
        "CR 400.3: it returns to its OWNER's graveyard, not the borrower's"
    );
    assert!(
        !e.state.players[0].battlefield.contains(&bears),
        "the borrower's battlefield lets go of it"
    );
    let obj = e.state.objects.get(&bears).expect("obj");
    assert_eq!(obj.controller, obj.owner, "CR 400.7: control resets");
}

/// A reanimated creature's **static ability** is created for the player who now controls it.
/// Captain of the Watch pumps "other Soldier creatures you control": stolen out of its owner's
/// graveyard, it must pump the *thief's* Soldiers and stop pumping the owner's.
#[test]
fn reanimated_static_ability_serves_its_new_controller() {
    let decks = Some(vec![
        deck_with("swamp", &["reanimate"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(4102, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // A Soldier on each side (Fencing Ace is a 1/1 Human Soldier).
    let my_soldier = inject_creature_on_battlefield(&mut e, 0, "fencing_ace");
    let their_soldier = inject_creature_on_battlefield(&mut e, 1, "fencing_ace");
    // `inject_creature_on_battlefield` stamps a 2/2 snapshot; clear it so the card's own P/T and
    // the anthem layer are what we are reading.
    for oid in [my_soldier, their_soldier] {
        if let Some(o) = e.state.objects.get_mut(&oid) {
            o.power = None;
            o.toughness = None;
        }
    }
    let base_power = e.effective_power(my_soldier).expect("power");
    assert_eq!(
        e.effective_power(their_soldier),
        Some(base_power),
        "both Soldiers start equal"
    );

    // The Captain is in the OPPONENT's graveyard; P0 reanimates it.
    let captain = inject_graveyard_card(&mut e, 1, "captain_of_the_watch");
    relocate_to_hand(&mut e, 0, "reanimate");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "reanimate");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![tricerules_proto::ruled::v1::TargetRef {
                object_id: captain,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast reanimate on the Captain");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects.get(&captain).expect("obj").controller,
        0,
        "the Captain is controlled by the caster"
    );

    assert_eq!(
        e.effective_power(my_soldier),
        Some(base_power + 1),
        "the anthem serves its NEW controller's Soldiers"
    );
    assert_eq!(
        e.effective_power(their_soldier),
        Some(base_power),
        "and no longer its owner's, even though they still own the card"
    );
}

/// Reanimate only takes creature cards (`GraveyardCardType::Creature`), so a noncreature card in
/// a graveyard is rejected at cast time rather than fizzling later.
#[test]
fn reanimate_cannot_target_a_noncreature_card() {
    let decks = Some(vec![
        deck_with("swamp", &["reanimate"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(4103, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let anthem = inject_graveyard_card(&mut e, 1, "glorious_anthem");
    relocate_to_hand(&mut e, 0, "reanimate");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "reanimate");
    assert!(
        e.apply_command(
            0,
            &cast_spell(
                idx,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: anthem,
                    damage_amount: 0,
                }],
            ),
        )
        .is_err(),
        "an enchantment in a graveyard is not a legal Reanimate target"
    );
}
