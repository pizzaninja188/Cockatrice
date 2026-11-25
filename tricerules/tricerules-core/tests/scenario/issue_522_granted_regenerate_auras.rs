//! Actual-card scenarios for the Auras that grant "{cost}: Regenerate this creature." (issue #522):
//! Molting Snakeskin, Savage Silhouette and Trollhide.
//!
//! CR 303.4a/613.4c: the Aura attaches and pumps the enchanted creature. CR 613.1f/602.2: the
//! enchanted creature has the granted ability, its controller activates it by paying the printed
//! mana, and "this creature" is the enchanted creature. CR 701.19a-b: the resolved ability puts a
//! regeneration shield on that creature, which replaces its next destruction this turn (tap,
//! remove from combat, remove all damage). The grant ends when the Aura leaves.
use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_core::{AttachmentRecipient, Zone};

/// Granted abilities follow the creature's (empty) printed ability list.
const GRANTED_SLOT: u32 = 1;

struct Case {
    id: &'static str,
    /// Aura casting cost as (black, green, generic).
    cast: (u32, u32, u32),
    /// Printed cost of the granted ability as (black, green, generic).
    activate: (u32, u32, u32),
    /// Printed granted activation line.
    line: &'static str,
    /// Attached (power, toughness) modifier.
    pump: (u32, u32),
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "molting_snakeskin",
            cast: (1, 0, 0),
            activate: (1, 0, 2),
            line: "{2}{B}: Regenerate this creature.",
            pump: (2, 0),
        },
        Case {
            id: "savage_silhouette",
            cast: (0, 1, 2),
            activate: (0, 1, 1),
            line: "{1}{G}: Regenerate this creature.",
            pump: (2, 2),
        },
        Case {
            id: "trollhide",
            cast: (0, 1, 2),
            activate: (0, 1, 1),
            line: "{1}{G}: Regenerate this creature.",
            pump: (2, 2),
        },
    ]
}

fn ability_texts(engine: &mut GameEngine, oid: u32) -> Vec<String> {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == oid)
                .map(|object| {
                    object
                        .activated_abilities
                        .iter()
                        .map(|ability| ability.text.clone())
                        .collect()
                }),
            _ => None,
        })
        .unwrap_or_default()
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn gift((b, g, c): (u32, u32, u32)) -> ManaGift {
    ManaGift {
        b,
        g,
        c,
        ..Default::default()
    }
}

fn shields(e: &GameEngine, oid: u32) -> u32 {
    e.state.objects[&oid].regeneration_shields
}

/// Player 0 enchants a fresh Grizzly Bears with the case's Aura; returns (bears, aura).
fn enchanted_bears(case: &Case, seed: u64) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "molting_snakeskin",
                "savage_silhouette",
                "trollhide",
                "murder",
            ],
        ),
        vec!["swamp".into(); 20],
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #522 card data");
    advance_to_main1_from_game_start(&mut e);
    let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    ensure_card_in_hand(&mut e, 0, case.id);
    give_mana(&mut e, 0, gift(case.cast));
    let slot = hand_index_for_card(&e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(bears)))
        .unwrap_or_else(|err| panic!("{}: cast Aura: {err:?}", case.id));
    resolve_entire_stack_two_player(&mut e);
    let aura = battlefield_object_for_card(&e, 0, case.id);
    (e, bears, aura)
}

fn activate_regenerate(e: &mut GameEngine, case: &Case, bears: u32) {
    give_mana(e, 0, gift(case.activate));
    apply_ability(e, 0, bears, GRANTED_SLOT, vec![])
        .unwrap_or_else(|err| panic!("{}: activate granted Regenerate: {err:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: printed cost paid",
        case.id
    );
    assert_eq!(
        e.state
            .stack
            .last()
            .and_then(|item| item.source_permanent_id),
        Some(bears),
        "{}: the enchanted creature is the ability source",
        case.id
    );
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_522_auras_pump_and_publish_the_printed_regenerate_line() {
    for (index, case) in cases().into_iter().enumerate() {
        let (mut e, bears, aura) = enchanted_bears(&case, 522_000 + index as u64);
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(AttachmentRecipient::Object(bears)),
            "{}",
            case.id
        );
        assert_eq!(
            (e.effective_power(bears), e.effective_toughness(bears)),
            (Some(2 + case.pump.0), Some(2 + case.pump.1)),
            "{}: attached pump",
            case.id
        );
        assert_eq!(
            ability_texts(&mut e, bears),
            vec![case.line.to_string()],
            "{}: exact granted line",
            case.id
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 0, bears),
            vec![case.line],
            "{}: rules annotation names the granted ability",
            case.id
        );
    }
}

#[test]
fn issue_522_granted_regenerate_shield_replaces_one_destruction() {
    for (index, case) in cases().into_iter().enumerate() {
        let (mut e, bears, aura) = enchanted_bears(&case, 522_100 + index as u64);
        activate_regenerate(&mut e, &case, bears);
        assert_eq!(shields(&e, bears), 1, "{}: one shield", case.id);
        assert!(!e.state.objects[&bears].tapped, "{}", case.id);

        cast_targeted_instant_and_resolve(
            &mut e,
            0,
            "murder",
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
            target_object(bears),
        );
        assert_eq!(
            e.state.objects[&bears].zone,
            Zone::Battlefield,
            "{}: the shield replaces destruction",
            case.id
        );
        assert!(
            e.state.objects[&bears].tapped,
            "{}: regenerated creature taps",
            case.id
        );
        assert_eq!(shields(&e, bears), 0, "{}: shield used up", case.id);
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(AttachmentRecipient::Object(bears)),
            "{}",
            case.id
        );
    }
}

#[test]
fn issue_522_granted_regenerate_rejects_insufficient_mana_without_state_change() {
    for (index, case) in cases().into_iter().enumerate() {
        let (mut e, bears, _) = enchanted_bears(&case, 522_200 + index as u64);
        // One mana short: only the colored part of the printed cost.
        let (b, g, _) = case.activate;
        give_mana(&mut e, 0, gift((b, g, 0)));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, bears, GRANTED_SLOT, vec![]).is_err(),
            "{}: activation needs the full printed cost",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana spent", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(shields(&e, bears), 0, "{}", case.id);
    }
}

#[test]
fn issue_522_granted_regenerate_leaves_with_the_aura() {
    for (index, case) in cases().into_iter().enumerate() {
        let (mut e, bears, aura) = enchanted_bears(&case, 522_300 + index as u64);
        e.state.players[0].battlefield.retain(|oid| *oid != aura);
        e.state.players[0].graveyard.push(aura);
        e.state.objects.get_mut(&aura).expect("aura").zone = Zone::Graveyard;

        assert_eq!(
            (e.effective_power(bears), e.effective_toughness(bears)),
            (Some(2), Some(2)),
            "{}: pump ends",
            case.id
        );
        assert!(ability_texts(&mut e, bears).is_empty(), "{}", case.id);
        assert!(
            zone_view_rules_annotation_labels(&mut e, 0, bears).is_empty(),
            "{}",
            case.id
        );
        give_mana(&mut e, 0, gift(case.activate));
        assert!(
            apply_ability(&mut e, 0, bears, GRANTED_SLOT, vec![]).is_err(),
            "{}: the granted slot is gone",
            case.id
        );
    }
}
