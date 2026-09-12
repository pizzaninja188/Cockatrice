use tricerules_cards::primitives::{
    BattlefieldAggregate, BattlefieldPermanentFilter, CardTypeFilter, DiscardQuantity,
    DrawDiscardOrder, EffectSubject, EntersTappedAffected, EntryCost, GameCondition, LifeAmount,
    ObjectContributionKind, ObjectPaymentConstraint, PermanentEventFilter, PermanentTypeFilter,
    PlayerLifeAggregate, PlayerRecipient, RelativePlayerSet, ResolutionCost, SearchDestination,
    SearchZoneSelection, SpellCastFilter, StackSpellFilter, StaticAbilityDef, TargetController,
    TargetFilter, TargetGroupDef, TargetKind, TargetingDef, TargetingSourceFilter,
    TypeLineAddition, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityCost, AbilityId, AbilityPresentation, AbilitySourceZone, ActivatedAbilityDef,
    ActivationTiming, Amount, BasicLandType, CastTriggerPlayer, CharacteristicDefiningAbility,
    CounterKind, IdentifiedAbility, Keyword, LibraryPartitionKind, ManaAmount, ManaCost,
    SpellEffectKind, TriggerCondition, TriggeredAbilityDef,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecipeSurface {
    KeywordClause,
    SpellClause,
    EtbAbility,
    TriggeredAbility,
    ActivatedAbility,
    /// An activated ability that functions from a nonbattlefield zone and may appear on any card
    /// type. Cycling on Lightshield Parry demonstrates why this is broader than permanent-only
    /// [`Self::ActivatedAbility`].
    ZoneActivatedAbility,
    StaticAbility,
    /// A static ability that functions on the source object while it is a spell, regardless of
    /// whether the card will become a permanent after resolving.
    SpellStaticAbility,
    CharacteristicAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct RecipeId(&'static str);

impl RecipeId {
    pub(super) const fn as_str(self) -> &'static str {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CalibrationCard {
    pub(super) name: &'static str,
    pub(super) clause: &'static str,
}

#[derive(Debug)]
pub(super) struct RecipeCalibration {
    pub(super) positive_cards: &'static [CalibrationCard],
    pub(super) negative_near_misses: &'static [&'static str],
}

pub(super) struct Recipe {
    pub(super) id: RecipeId,
    pub(super) label: &'static str,
    pub(super) surface: RecipeSurface,
    pub(super) matcher: fn(&str, &RecipeContext) -> Option<RecipeEmission>,
    pub(super) calibration: RecipeCalibration,
}

pub(super) struct RecipeContext {
    pub(super) triggered_ability_id: AbilityId,
    pub(super) activated_ability_id: AbilityId,
    pub(super) static_ability_id: AbilityId,
    pub(super) characteristic_ability_id: AbilityId,
    pub(super) presentation: AbilityPresentation,
    pub(super) source_is_land: bool,
    pub(super) source_is_creature: bool,
    pub(super) source_is_vehicle: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RecipeEmission {
    Keywords(Vec<Keyword>),
    SpellEffect(SpellEffectKind),
    TriggeredAbility(TriggeredAbilityDef),
    ActivatedAbility(ActivatedAbilityDef),
    StaticAbility(IdentifiedAbility<StaticAbilityDef>),
    CharacteristicAbility(IdentifiedAbility<CharacteristicDefiningAbility>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RecipeMatch {
    pub(super) id: RecipeId,
    pub(super) label: &'static str,
    pub(super) emission: RecipeEmission,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RecipeAmbiguity {
    pub(super) recipe_ids: Vec<RecipeId>,
}

impl std::fmt::Display for RecipeAmbiguity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "clause matched multiple exact recipes: {}",
            self.recipe_ids
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn parse_count_word(value: &str) -> Option<u32> {
    match value {
        "a" | "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        _ => value.parse().ok(),
    }
}

fn draw_effect(text: &str) -> Option<SpellEffectKind> {
    let count = text
        .strip_prefix("Draw ")?
        .strip_suffix('.')?
        .strip_suffix(" card")
        .or_else(|| text.strip_prefix("Draw ")?.strip_suffix(" cards."))?;
    Some(SpellEffectKind::Draw {
        who: PlayerRecipient::Controller,
        count: Amount::Fixed(parse_count_word(count)?),
    })
}

fn gain_life_effect(text: &str) -> Option<SpellEffectKind> {
    let amount = text
        .strip_prefix("You gain ")?
        .strip_suffix(" life.")?
        .parse()
        .ok()?;
    Some(SpellEffectKind::GainLife {
        amount: Amount::Fixed(amount),
    })
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub(super) fn keyword_ident(token: &str) -> Option<Keyword> {
    match token
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_lowercase()
        .as_str()
    {
        "flying" => Some(Keyword::Flying),
        "convoke" => Some(Keyword::Convoke),
        "reach" => Some(Keyword::Reach),
        "intimidate" => Some(Keyword::Intimidate),
        "vigilance" => Some(Keyword::Vigilance),
        "lifelink" => Some(Keyword::Lifelink),
        "haste" => Some(Keyword::Haste),
        "deathtouch" => Some(Keyword::Deathtouch),
        "menace" => Some(Keyword::Menace),
        "trample" => Some(Keyword::Trample),
        "first strike" => Some(Keyword::FirstStrike),
        "double strike" => Some(Keyword::DoubleStrike),
        "indestructible" => Some(Keyword::Indestructible),
        "hexproof" => Some(Keyword::Hexproof),
        "shroud" => Some(Keyword::Shroud),
        "defender" => Some(Keyword::Defender),
        "flash" => Some(Keyword::Flash),
        _ => None,
    }
}

pub(super) fn french_vanilla_keywords(clause: &str) -> Option<Vec<Keyword>> {
    let mut cleaned = String::with_capacity(clause.len());
    let mut reminder_depth = 0u32;
    for character in clause.chars() {
        match character {
            '(' => reminder_depth += 1,
            ')' => reminder_depth = reminder_depth.saturating_sub(1),
            _ if reminder_depth == 0 => cleaned.push(character),
            _ => {}
        }
    }
    let mut keywords = Vec::new();
    for token in cleaned
        .split(['\n', ',', ';'])
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        let keyword = keyword_ident(token)?;
        if !keywords.contains(&keyword) {
            keywords.push(keyword);
        }
    }
    Some(keywords)
}

fn match_keywords(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    french_vanilla_keywords(text)
        .filter(|keywords| !keywords.is_empty())
        .map(RecipeEmission::Keywords)
}

fn match_spell_draw(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    draw_effect(text).map(RecipeEmission::SpellEffect)
}

fn match_spell_gain_life(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    gain_life_effect(text).map(RecipeEmission::SpellEffect)
}

fn match_spell_destroy_creature(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Destroy target creature.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        })
    })
}

fn match_spell_return_creature(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Return target creature to its owner's hand.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        })
    })
}

fn match_spell_counter(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Counter target spell.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::CounterTargetSpell {
            spell_filter: StackSpellFilter::default(),
            unless_controller_pays: None,
            unless_controller_pays_by_cast_cost: None,
        })
    })
}

fn match_spell_pump(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    let deltas = text
        .strip_prefix("Target creature gets +")?
        .strip_suffix(" until end of turn.")?;
    let (power, toughness) = deltas.split_once("/+")?;
    Some(RecipeEmission::SpellEffect(SpellEffectKind::PumpTarget {
        power: power.parse().ok()?,
        toughness: toughness.parse().ok()?,
        scale: None,
        subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
    }))
}

fn triggered_ability(context: &RecipeContext, effect: SpellEffectKind) -> RecipeEmission {
    triggered_ability_with(
        context,
        TriggerCondition::WhenSelfEntersBattlefield,
        vec![effect],
    )
}

fn triggered_ability_with(
    context: &RecipeContext,
    trigger: TriggerCondition,
    effect: Vec<SpellEffectKind>,
) -> RecipeEmission {
    RecipeEmission::TriggeredAbility(TriggeredAbilityDef {
        ability_id: context.triggered_ability_id.clone(),
        presentation: context.presentation.clone(),
        trigger,
        effect,
        modal: None,
        targeting: None,
        may: false,
        intervening_if: None,
        max_triggers_per_turn: None,
        triggers_only_once: false,
    })
}

fn etb_instruction(text: &str) -> Option<&str> {
    text.strip_prefix("When this creature enters, ")
}

fn match_etb_draw(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    let effect = draw_effect(&capitalize(etb_instruction(text)?))?;
    Some(triggered_ability(context, effect))
}

fn match_etb_gain_life(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    let effect = gain_life_effect(&capitalize(etb_instruction(text)?))?;
    Some(triggered_ability(context, effect))
}

fn match_land_etb_gain_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_land && text == "When this land enters, you gain 1 life.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            },
        )
    })
}

fn match_land_etb_scry_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_land && text == "When this land enters, scry 1.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::Scry {
                count: Amount::Fixed(1),
            },
        )
    })
}

fn match_land_etb_surveil_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_land && text == "When this land enters, surveil 1.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            },
        )
    })
}

fn match_land_etb_damage_target_opponent_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_land
        && text == "When this land enters, it deals 1 damage to target opponent.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..TargetFilter::default()
            };
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability(
                context,
                SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(1),
                    target,
                },
            ) else {
                unreachable!("triggered_ability always returns a triggered ability")
            };
            ability.targeting = Some(TargetingDef {
                groups: vec![TargetGroupDef {
                    min: 1,
                    max: 1,
                    prompt: "Choose target opponent".into(),
                    effect_indices: vec![0],
                    distinct_from: Vec::new(),
                    same_graveyard: false,
                    cast_cost_expansion: None,
                }],
            });
            RecipeEmission::TriggeredAbility(ability)
        })
}

fn match_etb_opponent_discard(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (etb_instruction(text)? == "each opponent discards a card.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::Discard {
                who: PlayerRecipient::EachOpponent,
                quantity: DiscardQuantity::Exact(1),
            },
        )
    })
}

fn match_etb_explore(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "When this creature enters, it explores.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::Explore {
                subject: EffectSubject::Source,
            },
        )
    })
}

fn match_etb_surveil_two(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "When this creature enters, surveil 2.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::LibraryPartition {
                count: 2,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            },
        )
    })
}

fn match_creature_trigger_create_token(
    text: &str,
    context: &RecipeContext,
    expected: &str,
    trigger: TriggerCondition,
    token: &str,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == expected).then(|| {
        triggered_ability_with(
            context,
            trigger,
            vec![SpellEffectKind::CreateTokens {
                token: token.to_string(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }],
        )
    })
}

fn match_etb_create_treasure(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    match_creature_trigger_create_token(
        text,
        context,
        "When this creature enters, create a Treasure token.",
        TriggerCondition::WhenSelfEntersBattlefield,
        "treasure",
    )
}

