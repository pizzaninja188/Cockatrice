//! Farewell: paid modal casting, sequential untargeted simultaneous exile instructions.
use super::helpers::*;
use tricerules_cards::primitives::{ProtectionQuality, ZoneEventCardinality};
use tricerules_cards::{
    CastTriggerPlayer, Color, ContinuousEffectKind, EffectDuration, Keyword, PermanentTypeFilter,
    TriggerCondition, TypeLineAddition,
};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};

fn setup() -> GameEngine {
    let deck = deck_with("forest", &["farewell"]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_201,
        &[4, 9, 27],
        20,
        Some(vec![deck; 3]),
        true,
    )
    .expect("Farewell registry support");
    advance_to_main1_from_game_start(&mut engine);
    ensure_card_in_hand(&mut engine, 0, "farewell");
    engine
}

fn begin_cast(engine: &mut GameEngine, modes: &[u32]) -> u32 {
    let actor = engine.state.players[0].id;
    engine.state.players[0].mana_pool.colorless = 4;
    engine.state.players[0].mana_pool.white = 2;
    let slot = hand_index_for_card(engine, 0, "farewell");
    let card = engine.state.players[0].hand[slot];
    engine
        .apply_command(
            actor,
            &cast_modal_spell(slot, modes.iter().map(|&i| (i, vec![])).collect()),
        )
        .expect("actual paid Farewell cast");
    card
}

fn resolve_one(engine: &mut GameEngine) -> RuledEventBatch {
    let mut result = Default::default();
    for _ in 0..engine.state.players.len() {
        result = engine
            .apply_command(engine.state.priority_player_id(), &pass())
            .unwrap();
    }
    result
}

fn cast(engine: &mut GameEngine, modes: &[u32]) -> RuledEventBatch {
    begin_cast(engine, modes);
    resolve_one(engine)
}

fn modify(engine: &mut GameEngine, oid: u32, source: Option<u32>, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: source,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: if source.is_some() {
            EffectDuration::WhileSourceOnBattlefield
        } else {
            EffectDuration::Indefinite
        },
        timestamp: engine.state.command_index,
    });
}

