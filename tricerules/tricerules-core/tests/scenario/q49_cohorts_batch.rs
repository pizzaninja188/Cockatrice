//! Actual-card scenarios for the queue-v49 reviewed cohorts (batch A).
//!
//! Expectations are printed Oracle values. CR 702.16b-f governs protection's five
//! effects, CR 702.7/702.8/702.9/702.15/702.19/702.20/702.111 the keyword grants,
//! CR 303.4a Aura attachment, CR 613.4c the layer-7c pump, CR 613.1f the keyword
//! grant and CR 704.5m unattached-Aura cleanup.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{TargetRef, TargetRefKind};

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
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

fn cast_named(
    e: &mut GameEngine,
    player: i32,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) {
    inject_card_into_hand(e, player as usize, card);
    give_mana(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    e.apply_scenario_command(player, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
}

fn cast_aura_on(e: &mut GameEngine, card: &str, mana: ManaGift, target: u32) {
    cast_named(e, 0, card, mana, target_object(target));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects.get(&target).map(|o| o.zone),
        Some(Zone::Battlefield),
        "{card}: recipient"
    );
}

fn keywords(e: &GameEngine, oid: u32) -> Vec<Keyword> {
    e.characteristics(oid).expect("characteristics").keywords
}

fn protections(e: &GameEngine, oid: u32) -> Vec<tricerules_cards::primitives::ProtectionQuality> {
    e.characteristics(oid).expect("characteristics").protections
}

fn fight_targets(first: u32, second: u32) -> Vec<TargetRef> {
    vec![
        TargetRef {
            object_id: first,
            group_index: 0,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
        TargetRef {
            object_id: second,
            group_index: 1,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
    ]
}

// ── 01 protection from white ───────────────────────────────────────────────

#[test]
fn q49_protection_white_canary_simplest_blood_knight_rejects_white_targeting() {
    let mut e = main1_engine(49_010_001, &["blood_knight"], &[]);
    let knight = move_ready_to_battlefield(&mut e, 0, "blood_knight");
    assert!(e.effective_has_keyword(knight, Keyword::FirstStrike));
    assert!(protections(&e, knight).contains(
        &tricerules_cards::primitives::ProtectionQuality::Color(
            tricerules_cards::primitives::Color::White
        )
    ));
    inject_card_into_hand(&mut e, 0, "holy_strength");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "holy_strength");
    let hand_before = e.state.players[0].hand.len();
    let mana_before = (
        e.state.players[0].mana_pool.white,
        e.state.players[0].mana_pool.green,
        e.state.players[0].mana_pool.colorless,
    );
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(knight)))
            .is_err(),
        "a white Aura cannot target a creature with protection from white"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "the Aura stays in hand"
    );
    assert_eq!(
        (
            e.state.players[0].mana_pool.white,
            e.state.players[0].mana_pool.green,
            e.state.players[0].mana_pool.colorless,
        ),
        mana_before,
        "no mana is spent on the rejected cast"
    );
    assert!(e.state.stack.is_empty(), "nothing was put on the stack");
    cast_named(
        &mut e,
        0,
        "oakenform",
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
        target_object(knight),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(knight),
        Some(5),
        "a green Aura still targets it"
    );
    assert_eq!(e.effective_toughness(knight), Some(5));
    let aura = battlefield_object_for_card(&e, 0, "oakenform");
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(tricerules_core::AttachmentRecipient::Object(knight)),
        "the green Aura attached"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q49_protection_white_canary_hardest_defender_of_chaos_flashes() {
    let mut e = main1_engine(49_010_002, &[], &["defender_of_chaos"]);
    e.apply_scenario_command(0, &pass())
        .expect("offer priority to the nonactive player");
    inject_card_into_hand(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        1,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "grizzly_bears");
    assert!(
        e.apply_scenario_command(1, &cast_spell(slot, vec![]))
            .is_err(),
        "a non-Flash creature cannot be cast during the opponent's main phase"
    );
    inject_card_into_hand(&mut e, 1, "defender_of_chaos");
    give_mana(
        &mut e,
        1,
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "defender_of_chaos");
    e.apply_scenario_command(1, &cast_spell(slot, vec![]))
        .expect("Flash allows the creature during the opponent's main phase");
    resolve_entire_stack_two_player(&mut e);
    let chaos = battlefield_object_for_card(&e, 1, "defender_of_chaos");
    assert!(e.effective_has_keyword(chaos, Keyword::Flash));
    assert!(protections(&e, chaos).contains(
        &tricerules_cards::primitives::ProtectionQuality::Color(
            tricerules_cards::primitives::Color::White
        )
    ));
}