fn match_dies_create_treasure(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    match_creature_trigger_create_token(
        text,
        context,
        "When this creature dies, create a Treasure token.",
        TriggerCondition::WhenSelfDies,
        "treasure",
    )
}

fn match_etb_create_food(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    match_creature_trigger_create_token(
        text,
        context,
        "When this creature enters, create a Food token.",
        TriggerCondition::WhenSelfEntersBattlefield,
        "food",
    )
}

fn match_etb_scry_two(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "When this creature enters, scry 2.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::Scry {
                count: Amount::Fixed(2),
            },
        )
    })
}

fn match_etb_surveil_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "When this creature enters, surveil 1.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            },
        )
    })
}

fn match_etb_exile_top_with_play_permission(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "When this creature enters, exile the top card of your library. Until the end of your next turn, you may play that card.")
        .then(|| {
            triggered_ability(
                context,
                SpellEffectKind::ExileTopWithPlayPermission {
                    player: PlayerRecipient::Controller,
                    count: 1,
                    count_by_cast_cost: None,
                },
            )
        })
}

fn match_etb_put_plus_one_counter_on_target_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "When this creature enters, put a +1/+1 counter on target creature.")
        .then(|| {
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability(
                context,
                SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
            ) else {
                unreachable!("triggered_ability always returns a triggered ability")
            };
            ability.targeting = Some(TargetingDef {
                groups: vec![TargetGroupDef {
                    min: 1,
                    max: 1,
                    prompt: "Choose target creature".into(),
                    effect_indices: vec![0],
                    distinct_from: Vec::new(),
                    same_graveyard: false,
                    cast_cost_expansion: None,
                }],
            });
            RecipeEmission::TriggeredAbility(ability)
        })
}

fn match_etb_create_ally(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    match_creature_trigger_create_token(
        text,
        context,
        "When this creature enters, create a 1/1 white Ally creature token.",
        TriggerCondition::WhenSelfEntersBattlefield,
        "ally_w_1_1",
    )
}

fn match_etb_draw_discard_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "When this creature enters, draw a card, then discard a card.")
        .then(|| {
            triggered_ability(
                context,
                SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::Controller,
                    draw_count: 1,
                    discard_count: 1,
                    order: DrawDiscardOrder::DrawThenDiscard,
                    optional: false,
                },
            )
        })
}

fn match_self_attacks_surveil_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "Whenever this creature attacks, surveil 1.").then(
        || {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0,
                },
                vec![SpellEffectKind::LibraryPartition {
                    count: 1,
                    top_min: 0,
                    top_max: None,
                    kind: LibraryPartitionKind::Surveil,
                }],
            )
        },
    )
}

fn match_etb_create_map(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    match_creature_trigger_create_token(
        text,
        context,
        "When this creature enters, create a Map token.",
        TriggerCondition::WhenSelfEntersBattlefield,
        "map",
    )
}

fn match_etb_mill_two(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "When this creature enters, mill two cards.").then(
        || {
            triggered_ability(
                context,
                SpellEffectKind::Mill {
                    count: Amount::Fixed(2),
                    who: PlayerRecipient::Controller,
                },
            )
        },
    )
}

fn match_etb_return_opponent_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "When this creature enters, return target creature an opponent controls to its owner's hand.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                ..TargetFilter::default()
            };
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability(
                context,
                SpellEffectKind::ReturnToOwnersHand {
                    subject: EffectSubject::Chosen(Box::new(target)),
                },
            ) else {
                unreachable!("triggered_ability always returns a triggered ability")
            };
            ability.targeting = Some(TargetingDef {
                groups: vec![TargetGroupDef {
                    min: 1,
                    max: 1,
                    prompt: "Choose target creature an opponent controls".into(),
                    effect_indices: vec![0],
                    distinct_from: Vec::new(),
                    same_graveyard: false,
                    cast_cost_expansion: None,
                }],
            });
            RecipeEmission::TriggeredAbility(ability)
        })
}

fn match_self_combat_damage_create_food(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    match_creature_trigger_create_token(
        text,
        context,
        "Whenever this creature deals combat damage to a player, create a Food token.",
        TriggerCondition::WheneverSelfDealsCombatDamageToPlayer,
        "food",
    )
}

fn match_self_dies_draw_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "When this creature dies, draw a card.").then(|| {
        triggered_ability_with(
            context,
            TriggerCondition::WhenSelfDies,
            vec![SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
        )
    })
}

fn match_etb_drain_two(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "When this creature enters, each opponent loses 2 life and you gain 2 life.").then(
        || {
            triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
                vec![
                    SpellEffectKind::LoseLife {
                        amount: LifeAmount::Fixed(2),
                        who: PlayerRecipient::EachOpponent,
                    },
                    SpellEffectKind::GainLife {
                        amount: Amount::Fixed(2),
                    },
                ],
            )
        },
    )
}

fn match_other_controlled_creature_enters_gain_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Whenever another creature you control enters, you gain 1 life.").then(|| {
        triggered_ability_with(
            context,
            TriggerCondition::WheneverPermanentEntersBattlefield {
                controller: CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(PermanentTypeFilter::Creature),
                    exclude_source: true,
                    ..PermanentEventFilter::default()
                },
                creature_filter: None,
            },
            vec![SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            }],
        )
    })
}

fn match_prowess(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Prowess").then(|| {
        RecipeEmission::TriggeredAbility(TriggeredAbilityDef {
            ability_id: context.triggered_ability_id.clone(),
            presentation: context.presentation.clone(),
            trigger: TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter: SpellCastFilter {
                    card_type: Some(CardTypeFilter::Noncreature),
                    ..SpellCastFilter::default()
                },
                ordinal: None,
                ordinal_scope: Default::default(),
            },
            effect: vec![SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 1,
                scale: None,
                subject: EffectSubject::Source,
            }],
            modal: None,
            targeting: None,
            may: false,
            intervening_if: None,
            max_triggers_per_turn: None,
            triggers_only_once: false,
        })
    })
}

fn match_crew(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    if !context.source_is_vehicle {
        return None;
    }
    let threshold_text = text.strip_prefix("Crew ")?;
    let threshold = threshold_text.parse::<u32>().ok()?;
    if threshold == 0 || threshold.to_string() != threshold_text {
        return None;
    }

    Some(RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::TapPermanents {
            constraint: ObjectPaymentConstraint::AggregateMinimum {
                minimum: threshold,
                contribution: ObjectContributionKind::CurrentPower,
            },
            filter: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
            exclude_source: true,
        }],
        effect: vec![SpellEffectKind::AddTypes {
            subject: EffectSubject::Source,
            addition: TypeLineAddition {
                card_types: vec![PermanentTypeFilter::Creature],
                creature_types: Vec::new(),
            },
        }],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    }))
}

fn match_mana_ward(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    let cost = exact_mana_cost(text.strip_prefix("Ward ")?)?;
    Some(triggered_ability_with(
        context,
        TriggerCondition::WheneverSelfBecomesTarget {
            source: TargetingSourceFilter::SpellOrAbility,
            source_controller: CastTriggerPlayer::Opponent,
        },
        vec![SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
            cost: ResolutionCost::Mana(cost),
        }],
    ))
}

fn match_changeling(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Changeling").then(|| {
        RecipeEmission::CharacteristicAbility(IdentifiedAbility {
            ability_id: context.characteristic_ability_id.clone(),
            presentation: context.presentation.clone(),
            definition: CharacteristicDefiningAbility::Changeling,
        })
    })
}

fn match_spell_cannot_be_countered(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "This spell can't be countered.").then(|| {
        RecipeEmission::StaticAbility(IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: context.presentation.clone(),
            definition: StaticAbilityDef::SpellCannotBeCountered,
        })
    })
}

fn parse_mana_amount(symbol: char) -> Option<ManaAmount> {
    let mut amount = ManaAmount::default();
    match symbol {
        'W' => amount.w = 1,
        'U' => amount.u = 1,
        'B' => amount.b = 1,
        'R' => amount.r = 1,
        'G' => amount.g = 1,
        'C' => amount.c = 1,
        _ => return None,
    }
    Some(amount)
}

fn match_tap_for_one_mana(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    let symbol = text.strip_prefix("{T}: Add {")?.strip_suffix("}.")?;
    let mut chars = symbol.chars();
    let amount = parse_mana_amount(chars.next()?)?;
    if chars.next().is_some() {
        return None;
    }
    Some(RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Tap],
        effect: vec![SpellEffectKind::ProduceMana {
            options: vec![amount],
            restriction: None,
            conditional: None,
        }],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    }))
}

fn match_tap_for_any_color(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "{T}: Add one mana of any color.").then(|| {
        RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            cost_modifiers: Vec::new(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::Tap],
            effect: vec![SpellEffectKind::ProduceMana {
                options: ['W', 'U', 'B', 'R', 'G']
                    .into_iter()
                    .map(|symbol| {
                        parse_mana_amount(symbol).expect("five-color recipe uses valid symbols")
                    })
                    .collect(),
                restriction: None,
                conditional: None,
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: Vec::new(),
            activation_limit: None,
        })
    })
}

fn match_pay_one_tap_for_any_color(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_land && text == "{1}, {T}: Add one mana of any color.").then(|| {
        RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            cost_modifiers: Vec::new(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![
                AbilityCost::Mana(ManaCost::parse("{1}").expect("static recipe mana cost")),
                AbilityCost::Tap,
            ],
            effect: vec![SpellEffectKind::ProduceMana {
                options: ['W', 'U', 'B', 'R', 'G']
                    .into_iter()
                    .map(|symbol| {
                        parse_mana_amount(symbol).expect("five-color recipe uses valid symbols")
                    })
                    .collect(),
                restriction: None,
                conditional: None,
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: Vec::new(),
            activation_limit: None,
        })
    })
}

fn match_tap_for_multicolor_mana(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    if !context.source_is_land {
        return None;
    }
    let body = text.strip_prefix("{T}: Add ")?.strip_suffix('.')?;
    let symbols = if let Some((first_two, third)) = body.split_once(", or ") {
        let (first, second) = first_two.split_once(", ")?;
        vec![first, second, third]
    } else {
        let (first, second) = body.split_once(" or ")?;
        vec![first, second]
    };
    let mut options = Vec::with_capacity(symbols.len());
    for token in symbols {
        let symbol = token.strip_prefix('{')?.strip_suffix('}')?;
        let mut chars = symbol.chars();
        let symbol = chars.next()?;
        if chars.next().is_some() || symbol == 'C' {
            return None;
        }
        let amount = parse_mana_amount(symbol)?;
        if options.contains(&amount) {
            return None;
        }
        options.push(amount);
    }
    if !matches!(options.len(), 2 | 3) {
        return None;
    }

    Some(RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Tap],
        effect: vec![SpellEffectKind::ProduceMana {
            options,
            restriction: None,
            conditional: None,
        }],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    }))
}

