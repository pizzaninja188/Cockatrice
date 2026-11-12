//! Actual-card scenarios for the reviewed self-pump creature cohort.
//!
//! Expectations are printed Oracle values. CR 602.1a/602.2 govern the activated ability and its
//! cost, CR 611.2a the until-end-of-turn pump, CR 514.2 cleanup expiry, CR 302.6 the tap-cost
//! summoning restriction, CR 107.4e hybrid payment, and CR 107.4f/119.4 Phyrexian payment.
use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1 as rv1;

enum Activation {
    Mana(ManaGift),
    Tap,
}

struct Case {
    id: &'static str,
    cast: ManaGift,
    activation: Activation,
    power: u32,
    toughness: u32,
    pump_power: u32,
    pump_toughness: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "flamekin_brawler",
            cast: ManaGift {
                r: 1,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                r: 1,
                ..Default::default()
            }),
            power: 0,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "immolating_souleater",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                r: 1,
                ..Default::default()
            }),
            power: 1,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "adarkar_sentinel",
            cast: ManaGift {
                c: 5,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                c: 1,
                ..Default::default()
            }),
            power: 3,
            toughness: 3,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "angelfire_crusader",
            cast: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                r: 1,
                ..Default::default()
            }),
            power: 2,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "boa_constrictor",
            cast: ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            activation: Activation::Tap,
            power: 3,
            toughness: 3,
            pump_power: 3,
            pump_toughness: 3,
        },
        Case {
            id: "capashen_templar",
            cast: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                w: 1,
                ..Default::default()
            }),
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "carrion_ants",
            cast: ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                c: 1,
                ..Default::default()
            }),
            power: 0,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "dragon_engine",
            cast: ManaGift {
                c: 3,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                c: 2,
                ..Default::default()
            }),
            power: 1,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "dread_shade",
            cast: ManaGift {
                b: 3,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                b: 1,
                ..Default::default()
            }),
            power: 3,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "fetid_horror",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                b: 1,
                ..Default::default()
            }),
            power: 1,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "firescreamer",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                r: 1,
                ..Default::default()
            }),
            power: 2,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "flame_spirit",
            cast: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                r: 1,
                ..Default::default()
            }),
            power: 2,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "frozen_shade",
            cast: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                b: 1,
                ..Default::default()
            }),
            power: 0,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "hoar_shade",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                b: 1,
                ..Default::default()
            }),
            power: 1,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "honor_guard",
            cast: ManaGift {
                w: 1,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                w: 1,
                ..Default::default()
            }),
            power: 1,
            toughness: 1,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "jousting_dummy",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                c: 3,
                ..Default::default()
            }),
            power: 2,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "kjeldoran_outrider",
            cast: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                w: 1,
                ..Default::default()
            }),
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "llanowar_vanguard",
            cast: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            activation: Activation::Tap,
            power: 1,
            toughness: 1,
            pump_power: 0,
            pump_toughness: 4,
        },
        Case {
            id: "loch_korrigan",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                u: 1,
                ..Default::default()
            }),
            power: 1,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "looming_shade",
            cast: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                b: 1,
                ..Default::default()
            }),
            power: 1,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "molten_ravager",
            cast: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                r: 1,
                ..Default::default()
            }),
            power: 0,
            toughness: 4,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "nantuko_shade",
            cast: ManaGift {
                b: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                b: 1,
                ..Default::default()
            }),
            power: 2,
            toughness: 1,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "narstad_scrapper",
            cast: ManaGift {
                c: 5,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                c: 2,
                ..Default::default()
            }),
            power: 3,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "parapet_watchers",
            cast: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                w: 1,
                ..Default::default()
            }),
            power: 2,
            toughness: 2,
            pump_power: 0,
            pump_toughness: 1,
        },
        Case {
            id: "primeval_shambler",
            cast: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                b: 1,
                ..Default::default()
            }),
            power: 3,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 1,
        },
        Case {
            id: "ridgeline_rager",
            cast: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                r: 1,
                ..Default::default()
            }),
            power: 1,
            toughness: 2,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "sea_spirit",
            cast: ManaGift {
                u: 1,
                c: 4,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                u: 1,
                ..Default::default()
            }),
            power: 2,
            toughness: 3,
            pump_power: 1,
            pump_toughness: 0,
        },
        Case {
            id: "storm_shaman",
            cast: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            activation: Activation::Mana(ManaGift {
                r: 1,
                ..Default::default()
            }),
            power: 0,
            toughness: 4,
            pump_power: 1,
            pump_toughness: 0,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
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

fn clear_summoning_sickness(e: &mut GameEngine, creature: u32) {
    e.state
        .objects
        .get_mut(&creature)
        .expect("creature")
        .summoning_sick = false;
}

fn activate_pump(e: &mut GameEngine, creature: u32, activation: &Activation, id: &str) {
    match activation {
        Activation::Mana(mana) => {
            give_mana(e, 0, gift(mana));
            apply_ability(e, 0, creature, 0, vec![]).expect("activate the self-pump");
            assert_pool_empty(e, id);
        }
        Activation::Tap => {
            apply_ability(e, 0, creature, 0, vec![]).expect("activate the tap self-pump");
            assert!(
                e.state.objects[&creature].tapped,
                "{id}: the tap cost taps the source"
            );
        }
    }
    assert_eq!(e.state.stack.len(), 1, "{id}: one ability is on the stack");
    resolve_entire_stack_two_player(e);
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

#[test]
fn pump_self_cohort_cards_cast_with_exact_printed_characteristics() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(770_000 + case_index as u64);
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
fn pump_self_cohort_cards_activate_the_exact_self_pump_without_targeting() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(771_000 + case_index as u64);
        let creature = cast_creature(&mut e, case.id, case.cast);
        if matches!(case.activation, Activation::Tap) {
            clear_summoning_sickness(&mut e, creature);
        }
        let bystander = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        activate_pump(&mut e, creature, &case.activation, case.id);
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
fn pump_self_cohort_cards_pump_expires_at_cleanup() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(772_000 + case_index as u64);
        let creature = cast_creature(&mut e, case.id, case.cast);
        if matches!(case.activation, Activation::Tap) {
            clear_summoning_sickness(&mut e, creature);
        }
        activate_pump(&mut e, creature, &case.activation, case.id);
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
fn pump_self_cohort_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(773_000 + case_index as u64);
        let creature = cast_creature(&mut e, case.id, case.cast);
        let before_pool = pool(&e);
        let before_command_index = e.state.command_index;
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: the self-pump needs its exact cost",
            case.id
        );
        assert_eq!(
            pool(&e),
            before_pool,
            "{}: a rejected activation spends no mana",
            case.id
        );
        if !matches!(case.activation, Activation::Tap) {
            assert!(
                !e.state.objects[&creature].tapped,
                "{}: a rejected mana activation does not tap",
                case.id
            );
        }
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

// The activation has no target, so a stale-target path is N/A. Hidden zones and public board
// state are checked by the actual-card scenarios above.

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
fn immolating_souleater_pays_phyrexian_with_red_mana_or_two_life() {
    let cast = ManaGift {
        c: 2,
        ..Default::default()
    };

    let mut red = engine(774_000);
    let creature = cast_creature(
        &mut red,
        "immolating_souleater",
        ManaGift {
            c: cast.c,
            ..Default::default()
        },
    );
    give_mana(
        &mut red,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    activate_pump_flex(
        &mut red,
        creature,
        vec![rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: false,
            mana_color: 0,
        }],
        "immolating_souleater red",
    );
    assert_eq!(red.state.players[0].life, 20);
    assert_eq!(pool(&red), (0, 0, 0, 0, 0, 0));
    assert_eq!(red.effective_power(creature), Some(2));

    let mut life = engine(774_001);
    let creature = cast_creature(
        &mut life,
        "immolating_souleater",
        ManaGift {
            c: cast.c,
            ..Default::default()
        },
    );
    activate_pump_flex(
        &mut life,
        creature,
        vec![rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
            mana_color: 0,
        }],
        "immolating_souleater life",
    );
    assert_eq!(life.state.players[0].life, 18);
    assert_eq!(life.effective_power(creature), Some(2));

    let mut one_life = engine(774_002);
    let creature = cast_creature(
        &mut one_life,
        "immolating_souleater",
        ManaGift {
            c: cast.c,
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
    assert_eq!(one_life.effective_power(creature), Some(1));
    assert!(one_life.state.stack.is_empty());
    assert_eq!(one_life.state.command_index, before_command_index);
}

#[test]
fn tap_cost_cards_require_a_ready_untapped_source() {
    let cases = [
        (
            775_000,
            "boa_constrictor",
            ManaGift {
                g: 1,
                c: 4,
                ..Default::default()
            },
            3,
            3,
            3,
            3,
        ),
        (
            775_001,
            "llanowar_vanguard",
            ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            1,
            1,
            0,
            4,
        ),
    ];
    for (seed, id, cast, power, toughness, pump_power, pump_toughness) in cases {
        let mut e = engine(seed);
        let creature = cast_creature(&mut e, id, cast);
        let before_command_index = e.state.command_index;
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{id}: a summoning-sick creature cannot pay a tap cost"
        );
        assert!(!e.state.objects[&creature].tapped);
        assert_eq!(e.effective_power(creature), Some(power));
        assert_eq!(e.effective_toughness(creature), Some(toughness));
        assert_eq!(e.state.command_index, before_command_index);

        clear_summoning_sickness(&mut e, creature);
        apply_ability(&mut e, 0, creature, 0, vec![]).expect("activate the tap self-pump");
        assert!(e.state.objects[&creature].tapped);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(creature), Some(power + pump_power));
        assert_eq!(
            e.effective_toughness(creature),
            Some(toughness + pump_toughness)
        );

        let before_command_index = e.state.command_index;
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{id}: an already tapped creature cannot pay a tap cost"
        );
        assert_eq!(
            e.effective_power(creature),
            Some(power + pump_power),
            "{id}: the rejected activation does not pump again"
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(toughness + pump_toughness)
        );
        assert_eq!(e.state.command_index, before_command_index);
    }
}