#[test]
fn q49_protection_white_cards_expose_their_printed_keywords_and_protection() {
    use tricerules_cards::primitives::{Color, ProtectionQuality};
    struct Case {
        id: &'static str,
        mana: ManaGift,
        keywords: &'static [Keyword],
    }
    for (index, case) in [
        Case {
            id: "blood_knight",
            mana: ManaGift {
                r: 2,
                ..Default::default()
            },
            keywords: &[Keyword::FirstStrike],
        },
        Case {
            id: "defender_of_chaos",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            keywords: &[Keyword::Flash],
        },
        Case {
            id: "voice_of_truth",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            keywords: &[Keyword::Flying],
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_011_000 + index as u64, &[case.id], &[]);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let oid = battlefield_object_for_card(&e, 0, case.id);
        let value = e.characteristics(oid).expect("characteristics");
        for keyword in case.keywords {
            assert!(value.keywords.contains(keyword), "{}: {keyword:?}", case.id);
        }
        assert!(
            value
                .protections
                .contains(&ProtectionQuality::Color(Color::White)),
            "{}: protection from white",
            case.id
        );
        assert_eq!(
            e.state.objects[&oid].zone,
            Zone::Battlefield,
            "{}: resolved",
            case.id
        );
    }
}

#[test]
fn q49_protection_white_cards_reject_white_aura_targeting() {
    for (index, id) in ["blood_knight", "defender_of_chaos", "voice_of_truth"]
        .into_iter()
        .enumerate()
    {
        let mut e = main1_engine(49_014_000 + index as u64, &[id], &[]);
        let protected = move_ready_to_battlefield(&mut e, 0, id);
        inject_card_into_hand(&mut e, 0, "holy_strength");
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "holy_strength");
        let hand_before = e.state.players[0].hand.len();
        let mana_before = e.state.players[0].mana_pool.white;
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(protected)))
                .is_err(),
            "{id}: a white Aura cannot target it"
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{id}: the Aura stays in hand"
        );
        assert_eq!(
            e.state.players[0].mana_pool.white, mana_before,
            "{id}: no mana is spent"
        );
        assert!(
            e.state.stack.is_empty(),
            "{id}: nothing was put on the stack"
        );
    }
}

#[test]
fn q49_protection_white_cards_prevent_white_source_damage() {
    for (index, id) in ["blood_knight", "defender_of_chaos", "voice_of_truth"]
        .into_iter()
        .enumerate()
    {
        let mut e = main1_engine(49_012_000 + index as u64, &[id], &["savannah_lions"]);
        let protected = move_ready_to_battlefield(&mut e, 0, id);
        let lions = move_ready_to_battlefield(&mut e, 1, "savannah_lions");
        cast_named(
            &mut e,
            0,
            "prey_upon",
            ManaGift {
                g: 1,
                ..Default::default()
            },
            fight_targets(protected, lions),
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&protected].zone,
            Zone::Battlefield,
            "{id}: damage from a white source is prevented"
        );
        assert_eq!(
            e.state.objects[&protected].damage, 0,
            "{id}: no damage marked"
        );
        assert_eq!(
            e.state.objects[&lions].zone,
            Zone::Graveyard,
            "{id}: the protected creature still deals its damage"
        );
    }
}

