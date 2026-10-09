//! Shared setup for scenarios and registry conformance; resources are not semantic proof.
use super::*;
use tricerules_core::Zone;

/// Seed independently named draws in order; card expectations belong to the caller.
/// Injection is fixture setup, not a simulated rules command or entry-event proof.
pub(crate) fn library_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let objects: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|object_id| !objects.contains(object_id));
    for object_id in objects.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    objects
}

pub(crate) fn game(seed: u64, players: &[i32], card: &str, ability: Option<usize>) -> GameEngine {
    let artifact_return_fixture = matches!(
        (card, ability),
        ("trading_post", Some(2)) | ("goblin_engineer", Some(0)) | ("goblin_welder", Some(0))
    );
    let mut cards = vec![
        card,
        "grizzly_bears",
        "grizzly_bears",
        "grizzly_bears",
        "island",
        "explosive_apparatus",
    ];
    cards.extend(match card {
        "arcane_denial" => Some("opt"),
        "annul" => Some("short_sword"),
        "flashfreeze" => Some("hill_giant"),
        "flusterstorm" => Some("divination"),
        _ => None,
    });
    if card == "decimate" {
        cards.push("ominous_seas");
    }
    if card == "primevals_glorious_rebirth" {
        cards.extend(["ghalta,_primal_hunger", "alhammarrets_archive"]);
    }
    if card == "inventors_fair" {
        cards.extend(["sol_ring", "sol_ring"]);
    }
    if card == "trash_for_treasure" {
        cards.push("mind_stone");
    }
    if card == "kuldotha_forgemaster" && ability == Some(0) {
        cards.push("sol_ring");
    }
    if card == "metalwork_colossus" && ability == Some(0) {
        cards.push("sol_ring");
    }
    if artifact_return_fixture {
        // This ability targets an artifact already in the graveyard before paying its cost.
        cards.push("sol_ring");
    }
    if card == "fanatic_of_rhonas" && ability == Some(1) {
        cards.push("air_elemental");
    }
    let deck = super::deck_with("forest", &cards);
    let mut e = if card == "war_room" {
        // War Room's draw cost is undefined without a declaration. Supply a real commander
        // for this exact card fixture rather than weakening activation legality or baseline.
        let decks = players
            .iter()
            .map(|_| tricerules_core::EngineDeck {
                mainboard: deck.clone(),
                commanders: vec!["kami_of_the_crescent_moon".into()],
            })
            .collect();
        GameEngine::new_with_commander_decks(
            tricerules_cards::registry::global(),
            seed,
            players,
            20,
            Some(decks),
            true,
        )
        .unwrap()
    } else {
        GameEngine::new(
            tricerules_cards::registry::global(),
            seed,
            players,
            20,
            Some(vec![deck; players.len()]),
            true,
        )
        .unwrap()
    };
    super::advance_to_main1_from_game_start(&mut e);
    for player in 0..e.state.players.len() {
        if card == "primevals_glorious_rebirth" && player == 0 {
            // This exact legendary sorcery needs a controlled legendary creature before casting.
            // A separate legendary graveyard permanent exercises a nonempty return.
            super::relocate_to_battlefield(&mut e, player, "ghalta,_primal_hunger", false);
            let dead = super::take_oid_from_library_or_hand(&mut e, player, "alhammarrets_archive");
            e.state.players[player].graveyard.push(dead);
            e.state.objects.get_mut(&dead).unwrap().zone = Zone::Graveyard;
        }
        super::relocate_to_battlefield(&mut e, player, "grizzly_bears", false);
        super::relocate_to_battlefield(&mut e, player, "explosive_apparatus", false);
        if card == "metalwork_colossus" && ability == Some(0) && player == 0 {
            // The graveyard source is not a battlefield sacrifice candidate. Supply both costs.
            super::relocate_to_battlefield(&mut e, player, "sol_ring", false);
        }
        if card == "kuldotha_forgemaster" && ability == Some(0) && player == 0 {
            // The source and Explosive Apparatus supply two of the three artifacts.
            // Complete this exact activation fixture without changing the baseline.
            super::relocate_to_battlefield(&mut e, player, "sol_ring", false);
        }
        if card == "decimate" {
            super::relocate_to_battlefield(&mut e, player, "ominous_seas", false);
        }
        if card == "inventors_fair" {
            // The search activation requires three artifacts before its costs are paid.
            // Explosive Apparatus is already present; add the two missing fixture resources.
            super::relocate_to_battlefield(&mut e, player, "sol_ring", false);
            super::relocate_to_battlefield(&mut e, player, "sol_ring", false);
        }
        super::relocate_to_hand(&mut e, player, "grizzly_bears");
        super::relocate_to_battlefield(&mut e, player, "forest", false);
        super::relocate_to_battlefield(&mut e, player, "island", false);
        let dead = super::take_oid_from_library_or_hand(&mut e, player, "grizzly_bears");
        e.state.players[player].graveyard.push(dead);
        e.state.objects.get_mut(&dead).unwrap().zone = tricerules_core::Zone::Graveyard;
        if card == "trash_for_treasure" {
            // A separate graveyard artifact is required before paying the sacrifice cost.
            let dead = super::take_oid_from_library_or_hand(&mut e, player, "mind_stone");
            e.state.players[player].graveyard.push(dead);
            e.state.objects.get_mut(&dead).unwrap().zone = tricerules_core::Zone::Graveyard;
        }
        if artifact_return_fixture && player == 0 {
            let dead = super::take_oid_from_library_or_hand(&mut e, player, "sol_ring");
            e.state.players[player].graveyard.push(dead);
            e.state.objects.get_mut(&dead).unwrap().zone = tricerules_core::Zone::Graveyard;
        }
        super::grant_pool(&mut e, player);
        if card == "metalwork_colossus" && ability.is_none() && player == 0 {
            // Generic payment uses colorless for generic costs. Nine is insufficient for
            // this eleven-mana creature's ten-mana cost with the one-MV Apparatus fixture.
            e.state.players[player].mana_pool.colorless = 11;
        }
    }
    if card == "fanatic_of_rhonas" && ability == Some(1) {
        // Ferocious needs a controlled creature with current power at least four.
        super::relocate_to_battlefield(&mut e, 0, "air_elemental", false);
    }
    if card == "maze_of_ith" && ability == Some(0) {
        // Supply a legally declared attacker; do not weaken the target or baseline.
        let actor = e.state.active_player_id();
        e.apply_command(actor, &super::primitive_yield()).unwrap();
        super::pass_priority_round(&mut e);
        let assignment =
            e.initial_response_batch().legal_by_player[&actor].legal_attack_assignments[0];
        e.apply_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(DeclareAttackers {
                    assignments: vec![assignment],
                })),
            },
        )
        .unwrap();
    }
    e
}

