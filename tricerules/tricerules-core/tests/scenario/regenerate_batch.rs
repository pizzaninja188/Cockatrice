//! Actual-card scenarios for the reviewed regenerate creature family.
//!
//! Expectations are printed Oracle values. CR 701.19a/b define regeneration as a replacement
//! effect created by the activated ability; CR 701.19c makes "can't be regenerated" bypass it.
//! The scenario proves exact costs, shield placement, lethal-damage survival, and rejection when
//! the cost cannot be paid. Rimebound Dead is excluded: its `{S}` cost is blocked by issue #516.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    regenerate: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "ancient_silverback",
            cast: ManaGift {
                g: 2,
                c: 4,
                ..Default::default()
            },
            regenerate: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 6,
            toughness: 5,
        },
        Case {
            id: "clay_statue",
            cast: ManaGift {
                c: 4,
                ..Default::default()
            },
            regenerate: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 1,
        },
        Case {
            id: "diabolic_machine",
            cast: ManaGift {
                c: 7,
                ..Default::default()
            },
            regenerate: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 4,
            toughness: 4,
        },
        Case {
            id: "dutiful_thrull",
            cast: ManaGift {
                w: 1,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "horned_troll",
            cast: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            regenerate: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "metathran_zombie",
            cast: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "phyrexian_monitor",
            cast: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "restless_dead",
            cast: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "revered_dead",
            cast: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            regenerate: ManaGift {
                w: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "skeletal_wurm",
            cast: ManaGift {
                b: 1,
                c: 7,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 7,
            toughness: 6,
        },
        Case {
            id: "unworthy_dead",
            cast: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "uthden_troll",
            cast: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            regenerate: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "walking_dead",
            cast: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
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

fn activate_regeneration(e: &mut GameEngine, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, creature, 0, vec![]).expect("activate regeneration");
    pass_both_players(e);
    assert_pool_empty(e, id);
    assert_eq!(
        e.state.objects[&creature].regeneration_shields, 1,
        "{id}: one regeneration shield"
    );
}

#[test]
fn regenerate_batch_cards_cast_with_exact_printed_characteristics() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(780_000 + case_index as u64);
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
fn regenerate_batch_cards_place_a_shield_with_the_exact_cost() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(781_000 + case_index as u64);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        activate_regeneration(&mut e, creature, gift(&case.regenerate), case.id);
        assert!(
            !e.state.objects[&creature].tapped,
            "{}: placing a shield does not tap",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].damage, 0,
            "{}: placing a shield does not change damage",
            case.id
        );
    }
}

#[test]
fn regenerate_batch_cards_survive_lethal_damage_by_consuming_the_shield() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(782_000 + case_index as u64);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        activate_regeneration(&mut e, creature, gift(&case.regenerate), case.id);
        e.state.objects.get_mut(&creature).expect("creature").damage = case.toughness;
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}: regeneration replaces destruction",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}: the shield is consumed",
            case.id
        );
        assert!(
            e.state.objects[&creature].tapped,
            "{}: regeneration taps the creature",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].damage, 0,
            "{}: regeneration clears damage",
            case.id
        );
    }
}

#[test]
fn regenerate_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(783_000 + case_index as u64);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: regeneration needs its exact mana",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: a rejected activation spends no mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}: a rejected activation creates no shield",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: a rejected activation adds no stack object",
            case.id
        );
    }
}
