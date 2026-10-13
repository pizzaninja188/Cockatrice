//! Wildsear's hand-enchantment grant and the captured Cascade resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use std::collections::HashSet;
use tricerules_cards::primitives::{SpellEffectKind, SpellKeyword};
use tricerules_core::{EngineDeck, Zone};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    ChoiceKind, DevCommand, DevMoveCard, DevZone, ResolutionChoiceDecision, RuledCommand,
};

const WILDSEAR: &str = "wildsear,_scouring_maw";

const CASCADE_COST_FIXTURE: &str = r#"
(
    id: "wildsear_cascade_cost_fixture",
    name: "Cascade Cost Fixture",
    face_id: "wildsear_cascade_cost_fixture",
    mana_cost: "{3}",
    flashback_cost: Some("{0}"),
    types: ["Enchantment"],
    additional_costs: [SacrificePermanent(filter: (kind: Creature, controller: You))],
    spell_keywords: [Cascade],
)
"#;

const CASCADE_ADVENTURE_FACE_FIXTURE: &str = r#"
(
    id: "cascade_adventure_face_fixture",
    name: "Cascade Adventure Face Fixture",
    layout: Adventure,
    faces: [
        (
            name: "Low Trail",
            face_id: "cascade_adventure_face_fixture_low",
            mana_cost: "{1}{G}",
            types: ["Creature", "Beast"],
            power: 2,
            toughness: 2,
        ),
        (
            name: "High Trail",
            face_id: "cascade_adventure_face_fixture_high",
            mana_cost: "{3}{G}",
            types: ["Instant"],
            spell_effect: [Draw(who: Controller, count: 1)],
        ),
    ],
)
"#;

const NESTED_CASCADE_HIT_FIXTURE: &str = r#"
(
    id: "nested_cascade_hit_fixture",
    name: "Nested Cascade Hit Fixture",
    face_id: "nested_cascade_hit_fixture",
    mana_cost: "{2}{G}",
    types: ["Instant"],
    spell_keywords: [Cascade],
)
"#;

const HIGH_CASCADE_ENCHANTMENT_FIXTURE: &str = r#"
(
    id: "high_cascade_enchantment_fixture",
    name: "High Cascade Enchantment Fixture",
    face_id: "high_cascade_enchantment_fixture",
    mana_cost: "{4}{R}{G}",
    types: ["Enchantment"],
)
"#;

const CASCADE_GRANT_FIXTURE: &str = r#"
(
    id: "cascade_grant_fixture",
    name: "Cascade Grant Fixture",
    face_id: "cascade_grant_fixture",
    types: ["Creature", "Druid"],
    power: 1,
    toughness: 1,
    static_abilities: [(
        ability_id: "fixture_cascade",
        presentation: Fallback,
        definition: GrantSpellKeyword(
            filter: (card_type: Some(Enchantment), origin: Some(Hand)),
            keyword: Cascade,
        ),
    )],
)
"#;

fn wildsear_fixture_registry() -> &'static tricerules_cards::CardRegistry {
    Box::leak(Box::new(
        tricerules_cards::CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/forest.ron"),
                include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
                include_str!("../../../tricerules-cards/data/garruks_uprising.ron"),
                include_str!("../../../tricerules-cards/data/opt.ron"),
                include_str!("../../../tricerules-cards/data/counterspell.ron"),
                include_str!("../../../tricerules-cards/data/wildsear,_scouring_maw.ron"),
                CASCADE_COST_FIXTURE,
                CASCADE_ADVENTURE_FACE_FIXTURE,
                NESTED_CASCADE_HIT_FIXTURE,
                HIGH_CASCADE_ENCHANTMENT_FIXTURE,
                CASCADE_GRANT_FIXTURE,
            ],
            &[],
        )
        .expect("Wildsear rule fixtures are valid card data"),
    ))
}

