//! Actual-card scenarios for the reviewed flying self-pump creature family.
//!
//! Expectations are printed Oracle values. CR 702.9a makes flying an evasion ability, so a
//! ground creature cannot block the attacker while a flying creature can. CR 602.2 governs the
//! activated ability, CR 611.2a the until-end-of-turn pump, and CR 514.2 its cleanup expiry.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

struct Case {
    id: &'static str,
    cast: ManaGift,
    pump: ManaGift,
    power: u32,
    toughness: u32,
    pump_power: u32,
    pump_toughness: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "dragon_hatchling",
            cast: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            pump: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "moltensteel_dragon",
            cast: ManaGift {
                r: 2,
                c: 4,
                ..Default::default()
            },
            pump: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 4,
            toughness: 4,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "aven_flock",
            cast: ManaGift {
                w: 1,
                c: 4,
                ..Default::default()
            },
            pump: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 3,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "balshan_collaborator",
            cast: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            pump: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "blistering_dieflyn",
            cast: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            pump: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "drifting_shade",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            pump: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "dungeon_shade",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            pump: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "firefly",
            cast: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
            pump: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "furnace_whelp",
            cast: ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
            pump: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "granite_gargoyle",
            cast: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            pump: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "hellkite_punisher",
            cast: ManaGift {
                r: 2,
                c: 5,
                ..Default::default()
            },
            pump: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 6,
            toughness: 6,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "killer_bees",
            cast: ManaGift {
                g: 2,
                c: 1,
                ..Default::default()
            },
            pump: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "marble_gargoyle",
            cast: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            pump: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "moonwing_moth",
            cast: ManaGift {
                w: 2,
                c: 1,
                ..Default::default()
            },
            pump: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "tower_drake",
            cast: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            pump: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
            pump_power: 0,
            pump_toughness: 1,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let mut e = game_start_engine(seed);
    advance_to_main1_from_game_start(&mut e);
    e
}

fn game_start_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine")
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn assert_pool_empty(e: &GameEngine, id: &str) {
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cost consumed");
}

fn cast_creature(e: &mut GameEngine, id: &str, mana: ManaGift) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    semantic::accepted(e, 0, &cast_spell(hand_slot, vec![]));
    assert_pool_empty(e, id);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{id}: resolves onto the battlefield"
    );
    assert!(
        !e.state.players[0].hand.contains(&slot),
        "{id}: the cast card leaves its owner's hand"
    );
    slot
}

fn advance_flying_attack_to_manual_blocks(e: &mut GameEngine, id: &str) -> u32 {
    advance_to_declare_attackers(e);
    let flyer = inject_creature_on_battlefield(e, 0, id);
    e.apply_scenario_command(0, &declare_attackers(vec![flyer]))
        .expect("declare the flying attacker");
    e.apply_scenario_command(0, &pass())
        .expect("active player passes declare attackers");
    e.apply_scenario_command(1, &pass())
        .expect("defending player passes declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    flyer
}

fn activate_pump(e: &mut GameEngine, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, creature, 0, vec![]).expect("activate the self-pump");
    assert_pool_empty(e, id);
    assert_eq!(e.state.stack.len(), 1, "{id}: one ability is on the stack");
    assert!(
        !e.state.objects[&creature].tapped,
        "{id}: the activated ability does not tap"
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn pump_self_batch_cards_cast_with_exact_printed_characteristics() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(790_000 + case_index as u64);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        let creature = cast_creature(&mut e, case.id, case.cast);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power),
            "{}: exact power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{}: exact toughness",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::Flying),
            "{}: printed flying",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].owner, e.state.players[0].id,
            "{}: owner",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].controller, e.state.players[0].id,
            "{}: controller",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: opponent hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: opponent library is unchanged",
            case.id
        );
    }
}

#[test]
fn pump_self_batch_flying_rejects_ground_block_and_accepts_flying_block() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = game_start_engine(791_000 + case_index as u64);
        let ground_blocker = inject_creature_on_battlefield(&mut e, 1, "coral_merfolk");
        let flying_blocker = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
        let attacker = advance_flying_attack_to_manual_blocks(&mut e, case.id);
        let rejected = e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: ground_blocker,
            }]),
        );
        assert!(
            matches!(rejected, Err(tricerules_core::EngineError::Illegal(_))),
            "{}: a ground-only creature cannot block a flyer",
            case.id
        );
        assert!(
            !e.state.combat.as_ref().expect("combat").blockers_declared,
            "{}: the rejected block leaves blockers undeclared",
            case.id
        );
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: flying_blocker,
            }]),
        )
        .expect("a flying creature can block a flyer");
    }
}