/// Existing reviewed conformance source setup. Other zone abilities need explicit fixtures.
pub(crate) fn ability_source(
    e: &mut GameEngine,
    player: usize,
    card: &str,
    face: usize,
    ability: usize,
) -> u32 {
    let oid = if card == "boseiju,_who_endures" && ability == 1 {
        // Channel discards its hand source; its target/search use the existing fixture resources.
        super::relocate_to_hand(e, player, card)
    } else if (card == "fanatic_of_rhonas" && ability == 2)
        || (card == "metalwork_colossus" && ability == 0)
    {
        let oid = super::take_oid_from_library_or_hand(e, player, card);
        e.state.players[player].graveyard.push(oid);
        e.state.objects.get_mut(&oid).unwrap().zone = Zone::Graveyard;
        oid
    } else {
        // Select the fixture face before the first battlefield publication seeds its slots.
        // This is initial setup, not a front-face permanent changing faces in play.
        let oid = super::relocate_to_hand(e, player, card);
        e.state.objects.get_mut(&oid).unwrap().face_up_index = face;
        super::relocate_to_battlefield(e, player, card, false)
    };
    e.state.objects.get_mut(&oid).unwrap().face_up_index = face;
    if card == "chandra,_novice_pyromancer" {
        // Direct relocation skips entry. This is the printed starting loyalty, not an expectation.
        e.state
            .objects
            .get_mut(&oid)
            .unwrap()
            .set_counter(tricerules_cards::CounterKind::Loyalty, 5);
    }
    if card == "jace,_wielder_of_mysteries" {
        // Direct relocation skips intrinsic entry; the ultimate needs eight loyalty.
        // Paid-card scenarios separately prove entry at four and rejection below eight.
        e.state.objects.get_mut(&oid).unwrap().set_counter(
            tricerules_cards::CounterKind::Loyalty,
            if ability == 1 { 8 } else { 4 },
        );
    }
    if card == "pentad_prism" {
        // Direct relocation skips casting/entry. Dedicated paid scenarios prove Sunburst.
        e.state
            .objects
            .get_mut(&oid)
            .unwrap()
            .set_counter(tricerules_cards::CounterKind::Charge, 2);
    }
    if card == "pentavus" {
        // Direct relocation skips entry. Actual-card scenarios independently prove entry.
        e.state
            .objects
            .get_mut(&oid)
            .unwrap()
            .set_counter(tricerules_cards::CounterKind::PlusOnePlusOne, 5);
        if ability == 1 {
            // Any controlled Pentavite creature pays, including a real non-token creature.
            let payment = e.state.players[player]
                .battlefield
                .iter()
                .copied()
                .find(|id| e.state.objects[id].card_id == "grizzly_bears")
                .expect("existing conformance creature resource");
            e.state
                .continuous_effects
                .push(tricerules_core::ContinuousEffect {
                    trigger_grant_origin: None,
                    source_id: None,
                    affected: tricerules_core::AffectedScope::Single(payment),
                    kind: tricerules_cards::ContinuousEffectKind::Layer4SetCreatureTypes(vec![
                        "Pentavite".into(),
                    ]),
                    condition: None,
                    duration: tricerules_cards::EffectDuration::Indefinite,
                    timestamp: e.state.command_index,
                });
        }
    }
    oid
}