fn wildsear_fixture_engine(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        wildsear_fixture_registry(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new Wildsear fixture game");
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    engine
}

fn spell_keyword_origins(engine: &GameEngine) -> Vec<tricerules_core::state::SpellKeywordOrigin> {
    engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("multiple Cascade instances require CR 603.3b ordering")
        .candidates
        .iter()
        .map(|trigger| match trigger.ability_origin.as_ref() {
            Some(tricerules_core::state::TriggerAbilityOrigin::SpellKeyword(origin)) => {
                origin.clone()
            }
            other => panic!("spell keyword trigger retains its origin: {other:?}"),
        })
        .collect()
}

fn stack_target(object_id: u32) -> Vec<rv1::TargetRef> {
    vec![rv1::TargetRef {
        object_id,
        kind: rv1::TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

fn wildsear_engine(seed: u64) -> GameEngine {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck.clone(), deck]),
        true,
    )
    .expect("new Wildsear scenario game");
    advance_to_main1_from_game_start(&mut engine);
    engine.enable_dev_commands();
    engine
}

fn set_library_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let object_ids = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect::<Vec<_>>();
    engine.state.players[player]
        .library
        .retain(|object_id| !object_ids.contains(object_id));
    for object_id in object_ids.iter().rev() {
        engine.state.players[player].library.push_front(*object_id);
    }
    object_ids
}

fn move_wildsear_to_graveyard(engine: &mut GameEngine, player: i32) {
    let command = RuledCommand {
        cmd: Some(tricerules_proto::ruled::v1::ruled_command::Cmd::DevCommand(
            DevCommand {
                target_player_id: player,
                dev: Some(Dev::MoveCard(DevMoveCard {
                    card_name: "Wildsear, Scouring Maw".into(),
                    zone: DevZone::Graveyard as i32,
                    ready: false,
                })),
            },
        )),
    };
    engine
        .apply_scenario_command(player, &command)
        .expect("remove Wildsear after the enchantment cast commits");
}

fn cascade_cast_announcement(
    engine: &mut GameEngine,
    player: i32,
    object_id: u32,
) -> rv1::SpellCastAnnouncement {
    let generation = engine.state.zone_change_generation[&object_id];
    let action = engine.initial_response_batch().legal_by_player[&player]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == object_id)
        .cloned()
        .expect("Cascade publishes the qualifying exile spell");
    rv1::SpellCastAnnouncement {
        face_index: action.face_index,
        source: Some(exile_cast_source(object_id, generation)),
        cast_method: action.cast_method,
        casting_permission_id: action.casting_permission_id,
        ..Default::default()
    }
}