#[test]
fn hybrid_cards_accept_each_color_and_reject_an_off_color() {
    let cases = [
        (
            776_000,
            "loch_korrigan",
            ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            ManaGift {
                u: 1,
                ..Default::default()
            },
            ManaGift {
                b: 1,
                ..Default::default()
            },
            ManaGift {
                g: 1,
                ..Default::default()
            },
            1,
            1,
        ),
        (
            776_001,
            "parapet_watchers",
            ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            ManaGift {
                w: 1,
                ..Default::default()
            },
            ManaGift {
                u: 1,
                ..Default::default()
            },
            ManaGift {
                b: 1,
                ..Default::default()
            },
            0,
            1,
        ),
    ];
    for (seed, id, cast, first, second, wrong, pump_power, pump_toughness) in cases {
        let base_power = printed_power(id);
        let base_toughness = printed_toughness(id);
        for (branch, mana) in [("first", first), ("second", second)] {
            let mut e = engine(seed + u64::from(branch == "second"));
            let creature = cast_creature(&mut e, id, gift(&cast));
            give_mana(&mut e, 0, gift(&mana));
            apply_ability(&mut e, 0, creature, 0, vec![])
                .unwrap_or_else(|error| panic!("{id} {branch} hybrid branch: {error}"));
            assert_pool_empty(&e, id);
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(
                e.effective_power(creature),
                Some(base_power + pump_power),
                "{id} {branch} branch pumps power"
            );
            assert_eq!(
                e.effective_toughness(creature),
                Some(base_toughness + pump_toughness),
                "{id} {branch} branch pumps toughness"
            );
        }

        let mut e = engine(seed + 2);
        let creature = cast_creature(&mut e, id, cast);
        give_mana(&mut e, 0, wrong);
        let before_pool = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{id}: an off-color mana can't pay the hybrid symbol"
        );
        assert_eq!(pool(&e), before_pool);
        assert_eq!(e.effective_power(creature), Some(base_power));
        assert_eq!(e.effective_toughness(creature), Some(base_toughness));
        assert!(e.state.stack.is_empty());
    }
}

fn printed_power(id: &str) -> u32 {
    match id {
        "loch_korrigan" => 1,
        "parapet_watchers" => 2,
        other => panic!("unexpected hybrid card {other}"),
    }
}

fn printed_toughness(id: &str) -> u32 {
    match id {
        "loch_korrigan" => 1,
        "parapet_watchers" => 2,
        other => panic!("unexpected hybrid card {other}"),
    }
}
