//! Actual-card scenarios for the queue-v66 batch A cohorts.
//!
//! Three stable typed shapes: an activated card-draw ability with printed mana and life costs,
//! an Aura that controls the enchanted creature (with a printed pump rider on the hardest card),
//! and a permanent that investigates by creating one Clue token. Expectations are printed Oracle
//! values. CR 602.2 and 118.3 govern activation and costs; CR 119.4 stops a life payment below
//! zero; CR 303.4 and 613.1b cover Aura control; CR 603.6a entry triggers; CR 701.16a and 111.10f
//! create the Clue artifact token; CR 121.1 draws the card.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{AttachmentRecipient, Zone};
use tricerules_proto::ruled::v1::TargetRef;

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn hand_size(e: &GameEngine, player: usize) -> usize {
    e.state.players[player].hand.len()
}

fn activate(
    e: &mut GameEngine,
    player: i32,
    permanent: u32,
    ability: u32,
    targets: Vec<TargetRef>,
) {
    semantic::accepted(
        e,
        player,
        &activate_ability_for(e, permanent, ability, targets),
    );
}

fn target_object(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        counter_amount: 0,
        expected_zone_change_generation: None,
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: 0,
    }]
}

fn cast_from_hand(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) -> u32 {
    let id = inject_card_into_hand(e, player, card);
    give_mana(e, player as i32, mana);
    let slot = hand_index_for_card(e, player, card);
    semantic::accepted(e, player as i32, &cast_spell(slot, targets));
    id
}

// ── 01 activated draw ──────────────────────────────────────────────────────

#[test]
fn q66_draw_canary_simplest_arcane_encyclopedia_draws_for_mana_and_tap() {
    let mut e = semantic::main_phase(66_010_001);
    let tome = inject_permanent_on_battlefield(&mut e, 0, "arcane_encyclopedia");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let before = hand_size(&e, 0);

    activate(&mut e, 0, tome, 0, vec![]);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed {{3}} cost");
    assert!(e.state.objects[&tome].tapped, "tap is part of the cost");
    assert_eq!(hand_size(&e, 0), before, "the card is drawn on resolution");
    assert_eq!(e.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut e);
    assert_eq!(hand_size(&e, 0), before + 1, "exactly one card");
    assert!(e.state.stack.is_empty());

    // A tapped source cannot pay the tap cost again.
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, tome, 0, vec![]))
            .is_err(),
        "tapped Arcane Encyclopedia cannot activate"
    );
    assert_eq!(
        hand_size(&e, 0),
        before + 1,
        "the rejected activation draws nothing"
    );
}

#[test]
fn q66_draw_canary_hardest_greed_pays_life_and_rejects_insufficient_life() {
    let mut e = semantic::main_phase(66_010_002);
    let greed = inject_permanent_on_battlefield(&mut e, 0, "greed");
    let before = hand_size(&e, 0);
    e.state.players[0].life = 20;
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );

    activate(&mut e, 0, greed, 0, vec![]);
    assert_eq!(
        e.state.players[0].life, 18,
        "two life is paid as an activation cost"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed {{B}} is paid");
    assert!(!e.state.objects[&greed].tapped, "Greed does not tap");
    assert_eq!(hand_size(&e, 0), before, "the draw waits for resolution");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(hand_size(&e, 0), before + 1, "exactly one card");

    // CR 119.4: a player cannot pay life below zero.
    e.state.players[0].life = 1;
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, greed, 0, vec![]))
            .is_err(),
        "one life cannot pay a two-life cost"
    );
    assert_eq!(e.state.players[0].life, 1, "no partial life payment");
    assert_eq!(hand_size(&e, 0), before + 1, "no card was drawn");
}

