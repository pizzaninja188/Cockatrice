//! Actual-card scenarios for the reviewed green regeneration cohort.
//!
//! Expectations are printed Oracle values. CR 701.19a/b define regeneration as a
//! replacement shield created by the activated ability; CR 602.2 activates it and
//! CR 118.3 requires the exact printed mana. The scenario proves precise costs,
//! shield placement, lethal-damage survival, and rejection when the cost cannot
//! be paid.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    regenerate: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "skyshroud_troll",
            cast: ManaGift {
                g: 2,
                c: 2,
                ..Default::default()
            },
            regenerate: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "gorilla_chieftain",
            cast: ManaGift {
                g: 2,
                c: 2,
                ..Default::default()
            },
            regenerate: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
        Case {
            id: "tel-jilad_exile",
            cast: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            regenerate: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 3,
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
fn regenerate_green_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_330_000 + index as u64);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
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
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn regenerate_green_batch_cards_place_a_shield_with_the_exact_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_331_000 + index as u64);
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
fn regenerate_green_batch_cards_survive_lethal_damage_by_consuming_the_shield() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_332_000 + index as u64);
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
fn regenerate_green_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_333_000 + index as u64);
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
