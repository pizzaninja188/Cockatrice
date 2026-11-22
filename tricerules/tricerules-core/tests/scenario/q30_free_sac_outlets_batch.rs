//! Actual-card scenarios for the queue-v30 free sacrifice outlets.
//!
//! Expectations are printed Oracle values. CR 118.3/701.21a pay the sacrifice as an
//! activation cost; CR 605.1a/605.3b resolve a targetless mana ability immediately;
//! CR 602.2 puts other abilities on the stack. The source is in the graveyard before
//! the ability resolves.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
}

fn cases() -> [Case; 8] {
    [
        Case {
            id: "blood_pet",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "blood_vassal",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "catalyst_elemental",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "reckless_barbarian",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
        },
        Case {
            id: "alchemists_apprentice",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "hapless_researcher",
            mana: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "kami_of_false_hope",
            mana: ManaGift {
                w: 1,
                ..Default::default()
            },
            body: (1, 1),
        },
        Case {
            id: "spore_frog",
            mana: ManaGift {
                g: 1,
                ..Default::default()
            },
            body: (1, 1),
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
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn q30_free_sac_outlets_canary_simplest() {
    let case = &cases()[0];
    let mut e = engine(30_030_001);
    let source = cast(&mut e, case);
    apply_ability(&mut e, 0, source, 0, vec![]).expect("sacrifice for black mana");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(pool(&e), (0, 0, 1, 0, 0, 0));
}

#[test]
fn q30_free_sac_outlets_canary_hardest() {
    let case = &cases()[5];
    let mut e = engine(30_030_002);
    let source = cast(&mut e, case);
    let hand_before = e.state.players[0].hand.len();
    let drawn = inject_library_card(&mut e, 0, "forest");
    e.state.players[0].library.retain(|oid| *oid != drawn);
    e.state.players[0].library.push_front(drawn);
    apply_ability(&mut e, 0, source, 0, vec![]).expect("sacrifice to loot");
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("mandatory discard");
    assert_eq!(pending.deciding_player, e.state.players[0].id);
    let discarded = pending.presentation.candidates[0];
    e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("discard one card");
    assert_eq!(e.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(e.state.players[0].hand.len(), hand_before);
    assert_eq!(e.state.objects[&drawn].zone, Zone::Hand);
    assert!(e.state.pending_resolution.is_none());
}

#[test]
fn q30_free_sac_outlets_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_031_000 + index as u64);
        let source = cast(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}",
            case.id
        );
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact printed cost",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q30_free_sac_outlets_cards_sacrifice_as_cost_and_resolve() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_032_000 + index as u64);
        let source = cast(&mut e, &case);
        let hand_before = e.state.players[0].hand.len();
        let drawn = inject_library_card(&mut e, 0, "forest");
        e.state.players[0].library.retain(|oid| *oid != drawn);
        e.state.players[0].library.push_front(drawn);
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate the sacrifice outlet");
        let mana_ability = matches!(
            case.id,
            "blood_pet" | "blood_vassal" | "catalyst_elemental" | "reckless_barbarian"
        );
        if mana_ability {
            assert!(
                e.state.stack.is_empty(),
                "{}: mana ability resolves immediately",
                case.id
            );
        } else {
            assert_eq!(
                e.state.stack.len(),
                1,
                "{}: non-mana ability uses the stack",
                case.id
            );
        }
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: the source is sacrificed before resolution",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        match case.id {
            "blood_pet" => assert_eq!(pool(&e), (0, 0, 1, 0, 0, 0), "{}", case.id),
            "blood_vassal" => assert_eq!(pool(&e), (0, 0, 2, 0, 0, 0), "{}", case.id),
            "catalyst_elemental" | "reckless_barbarian" => {
                assert_eq!(pool(&e), (0, 0, 0, 2, 0, 0), "{}", case.id)
            }
            "alchemists_apprentice" => {
                assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{}", case.id);
                assert_eq!(
                    e.state.players[0].hand.len(),
                    hand_before + 1,
                    "{}",
                    case.id
                );
            }
            "hapless_researcher" => {
                assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{}", case.id);
                let pending = e
                    .state
                    .pending_resolution
                    .as_ref()
                    .expect("mandatory discard");
                let discarded = pending.presentation.candidates[0];
                e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
                    .expect("discard one card");
                assert_eq!(
                    e.state.objects[&discarded].zone,
                    Zone::Graveyard,
                    "{}: the discarded card goes to the graveyard",
                    case.id
                );
                assert_eq!(
                    e.state.players[0].hand.len(),
                    hand_before,
                    "{}: draw one then discard one",
                    case.id
                );
                assert_eq!(e.state.objects[&drawn].zone, Zone::Hand, "{}", case.id);
            }
            "kami_of_false_hope" | "spore_frog" => {
                assert!(e.state.pending_resolution.is_none(), "{}", case.id);
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn q30_free_sac_outlets_cards_reject_activation_without_the_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(30_033_000 + index as u64);
        let source = cast(&mut e, &case);
        e.state.players[0].battlefield.retain(|oid| *oid != source);
        e.state.objects.get_mut(&source).expect("creature").zone = Zone::Graveyard;
        e.state.players[0].graveyard.push(source);
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: the sacrificed source cannot activate from the graveyard",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&source),
            "{}",
            case.id
        );
    }
}

#[test]
fn q30_free_sac_outlets_kami_and_spore_prevent_combat_damage() {
    for (index, id) in ["kami_of_false_hope", "spore_frog"].into_iter().enumerate() {
        let mut e = engine(30_034_000 + index as u64);
        let source = inject_creature_on_battlefield(&mut e, 1, id);
        end_active_turn(&mut e, 0);
        advance_to_main1_from_game_start(&mut e);
        assert_eq!(e.state.active_player_id(), e.state.players[1].id, "{}", id);
        apply_ability(&mut e, 1, source, 0, vec![]).expect("activate fog");
        resolve_entire_stack_two_player(&mut e);
        let attacker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(1, &primitive_yield())
            .expect("to combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(1, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        pass_both_players(&mut e);
        e.apply_scenario_command(1, &pass()).expect("p1 pass");
        e.apply_scenario_command(0, &pass()).expect("p0 pass");
        e.apply_scenario_command(1, &pass())
            .expect("p1 pass to damage");
        e.apply_scenario_command(0, &pass())
            .expect("damage resolves");
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: combat damage prevented",
            id
        );
        assert_eq!(e.state.objects[&source].zone, Zone::Graveyard, "{}", id);
    }
}
