use super::*;

fn clamp_public_count(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
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
    filter: &PermanentEventFilter,
    fact: &TurnObjectFact,
    context: ConditionContext<'_>,
) -> bool {
    let type_matches = filter
        .permanent_type
        .is_none_or(|permanent_type| fact.has_type(permanent_type.as_str()));
    type_matches
        && filter
            .required_subtypes
            .iter()
            .all(|subtype| fact.has_type(subtype))
        && (!filter.exclude_source
            || fact.object_id != context.source_object_id
            || fact.zone_change_generation != context.source_zone_change)
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

    pub(super) fn record_spell_cast(&mut self, caster: PlayerId) -> u32 {
        self.state.turn_history.current.spells_cast = self
            .state
            .turn_history
            .current
            .spells_cast
            .saturating_add(1);
        let record = self.state.turn_history.current.player_mut(caster);
        record.spells_cast = record.spells_cast.saturating_add(1);
        record.spells_cast
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
        match condition {
            GameCondition::CastSnapshot { index } => context
                .stack_item
                .filter(|item| item.ability_text.is_none() && !item.is_copy)
                .and_then(|item| item.cast_condition_results.get(*index as usize))
                .copied()
                .unwrap_or(false),
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
            GameCondition::SpellsCastThisTurn { players, .. } => {
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
                                .spells_cast,
                        )
                    });
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
                        ) && permanent_event_fact_matches(filter, fact, context)
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

    fn condition_object_identity(
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

    pub(super) fn battlefield_aggregate_value(
        &self,
        filter: &BattlefieldPermanentFilter,
        aggregate: BattlefieldAggregate,
        context: ConditionContext,
    ) -> u32 {
        fn leaf_matches(
            engine: &GameEngine,
            filter: &BattlefieldPermanentFilter,
            characteristics: &Characteristics,
            context: ConditionContext,
        ) -> bool {
            let leaf = relative_player_set_contains(
                &engine.state,
                filter.controllers,
                context.controller,
                characteristics.controller,
            ) && filter.card_type.is_none_or(|card_type| match card_type {
                CardTypeFilter::BasicLand => {
                    characteristics.has_type("Land")
                        && characteristics
                            .supertypes
                            .iter()
                            .any(|value| value == "Basic")
                }
                CardTypeFilter::Land => characteristics.has_type("Land"),
                CardTypeFilter::Enchantment => characteristics.has_type("Enchantment"),
                CardTypeFilter::Instant => characteristics.has_type("Instant"),
                CardTypeFilter::Sorcery => characteristics.has_type("Sorcery"),
                CardTypeFilter::InstantOrSorcery => {
                    characteristics.has_type("Instant") || characteristics.has_type("Sorcery")
                }
                CardTypeFilter::Creature => characteristics.is_creature(),
                CardTypeFilter::Artifact => characteristics.is_artifact(),
                CardTypeFilter::Planeswalker => characteristics.has_type("Planeswalker"),
                CardTypeFilter::Battle => characteristics.has_type("Battle"),
                CardTypeFilter::Nonland => !characteristics.has_type("Land"),
                CardTypeFilter::Noncreature => !characteristics.is_creature(),
            }) && filter
                .color
                .is_none_or(|color| characteristics.colors.contains(&color))
                && filter
                    .required_subtypes
                    .iter()
                    .all(|subtype| characteristics.has_type(subtype))
                && filter
                    .name
                    .as_ref()
                    .is_none_or(|name| characteristics.has_name(name));
            leaf && filter.any_of.as_ref().is_none_or(|branches| {
                branches
                    .iter()
                    .any(|branch| leaf_matches(engine, branch, characteristics, context))
            })
        }

        let matching: Vec<_> = self
            .state
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
            .filter(|(_, characteristics)| leaf_matches(self, filter, characteristics, context))
            .collect();

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
            Amount::Count(CountExpression::BattlefieldCreatures { filter }) => self
                .battlefield_creature_count(filter, context.controller, context.source_object_id),
            Amount::Count(CountExpression::GraveyardCardsNamed { owners, name }) => {
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
                clamp_public_count(count)
            }
            Amount::Count(CountExpression::CreatureDeathsThisTurn) => {
                self.state.turn_history.current.creatures_died
            }
            Amount::Count(CountExpression::CardsMatchingResult { filter }) => context
                .previous_effect_result
                .zip(context.stack_item)
                .map_or(0, |(previous, top)| {
                    super::resolution::card_result_count(self, top, previous, filter)
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let registry = CardRegistry::from_chunks_and_tokens(&[r#"(id: "test", name: "Test", types: ["Creature"], power: 1, toughness: 1,
            triggered_abilities: [(trigger: WhenSelfEntersBattlefield, text: "Sacrifice this.", effect: [ChooseResolutionBranch(optional: true,
                branches: [(label: "Sacrifice this", cost: SacrificePermanent(filter: (kind: AnyPermanent, controller: You), source_only: true), effects: [Draw(count: 1)])])])])"#], &[]).unwrap();
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
            chosen_x: 0,
            chosen_modes: vec![],
            cast_cost_receipts: vec![],
            cast_condition_results: vec![],
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: Default::default(),
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
            any_of: Some(vec![
                BattlefieldPermanentFilter {
                    any_of: None,
                    controllers: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::Land),
                    color: None,
                    name: None,
                    required_subtypes: vec![],
                    exclude_source: false,
                },
                BattlefieldPermanentFilter {
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
