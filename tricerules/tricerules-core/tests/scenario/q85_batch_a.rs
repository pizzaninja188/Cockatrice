//! Actual-card scenarios for the queue-v85 batch A cohorts.
//!
//! Three stable typed shapes, each one printed keyword plus one activated ability:
//! 01 prevention shields (`{T}: Prevent the next N damage ... any target`),
//! 02 Flowstone self-pump (`{R}: This creature gets +1/-1 until end of turn`), and
//! 03 deathtouch grants (`{B}: This creature gains deathtouch until end of turn`).
//!
//! Expectations are printed Oracle values. CR 615.1a/615.7 (prevention shield), 115.4 (any target),
//! 602.2 and 302.6 (tap ability, summoning sickness), 514.2 (until end of turn), 613.4c
//! (P/T modification), 704.5f (zero toughness), 702.20 (vigilance), 702.9 (flying),
//! 702.111 (menace), 702.19 (trample), 702.3 (defender), 702.2 (deathtouch), and
//! 702.73 with 205.3m (changeling is every creature type) govern.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

// ── 01 prevention shield clerics ──────────────────────────────────────────

struct ShieldCase {
    id: &'static str,
    body: (u32, u32),
    amount: u32,
    keywords: &'static [Keyword],
    cost: ManaGift,
}

fn shield_cases() -> [ShieldCase; 4] {
    [
        ShieldCase {
            id: "militant_monk",
            body: (2, 1),
            amount: 1,
            keywords: &[Keyword::Vigilance],
            cost: ManaGift {
                w: 2,
                c: 1,
                ..Default::default()
            },
        },
        ShieldCase {
            id: "benevolent_ancestor",
            body: (0, 4),
            amount: 1,
            keywords: &[Keyword::Defender],
            cost: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        ShieldCase {
            id: "alabaster_wall",
            body: (0, 4),
            amount: 1,
            keywords: &[Keyword::Defender],
            cost: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        ShieldCase {
            id: "aven_redeemer",
            body: (2, 2),
            amount: 2,
            keywords: &[Keyword::Flying],
            cost: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
        },
    ]
}

fn shield_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &["lightning_bolt"]),
        deck_with("mountain", &["grizzly_bears"]),
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
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn shield_case(id: &str) -> ShieldCase {
    shield_cases()
        .into_iter()
        .find(|case| case.id == id)
        .expect("known shield case")
}

fn shield_gift(m: &ManaGift) -> ManaGift {
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
fn q85_shield_canary_militant_monk_shields_a_player_and_keeps_vigilance() {
    let case = shield_case("militant_monk");
    let mut e = shield_engine(20_850_000);
    let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
    assert_eq!(
        e.state.objects[&source].card_id, case.id,
        "canary: the simplest card is injected as itself"
    );
    assert!(
        e.effective_has_keyword(source, Keyword::Vigilance),
        "printed Vigilance is live"
    );
    let player = e.state.players[1].id as u32;
    apply_ability(&mut e, 0, source, 0, target_player(1)).expect("activate on the opponent");
    assert!(e.state.objects[&source].tapped, "the tap cost is paid");
    assert_eq!(
        e.state.remaining_damage_prevention(player),
        0,
        "no shield before the ability resolves"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.remaining_damage_prevention(player),
        1,
        "exactly one point of prevention"
    );

    let life = e.state.players[1].life;
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast lightning bolt");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        life - 2,
        "one of the three damage is prevented"
    );
    assert_eq!(
        e.state.remaining_damage_prevention(player),
        0,
        "the one-point shield is consumed"
    );
}

#[test]
fn q85_shield_cards_prevent_their_exact_printed_amount_on_a_creature() {
    for (index, case) in shield_cases().iter().enumerate() {
        let mut e = shield_engine(20_851_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        apply_ability(&mut e, 0, source, 0, target_object(bear)).expect("activate on the bear");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.remaining_damage_prevention(bear),
            case.amount,
            "{}: printed prevention amount",
            case.id
        );
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, "shock");
        let slot = hand_index_for_card(&e, 0, "shock");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
            .expect("cast shock");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&bear].damage,
            2u32.saturating_sub(case.amount),
            "{}: shield absorbs {} of the two damage",
            case.id,
            case.amount
        );
        assert_eq!(
            e.state.remaining_damage_prevention(bear),
            case.amount.saturating_sub(2),
            "{}: residual shield",
            case.id
        );
    }
}

