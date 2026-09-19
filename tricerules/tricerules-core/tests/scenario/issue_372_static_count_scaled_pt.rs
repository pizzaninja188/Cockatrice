//! Issue #372 focused scenarios for the retained graveyard-count-scaled static P/T cohort.
//!
//! Oracle and current Comprehensive Rules were checked 2026-09-18. CR 404.2 governs reading
//! printed public graveyard card data; CR 613.4c and the layer system order the count-scaled
//! modifiers after base P/T setters; CR 208.2a / 604.3 govern Neo Exdeath's power-defining CDA;
//! CR 303.4 / 611.3 govern Song of Stupefaction's attached modifier and its disappearance when
//! the Aura leaves; CR 603.4 rechecks Exdeath's intervening-if on resolution.

use super::helpers::*;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

fn engine_with(seed: u64, own: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", own), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn stats(engine: &GameEngine, oid: u32) -> (u32, u32) {
    let characteristics = engine.characteristics(oid).expect("characteristics");
    (
        characteristics.power.expect("power"),
        characteristics.toughness.expect("toughness"),
    )
}

fn remove_from_battlefield(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .battlefield
        .retain(|candidate| *candidate != object_id);
    let object = engine.state.objects.get_mut(&object_id).expect("object");
    object.zone = Zone::Exile;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn remove_graveyard_card(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|candidate| *candidate != object_id);
    let object = engine.state.objects.get_mut(&object_id).expect("object");
    object.zone = Zone::Exile;
}

fn seat_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].library.push_front(object_id);
    object_id
}

fn advance_to_end_step(engine: &mut GameEngine, active: i32) {
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine
        .apply_command(active, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(active, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(active, &primitive_yield())
        .expect("end combat to main 2");
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 2 to end step");
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

fn resolve_top_stack(engine: &mut GameEngine) -> RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine.apply_command(first, &pass()).expect("first pass");
    engine
        .apply_command(second, &pass())
        .expect("second pass resolves stack item")
}

fn select_optional_effect() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: 0,
            ..Default::default()
        })),
    }
}

fn decline_optional_effect() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::Decline as i32,
            ..Default::default()
        })),
    }
}

fn pass_until_song_entry_choice(engine: &mut GameEngine) {
    for _ in 0..8 {
        if engine.state.pending_resolution.is_some() {
            return;
        }
        answer_trigger_order_in_engine_order(engine);
        let priority = engine.state.priority_player_id();
        if engine.apply_command(priority, &pass()).is_err() {
            assert!(
                engine.state.pending_resolution.is_some(),
                "the Song entry choice must park a private decision"
            );
            return;
        }
    }
    panic!("Song's entry trigger did not park a choice");
}

fn cast_song_of_stupefaction(engine: &mut GameEngine, target: u32, accept_mill: bool) -> u32 {
    inject_card_into_hand(engine, 0, "song_of_stupefaction");
    give_mana(
        engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "song_of_stupefaction");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Song of Stupefaction");
    // The Aura spell resolves, then its optional entry mill triggers.
    engine.apply_command(0, &pass()).expect("caster pass");
    engine
        .apply_command(1, &pass())
        .expect("opponent pass resolves the Aura spell");
    let aura = battlefield_object_for_card(engine, 0, "song_of_stupefaction");
    pass_until_song_entry_choice(engine);
    let decision = if accept_mill {
        select_optional_effect()
    } else {
        decline_optional_effect()
    };
    engine
        .apply_command(0, &decision)
        .expect("answer the optional entry mill");
    resolve_entire_stack_two_player(engine);
    aura
}

fn move_exdeath_to_battlefield(engine: &mut GameEngine) -> u32 {
    let exdeath = move_ready_to_battlefield(
        engine,
        0,
        "exdeath,_void_warlock_neo_exdeath,_dimensions_end",
    );
    // The front face's "when Exdeath enters, you gain 3 life" trigger resolves before the turn
    // can advance.
    resolve_entire_stack_two_player(engine);
    exdeath
}

