//! Actual-card scenarios for the queue-v30 protection remnants.
//!
//! Expectations are printed Oracle values. CR 702.16b-f make protection a static
//! quality: a matching-color source cannot target the permanent, and the printed
//! flying/defender keywords are independent. CR 702.29a makes cycling a hand-only
//! {2} discard-and-draw ability.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::{Color, ProtectionQuality};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, AbilitySourceZone, ActivateAbility, RuledCommand,
};

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    protection: Color,
    flying: bool,
    defender: bool,
    cycling: bool,
}

fn cases() -> [Case; 8] {
    [
        Case {
            id: "cemetery_gate",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            body: (0, 5),
            protection: Color::Black,
            flying: false,
            defender: true,
            cycling: false,
        },
        Case {
            id: "wall_of_light",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 5),
            protection: Color::Black,
            flying: false,
            defender: true,
            cycling: false,
        },
        Case {
            id: "vodalian_zombie",
            mana: ManaGift {
                u: 1,
                b: 1,
                ..Default::default()
            },
            body: (2, 2),
            protection: Color::Green,
            flying: false,
            defender: false,
            cycling: false,
        },
        Case {
            id: "zombie_outlander",
            mana: ManaGift {
                u: 1,
                b: 1,
                ..Default::default()
            },
            body: (2, 2),
            protection: Color::Green,
            flying: false,
            defender: false,
            cycling: false,
        },
        Case {
            id: "voice_of_reason",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            body: (2, 2),
            protection: Color::Blue,
            flying: true,
            defender: false,
            cycling: false,
        },
        Case {
            id: "windreaper_falcon",
            mana: ManaGift {
                r: 1,
                g: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            protection: Color::Blue,
            flying: true,
            defender: false,
            cycling: false,
        },
        Case {
            id: "darkwatch_elves",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            protection: Color::Black,
            flying: false,
            defender: false,
            cycling: true,
        },
        Case {
            id: "disciple_of_grace",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 2),
            protection: Color::Black,
            flying: false,
            defender: false,
            cycling: true,
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

fn cast(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

fn matching_removal(color: Color) -> (&'static str, ManaGift) {
    match color {
        Color::Black => (
            "murder",
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        ),
        Color::Green => (
            "giant_growth",
            ManaGift {
                g: 1,
                ..Default::default()
            },
        ),
        Color::Blue => (
            "unsummon",
            ManaGift {
                u: 1,
                ..Default::default()
            },
        ),
        _ => unreachable!(),
    }
}

fn hand_ability(e: &GameEngine, source: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: source,
            source_zone: AbilitySourceZone::Hand as i32,
            expected_zone_change_generation: e
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0),
            ability_index: 0,
            ..Default::default()
        })),
    }
}

#[test]
fn q30_protection_remnants_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(30_050_001);
    let source = cast(&mut e, case);
    assert_eq!(
        e.characteristics(source)
            .expect("characteristics")
            .protections,
        [ProtectionQuality::Color(Color::Green)]
    );
}

#[test]
fn q30_protection_remnants_canary_hardest() {
    let case = &cases()[6];
    let mut e = engine(30_050_002);
    let source = cast(&mut e, case);
    let drawn = inject_library_card(&mut e, 0, "forest");
    e.state.players[0].library.retain(|oid| *oid != drawn);
    e.state.players[0].library.push_front(drawn);
    inject_card_into_hand(&mut e, 0, case.id);
    let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &hand_ability(&e, card))
        .expect("cycle Darkwatch Elves");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
}

#[test]
fn q30_protection_remnants_cards_cast_with_exact_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_051_000 + index as u64);
        let source = cast(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}",
            case.id
        );
        assert_eq!(
            e.characteristics(source)
                .expect("characteristics")
                .protections,
            [ProtectionQuality::Color(case.protection)],
            "{}",
            case.id
        );
        let keywords = e.characteristics(source).expect("characteristics").keywords;
        assert_eq!(
            keywords.contains(&tricerules_cards::primitives::Keyword::Flying),
            case.flying,
            "{}",
            case.id
        );
        assert_eq!(
            keywords.contains(&tricerules_cards::primitives::Keyword::Defender),
            case.defender,
            "{}",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
    }
}

#[test]
fn q30_protection_remnants_cards_reject_matching_color_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_052_000 + index as u64);
        let protected = inject_creature_on_battlefield(&mut e, 1, case.id);
        let (removal, mana) = matching_removal(case.protection);
        inject_card_into_hand(&mut e, 0, removal);
        give_mana(&mut e, 0, mana);
        let slot = hand_index_for_card(&e, 0, removal);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(protected)))
                .is_err(),
            "{}: a {} source cannot target it",
            case.id,
            match case.protection {
                Color::Black => "black",
                Color::Green => "green",
                Color::Blue => "blue",
                _ => unreachable!(),
            }
        );
        assert_eq!(
            e.state.objects[&protected].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q30_protection_remnants_cards_cycle_from_hand_where_printed() {
    for (index, case) in cases()
        .into_iter()
        .enumerate()
        .filter(|(_, case)| case.cycling)
    {
        let mut e = engine(30_053_000 + index as u64);
        let source = cast(&mut e, &case);
        inject_card_into_hand(&mut e, 0, case.id);
        let card = e.state.players[0].hand[hand_index_for_card(&e, 0, case.id)];
        let drawn = inject_library_card(&mut e, 0, "forest");
        e.state.players[0].library.retain(|oid| *oid != drawn);
        e.state.players[0].library.push_front(drawn);
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        e.apply_scenario_command(0, &hand_ability(&e, card))
            .expect("cycle");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&card].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
