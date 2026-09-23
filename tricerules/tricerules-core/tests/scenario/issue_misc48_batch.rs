//! Batch 50: two Standard Instants reuse the chosen-creature pump and power-damage effects.
//! Governed by CR 115, 120.1-120.2b, 608.2b-c, 613.4c, and 702.20.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

fn engine(seed: u64) -> GameEngine {
    let mut e = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast_two_targets(e: &mut GameEngine, player: i32, card: &str, source: u32, target: u32) {
    inject_card_into_hand(e, player as usize, card);
    let slot = hand_index_for_card(e, player as usize, card);
    let mut targets = target_object(source);
    let mut target_ref = target_object(target);
    target_ref[0].group_index = 1;
    targets.extend(target_ref);
    semantic::accepted(e, player, &cast_spell(slot, targets));
}

fn murder_in_response(e: &mut GameEngine, player: i32, target: u32) {
    // The active player receives priority again after casting the candidate spell.
    semantic::accepted(e, 0, &pass());
    inject_card_into_hand(e, player as usize, "murder");
    let slot = hand_index_for_card(e, player as usize, "murder");
    semantic::accepted(e, player, &cast_spell(slot, target_object(target)));
    semantic::complete(e, 16, |_| None).require_exercised();
}

#[test]
fn issue_misc48_rabid_gnaw_uses_the_buffed_power_and_revalidates_both_targets() {
    let mut resolves = engine(948_001);
    let source = inject_creature_with_stats(&mut resolves, 0, "hill_giant", 2, 3);
    let target = inject_creature_with_stats(&mut resolves, 1, "hill_giant", 4, 5);
    cast_two_targets(&mut resolves, 0, "rabid_gnaw", source, target);
    semantic::complete(&mut resolves, 12, |_| None).require_exercised();
    assert_eq!(resolves.effective_power(source), Some(3));
    assert_eq!(resolves.state.objects[&target].damage, 3);
    assert_eq!(resolves.state.objects[&target].zone, Zone::Battlefield);

    // If the first target becomes illegal, the card's ruling says nothing happens,
    // even though the damage target remains a legal creature not controlled by the caster.
    let mut illegal_source = engine(948_002);
    let source = inject_creature_with_stats(&mut illegal_source, 0, "hill_giant", 2, 3);
    let target = inject_creature_with_stats(&mut illegal_source, 1, "hill_giant", 4, 5);
    cast_two_targets(&mut illegal_source, 0, "rabid_gnaw", source, target);
    murder_in_response(&mut illegal_source, 1, source);
    assert_eq!(illegal_source.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(illegal_source.state.objects[&target].damage, 0);

    // If only the damage target becomes illegal, the targeted creature still receives +1/+0.
    let mut illegal_damage_target = engine(948_003);
    let source = inject_creature_with_stats(&mut illegal_damage_target, 0, "hill_giant", 2, 3);
    let target = inject_creature_with_stats(&mut illegal_damage_target, 1, "hill_giant", 4, 5);
    cast_two_targets(&mut illegal_damage_target, 0, "rabid_gnaw", source, target);
    murder_in_response(&mut illegal_damage_target, 1, target);
    assert_eq!(
        illegal_damage_target.state.objects[&target].zone,
        Zone::Graveyard
    );
    assert_eq!(illegal_damage_target.effective_power(source), Some(3));
}

#[test]
fn issue_misc48_diplomatic_relations_grants_vigilance_and_damages_an_opponent_creature() {
    let mut e = engine(948_010);
    let source = inject_creature_with_stats(&mut e, 0, "hill_giant", 2, 3);
    let target = inject_creature_with_stats(&mut e, 1, "hill_giant", 4, 5);
    cast_two_targets(&mut e, 0, "diplomatic_relations", source, target);
    semantic::complete(&mut e, 12, |_| None).require_exercised();
    assert_eq!(e.effective_power(source), Some(3));
    assert!(e.effective_has_keyword(source, Keyword::Vigilance));
    assert_eq!(e.state.objects[&target].damage, 3);

    // The damage target can become illegal independently; the first target still gets both buffs.
    let mut illegal_damage_target = engine(948_011);
    let source = inject_creature_with_stats(&mut illegal_damage_target, 0, "hill_giant", 2, 3);
    let target = inject_creature_with_stats(&mut illegal_damage_target, 1, "hill_giant", 4, 5);
    cast_two_targets(
        &mut illegal_damage_target,
        0,
        "diplomatic_relations",
        source,
        target,
    );
    murder_in_response(&mut illegal_damage_target, 1, target);
    assert_eq!(illegal_damage_target.effective_power(source), Some(3));
    assert!(illegal_damage_target.effective_has_keyword(source, Keyword::Vigilance));
}