#[test]
fn issue_372_xande_counts_own_noncreature_nonland_graveyard_cards_live() {
    let mut engine = engine_with(372_001, &["xande,_dark_mage"]);
    let xande = move_ready_to_battlefield(&mut engine, 0, "xande,_dark_mage");
    assert_eq!(stats(&engine, xande), (3, 3));

    let own_instant = inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    assert_eq!(
        stats(&engine, xande),
        (4, 4),
        "a noncreature, nonland card in the controller's graveyard scales both components"
    );

    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "forest");
    assert_eq!(
        stats(&engine, xande),
        (4, 4),
        "creature and land cards are outside the noncreature, nonland cohort"
    );

    inject_graveyard_card(&mut engine, 1, "lightning_bolt");
    assert_eq!(
        stats(&engine, xande),
        (4, 4),
        "an opponent's graveyard never contributes"
    );

    remove_graveyard_card(&mut engine, 0, own_instant);
    assert_eq!(
        stats(&engine, xande),
        (3, 3),
        "removing the qualifying card removes the scaling live"
    );
}

#[test]
fn issue_372_moon_vigil_affine_sum_reevaluates_battlefield_and_graveyard() {
    let mut engine = engine_with(372_010, &["moon-vigil_adherents"]);
    let adherents = move_ready_to_battlefield(&mut engine, 0, "moon-vigil_adherents");
    assert_eq!(
        stats(&engine, adherents),
        (1, 1),
        "the source itself is one creature you control"
    );

    let own_bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    assert_eq!(stats(&engine, adherents), (2, 2));

    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    assert_eq!(
        stats(&engine, adherents),
        (2, 2),
        "an opponent's battlefield creature does not contribute"
    );

    let own_graveyard_bear = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    assert_eq!(
        stats(&engine, adherents),
        (3, 3),
        "the graveyard creature card leaf contributes to the affine sum"
    );

    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    assert_eq!(
        stats(&engine, adherents),
        (3, 3),
        "land cards and the opponent's graveyard stay outside the sum"
    );

    remove_from_battlefield(&mut engine, 0, own_bear);
    assert_eq!(
        stats(&engine, adherents),
        (2, 2),
        "a creature leaving the battlefield drops the battlefield leaf"
    );

    remove_graveyard_card(&mut engine, 0, own_graveyard_bear);
    assert_eq!(
        stats(&engine, adherents),
        (1, 1),
        "removing the graveyard card drops the graveyard leaf"
    );
}

#[test]
fn issue_372_exdeath_below_the_six_permanent_card_gate_stays_front() {
    let mut engine = engine_with(
        372_020,
        &["exdeath,_void_warlock_neo_exdeath,_dimensions_end"],
    );
    let exdeath = move_exdeath_to_battlefield(&mut engine);
    assert_eq!(stats(&engine, exdeath), (3, 3));

    for _ in 0..5 {
        inject_graveyard_card(&mut engine, 0, "forest");
    }
    inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    advance_to_end_step(&mut engine, 0);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&exdeath].face_up_index, 0,
        "five permanent cards plus a nonpermanent card stay below the six-permanent gate"
    );
    assert_eq!(stats(&engine, exdeath), (3, 3));
}

#[test]
fn issue_372_exdeath_six_permanent_cards_transform_and_cda_reevaluates() {
    let mut engine = engine_with(
        372_030,
        &["exdeath,_void_warlock_neo_exdeath,_dimensions_end"],
    );
    let exdeath = move_exdeath_to_battlefield(&mut engine);
    for _ in 0..6 {
        inject_graveyard_card(&mut engine, 0, "forest");
    }
    advance_to_end_step(&mut engine, 0);
    resolve_top_stack(&mut engine);
    assert_eq!(
        engine.state.objects[&exdeath].face_up_index, 1,
        "the sixth permanent card turns the source into its back face"
    );
    assert_eq!(
        stats(&engine, exdeath),
        (6, 3),
        "power comes from the permanent-card count while toughness stays printed"
    );

    let extra = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    assert_eq!(stats(&engine, exdeath), (7, 3));
    inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    assert_eq!(
        stats(&engine, exdeath),
        (7, 3),
        "an instant card is not a permanent card"
    );
    inject_graveyard_card(&mut engine, 1, "forest");
    assert_eq!(
        stats(&engine, exdeath),
        (7, 3),
        "an opponent's graveyard never contributes"
    );

    remove_graveyard_card(&mut engine, 0, extra);
    assert_eq!(
        stats(&engine, exdeath),
        (6, 3),
        "the CDA reevaluates as permanent cards leave the graveyard"
    );
}