#[test]
fn q49_protection_white_cards_cannot_be_blocked_by_white() {
    for (index, id) in ["blood_knight", "defender_of_chaos", "voice_of_truth"]
        .into_iter()
        .enumerate()
    {
        let mut e = main1_engine(49_013_000 + index as u64, &[id], &["savannah_lions"]);
        let protected = move_ready_to_battlefield(&mut e, 0, id);
        let blocker_card = if id == "voice_of_truth" {
            "serra_angel"
        } else {
            "savannah_lions"
        };
        let blocker = inject_creature_on_battlefield(&mut e, 1, blocker_card);
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![protected]))
            .expect("declare the protected attacker");
        pass_both_players(&mut e);
        let legal = &e.initial_response_batch().legal_by_player[&1];
        if id == "voice_of_truth" {
            assert!(
                e.effective_has_keyword(blocker, Keyword::Flying),
                "{id}: the white blocker has flying"
            );
        }
        assert!(
            !legal
                .legal_block_pairs
                .iter()
                .any(|pair| pair.attacker_id == protected && pair.blocker_id == blocker),
            "{id}: a white source cannot block it"
        );
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: protected,
                    blocker_id: blocker,
                }]),
            )
            .is_err(),
            "{id}: the engine rejects the protected block"
        );
    }
}

// ── 02 Aura keyword grants ─────────────────────────────────────────────────

