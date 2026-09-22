//! Actual-card semantics for five pinned Standard spells.
//! Pinned Oracle and current Scryfall rulings reviewed 2026-09-22.

use super::helpers::*;
use tricerules_cards::{primitives::CounterKind, Keyword};
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast_and_resolve(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_misc32_gravkill_target_union() {
    let mut spacecraft = engine(832_001);
    let ship = inject_permanent_on_battlefield(&mut spacecraft, 1, "uthros_scanship");
    assert_eq!(
        spacecraft.effective_power(ship),
        None,
        "unstationed Spacecraft is not a creature"
    );
    cast_and_resolve(&mut spacecraft, "gravkill", target_object(ship));
    assert_eq!(spacecraft.state.objects[&ship].zone, Zone::Exile);

    let mut creature = engine(832_002);
    let bear = inject_creature_on_battlefield(&mut creature, 1, "grizzly_bears");
    cast_and_resolve(&mut creature, "gravkill", target_object(bear));
    assert_eq!(creature.state.objects[&bear].zone, Zone::Exile);

    let mut artifact = engine(832_003);
    let boots = inject_permanent_on_battlefield(&mut artifact, 1, "swiftfoot_boots");
    inject_card_into_hand(&mut artifact, 0, "gravkill");
    let slot = hand_index_for_card(&artifact, 0, "gravkill");
    assert!(artifact
        .apply_command(0, &cast_spell(slot, target_object(boots)))
        .is_err());
}

#[test]
fn issue_misc32_darkness_descends_all_creatures() {
    let mut e = engine(832_010);
    let own = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 4, 4);
    let opposing = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 4, 4);
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "swiftfoot_boots");
    cast_and_resolve(&mut e, "darkness_descends", vec![]);
    for oid in [own, opposing] {
        assert_eq!(
            e.state.objects[&oid].counter_count(CounterKind::MinusOneMinusOne),
            2
        );
    }
    assert_eq!(
        e.state.objects[&artifact].counter_count(CounterKind::MinusOneMinusOne),
        0
    );
}

#[test]
fn issue_misc32_preposterous_proportions_snapshot() {
    let mut e = engine(832_020);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve(&mut e, "preposterous_proportions", vec![]);
    assert_eq!(
        (e.effective_power(own), e.effective_toughness(own)),
        (Some(12), Some(12))
    );
    assert!(e.effective_has_keyword(own, Keyword::Vigilance));
    assert_eq!(
        (e.effective_power(opposing), e.effective_toughness(opposing)),
        (Some(2), Some(2))
    );
    assert!(!e.effective_has_keyword(opposing, Keyword::Vigilance));
    let later = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert_eq!(
        e.effective_power(later),
        Some(2),
        "later entry is outside the snapshot"
    );
    assert!(!e.effective_has_keyword(later, Keyword::Vigilance));
}

#[test]
fn issue_misc32_bewildering_blizzard_scope() {
    let mut e = engine(832_030);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let before = e.state.players[0].hand.len();
    cast_and_resolve(&mut e, "bewildering_blizzard", vec![]);
    assert_eq!(
        e.state.players[0].hand.len(),
        before + 3,
        "spell leaves hand and draws three"
    );
    assert_eq!(e.effective_power(own), Some(2));
    assert_eq!(e.effective_power(opposing), Some(0));
    assert_eq!(e.effective_toughness(opposing), Some(2));
    let later = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    assert_eq!(
        e.effective_power(later),
        Some(2),
        "later opposing entry is unaffected"
    );
}

#[test]
fn issue_misc32_stroke_of_midnight_recipient() {
    let mut destroyed = engine(832_039);
    let bear = inject_creature_on_battlefield(&mut destroyed, 1, "grizzly_bears");
    cast_and_resolve(&mut destroyed, "stroke_of_midnight", target_object(bear));
    assert_eq!(destroyed.state.objects[&bear].zone, Zone::Graveyard);
    assert_eq!(
        battlefield_token_oids(&destroyed, 1, "human_w_1_1").len(),
        1
    );
    assert!(battlefield_token_oids(&destroyed, 0, "human_w_1_1").is_empty());

    let mut e = engine(832_040);
    let myr = inject_creature_on_battlefield(&mut e, 1, "darksteel_myr");
    assert!(e.effective_has_keyword(myr, Keyword::Indestructible));
    cast_and_resolve(&mut e, "stroke_of_midnight", target_object(myr));
    assert_eq!(e.state.objects[&myr].zone, Zone::Battlefield);
    assert_eq!(battlefield_token_oids(&e, 1, "human_w_1_1").len(), 1);
    assert!(battlefield_token_oids(&e, 0, "human_w_1_1").is_empty());

    let mut illegal = engine(832_041);
    let land = inject_permanent_on_battlefield(&mut illegal, 1, "forest");
    inject_card_into_hand(&mut illegal, 0, "stroke_of_midnight");
    let slot = hand_index_for_card(&illegal, 0, "stroke_of_midnight");
    assert!(illegal
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .is_err());
    assert!(battlefield_token_oids(&illegal, 1, "human_w_1_1").is_empty());

    let mut fizzled = engine(832_042);
    let bear = inject_creature_on_battlefield(&mut fizzled, 1, "grizzly_bears");
    inject_card_into_hand(&mut fizzled, 0, "stroke_of_midnight");
    let slot = hand_index_for_card(&fizzled, 0, "stroke_of_midnight");
    semantic::accepted(&mut fizzled, 0, &cast_spell(slot, target_object(bear)));
    fizzled.state.players[1]
        .battlefield
        .retain(|oid| *oid != bear);
    fizzled.state.players[1].graveyard.push(bear);
    fizzled.state.objects.get_mut(&bear).unwrap().zone = Zone::Graveyard;
    *fizzled
        .state
        .zone_change_generation
        .entry(bear)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzled);
    assert!(battlefield_token_oids(&fizzled, 1, "human_w_1_1").is_empty());
}
