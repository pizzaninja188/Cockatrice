//! Reviewed removal/damage one-shot scenarios: Lightning Strike, Flame Lash, Seismic Rupture,
//! Deathmark, Death in the Family, Epic Downfall, Repel Calamity, Eriette's Lullaby and Grapple
//! with Death.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 115 (targets),
//! 120 (damage), 701.8 (destroy), 701.13 (exile), 105.2 (color), 202.3 (mana value), and 119.3.

use super::helpers::*;
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

/// Put `card` in P0's hand and cast it with `targets`, resolving the whole stack.
fn cast_and_resolve(e: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, 0, card);
    let slot = hand_index_for_card(e, 0, card);
    semantic::accepted(e, 0, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

#[test]
fn issue_removal_lightning_strike() {
    let mut e = engine(723_001);
    let before = e.state.players[1].life;
    cast_and_resolve(&mut e, "lightning_strike", target_player_damage(1, 3));
    assert_eq!(
        e.state.players[1].life,
        before - 3,
        "three damage to a player"
    );

    // The same spell destroys a 3/3 creature.
    let mut creature_case = engine(723_011);
    let bear = inject_creature_with_stats(&mut creature_case, 1, "grizzly_bears", 3, 3);
    cast_and_resolve(&mut creature_case, "lightning_strike", target_object(bear));
    assert_eq!(creature_case.state.objects[&bear].zone, Zone::Graveyard);
}

#[test]
fn issue_removal_flame_lash() {
    let mut e = engine(723_002);
    let angel = inject_creature_with_stats(&mut e, 1, "serra_angel", 4, 4);
    cast_and_resolve(&mut e, "flame_lash", target_object(angel));
    assert_eq!(
        e.state.objects[&angel].zone,
        Zone::Graveyard,
        "four damage destroys a 4/4"
    );
}

#[test]
fn issue_removal_seismic_rupture() {
    let mut e = engine(723_003);
    let own_bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let flyer = inject_creature_on_battlefield(&mut e, 0, "serra_angel");
    let opposing_bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve(&mut e, "seismic_rupture", vec![]);
    assert_eq!(e.state.objects[&own_bear].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&opposing_bear].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&flyer].zone,
        Zone::Battlefield,
        "a creature with flying is excluded"
    );
}

#[test]
fn issue_removal_deathmark() {
    // A green creature is a legal target and is destroyed.
    let mut green = engine(723_004);
    let bear = inject_creature_on_battlefield(&mut green, 1, "grizzly_bears");
    cast_and_resolve(&mut green, "deathmark", target_object(bear));
    assert_eq!(green.state.objects[&bear].zone, Zone::Graveyard);

    // A white creature is a legal target.
    let mut white = engine(723_014);
    let lions = inject_creature_on_battlefield(&mut white, 1, "savannah_lions");
    cast_and_resolve(&mut white, "deathmark", target_object(lions));
    assert_eq!(white.state.objects[&lions].zone, Zone::Graveyard);

    // A blue creature is not.
    let mut blue = engine(723_024);
    let merfolk = inject_creature_on_battlefield(&mut blue, 1, "coral_merfolk");
    inject_card_into_hand(&mut blue, 0, "deathmark");
    let slot = hand_index_for_card(&blue, 0, "deathmark");
    assert!(
        blue.apply_command(0, &cast_spell(slot, target_object(merfolk)))
            .is_err(),
        "a blue creature is not green or white"
    );
}

#[test]
fn issue_removal_death_in_the_family() {
    // Mana value 2 is exiled.
    let mut small = engine(723_005);
    let bear = inject_creature_on_battlefield(&mut small, 1, "grizzly_bears");
    cast_and_resolve(&mut small, "death_in_the_family", target_object(bear));
    assert_eq!(small.state.objects[&bear].zone, Zone::Exile);

    // Mana value 4 is not a legal target.
    let mut big = engine(723_015);
    let giant = inject_creature_on_battlefield(&mut big, 1, "hill_giant");
    inject_card_into_hand(&mut big, 0, "death_in_the_family");
    let slot = hand_index_for_card(&big, 0, "death_in_the_family");
    assert!(
        big.apply_command(0, &cast_spell(slot, target_object(giant)))
            .is_err(),
        "mana value 4 exceeds the 3-or-less bound"
    );
}