#[test]
fn wildsear_grants_cascade_to_hand_enchantments_and_decline_random_bottoms_the_exact_exile_prefix()
{
    let mut engine = wildsear_engine(20_261_012);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    let _wildsear = inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let cohort = set_library_top(
        &mut engine,
        caster_index,
        &["forest", "garruks_uprising", "opt"],
    );
    let initial_library_len = engine.state.players[caster_index].library.len();
    let initial_generations = cohort
        .iter()
        .map(|object_id| {
            (
                *object_id,
                engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect::<Vec<_>>();

    inject_card_into_hand(&mut engine, caster_index, "garruks_uprising");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            g: 3,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, caster_index, "garruks_uprising");
    engine
        .apply_scenario_command(caster, &cast_spell(hand_index, Vec::new()))
        .expect("cast Garruk's Uprising from hand");

    let cascade = engine
        .state
        .stack
        .iter()
        .find(|item| item.ability_text.as_deref() == Some("Cascade"))
        .expect("Wildsear grants a separate Cascade trigger");
    assert_eq!(
        cascade.triggered_ability.as_ref().unwrap().effect,
        [SpellEffectKind::Cascade {
            source_mana_value: 3
        }]
    );

    // The trigger belongs to the committed spell; removing its grant source does not erase it.
    move_wildsear_to_graveyard(&mut engine, caster);
    assert_eq!(
        engine.state.objects[&_wildsear].zone,
        Zone::Graveyard,
        "the static grant source has left the battlefield"
    );

    let first = engine.state.priority_player_id();
    engine
        .apply_scenario_command(first, &pass())
        .expect("first priority pass");
    let second = engine.state.priority_player_id();
    let cascade_batch = engine
        .apply_scenario_command(second, &pass())
        .expect("resolve the captured Cascade trigger");
    let reveal = cascade_batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(tricerules_proto::ruled::v1::ruled_event::Ev::CardsRevealed(reveal)) => {
                Some(reveal)
            }
            _ => None,
        })
        .expect("Cascade publicly reveals the exact prefix");
    assert_eq!(reveal.zone_owner_player_id, caster);
    assert_eq!(
        reveal
            .cards
            .iter()
            .map(|card| card.object_id)
            .collect::<Vec<_>>(),
        cohort
    );

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Cascade offers its last qualifying card");
    assert_eq!(pending.deciding_player, caster);
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
    assert_eq!(pending.presentation.candidates, [*cohort.last().unwrap()]);
    let tricerules_core::state::ResolutionContinuation::SpecialCast {
        completion:
            tricerules_core::state::SpecialCastCompletion::Cascade {
                library_owner,
                found,
                cohort: bound_cohort,
            },
        ..
    } = &pending.continuation
    else {
        panic!("the offer retains Cascade's exact exile cohort");
    };
    assert_eq!(*library_owner, caster);
    assert_eq!(found.unwrap().object_id, *cohort.last().unwrap());
    assert_eq!(
        bound_cohort
            .iter()
            .map(|reference| reference.object_id)
            .collect::<Vec<_>>(),
        cohort
    );

    engine
        .apply_scenario_command(
            caster,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the optional Cascade cast");
    assert!(
        engine.state.pending_resolution.is_none(),
        "pending after the special cast: {:?}",
        engine.state.pending_resolution.as_ref().map(|pending| (
            pending.presentation.prompt.as_str(),
            pending.presentation.choice_kind
        ))
    );
    assert_eq!(
        engine.state.players[caster_index].library.len(),
        initial_library_len
    );
    let randomized_bottom = engine.state.players[caster_index]
        .library
        .iter()
        .rev()
        .take(cohort.len())
        .copied()
        .collect::<HashSet<_>>();
    assert_eq!(randomized_bottom, cohort.iter().copied().collect());
    for (object_id, before_generation) in initial_generations {
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Library);
        assert!(
            engine.state.zone_change_generation[&object_id] > before_generation,
            "the returned card is a new zone incarnation"
        );
    }
}

#[test]
fn wildsear_does_not_grant_cascade_to_hand_cast_instants() {
    let mut engine = wildsear_engine(20_261_013);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    inject_card_into_hand(&mut engine, caster_index, "opt");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, caster_index, "opt");
    engine
        .apply_scenario_command(caster, &cast_spell(hand_index, Vec::new()))
        .expect("cast Opt from hand");

    assert_eq!(engine.state.stack.len(), 1);
    assert!(engine.state.stack[0].ability_text.is_none());
}

#[test]
fn wildsear_cascade_without_a_qualifying_card_bottoms_the_entire_revealed_prefix() {
    let mut engine = wildsear_engine(20_261_015);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let original_library = engine.state.players[caster_index]
        .library
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let original_library_len = engine.state.players[caster_index].library.len();
    let original_generations = original_library
        .iter()
        .map(|object_id| {
            (
                *object_id,
                engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0),
            )
        })
        .collect::<Vec<_>>();
    inject_card_into_hand(&mut engine, caster_index, "garruks_uprising");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            g: 3,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, caster_index, "garruks_uprising");
    engine
        .apply_scenario_command(caster, &cast_spell(hand_index, Vec::new()))
        .expect("cast Garruk's Uprising from hand");
    let first = engine.state.priority_player_id();
    engine
        .apply_scenario_command(first, &pass())
        .expect("first priority pass");
    let second = engine.state.priority_player_id();
    engine
        .apply_scenario_command(second, &pass())
        .expect("resolve Cascade without a qualifying nonland card");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.players[caster_index].library.len(),
        original_library_len
    );
    assert_eq!(
        engine.state.players[caster_index]
            .library
            .iter()
            .copied()
            .collect::<HashSet<_>>(),
        original_library
    );
    for (object_id, before_generation) in original_generations {
        assert_eq!(engine.state.objects[&object_id].zone, Zone::Library);
        assert!(
            engine.state.zone_change_generation[&object_id] > before_generation,
            "an uncast card returns as a new library incarnation"
        );
    }
}