#[test]
fn q66_draw_cards_use_their_printed_costs() {
    // Azure Mage: {3}{U} and no tap.
    let mut e = semantic::main_phase(66_011_001);
    let mage = inject_permanent_on_battlefield(&mut e, 0, "azure_mage");
    assert_eq!(
        (e.effective_power(mage), e.effective_toughness(mage)),
        (Some(2), Some(1))
    );
    let before = hand_size(&e, 0);
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    activate(&mut e, 0, mage, 0, vec![]);
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "exact printed {{3}}{{U}} cost"
    );
    assert!(!e.state.objects[&mage].tapped, "Azure Mage does not tap");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(hand_size(&e, 0), before + 1);

    // Book of Rass: {2} and 2 life, no tap.
    let mut e = semantic::main_phase(66_011_002);
    let book = inject_permanent_on_battlefield(&mut e, 0, "book_of_rass");
    let before = hand_size(&e, 0);
    e.state.players[0].life = 20;
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    activate(&mut e, 0, book, 0, vec![]);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact printed {{2}} cost");
    assert_eq!(e.state.players[0].life, 18, "two life is part of the cost");
    assert!(!e.state.objects[&book].tapped, "Book of Rass does not tap");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(hand_size(&e, 0), before + 1);

    // Insufficient mana is rejected before any cost is paid.
    let mut e = semantic::main_phase(66_011_003);
    let tome = inject_permanent_on_battlefield(&mut e, 0, "arcane_encyclopedia");
    let before = hand_size(&e, 0);
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, tome, 0, vec![]))
            .is_err(),
        "two generic cannot pay {{3}}"
    );
    assert!(!e.state.objects[&tome].tapped, "no cost was paid");
    assert_eq!(hand_size(&e, 0), before, "no card was drawn");
    assert_eq!(e.state.stack.len(), 0);
}

// ── 02 control Auras ───────────────────────────────────────────────────────

fn bounce_aura(e: &mut GameEngine, aura: u32) {
    semantic::accepted(e, 0, &pass());
    give_mana(
        e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 1, "boomerang");
    semantic::accepted(e, 1, &cast_spell(slot, target_object(aura)));
    resolve_entire_stack_two_player(e);
}

#[test]
fn q66_control_canary_simplest_control_magic_takes_and_restores_control() {
    let mut e = semantic::main_phase(66_020_001);
    inject_card_into_hand(&mut e, 1, "boomerang");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let control = cast_from_hand(
        &mut e,
        0,
        "control_magic",
        ManaGift {
            u: 4,
            ..Default::default()
        },
        target_object(bear),
    );
    assert_eq!(
        e.state.objects[&bear].controller, 1,
        "control changes only on resolution"
    );
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.objects[&control].zone, Zone::Battlefield);
    assert_eq!(
        e.state.objects[&control].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    assert_eq!(e.state.objects[&bear].owner, 1, "ownership never changes");
    assert_eq!(e.state.objects[&bear].controller, 0, "the Aura controls it");
    assert!(e.state.players[0].battlefield.contains(&bear));
    assert!(!e.state.players[1].battlefield.contains(&bear));
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "Control Magic does not change the body"
    );

    // The Aura leaving restores the owner's control.
    bounce_aura(&mut e, control);
    assert_eq!(e.state.objects[&control].zone, Zone::Hand);
    assert_eq!(e.state.objects[&bear].controller, 1);
    assert!(e.state.players[1].battlefield.contains(&bear));
    assert!(!e.state.players[0].battlefield.contains(&bear));
}

#[test]
fn q66_control_canary_hardest_spirit_away_adds_the_printed_body_and_flying() {
    let mut e = semantic::main_phase(66_020_002);
    inject_card_into_hand(&mut e, 1, "boomerang");
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let aura = cast_from_hand(
        &mut e,
        0,
        "spirit_away",
        ManaGift {
            u: 7,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.objects[&bear].controller, 0);
    assert_eq!(
        e.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(4), Some(4)),
        "printed +2/+2"
    );
    assert!(
        e.effective_has_keyword(bear, tricerules_cards::Keyword::Flying),
        "printed flying"
    );

    bounce_aura(&mut e, aura);
    assert_eq!(e.state.objects[&bear].controller, 1);
    assert_eq!(
        (e.effective_power(bear), e.effective_toughness(bear)),
        (Some(2), Some(2)),
        "the pump ends with the Aura"
    );
    assert!(
        !e.effective_has_keyword(bear, tricerules_cards::Keyword::Flying),
        "flying ends with the Aura"
    );
}

#[test]
fn q66_control_persuasion_matches_the_shared_control_shape() {
    let mut e = semantic::main_phase(66_021_001);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let persuasion = cast_from_hand(
        &mut e,
        0,
        "persuasion",
        ManaGift {
            u: 5,
            ..Default::default()
        },
        target_object(bear),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&persuasion].zone, Zone::Battlefield);
    assert_eq!(e.state.objects[&bear].controller, 0);
    assert_eq!(e.state.objects[&bear].owner, 1);
    assert_eq!(
        e.state.objects[&persuasion].attached_to,
        Some(AttachmentRecipient::Object(bear))
    );
}

// ── 03 investigate permanents ──────────────────────────────────────────────

