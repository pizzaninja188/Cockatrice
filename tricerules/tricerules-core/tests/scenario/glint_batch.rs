//! Actual-card scenarios for the reviewed Glint pump-and-hexproof family.
//!
//! Expectations are printed Oracle values. CR 115.1 provides one creature target;
//! CR 611.2a and 514.2 give the until-end-of-turn duration; CR 613.4c applies the
//! layer 7c pump and CR 613.1f applies the layer 6 hexproof grant. CR 601.2c and
//! 702.11b reject an opponent's target at cast time; CR 608.2b fizzles a spell whose
//! only target is no longer a creature the caster controls at resolution.
use crate::helpers::ScenarioCommandExt;

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
            id: "beaming_defiance",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
        Case {
            id: "glint",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            power: 0,
            toughness: 3,
        },
        Case {
            id: "woodcutters_grit",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 3,
            toughness: 3,
        },
    ]
}

fn engine(seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("forest", &["grizzly_bears"]),
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
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    (e, own, opposing)
}

fn cast_resolve(e: &mut GameEngine, case: &Case, target: u32) {
    give_mana(e, 0, gift(&case.mana));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{}", case.id);
}

#[test]
fn glint_batch_cards_pump_and_grant_hexproof_until_end_of_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let (mut e, own, opposing) = engine(800_000 + index as u64);
        cast_resolve(&mut e, &case, own);
        assert_eq!(e.effective_power(own), Some(2 + case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(own),
            Some(2 + case.toughness),
            "{}",
            case.id
        );
        assert!(
            e.effective_has_keyword(own, Keyword::Hexproof),
            "{}",
            case.id
        );
        assert_eq!(
            e.effective_power(opposing),
            Some(2),
            "{}: only the chosen creature is pumped",
            case.id
        );
        assert_eq!(
            e.effective_toughness(opposing),
            Some(2),
            "{}: the chosen creature's toughness is the only one pumped",
            case.id
        );
        assert!(
            !e.effective_has_keyword(opposing, Keyword::Hexproof),
            "{}: only the chosen creature gains hexproof",
            case.id
        );

        // The granted hexproof rejects an opponent's targeted removal spell.
        e.apply_scenario_command(0, &pass())
            .expect("active player passes priority");
        give_mana(
            &mut e,
            1,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let hand_before = e.state.players[1].hand.len();
        let removal = inject_card_into_hand(&mut e, 1, "shock");
        let removal_slot = hand_index_for_card(&e, 1, "shock");
        let error = e
            .apply_scenario_command(1, &cast_spell(removal_slot, target_object(own)))
            .expect_err("hexproof rejects the opponent's removal");
        assert!(
            matches!(error, tricerules_core::EngineError::Illegal(_)),
            "{}: the hexproof restriction, not another cause, rejects the removal: {error:?}",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            hand_before + 1,
            "{}: rejected removal stays in hand",
            case.id
        );
        assert!(e.state.players[1].hand.contains(&removal), "{}", case.id);
        e.apply_scenario_command(1, &pass())
            .expect("opponent passes priority");

        end_active_turn(&mut e, 0);
        assert_eq!(e.effective_power(own), Some(2), "{}", case.id);
        assert_eq!(e.effective_toughness(own), Some(2), "{}", case.id);
        assert!(
            !e.effective_has_keyword(own, Keyword::Hexproof),
            "{}: keyword grant ends with the turn",
            case.id
        );
    }
}

#[test]
fn glint_batch_cards_reject_an_opponents_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let (mut e, _, opposing) = engine(801_000 + index as u64);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        let pool_before = mana_pool(&e);
        let error = e
            .apply_scenario_command(0, &cast_spell(hand_slot, target_object(opposing)))
            .expect_err("an opponent's creature is not a legal target");
        assert!(
            matches!(error, tricerules_core::EngineError::Illegal(_)),
            "{}: the target restriction, not another cause, rejects the cast: {error:?}",
            case.id
        );
        assert_eq!(e.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert_eq!(mana_pool(&e), pool_before, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn glint_batch_cards_fizzle_if_the_creature_changes_controller() {
    for (index, case) in cases().into_iter().enumerate() {
        let (mut e, own, _) = engine(802_000 + index as u64);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(hand_slot, target_object(own)))
            .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));

        // The target is no longer a creature the caster controls; CR 608.2b fizzles.
        e.state.players[0]
            .battlefield
            .retain(|candidate| *candidate != own);
        e.state.players[1].battlefield.push(own);
        let new_controller = e.state.players[1].id;
        let object = e.state.objects.get_mut(&own).expect("target");
        object.controller = new_controller;
        object.base_controller = new_controller;

        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.effective_power(own), Some(2), "{}", case.id);
        assert_eq!(e.effective_toughness(own), Some(2), "{}", case.id);
        assert!(
            !e.effective_has_keyword(own, Keyword::Hexproof),
            "{}: fizzled spell grants nothing",
            case.id
        );
        assert_eq!(e.state.objects[&slot].zone, Zone::Graveyard, "{}", case.id);
    }
}

fn mana_pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
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
