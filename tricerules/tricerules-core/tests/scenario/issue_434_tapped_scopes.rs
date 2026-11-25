//! Issue #434 — tapped and supertype creature scopes (Adept Watershaper, The Wandering Rescuer,
//! The Seriema).
//!
//! Oracle checked 2026-11-25 against the pinned Scryfall `oracle_cards` snapshot; Scryfall rulings
//! could not be fetched (HTTP 403) and none are relied on. The keyword grants are static abilities
//! re-evaluated continuously (CR 604.1, 611.3a) in layer 6 (CR 613.1f). Tapped is a status, not a
//! characteristic (CR 110.5); legendary is a supertype (CR 205.4). "Other" excludes the source.
//! The Seriema's anthem is printed in its 7+ striation, so it applies only while the Spacecraft has
//! seven or more charge counters (CR 722.2, 722.2a/b).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, CounterKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::ChoiceKind;

fn engine(seed: u64, own: &[&str]) -> GameEngine {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("plains", own), deck_with("swamp", &[])]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn set_tapped(e: &mut GameEngine, oid: u32, tapped: bool) {
    e.state.objects.get_mut(&oid).unwrap().tapped = tapped;
}

fn remove_all_abilities(e: &mut GameEngine, oid: u32) {
    e.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: e.state.command_index,
    });
}

fn murder(e: &mut GameEngine, target: u32) {
    inject_card_into_hand(e, 0, "murder");
    give_mana(
        e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Murder");
    resolve_entire_stack_two_player(e);
}

/// Shared "Other tapped creatures you control have <keyword>." behavior.
fn assert_tapped_anthem(id: &str, keyword: Keyword, seed: u64) {
    let mut e = engine(seed, &[id]);
    let source = move_ready_to_battlefield(&mut e, 0, id);
    let ally = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    assert!(
        !e.effective_has_keyword(ally, keyword),
        "{id}: untapped ally"
    );
    set_tapped(&mut e, ally, true);
    set_tapped(&mut e, opposing, true);
    set_tapped(&mut e, source, true);
    assert!(e.effective_has_keyword(ally, keyword), "{id}: tapped ally");
    assert!(
        !e.effective_has_keyword(opposing, keyword),
        "{id}: opponent's tapped creature"
    );
    assert!(
        !e.effective_has_keyword(source, keyword),
        "{id}: the source is excluded by other"
    );

    // Re-evaluated continuously: untapping removes the grant, tapping again restores it.
    set_tapped(&mut e, ally, false);
    assert!(
        !e.effective_has_keyword(ally, keyword),
        "{id}: untapped again"
    );
    set_tapped(&mut e, ally, true);
    assert!(e.effective_has_keyword(ally, keyword), "{id}: tapped again");

    // Layer-6 ability removal on the source ends its static grant.
    remove_all_abilities(&mut e, source);
    assert!(
        !e.effective_has_keyword(ally, keyword),
        "{id}: source lost its abilities"
    );
}

#[test]
fn issue_434_adept_watershaper_grants_indestructible_to_other_tapped_creatures() {
    assert_tapped_anthem("adept_watershaper", Keyword::Indestructible, 434_001);
}

#[test]
fn issue_434_wandering_rescuer_grants_hexproof_to_other_tapped_creatures() {
    assert_tapped_anthem("the_wandering_rescuer", Keyword::Hexproof, 434_002);
}

#[test]
fn issue_434_adept_watershaper_keeps_a_tapped_ally_alive_through_destroy() {
    let mut e = engine(434_003, &["adept_watershaper"]);
    move_ready_to_battlefield(&mut e, 0, "adept_watershaper");
    let tapped = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let untapped = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    set_tapped(&mut e, tapped, true);
    murder(&mut e, tapped);
    assert_eq!(
        e.state.objects[&tapped].zone,
        Zone::Battlefield,
        "indestructible survives destroy"
    );
    murder(&mut e, untapped);
    assert_eq!(
        e.state.objects[&untapped].zone,
        Zone::Graveyard,
        "an untapped creature is unprotected"
    );
}

#[test]
fn issue_434_the_seriema_searches_and_protects_tapped_legends_at_seven_counters() {
    let mut e = engine(434_004, &["the_seriema"]);
    let legend_in_library = inject_library_card(&mut e, 0, "barktooth_warbeard");
    let nonlegend_in_library = inject_library_card(&mut e, 0, "grizzly_bears");
    let seriema = move_ready_to_battlefield(&mut e, 0, "the_seriema");

    // Mandatory ETB search for a legendary creature card, revealed into hand.
    answer_trigger_order_in_engine_order(&mut e);
    e.apply_scenario_command(0, &pass())
        .expect("controller pass");
    let batch = e
        .apply_scenario_command(1, &pass())
        .expect("entry trigger resolves");
    let search = find_resolution_choice(&batch).expect("library search");
    assert_eq!(search.choice_kind(), ChoiceKind::LibrarySearch);
    assert!(search.candidate_object_ids.contains(&legend_in_library));
    assert!(!search.candidate_object_ids.contains(&nonlegend_in_library));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![legend_in_library]))
        .expect("take the legendary creature card");
    assert_eq!(e.state.objects[&legend_in_library].zone, Zone::Hand);

    let legend = inject_creature_on_battlefield(&mut e, 0, "barktooth_warbeard");
    let nonlegend = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opposing_legend = inject_creature_on_battlefield(&mut e, 1, "barktooth_warbeard");
    for oid in [legend, nonlegend, opposing_legend, seriema] {
        set_tapped(&mut e, oid, true);
    }

    // Below 7 charge counters the 7+ striation does nothing.
    e.state
        .objects
        .get_mut(&seriema)
        .unwrap()
        .set_counter(CounterKind::Charge, 6);
    assert!(
        !e.effective_has_keyword(legend, Keyword::Indestructible),
        "six counters grant nothing"
    );
    assert!(!e.effective_has_keyword(seriema, Keyword::Flying));
    assert!(!e.characteristics(seriema).unwrap().is_creature());

    e.state
        .objects
        .get_mut(&seriema)
        .unwrap()
        .set_counter(CounterKind::Charge, 7);
    let animated = e.characteristics(seriema).unwrap();
    assert!(animated.is_creature());
    assert_eq!((animated.power, animated.toughness), (Some(5), Some(5)));
    assert!(e.effective_has_keyword(seriema, Keyword::Flying));
    assert!(
        e.effective_has_keyword(legend, Keyword::Indestructible),
        "tapped legendary creature"
    );
    assert!(
        !e.effective_has_keyword(nonlegend, Keyword::Indestructible),
        "tapped nonlegendary creature"
    );
    assert!(
        !e.effective_has_keyword(opposing_legend, Keyword::Indestructible),
        "opponent's tapped legendary creature"
    );
    assert!(
        !e.effective_has_keyword(seriema, Keyword::Indestructible),
        "other excludes The Seriema itself"
    );
    set_tapped(&mut e, legend, false);
    assert!(
        !e.effective_has_keyword(legend, Keyword::Indestructible),
        "untapped legendary creature"
    );
}
