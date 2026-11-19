//! Actual-card scenarios for the queue-v42 pump-and-keyword instant cohort.
//!
//! Expectations are printed Oracle values. CR 115.1/601.2c require one creature target;
//! CR 608.2b makes a stale sole target fizzle; CR 611.2a/613.4c apply the until-end-of-turn
//! pump, CR 613.1f the layer-6 keyword grant and CR 701.26 the printed untap.
use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
    keyword: Keyword,
    untap: bool,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "slaughter_cry",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            pump: (3, 0),
            keyword: Keyword::FirstStrike,
            untap: false,
        },
        Case {
            id: "thunder_strike",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            pump: (2, 0),
            keyword: Keyword::FirstStrike,
            untap: false,
        },
        Case {
            id: "unlikely_aid",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            pump: (2, 0),
            keyword: Keyword::Indestructible,
            untap: false,
        },
        Case {
            id: "mortals_resolve",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            pump: (1, 1),
            keyword: Keyword::Indestructible,
            untap: false,
        },
        Case {
            id: "wings_of_the_cosmos",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            pump: (1, 3),
            keyword: Keyword::Flying,
            untap: true,
        },
        Case {
            id: "escape_from_orthanc",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            pump: (1, 3),
            keyword: Keyword::Flying,
            untap: true,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[]),
        deck_with("forest", &["grizzly_bears"]),
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

fn begin_cast_at(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    slot
}

#[test]
fn q42_pump_keyword_canary_simplest_pumps_and_grants_first_strike() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "thunder_strike")
        .expect("Thunder Strike case");
    let mut e = engine(42_090_001);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    begin_cast_at(&mut e, &case, target);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(4), Some(2)),
        "the printed +2/+0 applies"
    );
    assert!(e.effective_has_keyword(target, Keyword::FirstStrike));
}

#[test]
fn q42_pump_keyword_canary_hardest_untaps_the_target() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "escape_from_orthanc")
        .expect("Escape from Orthanc case");
    let mut e = engine(42_090_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    e.state.objects.get_mut(&target).expect("target").tapped = true;
    begin_cast_at(&mut e, &case, target);
    resolve_entire_stack_two_player(&mut e);
    assert!(!e.state.objects[&target].tapped, "the target untaps");
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(5))
    );
    assert!(e.effective_has_keyword(target, Keyword::Flying));
}

#[test]
fn q42_pump_keyword_cards_apply_their_printed_values() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_091_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        e.state.objects.get_mut(&target).expect("target").tapped = true;
        begin_cast_at(&mut e, &case, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.pump.0) as u32),
                Some((2 + case.pump.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert!(
            e.effective_has_keyword(target, case.keyword),
            "{}: printed keyword",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: single target only",
            case.id
        );
        assert!(
            !e.effective_has_keyword(bystander, case.keyword),
            "{}: keyword is single-target",
            case.id
        );
        assert_eq!(
            e.state.objects[&target].tapped, !case.untap,
            "{}: only the flying pair untaps its target",
            case.id
        );
    }
}

#[test]
fn q42_pump_keyword_cards_fizzle_when_the_target_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_092_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let slot = begin_cast_at(&mut e, &case, target);
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&slot].zone,
            Zone::Graveyard,
            "{}: the spell fizzles into its owner's graveyard",
            case.id
        );
        assert!(e.state.players[0].graveyard.contains(&slot), "{}", case.id);
    }
}

#[test]
fn q42_pump_keyword_cards_reject_noncreature_targets_and_missing_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_093_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let noncreature = inject_permanent_on_battlefield(&mut e, 1, "island");
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = e.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == slot)
            .expect("the injected card is in hand");
        let before = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
                .is_err(),
            "{}: the printed mana cost is required",
            case.id
        );
        assert_eq!(e.state.command_index, before, "{}", case.id);
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(e.effective_power(target), Some(2), "{}", case.id);

        // Exact mana, but a noncreature permanent is not a legal target.
        give_mana(&mut e, 0, gift(&case.mana));
        let pool_before = pool(&e);
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(noncreature)))
                .is_err(),
            "{}: a noncreature permanent is not a legal target",
            case.id
        );
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(
            pool(&e),
            pool_before,
            "{}: a rejected cast spends no mana",
            case.id
        );

        // A player is not a legal target either.
        assert!(
            e.apply_scenario_command(0, &cast_spell(hand_slot, target_player(1)))
                .is_err(),
            "{}: a player is not a legal target",
            case.id
        );
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(
            pool(&e),
            pool_before,
            "{}: a rejected cast spends no mana",
            case.id
        );
        assert_eq!(e.effective_power(target), Some(2), "{}", case.id);
    }
}