#[test]
fn pump_self_batch_cards_activate_the_exact_self_pump_without_targeting() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(792_000 + case_index as u64);
        let creature = cast_creature(&mut e, case.id, case.cast);
        let bystander = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        activate_pump(&mut e, creature, case.pump, case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power + case.pump_power),
            "{}: exact power pump",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness + case.pump_toughness),
            "{}: exact toughness pump",
            case.id
        );
        assert_eq!(
            e.effective_power(bystander),
            Some(2),
            "{}: only the source is pumped",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bystander),
            Some(2),
            "{}: the opponent's creature is unchanged",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}: the source stays on the battlefield",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: opponent hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: opponent library is unchanged",
            case.id
        );
    }
}

#[test]
fn pump_self_batch_cards_pump_expires_at_cleanup() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(793_000 + case_index as u64);
        let creature = cast_creature(&mut e, case.id, case.cast);
        activate_pump(&mut e, creature, case.pump, case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power + case.pump_power),
            "{}: the pump is active",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power),
            "{}: CR 514.2 restores power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{}: CR 514.2 restores toughness",
            case.id
        );
    }
}

#[test]
fn pump_self_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(794_000 + case_index as u64);
        let creature = cast_creature(&mut e, case.id, case.cast);
        let before_pool = pool(&e);
        let before_command_index = e.state.command_index;
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: the self-pump needs its exact mana",
            case.id
        );
        assert_eq!(
            pool(&e),
            before_pool,
            "{}: a rejected activation spends no mana",
            case.id
        );
        assert_eq!(
            e.effective_power(creature),
            Some(case.power),
            "{}: a rejected activation does not pump",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{}: a rejected activation does not pump toughness",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: a rejected activation adds no stack object",
            case.id
        );
        assert_eq!(
            e.state.command_index, before_command_index,
            "{}: a rejected activation changes no command position",
            case.id
        );
    }
}

// The activation has no target (CR 602.2 still requires paying its cost), so a stale-target
// path is N/A. Public offers and hidden zones are checked by the actual-card scenarios above.

fn activate_pump_flex(e: &mut GameEngine, creature: u32, flex: Vec<rv1::FlexPipPayment>, id: &str) {
    let mut command = activate_ability_for(e, creature, 0, vec![]);
    let Some(rv1::ruled_command::Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        panic!("{id}: activation command");
    };
    activation.flex_payments = flex;
    semantic::accepted(e, 0, &command);
    assert_eq!(e.state.stack.len(), 1, "{id}: one ability is on the stack");
    resolve_entire_stack_two_player(e);
}

