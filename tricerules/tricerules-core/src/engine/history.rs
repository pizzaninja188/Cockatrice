use super::*;
use crate::state::TurnRecord;

/// Shared public-cohort matcher. The caller supplies the characteristic layer appropriate to
/// its consumer, so layer-7 static counts never recursively evaluate layer 7.
pub(super) fn battlefield_permanent_matches(
    state: &GameState,
    filter: &BattlefieldPermanentFilter,
    oid: ObjectId,
    c: &Characteristics,
    context: ConditionContext<'_>,
) -> bool {
    relative_player_set_contains(state, filter.controllers, context.controller, c.controller)
        && filter.token.is_none_or(|token| {
            state
                .objects
                .get(&oid)
                .is_some_and(|object| object.is_token() == token)
        })
        && (!filter.exclude_source
            || oid != context.source_object_id
            || state.zone_change_generation.get(&oid).copied().unwrap_or(0)
                != context.source_zone_change)
        && filter.card_type.is_none_or(|kind| match kind {
            CardTypeFilter::BasicLand => {
                c.has_type("Land") && c.supertypes.iter().any(|s| s == "Basic")
            }
            CardTypeFilter::Land => c.has_type("Land"),
            CardTypeFilter::Enchantment => c.has_type("Enchantment"),
            CardTypeFilter::Instant => c.has_type("Instant"),
            CardTypeFilter::Sorcery => c.has_type("Sorcery"),
            CardTypeFilter::InstantOrSorcery => c.has_type("Instant") || c.has_type("Sorcery"),
            CardTypeFilter::Creature => c.is_creature(),
            CardTypeFilter::Artifact => c.is_artifact(),
            CardTypeFilter::Planeswalker => c.has_type("Planeswalker"),
            CardTypeFilter::Battle => c.has_type("Battle"),
            CardTypeFilter::Nonland => !c.has_type("Land"),
            CardTypeFilter::NonlandPermanent => {
                !c.has_type("Land")
                    && [
                        "Artifact",
                        "Battle",
                        "Creature",
                        "Enchantment",
                        "Planeswalker",
                    ]
                    .iter()
                    .any(|kind| c.has_type(kind))
            }
            CardTypeFilter::Noncreature => !c.is_creature(),
        })
        && filter.color.is_none_or(|color| c.colors.contains(&color))
        && filter
            .required_subtypes
            .iter()
            .all(|subtype| c.has_type(subtype))
        && filter.name.as_ref().is_none_or(|name| c.has_name(name))
        && filter.any_of.as_ref().is_none_or(|branches| {
            branches
                .iter()
                .any(|branch| battlefield_permanent_matches(state, branch, oid, c, context))
        })
}

pub(super) fn battlefield_quantity_value(
    state: &GameState,
    expression: &CountExpression,
    context: ConditionContext<'_>,
    characteristics: impl Fn(ObjectId) -> Option<Characteristics>,
) -> Option<i64> {
    if !matches!(
        expression,
        CountExpression::BattlefieldPermanents { .. }
            | CountExpression::BattlefieldCreatures { .. }
            | CountExpression::BattlefieldMaximum { .. }
    ) {
        return None;
    }
    let mut values = state
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter().copied())
        .filter_map(|oid| characteristics(oid).map(|c| (oid, c)))
        .filter(|(oid, c)| match expression {
            CountExpression::BattlefieldPermanents { filter }
            | CountExpression::BattlefieldMaximum { filter, .. } => {
                battlefield_permanent_matches(state, filter, *oid, c, context)
            }
            CountExpression::BattlefieldCreatures { filter } => {
                relative_player_set_contains(
                    state,
                    filter.controllers,
                    context.controller,
                    c.controller,
                ) && c.is_creature()
                    && (!filter.exclude_source
                        || *oid != context.source_object_id
                        || state.zone_change_generation.get(oid).copied().unwrap_or(0)
                            != context.source_zone_change)
                    && filter
                        .subtype
                        .as_ref()
                        .is_none_or(|subtype| c.has_type(subtype))
                    && filter
                        .required_keywords
                        .iter()
                        .all(|keyword| c.has_keyword(*keyword))
                    && state.objects.get(oid).is_some_and(|object| {
                        filter.tapped.is_none_or(|tapped| object.tapped == tapped)
                            && (!filter.requires_any_counter || object.has_any_counter())
                            && filter
                                .required_counter
                                .is_none_or(|counter| object.counter_count(counter) > 0)
                    })
            }
            _ => false,
        })
        .map(|(_, c)| c);
    Some(match expression {
        CountExpression::BattlefieldMaximum { characteristic, .. } => values
            .filter_map(|c| match characteristic {
                tricerules_cards::PowerToughnessCharacteristic::Power => c.signed_power,
                tricerules_cards::PowerToughnessCharacteristic::Toughness => c.signed_toughness,
            })
            .max()
            .unwrap_or(0),
        _ => values.by_ref().count().try_into().unwrap_or(i64::MAX),
    })
}

/// Commit an already authorized life change. Star Charter and Flamecache Gecko need actual
/// gains/losses, including damage and payments, rather than the net change or emitted UI events.
/// Callers own prevention/prohibition and transaction validation; trigger dispatch never records
/// this again. Initialization does not pass through this boundary (CR 119.2-119.4).
pub(super) fn commit_life_change(state: &mut GameState, player_idx: usize, delta: i32) {
    if delta == 0 {
        return;
    }
    let player = &mut state.players[player_idx];
    player.life += delta;
    let player_id = player.id;
    let record = state.turn_history.current.player_mut(player_id);
    let total = if delta > 0 {
        &mut record.life_gained
    } else {
        &mut record.life_lost
    };
    *total = total.saturating_add(u64::from(delta.unsigned_abs()));
}