#[test]
fn q85_shield_cards_require_a_target_and_reject_a_second_activation() {
    for (index, case) in shield_cases().iter().enumerate() {
        let mut e = shield_engine(20_852_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the shield needs an any-target recipient",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: a rejected activation pays nothing",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);

        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("first activation");
        resolve_entire_stack_two_player(&mut e);
        let player = e.state.players[1].id as u32;
        assert_eq!(
            e.state.remaining_damage_prevention(player),
            case.amount,
            "{}: the first activation grants the exact printed amount",
            case.id
        );
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_err(),
            "{}: an already-tapped creature cannot pay {{T}} again",
            case.id
        );
        assert_eq!(
            e.state.remaining_damage_prevention(player),
            case.amount,
            "{}: the rejected activation adds no shield",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn q85_shield_cards_expire_at_cleanup() {
    for (index, case) in shield_cases().iter().enumerate() {
        let mut e = shield_engine(20_853_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        apply_ability(&mut e, 0, source, 0, target_player(1)).expect("activate");
        resolve_entire_stack_two_player(&mut e);
        let player = e.state.players[1].id as u32;
        assert_eq!(e.state.remaining_damage_prevention(player), case.amount);
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.state.remaining_damage_prevention(player),
            0,
            "{}: CR 514.2 ends the shield",
            case.id
        );
    }
}

#[test]
fn q85_shield_cards_apply_summoning_sickness_to_the_tap_cost() {
    for (index, case) in shield_cases().iter().enumerate() {
        let mut e = shield_engine(20_854_000 + index as u64);
        give_mana(&mut e, 0, shield_gift(&case.cost));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert!(e.state.objects[&source].summoning_sick, "{}", case.id);
        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(1)).is_err(),
            "{}: a summoning-sick creature cannot pay {{T}}",
            case.id
        );
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(
            e.state
                .remaining_damage_prevention(e.state.players[1].id as u32),
            0,
            "{}",
            case.id
        );
    }
}

#[test]
fn q85_shield_cards_cast_with_their_exact_printed_characteristics() {
    for (index, case) in shield_cases().iter().enumerate() {
        let mut e = shield_engine(20_855_000 + index as u64);
        give_mana(&mut e, 0, shield_gift(&case.cost));
        inject_card_into_hand(&mut e, 0, case.id);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed cost",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        let source = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(source, *keyword),
                "{}: printed {keyword:?}",
                case.id
            );
        }
    }
}

#[test]
fn q85_shield_cards_fizzle_when_the_any_target_recipient_leaves_before_resolution() {
    for (index, case) in shield_cases().iter().enumerate() {
        let mut e = shield_engine(20_856_000 + index as u64);
        let source = inject_creature_with_stats(&mut e, 0, case.id, case.body.0, case.body.1);
        let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        apply_ability(&mut e, 0, source, 0, target_object(bear)).expect("activate on the bear");
        // Kill the targeted creature while the shield ability is still on the stack: the
        // prevention ability resolves with a stale target and places no shield (CR 608.2b).
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        inject_card_into_hand(&mut e, 0, "lightning_bolt");
        let slot = hand_index_for_card(&e, 0, "lightning_bolt");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(bear)))
            .expect("cast lightning bolt at the same creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&bear].zone,
            Zone::Graveyard,
            "{}: the bolt resolves first and the recipient leaves",
            case.id
        );
        assert_eq!(
            e.state.remaining_damage_prevention(bear),
            0,
            "{}: a stale target means no shield is placed",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

// ── 02 Flowstone self-pump ────────────────────────────────────────────────

struct PumpCase {
    id: &'static str,
    activation: ManaGift,
    body: (u32, u32),
    keywords: &'static [Keyword],
}

fn pump_cases() -> [PumpCase; 3] {
    [
        PumpCase {
            id: "flowstone_kavu",
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (2, 3),
            keywords: &[Keyword::Menace],
        },
        PumpCase {
            id: "flowstone_wall",
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (0, 6),
            keywords: &[Keyword::Defender],
        },
        PumpCase {
            id: "flowstone_mauler",
            activation: ManaGift {
                r: 1,
                ..Default::default()
            },
            body: (4, 5),
            keywords: &[Keyword::Trample],
        },
    ]
}

fn pump_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("island", &[]),
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

fn pump_gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn activate_pump(e: &mut GameEngine, source: u32, case: &PumpCase) {
    give_mana(e, 0, pump_gift(&case.activation));
    e.apply_scenario_command(0, &activate_ability_for(e, source, 0, vec![]))
        .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    resolve_entire_stack_two_player(e);
    assert!(e.state.stack.is_empty(), "{}", case.id);
    assert!(!e.state.objects[&source].tapped, "{}: no tap cost", case.id);
}

#[test]
fn q85_pump_canary_flowstone_kavu_applies_the_printed_plus_one_minus_one() {
    let case = &pump_cases()[0];
    let mut e = pump_engine(20_860_000, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    activate_pump(&mut e, source, case);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(3), Some(2)),
        "2/3 becomes 3/2"
    );
    assert!(
        e.effective_has_keyword(source, Keyword::Menace),
        "printed Menace survives the pump"
    );
}

