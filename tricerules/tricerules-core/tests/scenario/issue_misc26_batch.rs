//! Actual-card scenarios for five pinned Standard Food, Treasure, and Clue identities.
//! Exact Scryfall Oracle and rulings checked 2026-09-22. CR 111.10, 601.2, 603.2/603.6a,
//! 613.4c, 702.7/702.10/702.17/702.19, and 701.8 govern the selected behavior.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, RuledCommand, TargetRef};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast(e: &mut GameEngine, player: i32, card_id: &str, targets: Vec<TargetRef>) {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    if e.state.priority_player_id() != player {
        let priority = e.state.priority_player_id();
        semantic::accepted(e, priority, &pass());
    }
    semantic::accepted(e, player, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

fn battlefield_object(e: &GameEngine, player: usize, card_id: &str) -> u32 {
    *e.state.players[player]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on battlefield"))
}

fn generation(e: &GameEngine, object_id: u32) -> u64 {
    e.state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn activate_on(e: &GameEngine, object_id: u32, targets: Vec<TargetRef>) -> RuledCommand {
    let mut cmd = activate_ability_with_costs(object_id, 0, targets, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = cmd.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(e, object_id);
    cmd
}

#[test]
fn issue_misc26_dori_creates_treasure_on_entry_and_has_trample() {
    let mut e = engine(826_001);
    cast(&mut e, 0, "dori,_bearer_of_friends", vec![]);
    let dori = battlefield_object(&e, 0, "dori,_bearer_of_friends");
    assert!(e.effective_has_keyword(dori, Keyword::Trample));
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert_eq!(battlefield_token_oids(&e, 1, "treasure").len(), 0);
}

#[test]
fn issue_misc26_skybeast_tracker_only_rewards_own_five_mana_spell() {
    let mut e = engine(826_002);
    cast(&mut e, 0, "skybeast_tracker", vec![]);
    let tracker = battlefield_object(&e, 0, "skybeast_tracker");
    assert!(e.effective_has_keyword(tracker, Keyword::Reach));
    cast(&mut e, 0, "grizzly_bears", vec![]);
    assert!(
        battlefield_token_oids(&e, 0, "food").is_empty(),
        "mana value 2 does not qualify"
    );
    cast(&mut e, 1, "rowdy_research", vec![]);
    assert!(
        battlefield_token_oids(&e, 0, "food").is_empty(),
        "opponent's mana value 5 spell does not qualify"
    );
    cast(&mut e, 0, "mechan_assembler", vec![]);
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);
    assert!(battlefield_token_oids(&e, 1, "food").is_empty());
}

#[test]
fn issue_misc26_reckless_lackey_sacrifices_as_cost_then_draws_and_makes_treasure() {
    let mut e = engine(826_003);
    cast(&mut e, 0, "reckless_lackey", vec![]);
    let lackey = battlefield_object(&e, 0, "reckless_lackey");
    assert!(e.effective_has_keyword(lackey, Keyword::FirstStrike));
    assert!(e.effective_has_keyword(lackey, Keyword::Haste));
    let before_hand = e.state.players[0].hand.len();
    let activate = activate_on(&e, lackey, vec![]);
    semantic::accepted(&mut e, 0, &activate);
    assert_eq!(
        e.state.objects[&lackey].zone,
        Zone::Graveyard,
        "sacrifice is paid before resolution"
    );
    assert_eq!(e.state.players[0].hand.len(), before_hand);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), before_hand + 1);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
}

#[test]
fn issue_misc26_corsair_captain_buffs_only_other_controlled_pirates() {
    let mut e = engine(826_004);
    let ally = inject_creature_with_stats(&mut e, 0, "reckless_lackey", 1, 2);
    let enemy = inject_creature_with_stats(&mut e, 1, "reckless_lackey", 1, 2);
    cast(&mut e, 0, "corsair_captain", vec![]);
    let captain = battlefield_object(&e, 0, "corsair_captain");
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert_eq!(
        e.effective_power(captain),
        Some(2),
        "Captain excludes itself"
    );
    assert_eq!(e.effective_power(ally), Some(2));
    assert_eq!(e.effective_toughness(ally), Some(3));
    assert_eq!(
        e.effective_power(enemy),
        Some(1),
        "opponent Pirate is outside scope"
    );
    cast(&mut e, 0, "murder", target_object(captain));
    assert_eq!(e.state.objects[&captain].zone, Zone::Graveyard);
    assert_eq!(
        e.effective_power(ally),
        Some(1),
        "anthem ends when Captain leaves"
    );
    assert_eq!(e.effective_toughness(ally), Some(2));
}

#[test]
fn issue_misc26_sharepot_creates_food_then_rejects_land_target_and_destroys_creature() {
    let mut e = engine(826_005);
    cast(&mut e, 0, "bumbleflowers_sharepot", vec![]);
    let sharepot = battlefield_object(&e, 0, "bumbleflowers_sharepot");
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);
    let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
    let bad = activate_on(&e, sharepot, target_object(land));
    assert!(
        e.apply_command(0, &bad).is_err(),
        "land is not a legal target"
    );
    assert_eq!(
        e.state.objects[&sharepot].zone,
        Zone::Battlefield,
        "rejected activation pays no sacrifice cost"
    );
    let victim = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let good = activate_on(&e, sharepot, target_object(victim));
    semantic::accepted(&mut e, 0, &good);
    assert_eq!(
        e.state.objects[&sharepot].zone,
        Zone::Graveyard,
        "the source is a sacrifice cost"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);

    let mut combat = engine(826_015);
    cast(&mut combat, 0, "bumbleflowers_sharepot", vec![]);
    let sharepot = battlefield_object(&combat, 0, "bumbleflowers_sharepot");
    let victim = inject_creature_on_battlefield(&mut combat, 1, "grizzly_bears");
    semantic::accepted(&mut combat, 0, &primitive_yield());
    let late = activate_on(&combat, sharepot, target_object(victim));
    assert!(
        combat.apply_command(0, &late).is_err(),
        "sorcery-only ability is illegal at beginning of combat"
    );
    assert_eq!(combat.state.objects[&sharepot].zone, Zone::Battlefield);
}