#[test]
fn issue_removal_epic_downfall() {
    // Mana value 4 is exiled.
    let mut big = engine(723_006);
    let giant = inject_creature_on_battlefield(&mut big, 1, "hill_giant");
    cast_and_resolve(&mut big, "epic_downfall", target_object(giant));
    assert_eq!(big.state.objects[&giant].zone, Zone::Exile);

    // Mana value 2 is not a legal target.
    let mut small = engine(723_016);
    let bear = inject_creature_on_battlefield(&mut small, 1, "grizzly_bears");
    inject_card_into_hand(&mut small, 0, "epic_downfall");
    let slot = hand_index_for_card(&small, 0, "epic_downfall");
    assert!(
        small
            .apply_command(0, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "mana value 2 is below the 3-or-greater bound"
    );
}

#[test]
fn issue_removal_repel_calamity() {
    // Power 4 alone qualifies.
    let mut power = engine(723_007);
    let brute = inject_creature_with_stats(&mut power, 1, "grizzly_bears", 4, 1);
    cast_and_resolve(&mut power, "repel_calamity", target_object(brute));
    assert_eq!(power.state.objects[&brute].zone, Zone::Graveyard);

    // Toughness 4 alone qualifies.
    let mut tough = engine(723_017);
    let wall = inject_creature_with_stats(&mut tough, 1, "grizzly_bears", 1, 4);
    cast_and_resolve(&mut tough, "repel_calamity", target_object(wall));
    assert_eq!(tough.state.objects[&wall].zone, Zone::Graveyard);

    // A 3/3 qualifies on neither.
    let mut mid = engine(723_027);
    let bear = inject_creature_with_stats(&mut mid, 1, "grizzly_bears", 3, 3);
    inject_card_into_hand(&mut mid, 0, "repel_calamity");
    let slot = hand_index_for_card(&mid, 0, "repel_calamity");
    assert!(
        mid.apply_command(0, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "3/3 is neither power nor toughness 4"
    );
}

#[test]
fn issue_removal_eriettes_lullaby() {
    let mut e = engine(723_008);
    let tapped = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state.objects.get_mut(&tapped).expect("object").tapped = true;
    let life = e.state.players[0].life;
    cast_and_resolve(&mut e, "eriettes_lullaby", target_object(tapped));
    assert_eq!(e.state.objects[&tapped].zone, Zone::Graveyard);
    assert_eq!(
        e.state.players[0].life,
        life + 2,
        "the caster gains two life"
    );

    // An untapped creature is not a legal target.
    let mut untapped = engine(723_018);
    let bear = inject_creature_on_battlefield(&mut untapped, 1, "grizzly_bears");
    inject_card_into_hand(&mut untapped, 0, "eriettes_lullaby");
    let slot = hand_index_for_card(&untapped, 0, "eriettes_lullaby");
    assert!(
        untapped
            .apply_command(0, &cast_spell(slot, target_object(bear)))
            .is_err(),
        "an untapped creature is not a legal target"
    );
}

#[test]
fn issue_removal_grapple_with_death() {
    // An artifact is destroyed and the caster gains one life.
    let mut artifact = engine(723_009);
    let boots = inject_permanent_on_battlefield(&mut artifact, 1, "swiftfoot_boots");
    let life = artifact.state.players[0].life;
    cast_and_resolve(&mut artifact, "grapple_with_death", target_object(boots));
    assert_eq!(artifact.state.objects[&boots].zone, Zone::Graveyard);
    assert_eq!(artifact.state.players[0].life, life + 1);

    // A creature is destroyed too.
    let mut creature = engine(723_019);
    let bear = inject_creature_on_battlefield(&mut creature, 1, "grizzly_bears");
    cast_and_resolve(&mut creature, "grapple_with_death", target_object(bear));
    assert_eq!(creature.state.objects[&bear].zone, Zone::Graveyard);

    // A nonartifact, noncreature permanent is not a legal target.
    let mut land = engine(723_029);
    let forest = inject_permanent_on_battlefield(&mut land, 1, "forest");
    inject_card_into_hand(&mut land, 0, "grapple_with_death");
    let slot = hand_index_for_card(&land, 0, "grapple_with_death");
    assert!(
        land.apply_command(0, &cast_spell(slot, target_object(forest)))
            .is_err(),
        "a land is neither an artifact nor a creature"
    );
}
