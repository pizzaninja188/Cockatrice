//! Actual-card publication of Oracle fragment spans (issue #531).
//!
//! A granted quoted ability, a single keyword of a keyword line and a single sentence of a
//! longer line used to publish only generated fallback text. Each now publishes one span whose
//! Unicode-scalar range selects exactly the printed fragment from the fingerprinted Oracle line.
//! Presentation only: no rules surface changes (the mechanics are covered by issues #415, #522,
//! #64 and the ward cohort tests).
use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_core::AttachmentRecipient;
use tricerules_proto::ruled::v1 as rv1;

/// Select `presentation`'s spans from the printed line exactly as a client does.
fn printed(presentation: &rv1::PresentationRef, oracle_line: &str) -> String {
    assert!(
        !presentation.oracle_text_sha256.is_empty(),
        "{}: fragments require the face fingerprint",
        presentation.card_id
    );
    let [span] = presentation.oracle_spans.as_slice() else {
        panic!(
            "{}: expected exactly one fragment span, got {:?}",
            presentation.card_id, presentation.oracle_spans
        );
    };
    let (start, end) = (
        span.start.expect("fragment start") as usize,
        span.end.expect("fragment end") as usize,
    );
    oracle_line.chars().skip(start).take(end - start).collect()
}

fn zone_view_ability(engine: &mut GameEngine, oid: u32, slot: usize) -> rv1::PresentationRef {
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
                .and_then(|object| object.activated_abilities.get(slot))
                .and_then(|ability| ability.presentation.clone()),
            _ => None,
        })
        .expect("zone view publishes the granted ability presentation")
}

fn engine_with(seed: u64, cards: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", cards),
        deck_with("island", &["boomerang"]),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("issue #531 card data");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

/// Equipment enters and is attached directly; an Aura is cast at the creature (CR 303.4a).
fn attach(engine: &mut GameEngine, card_id: &str, creature: u32, aura_cost: Option<ManaGift>) {
    let Some(cost) = aura_cost else {
        let attachment = move_ready_to_battlefield(engine, 0, card_id);
        engine
            .state
            .objects
            .get_mut(&attachment)
            .expect("attachment object")
            .attached_to = Some(AttachmentRecipient::Object(creature));
        return;
    };
    ensure_card_in_hand(engine, 0, card_id);
    give_mana(engine, 0, cost);
    let slot = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .unwrap_or_else(|err| panic!("{card_id}: cast Aura: {err:?}"));
    resolve_entire_stack_two_player(engine);
    let aura = battlefield_object_for_card(engine, 0, card_id);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(creature))
    );
}

#[test]
fn issue_531_granted_quoted_abilities_publish_the_quoted_fragment() {
    let green = |g, c| ManaGift {
        g,
        c,
        ..Default::default()
    };
    let blue = ManaGift {
        u: 1,
        c: 1,
        ..Default::default()
    };
    for (seed, card_id, aura_cost, line, expected) in [
        (
            531_001,
            "trollhide",
            Some(green(1, 2)),
            "Enchanted creature gets +2/+2 and has \"{1}{G}: Regenerate this creature.\" (The next time the creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.)",
            "{1}{G}: Regenerate this creature.",
        ),
        (
            531_002,
            "wrench",
            None,
            "Equipped creature gets +1/+1 and has vigilance and \"{3}, {T}: Tap target creature.\"",
            "{3}, {T}: Tap target creature.",
        ),
        (
            531_003,
            "hermetic_study",
            Some(blue),
            "Enchanted creature has \"{T}: This creature deals 1 damage to any target.\"",
            "{T}: This creature deals 1 damage to any target.",
        ),
    ] {
        let mut engine = engine_with(seed, &[card_id]);
        let bears = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        attach(&mut engine, card_id, bears, aura_cost);
        // Grizzly Bears has no printed abilities, so the granted ability is the only one.
        let presentation = zone_view_ability(&mut engine, bears, 0);
        assert_eq!(presentation.card_id, card_id);
        assert_eq!(printed(&presentation, line), expected, "{card_id}");
        assert!(
            presentation.whole_oracle_lines().is_empty(),
            "{card_id}: the granting line is not repeated"
        );
    }
}

#[test]
fn issue_531_ward_keyword_trigger_publishes_only_its_keyword_on_the_stack() {
    let mut engine = engine_with(531_010, &["humpback_whales"]);
    let whales = inject_creature_on_battlefield(&mut engine, 0, "humpback_whales");
    let caster = engine.state.players[1].id;
    let caster_index = engine.state.player_idx(caster).expect("opponent");
    inject_card_into_hand(&mut engine, caster_index, "boomerang");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let priority = engine.state.priority_player_id();
    engine
        .apply_scenario_command(priority, &pass())
        .expect("yield priority to the opponent");
    let slot = hand_index_for_card(&engine, caster_index, "boomerang");
    engine
        .apply_scenario_command(caster, &cast_spell(slot, target_object(whales)))
        .expect("target Humpback Whales");
    let ward = engine.state.stack.last().expect("ward trigger").id;
    let presentation = engine.state.stack_presentations[&ward]
        .primary
        .clone()
        .expect("ward trigger presentation");
    assert_eq!(
        printed(
            &presentation,
            "Vigilance, ward {2} (Whenever this creature becomes the target of a spell or ability an opponent controls, counter it unless that player pays {2}.)"
        ),
        "ward {2}"
    );
}

#[test]
fn issue_531_delayed_trigger_publishes_its_printed_sentence() {
    let mut engine = engine_with(531_020, &["ray_of_command"]);
    let creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_card_in_hand(&mut engine, 0, "ray_of_command");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "ray_of_command");
    engine
        .apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Ray of Command");
    resolve_entire_stack_two_player(&mut engine);
    end_active_turn(&mut engine, 0);
    let trigger = engine.state.stack.last().expect("control-loss trigger").id;
    let presentation = engine.state.stack_presentations[&trigger]
        .primary
        .clone()
        .expect("delayed trigger presentation");
    assert_eq!(
        printed(
            &presentation,
            "Untap target creature an opponent controls and gain control of it until end of turn. That creature gains haste until end of turn. When you lose control of the creature, tap it."
        ),
        "When you lose control of the creature, tap it."
    );
}