fn clamp_public_count(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// Shared by live conditions and static characteristics; never re-query the departed object.
pub(super) fn permanent_history_count(
    state: &GameState,
    facts: &[crate::state::PermanentHistoryFact],
    players: RelativePlayerSet,
    permanent_type: Option<PermanentTypeFilter>,
    controller: PlayerId,
) -> u32 {
    clamp_public_count(
        facts
            .iter()
            .filter(|fact| {
                relative_player_set_contains(state, players, controller, fact.player)
                    && permanent_type
                        .is_none_or(|kind| fact.types.iter().any(|t| t == kind.as_str()))
            })
            .count(),
    )
}

/// Snapshot after replacement handling using the printed destination-zone card, not its
/// former animation, copied values, face-down status, or Adventure spell face (CR 700.11).
pub(super) fn graveyard_entry_fact(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Option<crate::state::PermanentHistoryFact> {
    let object = state.objects.get(&oid)?;
    if object.zone != Zone::Graveyard || object.is_token() {
        return None;
    }
    let types: Vec<String> = registry
        .get(&object.card_id)?
        .card_types_outside_stack()
        .into_iter()
        .map(str::to_owned)
        .collect();
    if !types.iter().any(|t| {
        matches!(
            t.as_str(),
            "Artifact" | "Battle" | "Creature" | "Enchantment" | "Land" | "Planeswalker"
        )
    }) {
        return None;
    }
    Some(crate::state::PermanentHistoryFact {
        object_id: oid,
        zone_change_generation: state.zone_change_generation.get(&oid).copied().unwrap_or(0),
        player: object.owner,
        types,
    })
}

pub(super) fn spell_cast_matches(
    filter: &SpellCastFilter,
    fact: &crate::state::SpellCastFact,
) -> bool {
    filter
        .card_type
        .is_none_or(|kind| fact.matched_card_types.contains(&kind))
        && filter
            .targeted_permanent_type
            .is_none_or(|kind| fact.targeted_permanent_types.contains(&kind))
        && filter.required_subtypes.iter().all(|subtype| {
            fact.types.contains(subtype)
                || (fact.all_creature_types && tricerules_cards::is_creature_type(subtype))
        })
        && filter
            .min_mana_value
            .is_none_or(|min| fact.mana_value >= min)
        && filter
            .max_mana_value
            .is_none_or(|max| fact.mana_value <= max)
        && filter.origin.is_none_or(|origin| {
            matches!(
                (origin, fact.origin),
                (SpellCastOrigin::Hand, Zone::Hand)
                    | (SpellCastOrigin::Graveyard, Zone::Graveyard)
                    | (SpellCastOrigin::Exile, Zone::Exile)
            )
        })
        && filter.any_of.as_ref().is_none_or(|branches| {
            branches
                .iter()
                .any(|branch| spell_cast_matches(branch, fact))
        })
}

/// One count per committed occurrence, not per matching OR branch or live stack object.
pub(super) fn spell_cast_count(
    state: &GameState,
    players: ConditionPlayerSet,
    filter: &SpellCastFilter,
    controller: PlayerId,
    item: Option<&StackItem>,
    exclude_source: bool,
) -> u32 {
    spell_cast_count_in_record(
        state,
        &state.turn_history.current,
        players,
        filter,
        controller,
        item,
        exclude_source,
    )
}

pub(super) fn spell_cast_count_in_record(
    state: &GameState,
    record: &TurnRecord,
    players: ConditionPlayerSet,
    filter: &SpellCastFilter,
    controller: PlayerId,
    item: Option<&StackItem>,
    exclude_source: bool,
) -> u32 {
    let chosen = selected_condition_player(
        players,
        item.map(|item| item.targets.as_slice()),
        item.and_then(|item| item.trigger_context.affected_player),
    );
    let excluded = exclude_source
        .then(|| item.and_then(|item| item.cast_occurrence))
        .flatten();
    clamp_public_count(
        record
            .spell_casts
            .iter()
            .filter(|fact| {
                let selected = match players {
                    ConditionPlayerSet::Relative(set) => {
                        relative_player_set_contains(state, set, controller, fact.caster)
                    }
                    _ => chosen == Some(fact.caster),
                };
                selected && excluded != Some(fact.occurrence) && spell_cast_matches(filter, fact)
            })
            .count(),
    )
}

fn selected_condition_player(
    players: ConditionPlayerSet,
    targets: Option<&[StackTarget]>,
    affected_player: Option<PlayerId>,
) -> Option<PlayerId> {
    match players {
        ConditionPlayerSet::Relative(_) => None,
        ConditionPlayerSet::AffectedPlayer => affected_player,
        ConditionPlayerSet::ChosenTarget {
            group_index,
            target_index,
        } => {
            let target = targets?
                .iter()
                .filter(|target| target.group_index == group_index)
                .nth(target_index as usize)?;
            // Unspecified presentation kinds may still identify an authoritative player,
            // but explicit object kinds and generation-bound objects never do.
            if !matches!(
                rv1::TargetRefKind::try_from(target.kind),
                Ok(rv1::TargetRefKind::Player | rv1::TargetRefKind::Unspecified)
            ) || target.zone_change_generation.is_some()
            {
                return None;
            }
            PlayerId::try_from(target.object_id).ok()
        }
    }
}

pub(super) fn life_changed_this_turn(
    state: &GameState,
    players: ConditionPlayerSet,
    change: LifeChangeKind,
    quantifier: PlayerQuantifier,
    controller: PlayerId,
    targets: Option<&[StackTarget]>,
    affected_player: Option<PlayerId>,
) -> bool {
    let chosen = selected_condition_player(players, targets, affected_player);
    let mut selected = state
        .players
        .iter()
        .filter(|player| {
            !player.has_lost
                && match players {
                    ConditionPlayerSet::Relative(set) => {
                        relative_player_set_contains(state, set, controller, player.id)
                    }
                    ConditionPlayerSet::ChosenTarget { .. }
                    | ConditionPlayerSet::AffectedPlayer => chosen == Some(player.id),
                }
        })
        .peekable();
    if selected.peek().is_none() {
        return false;
    }
    let matches = |player: &PlayerState| {
        let record = state.turn_history.current.player(player.id);
        match change {
            LifeChangeKind::Gain => record.life_gained > 0,
            LifeChangeKind::Loss => record.life_lost > 0,
            LifeChangeKind::Either => record.life_gained > 0 || record.life_lost > 0,
        }
    };
    match quantifier {
        PlayerQuantifier::Any => selected.any(matches),
        PlayerQuantifier::All => selected.all(matches),
    }
}

pub(super) fn relative_player_set_contains(
    state: &GameState,
    set: RelativePlayerSet,
    reference: PlayerId,
    candidate: PlayerId,
) -> bool {
    match set {
        RelativePlayerSet::Controller => candidate == reference,
        RelativePlayerSet::Opponents => state.are_opponents(candidate, reference),
        RelativePlayerSet::All => true,
    }
}

pub(super) fn player_life_aggregate_value(
    state: &GameState,
    players: RelativePlayerSet,
    aggregate: PlayerLifeAggregate,
    reference: PlayerId,
    mut life_of: impl FnMut(PlayerId) -> Option<i32>,
) -> Option<i32> {
    let values = state
        .players
        .iter()
        .filter(|player| relative_player_set_contains(state, players, reference, player.id))
        .filter_map(|player| life_of(player.id));
    match aggregate {
        PlayerLifeAggregate::Minimum => values.min(),
        PlayerLifeAggregate::Maximum => values.max(),
    }
}

pub(super) fn graveyard_aggregate_value(
    state: &GameState,
    registry: &'static CardRegistry,
    owners: RelativePlayerSet,
    aggregate: GraveyardAggregate,
    filter: Option<&ZoneCardFilter>,
    controller: PlayerId,
    resolving_spell_id: Option<ObjectId>,
) -> u32 {
    let definitions: Vec<_> = state
        .players
        .iter()
        .filter(|player| relative_player_set_contains(state, owners, controller, player.id))
        .flat_map(|player| player.graveyard.iter().copied())
        .filter(|oid| Some(*oid) != resolving_spell_id)
        .filter_map(|oid| state.objects.get(&oid))
        .filter(|object| object.zone == Zone::Graveyard && !object.is_token())
        .filter(|object| {
            super::resolution::library_card_matches_filter(state, registry, object.id, filter)
        })
        .filter_map(|object| registry.get(&object.card_id))
        .collect();

    match aggregate {
        GraveyardAggregate::CardCount => clamp_public_count(definitions.len()),
        GraveyardAggregate::DistinctCardTypes => clamp_public_count(
            definitions
                .iter()
                .flat_map(|definition| definition.card_types_outside_stack())
                .collect::<HashSet<_>>()
                .len(),
        ),
    }
}

pub(super) fn creature_event_fact_matches(
    filter: &CreatureEventFilter,
    fact: &TurnObjectFact,
) -> bool {
    filter
        .required_subtypes
        .iter()
        .all(|subtype| fact.has_type(subtype))
        && filter
            .required_keywords
            .iter()
            .all(|keyword| fact.keywords.contains(keyword))
        && filter
            .excluded_keywords
            .iter()
            .all(|keyword| !fact.keywords.contains(keyword))
        && filter.power.is_none_or(|comparison| {
            fact.power.is_some_and(|power| match comparison {
                PowerComparison::AtLeast(minimum) => power >= minimum,
                PowerComparison::AtMost(maximum) => power <= maximum,
            })
        })
}

pub(super) fn permanent_event_fact_matches(
    state: &GameState,
    filter: &PermanentEventFilter,
    fact: &TurnObjectFact,
    context: ConditionContext<'_>,
) -> bool {
    let type_matches = filter
        .permanent_type
        .is_none_or(|permanent_type| fact.has_type(permanent_type.as_str()));
    type_matches
        && filter
            .excluded_types
            .iter()
            .all(|kind| !fact.has_type(kind.as_str()))
        && (filter.any_subtypes.is_empty()
            || filter.any_subtypes.iter().any(|kind| fact.has_type(kind)))
        && filter.token.is_none_or(|token| token == fact.is_token)
        && filter.owner.is_none_or(|owner| match owner {
            CastTriggerPlayer::Controller => fact.owner == context.controller,
            CastTriggerPlayer::Opponent => state.are_opponents(fact.owner, context.controller),
            CastTriggerPlayer::AnyPlayer => true,
        })
        && (!filter.source_only
            || (fact.object_id == context.source_object_id
                && fact.zone_change_generation == context.source_zone_change))
        && filter
            .required_subtypes
            .iter()
            .all(|subtype| fact.has_type(subtype))
        && (!filter.exclude_source
            || fact.object_id != context.source_object_id
            || fact.zone_change_generation != context.source_zone_change)
        && filter.any_of.as_ref().is_none_or(|branches| {
            branches
                .iter()
                .any(|branch| permanent_event_fact_matches(state, branch, fact, context))
        })
}

impl GameEngine {
    /// Classify the initial legal targets without committing anything. Callers retain this
    /// snapshot across payment, then record it only when the original action completes. Copies,
    /// retargeting, and untargeted references (including Ward) do not call this boundary.
    pub(super) fn crime_event(
        &self,
        player: PlayerId,
        targets: &[rv1::TargetRef],
    ) -> Option<GameEvent> {
        targets
            .iter()
            .any(|target| {
                let oid = target.object_id;
                let Ok(kind) = rv1::TargetRefKind::try_from(target.kind) else {
                    return false;
                };
                let opponent = match kind {
                    rv1::TargetRefKind::Player => self
                        .state
                        .players
                        .iter()
                        .find(|candidate| candidate.id as ObjectId == oid)
                        .map(|p| p.id),
                    rv1::TargetRefKind::Stack => self
                        .state
                        .stack
                        .iter()
                        .find(|item| item.id == oid)
                        .map(|item| item.controller),
                    rv1::TargetRefKind::Permanent => self
                        .state
                        .objects
                        .get(&oid)
                        .filter(|object| object.zone == Zone::Battlefield)
                        .and_then(|_| self.characteristics(oid))
                        .map(|c| c.controller),
                    rv1::TargetRefKind::Graveyard => self
                        .state
                        .objects
                        .get(&oid)
                        .filter(|object| object.zone == Zone::Graveyard)
                        .map(|object| object.owner),
                    rv1::TargetRefKind::Unspecified => {
                        // Legacy callers omit the kind; infer it only from authoritative live state.
                        if let Some(p) = self.state.players.iter().find(|p| p.id as ObjectId == oid)
                        {
                            Some(p.id)
                        } else if let Some(item) =
                            self.state.stack.iter().find(|item| item.id == oid)
                        {
                            Some(item.controller)
                        } else {
                            self.state
                                .objects
                                .get(&oid)
                                .and_then(|object| match object.zone {
                                    Zone::Battlefield => {
                                        self.characteristics(oid).map(|c| c.controller)
                                    }
                                    Zone::Graveyard => Some(object.owner),
                                    _ => None,
                                })
                        }
                    }
                };
                opponent.is_some_and(|opponent| self.state.are_opponents(player, opponent))
            })
            .then_some(GameEvent::CrimeCommitted { player })
    }

    /// Record a committed simultaneous event set. This is deliberately separate from trigger
    /// matching: transactional cast/activation checks may collect prospective triggers, but turn
    /// history must only observe changes that actually reached game state.
    pub(super) fn record_committed_events(&mut self, events: &[GameEvent]) {
        // Record the whole committed cohort before inspecting any characteristics or triggers.
        // Pre-move derived types distinguish animated lands from actual nonland permanents.
        self.state
            .turn_history
            .current
            .nonland_permanent_left_battlefield |= events.iter().any(|event| {
            matches!(event, GameEvent::ZoneChanges(batch) if batch.moves.iter().any(|movement|
                movement.origin == Zone::Battlefield && movement.destination != Zone::Battlefield
                    && !movement.before.has_type("Land")))
        });
        let departed_controllers = events.iter().flat_map(|event| match event {
            GameEvent::ZoneChanges(batch) => batch
                .moves
                .iter()
                .filter(|movement| {
                    movement.origin == Zone::Battlefield
                        && movement.destination != Zone::Battlefield
                })
                .map(|movement| movement.before.controller)
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        });
        for controller in departed_controllers {
            self.state
                .turn_history
                .current
                .player_mut(controller)
                .permanent_left_battlefield = true;
        }
        let deaths = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::Dies {
                        was_creature: true,
                        ..
                    }
                )
            })
            .count() as u32;
        self.state.turn_history.current.creatures_died = self
            .state
            .turn_history
            .current
            .creatures_died
            .saturating_add(deaths);

        for event in events {
            match event {
                GameEvent::Sacrificed { source, player } => {
                    self.state.turn_history.current.permanents_sacrificed.push(
                        crate::state::PermanentHistoryFact {
                            object_id: source.object_id,
                            zone_change_generation: source.zone_change_generation,
                            player: *player,
                            types: source.types.clone(),
                        },
                    );
                }
                GameEvent::LeavesBattlefield { source } => {
                    // All members of a simultaneous departure set were captured before any move.
                    // Restore that snapshot over the individual move's sequential bookkeeping.
                    self.state.last_known_pt_by_generation.insert(
                        (source.object_id, source.zone_change_generation),
                        source.power_toughness,
                    );
                }
                GameEvent::CrimeCommitted { player } => {
                    let record = self.state.turn_history.current.player_mut(*player);
                    record.crimes_committed = record.crimes_committed.saturating_add(1);
                }
                GameEvent::EntersBattlefield { object_id } => {
                    if let Some(characteristics) = self.characteristics(*object_id) {
                        self.state
                            .turn_history
                            .current
                            .permanents_entered
                            .push(TurnObjectFact {
                                object_id: *object_id,
                                zone_change_generation: self
                                    .state
                                    .zone_change_generation
                                    .get(object_id)
                                    .copied()
                                    .unwrap_or(0),
                                controller: characteristics.controller,
                                owner: self.state.objects[object_id].owner,
                                is_token: self.state.objects[object_id].is_token(),
                                types: characteristics.types,
                                all_creature_types: characteristics.all_creature_types,
                                keywords: characteristics.keywords,
                                power: characteristics.power,
                            });
                    }
                }
                GameEvent::CardDrawn { drawer, .. } => {
                    let record = self.state.turn_history.current.player_mut(*drawer);
                    record.cards_drawn = record.cards_drawn.saturating_add(1);
                }
                GameEvent::AttackersDeclared {
                    attacking_player,
                    attacks,
                } if !attacks.is_empty() => {
                    self.state
                        .turn_history
                        .current
                        .player_mut(*attacking_player)
                        .attacked = true;
                    let facts = attacks
                        .iter()
                        .filter_map(|attack| {
                            let characteristics =
                                self.characteristics(attack.attacker.object_id)?;
                            Some(TurnObjectFact {
                                object_id: attack.attacker.object_id,
                                zone_change_generation: attack.attacker.zone_change_generation,
                                controller: *attacking_player,
                                owner: self.state.objects[&attack.attacker.object_id].owner,
                                is_token: self.state.objects[&attack.attacker.object_id].is_token(),
                                types: characteristics.types,
                                all_creature_types: characteristics.all_creature_types,
                                keywords: characteristics.keywords,
                                power: characteristics.power,
                            })
                        })
                        .collect::<Vec<_>>();
                    self.state
                        .turn_history
                        .current
                        .declared_attackers
                        .extend(facts);
                }
                GameEvent::DamageDealt { event }
                    if event.amount > 0
                        && matches!(event.recipient, damage::DamageRecipient::Permanent(_)) =>
                {
                    let damage::DamageRecipient::Permanent(object_id) = event.recipient else {
                        unreachable!()
                    };
                    let identity = (
                        object_id,
                        self.state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0),
                    );
                    if !self
                        .state
                        .turn_history
                        .current
                        .damaged_objects
                        .contains(&identity)
                    {
                        self.state
                            .turn_history
                            .current
                            .damaged_objects
                            .push(identity);
                    }
                }
                _ => {}
            }
        }
    }

    /// Only actual casts enter this path, including a copy that is actually cast (CR 707.12).
    /// Directly created stack copies never call it. Origin is captured before stack entry.
    pub(super) fn record_spell_cast(
        &mut self,
        caster: PlayerId,
        object_id: ObjectId,
        origin: Zone,
        mana_value: u32,
        mana_spent: u64,
    ) -> crate::state::SpellCastFact {
        let item_index = self
            .state
            .stack
            .iter()
            .position(|item| item.id == object_id)
            .expect("committed cast has a stack item");
        let targets = self.state.stack[item_index].targets.clone();
        let targeted_characteristics = targets
            .iter()
            .filter_map(|target| {
                let expected_generation = target.zone_change_generation?;
                let object = self.state.objects.get(&target.object_id)?;
                let current_generation = self
                    .state
                    .zone_change_generation
                    .get(&target.object_id)
                    .copied()
                    .unwrap_or(0);
                (object.zone == Zone::Battlefield && current_generation == expected_generation)
                    .then(|| self.characteristics(target.object_id))
                    .flatten()
            })
            .collect::<Vec<_>>();
        let targeted_permanent_types = [
            PermanentTypeFilter::Creature,
            PermanentTypeFilter::Artifact,
            PermanentTypeFilter::Enchantment,
            PermanentTypeFilter::Land,
            PermanentTypeFilter::Planeswalker,
            PermanentTypeFilter::Battle,
        ]
        .into_iter()
        .filter(|kind| {
            targeted_characteristics
                .iter()
                .any(|characteristics| characteristics.has_type(kind.as_str()))
        })
        .collect();
        let item = &mut self.state.stack[item_index];
        let occurrence = StackObjectRef {
            object_id,
            zone_change_generation: self.state.objects.contains_key(&object_id).then(|| {
                self.state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0)
            }),
        };
        item.cast_occurrence = Some(occurrence);
        let definition = self
            .registry
            .get(&item.card_id)
            .expect("committed cast has a validated definition");
        let face = definition
            .face(item.face_index)
            .expect("committed cast has a validated face");
        let mut types = face.types.clone();
        // CR 715.3b: existing Adventure data encodes the alternative face through layout,
        // even when its type list omits the Adventure spell subtype (e.g. Stomp).
        if definition.layout == tricerules_cards::Layout::Adventure
            && item.face_index == 1
            && !types.iter().any(|kind| kind == "Adventure")
        {
            types.push("Adventure".into());
        }
        let mut fact = crate::state::SpellCastFact {
            cast_method: item.cast_method,
            occurrence,
            caster,
            origin,
            face_index: item.face_index,
            types,
            all_creature_types: face
                .characteristic_defining_abilities
                .iter()
                .any(|ability| {
                    ability.definition
                        == tricerules_cards::CharacteristicDefiningAbility::Changeling
                }),
            mana_value,
            mana_spent,
            matched_card_types: CardTypeFilter::ALL
                .into_iter()
                .filter(|filter| face.matches_card_type(*filter))
                .collect(),
            targeted_permanent_types,
            ordinal: 0,
        };
        self.state.turn_history.current.spells_cast = self
            .state
            .turn_history
            .current
            .spells_cast
            .saturating_add(1);
        let record = self.state.turn_history.current.player_mut(caster);
        record.spells_cast = record.spells_cast.saturating_add(1);
        fact.ordinal = record.spells_cast;
        self.state
            .turn_history
            .current
            .spell_casts
            .push(fact.clone());
        fact
    }

    /// CR 700.14: record only committed mana payments for spells. Independent of watcher presence.
    pub(super) fn record_spell_mana_spent(&mut self, player: PlayerId, amount: u64) -> GameEvent {
        let record = self.state.turn_history.current.player_mut(player);
        let before = record.mana_spent_casting_spells;
        let after = before.saturating_add(amount);
        record.mana_spent_casting_spells = after;
        GameEvent::ManaSpentCastingSpell {
            player,
            before,
            after,
        }
    }

    pub(super) fn fire_card_drawn(&mut self, drawer: PlayerId) {
        let ordinal = self
            .state
            .turn_history
            .current
            .player(drawer)
            .cards_drawn
            .saturating_add(1);
        self.fire_triggers(&[GameEvent::CardDrawn { drawer, ordinal }]);
    }

    pub(super) fn condition_holds(
        &self,
        condition: &GameCondition,
        context: ConditionContext,
    ) -> bool {
        self.condition_holds_with_trigger_context(condition, context, None)
    }

    pub(super) fn condition_holds_with_trigger_context(
        &self,
        condition: &GameCondition,
        context: ConditionContext,
        trigger_context: Option<&TriggerContext>,
    ) -> bool {
        match condition {
            GameCondition::AnyOf(branches) => branches.iter().any(|branch| {
                self.condition_holds_with_trigger_context(branch, context, trigger_context)
            }),
            GameCondition::HasEnduringStory { players } => {
                self.state.players.iter().any(|player| {
                    relative_player_set_contains(
                        &self.state,
                        *players,
                        context.controller,
                        player.id,
                    ) && player.has_enduring_story
                })
            }
            GameCondition::Void => self.state.turn_history.current.void_holds(),
            GameCondition::PermanentLeftBattlefieldThisTurn { controllers } => self
                .state
                .players
                .iter()
                .filter(|player| {
                    relative_player_set_contains(
                        &self.state,
                        *controllers,
                        context.controller,
                        player.id,
                    )
                })
                .any(|player| {
                    self.state
                        .turn_history
                        .current
                        .player(player.id)
                        .permanent_left_battlefield
                }),
            GameCondition::CastSnapshot { index } => context
                .stack_item
                .filter(|item| item.ability_text.is_none() && !item.is_copy)
                .and_then(|item| item.cast_condition_results.get(*index as usize))
                .copied()
                .unwrap_or(false),
            GameCondition::TriggeringSpellManaSpent { comparison } => {
                let spent = trigger_context
                    .or_else(|| context.stack_item.map(|item| &item.trigger_context))
                    .and_then(|trigger| trigger.triggering_spell_mana_spent);
                match (spent, comparison) {
                    (Some(spent), SpellManaSpentComparison::AtLeast(minimum)) => spent >= *minimum,
                    (Some(spent), SpellManaSpentComparison::GreaterThanSourcePowerOrToughness) => {
                        let source_is_current_creature = self
                            .state
                            .objects
                            .get(&context.source_object_id)
                            .is_some_and(|object| object.zone == Zone::Battlefield)
                            && self
                                .state
                                .zone_change_generation
                                .get(&context.source_object_id)
                                .copied()
                                .unwrap_or(0)
                                == context.source_zone_change;
                        source_is_current_creature
                            && self
                                .characteristics(context.source_object_id)
                                .filter(|characteristics| characteristics.is_creature())
                                .is_some_and(|characteristics| {
                                    let spent = i128::from(spent);
                                    spent > i128::from(characteristics.signed_power.unwrap_or(0))
                                        || spent
                                            > i128::from(
                                                characteristics.signed_toughness.unwrap_or(0),
                                            )
                                })
                    }
                    (None, _) => false,
                }
            }
            GameCondition::LifeChangedThisTurn {
                players,
                change,
                quantifier,
            } => life_changed_this_turn(
                &self.state,
                *players,
                *change,
                *quantifier,
                context.controller,
                context.stack_item.map(|item| item.targets.as_slice()),
                context
                    .stack_item
                    .and_then(|item| item.trigger_context.affected_player),
            ),
            GameCondition::ActivePlayer { players } => relative_player_set_contains(
                &self.state,
                *players,
                context.controller,
                self.state.active_player_id(),
            ),
            GameCondition::PlayerLifeAggregate {
                players, aggregate, ..
            } => player_life_aggregate_value(
                &self.state,
                *players,
                *aggregate,
                context.controller,
                |player_id| {
                    self.state
                        .players
                        .iter()
                        .find(|player| player.id == player_id)
                        .map(|player| player.life)
                },
            )
            .is_some_and(|value| condition.matches_life_value(value)),
            GameCondition::CreatureDeathsThisTurn { .. } => {
                condition.matches_value(self.state.turn_history.current.creatures_died)
            }
            GameCondition::PermanentCardsEnteredGraveyardThisTurn {
                players,
                permanent_type,
                ..
            } => condition.matches_value(permanent_history_count(
                &self.state,
                &self
                    .state
                    .turn_history
                    .current
                    .permanent_cards_entered_graveyard,
                *players,
                *permanent_type,
                context.controller,
            )),
            GameCondition::PermanentsSacrificedThisTurn {
                players,
                permanent_type,
                ..
            } => condition.matches_value(permanent_history_count(
                &self.state,
                &self.state.turn_history.current.permanents_sacrificed,
                *players,
                *permanent_type,
                context.controller,
            )),
            GameCondition::SpellsCastThisTurn {
                players, filter, ..
            } => condition.matches_value(super::history::spell_cast_count(
                &self.state,
                ConditionPlayerSet::Relative(*players),
                filter,
                context.controller,
                None,
                false,
            )),
            GameCondition::SpellsCastLastTurn {
                players, filter, ..
            } => {
                let count = if *players == RelativePlayerSet::All
                    && *filter == SpellCastFilter::default()
                {
                    self.state.turn_history.previous.spells_cast
                } else {
                    spell_cast_count_in_record(
                        &self.state,
                        &self.state.turn_history.previous,
                        ConditionPlayerSet::Relative(*players),
                        filter,
                        context.controller,
                        None,
                        false,
                    )
                };
                condition.matches_value(count)
            }
            GameCondition::CrimesCommittedThisTurn { players, .. } => {
                let count = self
                    .state
                    .players
                    .iter()
                    .filter(|player| {
                        relative_player_set_contains(
                            &self.state,
                            *players,
                            context.controller,
                            player.id,
                        )
                    })
                    .fold(0u32, |total, player| {
                        total.saturating_add(
                            self.state
                                .turn_history
                                .current
                                .player(player.id)
                                .crimes_committed,
                        )
                    });
                condition.matches_value(count)
            }
            GameCondition::CardsDrawnThisTurn { players, .. } => {
                let count = self
                    .state
                    .players
                    .iter()
                    .filter(|player| {
                        relative_player_set_contains(
                            &self.state,
                            *players,
                            context.controller,
                            player.id,
                        )
                    })
                    .fold(0u32, |total, player| {
                        total.saturating_add(
                            self.state
                                .turn_history
                                .current
                                .player(player.id)
                                .cards_drawn,
                        )
                    });
                condition.matches_value(count)
            }
            GameCondition::AttackedThisTurn { players } => self
                .state
                .players
                .iter()
                .filter(|player| {
                    relative_player_set_contains(
                        &self.state,
                        *players,
                        context.controller,
                        player.id,
                    )
                })
                .any(|player| self.state.turn_history.current.player(player.id).attacked),
            GameCondition::AttackersDeclaredThisTurn {
                players, filter, ..
            } => {
                let count = self
                    .state
                    .turn_history
                    .current
                    .declared_attackers
                    .iter()
                    .filter(|fact| {
                        relative_player_set_contains(
                            &self.state,
                            *players,
                            context.controller,
                            fact.controller,
                        ) && creature_event_fact_matches(filter, fact)
                    })
                    .count();
                condition.matches_value(clamp_public_count(count))
            }
            GameCondition::PermanentsEnteredThisTurn {
                controllers,
                filter,
                ..
            } => {
                let count = self
                    .state
                    .turn_history
                    .current
                    .permanents_entered
                    .iter()
                    .filter(|fact| {
                        relative_player_set_contains(
                            &self.state,
                            *controllers,
                            context.controller,
                            fact.controller,
                        ) && permanent_event_fact_matches(&self.state, filter, fact, context)
                    })
                    .count();
                condition.matches_value(clamp_public_count(count))
            }
            GameCondition::SourceCounterCount { counter, .. } => {
                let count = self
                    .state
                    .objects
                    .get(&context.source_object_id)
                    .filter(|object| object.zone == Zone::Battlefield)
                    .filter(|_| {
                        self.state
                            .zone_change_generation
                            .get(&context.source_object_id)
                            .copied()
                            .unwrap_or(0)
                            == context.source_zone_change
                    })
                    .map(|object| object.counter_count(*counter))
                    .unwrap_or(0);
                condition.matches_value(count)
            }
            GameCondition::ObjectWasDealtDamageThisTurn { object } => self
                .condition_object_identity(*object, context)
                .is_some_and(|identity| {
                    self.state
                        .turn_history
                        .current
                        .damaged_objects
                        .contains(&identity)
                }),
            GameCondition::ObjectTapped { object, tapped } => {
                let Some((object_id, expected_generation)) =
                    self.condition_object_identity(*object, context)
                else {
                    return false;
                };
                let live_tapped = self
                    .state
                    .objects
                    .get(&object_id)
                    .filter(|candidate| candidate.zone == Zone::Battlefield)
                    .filter(|_| {
                        self.state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0)
                            == expected_generation
                    })
                    .map(|candidate| candidate.tapped);
                let observed_tapped = live_tapped.or_else(|| {
                    matches!(object, ConditionObjectRef::Source)
                        .then(|| {
                            self.state
                                .last_known_tapped_by_generation
                                .get(&(object_id, expected_generation))
                                .copied()
                                .or_else(|| self.state.last_known_tapped.get(&object_id).copied())
                        })
                        .flatten()
                });
                observed_tapped == Some(*tapped)
            }
            GameCondition::ObjectMatches { object, filter } => self
                .condition_object_identity(*object, context)
                .is_some_and(|(object_id, expected_generation)| {
                    self.state
                        .zone_change_generation
                        .get(&object_id)
                        .copied()
                        .unwrap_or(0)
                        == expected_generation
                        && super::targeting::object_matches_scoped_mass_filter(
                            self,
                            object_id,
                            filter.as_ref(),
                            context.controller,
                        )
                }),
            GameCondition::BattlefieldCreatureCount { filter, .. } => {
                condition.matches_value(self.battlefield_creature_count(
                    filter,
                    context.controller,
                    context.source_object_id,
                ))
            }
            GameCondition::BattlefieldAggregate {
                filter, aggregate, ..
            } => condition
                .matches_value(self.battlefield_aggregate_value(filter, *aggregate, context)),
            GameCondition::UnlockedRoomDoorCount { controllers, .. } => condition
                .matches_value(self.unlocked_room_door_count(*controllers, context.controller)),
            GameCondition::GraveyardAggregate {
                owners,
                aggregate,
                filter,
                ..
            } => condition.matches_value(graveyard_aggregate_value(
                &self.state,
                self.registry,
                *owners,
                *aggregate,
                filter.as_ref(),
                context.controller,
                context.resolving_spell_id,
            )),
        }
    }

    pub(super) fn condition_object_identity(
        &self,
        object: ConditionObjectRef,
        context: ConditionContext<'_>,
    ) -> Option<(ObjectId, u64)> {
        match object {
            ConditionObjectRef::Source => {
                Some((context.source_object_id, context.source_zone_change))
            }
            ConditionObjectRef::ChosenTarget {
                group_index,
                target_index,
            } => context
                .stack_item?
                .targets
                .iter()
                .filter(|target| target.group_index == group_index)
                .nth(target_index as usize)
                .and_then(|target| {
                    target
                        .zone_change_generation
                        .map(|generation| (target.object_id, generation))
                }),
        }
    }

    pub(super) fn unlocked_room_door_count(
        &self,
        controllers: RelativePlayerSet,
        condition_controller: PlayerId,
    ) -> u32 {
        self.state
            .room_states
            .iter()
            .filter_map(|(object_id, room)| {
                let object = self.state.objects.get(object_id)?;
                (object.zone == Zone::Battlefield
                    && relative_player_set_contains(
                        &self.state,
                        controllers,
                        condition_controller,
                        object.controller,
                    ))
                .then_some(
                    room.unlocked
                        .into_iter()
                        .filter(|unlocked| *unlocked)
                        .count(),
                )
            })
            .sum::<usize>()
            .try_into()
            .unwrap_or(u32::MAX)
    }

    fn battlefield_matching_characteristics(
        &self,
        filter: &BattlefieldPermanentFilter,
        context: ConditionContext,
    ) -> Vec<(ObjectId, Characteristics)> {
        self.state
            .players
            .iter()
            .flat_map(|player| player.battlefield.iter().copied())
            .filter(|oid| {
                !filter.exclude_source
                    || *oid != context.source_object_id
                    || self
                        .state
                        .zone_change_generation
                        .get(oid)
                        .copied()
                        .unwrap_or(0)
                        != context.source_zone_change
            })
            .filter_map(|oid| {
                self.characteristics(oid)
                    .map(|characteristics| (oid, characteristics))
            })
            .filter(|(oid, characteristics)| {
                battlefield_permanent_matches(&self.state, filter, *oid, characteristics, context)
            })
            .collect()
    }

    pub(super) fn battlefield_aggregate_value(
        &self,
        filter: &BattlefieldPermanentFilter,
        aggregate: BattlefieldAggregate,
        context: ConditionContext,
    ) -> u32 {
        let matching = self.battlefield_matching_characteristics(filter, context);
        match aggregate {
            BattlefieldAggregate::Count => clamp_public_count(matching.len()),
            BattlefieldAggregate::DistinctNames => clamp_public_count(
                matching
                    .iter()
                    .filter_map(|(_, characteristics)| {
                        characteristics.primary_name().map(str::to_string)
                    })
                    .collect::<HashSet<_>>()
                    .len(),
            ),
            BattlefieldAggregate::TotalPower => matching
                .iter()
                .filter_map(|(_, characteristics)| characteristics.power)
                .fold(0u32, u32::saturating_add),
            BattlefieldAggregate::MaximumPower => matching
                .iter()
                .filter_map(|(_, characteristics)| characteristics.power)
                .max()
                .unwrap_or(0),
        }
    }

    pub(super) fn battlefield_creature_count(
        &self,
        filter: &tricerules_cards::BattlefieldCreatureCountFilter,
        controller: PlayerId,
        source_object_id: ObjectId,
    ) -> u32 {
        let count = self
            .state
            .players
            .iter()
            .flat_map(|player| player.battlefield.iter().copied())
            .filter(|oid| !filter.exclude_source || *oid != source_object_id)
            .filter_map(|oid| {
                self.characteristics(oid)
                    .map(|characteristics| (oid, characteristics))
            })
            .filter(|(oid, characteristics)| {
                relative_player_set_contains(
                    &self.state,
                    filter.controllers,
                    controller,
                    characteristics.controller,
                ) && characteristics.is_creature()
                    && (!filter.requires_any_counter
                        || self
                            .state
                            .objects
                            .get(oid)
                            .is_some_and(GameObject::has_any_counter))
                    && filter.required_counter.is_none_or(|counter| {
                        self.state
                            .objects
                            .get(oid)
                            .is_some_and(|object| object.counter_count(counter) > 0)
                    })
                    && filter.tapped.is_none_or(|tapped| {
                        self.state
                            .objects
                            .get(oid)
                            .is_some_and(|object| object.tapped == tapped)
                    })
                    && filter
                        .subtype
                        .as_ref()
                        .is_none_or(|subtype| characteristics.has_type(subtype))
                    && filter
                        .required_keywords
                        .iter()
                        .all(|keyword| characteristics.has_keyword(*keyword))
            })
            .count();
        clamp_public_count(count)
    }

    pub(super) fn resolve_amount(&self, amount: &Amount, context: AmountContext<'_>) -> u32 {
        match amount {
            Amount::CastCost(value) => {
                if context
                    .stack_item
                    .is_some_and(|item| item.cast_cost_condition_matches(&value.condition))
                {
                    value.if_selected
                } else {
                    value.otherwise
                }
            }
            Amount::Fixed(value) => *value,
            Amount::X => context.chosen_x,
            Amount::Conditional {
                condition,
                when_true,
                otherwise,
            } => {
                if self.condition_holds(
                    condition,
                    ConditionContext {
                        controller: context.controller,
                        source_object_id: context.source_object_id,
                        source_zone_change: context.source_zone_change,
                        resolving_spell_id: context.resolving_spell_id,
                        stack_item: context.stack_item,
                    },
                ) {
                    *when_true
                } else {
                    *otherwise
                }
            }
            Amount::Count(expression) => self
                .resolve_quantity(expression, context)
                .clamp(0, u32::MAX as i64) as u32,
        }
    }

    /// Signed intermediates are retained until the final nonnegative Amount conversion (CR 107.1b).
    fn resolve_quantity(&self, expression: &CountExpression, context: AmountContext<'_>) -> i64 {
        let condition_context = ConditionContext {
            controller: context.controller,
            source_object_id: context.source_object_id,
            source_zone_change: context.source_zone_change,
            resolving_spell_id: context.resolving_spell_id,
            stack_item: context.stack_item,
        };
        match expression {
            CountExpression::SpellsCastThisTurn {
                players,
                filter,
                exclude_source,
            } => spell_cast_count(
                &self.state,
                *players,
                filter,
                context.controller,
                context.stack_item,
                *exclude_source,
            ) as i64,
            CountExpression::BattlefieldPermanents { .. }
            | CountExpression::BattlefieldMaximum { .. }
            | CountExpression::BattlefieldCreatures { .. } => {
                battlefield_quantity_value(&self.state, expression, condition_context, |oid| {
                    self.characteristics(oid)
                })
                .unwrap_or(0)
            }
            CountExpression::GraveyardCards { owners, filter } => graveyard_aggregate_value(
                &self.state,
                self.registry,
                *owners,
                GraveyardAggregate::CardCount,
                filter.as_ref(),
                context.controller,
                context.resolving_spell_id,
            ) as i64,
            CountExpression::SourcePower => self.source_power_toughness(context).0,
            CountExpression::DeclaredAttackers { players, filter } => self
                .state
                .turn_history
                .current
                .declared_attackers
                .iter()
                .filter(|fact| {
                    relative_player_set_contains(
                        &self.state,
                        *players,
                        context.controller,
                        fact.controller,
                    ) && creature_event_fact_matches(filter, fact)
                })
                .map(|fact| (fact.object_id, fact.zone_change_generation))
                .collect::<HashSet<_>>()
                .len() as i64,
            CountExpression::Affine { constant, terms } => {
                terms.iter().fold(*constant as i64, |value, term| {
                    value.saturating_add(
                        (term.coefficient as i64)
                            .saturating_mul(self.resolve_quantity(&term.quantity, context)),
                    )
                })
            }
            CountExpression::GraveyardCardsNamed { owners, name } => {
                let count = self
                    .state
                    .players
                    .iter()
                    .filter(|player| {
                        relative_player_set_contains(
                            &self.state,
                            *owners,
                            context.controller,
                            player.id,
                        )
                    })
                    .flat_map(|player| player.graveyard.iter().copied())
                    .filter(|oid| Some(*oid) != context.resolving_spell_id)
                    .filter_map(|oid| self.state.objects.get(&oid))
                    .filter(|object| {
                        object.zone == Zone::Graveyard
                            && !object.is_token()
                            && self
                                .registry
                                .get(&object.card_id)
                                .is_some_and(|definition| definition.has_name_outside_stack(name))
                    })
                    .count();
                clamp_public_count(count) as i64
            }
            CountExpression::CreatureDeathsThisTurn => {
                self.state.turn_history.current.creatures_died as i64
            }
            CountExpression::CardsMatchingResult { filter } => context
                .previous_effect_result
                .zip(context.stack_item)
                .map_or(0, |(previous, top)| {
                    super::resolution::card_result_count(self, top, previous, filter)
                }) as i64,
            CountExpression::CardResultCharacteristicSum {
                filter,
                characteristic,
            } => context
                .previous_effect_result
                .zip(context.stack_item)
                .map_or(0, |(previous, top)| {
                    super::resolution::card_result_characteristic_sum(
                        self,
                        top,
                        previous,
                        filter,
                        *characteristic,
                    )
                }),
        }
    }

    /// Read one exact object's current public characteristics, or generation-keyed LKI after it
    /// leaves the battlefield. Missing numerical information is 0 under CR 107.2.
    pub(super) fn object_power_toughness(
        &self,
        oid: u32,
        zone_change_generation: u64,
    ) -> (i64, i64) {
        if self
            .state
            .objects
            .get(&oid)
            .is_some_and(|object| object.zone == Zone::Battlefield)
            && self
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0)
                == zone_change_generation
        {
            return self
                .characteristics(oid)
                .map(|c| (c.signed_power.unwrap_or(0), c.signed_toughness.unwrap_or(0)))
                .unwrap_or((0, 0));
        }
        self.state
            .last_known_pt_by_generation
            .get(&(oid, zone_change_generation))
            .map(|(power, toughness)| (power.unwrap_or(0), toughness.unwrap_or(0)))
            .unwrap_or((0, 0))
    }

    /// CR 608.2h: read the exact source incarnation's current public characteristics, or its
    /// generation-keyed LKI after it leaves. Missing numerical information is 0 under CR 107.2.
    pub(super) fn source_power_toughness(&self, context: AmountContext<'_>) -> (i64, i64) {
        self.object_power_toughness(context.source_object_id, context.source_zone_change)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_189_controller_relative_departure_condition_is_available() {
        let data = r#"(id: "departure_probe", name: "Departure Probe", face_id: "departure_probe", types: ["Instant"],
            cast_conditions: [PermanentLeftBattlefieldThisTurn(controllers: Controller)],
            spell_effect: [GainLife(amount: 1)])"#;
        let registry = CardRegistry::from_chunks_and_tokens(&[data], &[])
            .expect("controller-relative permanent departure condition");
        assert!(matches!(
            registry
                .get("departure_probe")
                .unwrap()
                .primary_face()
                .cast_conditions[0],
            GameCondition::PermanentLeftBattlefieldThisTurn {
                controllers: RelativePlayerSet::Controller
            }
        ));
    }

    #[test]
    fn issue_189_departures_are_committed_controller_relative_and_reset_per_turn() {
        let mut engine = GameEngine::new(
            189_001,
            &[0, 1],
            20,
            Some(vec![
                deck_with_cards(&["grizzly_bears", "serra_angel"], "forest"),
                deck_with_cards(&[], "island"),
            ]),
            true,
        )
        .unwrap();
        engine.state.players.push(PlayerState::new(2, 20));
        let context = ConditionContext {
            controller: 0,
            source_object_id: 0,
            source_zone_change: 0,
            resolving_spell_id: None,
            stack_item: None,
        };
        let holds = |engine: &GameEngine, controllers| {
            engine.condition_holds(
                &GameCondition::PermanentLeftBattlefieldThisTurn { controllers },
                context,
            )
        };

        assert!(!holds(&engine, RelativePlayerSet::All));
        let land = move_to_battlefield(&mut engine, 0, "forest");
        engine
            .commit_observed_zone_move(land, Zone::Hand, None)
            .unwrap();
        assert!(holds(&engine, RelativePlayerSet::Controller));
        assert!(!holds(&engine, RelativePlayerSet::Opponents));

        engine.state.turn_history.finish_turn();
        assert!(!holds(&engine, RelativePlayerSet::All));
        assert!(
            engine
                .state
                .turn_history
                .previous
                .player(0)
                .permanent_left_battlefield
        );

        let token = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        let token_face = engine
            .registry
            .get("grizzly_bears")
            .unwrap()
            .primary_face()
            .clone();
        let token_object = engine.state.objects.get_mut(&token).unwrap();
        token_object.base_controller = 1;
        token_object.controller = 1;
        token_object.token_origin = Some(CopiableValues {
            source_card_id: "grizzly_bears".into(),
            source_face_index: 0,
            face: token_face,
            room_faces: None,
            display_name: "Grizzly Bears".into(),
        });
        engine
            .state
            .death_replacement_effects
            .push(ActiveDeathReplacement {
                object_id: token,
                zone_change_generation: 0,
            });
        engine
            .commit_observed_zone_move(token, Zone::Graveyard, None)
            .unwrap();
        assert_eq!(engine.state.objects[&token].zone, Zone::Exile);
        assert!(!holds(&engine, RelativePlayerSet::Controller));
        assert!(holds(&engine, RelativePlayerSet::Opponents));

        let nontoken = move_to_battlefield(&mut engine, 0, "serra_angel");
        let nontoken_object = engine.state.objects.get_mut(&nontoken).unwrap();
        nontoken_object.base_controller = 2;
        nontoken_object.controller = 2;
        engine
            .commit_observed_zone_move(nontoken, Zone::Graveyard, None)
            .unwrap();
        assert!(
            engine
                .state
                .turn_history
                .current
                .player(2)
                .permanent_left_battlefield
        );
        assert!(holds(&engine, RelativePlayerSet::Opponents));
        assert!(holds(&engine, RelativePlayerSet::All));
    }

    #[test]
    fn issue_168_celebration_excludes_land_entries() {
        let data = r#"(id: "entry_probe", name: "Entry Probe", face_id: "entry_probe", types: ["Instant"],
            cast_conditions: [PermanentsEnteredThisTurn(controllers: Controller,
                filter: (excluded_types: [Land]), min: Some(2))],
            spell_effect: [GainLife(amount: 1)])"#;
        let registry = CardRegistry::from_chunks_and_tokens(&[data], &[]).unwrap();
        let condition = &registry
            .get("entry_probe")
            .unwrap()
            .primary_face()
            .cast_conditions[0];
        let decks = Some(vec![
            deck_with_cards(&[], "forest"),
            deck_with_cards(&[], "island"),
        ]);
        let mut engine = GameEngine::new(168001, &[0, 1], 20, decks, true).unwrap();
        let land = move_to_battlefield(&mut engine, 0, "forest");
        engine.record_committed_events(&[GameEvent::EntersBattlefield { object_id: land }]);
        engine.record_committed_events(&[GameEvent::EntersBattlefield { object_id: land }]);
        let context = ConditionContext {
            controller: 0,
            source_object_id: land,
            source_zone_change: 0,
            resolving_spell_id: None,
            stack_item: None,
        };
        assert!(
            !engine.condition_holds(condition, context),
            "lands must not enable Celebration"
        );
    }

    fn issue_167_condition(
        kind: &str,
        players: &str,
        card_type: &str,
        count: u32,
    ) -> GameCondition {
        let data = format!(
            r#"(id: "history_probe", name: "History Probe", face_id: "history_probe", types: ["Instant"],
            cast_conditions: [{kind}(players: {players}, permanent_type: {card_type}, min: Some({count}), max: Some({count}))],
            spell_effect: [GainLife(amount: 1)])"#
        );
        let registry =
            CardRegistry::from_chunks_and_tokens(&[&data], &[]).expect("history vocabulary");
        registry
            .get("history_probe")
            .unwrap()
            .primary_face()
            .cast_conditions[0]
            .clone()
    }

    fn issue_167_holds(
        engine: &GameEngine,
        kind: &str,
        players: &str,
        card_type: &str,
        count: u32,
    ) -> bool {
        engine.condition_holds(
            &issue_167_condition(kind, players, card_type, count),
            ConditionContext {
                controller: 0,
                source_object_id: 0,
                source_zone_change: 0,
                resolving_spell_id: None,
                stack_item: None,
            },
        )
    }

    #[test]
    fn issue_167_graveyard_entries_remember_occurrences_and_destination_card_types() {
        let mut engine = quantity_engine();
        let bear = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        let land = move_to_battlefield(&mut engine, 0, "island");
        let kind = "PermanentCardsEnteredGraveyardThisTurn";
        assert!(issue_167_holds(&engine, kind, "Controller", "None", 0));
        for oid in [bear, land] {
            super::super::resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Graveyard,
                None,
            )
            .unwrap();
        }
        assert!(issue_167_holds(&engine, kind, "Controller", "None", 2));
        assert!(issue_167_holds(
            &engine,
            kind,
            "Controller",
            "Some(Creature)",
            1
        ));
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            bear,
            Zone::Hand,
            None,
        )
        .unwrap();
        assert!(issue_167_holds(&engine, kind, "Controller", "None", 2));
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            bear,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert!(issue_167_holds(
            &engine,
            kind,
            "Controller",
            "Some(Creature)",
            2
        ));
        assert_eq!(
            engine.state.turn_history.current.creatures_died, 0,
            "entries do not manufacture death events"
        );
        engine.state.turn_history.finish_turn();
        assert!(issue_167_holds(&engine, kind, "Controller", "None", 0));
    }

    #[test]
    fn issue_167_sacrifice_history_is_actor_relative_and_uses_event_types() {
        let mut engine = quantity_engine();
        engine.state.players.push(PlayerState::new(2, 20));
        let bear = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            bear,
            Zone::Battlefield,
            Some(2),
        )
        .unwrap();
        let source = engine.trigger_source_snapshot(bear).unwrap();
        let died =
            super::super::resolution::sacrifice_permanent(&mut engine.state, engine.registry, bear)
                .unwrap();
        engine.fire_triggers(&sacrifice_events(source, true, 2, died));
        let kind = "PermanentsSacrificedThisTurn";
        assert!(issue_167_holds(&engine, kind, "Controller", "None", 0));
        assert!(issue_167_holds(
            &engine,
            kind,
            "Opponents",
            "Some(Creature)",
            1
        ));
        assert!(issue_167_holds(&engine, kind, "All", "Some(Artifact)", 0));
        assert!(issue_167_holds(
            &engine,
            "PermanentCardsEnteredGraveyardThisTurn",
            "Controller",
            "Some(Creature)",
            1
        ));
        assert_eq!(engine.state.turn_history.current.creatures_died, 1);
        engine.state.turn_history.finish_turn();
        assert!(issue_167_holds(&engine, kind, "All", "None", 0));
    }

    #[test]
    fn issue_167_tokens_replacements_and_nonsacrifice_moves_remain_distinct() {
        for token in [false, true] {
            for replaced in [false, true] {
                let mut engine = quantity_engine();
                let bear = move_to_battlefield(&mut engine, 0, "grizzly_bears");
                if token {
                    let face = engine
                        .registry
                        .get("grizzly_bears")
                        .unwrap()
                        .primary_face()
                        .clone();
                    engine.state.objects.get_mut(&bear).unwrap().token_origin =
                        Some(CopiableValues {
                            source_card_id: "grizzly_bears".into(),
                            source_face_index: 0,
                            face,
                            room_faces: None,
                            display_name: "Grizzly Bears".into(),
                        });
                }
                if replaced {
                    engine.state.death_replacement_effects.push(
                        crate::state::ActiveDeathReplacement {
                            object_id: bear,
                            zone_change_generation: 0,
                        },
                    );
                }
                let source = engine.trigger_source_snapshot(bear).unwrap();
                // Prospective trigger collection must not record history.
                engine.collect_event_triggers(&sacrifice_events(
                    source.clone(),
                    true,
                    0,
                    !replaced,
                ));
                assert!(issue_167_holds(
                    &engine,
                    "PermanentsSacrificedThisTurn",
                    "Controller",
                    "None",
                    0
                ));
                let died = super::super::resolution::sacrifice_permanent(
                    &mut engine.state,
                    engine.registry,
                    bear,
                )
                .unwrap();
                engine.fire_triggers(&sacrifice_events(source, true, 0, died));
                assert!(issue_167_holds(
                    &engine,
                    "PermanentsSacrificedThisTurn",
                    "Controller",
                    "Some(Creature)",
                    1
                ));
                assert!(issue_167_holds(
                    &engine,
                    "PermanentCardsEnteredGraveyardThisTurn",
                    "Controller",
                    "None",
                    u32::from(!token && !replaced)
                ));
                assert_eq!(
                    engine.state.turn_history.current.creatures_died,
                    u32::from(!replaced)
                );
                let land = move_to_battlefield(&mut engine, 0, "island");
                super::super::resolution::put_permanent_in_graveyard(
                    &mut engine.state,
                    engine.registry,
                    land,
                )
                .unwrap();
                assert!(
                    issue_167_holds(
                        &engine,
                        "PermanentsSacrificedThisTurn",
                        "Controller",
                        "None",
                        1
                    ),
                    "legend/direct moves are not sacrifices"
                );
            }
        }
    }

    #[test]
    fn issue_167_animation_and_copy_types_do_not_leak_into_destination_history() {
        let mut engine = quantity_engine();
        let land = move_to_battlefield(&mut engine, 0, "island");
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(land),
            kind: ContinuousEffectKind::Layer4AddTypes(tricerules_cards::TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Artifact, PermanentTypeFilter::Creature],
                creature_types: vec![],
            }),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
            trigger_grant_origin: None,
        });
        let source = engine.trigger_source_snapshot(land).unwrap();
        let died =
            super::super::resolution::sacrifice_permanent(&mut engine.state, engine.registry, land)
                .unwrap();
        engine.fire_triggers(&sacrifice_events(source, true, 0, died));
        assert!(issue_167_holds(
            &engine,
            "PermanentsSacrificedThisTurn",
            "Controller",
            "Some(Artifact)",
            1
        ));
        assert!(issue_167_holds(
            &engine,
            "PermanentCardsEnteredGraveyardThisTurn",
            "Controller",
            "Some(Creature)",
            0
        ));
        assert!(issue_167_holds(
            &engine,
            "PermanentCardsEnteredGraveyardThisTurn",
            "Controller",
            "Some(Land)",
            1
        ));

        let bear = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        engine.state.objects.get_mut(&bear).unwrap().copiable_values = Some(CopiableValues {
            source_card_id: "island".into(),
            source_face_index: 0,
            face: engine
                .registry
                .get("island")
                .unwrap()
                .primary_face()
                .clone(),
            room_faces: None,
            display_name: "Island".into(),
        });
        let source = engine.trigger_source_snapshot(bear).unwrap();
        let died =
            super::super::resolution::sacrifice_permanent(&mut engine.state, engine.registry, bear)
                .unwrap();
        engine.fire_triggers(&sacrifice_events(source, false, 0, died));
        assert!(issue_167_holds(
            &engine,
            "PermanentsSacrificedThisTurn",
            "Controller",
            "Some(Land)",
            2
        ));
        assert!(issue_167_holds(
            &engine,
            "PermanentCardsEnteredGraveyardThisTurn",
            "Controller",
            "Some(Creature)",
            1
        ));
        assert_eq!(engine.state.turn_history.current.creatures_died, 1);
    }

    #[test]
    fn issue_167_static_conditions_share_live_history_queries() {
        let mut engine = quantity_engine();
        let bear = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        let land = move_to_battlefield(&mut engine, 0, "island");
        for kind in [
            "PermanentCardsEnteredGraveyardThisTurn",
            "PermanentsSacrificedThisTurn",
        ] {
            engine.state.continuous_effects.push(ContinuousEffect {
                source_id: Some(bear),
                affected: AffectedScope::Single(bear),
                kind: ContinuousEffectKind::PtModify {
                    delta_power: 1,
                    delta_toughness: 0,
                },
                condition: Some(issue_167_condition(kind, "Controller", "None", 1)),
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 1,
                trigger_grant_origin: None,
            });
        }
        assert_eq!(engine.effective_power(bear), Some(2));
        let source = engine.trigger_source_snapshot(land).unwrap();
        let died =
            super::super::resolution::sacrifice_permanent(&mut engine.state, engine.registry, land)
                .unwrap();
        engine.fire_triggers(&sacrifice_events(source, false, 0, died));
        assert_eq!(engine.effective_power(bear), Some(4));
        engine.state.turn_history.finish_turn();
        assert_eq!(engine.effective_power(bear), Some(2));
    }

    fn quantity_context(source: ObjectId) -> AmountContext<'static> {
        AmountContext {
            stack_item: None,
            controller: 0,
            source_object_id: source,
            source_zone_change: 0,
            resolving_spell_id: None,
            chosen_x: 0,
            previous_effect_result: None,
        }
    }

    fn quantity_engine() -> GameEngine {
        GameEngine::new(
            165_001,
            &[0, 1],
            20,
            Some(vec![
                deck_with_cards(&["grizzly_bears", "serra_angel"], "island"),
                deck_with_cards(&[], "forest"),
            ]),
            true,
        )
        .unwrap()
    }

    #[test]
    fn issue_165_maxima_use_derived_signed_values_and_all_opponents() {
        let mut engine = quantity_engine();
        let mut filter = BattlefieldPermanentFilter {
            token: None,
            any_of: None,
            controllers: RelativePlayerSet::Controller,
            card_type: Some(CardTypeFilter::Creature),
            color: None,
            name: None,
            required_subtypes: vec![],
            exclude_source: false,
        };
        let maximum = |filter, characteristic| {
            Amount::Count(CountExpression::BattlefieldMaximum {
                filter,
                characteristic,
            })
        };
        use tricerules_cards::PowerToughnessCharacteristic::{Power, Toughness};
        assert_eq!(
            engine.resolve_amount(&maximum(filter.clone(), Power), quantity_context(0)),
            0
        );
        let source = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        let other = move_to_battlefield(&mut engine, 0, "serra_angel");
        engine.state.objects.get_mut(&source).unwrap().toughness = Some(7);
        assert_eq!(
            engine.resolve_amount(&maximum(filter.clone(), Power), quantity_context(source)),
            4
        );
        assert_eq!(
            engine.resolve_amount(
                &maximum(filter.clone(), Toughness),
                quantity_context(source)
            ),
            7
        );
        for oid in [source, other] {
            engine.state.continuous_effects.push(ContinuousEffect {
                source_id: None,
                affected: AffectedScope::Single(oid),
                kind: ContinuousEffectKind::PtModify {
                    delta_power: -5,
                    delta_toughness: 0,
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 1,
                trigger_grant_origin: None,
            });
        }
        let quantity = CountExpression::BattlefieldMaximum {
            filter: filter.clone(),
            characteristic: Power,
        };
        assert_eq!(
            engine.resolve_amount(&Amount::Count(quantity.clone()), quantity_context(source)),
            0
        );
        assert_eq!(
            engine.resolve_amount(
                &Amount::Count(CountExpression::Affine {
                    constant: 2,
                    terms: vec![tricerules_cards::QuantityTerm {
                        coefficient: -1,
                        quantity
                    }]
                }),
                quantity_context(source)
            ),
            3,
            "max(-3, -1) remains signed until the final amount"
        );
        engine.state.players.push(PlayerState::new(19, 20));
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            other,
            Zone::Hand,
            None,
        )
        .unwrap();
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            other,
            Zone::Battlefield,
            Some(19),
        )
        .unwrap();
        filter.controllers = RelativePlayerSet::Opponents;
        assert_eq!(
            engine.resolve_amount(&maximum(filter, Toughness), quantity_context(source)),
            4
        );
    }

    #[test]
    fn issue_165_cave_and_permanent_graveyard_counts_use_printed_cards_once() {
        let mut engine = quantity_engine();
        let source = move_to_graveyard(&mut engine, 0, "grizzly_bears");
        let cave = move_to_graveyard(&mut engine, 0, "island");
        let artifact = move_to_graveyard(&mut engine, 0, "island");
        let copied = move_to_graveyard(&mut engine, 0, "island");
        let token = move_to_graveyard(&mut engine, 0, "island");
        let instant = move_to_graveyard(&mut engine, 0, "island");
        let opponent = move_to_graveyard(&mut engine, 1, "forest");
        let registry = Box::leak(Box::new(CardRegistry::from_chunks_and_tokens(&[
            r#"(id: "test_cave", name: "Test Cave", face_id: "test_cave", types: ["Land", "Cave"])"#,
            r#"(id: "test_artifact", name: "Test Artifact", face_id: "test_artifact", types: ["Artifact", "Creature"], power: 1, toughness: 1)"#,
            r#"(id: "test_instant", name: "Test Instant", face_id: "test_instant", types: ["Instant"] , spell_effect: [GainLife(amount: 1)])"#,
            include_str!("../../../tricerules-cards/data/island.ron"),
            include_str!("../../../tricerules-cards/data/grizzly_bears.ron"),
        ], &[]).unwrap()));
        let values = CopiableValues {
            source_card_id: "test_cave".into(),
            source_face_index: 0,
            face: registry.get("test_cave").unwrap().primary_face().clone(),
            room_faces: None,
            display_name: "Test Cave".into(),
        };
        for oid in [cave, opponent] {
            engine.state.objects.get_mut(&oid).unwrap().card_id = "test_cave".into();
        }
        engine.state.objects.get_mut(&artifact).unwrap().card_id = "test_artifact".into();
        engine.state.objects.get_mut(&instant).unwrap().card_id = "test_instant".into();
        engine
            .state
            .objects
            .get_mut(&copied)
            .unwrap()
            .copiable_values = Some(values.clone());
        engine.state.objects.get_mut(&token).unwrap().token_origin = Some(values);
        engine.registry = registry;
        let global = CardRegistry::global();
        let SpellEffectKind::DamageAll { amount, .. } = &global
            .get("calamitous_cave-in")
            .unwrap()
            .primary_face()
            .spell_effect[0]
        else {
            panic!("Cave-In amount");
        };
        assert_eq!(
            engine.resolve_amount(amount, quantity_context(source)),
            1,
            "only the printed Cave in our graveyard counts"
        );
        let SpellEffectKind::PumpTarget {
            scale: Some(scale), ..
        } = &global
            .get("chupacabra_echo")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .effect[0]
        else {
            panic!("Echo amount");
        };
        let PtScaleBasis::Amount(amount) = &scale.basis else {
            panic!("Echo uses an ordinary amount basis");
        };
        assert_eq!(
            engine.resolve_amount(amount, quantity_context(source)),
            4,
            "artifact creature counts once, tokens and instants never count"
        );
    }

    #[test]
    fn issue_165_source_quantity_retains_signed_intermediates() {
        let mut engine = quantity_engine();
        let source = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::PtModify {
                delta_power: -5,
                delta_toughness: 0,
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
            trigger_grant_origin: None,
        });
        let amount = Amount::Count(CountExpression::Affine {
            constant: 2,
            terms: vec![tricerules_cards::QuantityTerm {
                coefficient: -1,
                quantity: CountExpression::SourcePower,
            }],
        });
        assert_eq!(engine.resolve_amount(&amount, quantity_context(source)), 5);
        assert_eq!(
            engine.resolve_amount(
                &Amount::Count(CountExpression::SourcePower),
                quantity_context(source)
            ),
            0
        );
    }

    #[test]
    fn issue_165_source_quantity_uses_departure_not_returned_generation() {
        let mut engine = quantity_engine();
        let source = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::PtModify {
                delta_power: 3,
                delta_toughness: 0,
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
            trigger_grant_origin: None,
        });
        let amount = Amount::Count(CountExpression::SourcePower);
        let context = quantity_context(source);
        assert_eq!(engine.resolve_amount(&amount, context), 5);
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Hand,
            None,
        )
        .unwrap();
        assert_eq!(
            engine.resolve_amount(&amount, context),
            5,
            "departure snapshots include modifiers"
        );
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Battlefield,
            Some(1),
        )
        .unwrap();
        engine.state.objects.get_mut(&source).unwrap().power = Some(9);
        assert_eq!(
            engine.resolve_amount(&amount, context),
            5,
            "a new generation cannot supply the old trigger's power"
        );
    }

    #[test]
    fn issue_165_simultaneous_departures_retain_pre_move_power() {
        let mut engine = quantity_engine();
        let source = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        let grantor = move_to_battlefield(&mut engine, 0, "serra_angel");
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: Some(grantor),
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::PtModify {
                delta_power: 3,
                delta_toughness: 0,
            },
            condition: None,
            duration: EffectDuration::WhileSourceOnBattlefield,
            timestamp: 1,
            trigger_grant_origin: None,
        });
        let events: Vec<_> = [grantor, source]
            .into_iter()
            .map(|oid| engine.battlefield_leave_event(oid).unwrap())
            .collect();
        for oid in [grantor, source] {
            super::super::resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Graveyard,
                None,
            )
            .unwrap();
        }
        engine.record_committed_events(&events);
        assert_eq!(
            engine.resolve_amount(
                &Amount::Count(CountExpression::SourcePower),
                quantity_context(source)
            ),
            5
        );
    }

    #[test]
    fn issue_165_public_cohorts_sum_deduplicate_and_exclude_resolving_card() {
        let mut engine = quantity_engine();
        let source = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        move_to_battlefield(&mut engine, 0, "island");
        move_to_battlefield(&mut engine, 1, "forest");
        let graveyard_card = move_to_graveyard(&mut engine, 0, "island");
        let filter = BattlefieldPermanentFilter {
            token: None,
            any_of: None,
            controllers: RelativePlayerSet::Controller,
            card_type: Some(CardTypeFilter::Land),
            color: None,
            name: None,
            required_subtypes: vec![],
            exclude_source: false,
        };
        let mut union = filter.clone();
        union.any_of = Some(vec![filter.clone(), filter]);
        let battlefield = CountExpression::BattlefieldPermanents { filter: union };
        let graveyard = CountExpression::GraveyardCards {
            owners: RelativePlayerSet::Controller,
            filter: None,
        };
        let amount = Amount::Count(CountExpression::Affine {
            constant: 0,
            terms: vec![
                tricerules_cards::QuantityTerm {
                    coefficient: 1,
                    quantity: battlefield,
                },
                tricerules_cards::QuantityTerm {
                    coefficient: 1,
                    quantity: graveyard,
                },
            ],
        });
        let mut context = quantity_context(source);
        assert_eq!(engine.resolve_amount(&amount, context), 2);
        context.resolving_spell_id = Some(graveyard_card);
        assert_eq!(engine.resolve_amount(&amount, context), 1);
        engine.state.turn_history.current.declared_attackers = vec![
            TurnObjectFact {
                object_id: source,
                zone_change_generation: 0,
                controller: 0,
                types: vec!["Creature".into()],
                owner: 0,
                is_token: false,
                all_creature_types: false,
                keywords: vec![],
                power: Some(2)
            };
            2
        ];
        let attackers = Amount::Count(CountExpression::DeclaredAttackers {
            players: RelativePlayerSet::All,
            filter: Default::default(),
        });
        assert_eq!(
            engine.resolve_amount(&attackers, context),
            1,
            "repeat combat does not duplicate one creature"
        );
        engine.state.turn_history.finish_turn();
        assert_eq!(engine.resolve_amount(&attackers, context), 0);
    }

    #[test]
    fn issue_172_new_turn_record_resets_spending_even_for_the_same_active_player() {
        let mut engine = GameEngine::new(172030, &[0, 1], 20, None, true).unwrap();
        let player = engine.state.active_player_id();
        engine.record_spell_mana_spent(player, 8);
        // Exercise the shared rollover with a consecutive turn for the same seat. This does not
        // implement extra-turn scheduling; it proves the ledger has no player-change dependency.
        engine.state.turn_history.finish_turn();
        engine.state.turn_instance += 1;
        assert_eq!(
            engine
                .state
                .turn_history
                .previous
                .player(player)
                .mana_spent_casting_spells,
            8
        );
        assert!(matches!(
            engine.record_spell_mana_spent(player, 4),
            GameEvent::ManaSpentCastingSpell {
                before: 0,
                after: 4,
                ..
            }
        ));
    }

    fn deck_with_cards(cards: &[&str], basic: &str) -> Vec<String> {
        let mut deck: Vec<_> = cards.iter().map(|card| (*card).to_string()).collect();
        while deck.len() < 20 {
            deck.push(basic.to_string());
        }
        deck
    }

    fn move_to_battlefield(engine: &mut GameEngine, player: usize, card_id: &str) -> ObjectId {
        let player_id = engine.state.players[player].id;
        let oid = engine
            .state
            .objects
            .values()
            .find(|object| {
                object.owner == player_id
                    && object.card_id == card_id
                    && object.zone != Zone::Battlefield
            })
            .expect("deck object")
            .id;
        engine.state.players[player]
            .hand
            .retain(|candidate| *candidate != oid);
        engine.state.players[player]
            .library
            .retain(|candidate| *candidate != oid);
        engine.state.players[player].battlefield.push(oid);
        let object = engine.state.objects.get_mut(&oid).expect("object");
        object.zone = Zone::Battlefield;
        object.summoning_sick = false;
        oid
    }

    fn move_to_graveyard(engine: &mut GameEngine, player: usize, card_id: &str) -> ObjectId {
        let player_id = engine.state.players[player].id;
        let oid = engine
            .state
            .objects
            .values()
            .find(|object| {
                object.owner == player_id
                    && object.card_id == card_id
                    && object.zone != Zone::Graveyard
            })
            .expect("deck object")
            .id;
        engine.state.players[player]
            .hand
            .retain(|candidate| *candidate != oid);
        engine.state.players[player]
            .library
            .retain(|candidate| *candidate != oid);
        engine.state.players[player].graveyard.push(oid);
        engine.state.objects.get_mut(&oid).expect("object").zone = Zone::Graveyard;
        oid
    }

    #[test]
    fn public_counts_saturate_at_the_wire_sized_amount_limit() {
        assert_eq!(clamp_public_count(7), 7);
        if usize::BITS > u32::BITS {
            assert_eq!(clamp_public_count(usize::MAX), u32::MAX);
        }
    }

    #[test]
    fn issue_171_resolution_sacrifice_can_require_the_exact_source() {
        let mut engine = GameEngine::new(
            171_002,
            &[0, 1],
            20,
            Some(vec![
                deck_with_cards(&["grizzly_bears", "hill_giant"], "forest"),
                deck_with_cards(&[], "island"),
            ]),
            true,
        )
        .unwrap();
        let source = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        move_to_battlefield(&mut engine, 0, "hill_giant");
        let registry = CardRegistry::from_chunks_and_tokens(&[r#"(id: "test", name: "Test", face_id: "test", types: ["Creature"], power: 1, toughness: 1,
            triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield, effect: [ChooseResolutionBranch(optional: true,
                branches: [(branch_id: "sacrifice_this", presentation: Fallback, cost: SacrificePermanent(filter: (kind: AnyPermanent, controller: You), source_only: true), effects: [Draw(count: 1)])])])])"#], &[]).unwrap();
        let SpellEffectKind::ChooseResolutionBranch { branches, .. } = &registry
            .get("test")
            .unwrap()
            .primary_face()
            .triggered_abilities[0]
            .effect[0]
        else {
            panic!("branch")
        };
        assert_eq!(
            engine.resolution_cost_candidates(0, source, 0, &branches[0].cost),
            vec![source]
        );
        engine.state.zone_change_generation.insert(source, 1);
        assert!(
            engine
                .resolution_cost_candidates(0, source, 0, &branches[0].cost)
                .is_empty(),
            "a returned source is a different object"
        );
    }

    #[test]
    fn issue_171_classification_uses_target_kind_controller_and_graveyard_owner() {
        let mut e = GameEngine::new(
            171_020,
            &[0, 1],
            20,
            Some(vec![
                deck_with_cards(&["grizzly_bears"], "forest"),
                deck_with_cards(&["hill_giant", "storm_crow"], "mountain"),
            ]),
            true,
        )
        .unwrap();
        // Lobby creation is two-seat-only; exercise the engine's player-set contract directly.
        e.state.players.push(crate::state::PlayerState::new(2, 20));
        let ours = move_to_battlefield(&mut e, 0, "grizzly_bears");
        let theirs = move_to_battlefield(&mut e, 1, "hill_giant");
        let grave = move_to_graveyard(&mut e, 1, "storm_crow");
        e.state.players[1].graveyard.retain(|id| *id != grave);
        e.state.players[2].graveyard.push(grave);
        e.state.objects.get_mut(&grave).unwrap().owner = 2;
        // Printed ownership is irrelevant on the battlefield; control is irrelevant in a graveyard.
        e.state.objects.get_mut(&ours).unwrap().owner = 2;
        e.state.objects.get_mut(&theirs).unwrap().owner = 0;
        e.state.objects.get_mut(&grave).unwrap().controller = 0;
        let virtual_id = e.state.next_object_id;
        e.state.stack.push(StackItem {
            id: virtual_id,
            controller: 2,
            card_id: "prodigal_pyromancer".into(),
            targets: vec![],
            ability_text: Some("Test ability".into()),
            source_permanent_id: Some(ours),
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: Some(0),
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            sneak_attack: None,
            chosen_x: 0,
            chosen_modes: vec![],
            cast_cost_receipts: vec![],
            cast_condition_results: vec![],
            cast_occurrence: None,
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        });
        for (kind, oid, expected) in [
            (rv1::TargetRefKind::Player, 0, false),
            (rv1::TargetRefKind::Player, 1, true),
            (rv1::TargetRefKind::Player, 2, true),
            (rv1::TargetRefKind::Permanent, ours, false),
            (rv1::TargetRefKind::Permanent, theirs, true),
            (rv1::TargetRefKind::Graveyard, grave, true),
            (rv1::TargetRefKind::Stack, virtual_id, true),
            (rv1::TargetRefKind::Permanent, virtual_id, false),
            (rv1::TargetRefKind::Graveyard, theirs, false),
            (rv1::TargetRefKind::Player, theirs, false),
            (rv1::TargetRefKind::Unspecified, virtual_id, true),
        ] {
            let targets = vec![
                rv1::TargetRef {
                    object_id: oid,
                    kind: kind as i32,
                    ..Default::default()
                };
                3
            ];
            assert_eq!(
                e.crime_event(0, &targets).is_some(),
                expected,
                "{kind:?} {oid}"
            );
        }
        assert_eq!(
            e.state.turn_history.current.player(0).crimes_committed,
            0,
            "classification is read-only"
        );
        assert!(e.crime_event(0, &[]).is_none());
        assert!(e
            .crime_event(
                0,
                &[rv1::TargetRef {
                    object_id: 1,
                    kind: 999,
                    ..Default::default()
                }]
            )
            .is_none());
        e.state.stack[0].controller = 0;
        assert!(e
            .crime_event(
                0,
                &[rv1::TargetRef {
                    object_id: virtual_id,
                    kind: rv1::TargetRefKind::Stack as i32,
                    ..Default::default()
                }]
            )
            .is_none());
    }

    #[test]
    fn issue_170_life_conditions_keep_each_players_gain_and_loss_separate() {
        let mut e = GameEngine::new(170003, &[10, 20], 20, None, true).unwrap();
        e.state.players.push(PlayerState::new(30, 20));
        commit_life_change(&mut e.state, 0, 2);
        commit_life_change(&mut e.state, 0, -2);
        commit_life_change(&mut e.state, 1, 2);
        commit_life_change(&mut e.state, 1, -1);
        let context = ConditionContext {
            controller: 10,
            source_object_id: 0,
            source_zone_change: 0,
            resolving_spell_id: None,
            stack_item: None,
        };
        for (players, change, quantifier, expected) in [
            (
                RelativePlayerSet::Controller,
                LifeChangeKind::Gain,
                PlayerQuantifier::Any,
                true,
            ),
            (
                RelativePlayerSet::Controller,
                LifeChangeKind::Loss,
                PlayerQuantifier::All,
                true,
            ),
            (
                RelativePlayerSet::Controller,
                LifeChangeKind::Either,
                PlayerQuantifier::Any,
                true,
            ),
            (
                RelativePlayerSet::Opponents,
                LifeChangeKind::Loss,
                PlayerQuantifier::Any,
                true,
            ),
            (
                RelativePlayerSet::Opponents,
                LifeChangeKind::Loss,
                PlayerQuantifier::All,
                false,
            ),
            (
                RelativePlayerSet::All,
                LifeChangeKind::Either,
                PlayerQuantifier::All,
                false,
            ),
        ] {
            assert_eq!(
                e.condition_holds(
                    &GameCondition::LifeChangedThisTurn {
                        players: ConditionPlayerSet::Relative(players),
                        change,
                        quantifier,
                    },
                    context
                ),
                expected,
                "{players:?} {change:?} {quantifier:?}"
            );
        }
        assert_eq!(e.state.players[0].life, 20);
        assert_eq!(e.state.players[1].life, 21);
        assert_eq!(e.state.turn_history.current.player(10).life_gained, 2);
        assert_eq!(e.state.turn_history.current.player(10).life_lost, 2);
        e.state.turn_history.finish_turn();
        assert!(!e.condition_holds(
            &GameCondition::LifeChangedThisTurn {
                players: ConditionPlayerSet::Relative(RelativePlayerSet::All),
                change: LifeChangeKind::Either,
                quantifier: PlayerQuantifier::Any,
            },
            context
        ));
    }

    #[test]
    fn issue_170_chosen_player_and_empty_sets_fail_closed() {
        let mut e = GameEngine::new(170004, &[10, 20], 20, None, true).unwrap();
        e.state.players.push(PlayerState::new(30, 20));
        commit_life_change(&mut e.state, 1, -1);
        let selector = ConditionPlayerSet::ChosenTarget {
            group_index: 2,
            target_index: 0,
        };
        let valid = StackTarget {
            object_id: 20,
            group_index: 2,
            damage_amount: 0,
            kind: rv1::TargetRefKind::Player as i32,
            zone_change_generation: None,
        };
        for (target, expected) in [
            (valid, true),
            (
                StackTarget {
                    kind: rv1::TargetRefKind::Unspecified as i32,
                    ..valid
                },
                true,
            ),
            (
                StackTarget {
                    object_id: 30,
                    ..valid
                },
                false,
            ),
            (
                StackTarget {
                    object_id: 999,
                    ..valid
                },
                false,
            ),
            (
                StackTarget {
                    group_index: 1,
                    ..valid
                },
                false,
            ),
            (
                StackTarget {
                    kind: rv1::TargetRefKind::Permanent as i32,
                    ..valid
                },
                false,
            ),
            (
                StackTarget {
                    zone_change_generation: Some(0),
                    ..valid
                },
                false,
            ),
        ] {
            assert_eq!(
                life_changed_this_turn(
                    &e.state,
                    selector,
                    LifeChangeKind::Loss,
                    PlayerQuantifier::Any,
                    10,
                    Some(&[target]),
                    None,
                ),
                expected
            );
        }
        for targets in [None, Some([].as_slice())] {
            assert!(!life_changed_this_turn(
                &e.state,
                selector,
                LifeChangeKind::Loss,
                PlayerQuantifier::All,
                10,
                targets,
                None,
            ));
        }
        e.state.players[1].has_lost = true;
        assert!(!life_changed_this_turn(
            &e.state,
            selector,
            LifeChangeKind::Loss,
            PlayerQuantifier::Any,
            10,
            Some(&[valid]),
            None,
        ));
        e.state.players.truncate(1);
        for quantifier in [PlayerQuantifier::Any, PlayerQuantifier::All] {
            assert!(!life_changed_this_turn(
                &e.state,
                ConditionPlayerSet::Relative(RelativePlayerSet::Opponents),
                LifeChangeKind::Either,
                quantifier,
                10,
                None,
                None,
            ));
        }
    }

    #[test]
    fn issue_170_gecko_rechecks_opponents_but_keeps_its_trigger_controller() {
        for departure in ["none", "source", "control", "opponent"] {
            let mut e = GameEngine::new(
                170005,
                &[0, 1],
                20,
                Some(vec![
                    deck_with_cards(&["flamecache_gecko"], "mountain"),
                    deck_with_cards(&[], "island"),
                ]),
                true,
            )
            .unwrap();
            e.state.players.push(PlayerState::new(2, 20));
            // Gain two / lose one remains loss, even though this opponent is above starting life.
            commit_life_change(&mut e.state, 1, 2);
            commit_life_change(&mut e.state, 1, -1);
            let source = move_to_battlefield(&mut e, 0, "flamecache_gecko");
            e.fire_triggers(&[GameEvent::EntersBattlefield { object_id: source }]);
            let mut events = Vec::new();
            e.flush_staged_triggers(&mut events);
            assert_eq!(e.state.stack.len(), 1);
            assert_eq!(e.state.stack[0].controller, 0);
            match departure {
                "source" => {
                    super::super::resolution::move_object_to_zone(
                        &mut e.state,
                        e.registry,
                        source,
                        Zone::Graveyard,
                        None,
                    )
                    .unwrap();
                }
                "control" => {
                    let object = e.state.objects.get_mut(&source).unwrap();
                    object.base_controller = 1;
                    object.controller = 1;
                    e.state.players[0].battlefield.retain(|oid| *oid != source);
                    e.state.players[1].battlefield.push(source);
                }
                "opponent" => e.state.players[1].has_lost = true,
                _ => {}
            }
            e.resolve_top_of_stack(&mut events).unwrap();
            let expected = u32::from(departure != "opponent");
            assert_eq!(e.state.players[0].mana_pool.black, expected, "{departure}");
            assert_eq!(e.state.players[0].mana_pool.red, expected, "{departure}");
            assert_eq!(e.state.players[1].mana_pool.red, 0);
            assert_eq!(e.state.turn_history.current.player(1).life_lost, 1);
        }
    }

    #[test]
    fn issue_170_continuous_conditions_share_life_history_and_live_controller() {
        let mut e = GameEngine::new(
            170006,
            &[0, 1],
            20,
            Some(vec![
                deck_with_cards(&["grizzly_bears"], "forest"),
                deck_with_cards(&[], "island"),
            ]),
            true,
        )
        .unwrap();
        let source = move_to_battlefield(&mut e, 0, "grizzly_bears");
        e.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: Some(source),
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Flying),
            condition: Some(GameCondition::LifeChangedThisTurn {
                players: ConditionPlayerSet::Relative(RelativePlayerSet::Controller),
                change: LifeChangeKind::Either,
                quantifier: PlayerQuantifier::Any,
            }),
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });
        assert!(!e
            .characteristics(source)
            .unwrap()
            .keywords
            .contains(&Keyword::Flying));
        commit_life_change(&mut e.state, 0, -1);
        assert!(e
            .characteristics(source)
            .unwrap()
            .keywords
            .contains(&Keyword::Flying));
        e.state.objects.get_mut(&source).unwrap().base_controller = 1;
        assert!(!e
            .characteristics(source)
            .unwrap()
            .keywords
            .contains(&Keyword::Flying));
        commit_life_change(&mut e.state, 1, 1);
        assert!(e
            .characteristics(source)
            .unwrap()
            .keywords
            .contains(&Keyword::Flying));
        e.state.turn_history.finish_turn();
        assert!(!e
            .characteristics(source)
            .unwrap()
            .keywords
            .contains(&Keyword::Flying));
    }

    #[test]
    fn issue_171_history_conditions_count_each_commit_and_reset_at_turn_boundary() {
        let mut e = GameEngine::new(171_021, &[0, 1], 20, None, true).unwrap();
        e.state.players.push(crate::state::PlayerState::new(2, 20));
        e.record_committed_events(&[
            GameEvent::CrimeCommitted { player: 0 },
            GameEvent::CrimeCommitted { player: 1 },
            GameEvent::CrimeCommitted { player: 2 },
        ]);
        let context = ConditionContext {
            controller: 0,
            source_object_id: 0,
            source_zone_change: 0,
            resolving_spell_id: None,
            stack_item: None,
        };
        for (players, count) in [
            (RelativePlayerSet::Controller, 1),
            (RelativePlayerSet::Opponents, 2),
            (RelativePlayerSet::All, 3),
        ] {
            assert!(e.condition_holds(
                &GameCondition::CrimesCommittedThisTurn {
                    players,
                    min: Some(count),
                    max: Some(count)
                },
                context
            ));
        }
        let condition = GameCondition::CrimesCommittedThisTurn {
            players: RelativePlayerSet::Controller,
            min: Some(1),
            max: None,
        };
        e.state.turn_history.finish_turn();
        e.state.turn_instance += 1; // same active player is also how an extra turn starts
        assert!(!e.condition_holds(&condition, context));
        assert_eq!(e.state.turn_history.previous.player(0).crimes_committed, 1);
        e.record_committed_events(&[GameEvent::CrimeCommitted { player: 0 }]);
        assert!(e.condition_holds(&condition, context));
    }

    #[test]
    fn issue_158_committed_entry_draw_and_damage_facts_are_generation_aware() {
        let decks = Some(vec![
            deck_with_cards(&["ornithopter"], "forest"),
            deck_with_cards(&[], "island"),
        ]);
        let mut engine = GameEngine::new(158_001, &[0, 1], 20, decks, true).expect("engine");
        let artifact = move_to_battlefield(&mut engine, 0, "ornithopter");
        let generation = engine
            .state
            .zone_change_generation
            .get(&artifact)
            .copied()
            .unwrap_or(0);
        engine.record_committed_events(&[
            GameEvent::EntersBattlefield {
                object_id: artifact,
            },
            GameEvent::CardDrawn {
                drawer: 0,
                ordinal: 1,
            },
            GameEvent::DamageDealt {
                event: damage::DamageEvent::noncombat(
                    artifact,
                    0,
                    "Ornithopter",
                    damage::DamageRecipient::Permanent(artifact),
                    1,
                ),
            },
            GameEvent::AttackersDeclared {
                attacking_player: 0,
                attacks: vec![AttackEdgeSnapshot {
                    attacker: TriggerObjectRef {
                        object_id: artifact,
                        zone_change_generation: generation,
                        controller_at_event: 0,
                    },
                    defender: CombatDefenderTarget::Player(1),
                    defending_player: 1,
                }],
            },
        ]);
        let context = ConditionContext {
            controller: 0,
            source_object_id: artifact,
            source_zone_change: generation,
            resolving_spell_id: None,
            stack_item: None,
        };
        assert!(engine.condition_holds(
            &GameCondition::CardsDrawnThisTurn {
                players: RelativePlayerSet::Controller,
                min: Some(1),
                max: None,
            },
            context,
        ));
        assert!(engine.condition_holds(
            &GameCondition::PermanentsEnteredThisTurn {
                controllers: RelativePlayerSet::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(PermanentTypeFilter::Artifact),
                    required_subtypes: vec![],
                    exclude_source: false,
                    ..Default::default()
                },
                min: Some(1),
                max: None,
            },
            context,
        ));
        assert!(engine.condition_holds(
            &GameCondition::AttackersDeclaredThisTurn {
                players: RelativePlayerSet::Controller,
                filter: CreatureEventFilter {
                    required_subtypes: vec!["Thopter".into()],
                    ..Default::default()
                },
                min: Some(1),
                max: None,
            },
            context,
        ));
        engine
            .state
            .objects
            .get_mut(&artifact)
            .expect("Ornithopter")
            .add_counters(CounterKind::PlusOnePlusOne, 2, 1);
        assert!(engine.condition_holds(
            &GameCondition::SourceCounterCount {
                counter: CounterKind::PlusOnePlusOne,
                min: Some(2),
                max: Some(2),
            },
            context,
        ));
        assert!(engine.condition_holds(
            &GameCondition::ObjectWasDealtDamageThisTurn {
                object: ConditionObjectRef::Source,
            },
            context,
        ));

        engine
            .state
            .zone_change_generation
            .insert(artifact, generation + 1);
        let returned_context = ConditionContext {
            source_zone_change: generation + 1,
            ..context
        };
        assert!(!engine.condition_holds(
            &GameCondition::ObjectWasDealtDamageThisTurn {
                object: ConditionObjectRef::Source,
            },
            returned_context,
        ));

        engine.state.turn_history.finish_turn();
        assert!(!engine.condition_holds(
            &GameCondition::CardsDrawnThisTurn {
                players: RelativePlayerSet::Controller,
                min: Some(1),
                max: None,
            },
            returned_context,
        ));
    }

    #[test]
    fn issue_158_filtered_aggregates_deduplicate_unions_and_names() {
        let decks = Some(vec![
            deck_with_cards(
                &["forest", "forest", "island", "azula_always_lies", "shock"],
                "plains",
            ),
            deck_with_cards(&[], "mountain"),
        ]);
        let mut engine = GameEngine::new(158_002, &[0, 1], 20, decks, true).expect("engine");
        move_to_battlefield(&mut engine, 0, "forest");
        move_to_battlefield(&mut engine, 0, "forest");
        move_to_battlefield(&mut engine, 0, "island");
        move_to_graveyard(&mut engine, 0, "azula_always_lies");
        move_to_graveyard(&mut engine, 0, "shock");
        let context = ConditionContext {
            controller: 0,
            source_object_id: 0,
            source_zone_change: 0,
            resolving_spell_id: None,
            stack_item: None,
        };

        let union = BattlefieldPermanentFilter {
            token: None,
            any_of: Some(vec![
                BattlefieldPermanentFilter {
                    token: None,
                    any_of: None,
                    controllers: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::Land),
                    color: None,
                    name: None,
                    required_subtypes: vec![],
                    exclude_source: false,
                },
                BattlefieldPermanentFilter {
                    token: None,
                    any_of: None,
                    controllers: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::BasicLand),
                    color: None,
                    name: None,
                    required_subtypes: vec![],
                    exclude_source: false,
                },
            ]),
            controllers: RelativePlayerSet::Controller,
            card_type: None,
            color: None,
            name: None,
            required_subtypes: vec![],
            exclude_source: false,
        };
        assert_eq!(
            engine.battlefield_aggregate_value(&union, BattlefieldAggregate::Count, context),
            3,
            "each basic land matches both branches but contributes only once"
        );
        assert_eq!(
            engine.battlefield_aggregate_value(
                &union,
                BattlefieldAggregate::DistinctNames,
                context,
            ),
            2
        );

        assert_eq!(
            graveyard_aggregate_value(
                &engine.state,
                engine.registry,
                RelativePlayerSet::Controller,
                GraveyardAggregate::CardCount,
                Some(&ZoneCardFilter {
                    subtype: Some("Lesson".into()),
                    ..Default::default()
                }),
                0,
                None,
            ),
            1
        );
    }

    #[test]
    fn battlefield_creature_count_requires_any_live_counter_kind() {
        let decks = Some(vec![
            deck_with_cards(&["grizzly_bears", "serra_angel"], "forest"),
            deck_with_cards(&["grizzly_bears"], "island"),
        ]);
        let mut engine = GameEngine::new(124_001, &[0, 1], 20, decks, true).expect("engine");
        let bear = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        let _angel = move_to_battlefield(&mut engine, 0, "serra_angel");
        let opposing_bear = move_to_battlefield(&mut engine, 1, "grizzly_bears");
        let filter = tricerules_cards::BattlefieldCreatureCountFilter {
            controllers: RelativePlayerSet::Controller,
            subtype: None,
            required_keywords: vec![],
            tapped: None,
            requires_any_counter: true,
            required_counter: None,
            exclude_source: false,
        };
        let condition = GameCondition::BattlefieldCreatureCount {
            filter: filter.clone(),
            min: Some(1),
            max: None,
        };
        let context = ConditionContext {
            controller: 0,
            source_object_id: 0,
            source_zone_change: 0,
            resolving_spell_id: None,
            stack_item: None,
        };

        assert_eq!(engine.battlefield_creature_count(&filter, 0, 0), 0);
        assert!(!engine.condition_holds(&condition, context));

        for counter in [
            CounterKind::PlusOnePlusOne,
            CounterKind::MinusOneMinusOne,
            CounterKind::Keyword(Keyword::Flying),
            CounterKind::Stun,
        ] {
            engine
                .state
                .objects
                .get_mut(&bear)
                .expect("bear")
                .set_counter(counter, 1);
            assert_eq!(engine.battlefield_creature_count(&filter, 0, 0), 1);
            assert!(engine.condition_holds(&condition, context));
            engine
                .state
                .objects
                .get_mut(&bear)
                .expect("bear")
                .set_counter(counter, 0);
            assert_eq!(engine.battlefield_creature_count(&filter, 0, 0), 0);
            assert!(!engine.condition_holds(&condition, context));
        }

        engine
            .state
            .objects
            .get_mut(&opposing_bear)
            .expect("opposing bear")
            .set_counter(CounterKind::Stun, 1);
        assert_eq!(
            engine.battlefield_creature_count(&filter, 0, 0),
            0,
            "controller-relative counts must not include an opponent's countered creature"
        );
    }

    #[test]
    fn graveyard_aggregate_excludes_the_spell_that_is_still_resolving() {
        let decks = Some(vec![
            deck_with_cards(&["growth_cycle"], "forest"),
            deck_with_cards(&[], "island"),
        ]);
        let mut engine = GameEngine::new(108_010, &[0, 1], 20, decks, true).expect("engine");
        let resolving_spell = move_to_graveyard(&mut engine, 0, "growth_cycle");

        assert_eq!(
            graveyard_aggregate_value(
                &engine.state,
                engine.registry,
                RelativePlayerSet::Controller,
                GraveyardAggregate::CardCount,
                None,
                0,
                None,
            ),
            1
        );
        assert_eq!(
            graveyard_aggregate_value(
                &engine.state,
                engine.registry,
                RelativePlayerSet::Controller,
                GraveyardAggregate::CardCount,
                None,
                0,
                Some(resolving_spell),
            ),
            0,
            "CR 608.2n does not count the resolving spell as already in its graveyard"
        );
    }

    #[test]
    fn battlefield_total_power_uses_derived_values_and_generation_aware_source_exclusion() {
        let decks = Some(vec![
            deck_with_cards(&["grizzly_bears", "serra_angel"], "forest"),
            deck_with_cards(&[], "island"),
        ]);
        let mut engine = GameEngine::new(67_010, &[0, 1], 20, decks, true).expect("engine");
        let bear = move_to_battlefield(&mut engine, 0, "grizzly_bears");
        let angel = move_to_battlefield(&mut engine, 0, "serra_angel");
        let filter = BattlefieldPermanentFilter {
            token: None,
            any_of: None,
            controllers: RelativePlayerSet::Controller,
            card_type: Some(CardTypeFilter::Creature),
            color: None,
            name: None,
            required_subtypes: vec![],
            exclude_source: true,
        };
        let context = ConditionContext {
            controller: 0,
            source_object_id: angel,
            source_zone_change: 0,
            resolving_spell_id: None,
            stack_item: None,
        };

        assert_eq!(engine.effective_power(bear), Some(2));
        assert_eq!(engine.effective_power(angel), Some(4));
        assert_eq!(
            engine.battlefield_aggregate_value(&filter, BattlefieldAggregate::TotalPower, context,),
            2,
            "the original source generation is excluded"
        );

        engine.state.zone_change_generation.insert(angel, 1);
        assert_eq!(
            engine.battlefield_aggregate_value(&filter, BattlefieldAggregate::TotalPower, context,),
            6,
            "the same physical id after a zone change is a new object and counts as another"
        );
    }
}
