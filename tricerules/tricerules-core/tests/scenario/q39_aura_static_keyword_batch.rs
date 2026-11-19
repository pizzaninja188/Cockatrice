//! Actual-card scenarios for the queue-v39 Aura static keyword cohort.
//!
//! Expectations are printed Oracle values. CR 303.4a attaches the Aura to its creature target;
//! CR 613.4c applies the static P/T modifier; CR 613.1f grants the printed keywords; CR 704.5m
//! sends an unattached Aura to the graveyard; CR 107.4e permits either hybrid half.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{AttachmentRecipient, Zone};

struct Case {
    id: &'static str,
    mana: ManaGift,
    delta: (i32, i32),
    keywords: &'static [Keyword],
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "wings_of_aesthir",
            mana: ManaGift {
                w: 1,
                u: 1,
                ..Default::default()
            },
            delta: (1, 0),
            keywords: &[Keyword::Flying, Keyword::FirstStrike],
        },
        Case {
            id: "prodigious_growth",
            mana: ManaGift {
                g: 2,
                c: 4,
                ..Default::default()
            },
            delta: (7, 7),
            keywords: &[Keyword::Trample],
        },
        Case {
            id: "candlelight_vigil",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            delta: (3, 2),
            keywords: &[Keyword::Vigilance],
        },
        Case {
            id: "mark_of_the_vampire",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            delta: (2, 2),
            keywords: &[Keyword::Lifelink],
        },
        Case {
            id: "gift_of_orzhova",
            mana: ManaGift {
                w: 2,
                c: 1,
                ..Default::default()
            },
            delta: (1, 1),
            keywords: &[Keyword::Flying, Keyword::Lifelink],
        },
        Case {
            id: "mythic_proportions",
            mana: ManaGift {
                g: 3,
                c: 4,
                ..Default::default()
            },
            delta: (8, 8),
            keywords: &[Keyword::Trample],
        },
    ]
}

fn gift(case: &Case) -> ManaGift {
    ManaGift {
        w: case.mana.w,
        u: case.mana.u,
        b: case.mana.b,
        r: case.mana.r,
        g: case.mana.g,
        c: case.mana.c,
    }
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
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

fn begin_cast_aura(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    let object_id = inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(case));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    assert_eq!(e.state.stack.len(), 1, "{}: one Aura on the stack", case.id);
    object_id
}

fn cast_aura(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    let object_id = begin_cast_aura(e, case, target);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&object_id].zone,
        Zone::Battlefield,
        "{}: the Aura resolves onto the battlefield",
        case.id
    );
    assert_eq!(
        e.state.objects[&object_id].attached_to,
        Some(AttachmentRecipient::Object(target)),
        "{}: the Aura is attached to its target",
        case.id
    );
    object_id
}

fn expected(values: (u32, u32), delta: (i32, i32)) -> (u32, u32) {
    (
        (values.0 as i32 + delta.0).max(0) as u32,
        (values.1 as i32 + delta.1).max(0) as u32,
    )
}

#[test]
fn q39_aura_static_keyword_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(39_010_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let aura = cast_aura(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(5), Some(4))
    );
    assert!(e.effective_has_keyword(target, Keyword::Vigilance));
    assert_eq!(e.state.objects[&aura].controller, e.state.players[0].id);
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn q39_aura_static_keyword_canary_hardest() {
    let case = &cases()[5];
    let mut e = engine(39_010_002);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 1, 1);
    let aura = cast_aura(&mut e, case, target);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(9), Some(9))
    );
    assert!(e.effective_has_keyword(target, Keyword::Trample));
    assert_eq!(e.state.objects[&aura].controller, e.state.players[0].id);
    assert_eq!(
        e.state.objects[&target].controller, e.state.players[1].id,
        "an Aura may enchant an opponent's creature"
    );
}

#[test]
fn q39_aura_static_keyword_cards_apply_exact_printed_bonus_and_keywords() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_011_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        cast_aura(&mut e, case, target);
        assert_eq!(
            (e.effective_power(target), e.effective_toughness(target)),
            {
                let (p, t) = expected((2, 2), case.delta);
                (Some(p), Some(t))
            },
            "{}: exact printed P/T bonus",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(target, *keyword),
                "{}: missing printed keyword {:?}",
                case.id,
                keyword
            );
            assert!(
                !e.effective_has_keyword(bystander, *keyword),
                "{}: only the enchanted creature gains {:?}",
                case.id,
                keyword
            );
        }
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: bystander is unaffected",
            case.id
        );
    }
}

#[test]
fn q39_aura_static_keyword_cards_reject_a_noncreature_target() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_012_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        let object_id = inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(case));
        let slot = hand_index_for_card(&e, 0, case.id);
        let before_pool = pool(&e);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{}: a land is not an Aura target",
            case.id
        );
        assert_eq!(pool(&e), before_pool, "{}: no mana is spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(
            e.state.players[0].hand.contains(&object_id),
            "{}: the rejected Aura stays in hand",
            case.id
        );
    }
}

#[test]
fn q39_aura_static_keyword_cards_fizzle_when_the_target_left() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_013_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let slot = begin_cast_aura(&mut e, case, target);
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
            "{}: the Aura spell fizzles",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&slot),
            "{}: the fizzled Aura is in its owner's graveyard",
            case.id
        );
        assert!(
            !e.state.players[1].graveyard.contains(&slot),
            "{}: the fizzled Aura is not in the opponent's graveyard",
            case.id
        );
    }
}

#[test]
fn q39_aura_static_keyword_unattached_auras_are_put_into_their_owners_graveyard() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_014_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        let aura = cast_aura(&mut e, case, target);
        e.state.players[1]
            .battlefield
            .retain(|object| *object != target);
        e.state.players[1].graveyard.push(target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(target).or_default() += 1;
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Graveyard,
            "{}: the unattached Aura is put into the graveyard",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&aura),
            "{}: the Aura is in its owner's graveyard",
            case.id
        );
    }
}