fn resolve_entry_and_collect_tokens(e: &mut GameEngine) -> Vec<u32> {
    // First pass cycle resolves the creature spell and parks its entry trigger; the second
    // resolves that trigger and publishes the Clue token creation.
    let _ = pass_both_players_with_batch(e);
    let batch = pass_both_players_with_batch(e);
    let created = token_created_events(&batch);
    for token in &created {
        let identity = token.identity.as_ref().expect("public token identity");
        assert_eq!(identity.name, "Clue");
        assert_eq!(identity.types, ["Artifact", "Clue"]);
        assert!(!identity.is_creature, "a Clue is not a creature");
        assert_eq!(identity.pt, "", "a Clue has no body");
        assert_eq!(identity.color, "", "a Clue is colorless");
    }
    battlefield_token_oids(e, 0, "clue")
}

#[test]
fn q66_investigate_canary_simplest_thraben_inspector_enters_and_investigates() {
    let mut e = semantic::main_phase(66_030_001);
    let inspector = cast_from_hand(
        &mut e,
        0,
        "thraben_inspector",
        ManaGift {
            w: 1,
            ..Default::default()
        },
        vec![],
    );
    let clues = resolve_entry_and_collect_tokens(&mut e);
    assert_eq!(e.state.objects[&inspector].zone, Zone::Battlefield);
    assert_eq!(
        (
            e.effective_power(inspector),
            e.effective_toughness(inspector)
        ),
        (Some(1), Some(2))
    );
    assert_eq!(clues.len(), 1, "exactly one Clue token");
    assert_eq!(
        battlefield_token_oids(&e, 1, "clue").len(),
        0,
        "the opponent gets no Clue"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        7,
        "investigate draws nothing"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn q66_investigate_canary_hardest_magnifying_glass_has_a_mana_ability_and_investigates() {
    // {T}: Add {C} is a mana ability: no stack, exact colorless mana.
    let mut e = semantic::main_phase(66_030_002);
    let glass = inject_permanent_on_battlefield(&mut e, 0, "magnifying_glass");
    let command = activate_ability_for(&e, glass, 0, vec![]);
    semantic::accepted(&mut e, 0, &command);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 1), "exact {{C}}");
    assert!(e.state.objects[&glass].tapped);
    assert!(
        e.state.stack.is_empty(),
        "a mana ability does not use the stack"
    );

    // {4}, {T}: Investigate creates exactly one Clue and pays four generic.
    let mut e = semantic::main_phase(66_030_003);
    let glass = inject_permanent_on_battlefield(&mut e, 0, "magnifying_glass");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    activate(&mut e, 0, glass, 1, vec![]);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{4}}");
    assert!(e.state.objects[&glass].tapped);
    let batch = pass_both_players_with_batch(&mut e);
    let created = token_created_events(&batch);
    assert_eq!(created.len(), 1, "one Clue token");
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 1);

    // Insufficient mana is rejected before the tap cost is paid.
    let mut e = semantic::main_phase(66_030_004);
    let glass = inject_permanent_on_battlefield(&mut e, 0, "magnifying_glass");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, glass, 1, vec![]))
            .is_err(),
        "three generic cannot pay {{4}}"
    );
    assert!(!e.state.objects[&glass].tapped);
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 0);
}

#[test]
fn q66_investigate_cards_create_one_clue_on_their_printed_path() {
    // Drownyard Explorers: entry trigger on a 2/4 body.
    let mut e = semantic::main_phase(66_031_001);
    let explorers = cast_from_hand(
        &mut e,
        0,
        "drownyard_explorers",
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
        vec![],
    );
    assert_eq!(resolve_entry_and_collect_tokens(&mut e).len(), 1);
    assert_eq!(
        (
            e.effective_power(explorers),
            e.effective_toughness(explorers)
        ),
        (Some(2), Some(4))
    );

    // Floodhound: {3}, {T}: Investigate on a 1/2 body.
    let mut e = semantic::main_phase(66_031_002);
    let hound = inject_permanent_on_battlefield(&mut e, 0, "floodhound");
    assert_eq!(
        (e.effective_power(hound), e.effective_toughness(hound)),
        (Some(1), Some(2))
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    activate(&mut e, 0, hound, 0, vec![]);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{3}}");
    assert!(e.state.objects[&hound].tapped);
    let batch = pass_both_players_with_batch(&mut e);
    assert_eq!(token_created_events(&batch).len(), 1);
    assert_eq!(battlefield_token_oids(&e, 0, "clue").len(), 1);
}