fn match_sacrifice_to_naturalize(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "{1}, Sacrifice this creature: Destroy target artifact or enchantment.").then(|| {
        RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            cost_modifiers: Vec::new(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![
                AbilityCost::Mana(ManaCost::parse("{1}").expect("static recipe mana cost")),
                AbilityCost::SacrificeSelf,
            ],
            effect: vec![SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![
                        PermanentTypeFilter::Artifact,
                        PermanentTypeFilter::Enchantment,
                    ],
                    ..TargetFilter::default()
                })),
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: Vec::new(),
            activation_limit: None,
        })
    })
}

fn match_land_tap_sacrifice_draw_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_land && text == "{4}, {T}, Sacrifice this land: Draw a card.").then(|| {
        RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            cost_modifiers: Vec::new(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![
                AbilityCost::Mana(ManaCost::parse("{4}").expect("static recipe mana cost")),
                AbilityCost::Tap,
                AbilityCost::SacrificeSelf,
            ],
            effect: vec![SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: Vec::new(),
            activation_limit: None,
        })
    })
}

fn match_land_pay_four_tap_surveil_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_land && text == "{4}, {T}: Surveil 1.").then(|| {
        RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            cost_modifiers: Vec::new(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![
                AbilityCost::Mana(ManaCost::parse("{4}").expect("static recipe mana cost")),
                AbilityCost::Tap,
            ],
            effect: vec![SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: Vec::new(),
            activation_limit: None,
        })
    })
}

fn exact_mana_cost(text: &str) -> Option<ManaCost> {
    if text.is_empty() {
        return None;
    }
    let cost = ManaCost::parse(text).ok()?;
    (!cost.has_x() && cost.to_string() == text).then_some(cost)
}

fn hand_discard_ability(
    context: &RecipeContext,
    mana_cost: ManaCost,
    effect: SpellEffectKind,
) -> RecipeEmission {
    RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Hand,
        costs: vec![AbilityCost::Mana(mana_cost), AbilityCost::DiscardSelf],
        effect: vec![effect],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    })
}

fn match_cycling_draw(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    let cost = exact_mana_cost(text.strip_prefix("Cycling ")?)?;
    Some(hand_discard_ability(
        context,
        cost,
        SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        },
    ))
}

fn typecycling_search(
    context: &RecipeContext,
    mana_cost: ManaCost,
    filter: ZoneCardFilter,
) -> RecipeEmission {
    hand_discard_ability(
        context,
        mana_cost,
        SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::Controller,
            optional: false,
            count: 1,
            count_by_cast_cost: None,
            filter: Some(filter),
            slots: Vec::new(),
            zones: SearchZoneSelection::default(),
            destination: SearchDestination::Hand,
            conditional_destination: None,
            shuffle: true,
            reveal: true,
            result_id: None,
        },
    )
}

fn match_basic_landcycling(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    let cost = exact_mana_cost(text.strip_prefix("Basic landcycling ")?)?;
    Some(typecycling_search(
        context,
        cost,
        ZoneCardFilter {
            card_type: Some(CardTypeFilter::BasicLand),
            ..ZoneCardFilter::default()
        },
    ))
}

fn match_basic_land_typecycling(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    let (land_type, cost) = BasicLandType::ALL.into_iter().find_map(|land_type| {
        let prefix = format!("{}cycling ", land_type.as_str());
        let cost = exact_mana_cost(text.strip_prefix(&prefix)?)?;
        Some((land_type, cost))
    })?;
    Some(typecycling_search(
        context,
        cost,
        ZoneCardFilter {
            required_subtypes: vec![land_type.as_str().to_string()],
            ..ZoneCardFilter::default()
        },
    ))
}

fn match_shockland_entry_payment(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == "As this land enters, you may pay 2 life. If you don't, it enters tapped.").then(
        || {
            RecipeEmission::StaticAbility(IdentifiedAbility {
                ability_id: context.static_ability_id.clone(),
                presentation: context.presentation.clone(),
                definition: StaticAbilityDef::EntersTapped {
                    affected: EntersTappedAffected::Self_,
                    condition: None,
                    unless_cost: Some(EntryCost::PayLife { amount: 2 }),
                },
            })
        },
    )
}

fn match_unconditional_enters_tapped(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_land && text == "This land enters tapped.").then(|| {
        RecipeEmission::StaticAbility(IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: context.presentation.clone(),
            definition: StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
        })
    })
}

fn land_count_entry_condition(min: Option<u32>, max: Option<u32>) -> GameCondition {
    GameCondition::BattlefieldAggregate {
        filter: BattlefieldPermanentFilter {
            token: None,
            any_of: None,
            controllers: RelativePlayerSet::Controller,
            card_type: Some(CardTypeFilter::Land),
            color: None,
            name: None,
            required_subtypes: Vec::new(),
            exclude_source: true,
        },
        aggregate: BattlefieldAggregate::Count,
        min,
        max,
    }
}

fn conditional_tapped_entry(context: &RecipeContext, condition: GameCondition) -> RecipeEmission {
    RecipeEmission::StaticAbility(IdentifiedAbility {
        ability_id: context.static_ability_id.clone(),
        presentation: context.presentation.clone(),
        definition: StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: Some(condition),
            unless_cost: None,
        },
    })
}

fn match_fast_land_entry(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_land
        && text == "This land enters tapped unless you control two or fewer other lands.")
        .then(|| conditional_tapped_entry(context, land_count_entry_condition(Some(3), None)))
}

fn match_slow_land_entry(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_land
        && text == "This land enters tapped unless you control two or more other lands.")
        .then(|| conditional_tapped_entry(context, land_count_entry_condition(None, Some(1))))
}

fn match_player_life_threshold_entry(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_land
        && text == "This land enters tapped unless a player has 13 or less life.")
        .then(|| {
            conditional_tapped_entry(
                context,
                GameCondition::PlayerLifeAggregate {
                    players: RelativePlayerSet::All,
                    aggregate: PlayerLifeAggregate::Minimum,
                    min: Some(14),
                    max: None,
                },
            )
        })
}

macro_rules! calibrations {
    ($first_name:literal => $first_clause:literal, $second_name:literal => $second_clause:literal; $($negative:literal),+ $(,)?) => {
        RecipeCalibration {
            positive_cards: &[
                CalibrationCard { name: $first_name, clause: $first_clause },
                CalibrationCard { name: $second_name, clause: $second_clause },
            ],
            negative_near_misses: &[$($negative),+],
        }
    };
}