#[test]
fn wildsear_cascade_casts_the_hit_for_free_and_bottoms_only_the_uncast_prefix() {
    let mut engine = wildsear_engine(20_261_014);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let [prefix, found] = set_library_top(&mut engine, caster_index, &["forest", "opt"])
        .try_into()
        .expect("two-card Cascade prefix");
    let initial_library_len = engine.state.players[caster_index].library.len();
    inject_card_into_hand(&mut engine, caster_index, "garruks_uprising");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            g: 3,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, caster_index, "garruks_uprising");
    engine
        .apply_scenario_command(caster, &cast_spell(hand_index, Vec::new()))
        .expect("cast Garruk's Uprising from hand");
    let first = engine.state.priority_player_id();
    engine
        .apply_scenario_command(first, &pass())
        .expect("first priority pass");
    let second = engine.state.priority_player_id();
    engine
        .apply_scenario_command(second, &pass())
        .expect("resolve the Cascade trigger");

    let announcement = cascade_cast_announcement(&mut engine, caster, found);
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: 0,
                        decision: ResolutionChoiceDecision::CastSpell as i32,
                        spell_cast_announcement: Some(announcement),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("cast Opt from exile without paying its mana cost");

    let canceled_transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("Cascade cast begins a payment transaction")
        .transaction_id;
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CancelSpellCast(
                    rv1::CancelSpellCast {
                        transaction_id: canceled_transaction_id,
                    },
                )),
            },
        )
        .expect("canceling the proposal reopens the Cascade offer");
    assert!(engine.state.pending_spell_cast.is_none());
    assert_eq!(engine.state.objects[&found].zone, Zone::Exile);
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| &pending.continuation),
        Some(tricerules_core::state::ResolutionContinuation::SpecialCast { .. })
    ));

    let retry_announcement = cascade_cast_announcement(&mut engine, caster, found);
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: 0,
                        decision: ResolutionChoiceDecision::CastSpell as i32,
                        spell_cast_announcement: Some(retry_announcement),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("retry the offered Cascade cast after canceling its first proposal");

    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("retry begins a fresh payment transaction")
        .transaction_id;
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("commit the free Cascade cast");

    assert!(
        engine.state.pending_resolution.is_none(),
        "pending after the special cast: {:?}",
        engine.state.pending_resolution.as_ref().map(|pending| (
            pending.presentation.prompt.as_str(),
            pending.presentation.choice_kind
        ))
    );
    assert_eq!(engine.state.objects[&found].zone, Zone::Stack);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == found && item.cast_by == Some(caster)));
    assert_eq!(engine.state.objects[&prefix].zone, Zone::Library);
    assert_eq!(
        engine.state.players[caster_index].library.len(),
        initial_library_len - 1
    );
    assert_eq!(
        engine.state.players[caster_index].library.back(),
        Some(&prefix),
        "the uncast land prefix is placed on the library bottom"
    );
}

