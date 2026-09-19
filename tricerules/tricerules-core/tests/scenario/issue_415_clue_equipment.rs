//! Issue #415 focused scenarios for the five retained Clue Equipment identities.
//!
//! CR 301.5 / 301.5a define the equipped creature; CR 702.6a makes each printed `Equip {N}` an
//! independent activated ability. CR 602.2b and 601.2h order the shared activation's costs, and
//! CR 701.21a makes `Sacrifice this Equipment` an atomic self-sacrifice paid before resolution.
//! CR 121.1 draws the card only on resolution. CR 508.1m stages Candlestick's granted attack
//! trigger, whose Surveil 2 uses the shipped private library partition (CR 701.25). CR 603.6c /
//! 603.10a make Lead Pipe's look-back equipped-creature-dies trigger drain each opponent. CR
//! 509.1b enforces Rope's one-blocker cap, and CR 611.3 / 613.1f / 613.4c apply the attached
//! static modifiers.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{AttachmentRecipient, TurnStep, Zone};
use tricerules_proto::ruled::v1::{BlockPair, ChoiceKind};

const COHORT: &[&str] = &["candlestick", "knife", "lead_pipe", "rope", "wrench"];

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", COHORT), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("issue #415 engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn engine_with_card(seed: u64, card_id: &str) -> (GameEngine, u32) {
    let mut engine = engine(seed);
    let equipment = move_ready_to_battlefield(&mut engine, 0, card_id);
    (engine, equipment)
}

/// Enter `card_id` through the engine and attach it directly so the static ability's continuous
/// effect applies without spending the scenario on an equip activation.
fn equip_to(engine: &mut GameEngine, card_id: &str, creature: u32) -> u32 {
    let equipment = move_ready_to_battlefield(engine, 0, card_id);
    engine
        .state
        .objects
        .get_mut(&equipment)
        .expect("equipment object")
        .attached_to = Some(AttachmentRecipient::Object(creature));
    equipment
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

#[test]
fn issue_415_each_clue_equipment_sacrifices_itself_to_draw_one() {
    for (offset, card_id) in COHORT.iter().enumerate() {
        let (mut engine, equipment) = engine_with_card(415_001 + offset as u64, card_id);
        let _drawn = seat_on_top(&mut engine, 0, &["storm_crow"]);
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let hand_before = engine.state.players[0].hand.len();

        apply_ability(&mut engine, 0, equipment, 0, vec![])
            .unwrap_or_else(|error| panic!("{card_id} activation: {error}"));
        assert_eq!(
            engine.state.players[0].mana_pool.colorless, 0,
            "{card_id} pays {{2}} as the activation cost"
        );
        assert_eq!(
            engine.state.objects[&equipment].zone,
            Zone::Graveyard,
            "{card_id} sacrifices itself as the activation cost"
        );
        assert_eq!(engine.state.stack.len(), 1, "{card_id} waits on the stack");
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before,
            "{card_id} draws only on resolution"
        );

        resolve_entire_stack_two_player(&mut engine);
        assert_eq!(
            engine.state.players[0].hand.len(),
            hand_before + 1,
            "{card_id} draws exactly one card"
        );
        assert_eq!(engine.state.objects[&_drawn[0]].zone, Zone::Hand);
    }
}

#[test]
fn issue_415_shared_activation_requires_battlefield_and_mana_and_is_atomic() {
    let (mut engine, equipment) = engine_with_card(415_020, "wrench");
    let _drawn = seat_on_top(&mut engine, 0, &["storm_crow"]);

    // No mana: the whole activation is rejected without partial payment.
    let hand_before = engine.state.players[0].hand.len();
    let before_index = engine.state.command_index;
    assert!(
        apply_ability(&mut engine, 0, equipment, 0, vec![]).is_err(),
        "{{2}} is required"
    );
    assert_eq!(engine.state.command_index, before_index);
    assert_eq!(engine.state.objects[&equipment].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.players[0].hand.len(), hand_before);

    // An Equipment in hand is not on the battlefield, so its battlefield activation is illegal.
    let in_hand = inject_card_into_hand(&mut engine, 0, "candlestick");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let mana_before = engine.state.players[0].mana_pool;
    assert!(
        apply_ability(&mut engine, 0, in_hand, 0, vec![]).is_err(),
        "the source must be on the battlefield"
    );
    assert_eq!(engine.state.objects[&in_hand].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].mana_pool, mana_before);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);

    // Activate once: the sacrifice moves the source to the graveyard, so it is gone as a source.
    apply_ability(&mut engine, 0, equipment, 0, vec![]).expect("first activation");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&equipment].zone, Zone::Graveyard);
    let hand_after_draw = engine.state.players[0].hand.len();
    assert!(
        apply_ability(&mut engine, 0, equipment, 0, vec![]).is_err(),
        "a sacrificed Equipment cannot be activated again"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_after_draw);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_415_candlestick_granted_attack_trigger_surveils_two_privately() {
    let mut engine = engine(415_040);
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    equip_to(&mut engine, "candlestick", attacker);
    let top = seat_on_top(&mut engine, 0, &["storm_crow", "hill_giant"]);
    engine.apply_command(0, &pass()).expect("attacker passes");
    engine.apply_command(1, &pass()).expect("defender passes");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);

    assert_eq!(
        engine.effective_power(attacker),
        Some(3),
        "the attached +1/+1 applies before the attack"
    );
    assert!(
        engine.state.stack.is_empty(),
        "the grant alone does not trigger"
    );

    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the equipped attacker");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the granted attack trigger reaches the stack"
    );

    let resolution = resolve_top_stack(&mut engine);
    let choice = find_resolution_choice(&resolution).expect("Surveil 2 choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!((choice.min, choice.max), (0, 2));
    assert_eq!(
        choice.candidate_object_ids, top,
        "the private choice is exactly the top two"
    );
    assert!(
        choice.public_reveal.is_none(),
        "surveil stays hidden from the opponent"
    );

    let ordering = engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("keep both surveilled cards on top");
    let order_choice = find_resolution_choice(&ordering).expect("top-order choice");
    assert_eq!((order_choice.min, order_choice.max), (2, 2));
    engine
        .apply_command(0, &submit_resolution_choice(vec![top[0], top[1]]))
        .expect("order the retained cards");
    assert_eq!(
        engine.state.players[0]
            .library
            .iter()
            .take(2)
            .copied()
            .collect::<Vec<_>>(),
        vec![top[1], top[0]]
    );
}

