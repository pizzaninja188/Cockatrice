//! Actual-card scenarios for the reviewed `Destroy target artifact.` family.
//!
//! Expectations are printed Oracle values. CR 115.1a and 601.2c require one
//! artifact target as the spell is cast; CR 608.2b leaves a stale target
//! alone; CR 701.8 destroys the permanent.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "shatter",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "smelt",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
        },
        Case {
            id: "verdigris",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
        },
    ]
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast_at(e: &mut GameEngine, case: &Case, target: u32) -> Result<u32, String> {
    ensure_in_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .map_err(|error| error.to_string())?;
    assert_eq!(e.state.stack.len(), 1, "{}: spell is on the stack", case.id);
    assert_eq!(
        e.state.objects[&target].zone,
        Zone::Battlefield,
        "{}: casting does not destroy the target",
        case.id
    );
    Ok(e.state.stack[0].id)
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

#[test]
fn destroy_artifact_batch_cards_destroy_an_artifact() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(920_000 + index as u64, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        cast_at(&mut e, &case, artifact).expect("cast at the artifact");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Graveyard,
            "{}: artifact destroyed",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&artifact),
            "{}: destroyed artifact is in its owner's graveyard",
            case.id
        );
        assert!(
            !e.state.players[0].graveyard.contains(&artifact),
            "{}: destroyed artifact is not in the caster's graveyard",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn destroy_artifact_batch_cards_reject_nonartifact_targets_without_spending_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(921_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let before = pool(&e);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
                .is_err(),
            "{}: a creature is not an artifact target",
            case.id
        );
        assert_eq!(
            pool(&e),
            before,
            "{}: rejected cast spends no mana",
            case.id
        );
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn destroy_artifact_batch_cards_fizzle_stale_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(922_000 + index as u64, case.id);
        let artifact = inject_permanent_on_battlefield(&mut e, 1, "bonesplitter");
        let spell = cast_at(&mut e, &case, artifact).expect("cast at the artifact");
        e.state.players[1]
            .battlefield
            .retain(|oid| *oid != artifact);
        e.state.objects.get_mut(&artifact).expect("artifact").zone = Zone::Exile;
        e.state.players[1].exile.push(artifact);
        *e.state.zone_change_generation.entry(artifact).or_insert(0) += 1;
        pass_both_players(&mut e);
        assert_eq!(e.state.objects[&artifact].zone, Zone::Exile, "{}", case.id);
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the spell did not resolve and went to its owner's graveyard",
            case.id
        );
        assert!(e.state.players[0].graveyard.contains(&spell), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