#[test]
fn q85_pump_cards_apply_the_exact_printed_pump_to_the_source_only() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = pump_engine(20_861_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        activate_pump(&mut e, source, case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0 + 1), Some(case.body.1 - 1)),
            "{}: printed +1/-1",
            case.id
        );
        assert_eq!(
            (e.effective_power(opponent), e.effective_toughness(opponent)),
            (Some(2), Some(2)),
            "{}: only the source is affected",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(source, *keyword),
                "{}: printed {keyword:?} is unaffected",
                case.id
            );
        }
    }
}

#[test]
fn q85_pump_cards_stack_two_activations_and_die_at_zero_toughness() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = pump_engine(20_862_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);

        // The printed toughness is the exact number of +1/-1 activations that reach zero. Assert
        // the surviving intermediate toughness after every activation, then the lethal one.
        for activation in 1..case.body.1 {
            activate_pump(&mut e, source, case);
            assert_eq!(
                e.effective_toughness(source),
                Some(case.body.1 - activation),
                "{}: activation {activation} leaves positive toughness",
                case.id
            );
            assert_eq!(
                e.state.objects[&source].zone,
                Zone::Battlefield,
                "{}: CR 704.5f does not apply while toughness is positive (activation {activation})",
                case.id
            );
        }
        assert_eq!(
            e.effective_toughness(source),
            Some(1),
            "{}: the last surviving activation leaves exactly one toughness",
            case.id
        );
        activate_pump(&mut e, source, case);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: CR 704.5f sends a zero-toughness creature to the graveyard",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&source),
            "{}",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: a dead source cannot be activated again",
            case.id
        );
    }
}

#[test]
fn q85_pump_cards_expire_at_cleanup() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = pump_engine(20_863_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate_pump(&mut e, source, case);
        assert_eq!(
            e.effective_toughness(source),
            Some(case.body.1 - 1),
            "{}: modified before cleanup",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}

#[test]
fn q85_pump_cards_reject_activation_without_the_printed_mana() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = pump_engine(20_864_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: the printed {{R}} is required",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: no pump is applied",
            case.id
        );
    }
}

#[test]
fn q85_pump_cards_carry_their_exact_printed_characteristics() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = pump_engine(20_865_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.objects[&source].card_id, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(source, *keyword),
                "{}: printed {keyword:?}",
                case.id
            );
        }
    }
}

fn face_has_changeling(card_id: &str) -> bool {
    tricerules_cards::registry::global()
        .get(card_id)
        .map(|definition| {
            definition
                .primary_face()
                .characteristic_defining_abilities
                .iter()
                .any(|ability| {
                    matches!(
                        ability.definition,
                        tricerules_cards::CharacteristicDefiningAbility::Changeling
                    )
                })
        })
        .unwrap_or(false)
}

// ── 03 deathtouch grants ──────────────────────────────────────────────────

struct DeathtouchCase {
    id: &'static str,
    activation: ManaGift,
    body: (u32, u32),
    keywords: &'static [Keyword],
    changeling: bool,
}

fn deathtouch_cases() -> [DeathtouchCase; 3] {
    [
        DeathtouchCase {
            id: "battlefly_swarm",
            activation: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (1, 1),
            keywords: &[Keyword::Flying],
            changeling: false,
        },
        DeathtouchCase {
            id: "fetid_imp",
            activation: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (1, 2),
            keywords: &[Keyword::Flying],
            changeling: false,
        },
        DeathtouchCase {
            id: "moonglove_changeling",
            activation: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (2, 2),
            keywords: &[],
            changeling: true,
        },
    ]
}