#[test]
fn wildsear_cascade_instances_keep_printed_and_exact_grant_provenance_after_additional_costs() {
    let mut engine = wildsear_fixture_engine(20_261_016);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    let first_wildsear = inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let second_grant =
        inject_creature_on_battlefield(&mut engine, caster_index, "cascade_grant_fixture");
    let sacrifice = inject_creature_on_battlefield(&mut engine, caster_index, "grizzly_bears");
    inject_card_into_hand(&mut engine, caster_index, "wildsear_cascade_cost_fixture");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, caster_index, "wildsear_cascade_cost_fixture");
    engine
        .apply_scenario_command(
            caster,
            &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, sacrifice)]),
        )
        .expect("pay the fixture enchantment's mandatory additional sacrifice");

    let origins = spell_keyword_origins(&engine);
    assert_eq!(origins.len(), 3);
    assert_eq!(
        origins
            .iter()
            .filter(|origin| matches!(
                origin,
                tricerules_core::state::SpellKeywordOrigin::Printed {
                    keyword: SpellKeyword::Cascade,
                    ..
                }
            ))
            .count(),
        1,
        "the spell's printed Cascade is one distinct occurrence"
    );

    let grants = origins
        .iter()
        .filter_map(|origin| match origin {
            tricerules_core::state::SpellKeywordOrigin::StaticGrant {
                keyword: SpellKeyword::Cascade,
                source_id,
                source_zone_change,
                definition,
            } => Some((*source_id, *source_zone_change, definition)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(grants.len(), 2);
    assert_eq!(
        grants
            .iter()
            .map(|(source_id, _, _)| *source_id)
            .collect::<HashSet<_>>(),
        [first_wildsear, second_grant].into_iter().collect()
    );
    for (source_id, generation, definition) in grants {
        assert_eq!(
            generation,
            engine
                .state
                .zone_change_generation
                .get(&source_id)
                .copied()
                .unwrap_or(0),
            "each granted instance retains the exact source incarnation"
        );
        assert_eq!(definition.ability_path.len(), 1);
        match source_id {
            id if id == first_wildsear => {
                assert_eq!(definition.card_id, WILDSEAR);
                assert_eq!(definition.ability_path[0].as_str(), "wildsear_cascade");
            }
            id if id == second_grant => {
                assert_eq!(definition.card_id, "cascade_grant_fixture");
                assert_eq!(definition.ability_path[0].as_str(), "fixture_cascade");
            }
            other => panic!("unexpected static grant source {other}"),
        }
    }
    assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Graveyard);
    assert!(engine
        .state
        .pending_trigger_order
        .as_ref()
        .unwrap()
        .candidates
        .iter()
        .all(|trigger| trigger.ability.effect
            == [SpellEffectKind::Cascade {
                source_mana_value: 3
            }]));
}

#[test]
fn wildsear_cascade_grant_ends_when_its_source_is_paid_as_an_additional_cost() {
    let mut engine = wildsear_fixture_engine(20_261_017);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    let sacrificed_wildsear = inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let surviving_grant =
        inject_creature_on_battlefield(&mut engine, caster_index, "cascade_grant_fixture");
    inject_card_into_hand(&mut engine, caster_index, "wildsear_cascade_cost_fixture");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, caster_index, "wildsear_cascade_cost_fixture");
    engine
        .apply_scenario_command(
            caster,
            &cast_spell_with_costs(
                slot,
                vec![],
                vec![permanent_cost_selection(0, sacrificed_wildsear)],
            ),
        )
        .expect("sacrifice one Wildsear while casting the enchantment");

    let origins = spell_keyword_origins(&engine);
    assert_eq!(origins.len(), 2, "one printed and one live-granted Cascade");
    assert!(origins.iter().any(|origin| matches!(
        origin,
        tricerules_core::state::SpellKeywordOrigin::Printed {
            keyword: SpellKeyword::Cascade,
            ..
        }
    )));
    let grants = origins
        .iter()
        .filter_map(|origin| match origin {
            tricerules_core::state::SpellKeywordOrigin::StaticGrant { source_id, .. } => {
                Some(*source_id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(grants, vec![surviving_grant]);
    assert!(!grants.contains(&sacrificed_wildsear));
    assert_eq!(
        engine.state.objects[&sacrificed_wildsear].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&surviving_grant].zone,
        Zone::Battlefield
    );
}

#[test]
fn wildsear_does_not_grant_cascade_to_an_enchantment_cast_from_the_graveyard() {
    let mut engine = wildsear_fixture_engine(20_261_018);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let sacrifice = inject_creature_on_battlefield(&mut engine, caster_index, "grizzly_bears");
    let card = inject_card_into_hand(&mut engine, caster_index, "wildsear_cascade_cost_fixture");
    engine.state.players[caster_index]
        .hand
        .retain(|object_id| *object_id != card);
    engine.state.players[caster_index].graveyard.push(card);
    engine.state.objects.get_mut(&card).unwrap().zone = Zone::Graveyard;
    let generation = engine.state.zone_change_generation.entry(card).or_default();
    *generation += 1;
    let generation = *generation;

    let command = RuledCommand {
        cmd: Some(rv1::ruled_command::Cmd::CastSpell(rv1::CastSpell {
            cast_method: rv1::CastMethod::Flashback as i32,
            source: Some(graveyard_cast_source(card, generation)),
            cost_selections: vec![permanent_cost_selection(0, sacrifice)],
            ..Default::default()
        })),
    };
    engine
        .apply_scenario_command(caster, &command)
        .expect("cast the enchantment from its graveyard by flashback");

    let cascade_triggers = engine
        .state
        .stack
        .iter()
        .filter(|item| item.ability_text.as_deref() == Some("Cascade"))
        .collect::<Vec<_>>();
    assert_eq!(cascade_triggers.len(), 1, "only printed Cascade applies");
    assert!(matches!(
        cascade_triggers[0]
            .trigger_use_key
            .as_ref()
            .map(|key| &key.ability_origin),
        Some(tricerules_core::state::TriggerAbilityOrigin::SpellKeyword(
            tricerules_core::state::SpellKeywordOrigin::Printed {
                keyword: SpellKeyword::Cascade,
                instance_index: 0,
            }
        ))
    ));
    assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Graveyard);
}

#[test]
fn wildsear_trigger_resolves_after_its_source_enchantment_is_countered() {
    let mut engine = wildsear_engine(20_261_019);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let [prefix, hit] = set_library_top(&mut engine, caster_index, &["forest", "opt"])
        .try_into()
        .expect("Cascade prefix and qualifying spell");
    let source = inject_card_into_hand(&mut engine, caster_index, "garruks_uprising");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let source_slot = hand_index_for_card(&engine, caster_index, "garruks_uprising");
    engine
        .apply_scenario_command(caster, &cast_spell(source_slot, vec![]))
        .expect("cast the enchantment that receives Cascade");
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.id == source && item.ability_text.is_none()));
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.ability_text.as_deref() == Some("Cascade")));

    engine
        .apply_scenario_command(engine.state.priority_player_id(), &pass())
        .expect("pass priority to the opponent");
    let counterspell = inject_card_into_hand(&mut engine, 1, "counterspell");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let counter_slot = hand_index_for_card(&engine, 1, "counterspell");
    engine
        .apply_scenario_command(1, &cast_spell(counter_slot, stack_target(source)))
        .expect("counter the source spell while its Cascade trigger waits on the stack");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&counterspell].zone, Zone::Graveyard);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.ability_text.as_deref() == Some("Cascade")));

    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("captured Cascade resolves after the source spell is gone");
    assert_eq!(pending.presentation.candidates, [hit]);
    assert_eq!(engine.state.objects[&prefix].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&hit].zone, Zone::Exile);
}

