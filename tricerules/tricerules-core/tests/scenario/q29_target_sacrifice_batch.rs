//! Queue-v29 canaries: keyword creature plus "When this creature becomes the target of a spell or
//! ability, sacrifice it."
//!
//! CR 603.2, 603.3/608.1, 701.21a and 608.2b. Expectations are printed Oracle values.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
    keyword: Keyword,
}

fn case(id: &'static str) -> Case {
    match id {
        "gossamer_phantasm" => Case {
            id,
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
            keyword: Keyword::Flying,
        },
        "phantasmal_dreadmaw" => Case {
            id,
            mana: ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            power: 6,
            toughness: 6,
            keyword: Keyword::Trample,
        },
        "illusionary_servant" => Case {
            id,
            mana: ManaGift {
                u: 2,
                c: 1,
                ..Default::default()
            },
            power: 3,
            toughness: 4,
            keyword: Keyword::Flying,
        },
        "phantasmal_dragon" => Case {
            id,
            mana: ManaGift {
                u: 2,
                c: 2,
                ..Default::default()
            },
            power: 5,
            toughness: 5,
            keyword: Keyword::Flying,
        },
        "skulking_ghost" => Case {
            id,
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 1,
            keyword: Keyword::Flying,
        },
        "phantasmal_abomination" => Case {
            id,
            mana: ManaGift {
                u: 2,
                c: 1,
                ..Default::default()
            },
            power: 5,
            toughness: 5,
            keyword: Keyword::Defender,
        },
        _ => unreachable!(),
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

fn cases() -> [Case; 6] {
    [
        case("gossamer_phantasm"),
        case("illusionary_servant"),
        case("phantasmal_dragon"),
        case("skulking_ghost"),
        case("phantasmal_abomination"),
        case("phantasmal_dreadmaw"),
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("forest", &["sorins_thirst"]),
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

fn resolve_top_stack(e: &mut GameEngine) {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    e.apply_scenario_command(first, &pass())
        .expect("first pass");
    e.apply_scenario_command(second, &pass())
        .expect("second pass resolves the top stack item");
}

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let hand = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    let source = battlefield_object_for_card(e, 0, case.id);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.power), Some(case.toughness)),
        "{}: printed body",
        case.id
    );
    assert!(
        e.effective_has_keyword(source, case.keyword),
        "{}: printed keyword",
        case.id
    );
    source
}

fn assert_canary(case: Case, seed: u64) {
    let mut e = engine(seed, case.id);
    let creature = cast_source(&mut e, &case);
    inject_card_into_hand(&mut e, 1, "sorins_thirst");
    e.apply_scenario_command(0, &pass())
        .expect("active player passes priority");
    give_mana(
        &mut e,
        1,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "sorins_thirst");
    e.apply_scenario_command(1, &cast_spell(slot, target_object(creature)))
        .unwrap_or_else(|error| panic!("{}: target the creature: {error:?}", case.id));
    assert_eq!(
        e.state.stack.len(),
        2,
        "{}: the sacrifice trigger is above the spell",
        case.id
    );
    resolve_top_stack(&mut e);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Graveyard,
        "{}: the trigger sacrifices the source",
        case.id
    );
    assert!(e.state.players[0].graveyard.contains(&creature));
    assert_eq!(e.state.stack.len(), 1, "{}", case.id);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 20,
        "{}: the fizzled spell gains no life",
        case.id
    );
    assert!(e.state.stack.is_empty(), "{}", case.id);
}

#[test]
fn q29_target_sacrifice_canary_simplest() {
    assert_canary(case("gossamer_phantasm"), 29_040_001);
}

#[test]
fn q29_target_sacrifice_canary_hardest() {
    assert_canary(case("phantasmal_dreadmaw"), 29_040_002);
}

#[test]
fn q29_target_sacrifice_cards_cast_with_exact_cost_body_and_keyword() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_041_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, case.keyword),
            "{}: printed keyword",
            case.id
        );
    }
}

#[test]
fn q29_target_sacrifice_cards_trigger_for_an_ability_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_042_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        let pinger = inject_creature_on_battlefield(&mut e, 1, "prodigal_sorcerer");
        e.apply_scenario_command(0, &pass())
            .expect("active player passes priority");
        apply_ability(&mut e, 1, pinger, 0, target_object(creature))
            .unwrap_or_else(|error| panic!("{}: activate the ping: {error:?}", case.id));
        assert_eq!(e.state.stack.len(), 2, "{}", case.id);
        resolve_top_stack(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "{}: an ability target also sacrifices the source",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q29_target_sacrifice_cards_trigger_for_their_own_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_043_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        inject_card_into_hand(&mut e, 0, "lightning_bolt");
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "lightning_bolt");
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .unwrap_or_else(|error| panic!("{}: bolt own creature: {error:?}", case.id));
        resolve_top_stack(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "{}: Oracle has no opponent-only restriction",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.stack.len(), 0, "{}", case.id);
    }
}

#[test]
fn q29_target_sacrifice_cards_ignore_non_targeting_spells() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_044_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        inject_card_into_hand(&mut e, 0, "grizzly_bears");
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                c: 1,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &cast_spell(slot, vec![]))
            .expect("cast a non-targeting creature spell");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}: a non-targeting spell does not sacrifice the source",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q29_target_sacrifice_cards_still_sacrifice_when_the_targeting_spell_is_countered() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(29_045_000 + index as u64, case.id);
        let creature = cast_source(&mut e, &case);
        inject_card_into_hand(&mut e, 1, "sorins_thirst");
        e.apply_scenario_command(0, &pass())
            .expect("active player passes priority");
        give_mana(
            &mut e,
            1,
            ManaGift {
                b: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 1, "sorins_thirst");
        e.apply_scenario_command(1, &cast_spell(slot, target_object(creature)))
            .expect("opponent targets the creature");
        e.apply_scenario_command(1, &pass())
            .expect("opponent passes priority to the creature's controller");
        let thirst = e.state.stack[0].id;
        inject_card_into_hand(&mut e, 0, "counterspell");
        give_mana(
            &mut e,
            0,
            ManaGift {
                u: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, "counterspell");
        e.apply_scenario_command(
            0,
            &cast_spell(
                slot,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: thirst,
                    group_index: 0,
                    kind: 0,
                    ..Default::default()
                }],
            ),
        )
        .expect("counter the targeting spell");
        resolve_top_stack(&mut e);
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: only the sacrifice trigger remains after the counterspell",
            case.id
        );
        assert!(
            !e.state.stack.iter().any(|item| item.id == thirst),
            "{}: the targeting spell was actually countered",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Graveyard,
            "{}: countering the spell does not save the source",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