fn deathtouch_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &["grizzly_bears"]),
        deck_with("mountain", &["grizzly_bears"]),
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

fn deathtouch_gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn activate_deathtouch(e: &mut GameEngine, source: u32, case: &DeathtouchCase) {
    give_mana(e, 0, deathtouch_gift(&case.activation));
    e.apply_scenario_command(0, &activate_ability_for(e, source, 0, vec![]))
        .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    resolve_entire_stack_two_player(e);
}

#[test]
fn q85_deathtouch_canary_battlefly_swarm_gains_deathtouch() {
    let case = &deathtouch_cases()[0];
    let mut e = deathtouch_engine(20_870_000);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    assert!(
        !e.effective_has_keyword(source, Keyword::Deathtouch),
        "no deathtouch before activation"
    );
    assert!(
        e.effective_has_keyword(source, Keyword::Flying),
        "printed Flying is live"
    );
    activate_deathtouch(&mut e, source, case);
    assert!(
        e.effective_has_keyword(source, Keyword::Deathtouch),
        "the grant is active after resolution"
    );
    assert!(!e.state.objects[&source].tapped, "no tap cost");
}

#[test]
fn q85_deathtouch_cards_grant_only_the_source_and_only_deathtouch() {
    for (index, case) in deathtouch_cases().iter().enumerate() {
        let mut e = deathtouch_engine(20_871_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        activate_deathtouch(&mut e, source, case);
        assert!(
            e.effective_has_keyword(source, Keyword::Deathtouch),
            "{}: the source has deathtouch",
            case.id
        );
        assert!(
            !e.effective_has_keyword(other, Keyword::Deathtouch),
            "{}: another creature is unaffected",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(source, *keyword),
                "{}: printed {keyword:?} is unaffected",
                case.id
            );
        }
    }
}

#[test]
fn q85_deathtouch_cards_expire_at_cleanup() {
    for (index, case) in deathtouch_cases().iter().enumerate() {
        let mut e = deathtouch_engine(20_872_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate_deathtouch(&mut e, source, case);
        assert!(e.effective_has_keyword(source, Keyword::Deathtouch));
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(source, Keyword::Deathtouch),
            "{}: CR 514.2 ends the grant",
            case.id
        );
    }
}

#[test]
fn q85_deathtouch_cards_reject_activation_without_black_mana() {
    for (index, case) in deathtouch_cases().iter().enumerate() {
        let mut e = deathtouch_engine(20_873_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: the printed {{B}} is required",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(
            !e.effective_has_keyword(source, Keyword::Deathtouch),
            "{}: no grant is applied",
            case.id
        );
        // The same activation must succeed once the printed mana is available, so this
        // rejection is about the missing cost and not about an absent card definition.
        activate_deathtouch(&mut e, source, case);
        assert!(
            e.effective_has_keyword(source, Keyword::Deathtouch),
            "{}: the printed mana both enables the activation and grants deathtouch",
            case.id
        );
    }
}

#[test]
fn q85_deathtouch_cards_carry_their_exact_printed_characteristics() {
    for (index, case) in deathtouch_cases().iter().enumerate() {
        let mut e = deathtouch_engine(20_874_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert_eq!(e.state.objects[&source].card_id, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(source, *keyword),
                "{}: printed {keyword:?}",
                case.id
            );
        }
        assert!(
            !e.effective_has_keyword(source, Keyword::Deathtouch),
            "{}: deathtouch is granted, not printed",
            case.id
        );
        if case.changeling {
            assert!(
                face_has_changeling(case.id),
                "{}: changeling card carries a CDA",
                case.id
            );
        }
    }
}

#[test]
fn q85_deathtouch_changeling_is_every_creature_type() {
    let mut e = deathtouch_engine(20_875_000);
    let source = inject_permanent_on_battlefield(&mut e, 0, "moonglove_changeling");
    let face = tricerules_cards::registry::global()
        .get("moonglove_changeling")
        .expect("registered")
        .primary_face();
    assert!(
        face.has_subtype("Elf") && face.has_subtype("Zombie") && face.has_subtype("Cat"),
        "CR 702.73/205.3m: changeling is every creature type"
    );
    assert!(
        !e.effective_has_keyword(source, Keyword::Deathtouch),
        "the printed deathtouch grant is an activated ability"
    );
}
