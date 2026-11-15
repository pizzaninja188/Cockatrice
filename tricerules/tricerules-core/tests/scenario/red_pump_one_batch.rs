//! Actual-card scenarios for the reviewed `{1}{R}` self-pump cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 define the activated
//! ability and exact cost; CR 611.2a/613.4c apply the pump until end of turn and
//! CR 514.2 ends it during cleanup.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    activation: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "igneous_cur",
            cast: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 2,
        },
        Case {
            id: "weaselback_redcap",
            cast: ManaGift {
                r: 1,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
        },
        Case {
            id: "hematite_golem",
            cast: ManaGift {
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 4,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card, "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
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
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    semantic::accepted(e, 0, &cast_spell(hand_slot, vec![]));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cast cost");
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{id}: resolves onto the battlefield"
    );
    slot
}

fn activate_pump(e: &mut GameEngine, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, creature, 0, vec![]).expect("activate the self-pump");
    assert_eq!(e.state.stack.len(), 1, "{id}: one ability is on the stack");
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact activation cost");
}

#[test]
fn red_pump_one_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_340_000 + index as u64, case.id);
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
            e.state.objects[&creature].controller, e.state.players[0].id,
            "{}: controller",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn red_pump_one_batch_cards_apply_the_printed_pump_only_to_themselves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_341_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        let bystander = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power + 2),
            "{}: +2 power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{}: +0 toughness",
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
            "{}: the bystander is unchanged",
            case.id
        );
        assert!(
            !e.state.objects[&creature].tapped,
            "{}: the pump does not tap",
            case.id
        );
    }
}

#[test]
fn red_pump_one_batch_cards_allow_repeated_activations_in_one_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_342_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power + 4),
            "{}: two activations accumulate",
            case.id
        );
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{}: toughness stays printed",
            case.id
        );
    }
}

#[test]
fn red_pump_one_batch_cards_pump_expires_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_343_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        activate_pump(&mut e, creature, gift(&case.activation), case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power + 2),
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
fn red_pump_one_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_344_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: the pump needs its exact mana",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert_eq!(
            e.effective_power(creature),
            Some(case.power),
            "{}: the rejected activation does not pump",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
