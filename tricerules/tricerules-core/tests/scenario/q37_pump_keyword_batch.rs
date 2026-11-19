//! Actual-card scenarios for the queue-v37 pump-and-keyword spell cohort.
//!
//! Expectations are printed Oracle values. CR 115.1a/601.2c choose the target as the
//! spell is cast; CR 613.4c applies the pump; CR 611.2a/514.2 end pump and keyword at
//! cleanup; CR 608.2b fizzles the spell when its only target is illegal at resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: i32,
    toughness: i32,
    keywords: &'static [Keyword],
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "aerial_maneuver",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 1,
            keywords: &[Keyword::Flying, Keyword::FirstStrike],
        },
        Case {
            id: "angelic_blessing",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
            keywords: &[Keyword::Flying],
        },
        Case {
            id: "coat_with_venom",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 2,
            keywords: &[Keyword::Deathtouch],
        },
        Case {
            id: "fervent_strike",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
            keywords: &[Keyword::FirstStrike, Keyword::Haste],
        },
        Case {
            id: "gift_of_strength",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
            keywords: &[Keyword::Reach],
        },
    ]
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

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
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

fn cast_at(e: &mut GameEngine, case: &Case, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
}

fn check(e: &GameEngine, object: u32, case: &Case, expect: bool) {
    for keyword in case.keywords {
        assert_eq!(
            e.effective_has_keyword(object, *keyword),
            expect,
            "{}: {keyword:?}",
            case.id
        );
    }
}

fn move_to_graveyard(e: &mut GameEngine, player: usize, object: u32) {
    e.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object);
    e.state.players[player].graveyard.push(object);
    e.state.objects.get_mut(&object).expect("object").zone = Zone::Graveyard;
    *e.state.zone_change_generation.entry(object).or_default() += 1;
}

#[test]
fn q37_pump_keyword_canary_simplest() {
    let case = &cases()[3];
    let mut e = engine(37_060_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast_at(&mut e, case, target_object(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(3), Some(2)),
        "printed +1/+0"
    );
    check(&e, target, case, true);
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(2))
    );
    check(&e, target, case, false);
}

#[test]
fn q37_pump_keyword_canary_hardest() {
    let case = &cases()[1];
    let mut e = engine(37_060_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_at(&mut e, case, target_object(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(5), Some(5)),
        "Angelic Blessing pumps an opponent's creature +3/+3"
    );
    check(&e, target, case, true);
}

#[test]
fn q37_pump_keyword_cards_apply_the_printed_pump_and_keywords_only_to_the_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_061_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        cast_at(&mut e, case, target_object(target));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            (
                Some((2 + case.power) as u32),
                Some((2 + case.toughness) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        check(&e, target, case, true);
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: untargeted creature unchanged",
            case.id
        );
        for keyword in case.keywords {
            assert!(!e.effective_has_keyword(bystander, *keyword), "{}", case.id);
        }
    }
}

#[test]
fn q37_pump_keyword_cards_require_a_creature_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_062_000 + index as u64);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect_err("the spell needs a creature target");
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert!(!e.state.players[0]
            .graveyard
            .iter()
            .any(|oid| e.state.objects[oid].card_id == case.id));
    }
}

#[test]
fn q37_pump_keyword_cards_fizzle_when_the_target_is_gone() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(37_063_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_at(&mut e, case, target_object(target));
        move_to_graveyard(&mut e, 0, target);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
        assert!(
            e.state.players[0]
                .graveyard
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the spell went to the graveyard without resolving",
            case.id
        );
    }
}