#[test]
fn wildsear_cascade_restricts_adventure_faces_by_the_committed_spells_mana_value() {
    let mut engine = wildsear_fixture_engine(20_261_020);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let [adventure] = set_library_top(
        &mut engine,
        caster_index,
        &["cascade_adventure_face_fixture"],
    )
    .try_into()
    .expect("Adventure is the Cascade hit");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut engine, caster_index, "garruks_uprising");
    let source_slot = hand_index_for_card(&engine, caster_index, "garruks_uprising");
    engine
        .apply_scenario_command(caster, &cast_spell(source_slot, vec![]))
        .expect("cast the three-mana enchantment source");
    pass_priority_round(&mut engine);

    let actions = engine.initial_response_batch().legal_by_player[&caster]
        .zone_cast_actions
        .iter()
        .filter(|action| action.object_id == adventure)
        .map(|action| action.face_index)
        .collect::<Vec<_>>();
    assert_eq!(
        actions,
        [0],
        "the front face has mana value 2; the Adventure face has mana value 4"
    );
}

#[test]
fn wildsear_cascade_bottoms_a_hit_when_its_mandatory_additional_cost_cannot_be_paid() {
    let mut engine = wildsear_fixture_engine(20_261_021);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let hit = set_library_top(
        &mut engine,
        caster_index,
        &["wildsear_cascade_cost_fixture"],
    )[0];
    inject_card_into_hand(
        &mut engine,
        caster_index,
        "high_cascade_enchantment_fixture",
    );
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            r: 1,
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    let source_slot =
        hand_index_for_card(&engine, caster_index, "high_cascade_enchantment_fixture");
    engine
        .apply_scenario_command(caster, &cast_spell(source_slot, vec![]))
        .expect("cast the high-value enchantment from hand");
    move_wildsear_to_graveyard(&mut engine, caster);
    let first = engine.state.priority_player_id();
    engine
        .apply_scenario_command(first, &pass())
        .expect("first priority pass");
    let second = engine.state.priority_player_id();
    let batch = engine
        .apply_scenario_command(second, &pass())
        .expect("resolve Cascade with no payable additional cost");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&hit].zone, Zone::Library);
    assert!(engine.state.players[caster_index].library.contains(&hit));
    assert!(batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(tricerules_proto::ruled::v1::ruled_event::Ev::Log(log))
            if log.text.contains("Cascade") && log.text.contains("bottom")
    )));
}

