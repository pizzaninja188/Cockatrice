//! CR 701.20 untap — the `UntapTarget` / `UntapAll` primitives and the two cards built on them,
//! Seeker of Skybreak (`{T}: Untap target creature`) and Vitalize (untap all creatures you
//! control). The mirror image of the `TapTarget` / `TapAllCreatures` coverage in
//! `spell_effects.rs`.

use crate::helpers::*;

fn target(oid: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: oid,
        damage_amount: 0,
    }]
}

fn set_tapped(e: &mut GameEngine, oid: u32, tapped: bool) {
    e.state.objects.get_mut(&oid).expect("object").tapped = tapped;
}

fn is_tapped(e: &GameEngine, oid: u32) -> bool {
    e.state.objects.get(&oid).expect("object").tapped
}

/// P0 at main 1 with an untapped Seeker of Skybreak and one Grizzly Bears, `bear_tapped` as asked.
fn seeker_engine(seed: u64, bear_tapped: bool) -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        deck_with("forest", &["seeker_of_skybreak", "grizzly_bears"]),
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let seeker = relocate_to_battlefield(&mut e, 0, "seeker_of_skybreak", false);
    let bear = relocate_to_battlefield(&mut e, 0, "grizzly_bears", bear_tapped);
    (e, seeker, bear)
}

#[test]
fn seeker_of_skybreak_untaps_a_tapped_creature_and_taps_itself() {
    let (mut e, seeker, bear) = seeker_engine(9101, true);
    assert!(is_tapped(&e, bear), "setup: the bear starts tapped");

    e.apply_command(0, &activate_ability(seeker, 0, target(bear)))
        .expect("activate the untap ability");
    // CR 118.12: the {T} cost is paid on activation, before the ability resolves.
    assert!(is_tapped(&e, seeker), "the {{T}} cost taps the Seeker");
    assert!(
        is_tapped(&e, bear),
        "the target is still tapped on the stack"
    );

    resolve_entire_stack_two_player(&mut e);
    assert!(!is_tapped(&e, bear), "the ability untapped its target");
    assert!(
        is_tapped(&e, seeker),
        "resolving the ability does not untap its own source"
    );
}

#[test]
fn untap_target_may_target_an_already_untapped_creature_and_does_nothing() {
    // CR 701.20 places no "tapped" restriction on "untap target creature": an untapped creature
    // is a legal target and the effect simply has no effect on it.
    let (mut e, seeker, bear) = seeker_engine(9102, false);
    assert!(!is_tapped(&e, bear), "setup: the bear starts untapped");

    e.apply_command(0, &activate_ability(seeker, 0, target(bear)))
        .expect("an untapped creature is a legal target");
    resolve_entire_stack_two_player(&mut e);
    assert!(!is_tapped(&e, bear), "still untapped, no crash, no-op");
}

#[test]
fn untap_target_rejects_a_target_outside_its_filter() {
    // The ability reads "untap target creature", so a land is not a legal target.
    let (mut e, seeker, _bear) = seeker_engine(9103, true);
    let forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    set_tapped(&mut e, forest, true);

    let err = e
        .apply_command(0, &activate_ability(seeker, 0, target(forest)))
        .expect_err("a land is not a creature");
    assert!(
        matches!(err, tricerules_core::EngineError::Illegal(_)),
        "expected Illegal, got {err:?}"
    );
    assert!(
        is_tapped(&e, forest),
        "the illegal activation changed nothing"
    );
    assert!(!is_tapped(&e, seeker), "a rejected activation pays no cost");
}

#[test]
fn vitalize_untaps_only_the_casters_creatures() {
    let decks = Some(vec![
        deck_with("forest", &["vitalize", "grizzly_bears"]),
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(9104, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let my_bear = relocate_to_battlefield(&mut e, 0, "grizzly_bears", true);
    let my_forest = inject_permanent_on_battlefield(&mut e, 0, "forest");
    set_tapped(&mut e, my_forest, true);
    let their_bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    set_tapped(&mut e, their_bear, true);

    cast_instant_and_resolve(
        &mut e,
        0,
        "vitalize",
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );

    assert!(!is_tapped(&e, my_bear), "my creature untapped");
    assert!(
        is_tapped(&e, my_forest),
        "`filter: (kind: Creature)` leaves my land tapped"
    );
    assert!(
        is_tapped(&e, their_bear),
        "`players: Controller` leaves the opponent's creature tapped"
    );
}