pub(super) static CATALOG: &[Recipe] = &[
    Recipe {
        id: RecipeId("keyword.supported_set"),
        label: "supported keyword clause",
        surface: RecipeSurface::KeywordClause,
        matcher: match_keywords,
        calibration: calibrations!(
            "Air Elemental" => "Flying",
            "Serra Angel" => "Flying, vigilance";
            "Ward—Pay 2 life.",
            "Flying and vigilance"
        ),
    },
    Recipe {
        id: RecipeId("spell.draw.fixed"),
        label: "draw spell",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_draw,
        calibration: calibrations!(
            "Divination" => "Draw two cards.",
            "Counsel of the Soratami" => "Draw two cards.";
            "You may draw a card.",
            "Draw two cards. You lose 2 life."
        ),
    },
    Recipe {
        id: RecipeId("spell.gain_life.fixed"),
        label: "gain-life spell",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_gain_life,
        calibration: calibrations!(
            "Angel's Mercy" => "You gain 7 life.",
            "Sacred Nectar" => "You gain 4 life.";
            "You may gain 4 life.",
            "You gain 4 life and draw a card."
        ),
    },
    Recipe {
        id: RecipeId("spell.destroy.creature"),
        label: "destroy target creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_destroy_creature,
        calibration: calibrations!(
            "Murder" => "Destroy target creature.",
            "Impale" => "Destroy target creature.";
            "Destroy up to one target creature.",
            "Destroy target creature or planeswalker."
        ),
    },
    Recipe {
        id: RecipeId("spell.return_to_hand.creature"),
        label: "return target creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_return_creature,
        calibration: calibrations!(
            "Unsummon" => "Return target creature to its owner's hand.",
            "Drown in Shapelessness" => "Return target creature to its owner's hand.";
            "Return up to one target creature to its owner's hand.",
            "Return target nonland permanent to its owner's hand."
        ),
    },
    Recipe {
        id: RecipeId("spell.counter.unrestricted"),
        label: "counter target spell",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_counter,
        calibration: calibrations!(
            "Counterspell" => "Counter target spell.",
            "Cancel" => "Counter target spell.";
            "Counter target spell unless its controller pays {3}.",
            "Counter up to one target spell."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump.creature.fixed"),
        label: "fixed creature pump",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_pump,
        calibration: calibrations!(
            "Giant Growth" => "Target creature gets +3/+3 until end of turn.",
            "Titanic Growth" => "Target creature gets +4/+4 until end of turn.";
            "Target creature gets +X/+X until end of turn.",
            "Up to one target creature gets +3/+3 until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("etb.draw.fixed"),
        label: "ETB draw",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_draw,
        calibration: calibrations!(
            "Cloudkin Seer" => "When this creature enters, draw a card.",
            "Elvish Visionary" => "When this creature enters, draw a card.";
            "When this creature enters, you may draw a card.",
            "When this creature enters, draw a card and lose 1 life."
        ),
    },
    Recipe {
        id: RecipeId("etb.gain_life.fixed"),
        label: "ETB gain life",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_gain_life,
        calibration: calibrations!(
            "Dawning Angel" => "When this creature enters, you gain 4 life.",
            "Hill Giant Herdgorger" => "When this creature enters, you gain 5 life.";
            "When this creature enters, you may gain 4 life.",
            "When this creature enters, you gain 4 life and draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.land.gain_life.one"),
        label: "land ETB gain 1 life",
        surface: RecipeSurface::EtbAbility,
        matcher: match_land_etb_gain_one,
        calibration: calibrations!(
            "Scoured Barrens" => "When this land enters, you gain 1 life.",
            "Stark Industries" => "When this land enters, you gain 1 life.";
            "When this land enters, you may gain 1 life.",
            "When this land enters, you gain 2 life.",
            "When this land enters, you gain 1 life and draw a card.",
            "When this artifact enters, you gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("etb.land.scry.one"),
        label: "land ETB scry 1",
        surface: RecipeSurface::EtbAbility,
        matcher: match_land_etb_scry_one,
        calibration: calibrations!(
            "Temple of Deceit" => "When this land enters, scry 1.",
            "Crystal Grotto" => "When this land enters, scry 1.";
            "When this land enters, you may scry 1.",
            "When this land enters, scry 2.",
            "When this land enters, target player scries 1.",
            "When this land enters, scry 1, then draw a card.",
            "When this artifact enters, scry 1."
        ),
    },
    Recipe {
        id: RecipeId("etb.land.surveil.one"),
        label: "land ETB surveil 1",
        surface: RecipeSurface::EtbAbility,
        matcher: match_land_etb_surveil_one,
        calibration: calibrations!(
            "Undercity Sewers" => "When this land enters, surveil 1.",
            "Conduit Pylons" => "When this land enters, surveil 1.";
            "When this land enters, you may surveil 1.",
            "When this land enters, surveil 2.",
            "When this land enters, target player surveils 1.",
            "When this land enters, surveil 1, then draw a card.",
            "When this land enters, look at the top card of your library. You may put it into your graveyard.",
            "When this artifact enters, surveil 1."
        ),
    },
    Recipe {
        id: RecipeId("etb.land.damage.target_opponent.one"),
        label: "land ETB damage target opponent 1",
        surface: RecipeSurface::EtbAbility,
        matcher: match_land_etb_damage_target_opponent_one,
        calibration: calibrations!(
            "Lonely Arroyo" => "When this land enters, it deals 1 damage to target opponent.",
            "Jagged Barrens" => "When this land enters, it deals 1 damage to target opponent.";
            "When this land enters, it may deal 1 damage to target opponent.",
            "When this land enters, it deals 2 damage to target opponent.",
            "When this land enters, it deals 1 damage to target player.",
            "When this land enters, it deals 1 damage to each opponent.",
            "When this land enters, target opponent loses 1 life.",
            "When this land enters, it deals 1 damage to target opponent and you gain 1 life.",
            "When this artifact enters, it deals 1 damage to target opponent."
        ),
    },
    Recipe {
        id: RecipeId("etb.discard.each_opponent.one"),
        label: "ETB opponent discard",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_opponent_discard,
        calibration: calibrations!(
            "Burglar Rat" => "When this creature enters, each opponent discards a card.",
            "Virus Beetle" => "When this creature enters, each opponent discards a card.";
            "When this creature enters, target opponent discards a card.",
            "When this creature enters, each opponent discards two cards."
        ),
    },
    Recipe {
        id: RecipeId("etb.explore.source"),
        label: "ETB self Explore",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_explore,
        calibration: calibrations!(
            "Cenote Scout" => "When this creature enters, it explores.",
            "Pathfinding Axejaw" => "When this creature enters, it explores.";
            "When this creature enters, target creature explores.",
            "When this creature enters, it may explore.",
            "When this creature enters, it explores twice.",
            "Whenever this creature attacks, it explores.",
            "Whenever this creature deals combat damage to a player, it explores.",
            "Whenever another creature enters, it explores.",
            "When this creature enters, it explores, then you gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("etb.surveil.two"),
        label: "ETB surveil 2",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_surveil_two,
        calibration: calibrations!(
            "A.I.M. Synthoids" => "When this creature enters, surveil 2.",
            "Imperious Inkmage" => "When this creature enters, surveil 2.";
            "When this creature enters, you may surveil 2.",
            "When this creature enters, surveil 3.",
            "When this creature enters, target player surveils 2.",
            "When this creature enters, surveil 2, then draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.create_token.treasure.one"),
        label: "ETB create Treasure",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_create_treasure,
        calibration: calibrations!(
            "Meticulous Artisan" => "When this creature enters, create a Treasure token.",
            "Plundering Pirate" => "When this creature enters, create a Treasure token.";
            "When this creature enters, you may create a Treasure token.",
            "When this creature enters, create two Treasure tokens.",
            "When this creature enters, create a tapped Treasure token.",
            "When this creature enters, create a Treasure token, then draw a card.",
            "Whenever another creature enters, create a Treasure token."
        ),
    },
    Recipe {
        id: RecipeId("dies.create_token.treasure.one"),
        label: "dies create Treasure",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_dies_create_treasure,
        calibration: calibrations!(
            "Gleaming Barrier" => "When this creature dies, create a Treasure token.",
            "Piggy Bank" => "When this creature dies, create a Treasure token.";
            "When this creature dies, you may create a Treasure token.",
            "When this creature dies, create two Treasure tokens.",
            "When this creature leaves the battlefield, create a Treasure token.",
            "When another creature dies, create a Treasure token.",
            "When this creature dies, create a Treasure token and draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.create_token.food.one"),
        label: "ETB create Food",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_create_food,
        calibration: calibrations!(
            "Canyon Crawler" => "When this creature enters, create a Food token.",
            "Fierce Witchstalker" => "When this creature enters, create a Food token.";
            "When this creature enters, you may create a Food token.",
            "When this creature enters, create two Food tokens.",
            "When this creature enters, create a tapped Food token.",
            "When this creature enters, create a Food token, then draw a card.",
            "Whenever another creature enters, create a Food token."
        ),
    },
    Recipe {
        id: RecipeId("etb.scry.two"),
        label: "ETB scry 2",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_scry_two,
        calibration: calibrations!(
            "Wakandan Drone Flock" => "When this creature enters, scry 2.",
            "Augury Owl" => "When this creature enters, scry 2.";
            "When this creature enters, you may scry 2.",
            "When this creature enters, scry 1.",
            "When this creature enters, target player scries 2.",
            "When this creature enters, scry 2, then draw a card.",
            "Whenever another creature enters, scry 2."
        ),
    },
    Recipe {
        id: RecipeId("etb.surveil.one"),
        label: "ETB surveil 1",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_surveil_one,
        calibration: calibrations!(
            "Shore Lurker" => "When this creature enters, surveil 1.",
            "Sanitation Automaton" => "When this creature enters, surveil 1.";
            "When this creature enters, you may surveil 1.",
            "When this creature enters, surveil 3.",
            "When this creature enters, target player surveils 1.",
            "When this creature enters, surveil 1, then draw a card.",
            "Whenever another creature enters, surveil 1."
        ),
    },
    Recipe {
        id: RecipeId("etb.exile_top.play_permission.end_of_next_turn"),
        label: "ETB exile top with play permission",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_exile_top_with_play_permission,
        calibration: calibrations!(
            "Gundabad Opportunist" => "When this creature enters, exile the top card of your library. Until the end of your next turn, you may play that card.",
            "Alania's Pathmaker" => "When this creature enters, exile the top card of your library. Until the end of your next turn, you may play that card.";
            "When this creature enters, you may exile the top card of your library. Until the end of your next turn, you may play that card.",
            "When this creature enters, exile the top two cards of your library. Until the end of your next turn, you may play those cards.",
            "When this creature enters, exile the top card of target player's library. Until the end of your next turn, you may play that card.",
            "When this creature enters, exile the top card of your library face down. Until the end of your next turn, you may play that card.",
            "When this creature enters, exile the top card of your library. Until end of turn, you may play that card.",
            "When this creature enters, exile the top card of your library. Until the end of your next turn, you may cast that card.",
            "Whenever another creature enters, exile the top card of your library. Until the end of your next turn, you may play that card.",
            "When this creature enters, exile the top card of your library. Until the end of your next turn, you may play that card. Create a Treasure token."
        ),
    },
    Recipe {
        id: RecipeId("etb.put_counter.plus_one_plus_one.target_creature.one"),
        label: "ETB put +1/+1 counter on target creature",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_put_plus_one_counter_on_target_creature,
        calibration: calibrations!(
            "Jeong Jeong's Deserters" => "When this creature enters, put a +1/+1 counter on target creature.",
            "Ironpaw Aspirant" => "When this creature enters, put a +1/+1 counter on target creature.";
            "When this creature enters, you may put a +1/+1 counter on target creature.",
            "When this creature enters, put two +1/+1 counters on target creature.",
            "When this creature enters, put a +1/+1 counter on this creature.",
            "When this creature enters, put a +1/+1 counter on target creature you control.",
            "When this creature enters, put a +1/+1 counter on target other creature.",
            "When this creature enters, put a flying counter on target creature.",
            "Whenever another creature enters, put a +1/+1 counter on target creature.",
            "When this creature enters, put a +1/+1 counter on target creature, then draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.create_token.ally_w_1_1.one"),
        label: "ETB create white 1/1 Ally",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_create_ally,
        calibration: calibrations!(
            "Invasion Reinforcements" => "When this creature enters, create a 1/1 white Ally creature token.",
            "Treetop Freedom Fighters" => "When this creature enters, create a 1/1 white Ally creature token.";
            "When this creature enters, you may create a 1/1 white Ally creature token.",
            "When this creature enters, create two 1/1 white Ally creature tokens.",
            "When this creature enters, create a 2/2 white Ally creature token.",
            "When this creature enters, create a 1/1 red Ally creature token.",
            "When this creature enters, create a 1/1 white Soldier creature token.",
            "When this creature enters, create a tapped 1/1 white Ally creature token.",
            "Whenever another creature enters, create a 1/1 white Ally creature token.",
            "When this creature enters, create a 1/1 white Ally creature token, then draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.draw_discard.controller.one_one"),
        label: "ETB draw then discard one",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_draw_discard_one,
        calibration: calibrations!(
            "Icewind Elemental" => "When this creature enters, draw a card, then discard a card.",
            "Temur Tawnyback" => "When this creature enters, draw a card, then discard a card.";
            "When this creature enters, you may draw a card, then discard a card.",
            "When this creature enters, draw two cards, then discard a card.",
            "When this creature enters, draw a card, then discard two cards.",
            "When this creature enters, discard a card, then draw a card.",
            "When this creature enters, target player draws a card, then discards a card.",
            "Whenever another creature enters, draw a card, then discard a card.",
            "When this creature enters, draw a card, then discard a card. If you discarded a land card this way, draw a card.",
            "If you would draw a card as this creature enters, draw two cards instead, then discard a card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_attacks.surveil.one"),
        label: "self-attacks surveil 1",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_attacks_surveil_one,
        calibration: calibrations!(
            "Boulderborn Dragon" => "Whenever this creature attacks, surveil 1.",
            "Il Mheg Pixie" => "Whenever this creature attacks, surveil 1.";
            "Whenever this creature attacks, you may surveil 1.",
            "Whenever this creature attacks, surveil 2.",
            "Whenever another creature attacks, surveil 1.",
            "Whenever one or more creatures you control attack, surveil 1.",
            "Whenever this creature becomes blocked, surveil 1.",
            "Whenever this creature attacks, surveil 1, then draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.create_token.map.one"),
        label: "ETB create Map",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_create_map,
        calibration: calibrations!(
            "Cartographer's Companion" => "When this creature enters, create a Map token.",
            "Waterwind Scout" => "When this creature enters, create a Map token.";
            "When this creature enters, you may create a Map token.",
            "When this creature enters, create two Map tokens.",
            "When this creature enters, create a tapped Map token.",
            "When this creature enters, create a Clue token.",
            "When this creature enters, create a Map token, then draw a card.",
            "Whenever another creature enters, create a Map token."
        ),
    },
    Recipe {
        id: RecipeId("etb.mill.controller.two"),
        label: "ETB mill two",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_mill_two,
        calibration: calibrations!(
            "Venomized Cat" => "When this creature enters, mill two cards.",
            "Scarblade Scout" => "When this creature enters, mill two cards.";
            "When this creature enters, you may mill two cards.",
            "When this creature enters, mill three cards.",
            "When this creature enters, target player mills two cards.",
            "When this creature enters, each player mills two cards.",
            "Whenever another creature enters, mill two cards.",
            "When this creature enters, mill two cards, then gain 2 life."
        ),
    },
    Recipe {
        id: RecipeId("etb.return_to_hand.opponent_creature"),
        label: "ETB return opponent creature",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_return_opponent_creature,
        calibration: calibrations!(
            "Bigfin Bouncer" => "When this creature enters, return target creature an opponent controls to its owner's hand.",
            "Exclusion Mage" => "When this creature enters, return target creature an opponent controls to its owner's hand.";
            "When this creature enters, you may return target creature an opponent controls to its owner's hand.",
            "When this creature enters, return up to one target creature an opponent controls to its owner's hand.",
            "When this creature enters, return target creature to its owner's hand.",
            "When this creature enters, return target creature you control to its owner's hand.",
            "When this creature enters, return target nonland permanent an opponent controls to its owner's hand.",
            "Whenever another creature enters, return target creature an opponent controls to its owner's hand.",
            "When this creature enters, return target creature an opponent controls to its owner's hand, then draw a card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_combat_damage_to_player.create_token.food.one"),
        label: "self combat damage to player create Food",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_combat_damage_create_food,
        calibration: calibrations!(
            "Eager Trufflesnout" => "Whenever this creature deals combat damage to a player, create a Food token.",
            "Scream Puff" => "Whenever this creature deals combat damage to a player, create a Food token.";
            "Whenever this creature deals combat damage to a player, you may create a Food token.",
            "Whenever this creature deals combat damage to a player, create two Food tokens.",
            "Whenever this creature deals combat damage to a player, create a tapped Food token.",
            "Whenever this creature deals combat damage to a player, create a Treasure token.",
            "Whenever this creature deals combat damage to an opponent, create a Food token.",
            "Whenever this creature deals combat damage to a planeswalker, create a Food token.",
            "Whenever this creature deals damage to a player, create a Food token.",
            "Whenever another creature deals combat damage to a player, create a Food token.",
            "Whenever this creature deals combat damage to a player, create a Food token, then draw a card."
        ),
    },
    Recipe {
        id: RecipeId("dies.draw.controller.one"),
        label: "self dies draw",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_dies_draw_one,
        calibration: calibrations!(
            "Buzz Bots" => "When this creature dies, draw a card.",
            "Outlaw Medic" => "When this creature dies, draw a card.";
            "When this creature dies, you may draw a card.",
            "When this creature dies, draw two cards.",
            "When this creature leaves the battlefield, draw a card.",
            "When this creature dies, target player draws a card.",
            "When this creature dies, draw a card and lose 1 life."
        ),
    },
    Recipe {
        id: RecipeId("etb.drain.each_opponent.two"),
        label: "ETB drain 2",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_drain_two,
        calibration: calibrations!(
            "Glidedive Duo" => "When this creature enters, each opponent loses 2 life and you gain 2 life.",
            "Vampire Spawn" => "When this creature enters, each opponent loses 2 life and you gain 2 life.";
            "When this creature enters, target opponent loses 2 life and you gain 2 life.",
            "When this creature enters, each opponent loses 1 life and you gain 1 life.",
            "When this creature enters, each player loses 2 life and you gain 2 life.",
            "When this creature enters, you gain 2 life and each opponent loses 2 life.",
            "When this creature enters, each opponent loses 2 life and you gain 2 life, then draw a card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.other_controlled_creature_etb.gain_life.one"),
        label: "other controlled creature ETB gain 1",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_other_controlled_creature_enters_gain_one,
        calibration: calibrations!(
            "Dazzling Angel" => "Whenever another creature you control enters, you gain 1 life.",
            "Hinterland Sanctifier" => "Whenever another creature you control enters, you gain 1 life.";
            "Whenever a creature you control enters, you gain 1 life.",
            "Whenever another creature you control enters, you may gain 1 life.",
            "Whenever another creature you control enters, you gain 2 life.",
            "Whenever another creature enters, you gain 1 life.",
            "Whenever a creature an opponent controls enters, you gain 1 life.",
            "Whenever another permanent you control enters, you gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("triggered.prowess"),
        label: "prowess",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_prowess,
        calibration: calibrations!(
            "Mistral Singer" => "Prowess",
            "Agent of Atlas" => "Prowess";
            "Magecraft — Whenever you cast or copy an instant or sorcery spell, this creature gets +1/+1 until end of turn.",
            "Whenever you cast an instant or sorcery spell, this creature gets +1/+1 until end of turn.",
            "Whenever you cast a noncreature spell, draw a card.",
            "Prowess 2",
            "Prowess — Whenever you cast a noncreature spell, this creature gets +2/+2 until end of turn.",
            "Prowess if you control an artifact."
        ),
    },
    Recipe {
        id: RecipeId("activated.crew.aggregate_power"),
        label: "Crew",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_crew,
        calibration: calibrations!(
            "Skybox Ferry" => "Crew 2",
            "Cultivator's Caravan" => "Crew 3";
            "Crew 0",
            "Crew 03",
            "Crew X",
            "Crew 3 only once each turn",
            "Crew 3. Activate only as a sorcery.",
            "Tap any number of creatures you control with total power 3 or more: This permanent becomes an artifact creature until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("triggered.ward.mana"),
        label: "mana Ward",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_mana_ward,
        calibration: calibrations!(
            "Spider-Rex, Daring Dino" => "Ward {2}",
            "Marauding Brinefang" => "Ward {3}";
            "Ward—Discard a card.",
            "Ward—Pay 3 life.",
            "Ward—Sacrifice a permanent.",
            "Ward {X}",
            "Flying, ward {2}",
            "Other creatures you control have ward {2}.",
            "Ward {2}. Whenever this creature becomes the target of a spell, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("characteristic.changeling"),
        label: "Changeling",
        surface: RecipeSurface::CharacteristicAbility,
        matcher: match_changeling,
        calibration: calibrations!(
            "Prideful Feastling" => "Changeling",
            "Chitinous Graspling" => "Changeling";
            "Changeling 1",
            "Creatures you control have changeling.",
            "Target creature gains changeling until end of turn.",
            "Create a 2/2 colorless Shapeshifter creature token with changeling.",
            "Changeling. This creature gets +1/+1 for each creature type it has."
        ),
    },
    Recipe {
        id: RecipeId("static.spell.cannot_be_countered"),
        label: "self uncounterability",
        surface: RecipeSurface::SpellStaticAbility,
        matcher: match_spell_cannot_be_countered,
        calibration: calibrations!(
            "Gigantic Big Bear" => "This spell can't be countered.",
            "Carnage Tyrant" => "This spell can't be countered.";
            "Spells you control can't be countered.",
            "Creature spells you control can't be countered.",
            "This ability can't be countered.",
            "This spell can't be countered if mana from a Treasure was spent to cast it.",
            "This spell can't be countered by blue spells or abilities.",
            "Target spell can't be countered this turn."
        ),
    },
    Recipe {
        id: RecipeId("activated.mana.tap_one"),
        label: "tap for one mana",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_tap_for_one_mana,
        calibration: calibrations!(
            "Llanowar Elves" => "{T}: Add {G}.",
            "Elvish Mystic" => "{T}: Add {G}.";
            "{T}: Add {G}{G}.",
            "{T}, Pay 1 life: Add {G}."
        ),
    },
    Recipe {
        id: RecipeId("activated.mana.tap_any_color"),
        label: "tap for any color",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_tap_for_any_color,
        calibration: calibrations!(
            "Great Forest Druid" => "{T}: Add one mana of any color.",
            "Oasis Gardener" => "{T}: Add one mana of any color.";
            "{T}: Add one mana of any color or {C}.",
            "{T}: Add one mana of any type.",
            "{T}: Add two mana of any one color.",
            "{T}, Pay 1 life: Add one mana of any color.",
            "{T}: Add one mana of any color. Spend this mana only to cast creature spells.",
            "{2}, {T}: Add one mana of any color."
        ),
    },
    Recipe {
        id: RecipeId("activated.mana.pay_one_tap_any_color"),
        label: "pay one and tap for any color",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_pay_one_tap_for_any_color,
        calibration: calibrations!(
            "Crystal Grotto" => "{1}, {T}: Add one mana of any color.",
            "Conduit Pylons" => "{1}, {T}: Add one mana of any color.";
            "{1}: Add one mana of any color.",
            "{2}, {T}: Add one mana of any color.",
            "{1}, {T}: Add one mana of any type.",
            "{1}, {T}: Add two mana of any one color.",
            "{1}, {T}: Add one mana of any color. Spend this mana only to cast creature spells.",
            "{1}, {T}, Pay 1 life: Add one mana of any color."
        ),
    },
    Recipe {
        id: RecipeId("activated.mana.tap_two_or_three_colors"),
        label: "tap for multicolor mana",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_tap_for_multicolor_mana,
        calibration: calibrations!(
            "Rakdos Guildgate" => "{T}: Add {B} or {R}.",
            "Nomad Outpost" => "{T}: Add {R}, {W}, or {B}.";
            "{T}: Add {G}, {U}, {R}, or {W}.",
            "{T}: Add {C} or {G}.",
            "{T}: Add {G} or {G}.",
            "{T}: Add {G} and {U}.",
            "{T}: Add {G} or {U}. Spend this mana only to cast creature spells.",
            "{T}, Pay 1 life: Add {G} or {U}.",
            "{T}: Add {G}{U}."
        ),
    },
    Recipe {
        id: RecipeId("activated.sacrifice_self.destroy_artifact_or_enchantment"),
        label: "sacrifice-to-Naturalize",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_sacrifice_to_naturalize,
        calibration: calibrations!(
            "Cathar Commando" => "{1}, Sacrifice this creature: Destroy target artifact or enchantment.",
            "Thrashing Brontodon" => "{1}, Sacrifice this creature: Destroy target artifact or enchantment.";
            "{1}, {T}, Sacrifice this creature: Destroy target artifact or enchantment.",
            "{1}, Pay 1 life, Sacrifice this creature: Destroy target artifact or enchantment.",
            "{1}, Sacrifice another creature: Destroy target artifact or enchantment.",
            "{1}, Exile this creature: Destroy target artifact or enchantment.",
            "{1}, Sacrifice this artifact: Destroy target artifact or enchantment.",
            "{1}, Sacrifice this creature: Destroy up to one target artifact or enchantment.",
            "{1}, Sacrifice this creature: Destroy target artifact.",
            "{1}, Sacrifice this creature: Destroy target enchantment.",
            "{1}, Sacrifice this creature: Destroy target artifact or enchantment card in a graveyard.",
            "{1}, Sacrifice this creature: Destroy target artifact or enchantment. Activate only as a sorcery."
        ),
    },
    Recipe {
        id: RecipeId("activated.land.tap_sacrifice.draw_one"),
        label: "tap-sacrifice land draw",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_land_tap_sacrifice_draw_one,
        calibration: calibrations!(
            "Airship Engine Room" => "{4}, {T}, Sacrifice this land: Draw a card.",
            "North Pole Gates" => "{4}, {T}, Sacrifice this land: Draw a card.";
            "{3}, {T}, Sacrifice this land: Draw a card.",
            "{5}, {T}, Sacrifice this land: Draw a card.",
            "{4}, Sacrifice this land: Draw a card.",
            "{4}, {T}: Draw a card.",
            "{4}, {T}, Exile this land: Draw a card.",
            "{4}, {T}, Return this land to its owner's hand: Draw a card.",
            "{4}, {T}, Sacrifice this artifact: Draw a card.",
            "{4}, {T}, Sacrifice this land: You may draw a card.",
            "{4}, {T}, Sacrifice this land: Draw two cards.",
            "{4}, {T}, Sacrifice this land: Draw a card. Activate only as a sorcery.",
            "{4}, {T}, Sacrifice this land: Draw a card, then discard a card."
        ),
    },
    Recipe {
        id: RecipeId("activated.land.pay_four_tap.surveil_one"),
        label: "pay-four land Surveil 1",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_land_pay_four_tap_surveil_one,
        calibration: calibrations!(
            "Savage Mansion" => "{4}, {T}: Surveil 1.",
            "University Campus" => "{4}, {T}: Surveil 1.";
            "{3}, {T}: Surveil 1.",
            "{5}, {T}: Surveil 1.",
            "{4}: Surveil 1.",
            "{4}, {T}, Pay 1 life: Surveil 1.",
            "{4}, {T}: Surveil 2.",
            "{4}, {T}: You may surveil 1.",
            "{4}, {T}: Target player surveils 1.",
            "{4}, {T}: Surveil 1, then draw a card.",
            "{4}, {T}: Surveil 1. Activate only as a sorcery."
        ),
    },
    Recipe {
        id: RecipeId("activated.hand.cycling.draw"),
        label: "cycling draw",
        surface: RecipeSurface::ZoneActivatedAbility,
        matcher: match_cycling_draw,
        calibration: calibrations!(
            "Lightshield Parry" => "Cycling {2}",
            "Migrating Ketradon" => "Cycling {2}";
            "Cycle {2}",
            "Cycling {X}",
            "Cycling {2}{S}",
            "Cycling {2} — You may discard this card: Draw a card.",
            "Cycling {2}. Activate only as a sorcery.",
            "Cycling {2}. When you cycle this card, you gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("activated.hand.typecycling.basic_land"),
        label: "basic landcycling",
        surface: RecipeSurface::ZoneActivatedAbility,
        matcher: match_basic_landcycling,
        calibration: calibrations!(
            "Ash Barrens" => "Basic landcycling {1}",
            "Topiary Panther" => "Basic landcycling {1}{G}";
            "Basic landcycling {X}",
            "Basic landcycling {2}{S}",
            "Basic landcycling {2} — You may discard this card: Search your library.",
            "Basic landcycling {2}. Put that card onto the battlefield instead.",
            "Basic landcycling {2}. Activate only as a sorcery."
        ),
    },
    Recipe {
        id: RecipeId("activated.hand.typecycling.basic_land_type"),
        label: "basic land typecycling",
        surface: RecipeSurface::ZoneActivatedAbility,
        matcher: match_basic_land_typecycling,
        calibration: calibrations!(
            "Bedhead Beastie" => "Mountaincycling {2}",
            "Saber-Tooth Moose-Lion" => "Forestcycling {2}";
            "Wizardcycling {2}",
            "Landcycling {2}",
            "Plains Islandcycling {2}",
            "Forestcycling {X}",
            "Forestcycling {2}{S}",
            "Forestcycling {2}. Put that card onto the battlefield instead.",
            "Forestcycling {2}. Activate only as a sorcery."
        ),
    },
    Recipe {
        id: RecipeId("static.enters_tapped.unconditional"),
        label: "unconditional tapped entry",
        surface: RecipeSurface::StaticAbility,
        matcher: match_unconditional_enters_tapped,
        calibration: calibrations!(
            "Rakdos Guildgate" => "This land enters tapped.",
            "Nomad Outpost" => "This land enters tapped.";
            "This artifact enters tapped.",
            "This land enters tapped unless you control an Island.",
            "This land enters the battlefield tapped.",
            "This land enters tapped. When it enters, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("static.enters_tapped.land_count.fast"),
        label: "fast-land tapped entry",
        surface: RecipeSurface::StaticAbility,
        matcher: match_fast_land_entry,
        calibration: calibrations!(
            "Concealed Courtyard" => "This land enters tapped unless you control two or fewer other lands.",
            "Inspiring Vantage" => "This land enters tapped unless you control two or fewer other lands.";
            "This artifact enters tapped unless you control two or fewer other lands.",
            "This land enters tapped unless you control one or fewer other lands.",
            "This land enters tapped unless you control three or fewer other lands.",
            "This land enters tapped unless you control two or fewer lands.",
            "This land enters tapped unless you control two or fewer other basic lands.",
            "This land enters tapped unless an opponent controls two or fewer other lands.",
            "This land enters tapped unless you control two or fewer other lands. When it enters, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("static.enters_tapped.land_count.slow"),
        label: "slow-land tapped entry",
        surface: RecipeSurface::StaticAbility,
        matcher: match_slow_land_entry,
        calibration: calibrations!(
            "Sundown Pass" => "This land enters tapped unless you control two or more other lands.",
            "Shattered Sanctum" => "This land enters tapped unless you control two or more other lands.";
            "This artifact enters tapped unless you control two or more other lands.",
            "This land enters tapped unless you control one or more other lands.",
            "This land enters tapped unless you control three or more other lands.",
            "This land enters tapped unless you control two or more lands.",
            "This land enters tapped unless you control two or more other basic lands.",
            "This land enters tapped unless an opponent controls two or more other lands.",
            "This land enters tapped unless you control two or more other lands. When it enters, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("static.enters_tapped.player_life.minimum_fourteen"),
        label: "all-player life-threshold tapped entry",
        surface: RecipeSurface::StaticAbility,
        matcher: match_player_life_threshold_entry,
        calibration: calibrations!(
            "Raucous Carnival" => "This land enters tapped unless a player has 13 or less life.",
            "Etched Cornfield" => "This land enters tapped unless a player has 13 or less life.";
            "This artifact enters tapped unless a player has 13 or less life.",
            "This land enters tapped unless you have 13 or less life.",
            "This land enters tapped unless an opponent has 13 or less life.",
            "This land enters tapped unless each player has 13 or less life.",
            "This land enters tapped unless a player has 12 or less life.",
            "This land enters tapped unless a player has 14 or less life.",
            "This land enters tapped unless a player has 13 or more life.",
            "This land enters tapped unless a player has 13 or less life. When it enters, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("static.enters_tapped.unless_pay_life_2"),
        label: "shockland entry payment",
        surface: RecipeSurface::StaticAbility,
        matcher: match_shockland_entry_payment,
        calibration: calibrations!(
            "Blood Crypt" => "As this land enters, you may pay 2 life. If you don't, it enters tapped.",
            "Breeding Pool" => "As this land enters, you may pay 2 life. If you don't, it enters tapped.";
            "As this land enters, you may pay 2 life. It enters tapped.",
            "This land enters tapped unless you control an Island.",
            "As this land enters, you may pay 3 life. If you don't, it enters tapped.",
            "As this land enters, you may pay 2 life. If you don't, it enters tapped. When this land enters, draw a card."
        ),
    },
];

fn surface_applies(surface: RecipeSurface, is_spell: bool) -> bool {
    match surface {
        RecipeSurface::KeywordClause => true,
        RecipeSurface::SpellClause => is_spell,
        RecipeSurface::ZoneActivatedAbility
        | RecipeSurface::SpellStaticAbility
        | RecipeSurface::CharacteristicAbility => true,
        RecipeSurface::EtbAbility
        | RecipeSurface::TriggeredAbility
        | RecipeSurface::ActivatedAbility
        | RecipeSurface::StaticAbility => !is_spell,
    }
}

pub(super) fn match_clause(
    clause: &str,
    is_spell: bool,
    context: &RecipeContext,
) -> Result<Option<RecipeMatch>, RecipeAmbiguity> {
    match_clause_in(CATALOG, clause, is_spell, context)
}

pub(super) fn match_clause_in(
    catalog: &[Recipe],
    clause: &str,
    is_spell: bool,
    context: &RecipeContext,
) -> Result<Option<RecipeMatch>, RecipeAmbiguity> {
    let mut matches = catalog
        .iter()
        .filter(|recipe| surface_applies(recipe.surface, is_spell))
        .filter_map(|recipe| {
            (recipe.matcher)(clause, context).map(|emission| RecipeMatch {
                id: recipe.id,
                label: recipe.label,
                emission,
            })
        })
        .collect::<Vec<_>>();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.pop()),
        _ => Err(RecipeAmbiguity {
            recipe_ids: matches.into_iter().map(|matched| matched.id).collect(),
        }),
    }
}

pub(super) fn validate_catalog() -> Result<(), String> {
    validate_catalog_in(CATALOG)
}

fn validate_catalog_in(catalog: &[Recipe]) -> Result<(), String> {
    let context = RecipeContext {
        triggered_ability_id: AbilityId::new("triggered_01")?,
        activated_ability_id: AbilityId::new("activated_01")?,
        static_ability_id: AbilityId::new("static_01")?,
        characteristic_ability_id: AbilityId::new("characteristic_01")?,
        presentation: AbilityPresentation::OracleLines(vec![1]),
        source_is_land: true,
        source_is_creature: true,
        source_is_vehicle: true,
    };
    let mut ids = std::collections::BTreeSet::new();
    for recipe in catalog {
        if !ids.insert(recipe.id) {
            return Err(format!("duplicate recipe id {}", recipe.id.as_str()));
        }
        if recipe.calibration.positive_cards.len() < 2 {
            return Err(format!(
                "{} needs at least two positive calibration cards",
                recipe.id.as_str()
            ));
        }
        let names = recipe
            .calibration
            .positive_cards
            .iter()
            .map(|card| card.name)
            .collect::<std::collections::BTreeSet<_>>();
        if names.len() < 2 {
            return Err(format!(
                "{} needs at least two distinct named calibration cards",
                recipe.id.as_str()
            ));
        }
        if recipe.calibration.negative_near_misses.is_empty() {
            return Err(format!(
                "{} needs reviewed negative near-misses",
                recipe.id.as_str()
            ));
        }

        let is_spell = matches!(recipe.surface, RecipeSurface::SpellClause);
        for positive in recipe.calibration.positive_cards {
            if (recipe.matcher)(positive.clause, &context).is_none() {
                return Err(format!(
                    "{} positive calibration {} did not match its recipe",
                    recipe.id.as_str(),
                    positive.name
                ));
            }
            let matched = match_clause_in(catalog, positive.clause, is_spell, &context)
                .map_err(|ambiguity| {
                    format!(
                        "{} positive calibration {} is ambiguous: {ambiguity}",
                        recipe.id.as_str(),
                        positive.name
                    )
                })?
                .ok_or_else(|| {
                    format!(
                        "{} positive calibration {} was not consumed by the catalog",
                        recipe.id.as_str(),
                        positive.name
                    )
                })?;
            if matched.id != recipe.id {
                return Err(format!(
                    "{} positive calibration {} matched {}",
                    recipe.id.as_str(),
                    positive.name,
                    matched.id.as_str()
                ));
            }
        }
        for negative in recipe.calibration.negative_near_misses {
            if (recipe.matcher)(negative, &context).is_some() {
                return Err(format!(
                    "{} accepted near-miss {negative:?}",
                    recipe.id.as_str()
                ));
            }
            match match_clause_in(catalog, negative, is_spell, &context) {
                Ok(None) => {}
                Ok(Some(matched)) => {
                    return Err(format!(
                        "catalog matched {} near-miss {negative:?} as {}",
                        recipe.id.as_str(),
                        matched.id.as_str()
                    ));
                }
                Err(ambiguity) => {
                    return Err(format!(
                        "catalog found {} near-miss {negative:?} ambiguous: {ambiguity}",
                        recipe.id.as_str()
                    ));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> RecipeContext {
        RecipeContext {
            triggered_ability_id: AbilityId::new("triggered_01").unwrap(),
            activated_ability_id: AbilityId::new("activated_01").unwrap(),
            static_ability_id: AbilityId::new("static_01").unwrap(),
            characteristic_ability_id: AbilityId::new("characteristic_01").unwrap(),
            presentation: AbilityPresentation::OracleLines(vec![1]),
            source_is_land: true,
            source_is_creature: true,
            source_is_vehicle: true,
        }
    }

    #[test]
    fn catalog_has_stable_unique_ids_and_complete_calibration_metadata() {
        validate_catalog().expect("built-in recipe catalog should be valid");
    }

    #[test]
    fn recipe_without_two_named_calibration_cards_fails_validation() {
        let incomplete = [Recipe {
            id: RecipeId("synthetic.incomplete"),
            label: "synthetic incomplete",
            surface: RecipeSurface::KeywordClause,
            matcher: duplicate_matcher,
            calibration: RecipeCalibration {
                positive_cards: &[CalibrationCard {
                    name: "Only One",
                    clause: "Synthetic",
                }],
                negative_near_misses: &["No"],
            },
        }];
        assert_eq!(
            validate_catalog_in(&incomplete),
            Err("synthetic.incomplete needs at least two positive calibration cards".into())
        );
    }

    fn duplicate_matcher(_: &str, _: &RecipeContext) -> Option<RecipeEmission> {
        Some(RecipeEmission::Keywords(vec![Keyword::Flying]))
    }

    static AMBIGUOUS_CATALOG: &[Recipe] = &[
        Recipe {
            id: RecipeId("synthetic.first"),
            label: "synthetic first",
            surface: RecipeSurface::KeywordClause,
            matcher: duplicate_matcher,
            calibration: calibrations!("First One" => "Synthetic", "First Two" => "Synthetic"; "No"),
        },
        Recipe {
            id: RecipeId("synthetic.second"),
            label: "synthetic second",
            surface: RecipeSurface::KeywordClause,
            matcher: duplicate_matcher,
            calibration: calibrations!("Second One" => "Synthetic", "Second Two" => "Synthetic"; "No"),
        },
    ];

    #[test]
    fn multiple_matches_fail_closed_with_all_stable_ids() {
        let error = match_clause_in(AMBIGUOUS_CATALOG, "Synthetic", false, &context())
            .expect_err("ambiguous clauses must fail closed");
        assert_eq!(
            error.recipe_ids,
            [RecipeId("synthetic.first"), RecipeId("synthetic.second")]
        );
        assert_eq!(
            error.to_string(),
            "clause matched multiple exact recipes: synthetic.first, synthetic.second"
        );
    }

    #[test]
    fn shockland_clause_has_one_stable_static_recipe_id() {
        let matched = match_clause(
            "As this land enters, you may pay 2 life. If you don't, it enters tapped.",
            false,
            &context(),
        )
        .expect("shockland clause must not be ambiguous")
        .expect("shockland clause must be supported");

        assert_eq!(
            matched.id,
            RecipeId("static.enters_tapped.unless_pay_life_2")
        );
    }

    #[test]
    fn issue_253_recipes_are_land_only_and_do_not_absorb_existing_mana_or_entry_forms() {
        let land = context();
        for clause in ["{T}: Add {G}.", "{T}: Add one mana of any color."] {
            assert!(
                match_tap_for_multicolor_mana(clause, &land).is_none(),
                "{clause}"
            );
        }
        assert!(match_unconditional_enters_tapped(
            "As this land enters, you may pay 2 life. If you don't, it enters tapped.",
            &land,
        )
        .is_none());

        let mut nonland = context();
        nonland.source_is_land = false;
        assert!(match_tap_for_multicolor_mana("{T}: Add {G} or {U}.", &nonland).is_none());
        assert!(match_unconditional_enters_tapped("This land enters tapped.", &nonland).is_none());
    }

    #[test]
    fn issue_254_utility_land_clauses_emit_exact_typed_recipes() {
        let cases = [
            (
                "When this land enters, you gain 1 life.",
                RecipeId("etb.land.gain_life.one"),
            ),
            (
                "When this land enters, scry 1.",
                RecipeId("etb.land.scry.one"),
            ),
            (
                "When this land enters, surveil 1.",
                RecipeId("etb.land.surveil.one"),
            ),
            (
                "When this land enters, it deals 1 damage to target opponent.",
                RecipeId("etb.land.damage.target_opponent.one"),
            ),
            (
                "{1}, {T}: Add one mana of any color.",
                RecipeId("activated.mana.pay_one_tap_any_color"),
            ),
        ];
        for (clause, expected_id) in cases {
            let matched = match_clause(clause, false, &context())
                .expect("utility-land clause must not be ambiguous")
                .unwrap_or_else(|| panic!("utility-land clause must be supported: {clause}"));
            assert_eq!(matched.id, expected_id, "{clause}");
        }

        let gain = match_clause("When this land enters, you gain 1 life.", false, &context())
            .unwrap()
            .unwrap();
        let RecipeEmission::TriggeredAbility(gain) = gain.emission else {
            panic!("land lifegain must emit a triggered ability");
        };
        assert_eq!(gain.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            gain.effect,
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(1)
            }]
        );

        let damage = match_clause(
            "When this land enters, it deals 1 damage to target opponent.",
            false,
            &context(),
        )
        .unwrap()
        .unwrap();
        let RecipeEmission::TriggeredAbility(damage) = damage.emission else {
            panic!("land damage must emit a triggered ability");
        };
        assert_eq!(
            damage.effect,
            [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(1),
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    ..TargetFilter::default()
                },
            }]
        );
        let targeting = damage.targeting.expect("opponent damage must target");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
        assert_eq!(targeting.groups[0].effect_indices, [0]);

        let mana = match_clause("{1}, {T}: Add one mana of any color.", false, &context())
            .unwrap()
            .unwrap();
        let RecipeEmission::ActivatedAbility(mana) = mana.emission else {
            panic!("paid five-color mana must emit an activated ability");
        };
        assert_eq!(
            mana.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{1}").unwrap()),
                AbilityCost::Tap,
            ]
        );
        assert_eq!(mana.mana_options().unwrap().len(), 5);
        assert!(
            match_tap_for_any_color("{1}, {T}: Add one mana of any color.", &context(),).is_none()
        );
        assert!(
            match_pay_one_tap_for_any_color("{T}: Add one mana of any color.", &context(),)
                .is_none()
        );

        let mut nonland = context();
        nonland.source_is_land = false;
        for (clause, _) in cases {
            assert!(
                match_clause(clause, false, &nonland).unwrap().is_none(),
                "{clause}"
            );
        }
    }

    #[test]
    fn prowess_clause_has_one_stable_triggered_recipe_id() {
        let matched = match_clause("Prowess", false, &context())
            .expect("prowess clause must not be ambiguous")
            .expect("prowess clause must be supported");

        assert_eq!(matched.id, RecipeId("triggered.prowess"));
        assert!(matches!(
            matched.emission,
            RecipeEmission::TriggeredAbility(_)
        ));
    }

    #[test]
    fn sacrifice_to_naturalize_clause_has_one_stable_activated_recipe_id() {
        let matched = match_clause(
            "{1}, Sacrifice this creature: Destroy target artifact or enchantment.",
            false,
            &context(),
        )
        .expect("sacrifice-to-Naturalize clause must not be ambiguous")
        .expect("sacrifice-to-Naturalize clause must be supported");

        assert_eq!(
            matched.id,
            RecipeId("activated.sacrifice_self.destroy_artifact_or_enchantment")
        );
        assert!(matches!(
            matched.emission,
            RecipeEmission::ActivatedAbility(_)
        ));
    }

    #[test]
    fn issue_255_land_draw_clause_has_one_stable_activated_recipe_id() {
        let matched = match_clause(
            "{4}, {T}, Sacrifice this land: Draw a card.",
            false,
            &context(),
        )
        .expect("tap-sacrifice land draw clause must not be ambiguous")
        .expect("tap-sacrifice land draw clause must be supported");

        assert_eq!(
            matched.id,
            RecipeId("activated.land.tap_sacrifice.draw_one")
        );
        let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
            panic!("tap-sacrifice land draw recipe must emit an activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{4}").unwrap()),
                AbilityCost::Tap,
                AbilityCost::SacrificeSelf,
            ]
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        );
        assert!(ability.targeting.is_none());
        assert_eq!(ability.timing, ActivationTiming::Normal);

        let mut nonland = context();
        nonland.source_is_land = false;
        assert!(match_land_tap_sacrifice_draw_one(
            "{4}, {T}, Sacrifice this land: Draw a card.",
            &nonland,
        )
        .is_none());
    }

    #[test]
    fn issue_258_land_clauses_emit_exact_typed_recipes() {
        let cases = [
            (
                "{4}, {T}: Surveil 1.",
                "activated.land.pay_four_tap.surveil_one",
            ),
            (
                "This land enters tapped unless you control two or fewer other lands.",
                "static.enters_tapped.land_count.fast",
            ),
            (
                "This land enters tapped unless a player has 13 or less life.",
                "static.enters_tapped.player_life.minimum_fourteen",
            ),
            (
                "This land enters tapped unless you control two or more other lands.",
                "static.enters_tapped.land_count.slow",
            ),
        ];

        let emissions = cases.map(|(clause, expected_id)| {
            let matched = match_clause(clause, false, &context())
                .expect("issue #258 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #258 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
            matched.emission
        });

        let RecipeEmission::ActivatedAbility(surveil) = &emissions[0] else {
            panic!("Surveil recipe must emit an activated ability");
        };
        assert_eq!(surveil.ability_id.as_str(), "activated_01");
        assert_eq!(
            surveil.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{4}").unwrap()),
                AbilityCost::Tap,
            ]
        );
        assert_eq!(
            surveil.effect,
            [SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            }]
        );
        assert!(surveil.targeting.is_none());
        assert_eq!(surveil.timing, ActivationTiming::Normal);

        let expected_land_count = |min, max| StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: Some(GameCondition::BattlefieldAggregate {
                filter: BattlefieldPermanentFilter {
                    token: None,
                    any_of: None,
                    controllers: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::Land),
                    color: None,
                    name: None,
                    required_subtypes: Vec::new(),
                    exclude_source: true,
                },
                aggregate: BattlefieldAggregate::Count,
                min,
                max,
            }),
            unless_cost: None,
        };
        for (index, expected) in [
            (1, expected_land_count(Some(3), None)),
            (3, expected_land_count(None, Some(1))),
        ] {
            let RecipeEmission::StaticAbility(ability) = &emissions[index] else {
                panic!("land-count recipe must emit a static ability");
            };
            assert_eq!(ability.ability_id.as_str(), "static_01");
            assert_eq!(ability.definition, expected);
        }

        let RecipeEmission::StaticAbility(life) = &emissions[2] else {
            panic!("life-threshold recipe must emit a static ability");
        };
        assert_eq!(life.ability_id.as_str(), "static_01");
        assert_eq!(
            life.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: Some(GameCondition::PlayerLifeAggregate {
                    players: RelativePlayerSet::All,
                    aggregate: PlayerLifeAggregate::Minimum,
                    min: Some(14),
                    max: None,
                }),
                unless_cost: None,
            }
        );

        let mut nonland = context();
        nonland.source_is_land = false;
        for (clause, _) in cases {
            assert!(
                match_clause(clause, false, &nonland).unwrap().is_none(),
                "{clause}"
            );
        }
    }

    #[test]
    fn etb_explore_clause_emits_one_source_bound_trigger() {
        let matched = match_clause("When this creature enters, it explores.", false, &context())
            .expect("Explore clause must not be ambiguous")
            .expect("Explore clause must be supported");

        assert_eq!(matched.id, RecipeId("etb.explore.source"));
        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("Explore must emit a triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Explore {
                subject: EffectSubject::Source,
            }]
        );
        assert!(ability.targeting.is_none());
        assert!(!ability.may);
    }

    #[test]
    fn cycling_clauses_have_distinct_stable_zone_ability_recipe_ids() {
        for (clause, is_spell, expected_id) in [
            ("Cycling {2}", true, "activated.hand.cycling.draw"),
            (
                "Basic landcycling {1}{G}",
                false,
                "activated.hand.typecycling.basic_land",
            ),
            (
                "Mountaincycling {2}",
                false,
                "activated.hand.typecycling.basic_land_type",
            ),
        ] {
            let matched = match_clause(clause, is_spell, &context())
                .expect("cycling clause must not be ambiguous")
                .expect("cycling clause must be supported");
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
            assert!(matches!(
                matched.emission,
                RecipeEmission::ActivatedAbility(_)
            ));
        }
    }

    #[test]
    fn issue_257_clauses_have_distinct_stable_recipe_ids() {
        for (clause, expected_id) in [
            ("Crew 3", "activated.crew.aggregate_power"),
            ("Ward {2}", "triggered.ward.mana"),
            ("Changeling", "characteristic.changeling"),
            (
                "This spell can't be countered.",
                "static.spell.cannot_be_countered",
            ),
        ] {
            let matched = match_clause(clause, false, &context())
                .expect("issue #257 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #257 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
        }
    }

    #[test]
    fn issue_257_crew_emits_generation_bound_aggregate_payment_and_animation() {
        let matched = match_clause("Crew 4", false, &context())
            .unwrap()
            .expect("ordinary Crew must be supported");
        let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
            panic!("Crew must emit an activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(
            ability.costs,
            [AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::AggregateMinimum {
                    minimum: 4,
                    contribution: ObjectContributionKind::CurrentPower,
                },
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
                exclude_source: true,
            }]
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::AddTypes {
                subject: EffectSubject::Source,
                addition: TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Creature],
                    creature_types: Vec::new(),
                },
            }]
        );

        let mut nonvehicle = context();
        nonvehicle.source_is_vehicle = false;
        assert!(match_crew("Crew 4", &nonvehicle).is_none());
    }

    #[test]
    fn issue_257_mana_ward_emits_opponent_target_trigger_and_exact_cost() {
        let matched = match_clause("Ward {2}{U}", false, &context())
            .unwrap()
            .expect("non-X mana Ward must be supported");
        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("Ward must emit a triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverSelfBecomesTarget {
                source: TargetingSourceFilter::SpellOrAbility,
                source_controller: CastTriggerPlayer::Opponent,
            }
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::CounterTriggeringStackObjectUnlessPays {
                cost: ResolutionCost::Mana(ManaCost::parse("{2}{U}").unwrap()),
            }]
        );
    }

    #[test]
    fn issue_257_changeling_uses_characteristic_surface_and_stable_identity() {
        let matched = match_clause("Changeling", false, &context())
            .unwrap()
            .expect("Changeling must be supported");
        let RecipeEmission::CharacteristicAbility(ability) = matched.emission else {
            panic!("Changeling must emit a characteristic-defining ability");
        };
        assert_eq!(ability.ability_id.as_str(), "characteristic_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(
            ability.definition,
            CharacteristicDefiningAbility::Changeling
        );
    }

    #[test]
    fn issue_257_self_uncounterability_emits_stack_static_ability() {
        let matched = match_clause("This spell can't be countered.", true, &context())
            .unwrap()
            .expect("self uncounterability must apply on instant and sorcery faces too");
        let RecipeEmission::StaticAbility(ability) = matched.emission else {
            panic!("self uncounterability must emit a static ability");
        };
        assert_eq!(ability.ability_id.as_str(), "static_01");
        assert_eq!(ability.definition, StaticAbilityDef::SpellCannotBeCountered);
    }

    #[test]
    fn issue_260_creature_trigger_clauses_emit_exact_typed_abilities() {
        let cases = [
            (
                "Whenever this creature attacks, surveil 1.",
                "triggered.self_attacks.surveil.one",
            ),
            (
                "When this creature enters, create a Map token.",
                "etb.create_token.map.one",
            ),
            (
                "When this creature enters, mill two cards.",
                "etb.mill.controller.two",
            ),
            (
                "When this creature enters, return target creature an opponent controls to its owner's hand.",
                "etb.return_to_hand.opponent_creature",
            ),
            (
                "Whenever this creature deals combat damage to a player, create a Food token.",
                "triggered.self_combat_damage_to_player.create_token.food.one",
            ),
        ];

        let abilities = cases.map(|(clause, expected_id)| {
            let matched = match_clause(clause, false, &context())
                .expect("issue #260 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #260 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("issue #260 recipe must emit a triggered ability: {clause}");
            };
            assert_eq!(ability.ability_id.as_str(), "triggered_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert!(!ability.may);
            ability
        });

        assert_eq!(
            abilities[0].trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0,
            }
        );
        assert_eq!(
            abilities[0].effect,
            [SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            }]
        );

        for (index, token, trigger) in [
            (1, "map", TriggerCondition::WhenSelfEntersBattlefield),
            (
                4,
                "food",
                TriggerCondition::WheneverSelfDealsCombatDamageToPlayer,
            ),
        ] {
            assert_eq!(abilities[index].trigger, trigger);
            assert_eq!(
                abilities[index].effect,
                [SpellEffectKind::CreateTokens {
                    token: token.into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }]
            );
        }

        assert_eq!(
            abilities[2].trigger,
            TriggerCondition::WhenSelfEntersBattlefield
        );
        assert_eq!(
            abilities[2].effect,
            [SpellEffectKind::Mill {
                count: Amount::Fixed(2),
                who: PlayerRecipient::Controller,
            }]
        );

        assert_eq!(
            abilities[3].trigger,
            TriggerCondition::WhenSelfEntersBattlefield
        );
        assert_eq!(
            abilities[3].effect,
            [SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::Opponent,
                    ..TargetFilter::default()
                })),
            }]
        );
        let targeting = abilities[3]
            .targeting
            .as_ref()
            .expect("bounce trigger must publish its target group");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
        assert_eq!(targeting.groups[0].effect_indices, [0]);
    }

    #[test]
    fn issue_259_creature_etb_clauses_emit_exact_typed_abilities() {
        let cases = [
            (
                "When this creature enters, exile the top card of your library. Until the end of your next turn, you may play that card.",
                "etb.exile_top.play_permission.end_of_next_turn",
            ),
            (
                "When this creature enters, put a +1/+1 counter on target creature.",
                "etb.put_counter.plus_one_plus_one.target_creature.one",
            ),
            (
                "When this creature enters, create a 1/1 white Ally creature token.",
                "etb.create_token.ally_w_1_1.one",
            ),
            (
                "When this creature enters, draw a card, then discard a card.",
                "etb.draw_discard.controller.one_one",
            ),
        ];

        let abilities = cases.map(|(clause, expected_id)| {
            let matched = match_clause(clause, false, &context())
                .expect("issue #259 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #259 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("issue #259 recipe must emit a triggered ability: {clause}");
            };
            assert_eq!(ability.ability_id.as_str(), "triggered_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(!ability.may);
            ability
        });

        assert_eq!(
            abilities[0].effect,
            [SpellEffectKind::ExileTopWithPlayPermission {
                player: PlayerRecipient::Controller,
                count: 1,
                count_by_cast_cost: None,
            }]
        );
        assert_eq!(
            abilities[1].effect,
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            }]
        );
        let targeting = abilities[1]
            .targeting
            .as_ref()
            .expect("counter trigger must publish its target group");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
        assert_eq!(targeting.groups[0].effect_indices, [0]);
        assert_eq!(
            abilities[2].effect,
            [SpellEffectKind::CreateTokens {
                token: "ally_w_1_1".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }]
        );
        assert_eq!(
            abilities[3].effect,
            [SpellEffectKind::DrawDiscard {
                who: PlayerRecipient::Controller,
                draw_count: 1,
                discard_count: 1,
                order: DrawDiscardOrder::DrawThenDiscard,
                optional: false,
            }]
        );
    }
}
