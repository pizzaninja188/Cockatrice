//! Actual-card scenarios for the queue-v39 fixed Equipment cohort.
//!
//! Expectations are printed Oracle values. CR 301.5/702.6 cover Equipment and equip; CR 613.4c
//! applies the attached P/T modifier; CR 613.1f grants keywords; CR 704.5n detaches an Equipment
//! whose host left; CR 702.6a restricts equip timing; and CR 608.2b fizzles a stale equip target.
use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::{AttachmentRecipient, Zone};

struct Case {
    id: &'static str,
    cast: ManaGift,
    equip: ManaGift,
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "kitesail",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
            keywords: &[Keyword::Flying],
        },
        Case {
            id: "aeronauts_wings",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
            keywords: &[Keyword::Flying],
        },
        Case {
            id: "cobbled_wings",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 0,
            keywords: &[Keyword::Flying],
        },
        Case {
            id: "neurok_hoversail",
            cast: ManaGift {
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 0,
            toughness: 0,
            keywords: &[Keyword::Flying],
        },
        Case {
            id: "rangers_longbow",
            cast: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
            keywords: &[Keyword::Reach],
        },
        Case {
            id: "spidersilk_net",
            cast: ManaGift::default(),
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 0,
            toughness: 2,
            keywords: &[Keyword::Reach],
        },
        Case {
            id: "behemoth_sledge",
            cast: ManaGift {
                g: 1,
                w: 1,
                c: 1,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
            keywords: &[Keyword::Trample, Keyword::Lifelink],
        },
        Case {
            id: "loxodon_warhammer",
            cast: ManaGift {
                c: 3,
                ..Default::default()
            },
            equip: ManaGift {
                c: 3,
                ..Default::default()
            },
            power: 3,
            toughness: 0,
            keywords: &[Keyword::Trample, Keyword::Lifelink],
        },
        Case {
            id: "viridian_claw",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 1,
                ..Default::default()
            },
            power: 1,
            toughness: 0,
            keywords: &[Keyword::FirstStrike],
        },
        Case {
            id: "bladed_pinions",
            cast: ManaGift {
                c: 2,
                ..Default::default()
            },
            equip: ManaGift {
                c: 2,
                ..Default::default()
            },
            power: 0,
            toughness: 0,
            keywords: &[Keyword::Flying, Keyword::FirstStrike],
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
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

fn cast_equipment(e: &mut GameEngine, id: &str, cast: ManaGift) -> u32 {
    give_mana(e, 0, cast);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = hand_index_for_card(e, 0, id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {id}: {error:?}"));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cast cost");
    resolve_entire_stack_two_player(e);
    assert_eq!(e.state.objects[&slot].zone, Zone::Battlefield, "{id}");
    assert_eq!(
        e.state.objects[&slot].attached_to, None,
        "{id}: enters unattached"
    );
    slot
}

fn equip(e: &mut GameEngine, equipment: u32, creature: u32, mana: ManaGift, id: &str) {
    give_mana(e, 0, mana);
    apply_ability(e, 0, equipment, 0, target_object(creature)).expect("equip");
    pass_both_players(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact equip cost");
    assert_eq!(
        e.state.objects[&equipment].attached_to,
        Some(AttachmentRecipient::Object(creature)),
        "{id}: attached to the chosen creature"
    );
}

#[test]
fn q39_equipment_static_keyword_canary_simplest() {
    let case = &cases()[5];
    let mut e = engine(39_020_001);
    let host = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
    equip(&mut e, equipment, host, gift(&case.equip), case.id);
    assert_eq!(e.effective_power(host), Some(2));
    assert_eq!(e.effective_toughness(host), Some(4));
    assert!(e.effective_has_keyword(host, Keyword::Reach));
}

#[test]
fn q39_equipment_static_keyword_canary_hardest() {
    let case = &cases()[6];
    let mut e = engine(39_020_002);
    let host = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
    equip(&mut e, equipment, host, gift(&case.equip), case.id);
    assert_eq!(e.effective_power(host), Some(4));
    assert_eq!(e.effective_toughness(host), Some(4));
    assert!(e.effective_has_keyword(host, Keyword::Trample));
    assert!(e.effective_has_keyword(host, Keyword::Lifelink));
}

#[test]
fn q39_equipment_static_keyword_cards_grant_exact_printed_bonus_and_follow_reattachment() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_021_000 + index as u64);
        let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
        let first = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        let second = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
        equip(&mut e, equipment, first, gift(&case.equip), case.id);
        assert_eq!(
            e.effective_power(first),
            Some(2 + case.power),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(first),
            Some(2 + case.toughness),
            "{}",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(first, *keyword),
                "{}: {:?}",
                case.id,
                keyword
            );
        }
        equip(&mut e, equipment, second, gift(&case.equip), case.id);
        assert_eq!(
            e.effective_power(first),
            Some(2),
            "{}: old host power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(first),
            Some(2),
            "{}: old host toughness",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                !e.effective_has_keyword(first, *keyword),
                "{}: old host {:?}",
                case.id,
                keyword
            );
            assert!(
                e.effective_has_keyword(second, *keyword),
                "{}: new host {:?}",
                case.id,
                keyword
            );
        }
        assert_eq!(
            e.effective_power(second),
            Some(2 + case.power),
            "{}: new host power",
            case.id
        );
        assert_eq!(
            e.effective_toughness(second),
            Some(2 + case.toughness),
            "{}: new host toughness",
            case.id
        );
    }
}

#[test]
fn q39_equipment_static_keyword_cards_reject_opponent_creature_targets() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_022_000 + index as u64);
        let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
        let opponent = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
        give_mana(&mut e, 0, gift(&case.equip));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, equipment, 0, target_object(opponent)).is_err(),
            "{}: opponent creature is not a legal equip target",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected equip spends no mana",
            case.id
        );
        assert_eq!(e.state.objects[&equipment].attached_to, None, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q39_equipment_static_keyword_cards_equip_only_at_sorcery_speed() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_023_000 + index as u64);
        let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
        let host = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        end_active_turn(&mut e, 0);
        give_mana(&mut e, 0, gift(&case.equip));
        assert!(
            apply_ability(&mut e, 0, equipment, 0, target_object(host)).is_err(),
            "{}: equip is illegal outside a sorcery-speed window",
            case.id
        );
        assert_eq!(e.state.objects[&equipment].attached_to, None, "{}", case.id);
    }
}

#[test]
fn q39_equipment_static_keyword_cards_fizzle_when_equip_target_leaves() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(39_024_000 + index as u64);
        let equipment = cast_equipment(&mut e, case.id, gift(&case.cast));
        let host = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        give_mana(&mut e, 0, gift(&case.equip));
        apply_ability(&mut e, 0, equipment, 0, target_object(host)).expect("activate equip");
        e.state.players[0]
            .battlefield
            .retain(|object| *object != host);
        e.state.players[0].graveyard.push(host);
        e.state.objects.get_mut(&host).expect("host").zone = Zone::Graveyard;
        *e.state.zone_change_generation.entry(host).or_default() += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&equipment].attached_to, None, "{}", case.id);
        assert_eq!(
            e.state.objects[&equipment].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
