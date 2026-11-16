//! Actual-card scenarios for the queue-v20 activated regeneration cohort.
//!
//! Expectations are printed Oracle values. CR 701.19a/b define regeneration as a
//! replacement shield created by the activated ability; CR 602.2 activates it,
//! CR 118.3 requires the exact printed mana and CR 514.2 expires an unused shield.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    regenerate: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "asphodel_wanderer",
            cast: ManaGift {
                b: 1,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "odious_trow",
            cast: ManaGift {
                b: 1,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "pewter_golem",
            cast: ManaGift {
                c: 5,
                ..Default::default()
            },
            regenerate: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            power: 4,
            toughness: 2,
        },
        Case {
            id: "selesnya_sentry",
            cast: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            regenerate: ManaGift {
                g: 1,
                c: 5,
                ..Default::default()
            },
            power: 3,
            toughness: 2,
        },
        Case {
            id: "tangle_hulk",
            cast: ManaGift {
                c: 5,
                ..Default::default()
            },
            regenerate: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 5,
            toughness: 3,
        },
        Case {
            id: "votary_of_the_conclave",
            cast: ManaGift {
                w: 1,
                ..Default::default()
            },
            regenerate: ManaGift {
                g: 1,
                c: 2,
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

fn cast_creature(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.cast));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact full cost", case.id);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{}: resolves onto the battlefield",
        case.id
    );
    slot
}

fn activate_regeneration(e: &mut GameEngine, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, creature, 0, vec![]).expect("activate regeneration");
    pass_both_players(e);
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{id}: exact regeneration cost consumed"
    );
    assert_eq!(
        e.state.objects[&creature].regeneration_shields, 1,
        "{id}: one regeneration shield"
    );
}

#[test]
fn regenerate_activated_batch_cards_cast_with_exact_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_960_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
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
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn regenerate_activated_batch_cards_place_a_shield_with_the_exact_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_961_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
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
fn regenerate_activated_batch_cards_survive_lethal_damage_by_consuming_the_shield() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_962_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
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
fn regenerate_activated_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_963_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: regeneration needs its exact mana",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}: no shield is created",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}: no stack object", case.id);
    }
}

#[test]
fn regenerate_activated_batch_cards_expire_an_unused_shield_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_964_000 + index as u64);
        let creature = cast_creature(&mut e, &case);
        activate_regeneration(&mut e, creature, gift(&case.regenerate), case.id);
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.state.objects[&creature].regeneration_shields, 0,
            "{}: CR 514.2 removes an unused shield",
            case.id
        );
    }
}