fn grant_observer(engine: &mut GameEngine, source: u32, trigger: TriggerCondition) {
    let mut ability = tricerules_cards::registry::global()
        .get("ajanis_pridemate")
        .unwrap()
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = trigger;
    engine.state.add_triggered_ability_grant(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantTriggeredAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn farewell_paid_artifact_mode_exiles_overlap_once_and_preserves_unselected_types() {
    let mut engine = setup();
    let sword = inject_permanent_on_battlefield(&mut engine, 0, "short_sword");
    let soldier = inject_creature_on_battlefield(&mut engine, 1, "yotian_soldier");
    let bear = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "ominous_seas");
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let grave = inject_graveyard_card(&mut engine, 2, "island");
    let generation = engine
        .state
        .zone_change_generation
        .get(&soldier)
        .copied()
        .unwrap_or(0);
    cast(&mut engine, &[0]);
    for oid in [sword, soldier] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Exile);
    }
    for oid in [bear, enchantment, land] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    }
    assert_eq!(engine.state.objects[&grave].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&soldier],
        generation + 1
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn farewell_all_modes_in_reverse_submission_follow_printed_order_and_owner_identity() {
    let mut engine = setup();
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let enchantment = inject_permanent_on_battlefield(&mut engine, 2, "ominous_seas");
    let overlap = inject_creature_on_battlefield(&mut engine, 2, "yotian_soldier");
    // Stolen permanent: holder/controller 9, owner 27. Exile belongs to its owner.
    engine.state.players[2]
        .battlefield
        .retain(|oid| *oid != overlap);
    engine.state.players[1].battlefield.push(overlap);
    engine.state.objects.get_mut(&overlap).unwrap().controller = 9;
    engine
        .state
        .objects
        .get_mut(&overlap)
        .unwrap()
        .base_controller = 9;
    let graves: Vec<_> = (0..3)
        .map(|seat| inject_graveyard_card(&mut engine, seat, "island"))
        .collect();
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let spell = begin_cast(&mut engine, &[3, 2, 1, 0]);
    let batch = resolve_one(&mut engine);
    for oid in [artifact, bear, enchantment, overlap]
        .into_iter()
        .chain(graves.iter().copied())
    {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Exile);
        assert_eq!(engine.state.zone_change_generation[&oid], 1);
    }
    assert!(engine.state.players[2].exile.contains(&overlap));
    assert!(!engine.state.players[1].exile.contains(&overlap));
    assert_eq!(engine.state.objects[&overlap].owner, 27);
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&spell].zone,
        Zone::Graveyard,
        "spell enters after graveyards mode"
    );
    assert_eq!(engine.state.players[0].graveyard, vec![spell]);
    assert!(engine.state.players[1].graveyard.is_empty());
    assert!(engine.state.players[2].graveyard.is_empty());
    let moves: Vec<_> = batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::PermanentMoved(m))
                if [artifact, bear, enchantment, overlap].contains(&m.object_id) =>
            {
                Some(m)
            }
            _ => None,
        })
        .collect();
    assert_eq!(moves.len(), 4, "overlap moves only once");
    let pos = |oid| moves.iter().position(|m| m.object_id == oid).unwrap();
    assert!(pos(artifact) < pos(bear) && pos(overlap) < pos(bear) && pos(bear) < pos(enchantment));
    assert_eq!(
        moves
            .iter()
            .find(|m| m.object_id == overlap)
            .unwrap()
            .owner_player_id,
        27
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn farewell_freezes_current_type_cohort_anthem_lki_counters_and_attached_generation() {
    let mut engine = setup();
    let source = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let aura = inject_permanent_on_battlefield(&mut engine, 0, "pacifism");
    engine.state.objects.get_mut(&aura).unwrap().attached_to =
        Some(AttachmentRecipient::Object(bear));
    for oid in [bear, aura] {
        modify(
            &mut engine,
            oid,
            Some(source),
            ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Artifact],
                ..Default::default()
            }),
        );
    }
    modify(
        &mut engine,
        bear,
        Some(source),
        ContinuousEffectKind::PtModify {
            delta_power: 3,
            delta_toughness: 3,
        },
    );
    engine
        .state
        .objects
        .get_mut(&bear)
        .unwrap()
        .counters
        .insert(tricerules_cards::CounterKind::Quest, 2);
    cast(&mut engine, &[0]);
    for oid in [source, bear, aura] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Exile);
    }
    let diagnostic = engine.diagnostic_snapshot().unwrap();
    let key = serde_json::json!([bear, "0"]);
    let lki = |field: &str| {
        diagnostic["state"][field]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["key"] == key)
            .unwrap()["value"]
            .clone()
    };
    assert_eq!(
        lki("last_known_pt_by_generation"),
        serde_json::json!(["5", "5"])
    );
    assert!(engine.state.last_known_types_by_generation[&(bear, 0)]
        .iter()
        .any(|t| t == "Artifact"));
    assert_eq!(lki("last_known_counters_by_generation")["Quest"], 2);
    assert_eq!(
        engine.state.last_known_attached_object_by_generation[&(aura, 0)],
        (bear, 0)
    );
    assert!(engine.state.objects[&bear].counters.is_empty());
    assert!(engine.state.objects[&aura].attached_to.is_none());
}

#[test]
fn farewell_creature_mode_reads_resolution_types_and_bypasses_all_targeting_and_destruction_defenses(
) {
    let mut engine = setup();
    let victim = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let gained = inject_permanent_on_battlefield(&mut engine, 2, "forest");
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    for keyword in [Keyword::Indestructible, Keyword::Hexproof, Keyword::Shroud] {
        modify(
            &mut engine,
            victim,
            None,
            ContinuousEffectKind::Layer6AddKeyword(keyword),
        );
    }
    modify(
        &mut engine,
        victim,
        None,
        ContinuousEffectKind::Layer6AddProtection(ProtectionQuality::Color(Color::White)),
    );
    engine
        .state
        .objects
        .get_mut(&victim)
        .unwrap()
        .regeneration_shields = 1;
    grant_observer(&mut engine, victim, TriggerCondition::WhenSelfDies);
    grant_observer(
        &mut engine,
        victim,
        TriggerCondition::WheneverPlayerSacrificesPermanent {
            player: CastTriggerPlayer::Controller,
            filter: Default::default(),
        },
    );
    begin_cast(&mut engine, &[1]);
    modify(
        &mut engine,
        gained,
        None,
        ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            ..Default::default()
        }),
    );
    // Added type recipient has a viable body until the resolution-time exile.
    engine.state.objects.get_mut(&gained).unwrap().power = Some(2);
    engine.state.objects.get_mut(&gained).unwrap().toughness = Some(2);
    resolve_one(&mut engine);
    for oid in [victim, gained] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Exile);
    }
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&victim].regeneration_shields, 0);
    assert!(
        engine.state.stack.is_empty(),
        "no death or sacrifice triggers for exile"
    );
}

