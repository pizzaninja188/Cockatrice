//! Actual-card scenarios for "Regenerate target creature." instants (issue #523): Death Ward,
//! Regenerate and Mending Touch.
//!
//! CR 701.19a-b: the spell creates a regeneration shield on the chosen creature; the next time
//! that creature would be destroyed this turn, it is instead tapped, removed from combat and has
//! all damage removed. The shield lives on the creature, so it outlasts the spell that made it,
//! is used up by one destruction and expires at cleanup (CR 514.2). CR 701.19c: "can't be
//! regenerated" ignores it. CR 608.2b: with its sole target gone, the spell does nothing.
use crate::helpers::ScenarioCommandExt;
use crate::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    /// Printed mana cost as (white, green, generic).
    cost: (u32, u32, u32),
}

impl Case {
    fn mana(&self) -> ManaGift {
        let (w, g, c) = self.cost;
        ManaGift {
            w,
            g,
            c,
            ..Default::default()
        }
    }
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "death_ward",
            cost: (1, 0, 0),
        },
        Case {
            id: "regenerate",
            cost: (0, 1, 1),
        },
        Case {
            id: "mending_touch",
            cost: (0, 1, 0),
        },
    ]
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn engine(seed: u64) -> GameEngine {
    let mut e = engine_at_game_start(seed);
    advance_to_main1_from_game_start(&mut e);
    e
}

fn engine_at_game_start(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "death_ward",
                "regenerate",
                "mending_touch",
                "murder",
                "murder",
                "terminate",
            ],
        ),
        deck_with("swamp", &["murder"]),
    ]);
    GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new")
}

fn shields(e: &GameEngine, oid: u32) -> u32 {
    e.state.objects[&oid].regeneration_shields
}

/// Player 0 casts `card` at `target` and both players pass so it resolves.
fn regenerate_with(e: &mut GameEngine, case: &Case, target: u32) {
    cast_targeted_instant_and_resolve(e, 0, case.id, case.mana(), target_object(target));
    assert!(e.state.stack.is_empty(), "{}: spell resolved", case.id);
}

/// Player 0 casts Murder at `target` and both players pass.
fn murder(e: &mut GameEngine, target: u32) {
    cast_targeted_instant_and_resolve(
        e,
        0,
        "murder",
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
        target_object(target),
    );
}

#[test]
fn issue_523_spell_shield_replaces_exactly_one_destruction() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(523_000 + index as u64);
        // An opponent's creature is a legal target too: "target creature" has no controller limit.
        let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        regenerate_with(&mut e, &case, bears);
        assert_eq!(shields(&e, bears), 1, "{}: exactly one shield", case.id);
        assert!(
            !e.state.objects[&bears].tapped,
            "{}: shield alone does not tap",
            case.id
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 1, bears),
            vec!["Regeneration shield"],
            "{}",
            case.id
        );
        let spell_in_graveyard = e.state.players[0]
            .graveyard
            .iter()
            .any(|oid| e.state.objects[oid].card_id == case.id);
        assert!(
            spell_in_graveyard,
            "{}: resolved instant goes to graveyard",
            case.id
        );

        e.state.objects.get_mut(&bears).expect("bears").damage = 1;
        murder(&mut e, bears);
        let object = &e.state.objects[&bears];
        assert_eq!(object.zone, Zone::Battlefield, "{}: regenerated", case.id);
        assert!(object.tapped, "{}: regeneration taps", case.id);
        assert_eq!(object.damage, 0, "{}: regeneration clears damage", case.id);
        assert_eq!(
            object.regeneration_shields, 0,
            "{}: shield used up",
            case.id
        );

        murder(&mut e, bears);
        assert_eq!(
            e.state.objects[&bears].zone,
            Zone::Graveyard,
            "{}: a second destruction this turn is not replaced",
            case.id
        );
    }
}

#[test]
fn issue_523_spell_shield_saves_from_lethal_damage_and_removes_from_combat() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine_at_game_start(523_100 + index as u64);
        advance_to_declare_attackers(&mut e);
        let attacker = battlefield_object_for_card(&e, 0, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        assert!(e
            .state
            .combat
            .as_ref()
            .is_some_and(|combat| combat.attacking.contains(&attacker)));
        regenerate_with(&mut e, &case, attacker);
        assert_eq!(shields(&e, attacker), 1, "{}", case.id);

        // Lethal damage: the state-based destruction (CR 704.5g) is replaced.
        e.state.objects.get_mut(&attacker).expect("bears").damage = 2;
        pass_both_players(&mut e);
        let object = &e.state.objects[&attacker];
        assert_eq!(object.zone, Zone::Battlefield, "{}", case.id);
        assert_eq!(object.damage, 0, "{}", case.id);
        assert_eq!(object.regeneration_shields, 0, "{}", case.id);
        assert!(object.tapped, "{}", case.id);
        assert!(
            e.state
                .combat
                .as_ref()
                .is_none_or(|combat| !combat.attacking.contains(&attacker)),
            "{}: regeneration removes the creature from combat",
            case.id
        );
    }
}

#[test]
fn issue_523_spell_shield_expires_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(523_200 + index as u64);
        let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        regenerate_with(&mut e, &case, bears);
        assert_eq!(shields(&e, bears), 1, "{}", case.id);
        end_active_turn(&mut e, 0);
        assert_eq!(
            shields(&e, bears),
            0,
            "{}: shield lasts only this turn",
            case.id
        );
    }
}

#[test]
fn issue_523_cant_be_regenerated_ignores_the_spell_shield() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(523_300 + index as u64);
        let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        regenerate_with(&mut e, &case, bears);
        cast_targeted_instant_and_resolve(
            &mut e,
            0,
            "terminate",
            ManaGift {
                b: 1,
                r: 1,
                ..Default::default()
            },
            target_object(bears),
        );
        assert_eq!(e.state.objects[&bears].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn issue_523_rejects_noncreature_and_missing_targets_without_spending_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(523_400 + index as u64);
        let land = relocate_to_battlefield(&mut e, 1, "swamp", false);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana());
        let pool_before = pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal target",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, vec![]))
                .is_err(),
            "{}: the target is required",
            case.id
        );
        assert_eq!(pool(&e), pool_before, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(hand_index_for_card(&e, 0, case.id), slot, "{}", case.id);
        assert_eq!(shields(&e, land), 0, "{}", case.id);
    }
}

#[test]
fn issue_523_spell_fizzles_when_its_target_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(523_500 + index as u64);
        let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana());
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(bears)))
            .expect("cast at the creature");
        e.state.players[1].battlefield.retain(|oid| *oid != bears);
        e.state.objects.get_mut(&bears).expect("bears").zone = Zone::Exile;
        e.state.players[1].exile.push(bears);
        *e.state.zone_change_generation.entry(bears).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            shields(&e, bears),
            0,
            "{}: no shield on the stale object",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