#[test]
fn moltensteel_dragon_pays_phyrexian_with_red_mana_or_two_life() {
    // Cast with red mana for both Phyrexian symbols.
    let mut red_cast = engine(795_000);
    let creature = cast_creature(
        &mut red_cast,
        "moltensteel_dragon",
        ManaGift {
            r: 2,
            c: 4,
            ..Default::default()
        },
    );
    assert_eq!(red_cast.state.players[0].life, 20);
    assert_eq!(red_cast.effective_power(creature), Some(4));

    // Cast with four life instead of the two red symbols.
    let mut life_cast = engine(795_001);
    give_mana(
        &mut life_cast,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let slot = inject_card_into_hand(&mut life_cast, 0, "moltensteel_dragon");
    let hand_slot = hand_index_for_card(&life_cast, 0, "moltensteel_dragon");
    semantic::accepted(
        &mut life_cast,
        0,
        &cast_spell_flex(
            hand_slot,
            vec![],
            vec![
                rv1::FlexPipPayment {
                    pip_index: 1,
                    pay_life: true,
                    mana_color: 0,
                },
                rv1::FlexPipPayment {
                    pip_index: 2,
                    pay_life: true,
                    mana_color: 0,
                },
            ],
        ),
    );
    assert_eq!(
        life_cast.state.players[0].life, 16,
        "two symbols cost 4 life"
    );
    assert_eq!(pool(&life_cast), (0, 0, 0, 0, 0, 0));
    resolve_entire_stack_two_player(&mut life_cast);
    assert_eq!(life_cast.state.objects[&slot].zone, Zone::Battlefield);
    assert_eq!(life_cast.effective_power(slot), Some(4));

    // Activate with red mana.
    let mut red_activation = engine(795_002);
    let creature = cast_creature(
        &mut red_activation,
        "moltensteel_dragon",
        ManaGift {
            r: 2,
            c: 4,
            ..Default::default()
        },
    );
    give_mana(
        &mut red_activation,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    activate_pump_flex(
        &mut red_activation,
        creature,
        vec![rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: false,
            mana_color: 0,
        }],
        "moltensteel_dragon red activation",
    );
    assert_eq!(red_activation.state.players[0].life, 20);
    assert_eq!(pool(&red_activation), (0, 0, 0, 0, 0, 0));
    assert_eq!(red_activation.effective_power(creature), Some(5));

    // Activate with two life.
    let mut life_activation = engine(795_003);
    let creature = cast_creature(
        &mut life_activation,
        "moltensteel_dragon",
        ManaGift {
            r: 2,
            c: 4,
            ..Default::default()
        },
    );
    activate_pump_flex(
        &mut life_activation,
        creature,
        vec![rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
            mana_color: 0,
        }],
        "moltensteel_dragon life activation",
    );
    assert_eq!(life_activation.state.players[0].life, 18);
    assert_eq!(life_activation.effective_power(creature), Some(5));

    // At one life the Phyrexian life branch is illegal (CR 119.4).
    let mut one_life = engine(795_004);
    let creature = cast_creature(
        &mut one_life,
        "moltensteel_dragon",
        ManaGift {
            r: 2,
            c: 4,
            ..Default::default()
        },
    );
    one_life.state.players[0].life = 1;
    let before_command_index = one_life.state.command_index;
    let mut command = activate_ability_for(&one_life, creature, 0, vec![]);
    let Some(rv1::ruled_command::Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        panic!("activation command");
    };
    activation.flex_payments = vec![rv1::FlexPipPayment {
        pip_index: 0,
        pay_life: true,
        mana_color: 0,
    }];
    assert!(
        one_life.apply_scenario_command(0, &command).is_err(),
        "at one life, the two-life branch cannot be paid"
    );
    assert_eq!(one_life.state.players[0].life, 1);
    assert_eq!(one_life.effective_power(creature), Some(4));
    assert!(one_life.state.stack.is_empty());
    assert_eq!(one_life.state.command_index, before_command_index);
}

#[test]
fn blistering_dieflyn_hybrid_cost_accepts_either_color_and_rejects_green() {
    let cast = ManaGift {
        r: 1,
        c: 3,
        ..Default::default()
    };

    for (seed, mana, label) in [
        (
            796_000,
            ManaGift {
                b: 1,
                ..Default::default()
            },
            "black",
        ),
        (
            796_001,
            ManaGift {
                r: 1,
                ..Default::default()
            },
            "red",
        ),
    ] {
        let mut e = engine(seed);
        let creature = cast_creature(
            &mut e,
            "blistering_dieflyn",
            ManaGift {
                r: cast.r,
                c: cast.c,
                ..Default::default()
            },
        );
        activate_pump(
            &mut e,
            creature,
            mana,
            &format!("blistering_dieflyn {label}"),
        );
        assert_eq!(e.effective_power(creature), Some(1), "{label} branch pumps");
        assert_eq!(e.effective_toughness(creature), Some(1), "{label} branch");
    }

    let mut e = engine(796_002);
    let creature = cast_creature(
        &mut e,
        "blistering_dieflyn",
        ManaGift {
            r: cast.r,
            c: cast.c,
            ..Default::default()
        },
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let before_pool = pool(&e);
    assert!(
        apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
        "green mana cannot pay a black/red hybrid symbol"
    );
    assert_eq!(pool(&e), before_pool);
    assert_eq!(e.effective_power(creature), Some(0));
    assert_eq!(e.effective_toughness(creature), Some(1));
    assert!(e.state.stack.is_empty());
}