#[test]
fn farewell_one_instruction_departing_observers_each_collect_one_whole_cohort() {
    let mut engine = setup();
    for seat in 0..3 {
        let observer = inject_creature_on_battlefield(&mut engine, seat, "grizzly_bears");
        inject_creature_on_battlefield(&mut engine, seat, "hill_giant");
        grant_observer(
            &mut engine,
            observer,
            TriggerCondition::WheneverPermanentLeavesBattlefield {
                controller: CastTriggerPlayer::Controller,
                filter: Default::default(),
                destination: Default::default(),
                cardinality: ZoneEventCardinality::OneOrMore,
            },
        );
    }
    cast(&mut engine, &[1]);
    assert_eq!(
        engine.state.stack.len(),
        3,
        "each departing observer sees both simultaneous departures once"
    );
    assert_eq!(
        engine
            .state
            .stack
            .iter()
            .map(|s| s.controller)
            .collect::<Vec<_>>(),
        vec![4, 9, 27]
    );
}

#[test]
fn farewell_no_sbas_between_modes_orphan_aura_enters_graveyard_after_graveyard_exile() {
    for (modes, expected) in [(vec![1, 3], Zone::Graveyard), (vec![1, 2, 3], Zone::Exile)] {
        let mut engine = setup();
        let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        let aura = inject_permanent_on_battlefield(&mut engine, 1, "pacifism");
        engine.state.objects.get_mut(&aura).unwrap().attached_to =
            Some(AttachmentRecipient::Object(bear));
        let old = inject_graveyard_card(&mut engine, 1, "island");
        cast(&mut engine, &modes);
        assert_eq!(engine.state.objects[&bear].zone, Zone::Exile);
        assert_eq!(engine.state.objects[&old].zone, Zone::Exile);
        assert_eq!(engine.state.objects[&aura].zone, expected);
        assert_eq!(engine.state.zone_change_generation[&aura], 1);
        assert_eq!(
            engine.state.players[1].graveyard.contains(&aura),
            expected == Zone::Graveyard
        );
    }
}

#[test]
fn farewell_token_leaves_and_is_observed_before_ceasing_at_postresolution_sba() {
    let mut engine = setup();
    let token = inject_creature_on_battlefield(&mut engine, 1, "soldier_w_1_1");
    grant_observer(
        &mut engine,
        token,
        TriggerCondition::WhenSelfLeavesBattlefield,
    );
    let batch = cast(&mut engine, &[1, 3]);
    assert!(
        !engine.state.objects.contains_key(&token),
        "token ceases after instruction"
    );
    assert!(!engine.state.players[1].exile.contains(&token));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "self departure trigger survives token removal"
    );
    assert_eq!(engine.state.stack[0].source_permanent_id, Some(token));
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|ev| matches!(&ev.ev,Some(Ev::PermanentMoved(m)) if m.object_id == token))
            .count(),
        1
    );
}

#[test]
fn farewell_illegal_modal_submissions_reject_atomically_before_payment_or_state_change() {
    let mut engine = setup();
    engine.state.players[0].mana_pool.colorless = 4;
    engine.state.players[0].mana_pool.white = 2;
    let slot = hand_index_for_card(&engine, 0, "farewell");
    for (actor, modes) in [
        (4, vec![]),
        (4, vec![(0, vec![]), (0, vec![])]),
        (4, vec![(4, vec![])]),
        (9, vec![(0, vec![])]),
    ] {
        let before = format!("{:?}", engine.state);
        assert!(engine
            .apply_command(actor, &cast_modal_spell(slot, modes))
            .is_err());
        assert_eq!(format!("{:?}", engine.state), before);
    }
}

#[test]
fn farewell_serialized_paid_reverse_modal_command_replays_identically() {
    use prost::Message;
    fn fresh() -> (GameEngine, u32) {
        let mut engine = setup();
        let victim = inject_creature_on_battlefield(&mut engine, 1, "yotian_soldier");
        inject_graveyard_card(&mut engine, 2, "island");
        engine.state.players[0].mana_pool.colorless = 4;
        engine.state.players[0].mana_pool.white = 2;
        (engine, victim)
    }
    let (mut engine, victim) = fresh();
    let slot = hand_index_for_card(&engine, 0, "farewell");
    let commands = vec![
        (
            4,
            cast_modal_spell(slot, vec![(3, vec![]), (1, vec![]), (0, vec![])]),
        ),
        (4, pass()),
        (9, pass()),
        (27, pass()),
    ];
    let expected: Vec<_> = commands
        .iter()
        .map(|(actor, command)| engine.apply_command(*actor, command).unwrap())
        .collect();
    assert_eq!(engine.state.objects[&victim].zone, Zone::Exile);
    let (mut replay, _) = fresh();
    for ((actor, command), batch) in commands.into_iter().zip(expected) {
        let encoded = command.encode_to_vec();
        let decoded = RuledCommand::decode(encoded.as_slice()).unwrap();
        assert_eq!(replay.apply_command(actor, &decoded).unwrap(), batch);
    }
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
}