#[test]
fn wildsear_cascade_free_cast_still_pays_a_mandatory_additional_cost() {
    let mut engine = wildsear_fixture_engine(20_261_023);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let sacrifice = inject_creature_on_battlefield(&mut engine, caster_index, "grizzly_bears");
    let [prefix, hit] = set_library_top(
        &mut engine,
        caster_index,
        &["forest", "wildsear_cascade_cost_fixture"],
    )
    .try_into()
    .expect("two-card Cascade prefix");
    let initial_library_len = engine.state.players[caster_index].library.len();
    inject_card_into_hand(
        &mut engine,
        caster_index,
        "high_cascade_enchantment_fixture",
    );
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            r: 1,
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    let source_slot =
        hand_index_for_card(&engine, caster_index, "high_cascade_enchantment_fixture");
    engine
        .apply_scenario_command(caster, &cast_spell(source_slot, vec![]))
        .expect("cast the six-mana enchantment that Wildsear grants Cascade to");
    assert_eq!(
        engine.state.players[caster_index].mana_pool,
        Default::default(),
        "the source spell spends the available mana before Cascade resolves"
    );
    pass_priority_round(&mut engine);

    let mut announcement = cascade_cast_announcement(&mut engine, caster, hit);
    announcement.cost_selections = vec![permanent_cost_selection(0, sacrifice)];
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: 0,
                        decision: ResolutionChoiceDecision::CastSpell as i32,
                        spell_cast_announcement: Some(announcement),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("begin the free Cascade cast while announcing its additional cost");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .expect("the free cast opens the normal two-phase payment transaction")
        .transaction_id;
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("commit the free cast after paying its mandatory sacrifice");

    assert_eq!(engine.state.objects[&sacrifice].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&hit].zone, Zone::Stack);
    assert_eq!(
        engine.state.players[caster_index].mana_pool,
        Default::default()
    );
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.players[caster_index].library.len(),
        initial_library_len - 1
    );
    assert_eq!(
        engine.state.players[caster_index].library.back(),
        Some(&prefix),
        "the uncast prefix is bottomed after the free cast commits"
    );
}