#[test]
fn issue_372_song_attached_modifier_targets_only_the_enchanted_permanent() {
    let mut engine = engine_with(372_040, &["song_of_stupefaction", "grizzly_bears"]);
    let enchanted = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let bystander = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let aura = cast_song_of_stupefaction(&mut engine, enchanted, false);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(AttachmentRecipient::Object(enchanted))
    );

    for _ in 0..3 {
        inject_graveyard_card(&mut engine, 0, "forest");
    }
    assert_eq!(
        stats(&engine, enchanted),
        (0, 2),
        "three permanent cards give -3/-0 while toughness is unchanged"
    );
    assert_eq!(
        stats(&engine, bystander),
        (2, 2),
        "the attached modifier never affects an unenchanted permanent"
    );
    inject_graveyard_card(&mut engine, 1, "forest");
    assert_eq!(
        stats(&engine, enchanted),
        (0, 2),
        "an opponent's graveyard never contributes to the attached count"
    );

    engine.state.objects.get_mut(&aura).expect("Aura").zone = Zone::Graveyard;
    assert_eq!(
        stats(&engine, enchanted),
        (2, 2),
        "the modifier disappears when the Aura is unattached"
    );
}

#[test]
fn issue_372_song_entry_mill_is_optional_and_mills_two_when_taken() {
    // Decline: the library is untouched and no cards enter the graveyard.
    let mut decline = engine_with(372_050, &["song_of_stupefaction", "grizzly_bears"]);
    let target = relocate_to_battlefield(&mut decline, 0, "grizzly_bears", false);
    let library_before = decline.state.players[0].library.len();
    cast_song_of_stupefaction(&mut decline, target, false);
    assert_eq!(decline.state.players[0].library.len(), library_before);
    assert_eq!(count_card_id_in_graveyard(&decline, 0, "forest"), 0);

    // Accept: exactly the chosen top two cards are milled.
    let mut accept = engine_with(372_051, &["song_of_stupefaction", "grizzly_bears"]);
    let target = relocate_to_battlefield(&mut accept, 0, "grizzly_bears", false);
    let first = seat_on_top(&mut accept, 0, "lightning_bolt");
    let second = seat_on_top(&mut accept, 0, "forest");
    let library_before = accept.state.players[0].library.len();
    cast_song_of_stupefaction(&mut accept, target, true);
    assert_eq!(accept.state.players[0].library.len(), library_before - 2);
    assert_eq!(accept.state.objects[&first].zone, Zone::Graveyard);
    assert_eq!(accept.state.objects[&second].zone, Zone::Graveyard);
}

#[test]
fn issue_372_cda_resolves_before_song_count_scaling() {
    // Song is cast in the main phase, then Exdeath transforms at the end step. The CDA is a
    // layer-7b setter, so power becomes the permanent-card count before Song's layer-7c
    // modifier subtracts its own permanent-card count from the same public graveyard.
    let mut engine = engine_with(
        372_060,
        &[
            "song_of_stupefaction",
            "exdeath,_void_warlock_neo_exdeath,_dimensions_end",
        ],
    );
    let exdeath = move_exdeath_to_battlefield(&mut engine);
    for _ in 0..6 {
        inject_graveyard_card(&mut engine, 0, "forest");
    }
    cast_song_of_stupefaction(&mut engine, exdeath, false);

    advance_to_end_step(&mut engine, 0);
    resolve_top_stack(&mut engine);
    assert_eq!(engine.state.objects[&exdeath].face_up_index, 1);
    assert_eq!(
        stats(&engine, exdeath),
        (0, 3),
        "the layer-7b CDA resolves before the layer-7c -X/-0 modifier"
    );
}

#[test]
fn issue_372_base_pt_setter_resolves_before_song_count_scaling() {
    // Doc Ock's conditional base P/T setter is layer 7b; Song's attached count modifier is layer
    // 7c. With eight permanent cards the base 8/8 lands first and then -8/-0 leaves toughness.
    let mut engine = engine_with(
        372_070,
        &["song_of_stupefaction", "doc_ock,_sinister_scientist"],
    );
    let doc_ock = move_ready_to_battlefield(&mut engine, 0, "doc_ock,_sinister_scientist");
    for _ in 0..8 {
        inject_graveyard_card(&mut engine, 0, "forest");
    }
    assert_eq!(stats(&engine, doc_ock), (8, 8));

    cast_song_of_stupefaction(&mut engine, doc_ock, false);
    assert_eq!(
        stats(&engine, doc_ock),
        (0, 8),
        "the layer-7b base setter resolves before the layer-7c count modifier"
    );
}
