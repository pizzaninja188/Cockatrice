//! Actual-card scenarios for the queue-v18 damage-with-flashback cohort.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c choose the target at
//! cast, CR 120.3 resolves damage, CR 702.34a-d governs the graveyard alternative
//! cost and exile, and CR 608.2b makes a stale target fizzle.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{CastMethod, CastSpell, RuledCommand};

struct Case {
    id: &'static str,
    mana: ManaGift,
    flashback_mana: ManaGift,
    damage: u32,
    card_type: &'static str,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "firebolt",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            flashback_mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            damage: 2,
            card_type: "Sorcery",
        },
        Case {
            id: "fires_of_undeath",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            flashback_mana: ManaGift {
                b: 1,
                c: 5,
                ..Default::default()
            },
            damage: 2,
            card_type: "Instant",
        },
        Case {
            id: "geistflame",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            flashback_mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            damage: 1,
            card_type: "Instant",
        },
        Case {
            id: "tome_blast",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            flashback_mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            damage: 2,
            card_type: "Sorcery",
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

fn flashback_cast(spell: u32, target: u32, generation: u64) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            cast_method: CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(spell, generation)),
            targets: target_object(target),
            ..Default::default()
        })),
    }
}

#[test]
fn damage_flashback_batch_cards_deal_exact_damage_to_players_and_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_850_000 + index as u64 * 2);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_player(1)))
            .unwrap_or_else(|error| panic!("{}: cast at player: {error:?}", case.id));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - case.damage as i32,
            "{}: exact damage to the player",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the resolved spell goes to its owner's graveyard",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: the opponent's hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: the opponent's library is unchanged",
            case.id
        );

        let mut creature_engine = engine(9_850_001 + index as u64 * 2);
        let creature = inject_creature_with_stats(&mut creature_engine, 1, "grizzly_bears", 3, 3);
        give_mana(&mut creature_engine, 0, gift(&case.mana));
        inject_card_into_hand(&mut creature_engine, 0, case.id);
        let creature_slot = hand_index_for_card(&creature_engine, 0, case.id);
        creature_engine
            .apply_scenario_command(0, &cast_spell(creature_slot, target_object(creature)))
            .unwrap_or_else(|error| panic!("{}: cast at creature: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut creature_engine);
        assert_eq!(
            creature_engine.state.objects[&creature].damage, case.damage,
            "{}: exact nonlethal damage is marked on the creature",
            case.id
        );
        assert_eq!(
            creature_engine.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}: three toughness survives the printed damage",
            case.id
        );
        assert_eq!(
            creature_engine.state.players[1].life, 20,
            "{}: damaging a creature does not damage its controller",
            case.id
        );
    }
}

#[test]
fn damage_flashback_batch_cards_flash_back_from_the_graveyard_and_exile() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_851_000 + index as u64);
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_player(1)))
            .unwrap_or_else(|error| panic!("{}: first cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        let generation = e.state.zone_change_generation[&spell];

        give_mana(&mut e, 0, gift(&case.flashback_mana));
        let command = flashback_cast(spell, 1, generation);
        e.apply_scenario_command(0, &command)
            .unwrap_or_else(|error| panic!("{}: flashback: {error:?}", case.id));
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed flashback cost",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - 2 * case.damage as i32,
            "{}: the flashback cast deals its damage again",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Exile,
            "{}: CR 702.34a exiles a spell cast with flashback",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &command).is_err(),
            "{}: an exiled flashback card cannot be cast again",
            case.id
        );
    }
}

#[test]
fn damage_flashback_batch_cards_respect_sorcery_timing_but_instants_use_flashback_any_time() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_852_000 + index as u64);
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_player(1)))
            .unwrap_or_else(|error| panic!("{}: first cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let generation = e.state.zone_change_generation[&spell];
        end_active_turn(&mut e, 0);
        pass_both_players(&mut e);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.priority_player_id(),
            e.state.players[1].id,
            "{}: opponent has priority",
            case.id
        );
        e.apply_scenario_command(e.state.players[1].id, &pass())
            .expect("opponent passes priority");
        give_mana(&mut e, 0, gift(&case.flashback_mana));
        let command = flashback_cast(spell, 1, generation);
        if case.card_type == "Sorcery" {
            assert!(
                e.apply_scenario_command(0, &command).is_err(),
                "{}: a sorcery cannot be flashback-cast on the opponent's turn",
                case.id
            );
            assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        } else {
            e.apply_scenario_command(0, &command)
                .unwrap_or_else(|error| panic!("{}: instant flashback: {error:?}", case.id));
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(
                e.state.objects[&spell].zone,
                Zone::Exile,
                "{}: instant flashback resolved on the opponent's turn",
                case.id
            );
        }
    }
}

#[test]
fn damage_flashback_batch_cards_fizzle_when_the_chosen_creature_left() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_853_000 + index as u64);
        let creature = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
        give_mana(&mut e, 0, gift(&case.mana));
        let spell = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(creature)))
            .expect("cast at the creature");
        move_to_graveyard(&mut e, 1, creature);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: a fizzled spell deals no damage to the player",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the fizzled spell goes to its owner's graveyard",
            case.id
        );

        let generation = e.state.zone_change_generation[&spell];
        let flashback_target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 3, 3);
        give_mana(&mut e, 0, gift(&case.flashback_mana));
        let command = flashback_cast(spell, flashback_target, generation);
        e.apply_scenario_command(0, &command)
            .unwrap_or_else(|error| panic!("{}: flashback at the creature: {error:?}", case.id));
        move_to_graveyard(&mut e, 1, flashback_target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: a fizzled flashback spell deals no damage",
            case.id
        );
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Exile,
            "{}: CR 702.34a exiles a flashback spell even when it fizzles",
            case.id
        );
        assert!(
            !e.state.players[0].graveyard.contains(&spell),
            "{}: the fizzled flashback spell is not put into the graveyard",
            case.id
        );
    }
}

#[test]
fn damage_flashback_batch_cards_reject_a_non_damageable_target_without_spending() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_854_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let pool_before = pool(&e);
        let _slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let rejected_slot = e.state.players[0].hand[hand_slot];
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal damage target",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand[hand_slot], rejected_slot,
            "{}",
            case.id
        );
        assert_eq!(pool(&e), pool_before, "{}: no mana spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
