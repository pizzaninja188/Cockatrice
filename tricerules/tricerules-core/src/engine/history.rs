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
    registry: &CardRegistry,
    owners: RelativePlayerSet,
    aggregate: GraveyardAggregate,
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
        .filter(|object| object.zone == Zone::Graveyard && !object.is_token(registry))
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

impl GameEngine {
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
    }

    pub(super) fn record_spell_cast(&mut self) {
        self.state.turn_history.current.spells_cast = self
            .state
            .turn_history
            .current
            .spells_cast
            .saturating_add(1);
    }

    pub(super) fn condition_holds(
        &self,
        condition: &GameCondition,
        context: ConditionContext,
    ) -> bool {
        match condition {
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
            GameCondition::GraveyardAggregate {
                owners, aggregate, ..
            } => condition.matches_value(graveyard_aggregate_value(
                &self.state,
                self.registry,
                *owners,
                *aggregate,
                context.controller,
                context.resolving_spell_id,
            )),
        }
    }

    fn battlefield_aggregate_value(
        &self,
        filter: &BattlefieldPermanentFilter,
        aggregate: BattlefieldAggregate,
        context: ConditionContext,
    ) -> u32 {
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
            .filter(|(_, characteristics)| {
                relative_player_set_contains(
                    &self.state,
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
                    CardTypeFilter::Noncreature => !characteristics.is_creature(),
                }) && filter
                    .color
                    .is_none_or(|color| characteristics.colors.contains(&color))
            })
            .filter(|(oid, _)| {
                filter.name.as_ref().is_none_or(|name| {
                    self.effective_face(*oid)
                        .is_some_and(|face| face.name == *name)
                })
            })
            .collect();

        match aggregate {
            BattlefieldAggregate::Count => clamp_public_count(matching.len()),
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
            .filter_map(|oid| self.characteristics(oid))
            .filter(|characteristics| {
                relative_player_set_contains(
                    &self.state,
                    filter.controllers,
                    controller,
                    characteristics.controller,
                ) && characteristics.is_creature()
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
                            && !object.is_token(self.registry)
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
            Amount::Count(CountExpression::CardsMilledThisWay { filter }) => {
                let Some(EffectResult::MilledCards(object_ids)) = context.previous_effect_result
                else {
                    return 0;
                };
                let count = object_ids
                    .iter()
                    .filter_map(|oid| self.state.objects.get(oid))
                    .filter(|object| object.zone == Zone::Graveyard)
                    .filter_map(|object| self.registry.get(&object.card_id))
                    .filter(|definition| definition.matches_card_type_outside_stack(*filter))
                    .count();
                clamp_public_count(count)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .find(|object| object.owner == player_id && object.card_id == card_id)
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
            .find(|object| object.owner == player_id && object.card_id == card_id)
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
            controllers: RelativePlayerSet::Controller,
            card_type: Some(CardTypeFilter::Creature),
            color: None,
            name: None,
            exclude_source: true,
        };
        let context = ConditionContext {
            controller: 0,
            source_object_id: angel,
            source_zone_change: 0,
            resolving_spell_id: None,
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