#[test]
fn q49_aura_spell_shape_rejects_noncreature_targets_and_fizzles_on_stale_target() {
    let cases = [
        (
            "imposing_visage",
            ManaGift {
                r: 1,
                ..Default::default()
            },
        ),
        (
            "lance",
            ManaGift {
                w: 1,
                ..Default::default()
            },
        ),
        (
            "lifelink",
            ManaGift {
                w: 1,
                ..Default::default()
            },
        ),
        (
            "primal_frenzy",
            ManaGift {
                g: 1,
                ..Default::default()
            },
        ),
        (
            "vigilance",
            ManaGift {
                w: 1,
                ..Default::default()
            },
        ),
        (
            "aspect_of_gorgon",
            ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            "lightning_talons",
            ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        ),
        (
            "mantle_of_webs",
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        ),
        (
            "marked_by_honor",
            ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
        ),
        (
            "web",
            ManaGift {
                g: 1,
                ..Default::default()
            },
        ),
    ];
    for (index, (id, mana)) in cases.into_iter().enumerate() {
        let mut e = main1_engine(49_022_000 + index as u64 * 2, &[id], &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        let _ = inject_card_into_hand(&mut e, 0, id);
        give_mana(&mut e, 0, gift(&mana));
        let slot = hand_index_for_card(&e, 0, id);
        let aura = e.state.players[0].hand[slot];
        let hand_before = e.state.players[0].hand.len();
        let mana_before = (
            e.state.players[0].mana_pool.white,
            e.state.players[0].mana_pool.blue,
            e.state.players[0].mana_pool.black,
            e.state.players[0].mana_pool.red,
            e.state.players[0].mana_pool.green,
            e.state.players[0].mana_pool.colorless,
        );
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{id}: a noncreature is not an Enchant creature target"
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{id}: hand");
        assert_eq!(
            (
                e.state.players[0].mana_pool.white,
                e.state.players[0].mana_pool.blue,
                e.state.players[0].mana_pool.black,
                e.state.players[0].mana_pool.red,
                e.state.players[0].mana_pool.green,
                e.state.players[0].mana_pool.colorless,
            ),
            mana_before,
            "{id}: mana"
        );
        assert!(e.state.stack.is_empty(), "{id}: stack");
        assert_eq!(e.state.objects[&aura].zone, Zone::Hand, "{id}: aura hand");

        let mut e = main1_engine(49_022_001 + index as u64 * 2, &[id], &[]);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        give_mana(&mut e, 0, gift(&mana));
        let _ = inject_card_into_hand(&mut e, 0, id);
        let slot = hand_index_for_card(&e, 0, id);
        let aura = e.state.players[0].hand[slot];
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .unwrap_or_else(|error| panic!("{id}: cast the Aura: {error:?}"));
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Stack,
            "{id}: the Aura is on the stack"
        );
        cast_named(
            &mut e,
            0,
            "go_for_the_throat",
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            target_object(creature),
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "{id}: target removed"
        );
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Graveyard,
            "{id}: the Aura fizzles when its target is gone"
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

#[test]
fn q49_aura_keyword_canary_simplest_lance_grants_first_strike() {
    let mut e = main1_engine(49_020_001, &["lance"], &[]);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bystander = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    cast_aura_on(
        &mut e,
        "lance",
        ManaGift {
            w: 1,
            ..Default::default()
        },
        bear,
    );
    assert!(keywords(&e, bear).contains(&Keyword::FirstStrike));
    assert!(!keywords(&e, bystander).contains(&Keyword::FirstStrike));
}

#[test]
fn q49_aura_keyword_canary_hardest_lifelink_gains_life_from_damage_dealt() {
    let mut e = main1_engine(49_020_002, &["lifelink"], &["savannah_lions"]);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let lions = move_ready_to_battlefield(&mut e, 1, "savannah_lions");
    cast_aura_on(
        &mut e,
        "lifelink",
        ManaGift {
            w: 1,
            ..Default::default()
        },
        bear,
    );
    assert!(keywords(&e, bear).contains(&Keyword::Lifelink));
    cast_named(
        &mut e,
        0,
        "prey_upon",
        ManaGift {
            g: 1,
            ..Default::default()
        },
        fight_targets(bear, lions),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].life, 22,
        "lifelink gains the damage dealt"
    );
}

#[test]
fn q49_aura_keyword_lifelink_credits_the_enchanted_creatures_controller() {
    let mut e = main1_engine(
        49_020_003,
        &["lifelink", "savannah_lions"],
        &["grizzly_bears"],
    );
    let their_bear = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
    let own_lions = move_ready_to_battlefield(&mut e, 0, "savannah_lions");
    cast_aura_on(
        &mut e,
        "lifelink",
        ManaGift {
            w: 1,
            ..Default::default()
        },
        their_bear,
    );
    assert!(keywords(&e, their_bear).contains(&Keyword::Lifelink));
    e.apply_scenario_command(0, &pass())
        .expect("pass priority to the opponent");
    cast_named(
        &mut e,
        1,
        "swift_kick",
        ManaGift {
            r: 1,
            c: 3,
            ..Default::default()
        },
        fight_targets(their_bear, own_lions),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 23,
        "the enchanted creature's controller gains the life"
    );
    assert_eq!(
        e.state.players[0].life, 20,
        "the Aura's controller does not gain the life"
    );
}

#[test]
fn q49_aura_keyword_cards_grant_their_printed_keyword_and_stop_on_leaving() {
    struct Case {
        id: &'static str,
        mana: ManaGift,
        keyword: Keyword,
    }
    for (index, case) in [
        Case {
            id: "imposing_visage",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            keyword: Keyword::Menace,
        },
        Case {
            id: "lance",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "lifelink",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            keyword: Keyword::Lifelink,
        },
        Case {
            id: "primal_frenzy",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            keyword: Keyword::Trample,
        },
        Case {
            id: "vigilance",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            keyword: Keyword::Vigilance,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_021_000 + index as u64, &[case.id], &[]);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        cast_aura_on(&mut e, case.id, case.mana, bear);
        assert!(keywords(&e, bear).contains(&case.keyword), "{}", case.id);
        assert!(
            !keywords(&e, bystander).contains(&case.keyword),
            "{}: only the enchanted creature",
            case.id
        );
        let aura = battlefield_object_for_card(&e, 0, case.id);
        cast_named(
            &mut e,
            0,
            "disenchant",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            target_object(aura),
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard, "{}", case.id);
        assert!(
            !keywords(&e, bear).contains(&case.keyword),
            "{}: the grant ends with the Aura",
            case.id
        );
    }
}

// ── 03 Aura pump plus keyword ──────────────────────────────────────────────

#[test]
fn q49_aura_pump_keyword_canary_simplest_web_adds_toughness_and_reach() {
    let mut e = main1_engine(49_030_001, &["web"], &[]);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_aura_on(
        &mut e,
        "web",
        ManaGift {
            g: 1,
            ..Default::default()
        },
        bear,
    );
    assert_eq!(e.effective_power(bear), Some(2));
    assert_eq!(e.effective_toughness(bear), Some(4));
    assert!(keywords(&e, bear).contains(&Keyword::Reach));
}

#[test]
fn q49_aura_pump_keyword_canary_hardest_aspect_of_gorgon_deathtouch_kills() {
    let mut e = main1_engine(49_030_002, &["aspect_of_gorgon"], &["colossal_dreadmaw"]);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let dreadmaw = move_ready_to_battlefield(&mut e, 1, "colossal_dreadmaw");
    cast_aura_on(
        &mut e,
        "aspect_of_gorgon",
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
        bear,
    );
    assert_eq!(e.effective_power(bear), Some(3));
    assert_eq!(e.effective_toughness(bear), Some(5));
    assert!(keywords(&e, bear).contains(&Keyword::Deathtouch));
    cast_named(
        &mut e,
        0,
        "prey_upon",
        ManaGift {
            g: 1,
            ..Default::default()
        },
        fight_targets(bear, dreadmaw),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&dreadmaw].zone,
        Zone::Graveyard,
        "deathtouch destroys the larger creature"
    );
}