#[test]
fn issue_415_knife_bonus_applies_only_during_its_controllers_turn() {
    let mut engine = engine(415_050);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    equip_to(&mut engine, "knife", attacker);

    assert_eq!(engine.effective_power(attacker), Some(3));
    assert_eq!(engine.effective_toughness(attacker), Some(2));
    assert!(engine.effective_has_keyword(attacker, Keyword::FirstStrike));

    end_active_turn(&mut engine, 0);
    assert!(
        engine.state.active_player_id() == 1,
        "the opponent's turn has begun"
    );
    assert_eq!(
        engine.effective_power(attacker),
        Some(2),
        "the bonus is off during the opponent's turn"
    );
    assert!(!engine.effective_has_keyword(attacker, Keyword::FirstStrike));
}

#[test]
fn issue_415_lead_pipe_drains_each_opponent_when_the_equipped_creature_dies() {
    let mut engine = engine(415_060);
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let pipe = equip_to(&mut engine, "lead_pipe", creature);

    assert_eq!(
        engine.effective_power(creature),
        Some(4),
        "the attached +2/+0 applies"
    );

    engine
        .state
        .objects
        .get_mut(&creature)
        .expect("creature")
        .damage = 2;
    engine
        .apply_command(0, &pass())
        .expect("state-based actions kill the equipped creature");
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&pipe].zone,
        Zone::Battlefield,
        "the Equipment stays after its host dies"
    );
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the look-back dies trigger reaches the stack"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 19);
}

#[test]
fn issue_415_rope_grants_reach_and_caps_blockers_at_one() {
    let mut engine = engine(415_070);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker_a = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let blocker_b = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    equip_to(&mut engine, "rope", attacker);

    assert_eq!(engine.effective_power(attacker), Some(3));
    assert_eq!(engine.effective_toughness(attacker), Some(4));
    assert!(engine.effective_has_keyword(attacker, Keyword::Reach));
    assert!(
        zone_view_rules_annotation_labels(&mut engine, 0, attacker)
            .iter()
            .any(|label| label == "Can't be blocked by more than 1 creature"),
        "the blocker cap is public and engine-authoritative"
    );

    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine.apply_command(0, &pass()).expect("attacker passes");
    engine.apply_command(1, &pass()).expect("defender passes");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the equipped attacker");
    engine
        .apply_command(0, &pass())
        .expect("attacker passes after declaration");
    engine
        .apply_command(1, &pass())
        .expect("defender passes after declaration");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);

    let before_index = engine.state.command_index;
    assert!(
        engine
            .apply_command(
                1,
                &declare_blockers(vec![
                    BlockPair {
                        attacker_id: attacker,
                        blocker_id: blocker_a,
                    },
                    BlockPair {
                        attacker_id: attacker,
                        blocker_id: blocker_b,
                    },
                ]),
            )
            .is_err(),
        "two creatures cannot block the equipped attacker"
    );
    assert_eq!(engine.state.command_index, before_index);

    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_a,
            }]),
        )
        .expect("a single blocker remains legal");
}

#[test]
fn issue_415_wrench_grants_vigilance_and_a_tap_ability() {
    let mut engine = engine(415_080);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    equip_to(&mut engine, "wrench", attacker);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    assert_eq!(engine.effective_power(attacker), Some(3));
    assert_eq!(engine.effective_toughness(attacker), Some(3));
    assert!(engine.effective_has_keyword(attacker, Keyword::Vigilance));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, attacker, 0, target_object(target))
        .expect("activate the granted tap ability");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(
        engine.state.objects[&attacker].tapped,
        "the {{T}} cost taps the equipped creature"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.objects[&target].tapped, "the target is tapped");

    // The granted ability is part of the attached static, so it disappears with the Equipment.
    let wrench = battlefield_object_for_card(&engine, 0, "wrench");
    engine.state.objects.get_mut(&wrench).expect("wrench").zone = Zone::Graveyard;
    assert!(
        apply_ability(&mut engine, 0, attacker, 0, target_object(target)).is_err(),
        "the granted ability no longer exists without its Equipment"
    );
}
