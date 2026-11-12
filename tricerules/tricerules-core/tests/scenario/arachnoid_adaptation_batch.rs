//! Actual-card scenarios for the reviewed Arachnoid Adaptation pump-reach-untap family.
//!
//! Expectations are printed Oracle values. CR 115.1 and 608.2b govern the required creature
//! target and stale-target fizzle; CR 611.2a gives the pump/reach duration, CR 701.26 untaps the
//! chosen creature, and CR 514.2 ends the pump and reach during cleanup.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "arachnoid_adaptation",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "tethermages_advantage",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "vines_of_the_recluse",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 2,
        },
        Case {
            id: "witchs_web",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
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
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn begin_cast_at(e: &mut GameEngine, id: &str, mana: ManaGift, target: u32) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .expect("cast the pump spell");
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cost consumed");
    slot
}

fn cast_at(e: &mut GameEngine, id: &str, mana: ManaGift, target: u32) -> u32 {
    let slot = begin_cast_at(e, id, mana, target);
    assert_eq!(e.state.stack.len(), 1, "{id}: the spell is on the stack");
    assert_eq!(
        e.effective_power(target),
        Some(2),
        "{id}: no pre-resolution pump"
    );
    assert_eq!(e.effective_toughness(target), Some(2));
    assert!(!e.effective_has_keyword(target, Keyword::Reach));
    resolve_entire_stack_two_player(e);
    assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{id}");
    assert!(e.state.players[0].graveyard.contains(&slot), "{id}");
    assert!(!e.state.players[1].graveyard.contains(&slot), "{id}");
    slot
}

#[test]
fn arachnoid_adaptation_batch_cards_cast_with_exact_printed_effects() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(750_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        e.state.objects.get_mut(&target).expect("target").tapped = true;
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        let slot = cast_at(&mut e, case.id, gift(&case.mana), target);
        assert_eq!(
            e.effective_power(target),
            Some(2 + case.power),
            "{}: exact power pump",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(2 + case.toughness),
            "{}: exact toughness pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(target, Keyword::Reach),
            "{}: reach granted",
            case.id
        );
        assert!(!e.state.objects[&target].tapped, "{}: untapped", case.id);
        assert_eq!(
            e.state.objects[&target].controller, e.state.players[1].id,
            "{}: the target stays under its controller",
            case.id
        );
        assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard);
        assert_eq!(e.state.players[1].hand.len(), opponent_hand);
        assert_eq!(e.state.players[1].library.len(), opponent_library);
    }
}

#[test]
fn arachnoid_adaptation_batch_cards_expire_at_cleanup() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(751_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        e.state.objects.get_mut(&target).expect("target").tapped = true;
        cast_at(&mut e, case.id, gift(&case.mana), target);
        assert_eq!(e.effective_power(target), Some(2 + case.power));
        assert!(e.effective_has_keyword(target, Keyword::Reach));
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.effective_power(target),
            Some(2),
            "{}: power expires",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(2),
            "{}: toughness expires",
            case.id
        );
        assert!(
            !e.effective_has_keyword(target, Keyword::Reach),
            "{}: reach expires",
            case.id
        );
        assert!(
            !e.state.objects[&target].tapped,
            "{}: untap persists",
            case.id
        );
    }
}

#[test]
fn arachnoid_adaptation_batch_cards_buff_an_untapped_target() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(754_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        assert!(
            !e.state.objects[&target].tapped,
            "{}: starts untapped",
            case.id
        );
        cast_at(&mut e, case.id, gift(&case.mana), target);
        assert_eq!(
            e.effective_power(target),
            Some(2 + case.power),
            "{}: Vines ruling: an untapped target still gets the pump",
            case.id
        );
        assert_eq!(
            e.effective_toughness(target),
            Some(2 + case.toughness),
            "{}: untapped target still gets toughness",
            case.id
        );
        assert!(
            e.effective_has_keyword(target, Keyword::Reach),
            "{}: untapped target still gets reach",
            case.id
        );
        assert!(
            !e.state.objects[&target].tapped,
            "{}: stays untapped",
            case.id
        );
    }
}

#[test]
fn arachnoid_adaptation_batch_cards_reject_a_noncreature_target() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(752_000 + case_index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let before_pool = pool(&e);
        let before_command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal creature target",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand[hand_slot],
            slot,
            "{case_id}",
            case_id = case.id
        );
        assert_eq!(pool(&e), before_pool, "{}: no mana is spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(e.state.command_index, before_command_index, "{}", case.id);
    }
}

#[test]
fn arachnoid_adaptation_batch_cards_fizzle_when_the_target_left() {
    for (case_index, case) in cases().into_iter().enumerate() {
        let mut e = engine(753_000 + case_index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        e.state
            .objects
            .get_mut(&bystander)
            .expect("bystander")
            .tapped = true;
        let slot = begin_cast_at(&mut e, case.id, gift(&case.mana), target);
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: the spell fizzles into its owner's graveyard",
            case.id
        );
        assert!(e.state.players[0].graveyard.contains(&slot));
        assert!(!e.state.players[1].graveyard.contains(&slot));
        assert_eq!(e.effective_power(bystander), Some(2));
        assert_eq!(e.effective_toughness(bystander), Some(2));
        assert!(!e.effective_has_keyword(bystander, Keyword::Reach));
        assert!(
            e.state.objects[&bystander].tapped,
            "{}: bystander stays tapped",
            case.id
        );
    }
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