#[test]
fn q49_aura_pump_keyword_cards_apply_their_printed_pump_and_keyword() {
    struct Case {
        id: &'static str,
        mana: ManaGift,
        pump: (i32, i32),
        keyword: Keyword,
    }
    for (index, case) in [
        Case {
            id: "aspect_of_gorgon",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            pump: (1, 3),
            keyword: Keyword::Deathtouch,
        },
        Case {
            id: "lightning_talons",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            pump: (3, 0),
            keyword: Keyword::FirstStrike,
        },
        Case {
            id: "mantle_of_webs",
            mana: ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
            pump: (1, 3),
            keyword: Keyword::Reach,
        },
        Case {
            id: "marked_by_honor",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            pump: (2, 2),
            keyword: Keyword::Vigilance,
        },
        Case {
            id: "web",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            pump: (0, 2),
            keyword: Keyword::Reach,
        },
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = main1_engine(49_031_000 + index as u64, &[case.id], &[]);
        let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 1);
        cast_aura_on(&mut e, case.id, gift(&case.mana), bear);
        assert_eq!(
            e.effective_power(bear),
            Some((2 + case.pump.0) as u32),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_toughness(bear),
            Some((2 + case.pump.1) as u32),
            "{}",
            case.id
        );
        assert!(keywords(&e, bear).contains(&case.keyword), "{}", case.id);
        assert_eq!(e.effective_power(bystander), Some(2), "{}", case.id);
        assert_eq!(e.effective_toughness(bystander), Some(1), "{}", case.id);
        assert!(
            !keywords(&e, bystander).contains(&case.keyword),
            "{}: only the enchanted creature",
            case.id
        );
        let aura = battlefield_object_for_card(&e, 0, case.id);
        cast_named(
            &mut e,
            0,
            "disenchant",
            ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            target_object(aura),
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(bear), Some(2), "{}: pump ends", case.id);
        assert_eq!(
            e.effective_toughness(bear),
            Some(2),
            "{}: pump ends",
            case.id
        );
        assert!(
            !keywords(&e, bear).contains(&case.keyword),
            "{}: grant ends",
            case.id
        );
    }
}