#[test]
fn wildsear_cascade_on_a_cascade_hit_resolves_as_a_nested_trigger() {
    let mut engine = wildsear_fixture_engine(20_261_022);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    let [prefix, nested_hit, nested_found] = set_library_top(
        &mut engine,
        caster_index,
        &["forest", "nested_cascade_hit_fixture", "opt"],
    )
    .try_into()
    .expect("nested Cascade cohort");
    inject_card_into_hand(
        &mut engine,
        caster_index,
        "high_cascade_enchantment_fixture",
    );
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            r: 1,
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    let source_slot =
        hand_index_for_card(&engine, caster_index, "high_cascade_enchantment_fixture");
    engine
        .apply_scenario_command(caster, &cast_spell(source_slot, vec![]))
        .expect("cast the hand enchantment and trigger granted Cascade");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        [nested_hit]
    );
    let announcement = cascade_cast_announcement(&mut engine, caster, nested_hit);
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: 0,
                        decision: ResolutionChoiceDecision::CastSpell as i32,
                        spell_cast_announcement: Some(announcement),
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("cast the nested Cascade hit from exile");
    let transaction_id = engine
        .state
        .pending_spell_cast
        .as_ref()
        .unwrap()
        .transaction_id;
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::CommitSpellCast(
                    rv1::CommitSpellCast {
                        transaction_id,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("commit the free nested Cascade cast");

    let nested_trigger = engine
        .state
        .stack
        .iter()
        .find(|item| item.ability_text.as_deref() == Some("Cascade"))
        .expect("the printed Cascade keyword on the exile hit triggers once");
    assert_eq!(
        nested_trigger.triggered_ability.as_ref().unwrap().effect,
        [SpellEffectKind::Cascade {
            source_mana_value: 3
        }]
    );
    assert!(matches!(
        nested_trigger
            .trigger_use_key
            .as_ref()
            .map(|key| &key.ability_origin),
        Some(tricerules_core::state::TriggerAbilityOrigin::SpellKeyword(
            tricerules_core::state::SpellKeywordOrigin::Printed {
                keyword: SpellKeyword::Cascade,
                instance_index: 0,
            }
        ))
    ));
    assert_eq!(engine.state.objects[&prefix].zone, Zone::Library);
    assert_eq!(engine.state.objects[&nested_hit].zone, Zone::Stack);

    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("nested Cascade independently offers the cheaper library spell");
    assert_eq!(pending.presentation.candidates, [nested_found]);
}

#[test]
fn wildsear_cascade_declined_commander_hit_offers_the_command_zone_replacement() {
    let commander = WILDSEAR.to_owned();
    let deck = deck_with("forest", &[]);
    let decks = Some(vec![
        EngineDeck {
            mainboard: deck.clone(),
            commanders: vec![commander.clone()],
        },
        EngineDeck {
            mainboard: deck,
            commanders: vec![commander],
        },
    ]);
    let mut engine = GameEngine::new_with_commander_decks(
        wildsear_fixture_registry(),
        20_261_023,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new Wildsear Commander fixture game");
    advance_to_main1_from_game_start(&mut engine);
    let caster_index = engine.state.active_player_idx;
    let caster = engine.state.active_player_id();
    let commander = engine.state.players[caster_index].command_zone[0];
    engine.state.players[caster_index]
        .command_zone
        .retain(|object_id| *object_id != commander);
    engine.state.players[caster_index]
        .library
        .push_front(commander);
    engine.state.objects.get_mut(&commander).unwrap().zone = Zone::Library;
    *engine
        .state
        .zone_change_generation
        .entry(commander)
        .or_default() += 1;
    inject_creature_on_battlefield(&mut engine, caster_index, WILDSEAR);
    inject_card_into_hand(
        &mut engine,
        caster_index,
        "high_cascade_enchantment_fixture",
    );
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            r: 1,
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    let source_slot =
        hand_index_for_card(&engine, caster_index, "high_cascade_enchantment_fixture");
    engine
        .apply_scenario_command(caster, &cast_spell(source_slot, vec![]))
        .expect("cast the hand enchantment and receive Cascade");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates,
        [commander]
    );
    engine
        .apply_scenario_command(
            caster,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline casting the commander hit");
    assert!(matches!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .map(|pending| &pending.continuation),
        Some(tricerules_core::state::ResolutionContinuation::SpecialCastCommanderCleanup { .. })
    ));
    engine
        .apply_scenario_command(
            caster,
            &RuledCommand {
                cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                    rv1::SubmitResolutionChoice {
                        resolution_prompt_id: 0,
                        decision: ResolutionChoiceDecision::SelectBranch as i32,
                        selected_branch_index: 0,
                        ..Default::default()
                    },
                )),
            },
        )
        .expect("choose the command-zone replacement for the declined commander");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    assert!(engine.state.players[caster_index]
        .command_zone
        .contains(&commander));
    assert!(engine.state.pending_resolution.is_none());
}
