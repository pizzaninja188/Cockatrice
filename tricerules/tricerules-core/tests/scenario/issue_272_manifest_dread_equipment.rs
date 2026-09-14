//! Issue #272: Manifest Dread passes its exact face-down object to a following
//! non-targeting Equipment attachment without exposing hidden identity.

use crate::helpers::*;
use tricerules_cards::primitives::{AbilityCost, EffectSubject};
use tricerules_cards::{AbilityPresentation, CardRegistry, Keyword, SpellEffectKind};
use tricerules_core::{AttachmentRecipient, Zone};
use tricerules_proto::ruled::v1::{ruled_event::Ev, ChoiceKind, RuledEventBatch};

fn seat_on_top(engine: &mut GameEngine, card_ids: &[&str]) -> Vec<u32> {
    let ids: Vec<_> = card_ids
        .iter()
        .map(|id| inject_library_card(engine, 0, id))
        .collect();
    engine.state.players[0]
        .library
        .retain(|id| !ids.contains(id));
    for &id in ids.iter().rev() {
        engine.state.players[0].library.push_front(id);
    }
    ids
}

fn engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 4,
            b: 4,
            c: 12,
            ..Default::default()
        },
    );
    engine
}

fn cast_equipment(engine: &mut GameEngine, card_id: &str) -> u32 {
    inject_card_into_hand(engine, 0, card_id);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|e| panic!("cast {card_id}: {e:?}"));
    pass_both_players(engine);
    battlefield_object_for_card(engine, 0, card_id)
}

fn reach_manifest_choice(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == 0 { 1 } else { 0 };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("resolve trigger")
}

#[test]
fn equipment_manifest_dread_recipes_have_exact_registry_semantics() {
    let registry = CardRegistry::global();
    for (id, equip_cost) in [
        ("conductive_machete", "{4}"),
        ("cursed_windbreaker", "{3}"),
        ("killers_mask", "{2}"),
    ] {
        let face = registry.get(id).expect("registered card").primary_face();
        let trigger = &face.triggered_abilities[0];
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(
            trigger.effect,
            vec![
                SpellEffectKind::ManifestDread,
                SpellEffectKind::AttachEquipment {
                    equipment: EffectSubject::Source,
                    creature: EffectSubject::PreviousEffectObject
                },
            ]
        );
        let AbilityCost::Mana(cost) = &face.activated_abilities[0].costs[0] else {
            panic!("mana equip cost")
        };
        assert_eq!(cost.to_string(), equip_cost);
    }
}

#[test]
fn all_three_attach_to_the_exact_manifested_object_and_apply_their_modifier() {
    for (seed, card_id, power, toughness, keyword) in [
        (272_001, "conductive_machete", 4, 3, None),
        (272_002, "cursed_windbreaker", 2, 2, Some(Keyword::Flying)),
        (272_003, "killers_mask", 2, 2, Some(Keyword::Menace)),
    ] {
        let mut engine = engine(seed);
        let top = seat_on_top(&mut engine, &["grizzly_bears", "lightning_bolt"]);
        let equipment = cast_equipment(&mut engine, card_id);
        let parked = reach_manifest_choice(&mut engine);
        let choice = find_resolution_choice(&parked).expect("Manifest Dread choice");
        assert_eq!(choice.choice_kind(), ChoiceKind::ManifestDread);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!(choice.candidate_object_ids, top);
        let completed = engine
            .apply_command(0, &submit_resolution_choice(vec![top[0]]))
            .expect("manifest and attach");
        assert_eq!(engine.state.objects[&top[0]].zone, Zone::Battlefield);
        assert!(engine.state.objects[&top[0]].face_down);
        assert_eq!(
            engine.state.objects[&equipment].attached_to,
            Some(AttachmentRecipient::Object(top[0]))
        );
        assert_eq!(engine.effective_power(top[0]), Some(power));
        assert_eq!(engine.effective_toughness(top[0]), Some(toughness));
        let chars = engine.characteristics(top[0]).expect("manifested creature");
        if let Some(keyword) = keyword {
            assert!(chars.keywords.contains(&keyword));
        }
        let public_log = completed
            .events
            .iter()
            .filter_map(|event| match &event.ev {
                Some(Ev::Log(log)) if log.visible_to_player_id.is_none() => Some(log.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(public_log.contains("Face-down creature"));
        assert!(!public_log.contains("Grizzly Bears"));
    }
}

#[test]
fn attachment_tail_fails_closed_when_manifest_dread_produces_no_object() {
    let mut empty = engine(272_004);
    let equipment = cast_equipment(&mut empty, "conductive_machete");
    empty.state.players[0].library.clear();
    resolve_entire_stack_two_player(&mut empty);
    assert_eq!(empty.state.objects[&equipment].attached_to, None);
}

#[test]
fn replacement_completion_preserves_result_but_rejects_stale_source() {
    let mut engine = engine(272_006);
    let top = seat_on_top(&mut engine, &["grizzly_bears", "lightning_bolt"]);
    let equipment = cast_equipment(&mut engine, "conductive_machete");
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
    assert!(find_resolution_choice(&reach_manifest_choice(&mut engine)).is_some());
    let replacement = engine
        .apply_command(0, &submit_resolution_choice(vec![top[0]]))
        .expect("park replacement ordering");
    let choice = find_resolution_choice(&replacement).expect("replacement choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::ReplacementEffect);
    *engine
        .state
        .zone_change_generation
        .entry(equipment)
        .or_default() += 1;
    engine
        .apply_command(
            0,
            &submit_resolution_choice(vec![choice.candidate_object_ids[0]]),
        )
        .expect("finish entry");
    assert_eq!(engine.state.objects[&top[0]].zone, Zone::Battlefield);
    assert!(engine.state.objects[&top[0]].tapped);
    assert_eq!(engine.state.objects[&equipment].attached_to, None);
}
