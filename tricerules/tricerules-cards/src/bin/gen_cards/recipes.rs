use tricerules_cards::primitives::{
    ActivationLimit, BattlefieldAggregate, BattlefieldPermanentFilter, CardTypeFilter,
    CreatureScopeController, CreatureScopeFilter, DiscardQuantity, DrawDiscardOrder, EffectSubject,
    EntersTappedAffected, EntryCost, GameCondition, GraveyardDestination, GraveyardFilter,
    HandCardAction, LifeAmount, ObjectContributionKind, ObjectPaymentConstraint,
    PermanentEventFilter, PermanentTypeFilter, PlayerLifeAggregate, PlayerRecipient,
    RelativePlayerSet, ResolutionCost, SearchDestination, SearchZoneSelection, SpellCastFilter,
    SpellCostModifier, SpellManaSpentComparison, StackSpellFilter, StaticAbilityDef,
    TargetController, TargetFilter, TargetGroupDef, TargetKind, TargetMatchFilter,
    TargetObjectExclusion, TargetingDef, TargetingSourceFilter, TypeLineAddition, ZoneCardFilter,
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
    /// A clause that defines how an Aura permanent spell enters attached. Aura cards are
    /// permanents, so this stays separate from instant/sorcery [`Self::SpellClause`] recipes.
    AuraSpellClause,
    /// One complete bullet body inside an exact modal-spell assembly. Keeping this separate from
    /// ordinary spell clauses prevents a modal-only body from qualifying unrelated cards.
    ModalMode,
    /// A complete modal Oracle-text aggregate. The assembly owns the header and every bullet;
    /// individual bullet mechanics are matched again on [`Self::ModalMode`].
    ModalAssembly,
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
    /// Some exact templates have only one real-card calibration in the pinned corpus. Such
    /// recipes opt into a documented singleton rather than inventing a second card example.
    pub(super) minimum_positive_cards: usize,
}

pub(super) struct Recipe {
    pub(super) id: RecipeId,
    pub(super) label: &'static str,
    pub(super) surface: RecipeSurface,
    pub(super) matcher: fn(&str, &RecipeContext) -> Option<RecipeEmission>,
    pub(super) calibration: RecipeCalibration,
}

#[derive(Debug, Clone)]
pub(super) struct RecipeContext {
    pub(super) triggered_ability_id: AbilityId,
    pub(super) activated_ability_id: AbilityId,
    pub(super) static_ability_id: AbilityId,
    pub(super) characteristic_ability_id: AbilityId,
    pub(super) presentation: AbilityPresentation,
    pub(super) source_name: String,
    pub(super) source_is_artifact: bool,
    pub(super) source_is_land: bool,
    pub(super) source_is_creature: bool,
    pub(super) source_is_vehicle: bool,
    pub(super) source_is_aura: bool,
    pub(super) source_is_equipment: bool,
    pub(super) source_is_enchantment: bool,
    pub(super) source_is_instant: bool,
    pub(super) source_is_sorcery: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RecipeEmission {
    Keywords(Vec<Keyword>),
    SpellEffect(SpellEffectKind),
    SpellEffects(Vec<SpellEffectKind>),
    SpellEffectsWithTargeting {
        effects: Vec<SpellEffectKind>,
        targeting: TargetingDef,
    },
    SpellCostModifier(SpellCostModifier),
    TriggeredAbility(TriggeredAbilityDef),
    TriggeredAbilities(Vec<TriggeredAbilityDef>),
    ActivatedAbility(ActivatedAbilityDef),
    StaticAbility(IdentifiedAbility<StaticAbilityDef>),
    CharacteristicAbility(IdentifiedAbility<CharacteristicDefiningAbility>),
    ModalMode(ModalModeEmission),
    ModalAssembly(ModalAssemblyEmission),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ModalModeEmission {
    pub(super) effects: Vec<SpellEffectKind>,
    pub(super) targeting: Option<TargetingDef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ModalAssemblyEmission {
    pub(super) min_modes: u32,
    pub(super) max_modes: u32,
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

fn chosen_creature(controller: TargetController) -> EffectSubject {
    EffectSubject::Chosen(Box::new(TargetFilter {
        kind: TargetKind::Creature,
        controller,
        ..TargetFilter::default()
    }))
}

fn combat_trick(
    controller: TargetController,
    power: i32,
    toughness: i32,
    keywords: &[Keyword],
    untap: bool,
) -> RecipeEmission {
    let mut effects = vec![SpellEffectKind::PumpTarget {
        power,
        toughness,
        scale: None,
        subject: chosen_creature(controller),
    }];
    effects.push(SpellEffectKind::GrantKeywords {
        subject: chosen_creature(controller),
        keywords: keywords.to_vec(),
    });
    if untap {
        effects.push(SpellEffectKind::Untap {
            subject: chosen_creature(controller),
        });
    }
    RecipeEmission::SpellEffects(effects)
}

fn match_spell_pump_three_grant_trample(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Target creature gets +3/+3 and gains trample until end of turn.")
        .then(|| combat_trick(TargetController::Any, 3, 3, &[Keyword::Trample], false))
}

fn match_spell_controlled_creature_plus_zero_three_hexproof(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Target creature you control gets +0/+3 and gains hexproof until end of turn.")
        .then(|| combat_trick(TargetController::You, 0, 3, &[Keyword::Hexproof], false))
}

fn match_spell_controlled_creature_plus_one_one_hexproof_untap(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text
        == "Target creature you control gets +1/+1 and gains hexproof until end of turn. Untap it.")
        .then(|| combat_trick(TargetController::You, 1, 1, &[Keyword::Hexproof], true))
}

fn match_spell_creature_plus_one_three_reach_untap(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Target creature gets +1/+3 and gains reach until end of turn. Untap it.")
        .then(|| combat_trick(TargetController::Any, 1, 3, &[Keyword::Reach], true))
}

fn match_spell_creature_minus_three_three(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Target creature gets -3/-3 until end of turn.").then(|| {
        RecipeEmission::SpellEffects(vec![SpellEffectKind::PumpTarget {
            power: -3,
            toughness: -3,
            scale: None,
            subject: chosen_creature(TargetController::Any),
        }])
    })
}

fn match_spell_creature_deathtouch_indestructible(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    let controller =
        if text == "Target creature gains deathtouch and indestructible until end of turn." {
            TargetController::Any
        } else if text
            == "Target creature you control gains deathtouch and indestructible until end of turn."
        {
            TargetController::You
        } else {
            return None;
        };
    Some(RecipeEmission::SpellEffects(vec![
        SpellEffectKind::GrantKeywords {
            subject: chosen_creature(controller),
            keywords: vec![Keyword::Deathtouch, Keyword::Indestructible],
        },
    ]))
}

fn match_spell_draw_three_then_discard_one(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Draw three cards, then discard a card.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::DrawDiscard {
            who: PlayerRecipient::Controller,
            draw_count: 3,
            discard_count: 1,
            order: DrawDiscardOrder::DrawThenDiscard,
            optional: false,
        })
    })
}

fn match_spell_creature_plus_four_four_trample(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Target creature gets +4/+4 and gains trample until end of turn.")
        .then(|| combat_trick(TargetController::Any, 4, 4, &[Keyword::Trample], false))
}

fn match_spell_controlled_creature_plus_one_then_power_damage(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    let optional_target = if text
        == "Target creature you control gets +1/+0 until end of turn. It deals damage equal to its power to up to one target creature an opponent controls."
    {
        true
    } else if text
        == "Target creature you control gets +1/+0 until end of turn. It deals damage equal to its power to target creature an opponent controls."
    {
        false
    } else {
        return None;
    };
    Some({
        let source = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..TargetFilter::default()
        };
        let target = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::Opponent,
            ..TargetFilter::default()
        };
        RecipeEmission::SpellEffectsWithTargeting {
            effects: vec![
                SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 0,
                    scale: None,
                    subject: EffectSubject::Chosen(Box::new(source.clone())),
                },
                SpellEffectKind::CreatureDealsDamageEqualToPower { source, target },
            ],
            targeting: TargetingDef {
                groups: vec![
                    TargetGroupDef {
                        min: 1,
                        max: 1,
                        prompt: "Choose target creature you control".into(),
                        effect_indices: vec![0, 1],
                        distinct_from: Vec::new(),
                        same_graveyard: false,
                        cast_cost_expansion: None,
                    },
                    TargetGroupDef {
                        min: u32::from(!optional_target),
                        max: 1,
                        prompt: "Choose up to one target creature an opponent controls".into(),
                        effect_indices: vec![1],
                        distinct_from: Vec::new(),
                        same_graveyard: false,
                        cast_cost_expansion: None,
                    },
                ],
            },
        }
    })
}

fn match_spell_tapped_creature_target_reduction_three(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_instant
        && text == "This spell costs {3} less to cast if it targets a tapped creature.")
        .then(|| {
            RecipeEmission::SpellCostModifier(SpellCostModifier::TargetMatchGenericReduction {
                amount: 3,
                filter: TargetMatchFilter::Battlefield(TargetFilter {
                    kind: TargetKind::Creature,
                    tapped: Some(true),
                    ..TargetFilter::default()
                }),
            })
        })
}

fn match_affinity_for_artifacts(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Affinity for artifacts").then(|| {
        RecipeEmission::SpellCostModifier(SpellCostModifier::BattlefieldCountGenericReduction {
            amount_per_match: 1,
            filter: BattlefieldPermanentFilter {
                token: None,
                any_of: None,
                controllers: RelativePlayerSet::Controller,
                card_type: Some(CardTypeFilter::Artifact),
                color: None,
                name: None,
                required_subtypes: Vec::new(),
                exclude_source: false,
            },
            aggregate: BattlefieldAggregate::Count,
        })
    })
}

fn match_spell_surveil_two_draw_two_lose_two(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_sorcery && text == "Surveil 2, then draw two cards. You lose 2 life.").then(
        || {
            RecipeEmission::SpellEffects(vec![
                SpellEffectKind::LibraryPartition {
                    count: 2,
                    top_min: 0,
                    top_max: None,
                    kind: LibraryPartitionKind::Surveil,
                },
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(2),
                },
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(2),
                    who: PlayerRecipient::Controller,
                },
            ])
        },
    )
}

fn match_spell_return_nonland_then_surveil_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_instant
        && text == "Return target nonland permanent to its owner's hand. Surveil 1.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::AnyPermanent,
                excluded_permanent_types: vec![PermanentTypeFilter::Land],
                ..TargetFilter::default()
            };
            RecipeEmission::SpellEffectsWithTargeting {
                effects: vec![
                    SpellEffectKind::ReturnToOwnersHand {
                        subject: EffectSubject::Chosen(Box::new(target)),
                    },
                    SpellEffectKind::LibraryPartition {
                        count: 1,
                        top_min: 0,
                        top_max: None,
                        kind: LibraryPartitionKind::Surveil,
                    },
                ],
                targeting: TargetingDef {
                    groups: vec![TargetGroupDef {
                        min: 1,
                        max: 1,
                        prompt: "Choose target nonland permanent".into(),
                        effect_indices: vec![0],
                        distinct_from: Vec::new(),
                        same_graveyard: false,
                        cast_cost_expansion: None,
                    }],
                },
            }
        })
}

fn match_spell_controlled_creature_power_damage(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_instant
        && text
            == "Target creature you control deals damage equal to its power to target creature an opponent controls.")
        .then(|| {
            let source = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            };
            let target = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                ..TargetFilter::default()
            };
            RecipeEmission::SpellEffectsWithTargeting {
                effects: vec![SpellEffectKind::CreatureDealsDamageEqualToPower {
                    source,
                    target,
                }],
                targeting: TargetingDef {
                    groups: vec![
                        TargetGroupDef {
                            min: 1,
                            max: 1,
                            prompt: "Choose target creature you control".into(),
                            effect_indices: vec![0],
                            distinct_from: Vec::new(),
                            same_graveyard: false,
                            cast_cost_expansion: None,
                        },
                        TargetGroupDef {
                            min: 1,
                            max: 1,
                            prompt: "Choose target creature an opponent controls".into(),
                            effect_indices: vec![0],
                            distinct_from: Vec::new(),
                            same_graveyard: false,
                            cast_cost_expansion: None,
                        },
                    ],
                },
            }
        })
}

fn match_spell_return_graveyard_card_to_hand(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Return target card from your graveyard to your hand.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter::default(),
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
        })
    })
}

fn match_spell_controlled_creature_power_damage_to_noncontroller_permanent(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    ((context.source_is_instant || context.source_is_sorcery)
        && text
            == "Target creature you control deals damage equal to its power to target creature or planeswalker you don't control.")
        .then(|| {
            let source = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            };
            let target = TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::NotYou,
                permanent_types: vec![
                    PermanentTypeFilter::Creature,
                    PermanentTypeFilter::Planeswalker,
                ],
                ..TargetFilter::default()
            };
            RecipeEmission::SpellEffectsWithTargeting {
                effects: vec![SpellEffectKind::CreatureDealsDamageEqualToPower {
                    source,
                    target,
                }],
                targeting: TargetingDef {
                    groups: vec![
                        TargetGroupDef {
                            min: 1,
                            max: 1,
                            prompt: "Choose target creature you control".into(),
                            effect_indices: vec![0],
                            distinct_from: vec![1],
                            same_graveyard: false,
                            cast_cost_expansion: None,
                        },
                        TargetGroupDef {
                            min: 1,
                            max: 1,
                            prompt: "Choose target creature or planeswalker you don't control"
                                .into(),
                            effect_indices: vec![0],
                            distinct_from: vec![0],
                            same_graveyard: false,
                            cast_cost_expansion: None,
                        },
                    ],
                },
            }
        })
}

fn legendary_creature_search_effect() -> SpellEffectKind {
    SpellEffectKind::SearchLibrary {
        who: PlayerRecipient::Controller,
        optional: false,
        count: 1,
        count_by_cast_cost: None,
        filter: Some(ZoneCardFilter {
            card_type: Some(CardTypeFilter::Creature),
            required_supertypes: vec!["Legendary".into()],
            ..ZoneCardFilter::default()
        }),
        slots: Vec::new(),
        zones: SearchZoneSelection::default(),
        destination: SearchDestination::Hand,
        conditional_destination: None,
        shuffle: true,
        reveal: true,
        result_id: None,
    }
}

fn match_spell_search_legendary_creature(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Search your library for a legendary creature card, reveal it, put it into your hand, then shuffle.")
        .then(|| RecipeEmission::SpellEffect(legendary_creature_search_effect()))
}

fn match_artifact_etb_search_legendary_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    let expected = format!(
        "When {} enters, search your library for a legendary creature card, reveal it, put it into your hand, then shuffle.",
        context.source_name
    );
    (context.source_is_artifact && text == expected)
        .then(|| triggered_ability(context, legendary_creature_search_effect()))
}

fn match_spell_create_two_rat_tokens_cant_block(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Create two 1/1 black Rat creature tokens with \"This token can't block.\"").then(
        || {
            RecipeEmission::SpellEffect(SpellEffectKind::CreateTokens {
                token: "rat_b_1_1_cant_block".into(),
                count: Amount::Fixed(2),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            })
        },
    )
}

fn match_spell_source_damage_each_opponent_three(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == format!("{} deals 3 damage to each opponent.", context.source_name)).then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(3),
            who: PlayerRecipient::EachOpponent,
        })
    })
}

fn match_spell_source_damage_creature_four(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == format!("{} deals 4 damage to target creature.", context.source_name)).then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(4),
            target: TargetFilter::default_creature(),
        })
    })
}

fn match_spell_destroy_artifact_or_enchantment(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Destroy target artifact or enchantment.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    PermanentTypeFilter::Artifact,
                    PermanentTypeFilter::Enchantment,
                ],
                ..TargetFilter::default()
            })),
        })
    })
}

fn match_spell_destroy_creature_or_planeswalker(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Destroy target creature or planeswalker.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                permanent_types: vec![
                    PermanentTypeFilter::Creature,
                    PermanentTypeFilter::Planeswalker,
                ],
                ..TargetFilter::default()
            })),
        })
    })
}

fn match_spell_team_plus_three_trample(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Creatures you control get +3/+3 and gain trample until end of turn.").then(|| {
        RecipeEmission::SpellEffects(vec![
            SpellEffectKind::PumpAll {
                filter: creatures_you_control(),
                power: 3,
                toughness: 3,
            },
            SpellEffectKind::GrantKeywordsAll {
                filter: creatures_you_control(),
                keywords: vec![Keyword::Trample],
            },
        ])
    })
}

fn match_spell_destroy_creature_then_gain_two(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Destroy target creature. You gain 2 life.").then(|| {
        RecipeEmission::SpellEffects(vec![
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),
            },
        ])
    })
}

fn match_spell_creature_minus_two_two(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Target creature gets -2/-2 until end of turn.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::PumpTarget {
            power: -2,
            toughness: -2,
            scale: None,
            subject: chosen_creature(TargetController::Any),
        })
    })
}

fn match_spell_destroy_attacking_or_blocking_creature(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Destroy target attacking or blocking creature.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                combat_role: Some(tricerules_cards::primitives::CombatRole::AttackingOrBlocking),
                ..TargetFilter::default()
            })),
        })
    })
}

fn match_spell_exile_creature(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Exile target creature.").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
        })
    })
}

fn match_spell_creature_plus_three_zero_then_draw(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Target creature gets +3/+0 until end of turn.\nDraw a card.").then(|| {
        RecipeEmission::SpellEffects(vec![
            SpellEffectKind::PumpTarget {
                power: 3,
                toughness: 0,
                scale: None,
                subject: chosen_creature(TargetController::Any),
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        ])
    })
}

fn attached_modifier(
    context: &RecipeContext,
    delta_power: i32,
    delta_toughness: i32,
    keywords: Vec<Keyword>,
    doesnt_untap_during_untap_step: bool,
) -> RecipeEmission {
    RecipeEmission::StaticAbility(IdentifiedAbility {
        ability_id: context.static_ability_id.clone(),
        presentation: context.presentation.clone(),
        definition: StaticAbilityDef::AttachedModifier {
            condition: None,
            add_types: TypeLineAddition::default(),
            set_types: None,
            set_name: None,
            set_colors: None,
            delta_power,
            delta_toughness,
            set_power: None,
            set_toughness: None,
            remove_all_abilities: false,
            keywords,
            triggered_abilities: Vec::new(),
            activated_abilities: Vec::new(),
            restriction: Default::default(),
            doesnt_untap_during_untap_step,
            cant_untap: false,
        },
    })
}

fn match_enchant_creature(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_aura && text == "Enchant creature").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::AuraAttach {
            target: TargetFilter::default_creature(),
        })
    })
}

fn match_fixed_generic_equip(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    if !context.source_is_equipment {
        return None;
    }
    let value = text.strip_prefix("Equip {")?.strip_suffix('}')?;
    let generic = value.parse::<u32>().ok()?;
    if generic.to_string() != value {
        return None;
    }
    let mana_cost = exact_mana_cost(&format!("{{{generic}}}"))?;
    Some(RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Mana(mana_cost)],
        effect: vec![SpellEffectKind::Equip {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    }))
}

fn parse_signed_delta(value: &str) -> Option<i32> {
    let parsed = value.parse::<i32>().ok()?;
    let canonical = if parsed >= 0 {
        format!("+{parsed}")
    } else {
        parsed.to_string()
    };
    (canonical == value).then_some(parsed)
}

fn parse_attachment_keywords(value: &str) -> Option<Vec<Keyword>> {
    let normalized = value.replace(", and ", ", ").replace(" and ", ", ");
    let mut keywords = Vec::new();
    for token in normalized.split(", ") {
        let keyword = keyword_ident(token)?;
        if token.is_empty() || keywords.contains(&keyword) {
            return None;
        }
        keywords.push(keyword);
    }
    (!keywords.is_empty()).then_some(keywords)
}

fn match_attached_creature_modifier(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    let body = if let Some(body) = text.strip_prefix("Equipped creature ") {
        context.source_is_equipment.then_some(body)
    } else if let Some(body) = text.strip_prefix("Enchanted creature ") {
        context.source_is_aura.then_some(body)
    } else {
        None
    }?
    .strip_suffix('.')?;

    let (delta_power, delta_toughness, keywords) = if let Some(keywords) = body.strip_prefix("has ")
    {
        (0, 0, parse_attachment_keywords(keywords)?)
    } else {
        let stats_and_keywords = body.strip_prefix("gets ")?;
        let (stats, keywords) = match stats_and_keywords.split_once(" and has ") {
            Some((stats, keywords)) => (stats, parse_attachment_keywords(keywords)?),
            None => (stats_and_keywords, Vec::new()),
        };
        let (power, toughness) = stats.split_once('/')?;
        (
            parse_signed_delta(power)?,
            parse_signed_delta(toughness)?,
            keywords,
        )
    };
    Some(attached_modifier(
        context,
        delta_power,
        delta_toughness,
        keywords,
        false,
    ))
}

fn match_aura_etb_tap_attached(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_aura && text == "When this Aura enters, tap enchanted creature.").then(
        || {
            triggered_ability(
                context,
                SpellEffectKind::Tap {
                    subject: EffectSubject::AttachedObject,
                },
            )
        },
    )
}

fn match_aura_untap_step_restriction(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_aura
        && text == "Enchanted creature doesn't untap during its controller's untap step.")
        .then(|| attached_modifier(context, 0, 0, Vec::new(), true))
}

fn modal_targeting(prompt: &str, effect_index: u32) -> Option<TargetingDef> {
    modal_targeting_groups(vec![(prompt, vec![effect_index])])
}

fn modal_targeting_groups(groups: Vec<(&str, Vec<u32>)>) -> Option<TargetingDef> {
    Some(TargetingDef {
        groups: groups
            .into_iter()
            .map(|(prompt, effect_indices)| TargetGroupDef {
                min: 1,
                max: 1,
                prompt: prompt.into(),
                effect_indices,
                distinct_from: Vec::new(),
                same_graveyard: false,
                cast_cost_expansion: None,
            })
            .collect(),
    })
}

fn modal_mode(effects: Vec<SpellEffectKind>, targeting: Option<TargetingDef>) -> RecipeEmission {
    RecipeEmission::ModalMode(ModalModeEmission { effects, targeting })
}

fn match_modal_two_modes(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 3
        || !lines[1].starts_with("• ")
        || !lines[2].starts_with("• ")
        || lines[1].trim_start_matches("• ").trim().is_empty()
        || lines[2].trim_start_matches("• ").trim().is_empty()
    {
        return None;
    }
    let (min_modes, max_modes) = match lines[0] {
        "Choose one —" => (1, 1),
        "Choose one or both —" => (1, 2),
        _ => return None,
    };
    Some(RecipeEmission::ModalAssembly(ModalAssemblyEmission {
        min_modes,
        max_modes,
    }))
}

fn match_modal_damage_three_to_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == format!("{} deals 3 damage to target creature.", context.source_name)).then(|| {
        modal_mode(
            vec![SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target: TargetFilter::default_creature(),
            }],
            modal_targeting("Choose target creature", 0),
        )
    })
}

fn match_modal_destroy_artifact(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Destroy target artifact.").then(|| {
        modal_mode(
            vec![SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![PermanentTypeFilter::Artifact],
                    ..TargetFilter::default()
                })),
            }],
            modal_targeting("Choose target artifact", 0),
        )
    })
}

fn creatures_you_control() -> CreatureScopeFilter {
    CreatureScopeFilter {
        controller: Some(CreatureScopeController::YouControl),
        ..CreatureScopeFilter::default()
    }
}

fn match_modal_team_plus_one(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Creatures you control get +1/+1 until end of turn.").then(|| {
        modal_mode(
            vec![SpellEffectKind::PumpAll {
                filter: creatures_you_control(),
                power: 1,
                toughness: 1,
            }],
            None,
        )
    })
}

fn match_modal_team_hexproof(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Creatures you control gain hexproof until end of turn.").then(|| {
        modal_mode(
            vec![SpellEffectKind::GrantKeywordsAll {
                filter: creatures_you_control(),
                keywords: vec![Keyword::Hexproof],
            }],
            None,
        )
    })
}

fn match_modal_team_plus_two_power(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Creatures you control get +2/+0 until end of turn.").then(|| {
        modal_mode(
            vec![SpellEffectKind::PumpAll {
                filter: creatures_you_control(),
                power: 2,
                toughness: 0,
            }],
            None,
        )
    })
}

fn match_modal_create_two_goblins(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Create two 1/1 red Goblin creature tokens.").then(|| {
        modal_mode(
            vec![SpellEffectKind::CreateTokens {
                token: "goblin_r_1_1".into(),
                count: Amount::Fixed(2),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }],
            None,
        )
    })
}

fn match_modal_pump_target_three(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Target creature gets +3/+3 until end of turn.").then(|| {
        modal_mode(
            vec![SpellEffectKind::PumpTarget {
                power: 3,
                toughness: 3,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            }],
            modal_targeting("Choose target creature", 0),
        )
    })
}

fn match_modal_destroy_flying_creature(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Destroy target creature with flying.").then(|| {
        modal_mode(
            vec![SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    required_keywords: vec![Keyword::Flying],
                    ..TargetFilter::default()
                })),
            }],
            modal_targeting("Choose target creature with flying", 0),
        )
    })
}

fn match_modal_counter_spell(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Counter target spell.").then(|| {
        modal_mode(
            vec![SpellEffectKind::CounterTargetSpell {
                spell_filter: StackSpellFilter::default(),
                unless_controller_pays: None,
                unless_controller_pays_by_cast_cost: None,
            }],
            modal_targeting("Choose target spell", 0),
        )
    })
}

fn match_modal_surveil_two_draw_two(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Surveil 2, then draw two cards.").then(|| {
        modal_mode(
            vec![
                SpellEffectKind::LibraryPartition {
                    count: 2,
                    top_min: 0,
                    top_max: None,
                    kind: LibraryPartitionKind::Surveil,
                },
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(2),
                },
            ],
            None,
        )
    })
}

fn match_modal_counter_spell_unless_four(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Counter target spell unless its controller pays {4}.").then(|| {
        modal_mode(
            vec![SpellEffectKind::CounterTargetSpell {
                spell_filter: StackSpellFilter::default(),
                unless_controller_pays: Some(Amount::Fixed(4)),
                unless_controller_pays_by_cast_cost: None,
            }],
            modal_targeting("Choose target spell", 0),
        )
    })
}

fn match_modal_draw_two_discard_one(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Draw two cards, then discard a card.").then(|| {
        modal_mode(
            vec![SpellEffectKind::DrawDiscard {
                who: PlayerRecipient::Controller,
                draw_count: 2,
                discard_count: 1,
                order: DrawDiscardOrder::DrawThenDiscard,
                optional: false,
            }],
            None,
        )
    })
}

fn match_modal_creature_power_damage(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text
        == "Target creature you control deals damage equal to its power to target creature an opponent controls.")
        .then(|| {
            modal_mode(
                vec![SpellEffectKind::CreatureDealsDamageEqualToPower {
                    source: TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::You,
                        ..TargetFilter::default()
                    },
                    target: TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::Opponent,
                        ..TargetFilter::default()
                    },
                }],
                modal_targeting_groups(vec![
                    ("Choose target creature you control", vec![0]),
                    ("Choose target creature an opponent controls", vec![0]),
                ]),
            )
        })
}

fn match_modal_destroy_artifact_or_enchantment(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Destroy target artifact or enchantment.").then(|| {
        modal_mode(
            vec![SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![
                        PermanentTypeFilter::Artifact,
                        PermanentTypeFilter::Enchantment,
                    ],
                    ..TargetFilter::default()
                })),
            }],
            modal_targeting("Choose target artifact or enchantment", 0),
        )
    })
}

fn match_modal_source_damage_each_opponent_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text
        == format!(
            "{} deals 1 damage to each creature your opponents control.",
            context.source_name
        ))
    .then(|| {
        modal_mode(
            vec![SpellEffectKind::DamageAll {
                amount: Amount::Fixed(1),
                players: RelativePlayerSet::Opponents,
                kind: TargetFilter::default_creature(),
            }],
            None,
        )
    })
}

fn match_modal_source_damage_four_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == format!("{} deals 4 damage to target creature.", context.source_name)).then(|| {
        modal_mode(
            vec![SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(4),
                target: TargetFilter::default_creature(),
            }],
            modal_targeting("Choose target creature", 0),
        )
    })
}

fn match_modal_indestructible_artifact_or_creature(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Target artifact or creature gains indestructible until end of turn.").then(|| {
        modal_mode(
            vec![SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![
                        PermanentTypeFilter::Artifact,
                        PermanentTypeFilter::Creature,
                    ],
                    ..TargetFilter::default()
                })),
                keywords: vec![Keyword::Indestructible],
            }],
            modal_targeting("Choose target artifact or creature", 0),
        )
    })
}

fn match_modal_source_damage_tapped_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text
        == format!(
            "{} deals 2 damage to target tapped creature.",
            context.source_name
        ))
    .then(|| {
        modal_mode(
            vec![SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(2),
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    tapped: Some(true),
                    ..TargetFilter::default()
                },
            }],
            modal_targeting("Choose target tapped creature", 0),
        )
    })
}

fn match_modal_target_player_draw_two_lose_two(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Target player draws two cards and loses 2 life.").then(|| {
        modal_mode(
            vec![
                SpellEffectKind::TargetPlayerDraws {
                    count: 2,
                    target: TargetFilter {
                        kind: TargetKind::AnyPlayer,
                        ..TargetFilter::default()
                    },
                },
                SpellEffectKind::TargetPlayerLosesLife {
                    amount: 2,
                    target: TargetFilter {
                        kind: TargetKind::AnyPlayer,
                        ..TargetFilter::default()
                    },
                },
            ],
            modal_targeting_groups(vec![("Choose target player", vec![0, 1])]),
        )
    })
}

fn match_modal_creature_plus_two_lifelink(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Target creature gets +2/+2 and gains lifelink until end of turn.").then(|| {
        modal_mode(
            vec![
                SpellEffectKind::PumpTarget {
                    power: 2,
                    toughness: 2,
                    scale: None,
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    keywords: vec![Keyword::Lifelink],
                },
            ],
            modal_targeting_groups(vec![("Choose target creature", vec![0, 1])]),
        )
    })
}

fn match_modal_target_opponent_discard_two(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Target opponent discards two cards.").then(|| {
        modal_mode(
            vec![SpellEffectKind::ChooseHandCards {
                action: HandCardAction::Discard,
                count: 2,
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    ..TargetFilter::default()
                },
                chooser: Default::default(),
                card_filter: None,
                optional: false,
                visibility: Default::default(),
            }],
            modal_targeting("Choose target opponent", 0),
        )
    })
}

fn opponents_minus_one_minus_one() -> CreatureScopeFilter {
    CreatureScopeFilter {
        controller: Some(CreatureScopeController::Opponents),
        ..CreatureScopeFilter::default()
    }
}

fn match_modal_opponents_minus_one_minus_one(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Creatures your opponents control get -1/-1 until end of turn.").then(|| {
        modal_mode(
            vec![SpellEffectKind::PumpAll {
                filter: opponents_minus_one_minus_one(),
                power: -1,
                toughness: -1,
            }],
            None,
        )
    })
}

fn match_modal_target_player_discard_two(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Target player discards two cards.").then(|| {
        modal_mode(
            vec![SpellEffectKind::ChooseHandCards {
                action: HandCardAction::Discard,
                count: 2,
                target: TargetFilter {
                    kind: TargetKind::AnyPlayer,
                    ..TargetFilter::default()
                },
                chooser: Default::default(),
                card_filter: None,
                optional: false,
                visibility: Default::default(),
            }],
            modal_targeting("Choose target player", 0),
        )
    })
}

fn match_modal_target_indestructible_creature(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Target creature gains indestructible until end of turn.").then(|| {
        modal_mode(
            vec![SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                keywords: vec![Keyword::Indestructible],
            }],
            modal_targeting("Choose target creature", 0),
        )
    })
}

fn match_modal_destroy_toughness_four(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Destroy target creature with toughness 4 or greater.").then(|| {
        modal_mode(
            vec![SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    toughness: Some(tricerules_cards::PowerComparison::AtLeast(4)),
                    ..TargetFilter::default()
                })),
            }],
            modal_targeting("Choose target creature with toughness 4 or greater", 0),
        )
    })
}

fn match_modal_counter_and_keywords(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text
        == "Put a +1/+1 counter on target creature you control. It gains trample and hexproof until end of turn.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            };
            modal_mode(
                vec![
                    SpellEffectKind::PutCounters {
                        counter: CounterKind::PlusOnePlusOne,
                        count: Amount::Fixed(1),
                        subject: EffectSubject::Chosen(Box::new(target.clone())),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: EffectSubject::Chosen(Box::new(target)),
                        keywords: vec![Keyword::Trample, Keyword::Hexproof],
                    },
                ],
                modal_targeting_groups(vec![("Choose target creature you control", vec![0, 1])]),
            )
        })
}

fn match_modal_counter_and_indestructible(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text
        == "Put a +1/+1 counter on target creature you control. It gains indestructible until end of turn.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            };
            modal_mode(
                vec![
                    SpellEffectKind::PutCounters {
                        counter: CounterKind::PlusOnePlusOne,
                        count: Amount::Fixed(1),
                        subject: EffectSubject::Chosen(Box::new(target.clone())),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: EffectSubject::Chosen(Box::new(target)),
                        keywords: vec![Keyword::Indestructible],
                    },
                ],
                modal_targeting_groups(vec![("Choose target creature you control", vec![0, 1])]),
            )
        })
}

fn match_modal_creature_minus_one(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Target creature gets -1/-1 until end of turn.").then(|| {
        modal_mode(
            vec![SpellEffectKind::PumpTarget {
                power: -1,
                toughness: -1,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            }],
            modal_targeting("Choose target creature", 0),
        )
    })
}

fn match_modal_put_counter_creature(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == "Put a +1/+1 counter on target creature.").then(|| {
        modal_mode(
            vec![SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            }],
            modal_targeting("Choose target creature", 0),
        )
    })
}

fn match_modal_destroy_noncreature_artifact(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == "Destroy target noncreature artifact.").then(|| {
        modal_mode(
            vec![SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![PermanentTypeFilter::Artifact],
                    excluded_permanent_types: vec![PermanentTypeFilter::Creature],
                    ..TargetFilter::default()
                })),
            }],
            modal_targeting("Choose target noncreature artifact", 0),
        )
    })
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

fn match_equipment_etb_manifest_dread_attach(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_equipment
        && text == "When this Equipment enters, manifest dread, then attach this Equipment to that creature.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
                vec![
                    SpellEffectKind::ManifestDread,
                    SpellEffectKind::AttachEquipment {
                        equipment: EffectSubject::Source,
                        creature: EffectSubject::PreviousEffectObject,
                    },
                ],
            )
        })
}

fn match_equipment_etb_attach(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_equipment
        && text == "When this Equipment enters, attach it to target creature you control.")
        .then(|| {
            targeted_trigger(
                context,
                vec![SpellEffectKind::AttachSource {
                    target: TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::You,
                        ..TargetFilter::default()
                    },
                }],
                1,
                1,
                "Choose target creature you control",
            )
        })
}

fn match_equipment_etb_attach_first_strike(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_equipment
        && text == "When this Equipment enters, attach it to target creature you control. That creature gains first strike until end of turn.")
        .then(|| {
            targeted_trigger(
                context,
                vec![
                    SpellEffectKind::AttachSource {
                        target: TargetFilter {
                            kind: TargetKind::Creature,
                            controller: TargetController::You,
                            ..TargetFilter::default()
                        },
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: chosen_creature(TargetController::You),
                        keywords: vec![Keyword::FirstStrike],
                    },
                ],
                1,
                1,
                "Choose target creature you control",
            )
        })
}

fn etb_instruction(text: &str) -> Option<&str> {
    text.strip_prefix("When this creature enters, ")
}

fn exact_targeting(min: u32, max: u32, prompt: &str, effect_indices: Vec<u32>) -> TargetingDef {
    TargetingDef {
        groups: vec![TargetGroupDef {
            min,
            max,
            prompt: prompt.into(),
            effect_indices,
            distinct_from: Vec::new(),
            same_graveyard: false,
            cast_cost_expansion: None,
        }],
    }
}

fn targeted_trigger(
    context: &RecipeContext,
    effects: Vec<SpellEffectKind>,
    min: u32,
    max: u32,
    prompt: &str,
) -> RecipeEmission {
    let effect_indices =
        (0..u32::try_from(effects.len()).expect("effect count fits u32")).collect();
    let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
        context,
        TriggerCondition::WhenSelfEntersBattlefield,
        effects,
    ) else {
        unreachable!("triggered_ability_with always returns a triggered ability")
    };
    ability.targeting = Some(exact_targeting(min, max, prompt, effect_indices));
    RecipeEmission::TriggeredAbility(ability)
}

fn match_controller_gains_life_put_counter_on_source(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "Whenever you gain life, put a +1/+1 counter on this creature.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverPlayerGainsLife {
                    player: CastTriggerPlayer::Controller,
                },
                vec![SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }],
            )
        })
}

fn match_controller_gains_life_each_opponent_loses_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "Whenever you gain life, each opponent loses 1 life.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverPlayerGainsLife {
                    player: CastTriggerPlayer::Controller,
                },
                vec![SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(1),
                    who: PlayerRecipient::EachOpponent,
                }],
            )
        })
}

fn match_self_enters_or_dies_surveil_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    if !context.source_is_creature || text != "When this creature enters or dies, surveil 1." {
        return None;
    }
    let effect = vec![SpellEffectKind::LibraryPartition {
        count: 1,
        top_min: 0,
        top_max: None,
        kind: LibraryPartitionKind::Surveil,
    }];
    let RecipeEmission::TriggeredAbility(enters) = triggered_ability_with(
        context,
        TriggerCondition::WhenSelfEntersBattlefield,
        effect.clone(),
    ) else {
        unreachable!("triggered_ability_with always returns a triggered ability")
    };
    let suffix = context
        .triggered_ability_id
        .as_str()
        .strip_prefix("triggered_")?
        .parse::<u32>()
        .ok()?;
    let mut dies = enters.clone();
    dies.ability_id = AbilityId::new(format!("triggered_{:02}", suffix + 1)).ok()?;
    dies.trigger = TriggerCondition::WhenSelfDies;
    Some(RecipeEmission::TriggeredAbilities(vec![enters, dies]))
}

fn match_etb_pump_opponent_creature_minus_two_zero(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "When this creature enters, target creature an opponent controls gets -2/-0 until end of turn.")
        .then(|| {
            targeted_trigger(
                context,
                vec![SpellEffectKind::PumpTarget {
                    power: -2,
                    toughness: 0,
                    scale: None,
                    subject: chosen_creature(TargetController::Opponent),
                }],
                1,
                1,
                "Choose target creature an opponent controls",
            )
        })
}

fn match_etb_target_opponent_discards_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "When this creature enters, target opponent discards a card.")
        .then(|| {
            targeted_trigger(
                context,
                vec![SpellEffectKind::ChooseHandCards {
                    action: HandCardAction::Discard,
                    count: 1,
                    target: TargetFilter {
                        kind: TargetKind::OpponentPlayer,
                        ..TargetFilter::default()
                    },
                    chooser: Default::default(),
                    card_filter: None,
                    optional: false,
                    visibility: Default::default(),
                }],
                1,
                1,
                "Choose target opponent",
            )
        })
}

fn match_etb_create_clue(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && matches!(
            text,
            "When this creature enters, create a Clue token."
                | "When this creature enters, investigate."
        ))
    .then(|| {
        triggered_ability(
            context,
            SpellEffectKind::CreateTokens {
                token: "clue".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            },
        )
    })
}

fn match_aura_etb_draw_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_aura && text == "When this Aura enters, draw a card.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        )
    })
}

fn match_etb_create_printed_one_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    if !context.source_is_creature {
        return None;
    }
    let token = match text {
        "When this creature enters, create a 1/1 white Soldier creature token." => "soldier_w_1_1",
        "When this creature enters, create a 1/1 white and blue Merfolk creature token." => {
            "merfolk_wu_1_1"
        }
        _ => return None,
    };
    Some(triggered_ability(
        context,
        SpellEffectKind::CreateTokens {
            token: token.into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        },
    ))
}

fn match_etb_damage_any_target_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "When this creature enters, it deals 1 damage to any target.")
        .then(|| {
            targeted_trigger(
                context,
                vec![SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(1),
                    target: TargetFilter {
                        kind: TargetKind::AnyTarget,
                        ..TargetFilter::default()
                    },
                }],
                1,
                1,
                "Choose any target",
            )
        })
}

fn match_etb_put_counter_on_other_controlled_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "When this creature enters, put a +1/+1 counter on another target creature you control.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                excluded_objects: vec![TargetObjectExclusion::Source],
                ..TargetFilter::default()
            };
            targeted_trigger(
                context,
                vec![SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Chosen(Box::new(target)),
                }],
                1,
                1,
                "Choose another target creature you control",
            )
        })
}

fn match_etb_return_other_creature_up_to_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "When this creature enters, return up to one other target creature to its owner's hand.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::Creature,
                excluded_objects: vec![TargetObjectExclusion::Source],
                ..TargetFilter::default()
            };
            targeted_trigger(
                context,
                vec![SpellEffectKind::ReturnToOwnersHand {
                    subject: EffectSubject::Chosen(Box::new(target)),
                }],
                0,
                1,
                "Choose up to one other target creature",
            )
        })
}

fn match_etb_draw(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    let effect = draw_effect(&capitalize(etb_instruction(text)?))?;
    Some(triggered_ability(context, effect))
}

fn match_raid_etb_draw(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "Raid — When this creature enters, if you attacked this turn, draw a card.")
        .then(|| {
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability(
                context,
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                },
            ) else {
                unreachable!("triggered_ability always returns a triggered ability")
            };
            ability.intervening_if = Some(GameCondition::AttackedThisTurn {
                players: RelativePlayerSet::Controller,
            });
            RecipeEmission::TriggeredAbility(ability)
        })
}

fn match_artifact_etb_draw(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_artifact && text == "When this artifact enters, draw a card.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        )
    })
}

fn match_artifact_etb_scry_two(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_artifact && text == "When this artifact enters, scry 2.").then(|| {
        triggered_ability(
            context,
            SpellEffectKind::Scry {
                count: Amount::Fixed(2),
            },
        )
    })
}

fn match_artifact_etb_create_food(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_artifact && text == "When this artifact enters, create a Food token.").then(
        || {
            triggered_ability(
                context,
                SpellEffectKind::CreateTokens {
                    token: "food".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                },
            )
        },
    )
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

fn match_enchantment_etb_optional_linked_exile_gain_two(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_enchantment
        && text
            == "When this enchantment enters, exile up to one target nonland permanent an opponent controls until this enchantment leaves the battlefield. You gain 2 life.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::Opponent,
                excluded_permanent_types: vec![PermanentTypeFilter::Land],
                ..TargetFilter::default()
            };
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
                vec![
                    SpellEffectKind::ExileUntilSourceLeaves {
                        target: target.clone(),
                    },
                    SpellEffectKind::GainLife {
                        amount: Amount::Fixed(2),
                    },
                ],
            ) else {
                unreachable!("triggered_ability_with always returns a triggered ability")
            };
            ability.targeting = Some(TargetingDef {
                groups: vec![TargetGroupDef {
                    min: 0,
                    max: 1,
                    prompt: "Choose up to one target nonland permanent an opponent controls".into(),
                    effect_indices: vec![0],
                    distinct_from: Vec::new(),
                    same_graveyard: false,
                    cast_cost_expansion: None,
                }],
            });
            RecipeEmission::TriggeredAbility(ability)
        })
}

fn match_enchantment_etb_linked_exile(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_enchantment
        && text
            == "When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::Opponent,
                excluded_permanent_types: vec![PermanentTypeFilter::Land],
                ..TargetFilter::default()
            };
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
                vec![SpellEffectKind::ExileUntilSourceLeaves {
                    target: target.clone(),
                }],
            ) else {
                unreachable!("triggered_ability_with always returns a triggered ability")
            };
            ability.targeting = Some(TargetingDef {
                groups: vec![TargetGroupDef {
                    min: 1,
                    max: 1,
                    prompt: "Choose target nonland permanent an opponent controls".into(),
                    effect_indices: vec![0],
                    distinct_from: Vec::new(),
                    same_graveyard: false,
                    cast_cost_expansion: None,
                }],
            });
            RecipeEmission::TriggeredAbility(ability)
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

fn match_etb_return_other_controlled_permanent_up_to_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "When this creature enters, return up to one other target permanent you control to its owner's hand.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::You,
                excluded_objects: vec![TargetObjectExclusion::Source],
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
                    min: 0,
                    max: 1,
                    prompt: "Choose up to one other target permanent you control".into(),
                    effect_indices: vec![0],
                    distinct_from: Vec::new(),
                    same_graveyard: false,
                    cast_cost_expansion: None,
                }],
            });
            RecipeEmission::TriggeredAbility(ability)
        })
}

fn match_etb_mill_two_may(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "When this creature enters, you may mill two cards.")
        .then(|| {
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability(
                context,
                SpellEffectKind::Mill {
                    count: Amount::Fixed(2),
                    who: PlayerRecipient::Controller,
                },
            ) else {
                unreachable!("triggered_ability always returns a triggered ability")
            };
            ability.may = true;
            RecipeEmission::TriggeredAbility(ability)
        })
}

fn match_controller_casts_noncreature_put_counter(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "Whenever you cast a noncreature spell, put a +1/+1 counter on this creature.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverPlayerCastsSpell {
                    caster: CastTriggerPlayer::Controller,
                    filter: SpellCastFilter {
                        card_type: Some(CardTypeFilter::Noncreature),
                        ..SpellCastFilter::default()
                    },
                    ordinal: None,
                    ordinal_scope: Default::default(),
                },
                vec![SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }],
            )
        })
}

fn match_controller_draws_second_put_counter(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "Whenever you draw your second card each turn, put a +1/+1 counter on this creature.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverPlayerDrawsNthCard {
                    drawer: CastTriggerPlayer::Controller,
                    ordinal: 2,
                },
                vec![SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }],
            )
        })
}

fn match_self_attacks_gain_two(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "Whenever this creature attacks, you gain 2 life.").then(
        || {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0,
                },
                vec![SpellEffectKind::GainLife {
                    amount: Amount::Fixed(2),
                }],
            )
        },
    )
}

fn match_self_attacks_mill_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "Whenever this creature attacks, mill a card.").then(
        || {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0,
                },
                vec![SpellEffectKind::Mill {
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                }],
            )
        },
    )
}

fn match_landfall_mill_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "Landfall — Whenever a land you control enters, mill a card.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverPermanentEntersBattlefield {
                    controller: CastTriggerPlayer::Controller,
                    filter: PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Land),
                        ..PermanentEventFilter::default()
                    },
                    creature_filter: None,
                },
                vec![SpellEffectKind::Mill {
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                }],
            )
        })
}

fn match_landfall_damage_each_opponent_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "Landfall — Whenever a land you control enters, this creature deals 1 damage to each opponent.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverPermanentEntersBattlefield {
                    controller: CastTriggerPlayer::Controller,
                    filter: PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Land),
                        ..PermanentEventFilter::default()
                    },
                    creature_filter: None,
                },
                vec![SpellEffectKind::DamagePlayer {
                    amount: Amount::Fixed(1),
                    who: PlayerRecipient::EachOpponent,
                }],
            )
        })
}

fn match_self_attacks_optional_discard_then_draw(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "Whenever this creature attacks, you may discard a card. If you do, draw a card.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0,
                },
                vec![SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::Controller,
                    draw_count: 1,
                    discard_count: 1,
                    order: DrawDiscardOrder::DiscardThenDraw,
                    optional: true,
                }],
            )
        })
}

fn match_self_becomes_tapped_draw_then_discard(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "Whenever this creature becomes tapped, draw a card, then discard a card.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverSelfBecomesTapped,
                vec![SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::Controller,
                    draw_count: 1,
                    discard_count: 1,
                    order: DrawDiscardOrder::DrawThenDiscard,
                    optional: false,
                }],
            )
        })
}

fn match_self_becomes_tapped_optional_discard_then_draw(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverSelfBecomesTapped,
                vec![SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::Controller,
                    draw_count: 1,
                    discard_count: 1,
                    order: DrawDiscardOrder::DiscardThenDraw,
                    optional: true,
                }],
            )
        })
}

fn match_self_enters_optional_discard_then_draw(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "When this creature enters, you may discard a card. If you do, draw a card.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
                vec![SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::Controller,
                    draw_count: 1,
                    discard_count: 1,
                    order: DrawDiscardOrder::DiscardThenDraw,
                    optional: true,
                }],
            )
        })
}

fn match_controller_end_step_draw_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    ((context.source_is_artifact || context.source_is_enchantment)
        && text == "At the beginning of your end step, draw a card.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::AtBeginningOfEndStep {
                    player: CastTriggerPlayer::Controller,
                },
                vec![SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                }],
            )
        })
}

fn creature_graveyard_card_to_hand_effect() -> SpellEffectKind {
    SpellEffectKind::MoveGraveyardCards {
        filter: GraveyardFilter {
            card: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..ZoneCardFilter::default()
            }),
            ..GraveyardFilter::default()
        },
        destination: GraveyardDestination::Hand,
        linked_exile_id: None,
    }
}

fn match_etb_return_creature_card_to_hand(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "When this creature enters, return target creature card from your graveyard to your hand.")
        .then(|| triggered_ability(context, creature_graveyard_card_to_hand_effect()))
}

fn instant_or_sorcery_graveyard_card_to_hand_effect() -> SpellEffectKind {
    SpellEffectKind::MoveGraveyardCards {
        filter: GraveyardFilter {
            card: Some(ZoneCardFilter {
                card_type: Some(CardTypeFilter::InstantOrSorcery),
                ..ZoneCardFilter::default()
            }),
            ..GraveyardFilter::default()
        },
        destination: GraveyardDestination::Hand,
        linked_exile_id: None,
    }
}

fn match_etb_return_instant_or_sorcery_card_to_hand(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "When this creature enters, return target instant or sorcery card from your graveyard to your hand.")
        .then(|| {
            triggered_ability(context, instant_or_sorcery_graveyard_card_to_hand_effect())
        })
}

fn match_controller_creature_enters_damage_each_opponent(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_enchantment
        && text
            == "Whenever a creature you control enters, this enchantment deals 1 damage to each opponent.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverPermanentEntersBattlefield {
                    controller: CastTriggerPlayer::Controller,
                    filter: PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Creature),
                        ..PermanentEventFilter::default()
                    },
                    creature_filter: None,
                },
                vec![SpellEffectKind::DamagePlayer {
                    amount: Amount::Fixed(1),
                    who: PlayerRecipient::EachOpponent,
                }],
            )
        })
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

fn match_increment(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)")
        .then(|| {
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
                context,
                TriggerCondition::WheneverPlayerCastsSpell {
                    caster: CastTriggerPlayer::Controller,
                    filter: SpellCastFilter::default(),
                    ordinal: None,
                    ordinal_scope: Default::default(),
                },
                vec![SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }],
            ) else {
                unreachable!("triggered_ability_with always returns a triggered ability")
            };
            ability.intervening_if = Some(GameCondition::TriggeringSpellManaSpent {
                comparison: SpellManaSpentComparison::GreaterThanSourcePowerOrToughness,
            });
            RecipeEmission::TriggeredAbility(ability)
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

fn controller_turn_condition() -> GameCondition {
    GameCondition::ActivePlayer {
        players: RelativePlayerSet::Controller,
    }
}

fn match_controller_turn_self_first_strike(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "During your turn, this creature has first strike.")
        .then(|| {
            RecipeEmission::StaticAbility(IdentifiedAbility {
                ability_id: context.static_ability_id.clone(),
                presentation: context.presentation.clone(),
                definition: StaticAbilityDef::ConditionalSelfModifier {
                    condition: controller_turn_condition(),
                    set_types: None,
                    add_types: TypeLineAddition::default(),
                    base_power: None,
                    base_toughness: None,
                    delta_power: 0,
                    delta_toughness: 0,
                    keywords: vec![Keyword::FirstStrike],
                    activated_abilities: Vec::new(),
                    triggered_abilities: Vec::new(),
                    can_attack_as_though_without_defender: false,
                },
            })
        })
}

fn match_controller_turn_countered_creatures_first_strike(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "During your turn, creatures you control with +1/+1 counters on them have first strike.")
        .then(|| {
            RecipeEmission::StaticAbility(IdentifiedAbility {
                ability_id: context.static_ability_id.clone(),
                presentation: context.presentation.clone(),
                definition: StaticAbilityDef::AnthemKeyword {
                    filter: CreatureScopeFilter {
                        controller: Some(CreatureScopeController::YouControl),
                        required_counter: Some(CounterKind::PlusOnePlusOne),
                        ..CreatureScopeFilter::default()
                    },
                    condition: Some(controller_turn_condition()),
                    keyword: Keyword::FirstStrike,
                },
            })
        })
}

fn match_self_cannot_block(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "This creature can't block.").then(|| {
        RecipeEmission::StaticAbility(IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: context.presentation.clone(),
            definition: StaticAbilityDef::SelfCombatRestriction {
                restriction: tricerules_cards::primitives::CombatRestriction {
                    cant_block: true,
                    ..tricerules_cards::primitives::CombatRestriction::default()
                },
                condition: None,
            },
        })
    })
}

fn match_controller_creature_anthem_plus_one_plus_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    ((context.source_is_artifact || context.source_is_enchantment)
        && text == "Creatures you control get +1/+1.")
        .then(|| {
            RecipeEmission::StaticAbility(IdentifiedAbility {
                ability_id: context.static_ability_id.clone(),
                presentation: context.presentation.clone(),
                definition: StaticAbilityDef::AnthemPt {
                    filter: creatures_you_control(),
                    condition: None,
                    delta_power: 1,
                    delta_toughness: 1,
                },
            })
        })
}

fn match_extra_land_play(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    ((context.source_is_creature || context.source_is_enchantment)
        && text == "You may play an additional land on each of your turns.")
        .then(|| {
            RecipeEmission::StaticAbility(IdentifiedAbility {
                ability_id: context.static_ability_id.clone(),
                presentation: context.presentation.clone(),
                definition: StaticAbilityDef::ExtraLandPlays { count: 1 },
            })
        })
}

fn match_play_lands_from_own_graveyard(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    ((context.source_is_creature || context.source_is_enchantment)
        && text == "You may play lands from your graveyard.")
        .then(|| {
            RecipeEmission::StaticAbility(IdentifiedAbility {
                ability_id: context.static_ability_id.clone(),
                presentation: context.presentation.clone(),
                definition: StaticAbilityDef::PlayLandsFromOwnGraveyard,
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
    (text == "{1}, {T}: Add one mana of any color.").then(|| {
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

fn utility_activated_ability(
    context: &RecipeContext,
    costs: Vec<AbilityCost>,
    effect: Vec<SpellEffectKind>,
    targeting: Option<TargetingDef>,
) -> RecipeEmission {
    RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Battlefield,
        costs,
        effect,
        targeting,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    })
}

fn fixed_mana_cost(cost: &str) -> AbilityCost {
    AbilityCost::Mana(ManaCost::parse(cost).expect("closed utility recipe uses valid mana cost"))
}

fn five_color_mana_effect() -> SpellEffectKind {
    SpellEffectKind::ProduceMana {
        options: ['W', 'U', 'B', 'R', 'G']
            .into_iter()
            .map(|symbol| parse_mana_amount(symbol).expect("five-color recipe uses valid symbols"))
            .collect(),
        restriction: None,
        conditional: None,
    }
}

fn single_targeting(prompt: &str) -> Option<TargetingDef> {
    Some(TargetingDef {
        groups: vec![TargetGroupDef {
            min: 1,
            max: 1,
            prompt: prompt.into(),
            effect_indices: vec![0],
            distinct_from: Vec::new(),
            same_graveyard: false,
            cast_cost_expansion: None,
        }],
    })
}

fn match_artifact_pay_one_tap_sacrifice_any_color(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact
        && text == "{1}, {T}, Sacrifice this artifact: Add one mana of any color.")
        .then(|| {
            utility_activated_ability(
                context,
                vec![
                    fixed_mana_cost("{1}"),
                    AbilityCost::Tap,
                    AbilityCost::SacrificeSelf,
                ],
                vec![five_color_mana_effect()],
                None,
            )
        })
}

fn match_artifact_pay_two_tap_sacrifice_gain_three(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact && text == "{2}, {T}, Sacrifice this artifact: You gain 3 life.")
        .then(|| {
            utility_activated_ability(
                context,
                vec![
                    fixed_mana_cost("{2}"),
                    AbilityCost::Tap,
                    AbilityCost::SacrificeSelf,
                ],
                vec![SpellEffectKind::GainLife {
                    amount: Amount::Fixed(3),
                }],
                None,
            )
        })
}

fn match_artifact_pay_two_tap_sacrifice_gain_three_draw_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact
        && text == "{2}, {T}, Sacrifice this artifact: You gain 3 life and draw a card.")
        .then(|| {
            utility_activated_ability(
                context,
                vec![
                    fixed_mana_cost("{2}"),
                    AbilityCost::Tap,
                    AbilityCost::SacrificeSelf,
                ],
                vec![
                    SpellEffectKind::GainLife {
                        amount: Amount::Fixed(3),
                    },
                    SpellEffectKind::Draw {
                        who: PlayerRecipient::Controller,
                        count: Amount::Fixed(1),
                    },
                ],
                None,
            )
        })
}

fn match_artifact_pay_three_u_sacrifice_draw_two(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact && text == "{3}{U}, Sacrifice this artifact: Draw two cards.").then(
        || {
            utility_activated_ability(
                context,
                vec![fixed_mana_cost("{3}{U}"), AbilityCost::SacrificeSelf],
                vec![SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(2),
                }],
                None,
            )
        },
    )
}

fn match_creature_pay_two_sacrifice_draw_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "{2}, Sacrifice this creature: Draw a card.").then(
        || {
            utility_activated_ability(
                context,
                vec![fixed_mana_cost("{2}"), AbilityCost::SacrificeSelf],
                vec![SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                }],
                None,
            )
        },
    )
}

fn match_artifact_pay_three_tap_sacrifice_damage_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact
        && text == "{3}, {T}, Sacrifice this artifact: It deals 3 damage to target creature.")
        .then(|| {
            utility_activated_ability(
                context,
                vec![
                    fixed_mana_cost("{3}"),
                    AbilityCost::Tap,
                    AbilityCost::SacrificeSelf,
                ],
                vec![SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(3),
                    target: TargetFilter::default_creature(),
                }],
                single_targeting("Choose target creature"),
            )
        })
}

fn match_artifact_pay_seven_tap_sacrifice_destroy_permanent(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact
        && text == "{7}, {T}, Sacrifice this artifact: Destroy target permanent.")
        .then(|| {
            utility_activated_ability(
                context,
                vec![
                    fixed_mana_cost("{7}"),
                    AbilityCost::Tap,
                    AbilityCost::SacrificeSelf,
                ],
                vec![SpellEffectKind::Destroy {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        ..TargetFilter::default()
                    })),
                }],
                single_targeting("Choose target permanent"),
            )
        })
}

fn match_pay_one_for_any_color_once_per_turn(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text == "{1}: Add one mana of any color. Activate only once each turn.")
        .then(|| {
            RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
                ability_id: context.activated_ability_id.clone(),
                presentation: context.presentation.clone(),
                cost_modifiers: Vec::new(),
                source_zone: AbilitySourceZone::Battlefield,
                costs: vec![AbilityCost::Mana(
                    ManaCost::parse("{1}").expect("static recipe mana cost"),
                )],
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
                activation_limit: Some(ActivationLimit::PerTurn { max_activations: 1 }),
            })
        })
}

fn match_creature_self_pump_one_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "{1}{B}: This creature gets +1/+1 until end of turn.")
        .then(|| {
            RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
                ability_id: context.activated_ability_id.clone(),
                presentation: context.presentation.clone(),
                cost_modifiers: Vec::new(),
                source_zone: AbilitySourceZone::Battlefield,
                costs: vec![AbilityCost::Mana(
                    ManaCost::parse("{1}{B}").expect("static recipe mana cost"),
                )],
                effect: vec![SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 1,
                    scale: None,
                    subject: EffectSubject::Source,
                }],
                targeting: None,
                timing: ActivationTiming::Normal,
                conditions: Vec::new(),
                activation_limit: None,
            })
        })
}

fn match_creature_self_pump_two_two_once_per_turn(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "{2}{G}: This creature gets +2/+2 until end of turn. Activate only once each turn.")
        .then(|| {
            RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
                ability_id: context.activated_ability_id.clone(),
                presentation: context.presentation.clone(),
                cost_modifiers: Vec::new(),
                source_zone: AbilitySourceZone::Battlefield,
                costs: vec![AbilityCost::Mana(
                    ManaCost::parse("{2}{G}").expect("static recipe mana cost"),
                )],
                effect: vec![SpellEffectKind::PumpTarget {
                    power: 2,
                    toughness: 2,
                    scale: None,
                    subject: EffectSubject::Source,
                }],
                targeting: None,
                timing: ActivationTiming::Normal,
                conditions: Vec::new(),
                activation_limit: Some(ActivationLimit::PerTurn { max_activations: 1 }),
            })
        })
}

fn match_creature_team_pump_one_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    if !context.source_is_creature {
        return None;
    }
    let cost = match text {
        "{3}{W}: Creatures you control get +1/+1 until end of turn." => "{3}{W}",
        "{4}{W}: Creatures you control get +1/+1 until end of turn." => "{4}{W}",
        "{5}: Creatures you control get +1/+1 until end of turn." => "{5}",
        _ => return None,
    };
    Some(RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Mana(
            ManaCost::parse(cost).expect("closed team-pump recipe uses valid mana costs"),
        )],
        effect: vec![SpellEffectKind::PumpAll {
            filter: creatures_you_control(),
            power: 1,
            toughness: 1,
        }],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    }))
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

fn match_land_tap_sacrifice_search_basic_tapped(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_land
        && text
            == "{T}, Sacrifice this land: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.")
        .then(|| {
            RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
                ability_id: context.activated_ability_id.clone(),
                presentation: context.presentation.clone(),
                cost_modifiers: Vec::new(),
                source_zone: AbilitySourceZone::Battlefield,
                costs: vec![AbilityCost::Tap, AbilityCost::SacrificeSelf],
                effect: vec![SpellEffectKind::SearchLibrary {
                    who: PlayerRecipient::Controller,
                    optional: false,
                    count: 1,
                    count_by_cast_cost: None,
                    filter: Some(ZoneCardFilter {
                        card_type: Some(CardTypeFilter::BasicLand),
                        ..ZoneCardFilter::default()
                    }),
                    slots: Vec::new(),
                    zones: SearchZoneSelection::default(),
                    destination: SearchDestination::Battlefield { tapped: true },
                    conditional_destination: None,
                    shuffle: true,
                    reveal: false,
                    result_id: None,
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

fn match_unconditional_creature_enters_tapped(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == "This creature enters tapped.").then(|| {
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
    ($($positive_name:literal => $positive_clause:literal),+; $($negative:literal),+ $(,)?) => {
        RecipeCalibration {
            positive_cards: &[
                $(CalibrationCard { name: $positive_name, clause: $positive_clause }),+
            ],
            negative_near_misses: &[$($negative),+],
            minimum_positive_cards: 2,
        }
    };
}

macro_rules! singleton_calibrations {
    ($name:literal => $clause:literal; $($negative:literal),+ $(,)?) => {
        RecipeCalibration {
            positive_cards: &[CalibrationCard { name: $name, clause: $clause }],
            negative_near_misses: &[$($negative),+],
            minimum_positive_cards: 1,
        }
    };
}

pub(super) static CATALOG: &[Recipe] = &[
    Recipe {
        id: RecipeId("static.cost_reduction.affinity_artifacts"),
        label: "affinity for artifacts",
        surface: RecipeSurface::StaticAbility,
        matcher: match_affinity_for_artifacts,
        calibration: calibrations!(
            "Valkyrie Aerial Unit" => "Affinity for artifacts",
            "Memory Guardian" => "Affinity for artifacts";
            "Affinity for creatures",
            "Affinity for artifact",
            "Affinity for artifacts.",
            "Affinity for artifacts if you control an artifact",
            "Affinity for artifacts. Draw a card.",
            "Affinity for artifacts and flying"
        ),
    },
    Recipe {
        id: RecipeId("spell.cost_reduction.target_tapped_creature.three"),
        label: "costs three less when targeting a tapped creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_tapped_creature_target_reduction_three,
        calibration: calibrations!(
            "Quicksand Whirlpool" => "This spell costs {3} less to cast if it targets a tapped creature.",
            "Grounded for Life" => "This spell costs {3} less to cast if it targets a tapped creature.";
            "This spell costs {3} less to cast if it targets an untapped creature.",
            "This spell costs {2} less to cast if it targets a tapped creature.",
            "This spell costs {3} less to cast if it targets a tapped permanent.",
            "This spell costs {3} less to cast if it targets a tapped creature. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId(
            "etb.enchantment.exile_opponent_nonland_up_to_one_until_source_leaves.gain_life_two",
        ),
        label: "optional linked exile of opposing nonland then gain two life",
        surface: RecipeSurface::EtbAbility,
        matcher: match_enchantment_etb_optional_linked_exile_gain_two,
        calibration: calibrations!(
            "Liminal Hold" => "When this enchantment enters, exile up to one target nonland permanent an opponent controls until this enchantment leaves the battlefield. You gain 2 life.",
            "Prayer of Binding" => "When this enchantment enters, exile up to one target nonland permanent an opponent controls until this enchantment leaves the battlefield. You gain 2 life.";
            "When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield. You gain 2 life.",
            "When this enchantment enters, exile up to one target permanent an opponent controls until this enchantment leaves the battlefield. You gain 2 life.",
            "When this enchantment enters, exile up to one target nonland permanent until this enchantment leaves the battlefield. You gain 2 life.",
            "When this enchantment enters, exile up to one target nonland permanent an opponent controls until this enchantment leaves the battlefield. You gain 3 life.",
            "When this enchantment enters, you gain 2 life. Exile up to one target nonland permanent an opponent controls until this enchantment leaves the battlefield."
        ),
    },
    Recipe {
        id: RecipeId("etb.enchantment.exile_opponent_nonland_until_source_leaves"),
        label: "mandatory linked exile of opposing nonland",
        surface: RecipeSurface::EtbAbility,
        matcher: match_enchantment_etb_linked_exile,
        calibration: calibrations!(
            "Banishing Light" => "When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield.",
            "Stormplain Detainment" => "When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield.";
            "When this enchantment enters, exile up to one target nonland permanent an opponent controls until this enchantment leaves the battlefield.",
            "When this artifact enters, exile target nonland permanent an opponent controls until this artifact leaves the battlefield.",
            "When this enchantment enters, exile target artifact or creature an opponent controls until this enchantment leaves the battlefield.",
            "When this enchantment enters, exile target creature an opponent controls until this enchantment leaves the battlefield.",
            "When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield. You gain 2 life.",
            "When this enchantment enters, you gain 2 life. Exile target nonland permanent an opponent controls until this enchantment leaves the battlefield.",
            "When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield.\nWhen this enchantment enters, create a 1/1 red Mercenary creature token."
        ),
    },
    Recipe {
        id: RecipeId("spell.surveil_two.draw_two.lose_two"),
        label: "Surveil two then draw two and lose two life",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_surveil_two_draw_two_lose_two,
        calibration: calibrations!(
            "Risky Research" => "Surveil 2, then draw two cards. You lose 2 life.",
            "Diresight" => "Surveil 2, then draw two cards. You lose 2 life.";
            "Scry 2, then draw two cards. You lose 2 life.",
            "Surveil 1, then draw two cards. You lose 2 life.",
            "Surveil 2, then draw a card. You lose 2 life.",
            "Draw two cards, then surveil 2. You lose 2 life.",
            "Surveil 2, then draw two cards.",
            "Surveil 2, then draw two cards. You lose 1 life."
        ),
    },
    Recipe {
        id: RecipeId("spell.return_nonland_permanent_then_surveil_one"),
        label: "return target nonland permanent then Surveil one",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_return_nonland_then_surveil_one,
        calibration: calibrations!(
            "Unauthorized Exit" => "Return target nonland permanent to its owner's hand. Surveil 1.",
            "Banishing Betrayal" => "Return target nonland permanent to its owner's hand. Surveil 1.";
            "Return target creature to its owner's hand. Surveil 1.",
            "Return target permanent to its owner's hand. Surveil 1.",
            "Surveil 1. Return target nonland permanent to its owner's hand.",
            "Return target nonland permanent to its owner's hand. Surveil 2.",
            "Return up to one target nonland permanent to its owner's hand. Surveil 1."
        ),
    },
    Recipe {
        id: RecipeId("spell.damage.creature.equal_power.controlled_to_opponent"),
        label: "controlled creature power damage to opposing creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_controlled_creature_power_damage,
        calibration: calibrations!(
            "Quarrel" => "Target creature you control deals damage equal to its power to target creature an opponent controls.",
            "Rocky Rebuke" => "Target creature you control deals damage equal to its power to target creature an opponent controls.";
            "Target creature fights target creature an opponent controls.",
            "Target creature you control deals damage equal to its toughness to target creature an opponent controls.",
            "Target creature deals damage equal to its power to target creature an opponent controls.",
            "Target creature you control deals damage equal to its power to target creature.",
            "Target creature you control deals damage equal to its power to up to one target creature an opponent controls."
        ),
    },
    Recipe {
        id: RecipeId("spell.damage.creature.equal_power.controlled_to_noncontroller_permanent"),
        label: "controlled creature power damage to noncontroller creature or planeswalker",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_controlled_creature_power_damage_to_noncontroller_permanent,
        calibration: calibrations!(
            "Hard-Hitting Question" => "Target creature you control deals damage equal to its power to target creature or planeswalker you don't control.",
            "Bite Down" => "Target creature you control deals damage equal to its power to target creature or planeswalker you don't control.";
            "Target creature you control gets +1/+0 until end of turn. Then it deals damage equal to its power to target creature you don't control.",
            "Target creature you control deals damage equal to its power to target planeswalker you don't control.",
            "Target creature you control deals damage equal to its power to each creature or planeswalker you don't control."
        ),
    },
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
            "Destroy target creature or planeswalker. You gain 2 life."
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
        id: RecipeId("spell.pump_grant.creature.plus_3_plus_3.trample"),
        label: "creature +3/+3 and trample",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_pump_three_grant_trample,
        calibration: calibrations!(
            "Blitzball Shot" => "Target creature gets +3/+3 and gains trample until end of turn.",
            "Fanatical Strength" => "Target creature gets +3/+3 and gains trample until end of turn.";
            "Target creature gets +2/+2 and gains trample until end of turn.",
            "Target creature you control gets +3/+3 and gains trample until end of turn.",
            "Up to one target creature gets +3/+3 and gains trample until end of turn.",
            "Target creature gets +3/+3 and gains trample until end of turn. Untap it."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump_grant.creature_you_control.plus_0_plus_3.hexproof"),
        label: "controlled creature +0/+3 and hexproof",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_controlled_creature_plus_zero_three_hexproof,
        calibration: calibrations!(
            "Chase Inspiration" => "Target creature you control gets +0/+3 and gains hexproof until end of turn.",
            "Dive Down" => "Target creature you control gets +0/+3 and gains hexproof until end of turn.";
            "Target creature gets +0/+3 and gains hexproof until end of turn.",
            "Target creature you control gets +1/+3 and gains hexproof until end of turn.",
            "Target creature you control gets +0/+3 and gains ward {2} until end of turn.",
            "Target creature you control gets +0/+3 and gains hexproof until end of turn. Untap it."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump_grant_untap.creature_you_control.plus_1_plus_1.hexproof"),
        label: "controlled creature +1/+1 hexproof and untap",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_controlled_creature_plus_one_one_hexproof_untap,
        calibration: calibrations!(
            "Magic Damper" => "Target creature you control gets +1/+1 and gains hexproof until end of turn. Untap it.",
            "Shore Up" => "Target creature you control gets +1/+1 and gains hexproof until end of turn. Untap it.";
            "Target creature gets +1/+1 and gains hexproof until end of turn. Untap it.",
            "Target creature you control gets +1/+2 and gains hexproof until end of turn. Untap it.",
            "Target creature you control gets +1/+1 and gains hexproof until end of turn.",
            "Untap target creature you control. It gets +1/+1 and gains hexproof until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump_grant_untap.creature.plus_1_plus_3.reach"),
        label: "creature +1/+3 reach and untap",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_creature_plus_one_three_reach_untap,
        calibration: calibrations!(
            "High Stride" => "Target creature gets +1/+3 and gains reach until end of turn. Untap it.",
            "Leaping Ambush" => "Target creature gets +1/+3 and gains reach until end of turn. Untap it.";
            "Target creature you control gets +1/+3 and gains reach until end of turn. Untap it.",
            "Target creature gets +1/+2 and gains reach until end of turn. Untap it.",
            "Target creature gets +1/+3 and gains flying until end of turn. Untap it.",
            "Target creature gets +1/+3 and gains reach until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump.creature.minus_3_minus_3"),
        label: "creature -3/-3",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_creature_minus_three_three,
        calibration: calibrations!(
            "Last Gasp" => "Target creature gets -3/-3 until end of turn.",
            "Scorpion's Sting" => "Target creature gets -3/-3 until end of turn.";
            "Target creature gets -2/-2 until end of turn. You gain 2 life.",
            "Target creature you control gets -3/-3 until end of turn.",
            "Up to one target creature gets -3/-3 until end of turn.",
            "Target creature gets -3/-3 until end of turn. You gain 3 life."
        ),
    },
    Recipe {
        id: RecipeId("spell.grant.creature.deathtouch_indestructible"),
        label: "creature deathtouch and indestructible",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_creature_deathtouch_indestructible,
        calibration: calibrations!(
            "Alesha's Legacy" => "Target creature you control gains deathtouch and indestructible until end of turn.",
            "Offer Immortality" => "Target creature gains deathtouch and indestructible until end of turn.";
            "Target creature gains indestructible and deathtouch until end of turn.",
            "Target creature gains deathtouch and hexproof until end of turn.",
            "Target creature gains deathtouch and indestructible until end of turn. Untap it."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump_then_draw.creature.plus_3_plus_0"),
        label: "creature +3/+0 then draw",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_creature_plus_three_zero_then_draw,
        calibration: calibrations!(
            "Rebellious Strike" => "Target creature gets +3/+0 until end of turn.\nDraw a card.",
            "Sugar Rush" => "Target creature gets +3/+0 until end of turn.\nDraw a card.";
            "Target creature gets +3/+0 until end of turn. Draw a card.",
            "Target creature you control gets +3/+0 until end of turn.\nDraw a card.",
            "Target creature gets +2/+0 until end of turn.\nDraw a card.",
            "Draw a card.\nTarget creature gets +3/+0 until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("spell.draw_discard.draw_three.discard_one"),
        label: "draw three then discard one",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_draw_three_then_discard_one,
        calibration: calibrations!(
            "Enhanced Awareness" => "Draw three cards, then discard a card.",
            "Sift" => "Draw three cards, then discard a card.";
            "Draw three cards, then discard two cards.",
            "Draw three cards, then you may discard a card.",
            "Draw two cards, then discard a card.",
            "Discard a card, then draw three cards."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump_grant.creature.plus_4_plus_4.trample"),
        label: "creature +4/+4 and trample",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_creature_plus_four_four_trample,
        calibration: calibrations!(
            "Bestow Greatness" => "Target creature gets +4/+4 and gains trample until end of turn.",
            "Larger Than Life" => "Target creature gets +4/+4 and gains trample until end of turn.";
            "Target creature gets +3/+3 and gains trample until end of turn. Draw a card.",
            "Target creature gets +4/+4 and gains trample until end of turn. Untap it.",
            "Target creature you control gets +4/+4 and gains trample until end of turn.",
            "Up to one target creature gets +4/+4 and gains trample until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("spell.creature_power_damage.controlled_plus_one_to_opponent"),
        label: "controlled creature +1/+0 then power damage to opposing creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_controlled_creature_plus_one_then_power_damage,
        calibration: calibrations!(
            "Assert Perfection" => "Target creature you control gets +1/+0 until end of turn. It deals damage equal to its power to up to one target creature an opponent controls.",
            "Huatli's Final Strike" => "Target creature you control gets +1/+0 until end of turn. It deals damage equal to its power to target creature an opponent controls.";
            "Target creature gets +1/+0 until end of turn. It deals damage equal to its power to up to one target creature an opponent controls.",
            "Target creature you control gets +2/+0 until end of turn. It deals damage equal to its power to target creature an opponent controls.",
            "Target creature you control gets +1/+0 until end of turn. It deals damage equal to its power to up to one target creature or planeswalker an opponent controls.",
            "Target creature you control gets +1/+0 until end of turn if you've cast another instant or sorcery spell this turn. Then it deals damage equal to its power to up to one target creature an opponent controls."
        ),
    },
    Recipe {
        id: RecipeId("spell.return_graveyard_card.hand"),
        label: "return target graveyard card to hand",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_return_graveyard_card_to_hand,
        calibration: calibrations!(
            "Auroral Procession" => "Return target card from your graveyard to your hand.",
            "Recollect" => "Return target card from your graveyard to your hand.";
            "Return target creature card from your graveyard to your hand.",
            "Return target card from an opponent's graveyard to your hand.",
            "Return up to one target card from your graveyard to your hand.",
            "Return target card from your graveyard to the battlefield."
        ),
    },
    Recipe {
        id: RecipeId("spell.damage_player.each_opponent.fixed_three.source"),
        label: "source deals three damage to each opponent",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_source_damage_each_opponent_three,
        calibration: calibrations!(
            "Boltwave" => "Boltwave deals 3 damage to each opponent.",
            "Sizzle" => "Sizzle deals 3 damage to each opponent.";
            "Boltwave deals 2 damage to each opponent.",
            "It deals 3 damage to each opponent.",
            "Boltwave deals 3 damage to each player.",
            "Boltwave deals 3 damage to each opponent and you gain 3 life."
        ),
    },
    Recipe {
        id: RecipeId("spell.damage.creature.fixed_four.source"),
        label: "source deals four damage to target creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_source_damage_creature_four,
        calibration: calibrations!(
            "Bombard" => "Bombard deals 4 damage to target creature.",
            "Bathe in Dragonfire" => "Bathe in Dragonfire deals 4 damage to target creature.";
            "Bombard deals 3 damage to target creature.",
            "Bombard deals 4 damage to any target.",
            "It deals 4 damage to target creature.",
            "Bombard deals 4 damage to target creature and 1 damage to its controller."
        ),
    },
    Recipe {
        id: RecipeId("spell.destroy.artifact_or_enchantment"),
        label: "destroy target artifact or enchantment",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_destroy_artifact_or_enchantment,
        calibration: calibrations!(
            "Disenchant" => "Destroy target artifact or enchantment.",
            "Nature's Chant" => "Destroy target artifact or enchantment.";
            "Destroy up to one target artifact or enchantment.",
            "Destroy target noncreature artifact or enchantment.",
            "Destroy target artifact or enchantment. You gain 2 life.",
            "Destroy target artifact."
        ),
    },
    Recipe {
        id: RecipeId("spell.destroy.creature_or_planeswalker"),
        label: "destroy target creature or planeswalker",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_destroy_creature_or_planeswalker,
        calibration: calibrations!(
            "Hero's Downfall" => "Destroy target creature or planeswalker.",
            "Finishing Blow" => "Destroy target creature or planeswalker.";
            "Destroy up to one target creature or planeswalker.",
            "Destroy target creature. You gain 1 life.",
            "Destroy target creature or planeswalker. Its controller investigates.",
            "Exile target creature or planeswalker."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump_all.creatures_you_control.plus_3_plus_3.trample"),
        label: "creatures you control +3/+3 and trample",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_team_plus_three_trample,
        calibration: calibrations!(
            "Overrun" => "Creatures you control get +3/+3 and gain trample until end of turn.",
            "Kamahl, Heart of Krosa" => "Creatures you control get +3/+3 and gain trample until end of turn.";
            "Creatures you control get +2/+2 and gain trample until end of turn.",
            "Other creatures you control get +3/+3 and gain trample until end of turn.",
            "Creatures you control get +3/+3 and gain trample until end of turn. Untap those creatures.",
            "Creatures you control get +X/+X and gain trample until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("spell.destroy.creature_then_gain_life.two"),
        label: "destroy target creature then gain two life",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_destroy_creature_then_gain_two,
        calibration: calibrations!(
            "Sephiroth's Intervention" => "Destroy target creature. You gain 2 life.",
            "Venom's Hunger" => "Destroy target creature. You gain 2 life.";
            "Destroy target creature. You gain 3 life.",
            "Destroy target creature, then you gain 2 life.",
            "You gain 2 life, then destroy target creature.",
            "Destroy target creature. You may gain 2 life."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump.creature.minus_2_minus_2"),
        label: "creature -2/-2",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_creature_minus_two_two,
        calibration: calibrations!(
            "Disfigure" => "Target creature gets -2/-2 until end of turn.",
            "Stab" => "Target creature gets -2/-2 until end of turn.";
            "Target creature gets -4/-4 until end of turn.",
            "Target creature you control gets -2/-2 until end of turn.",
            "Up to one target creature gets -2/-2 until end of turn.",
            "Target creature gets -2/-2 until end of turn. You gain 2 life."
        ),
    },
    Recipe {
        id: RecipeId("spell.destroy.attacking_or_blocking_creature"),
        label: "destroy target attacking or blocking creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_destroy_attacking_or_blocking_creature,
        calibration: calibrations!(
            "Sudden Strike" => "Destroy target attacking or blocking creature.",
            "Divine Verdict" => "Destroy target attacking or blocking creature.";
            "Destroy target attacking creature.",
            "Destroy target attacking or blocking creature with power 3 or less.",
            "Destroy up to one target attacking or blocking creature.",
            "Exile target attacking or blocking creature."
        ),
    },
    Recipe {
        id: RecipeId("spell.exile.creature"),
        label: "exile target creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_exile_creature,
        calibration: calibrations!(
            "Wander Off" => "Exile target creature.",
            "Final Death" => "Exile target creature.";
            "Exile target creature you control.",
            "Exile up to one target creature.",
            "Exile target creature. Its controller investigates.",
            "Destroy target creature. You gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("aura.enchant.creature"),
        label: "enchant creature",
        surface: RecipeSurface::AuraSpellClause,
        matcher: match_enchant_creature,
        calibration: calibrations!(
            "Flight" => "Enchant creature",
            "Holy Strength" => "Enchant creature";
            "Enchant permanent",
            "Enchant creature you control",
            "Enchant tapped creature",
            "Enchant creature or Vehicle",
            "Enchant creature. When this Aura enters, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("activated.equip.generic_fixed"),
        label: "fixed generic Equip",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_fixed_generic_equip,
        calibration: calibrations!(
            "Bonesplitter" => "Equip {1}",
            "Vulshok Morningstar" => "Equip {2}";
            "Equip {W}",
            "Equip {X}",
            "Equip {01}",
            "Equip legendary creature {1}",
            "Fortify {1}",
            "Equip {1}. Activate only once each turn."
        ),
    },
    Recipe {
        id: RecipeId("static.attached_modifier.creature.fixed"),
        label: "attached creature modifier",
        surface: RecipeSurface::StaticAbility,
        matcher: match_attached_creature_modifier,
        calibration: calibrations!(
            "Short Bow" => "Equipped creature gets +1/+1 and has reach and vigilance.",
            "Unflinching Courage" => "Enchanted creature gets +2/+2 and has trample and lifelink.";
            "Equipped creature gets +X/+X.",
            "Enchanted creature gets +2/+2 until end of turn.",
            "Enchanted creature gets +2/+2 and has ward {2}.",
            "Enchanted creature gets +2/+2 and gains trample.",
            "Equipped creature has protection from red.",
            "Equipped creature has flying. It attacks each combat if able."
        ),
    },
    Recipe {
        id: RecipeId("etb.aura.tap_attached_creature"),
        label: "Aura ETB tap enchanted creature",
        surface: RecipeSurface::EtbAbility,
        matcher: match_aura_etb_tap_attached,
        calibration: calibrations!(
            "Charmed Sleep" => "When this Aura enters, tap enchanted creature.",
            "Colossification" => "When this Aura enters, tap enchanted creature.";
            "When this enchantment enters, tap enchanted creature.",
            "When this Aura enters, you may tap enchanted creature.",
            "When this Aura enters, tap target creature.",
            "When this Aura enters, untap enchanted creature.",
            "When this Aura enters, tap enchanted creature and draw a card."
        ),
    },
    Recipe {
        id: RecipeId("static.aura.attached_creature_untap_step"),
        label: "enchanted creature untap-step restriction",
        surface: RecipeSurface::StaticAbility,
        matcher: match_aura_untap_step_restriction,
        calibration: calibrations!(
            "Charmed Sleep" => "Enchanted creature doesn't untap during its controller's untap step.",
            "Starlight Snare" => "Enchanted creature doesn't untap during its controller's untap step.";
            "Enchanted creature doesn't untap during your untap step.",
            "Enchanted creature doesn't untap during its next untap step.",
            "Enchanted creature can't untap.",
            "Enchanted permanent doesn't untap during its controller's untap step.",
            "Equipped creature doesn't untap during its controller's untap step."
        ),
    },
    Recipe {
        id: RecipeId("modal.choose_one.two_modes"),
        label: "two-bullet modal spell assembly",
        surface: RecipeSurface::ModalAssembly,
        matcher: match_modal_two_modes,
        calibration: calibrations!(
            "Abrade" => "Choose one —\n• Abrade deals 3 damage to target creature.\n• Destroy target artifact.",
            "Family Reunion" => "Choose one —\n• Creatures you control get +1/+1 until end of turn.\n• Creatures you control gain hexproof until end of turn.",
            "Azula Always Lies" => "Choose one or both —\n• Target creature gets -1/-1 until end of turn.\n• Put a +1/+1 counter on target creature.",
            "Overwhelming Surge" => "Choose one or both —\n• Overwhelming Surge deals 3 damage to target creature.\n• Destroy target noncreature artifact.";
            "Choose two —\n• Draw a card.\n• You gain 2 life.",
            "Choose one —\n• Draw a card.",
            "Choose one —\n• Draw a card.\n• You gain 2 life.\n• Create a token.",
            "Choose one —\nDraw a card.\n• You gain 2 life.",
            "Choose one or both —\n• Draw a card.",
            "Choose one or both —\n• Draw a card.\n• You gain 2 life.\n• Create a token."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.damage.creature.three.source"),
        label: "source deals 3 damage to target creature mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_damage_three_to_creature,
        calibration: calibrations!(
            "Abrade" => "Abrade deals 3 damage to target creature.",
            "Thunderclap" => "Thunderclap deals 3 damage to target creature.";
            "Abrade deals 2 damage to target creature.",
            "Abrade deals 3 damage to any target.",
            "It deals 3 damage to target creature.",
            "Abrade deals 3 damage to target creature and 1 damage to you."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.destroy.artifact"),
        label: "destroy target artifact mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_destroy_artifact,
        calibration: calibrations!(
            "Abrade" => "Destroy target artifact.",
            "Ancient Grudge" => "Destroy target artifact.";
            "Destroy up to one target artifact.",
            "Destroy target artifact or planeswalker.",
            "Destroy target artifact with mana value 3 or less.",
            "Destroy target artifact. You gain 2 life."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.pump.team.plus_one_plus_one"),
        label: "team +1/+1 mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_team_plus_one,
        calibration: calibrations!(
            "Family Reunion" => "Creatures you control get +1/+1 until end of turn.",
            "Glorious Charge" => "Creatures you control get +1/+1 until end of turn.";
            "Target creature you control gets +1/+1 until end of turn.",
            "Creatures you control get +2/+2 until end of turn.",
            "Other creatures you control get +1/+1 until end of turn.",
            "Creatures you control get +1/+1 until end of turn and gain vigilance."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.grant.team.hexproof"),
        label: "team hexproof mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_team_hexproof,
        calibration: calibrations!(
            "Family Reunion" => "Creatures you control gain hexproof until end of turn.",
            "Blinding Fog" => "Creatures you control gain hexproof until end of turn.";
            "Target creature you control gains hexproof until end of turn.",
            "Creatures you control gain indestructible until end of turn.",
            "Other creatures you control gain hexproof until end of turn.",
            "Creatures you control gain hexproof and indestructible until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.pump.team.plus_two_power"),
        label: "team +2/+0 mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_team_plus_two_power,
        calibration: calibrations!(
            "Goblin Surprise" => "Creatures you control get +2/+0 until end of turn.",
            "Burn Bright" => "Creatures you control get +2/+0 until end of turn.";
            "Target creature you control gets +2/+0 until end of turn.",
            "Creatures you control get +2/+1 until end of turn.",
            "Attacking creatures you control get +2/+0 until end of turn.",
            "Creatures you control get +2/+0 and gain haste until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.create_tokens.goblin_red_one_one.two"),
        label: "create two red Goblin tokens mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_create_two_goblins,
        calibration: calibrations!(
            "Goblin Surprise" => "Create two 1/1 red Goblin creature tokens.",
            "Dragon Fodder" => "Create two 1/1 red Goblin creature tokens.";
            "Create a 1/1 red Goblin creature token.",
            "Create two 1/1 red Goblin creature tokens with haste.",
            "Create two tapped 1/1 red Goblin creature tokens.",
            "Create two 1/1 white Soldier creature tokens."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.pump.creature.plus_three_plus_three"),
        label: "target creature +3/+3 mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_pump_target_three,
        calibration: calibrations!(
            "Sarkhan's Resolve" => "Target creature gets +3/+3 until end of turn.",
            "Giant Growth" => "Target creature gets +3/+3 until end of turn.";
            "Up to one target creature gets +3/+3 until end of turn.",
            "Target creature you control gets +3/+3 until end of turn.",
            "Target creature gets +4/+4 until end of turn.",
            "Target creature gets +3/+3 and gains trample until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.destroy.creature.flying"),
        label: "destroy target creature with flying mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_destroy_flying_creature,
        calibration: calibrations!(
            "Sarkhan's Resolve" => "Destroy target creature with flying.",
            "Plummet" => "Destroy target creature with flying.";
            "Destroy up to one target creature with flying.",
            "Destroy target creature without flying.",
            "Destroy target creature with flying or reach.",
            "Destroy target creature with flying. You gain 2 life."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.counter.spell.unrestricted"),
        label: "counter target spell mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_counter_spell,
        calibration: calibrations!(
            "Spellgyre" => "Counter target spell.",
            "Counterspell" => "Counter target spell.";
            "Counter up to one target spell.",
            "Counter target spell unless its controller pays {3}.",
            "Counter target noncreature spell.",
            "Counter target spell. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.surveil_two.draw_two"),
        label: "Surveil 2 then draw two mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_surveil_two_draw_two,
        calibration: calibrations!(
            "Spellgyre" => "Surveil 2, then draw two cards.",
            "Diresight" => "Surveil 2, then draw two cards.";
            "Surveil 1, then draw two cards.",
            "Draw two cards, then surveil 2.",
            "Surveil 2, then draw a card.",
            "Surveil 2. Draw two cards."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.counter.spell.unless_four"),
        label: "counter target spell unless four mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_counter_spell_unless_four,
        calibration: calibrations!(
            "Confusticate and Bebother" => "Counter target spell unless its controller pays {4}.",
            "Offering to Asha" => "Counter target spell unless its controller pays {4}.";
            "Counter target spell unless its controller pays {3}.",
            "Counter up to one target spell unless its controller pays {4}.",
            "Counter target noncreature spell unless its controller pays {4}.",
            "Counter target spell unless its controller pays {4}. You gain 2 life."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.draw_two.discard_one"),
        label: "draw two then discard one mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_draw_two_discard_one,
        calibration: calibrations!(
            "Confusticate and Bebother" => "Draw two cards, then discard a card.",
            "Ghastly Discovery" => "Draw two cards, then discard a card.";
            "Draw two cards, then discard two cards.",
            "Draw two cards, then you may discard a card.",
            "Discard a card, then draw two cards.",
            "Draw two cards. Then discard a card."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.damage.creature.equal_power.controlled_to_opponent"),
        label: "controlled creature power damage to opposing creature mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_creature_power_damage,
        calibration: calibrations!(
            "Giantfall" => "Target creature you control deals damage equal to its power to target creature an opponent controls.",
            "Rabid Bite" => "Target creature you control deals damage equal to its power to target creature an opponent controls.";
            "Target creature deals damage equal to its power to target creature an opponent controls.",
            "Target creature you control deals damage equal to its power to up to one target creature an opponent controls.",
            "Target creature you control deals damage equal to its toughness to target creature an opponent controls.",
            "Target creature you control deals damage equal to its power to any target."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.destroy.artifact_or_enchantment"),
        label: "destroy target artifact or enchantment mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_destroy_artifact_or_enchantment,
        calibration: calibrations!(
            "Origin of Metalbending" => "Destroy target artifact or enchantment.",
            "Disenchant" => "Destroy target artifact or enchantment.";
            "Destroy target artifact or planeswalker.",
            "Destroy up to one target artifact or enchantment.",
            "Destroy target noncreature artifact or enchantment.",
            "Destroy target artifact or enchantment. You gain 2 life."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.put_counter.creature.plus_one_plus_one.indestructible"),
        label: "target creature counter and indestructible mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_counter_and_indestructible,
        calibration: singleton_calibrations!(
            "Origin of Metalbending" => "Put a +1/+1 counter on target creature you control. It gains indestructible until end of turn.";
            "Put a +1/+1 counter on target creature you control.",
            "Put a +1/+1 counter on target creature you control. It gains hexproof until end of turn.",
            "Put a +1/+1 counter on target creature you control. It gains indestructible until end of turn. Untap it."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.damage.each_opponent_creature.one.source"),
        label: "source deals one damage to each opposing creature mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_source_damage_each_opponent_one,
        calibration: calibrations!(
            "Iroh's Demonstration" => "Iroh's Demonstration deals 1 damage to each creature your opponents control.",
            "Blazing Volley" => "Blazing Volley deals 1 damage to each creature your opponents control.";
            "Iroh's Demonstration deals 1 damage to each creature.",
            "Iroh's Demonstration deals 2 damage to each creature your opponents control.",
            "Iroh's Demonstration deals 1 damage to each opponent.",
            "It deals 1 damage to each creature your opponents control."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.damage.creature.four.source"),
        label: "source deals four damage to target creature mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_source_damage_four_creature,
        calibration: calibrations!(
            "Iroh's Demonstration" => "Iroh's Demonstration deals 4 damage to target creature.",
            "Bombard" => "Bombard deals 4 damage to target creature.";
            "Iroh's Demonstration deals 5 damage to target creature.",
            "Iroh's Demonstration deals 4 damage to any target.",
            "It deals 4 damage to target creature.",
            "Iroh's Demonstration deals 4 damage to target creature and 1 damage to its controller."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.grant.indestructible.artifact_or_creature"),
        label: "artifact or creature gains indestructible mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_indestructible_artifact_or_creature,
        calibration: singleton_calibrations!(
            "Reroute Systems" => "Target artifact or creature gains indestructible until end of turn.";
            "Target artifact gains indestructible until end of turn.",
            "Target artifact or creature gains hexproof until end of turn.",
            "Target artifact or creature gains indestructible until end of turn. Untap it.",
            "Target artifact or creature you control gains indestructible until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.damage.tapped_creature.two.source"),
        label: "source deals two damage to tapped creature mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_source_damage_tapped_creature,
        calibration: singleton_calibrations!(
            "Reroute Systems" => "Reroute Systems deals 2 damage to target tapped creature.";
            "Reroute Systems deals 3 damage to target tapped creature.",
            "Reroute Systems deals 2 damage to target creature.",
            "Reroute Systems deals 2 damage to target untapped creature.",
            "It deals 2 damage to target tapped creature."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.target_player.draw_two.lose_two"),
        label: "target player draws two and loses two mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_target_player_draw_two_lose_two,
        calibration: calibrations!(
            "Reverent Howl" => "Target player draws two cards and loses 2 life.",
            "Shredder's Revenge" => "Target player draws two cards and loses 2 life.";
            "Target player draws two cards and loses 3 life.",
            "Target player draws two cards.",
            "Target player draws two cards and you lose 2 life.",
            "Each player draws two cards and loses 2 life."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.pump.creature.plus_two_plus_two.lifelink"),
        label: "target creature plus two plus two and lifelink mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_creature_plus_two_lifelink,
        calibration: calibrations!(
            "Reverent Howl" => "Target creature gets +2/+2 and gains lifelink until end of turn.",
            "Give In to Violence" => "Target creature gets +2/+2 and gains lifelink until end of turn.";
            "Target creature gets +2/+2 and gains lifelink until end of turn. Untap it.",
            "Target creature you control gets +2/+2 and gains lifelink until end of turn.",
            "Target creature gets +3/+3 and gains lifelink until end of turn.",
            "Target creature gets +2/+2 and gains trample until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.discard.target_opponent.two"),
        label: "target opponent discards two mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_target_opponent_discard_two,
        calibration: calibrations!(
            "Seeker's Folly" => "Target opponent discards two cards.",
            "Deception" => "Target opponent discards two cards.";
            "Target opponent discards a card.",
            "Target opponent discards two cards at random.",
            "Each opponent discards two cards.",
            "Target opponent discards two cards. You draw a card."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.pump.opponents.creatures.minus_one_minus_one"),
        label: "opposing creatures minus one minus one mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_opponents_minus_one_minus_one,
        calibration: calibrations!(
            "Seeker's Folly" => "Creatures your opponents control get -1/-1 until end of turn.",
            "Make Obsolete" => "Creatures your opponents control get -1/-1 until end of turn.";
            "Creatures your opponents control get -2/-2 until end of turn.",
            "Creatures you control get -1/-1 until end of turn.",
            "Creatures your opponents control get -1/-1 until end of turn. Draw a card.",
            "Creatures your opponents control get -1/-2 until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.discard.target_player.two"),
        label: "target player discards two mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_target_player_discard_two,
        calibration: calibrations!(
            "Shredder's Revenge" => "Target player discards two cards.",
            "Mind Rot" => "Target player discards two cards.";
            "Target player discards a card.",
            "Target player discards two cards at random.",
            "Each player discards two cards.",
            "Target player discards two cards. You draw a card."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.grant.indestructible.creature"),
        label: "target creature gains indestructible mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_target_indestructible_creature,
        calibration: singleton_calibrations!(
            "Valorous Stance" => "Target creature gains indestructible until end of turn.";
            "Target creature gains indestructible until end of turn. Untap it.",
            "Target creature you control gains indestructible until end of turn.",
            "Target creature gains hexproof until end of turn.",
            "Creatures you control gain indestructible until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.destroy.creature.toughness_at_least_four"),
        label: "destroy target creature with toughness four or greater mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_destroy_toughness_four,
        calibration: calibrations!(
            "Valorous Stance" => "Destroy target creature with toughness 4 or greater.",
            "Collar the Culprit" => "Destroy target creature with toughness 4 or greater.";
            "Destroy target creature with toughness 3 or greater.",
            "Destroy target creature with power 4 or greater.",
            "Destroy target creature with toughness 4 or greater. You gain 2 life.",
            "Destroy up to one target creature with toughness 4 or greater."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.put_counter.creature.plus_one_plus_one.trample_hexproof"),
        label: "target creature counter trample hexproof mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_counter_and_keywords,
        calibration: singleton_calibrations!(
            "Warg Tactics" => "Put a +1/+1 counter on target creature you control. It gains trample and hexproof until end of turn.";
            "Put a +1/+1 counter on target creature. It gains trample and hexproof until end of turn.",
            "Put a +1/+1 counter on target creature you control. It gains hexproof and trample until end of turn.",
            "Put two +1/+1 counters on target creature you control. It gains trample and hexproof until end of turn.",
            "Put a +1/+1 counter on target creature you control. It gains trample until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.pump.creature.minus_one_minus_one"),
        label: "target creature minus one minus one mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_creature_minus_one,
        calibration: calibrations!(
            "Azula Always Lies" => "Target creature gets -1/-1 until end of turn.",
            "Night // Day" => "Target creature gets -1/-1 until end of turn.";
            "Target creature gets -2/-2 until end of turn.",
            "Target creature you control gets -1/-1 until end of turn.",
            "Up to one target creature gets -1/-1 until end of turn.",
            "Target creature gets -1/-1 until end of turn. You gain 2 life."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.put_counter.creature.plus_one_plus_one"),
        label: "target creature plus one plus one counter mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_put_counter_creature,
        calibration: calibrations!(
            "Azula Always Lies" => "Put a +1/+1 counter on target creature.",
            "Vastwood Fortification" => "Put a +1/+1 counter on target creature.";
            "Put two +1/+1 counters on target creature.",
            "Put a +1/+1 counter on target creature you control.",
            "Put a +1/+1 counter on up to one target creature.",
            "Put a +1/+1 counter on target creature. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("modal_mode.destroy.noncreature_artifact"),
        label: "destroy target noncreature artifact mode",
        surface: RecipeSurface::ModalMode,
        matcher: match_modal_destroy_noncreature_artifact,
        calibration: calibrations!(
            "Overwhelming Surge" => "Destroy target noncreature artifact.",
            "Crush" => "Destroy target noncreature artifact.";
            "Destroy target noncreature artifact or enchantment.",
            "Destroy target noncreature permanent.",
            "Destroy target noncreature artifact. You gain 2 life.",
            "Destroy target artifact or planeswalker."
        ),
    },
    Recipe {
        id: RecipeId("triggered.controller_gains_life.put_counter.plus_one_plus_one.source.one"),
        label: "controller gains life put a +1/+1 counter on source",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_controller_gains_life_put_counter_on_source,
        calibration: calibrations!(
            "Ajani's Pridemate" => "Whenever you gain life, put a +1/+1 counter on this creature.",
            "Pest Mascot" => "Whenever you gain life, put a +1/+1 counter on this creature.";
            "Whenever you gain 1 life, put a +1/+1 counter on this creature.",
            "Whenever you gain life, put two +1/+1 counters on this creature.",
            "Whenever an opponent gains life, put a +1/+1 counter on this creature.",
            "Whenever you gain life, you may put a +1/+1 counter on this creature."
        ),
    },
    Recipe {
        id: RecipeId("triggered.controller_gains_life.each_opponent_loses_one"),
        label: "controller gains life each opponent loses one",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_controller_gains_life_each_opponent_loses_one,
        calibration: calibrations!(
            "Marauding Blight-Priest" => "Whenever you gain life, each opponent loses 1 life.",
            "Cliffhaven Vampire" => "Whenever you gain life, each opponent loses 1 life.";
            "Whenever you gain life, each opponent loses 2 life.",
            "Whenever an opponent gains life, each opponent loses 1 life.",
            "Whenever you gain life, target opponent loses 1 life.",
            "Whenever you gain life, each opponent loses 1 life and you gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_enters_or_dies.surveil.one"),
        label: "self enters or dies surveil one",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_enters_or_dies_surveil_one,
        calibration: calibrations!(
            "Lys Alana Informant" => "When this creature enters or dies, surveil 1.",
            "Thawbringer" => "When this creature enters or dies, surveil 1.";
            "When this creature enters or dies, surveil 2.",
            "Whenever this creature enters or dies, surveil 1.",
            "When this creature enters or dies, you may surveil 1.",
            "When another creature enters or dies, surveil 1."
        ),
    },
    Recipe {
        id: RecipeId("etb.pump.opponent_creature.minus_two_zero"),
        label: "creature ETB opposing creature -2/-0",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_pump_opponent_creature_minus_two_zero,
        calibration: calibrations!(
            "Cogwork Wrestler" => "When this creature enters, target creature an opponent controls gets -2/-0 until end of turn.",
            "Humbling Elder" => "When this creature enters, target creature an opponent controls gets -2/-0 until end of turn.";
            "When this creature enters, target creature an opponent controls gets -1/-0 until end of turn.",
            "When this creature enters, up to one target creature an opponent controls gets -2/-0 until end of turn.",
            "When this creature enters, target creature gets -2/-0 until end of turn.",
            "When this creature dies, target creature an opponent controls gets -2/-0 until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("etb.discard.target_opponent.one"),
        label: "creature ETB target opponent discards one",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_target_opponent_discards_one,
        calibration: calibrations!(
            "Corrupt Court Official" => "When this creature enters, target opponent discards a card.",
            "Ravenous Rats" => "When this creature enters, target opponent discards a card.";
            "When this creature enters, target opponent discards two cards.",
            "When this creature enters, you may have target opponent discard a card.",
            "When this creature dies, target opponent discards a card.",
            "When another creature enters, target opponent discards a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.create_token.clue.one"),
        label: "creature ETB create one Clue",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_create_clue,
        calibration: calibrations!(
            "Forecasting Fortune Teller" => "When this creature enters, create a Clue token.",
            "Novice Inspector" => "When this creature enters, investigate.";
            "When this creature enters, create two Clue tokens.",
            "When this creature enters, you may investigate.",
            "When this creature enters, create a tapped Clue token.",
            "Whenever another creature enters, investigate."
        ),
    },
    Recipe {
        id: RecipeId("etb.aura.draw.one"),
        label: "Aura ETB draw one",
        surface: RecipeSurface::EtbAbility,
        matcher: match_aura_etb_draw_one,
        calibration: calibrations!(
            "Feather of Flight" => "When this Aura enters, draw a card.",
            "Lofty Dreams" => "When this Aura enters, draw a card.";
            "When this Aura enters, draw two cards.",
            "When this Aura enters, you may draw a card.",
            "When this enchantment enters, draw a card.",
            "When this Aura dies, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.create_token.printed_one_one.one"),
        label: "creature ETB create one reviewed printed 1/1 token",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_create_printed_one_one,
        calibration: calibrations!(
            "Resolute Reinforcements" => "When this creature enters, create a 1/1 white Soldier creature token.",
            "Merrow Skyswimmer" => "When this creature enters, create a 1/1 white and blue Merfolk creature token.";
            "When this creature enters, create two 1/1 white Soldier creature tokens.",
            "When this creature enters, create a tapped 1/1 white Soldier creature token.",
            "When this creature enters, create a 2/2 white Soldier creature token.",
            "When another creature enters, create a 1/1 white Soldier creature token."
        ),
    },
    Recipe {
        id: RecipeId("etb.damage.any_target.one.source"),
        label: "creature ETB source deals one damage to any target",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_damage_any_target_one,
        calibration: calibrations!(
            "Mongoose Lizard" => "When this creature enters, it deals 1 damage to any target.",
            "Skeleton Archer" => "When this creature enters, it deals 1 damage to any target.";
            "When this creature enters, it deals 2 damage to any target.",
            "When this creature enters, it deals 1 damage to target opponent.",
            "When this creature enters, you may have it deal 1 damage to any target.",
            "When this creature dies, it deals 1 damage to any target."
        ),
    },
    Recipe {
        id: RecipeId("etb.put_counter.plus_one_plus_one.other_creature_you_control.one"),
        label: "creature ETB put a +1/+1 counter on another controlled creature",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_put_counter_on_other_controlled_creature,
        calibration: calibrations!(
            "Sterling Supplier" => "When this creature enters, put a +1/+1 counter on another target creature you control.",
            "Keen-Eyed Raven" => "When this creature enters, put a +1/+1 counter on another target creature you control.";
            "When this creature enters, put two +1/+1 counters on another target creature you control.",
            "When this creature enters, put a +1/+1 counter on another target creature.",
            "When this creature enters, put a +1/+1 counter on another target permanent you control.",
            "When this creature dies, put a +1/+1 counter on another target creature you control."
        ),
    },
    Recipe {
        id: RecipeId("etb.return_to_hand.other_creature.up_to_one"),
        label: "creature ETB return up to one other creature",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_return_other_creature_up_to_one,
        calibration: calibrations!(
            "Rimekin Recluse" => "When this creature enters, return up to one other target creature to its owner's hand.",
            "Matterbending Mage" => "When this creature enters, return up to one other target creature to its owner's hand.";
            "When this creature enters, return up to two other target creatures to their owners' hands.",
            "When this creature enters, return another target creature to its owner's hand.",
            "When this creature enters, return up to one other target permanent to its owner's hand.",
            "When this creature dies, return up to one other target creature to its owner's hand."
        ),
    },
    Recipe {
        id: RecipeId("triggered.etb.raid.draw.one"),
        label: "Raid ETB draw one",
        surface: RecipeSurface::EtbAbility,
        matcher: match_raid_etb_draw,
        calibration: calibrations!(
            "Storm Fleet Spy" => "Raid — When this creature enters, if you attacked this turn, draw a card.",
            "Skyship Buccaneer" => "Raid — When this creature enters, if you attacked this turn, draw a card.";
            "Raid — When this creature enters, draw a card.",
            "Raid — When this creature enters, if you attacked this turn, draw two cards.",
            "Raid — When another creature enters, if you attacked this turn, draw a card.",
            "Raid — When this creature enters, if an opponent attacked this turn, draw a card.",
            "Raid — When this creature enters, if you attacked this turn, you may draw a card."
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
        id: RecipeId("etb.artifact.draw.one"),
        label: "artifact ETB draw one",
        surface: RecipeSurface::EtbAbility,
        matcher: match_artifact_etb_draw,
        calibration: calibrations!(
            "Futurist Forge" => "When this artifact enters, draw a card.",
            "Prophetic Prism" => "When this artifact enters, draw a card.";
            "When this enchantment enters, draw a card.",
            "When this artifact enters, you may draw a card.",
            "When this artifact enters, draw two cards.",
            "When this artifact enters, draw a card, then discard a card.",
            "Whenever another artifact enters, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.artifact.scry.two"),
        label: "artifact ETB scry two",
        surface: RecipeSurface::EtbAbility,
        matcher: match_artifact_etb_scry_two,
        calibration: calibrations!(
            "Candy Trail" => "When this artifact enters, scry 2.",
            "Giant's Boulder" => "When this artifact enters, scry 2.";
            "When this enchantment enters, scry 2.",
            "When this artifact enters, you may scry 2.",
            "When this artifact enters, scry 1.",
            "When this artifact enters, target player scries 2.",
            "When this artifact enters, scry 2, then draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.artifact.create_food.one"),
        label: "artifact ETB create Food",
        surface: RecipeSurface::EtbAbility,
        matcher: match_artifact_etb_create_food,
        calibration: calibrations!(
            "Bumbleflower's Sharepot" => "When this artifact enters, create a Food token.",
            "Hot Dog Cart" => "When this artifact enters, create a Food token.";
            "When this enchantment enters, create a Food token.",
            "When this artifact enters, you may create a Food token.",
            "When this artifact enters, create two Food tokens.",
            "When this artifact enters, create a tapped Food token.",
            "When this artifact enters, create a Food token, then draw a card."
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
            "When this creature enters, one opponent discards a card.",
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
            "When this creature enters, create a 1/1 white Warrior creature token.",
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
            "When this creature enters, create a Blood token.",
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
            "When this creature enters, you may mill up to two cards.",
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
        id: RecipeId("etb.return_to_hand.other_permanent_you_control.up_to_one"),
        label: "ETB return up to one other controlled permanent",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_return_other_controlled_permanent_up_to_one,
        calibration: calibrations!(
            "Exosuit Savior" => "When this creature enters, return up to one other target permanent you control to its owner's hand.",
            "Mischievous Pup" => "When this creature enters, return up to one other target permanent you control to its owner's hand.";
            "When this creature enters, you may return up to one other target permanent you control to its owner's hand.",
            "When this creature enters, return one other target permanent you control to its owner's hand.",
            "When this creature enters, return up to two other target permanents you control to their owners' hands.",
            "When this creature enters, return up to one target permanent you control to its owner's hand.",
            "When this creature enters, return up to one other target creature you control to its owner's hand.",
            "When this creature enters, return up to one other target permanent to its owner's hand.",
            "When this creature enters, return up to one other target permanent an opponent controls to its owner's hand.",
            "Whenever another creature enters, return up to one other target permanent you control to its owner's hand.",
            "When this creature enters, return up to one other target permanent you control to its owner's hand, then draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.mill.controller.two.may"),
        label: "ETB may mill two",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_mill_two_may,
        calibration: calibrations!(
            "Daggerfang Duo" => "When this creature enters, you may mill two cards.",
            "Deathcap Marionette" => "When this creature enters, you may mill two cards.";
            "When this creature enters, you may mill up to two cards.",
            "When this creature enters, you may mill a card.",
            "When this creature enters, you may mill three cards.",
            "When this creature enters, target player may mill two cards.",
            "When this creature enters, each player may mill two cards.",
            "Whenever another creature enters, you may mill two cards.",
            "When this creature enters, you may mill two cards, then return a card from your graveyard to your hand."
        ),
    },
    Recipe {
        id: RecipeId(
            "triggered.controller_casts.noncreature.put_counter.plus_one_plus_one.source.one",
        ),
        label: "controller casts noncreature put one counter on source",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_controller_casts_noncreature_put_counter,
        calibration: calibrations!(
            "Boar-q-pine" => "Whenever you cast a noncreature spell, put a +1/+1 counter on this creature.",
            "Tempest Angler" => "Whenever you cast a noncreature spell, put a +1/+1 counter on this creature.";
            "Whenever you cast a noncreature spell, you may put a +1/+1 counter on this creature.",
            "Whenever you cast a creature spell, put a +1/+1 counter on this creature.",
            "Whenever you cast an instant or sorcery spell, put a +1/+1 counter on this creature.",
            "Whenever an opponent casts a noncreature spell, put a +1/+1 counter on this creature.",
            "Whenever you cast your second noncreature spell each turn, put a +1/+1 counter on this creature.",
            "Whenever you cast a noncreature spell, put two +1/+1 counters on this creature.",
            "Whenever you cast a noncreature spell, put a +1/+1 counter on target creature.",
            "Whenever you cast a noncreature spell, put a +1/+1 counter on this creature and scry 1."
        ),
    },
    Recipe {
        id: RecipeId("triggered.controller_draws.second.put_counter.plus_one_plus_one.source.one"),
        label: "controller draws second card put one counter on source",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_controller_draws_second_put_counter,
        calibration: calibrations!(
            "Atlantean Cavalry" => "Whenever you draw your second card each turn, put a +1/+1 counter on this creature.",
            "Lakeshore Apothecary" => "Whenever you draw your second card each turn, put a +1/+1 counter on this creature.";
            "Whenever you draw your second card each turn, you may put a +1/+1 counter on this creature.",
            "Whenever you draw your first card each turn, put a +1/+1 counter on this creature.",
            "Whenever you draw your third card each turn, put a +1/+1 counter on this creature.",
            "Whenever a player draws their second card each turn, put a +1/+1 counter on this creature.",
            "Whenever an opponent draws their second card each turn, put a +1/+1 counter on this creature.",
            "Whenever you draw your second card each turn, put two +1/+1 counters on this creature.",
            "Whenever you draw your second card each turn, put a +1/+1 counter on target creature.",
            "Whenever you draw your second card each turn, if this creature is tapped, put a +1/+1 counter on it.",
            "Whenever you draw your second card each turn, put a +1/+1 counter on this creature, then scry 1."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_attacks.gain_life.controller.two"),
        label: "self attacks gain two life",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_attacks_gain_two,
        calibration: calibrations!(
            "Herald of Faith" => "Whenever this creature attacks, you gain 2 life.",
            "Shopkeeper's Bane" => "Whenever this creature attacks, you gain 2 life.";
            "Whenever this creature attacks, you may gain 2 life.",
            "Whenever this creature attacks, you gain 1 life.",
            "Whenever this creature attacks, you gain 3 life.",
            "Whenever another creature attacks, you gain 2 life.",
            "Whenever one or more creatures you control attack, you gain 2 life.",
            "Whenever this creature attacks with another creature, you gain 2 life.",
            "Whenever this creature blocks, you gain 2 life.",
            "Whenever this creature attacks, each player gains 2 life.",
            "Whenever this creature attacks, you gain 2 life and draw a card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_attacks.mill.controller.one"),
        label: "self attacks mill one",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_attacks_mill_one,
        calibration: calibrations!(
            "Mysterio's Phantasm" => "Whenever this creature attacks, mill a card.",
            "Screaming Phantom" => "Whenever this creature attacks, mill a card.";
            "Whenever this creature attacks, you may mill a card.",
            "Whenever this creature attacks, mill two cards.",
            "Whenever this creature attacks, target player mills a card.",
            "Whenever this creature attacks, each player mills a card.",
            "Whenever another creature attacks, mill a card.",
            "Whenever one or more creatures you control attack, mill a card.",
            "Whenever this creature becomes blocked, mill a card.",
            "Whenever this creature attacks, mill a card, then draw a card."
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
        id: RecipeId("etb.equipment.attach_target_controlled_creature"),
        label: "Equipment ETB attach to target creature you control",
        surface: RecipeSurface::EtbAbility,
        matcher: match_equipment_etb_attach,
        calibration: calibrations!(
            "Meltstrider's Gear" => "When this Equipment enters, attach it to target creature you control.",
            "Malamet Scythe" => "When this Equipment enters, attach it to target creature you control.";
            "When this Equipment enters, attach it to a target creature you control.",
            "When this Equipment enters, attach it to target creature.",
            "When this Equipment enters, you may attach it to target creature you control.",
            "When this Equipment enters, attach another Equipment to target creature you control.",
            "When this Equipment enters, attach it to target artifact creature you control.",
            "When this Equipment enters, attach it to target creature you control. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.equipment.attach_target_controlled_creature_first_strike"),
        label: "Equipment ETB attach then grant first strike",
        surface: RecipeSurface::EtbAbility,
        matcher: match_equipment_etb_attach_first_strike,
        calibration: calibrations!(
            "Squire's Lightblade" => "When this Equipment enters, attach it to target creature you control. That creature gains first strike until end of turn.",
            "Coral Sword" => "When this Equipment enters, attach it to target creature you control. That creature gains first strike until end of turn.";
            "When this Equipment enters, attach it to target creature you control. That creature gains double strike until end of turn.",
            "When this Equipment enters, attach it to target creature you control. That creature gains first strike permanently.",
            "When this Equipment enters, attach it to target creature you control. You gain first strike until end of turn.",
            "When this Equipment enters, attach it to target creature you control, then that creature gains first strike until end of turn.",
            "When this Equipment enters, attach it to target creature you control. That creature gains first strike and vigilance until end of turn.",
            "When this Equipment enters, attach it to target creature you control. That creature gains first strike until end of turn. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.equipment.manifest_dread_attach"),
        label: "Equipment ETB manifest dread then attach",
        surface: RecipeSurface::EtbAbility,
        matcher: match_equipment_etb_manifest_dread_attach,
        calibration: calibrations!(
            "Conductive Machete" => "When this Equipment enters, manifest dread, then attach this Equipment to that creature.",
            "Cursed Windbreaker" => "When this Equipment enters, manifest dread, then attach this Equipment to that creature.",
            "Killer's Mask" => "When this Equipment enters, manifest dread, then attach this Equipment to that creature.";
            "When this Equipment enters, manifest dread, then attach this Equipment to target creature.",
            "When this Equipment enters, manifest dread, then attach this Equipment to a creature.",
            "When this Equipment enters, manifest dread, then you may attach this Equipment to that creature.",
            "When this Equipment enters, attach this Equipment to that creature, then manifest dread.",
            "When this Equipment enters, manifest one of the top two cards, then attach this Equipment to that creature.",
            "When this Equipment enters, manifest dread, then attach another Equipment to that creature.",
            "When this Equipment enters, manifest dread, then attach this Equipment to that creature. You draw a card.",
            "When this Equipment enters, manifest dread, then attach this Equipment to that creature. (It grants flying.)"
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
        id: RecipeId("triggered.increment"),
        label: "Increment",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_increment,
        calibration: calibrations!(
            "Cuboid Colony" => "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)",
            "Textbook Tabulator" => "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)",
            "Hungry Graffalon" => "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)";
            "Increment (Whenever you cast a spell, if you spent more than four mana, put a +1/+1 counter on this creature.)",
            "Increment (Whenever you cast a spell, if the spell's mana value is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)",
            "Increment (Whenever you cast a spell, if the amount of mana you spent is at least this creature's power or toughness, put a +1/+1 counter on this creature.)",
            "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power, put a +1/+1 counter on this creature.)",
            "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's toughness, put a +1/+1 counter on this creature.)",
            "Increment (Whenever an opponent casts a spell, if the amount of mana they spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)",
            "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put two +1/+1 counters on this creature.)",
            "Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.",
            "Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature. Increment",
            "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.) Whenever this creature attacks, draw a card."
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
            "Prophetic Prism" => "{1}, {T}: Add one mana of any color.";
            "{1}: Add one mana of any color.",
            "{2}, {T}: Add one mana of any color.",
            "{1}, {T}: Add one mana of any type.",
            "{1}, {T}: Add two mana of any one color.",
            "{1}, {T}: Add one mana of any color. Spend this mana only to cast creature spells.",
            "{1}, {T}, Pay 1 life: Add one mana of any color."
        ),
    },
    Recipe {
        id: RecipeId("activated.artifact.pay_one_tap_sacrifice.any_color"),
        label: "artifact tap-sacrifice for any color",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_artifact_pay_one_tap_sacrifice_any_color,
        calibration: calibrations!(
            "Golden Egg" => "{1}, {T}, Sacrifice this artifact: Add one mana of any color.",
            "Omni-Cheese Pizza" => "{1}, {T}, Sacrifice this artifact: Add one mana of any color.";
            "{1}, Sacrifice this artifact: Add one mana of any color.",
            "{1}, {T}, Sacrifice this artifact: Add {C}.",
            "{1}, {T}, Sacrifice this creature: Add one mana of any color.",
            "{2}, {T}, Sacrifice this artifact: Add one mana of any color.",
            "{1}, {T}, Sacrifice this artifact: Add two mana of any one color.",
            "{1}, {T}, Sacrifice this artifact: Add one mana of any color. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("activated.artifact.pay_two_tap_sacrifice.gain_three"),
        label: "artifact tap-sacrifice gain three",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_artifact_pay_two_tap_sacrifice_gain_three,
        calibration: calibrations!(
            "Instant Ramen" => "{2}, {T}, Sacrifice this artifact: You gain 3 life.",
            "Omni-Cheese Pizza" => "{2}, {T}, Sacrifice this artifact: You gain 3 life.";
            "{2}, Sacrifice this artifact: You gain 3 life.",
            "{2}, {T}: You gain 3 life.",
            "{2}, {T}, Sacrifice this creature: You gain 3 life.",
            "{1}, {T}, Sacrifice this artifact: You gain 3 life.",
            "{2}, {T}, Sacrifice this artifact: You gain 2 life.",
            "{2}, {T}, Sacrifice this artifact: You gain 3 life and create a Food token."
        ),
    },
    Recipe {
        id: RecipeId("activated.artifact.pay_two_tap_sacrifice.gain_three_draw_one"),
        label: "artifact tap-sacrifice gain three then draw one",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_artifact_pay_two_tap_sacrifice_gain_three_draw_one,
        calibration: calibrations!(
            "Candy Trail" => "{2}, {T}, Sacrifice this artifact: You gain 3 life and draw a card.",
            "Bagel and Schmear" => "{2}, {T}, Sacrifice this artifact: You gain 3 life and draw a card.";
            "{2}, Sacrifice this artifact: You gain 3 life and draw a card.",
            "{2}, {T}: You gain 3 life and draw a card.",
            "{2}, {T}, Sacrifice this creature: You gain 3 life and draw a card.",
            "{1}, {T}, Sacrifice this artifact: You gain 3 life and draw a card.",
            "{2}, {T}, Sacrifice this artifact: You gain 2 life and draw a card.",
            "{2}, {T}, Sacrifice this artifact: Draw a card and you gain 3 life.",
            "{2}, {T}, Sacrifice this artifact: You gain 3 life and draw two cards."
        ),
    },
    Recipe {
        id: RecipeId("activated.artifact.pay_three_u_sacrifice.draw_two"),
        label: "artifact sacrifice draw two",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_artifact_pay_three_u_sacrifice_draw_two,
        calibration: calibrations!(
            "Futurist Forge" => "{3}{U}, Sacrifice this artifact: Draw two cards.",
            "Sewer-veillance Cam" => "{3}{U}, Sacrifice this artifact: Draw two cards.";
            "{3}{U}, {T}, Sacrifice this artifact: Draw two cards.",
            "{3}{U}, Sacrifice this creature: Draw two cards.",
            "{2}{U}, Sacrifice this artifact: Draw two cards.",
            "{3}{U}, Sacrifice this artifact: Draw a card.",
            "{3}{U}, Sacrifice this artifact: You may draw two cards.",
            "{3}{U}, Sacrifice this artifact: Draw two cards, then discard a card."
        ),
    },
    Recipe {
        id: RecipeId("activated.creature.pay_two_sacrifice.draw_one"),
        label: "creature sacrifice draw one",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_creature_pay_two_sacrifice_draw_one,
        calibration: calibrations!(
            "Illvoi Galeblade" => "{2}, Sacrifice this creature: Draw a card.",
            "Red Herring" => "{2}, Sacrifice this creature: Draw a card.";
            "{2}, {T}, Sacrifice this creature: Draw a card.",
            "{2}, Sacrifice this artifact: Draw a card.",
            "{1}, Sacrifice this creature: Draw a card.",
            "{2}, Sacrifice another creature: Draw a card.",
            "{2}, Sacrifice this creature: Draw two cards.",
            "{2}, Sacrifice this creature: You may draw a card."
        ),
    },
    Recipe {
        id: RecipeId("activated.artifact.pay_three_tap_sacrifice.damage_creature_three"),
        label: "artifact tap-sacrifice damage creature three",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_artifact_pay_three_tap_sacrifice_damage_creature,
        calibration: calibrations!(
            "Bear Trap" => "{3}, {T}, Sacrifice this artifact: It deals 3 damage to target creature.",
            "Scrap Compactor" => "{3}, {T}, Sacrifice this artifact: It deals 3 damage to target creature.";
            "{3}, Sacrifice this artifact: It deals 3 damage to target creature.",
            "{3}, {T}, Sacrifice this creature: It deals 3 damage to target creature.",
            "{2}, {T}, Sacrifice this artifact: It deals 3 damage to target creature.",
            "{3}, {T}, Sacrifice this artifact: It deals 2 damage to target creature.",
            "{3}, {T}, Sacrifice this artifact: It deals 3 damage to any target.",
            "{3}, {T}, Sacrifice this artifact: It deals 3 damage to up to one target creature."
        ),
    },
    Recipe {
        id: RecipeId("activated.artifact.pay_seven_tap_sacrifice.destroy_permanent"),
        label: "artifact tap-sacrifice destroy permanent",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_artifact_pay_seven_tap_sacrifice_destroy_permanent,
        calibration: calibrations!(
            "Giant's Boulder" => "{7}, {T}, Sacrifice this artifact: Destroy target permanent.",
            "Goblin Firebomb" => "{7}, {T}, Sacrifice this artifact: Destroy target permanent.";
            "{7}, Sacrifice this artifact: Destroy target permanent.",
            "{7}, {T}, Sacrifice this creature: Destroy target permanent.",
            "{6}, {T}, Sacrifice this artifact: Destroy target permanent.",
            "{7}, {T}, Sacrifice this artifact: Destroy target creature.",
            "{7}, {T}, Sacrifice this artifact: Destroy up to one target permanent.",
            "{7}, {T}, Sacrifice this artifact: Exile target permanent.",
            "{7}, {T}, Sacrifice this artifact: Destroy target permanent. Activate only as a sorcery."
        ),
    },
    Recipe {
        id: RecipeId("activated.mana.pay_one.any_color.per_turn_one"),
        label: "pay one for any color once each turn",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_pay_one_for_any_color_once_per_turn,
        calibration: calibrations!(
            "Scarecrow Guide" => "{1}: Add one mana of any color. Activate only once each turn.",
            "Three Tree Mascot" => "{1}: Add one mana of any color. Activate only once each turn.";
            "{2}: Add one mana of any color. Activate only once each turn.",
            "{1}: Add two mana of any one color. Activate only once each turn.",
            "{1}: Add one mana of any type. Activate only once each turn.",
            "{1}: Add one mana of any color.",
            "{1}: Add one mana of any color. Activate only twice each turn.",
            "{1}: Add one mana of any color. Activate only once each turn. You gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("activated.pump.self.plus_1_plus_1.pay_1b"),
        label: "pay 1B to pump this creature +1/+1",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_creature_self_pump_one_one,
        calibration: calibrations!(
            "Burrog Banemaker" => "{1}{B}: This creature gets +1/+1 until end of turn.",
            "Ravine Raider" => "{1}{B}: This creature gets +1/+1 until end of turn.";
            "{B}{1}: This creature gets +1/+1 until end of turn.",
            "{1}{B}: This creature gets +2/+2 until end of turn.",
            "{1}{B}: Target creature gets +1/+1 until end of turn.",
            "{1}{B}: Creatures you control get +1/+1 until end of turn.",
            "{1}{B}: This creature gets +1/+1 until end of turn. Activate only once each turn.",
            "{1}{B}: This creature gets +1/+1 until end of turn if you control an artifact.",
            "{1}{B}: This creature gets +1/+1 until end of turn. You gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("activated.pump.self.plus_2_plus_2.pay_2g.per_turn_one"),
        label: "pay 2G to pump this creature +2/+2 once each turn",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_creature_self_pump_two_two_once_per_turn,
        calibration: calibrations!(
            "Kraven's Cats" => "{2}{G}: This creature gets +2/+2 until end of turn. Activate only once each turn.",
            "Mindful Biomancer" => "{2}{G}: This creature gets +2/+2 until end of turn. Activate only once each turn.";
            "{G}{2}: This creature gets +2/+2 until end of turn. Activate only once each turn.",
            "{2}{G}: This creature gets +1/+1 until end of turn. Activate only once each turn.",
            "{2}{G}: Target creature gets +2/+2 until end of turn. Activate only once each turn.",
            "{2}{G}: This creature gets +2/+2 until end of turn.",
            "{2}{G}: This creature gets +2/+2 until end of turn. Activate only twice each turn.",
            "{2}{G}: This creature gets +2/+2 until end of turn if you control an artifact. Activate only once each turn.",
            "{2}{G}: This creature gets +2/+2 until end of turn and gains trample. Activate only once each turn."
        ),
    },
    Recipe {
        id: RecipeId("activated.pump.creatures_you_control.plus_1_plus_1.fixed_cost"),
        label: "fixed-cost team +1/+1 pump",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_creature_team_pump_one_one,
        calibration: calibrations!(
            "Dwarven Provisioner" => "{3}{W}: Creatures you control get +1/+1 until end of turn.",
            "Dual-Sun Adepts" => "{5}: Creatures you control get +1/+1 until end of turn.";
            "{W}{3}: Creatures you control get +1/+1 until end of turn.",
            "{4}: Creatures you control get +1/+1 until end of turn.",
            "{5}{W}: Creatures you control get +1/+1 until end of turn.",
            "{3}{W}: Creatures you control get +2/+2 until end of turn.",
            "{3}{W}: Target creature you control gets +1/+1 until end of turn.",
            "{3}{W}: Other creatures you control get +1/+1 until end of turn.",
            "{3}{W}: Creatures you control get +1/+1 until end of turn. Activate only once each turn.",
            "{3}{W}: Creatures you control get +1/+1 until end of turn if you control an artifact.",
            "{3}{W}: Creatures you control get +1/+1 and gain vigilance until end of turn."
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
        id: RecipeId("activated.land.tap_sacrifice.search_basic_land.battlefield_tapped"),
        label: "tap-sacrifice land basic search",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_land_tap_sacrifice_search_basic_tapped,
        calibration: calibrations!(
            "Terramorphic Expanse" => "{T}, Sacrifice this land: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "Vibrant Cityscape" => "{T}, Sacrifice this land: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.";
            "Sacrifice this land, {T}: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "Sacrifice this land: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "{T}: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "{T}, Sacrifice this creature: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "{T}, Sacrifice this land: Search your library for a land card, put it onto the battlefield tapped, then shuffle.",
            "{T}, Sacrifice this land: Search your library for a basic land card, put it into your hand, then shuffle.",
            "{T}, Sacrifice this land: Search your library for a basic land card, put it onto the battlefield, then shuffle.",
            "{T}, Sacrifice this land: You may search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "{T}, Sacrifice this land: Search your library for a basic land card, put it onto the battlefield tapped.",
            "{T}, Sacrifice this land: Search your library for a basic land card, reveal it, put it onto the battlefield tapped, then shuffle.",
            "{T}, Sacrifice this land: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle. You gain 1 life."
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
        id: RecipeId("static.conditional_self.first_strike.controller_turn"),
        label: "self first strike during controller turn",
        surface: RecipeSurface::StaticAbility,
        matcher: match_controller_turn_self_first_strike,
        calibration: calibrations!(
            "Bearer of Glory" => "During your turn, this creature has first strike.",
            "Feisty Spikeling" => "During your turn, this creature has first strike.";
            "During your turn, this creature has double strike.",
            "During your opponent's turn, this creature has first strike.",
            "This creature has first strike during your turn.",
            "During your turn, this creature gets +1/+1."
        ),
    },
    Recipe {
        id: RecipeId("static.self_combat_restriction.cant_block"),
        label: "self cannot block",
        surface: RecipeSurface::StaticAbility,
        matcher: match_self_cannot_block,
        calibration: calibrations!(
            "Vampire Interloper" => "This creature can't block.",
            "Vampire Soulcaller" => "This creature can't block.";
            "This creature can't attack.",
            "This creature can't block or attack.",
            "This creature can't block unless you control another creature.",
            "Other creatures can't block."
        ),
    },
    Recipe {
        id: RecipeId("static.anthem_pt.creatures_you_control.plus_one_plus_one"),
        label: "creatures you control plus one plus one anthem",
        surface: RecipeSurface::StaticAbility,
        matcher: match_controller_creature_anthem_plus_one_plus_one,
        calibration: calibrations!(
            "Anthem of Champions" => "Creatures you control get +1/+1.",
            "Warleader's Call" => "Creatures you control get +1/+1.";
            "Creatures you control get +1/+1 until end of turn.",
            "Other creatures you control get +1/+1.",
            "Creatures you control get +2/+2.",
            "Creatures you control gain +1/+1."
        ),
    },
    Recipe {
        id: RecipeId("static.enters_tapped.creature.unconditional"),
        label: "unconditional tapped creature entry",
        surface: RecipeSurface::StaticAbility,
        matcher: match_unconditional_creature_enters_tapped,
        calibration: calibrations!(
            "Daring Thunder-Thief" => "This creature enters tapped.",
            "Diregraf Ghoul" => "This creature enters tapped.";
            "This artifact enters tapped.",
            "This creature enters the battlefield tapped.",
            "This creature enters tapped unless you pay 2 life.",
            "This creature enters tapped. When it enters, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("static.extra_land_plays.one"),
        label: "one additional land play each turn",
        surface: RecipeSurface::StaticAbility,
        matcher: match_extra_land_play,
        calibration: calibrations!(
            "Icetill Explorer" => "You may play an additional land on each of your turns.",
            "Case of the Locked Hothouse" => "You may play an additional land on each of your turns.";
            "You may play an additional land this turn.",
            "You may play up to two additional lands on each of your turns.",
            "You may play an additional land on each player's turn.",
            "You may play an additional land on each of your turns if you control a creature."
        ),
    },
    Recipe {
        id: RecipeId("static.play_lands_from_own_graveyard"),
        label: "play lands from own graveyard",
        surface: RecipeSurface::StaticAbility,
        matcher: match_play_lands_from_own_graveyard,
        calibration: calibrations!(
            "Icetill Explorer" => "You may play lands from your graveyard.",
            "Mole Man, Moloid Master" => "You may play lands from your graveyard.";
            "You may play land cards from any graveyard.",
            "You may cast spells from your graveyard.",
            "You may play lands from your hand.",
            "You may play lands from your graveyard this turn."
        ),
    },
    Recipe {
        id: RecipeId("static.anthem_keyword.countered_creatures.first_strike.controller_turn"),
        label: "countered creatures first strike during controller turn",
        surface: RecipeSurface::StaticAbility,
        matcher: match_controller_turn_countered_creatures_first_strike,
        calibration: singleton_calibrations!(
            "Inspiring Paladin" => "During your turn, creatures you control with +1/+1 counters on them have first strike.";
            "During your turn, creatures you control with +1/+1 counters on them have double strike.",
            "During your turn, creatures you control have first strike.",
            "Creatures you control with +1/+1 counters on them have first strike.",
            "During your turn, creatures you control with charge counters on them have first strike."
        ),
    },
    Recipe {
        id: RecipeId("triggered.landfall.mill.one"),
        label: "landfall mill one",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_landfall_mill_one,
        calibration: singleton_calibrations!(
            "Icetill Explorer" => "Landfall — Whenever a land you control enters, mill a card.";
            "Landfall — Whenever a land you control enters, mill two cards.",
            "Landfall — Whenever a land enters, mill a card.",
            "Landfall — Whenever a land you control enters, you may mill a card.",
            "Landfall — Whenever a land you control enters, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.landfall.damage.each_opponent.one"),
        label: "landfall damage each opponent one",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_landfall_damage_each_opponent_one,
        calibration: calibrations!(
            "Sabotender" => "Landfall — Whenever a land you control enters, this creature deals 1 damage to each opponent.",
            "Spitfire Lagac" => "Landfall — Whenever a land you control enters, this creature deals 1 damage to each opponent.";
            "Whenever a land you control enters, this creature deals 1 damage to each opponent.",
            "Landfall — Whenever a land enters, this creature deals 1 damage to each opponent.",
            "Landfall — Whenever a land you control enters, this creature deals 2 damage to each opponent.",
            "Landfall — Whenever a land you control enters, target opponent takes 1 damage.",
            "Landfall — Whenever a land you control enters, each opponent loses 1 life."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_attacks.optional_discard_then_draw"),
        label: "self attacks optional discard then draw",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_attacks_optional_discard_then_draw,
        calibration: singleton_calibrations!(
            "Null Group Biological Assets" => "Whenever this creature attacks, you may discard a card. If you do, draw a card.";
            "Whenever this creature attacks, discard a card, then draw a card.",
            "Whenever this creature attacks, you may draw a card, then discard a card.",
            "Whenever this creature attacks, you may discard two cards. If you do, draw a card.",
            "Whenever this creature attacks, you may discard a card. If you do, draw two cards."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_becomes_tapped.draw_then_discard"),
        label: "self becomes tapped draw then discard",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_becomes_tapped_draw_then_discard,
        calibration: calibrations!(
            "Silvergill Peddler" => "Whenever this creature becomes tapped, draw a card, then discard a card.",
            "Mechan Navigator" => "Whenever this creature becomes tapped, draw a card, then discard a card.";
            "Whenever this creature becomes tapped, you may draw a card, then discard a card.",
            "Whenever this creature becomes tapped, discard a card, then draw a card.",
            "Whenever this creature becomes tapped, draw two cards, then discard a card.",
            "Whenever this creature becomes tapped, draw a card, then discard two cards.",
            "Whenever this creature attacks, draw a card, then discard a card.",
            "When this creature enters, draw two cards, then discard a card.",
            "Whenever enchanted creature becomes tapped, draw a card, then discard a card.",
            "Whenever you tap this creature for a cost, draw a card, then discard a card.",
            "Whenever this creature becomes tapped, draw a card, then discard a card. You gain 1 life.",
            "Whenever this creature becomes tapped, draw a card, then discard a card. This ability triggers only once each turn."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_becomes_tapped.optional_discard_then_draw"),
        label: "self becomes tapped optional discard then draw",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_becomes_tapped_optional_discard_then_draw,
        calibration: calibrations!(
            "Rescue Leopard" => "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card.",
            "Volatile Wanderglyph" => "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card.";
            "Whenever this creature becomes tapped, discard a card. If you do, draw a card.",
            "Whenever this creature becomes tapped, you may draw a card, then discard a card.",
            "Whenever this creature becomes tapped, you may discard two cards. If you do, draw a card.",
            "Whenever this creature becomes tapped, you may discard a card. If you do, draw two cards.",
            "Whenever this creature attacks, you may discard two cards. If you do, draw a card.",
            "When this creature enters, you may discard two cards. If you do, draw a card.",
            "Whenever enchanted creature becomes tapped, you may discard a card. If you do, draw a card.",
            "Whenever you tap this creature for a cost, you may discard a card. If you do, draw a card.",
            "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card. You gain 1 life.",
            "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card. This ability triggers only once each turn."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_enters.optional_discard_then_draw"),
        label: "self enters optional discard then draw",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_enters_optional_discard_then_draw,
        calibration: calibrations!(
            "Yuyan Archers" => "When this creature enters, you may discard a card. If you do, draw a card.",
            "Discerning Peddler" => "When this creature enters, you may discard a card. If you do, draw a card.",
            "Rubble Rouser" => "When this creature enters, you may discard a card. If you do, draw a card.";
            "Whenever this creature enters, you may discard a card. If you do, draw a card.",
            "When this creature enters, discard a card. If you do, draw a card.",
            "When this creature enters, you may draw a card, then discard a card.",
            "When this creature enters, you may discard two cards. If you do, draw a card.",
            "When this creature enters, you may discard a card. If you do, draw two cards.",
            "When this creature enters or attacks, you may discard a card. If you do, draw a card.",
            "When this artifact enters, you may discard a card. If you do, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.controller_end_step.draw.one"),
        label: "controller end step draw one",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_controller_end_step_draw_one,
        calibration: calibrations!(
            "The Arkenstone" => "At the beginning of your end step, draw a card.",
            "Roaring Furnace" => "At the beginning of your end step, draw a card.";
            "At the beginning of your end step, draw two cards.",
            "At the beginning of each player's end step, draw a card.",
            "At the beginning of your upkeep, draw a card.",
            "At the beginning of your end step, you may draw a card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.etb.return_creature_card_from_graveyard.hand"),
        label: "creature ETB return target creature card to hand",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_return_creature_card_to_hand,
        calibration: singleton_calibrations!(
            "Vampire Soulcaller" => "When this creature enters, return target creature card from your graveyard to your hand.";
            "When this creature enters, return up to one target creature card from your graveyard to your hand.",
            "When this creature enters, return target card from your graveyard to your hand.",
            "When this creature enters, return target creature card from an opponent's graveyard to your hand.",
            "When this creature dies, return target creature card from your graveyard to your hand."
        ),
    },
    Recipe {
        id: RecipeId("triggered.etb.return_instant_or_sorcery_card_from_graveyard.hand"),
        label: "creature ETB return target instant or sorcery card to hand",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_etb_return_instant_or_sorcery_card_to_hand,
        calibration: calibrations!(
            "Shipwreck Dowser" => "When this creature enters, return target instant or sorcery card from your graveyard to your hand.",
            "Zealous Lorecaster" => "When this creature enters, return target instant or sorcery card from your graveyard to your hand.",
            "Salvager of Secrets" => "When this creature enters, return target instant or sorcery card from your graveyard to your hand.";
            "When this creature enters, return target creature card from a graveyard to your hand.",
            "When this creature enters, return target permanent card from your graveyard to your hand.",
            "When this creature enters, return target card from your graveyard to your hand.",
            "When this creature enters, return target instant or sorcery card from a graveyard to your hand.",
            "When this creature enters, return target instant or sorcery card from an opponent's graveyard to your hand.",
            "When this creature enters, return target instant or sorcery card from your graveyard to the top of your library.",
            "When this creature enters, return target instant or sorcery card from your graveyard to the battlefield.",
            "When this creature enters, return target instant or sorcery card from your graveyard to exile.",
            "When this creature enters, return up to one target instant or sorcery card from your graveyard to your hand.",
            "When this creature enters, choose an instant or sorcery card in your graveyard, then return it to your hand.",
            "When this creature enters, you may return target instant or sorcery card from your graveyard to your hand.",
            "When this creature enters, return two target instant or sorcery cards from your graveyard to your hand.",
            "When this creature enters, return target instant or sorcery card from your graveyard to your hand, then draw a card.",
            "When this creature enters, return target instant or sorcery card from your graveyard to your hand. You gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("triggered.controller_creature_enters.damage.each_opponent.one"),
        label: "controller creature enters damage each opponent one",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_controller_creature_enters_damage_each_opponent,
        calibration: calibrations!(
            "Warleader's Call" => "Whenever a creature you control enters, this enchantment deals 1 damage to each opponent.",
            "Impact Tremors" => "Whenever a creature you control enters, this enchantment deals 1 damage to each opponent.";
            "Whenever a creature you control enters, this enchantment deals 2 damage to each opponent.",
            "Whenever a creature enters, this enchantment deals 1 damage to each opponent.",
            "Whenever a creature you control enters, this enchantment deals 1 damage to each player.",
            "Whenever another creature you control enters, this enchantment deals 1 damage to each opponent."
        ),
    },
    Recipe {
        id: RecipeId("spell.search.legendary_creature.hand"),
        label: "search library for legendary creature to hand",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_search_legendary_creature,
        calibration: singleton_calibrations!(
            "Seek the Heart" => "Search your library for a legendary creature card, reveal it, put it into your hand, then shuffle.";
            "Search your library for a legendary creature card, reveal it, then shuffle.",
            "Search your library for a legendary permanent card, reveal it, put it into your hand, then shuffle.",
            "Search your library for a creature card, reveal it, put it into your hand, then shuffle.",
            "Search your library for a legendary creature card, put it into your hand, then shuffle."
        ),
    },
    Recipe {
        id: RecipeId("etb.artifact.search.legendary_creature.hand"),
        label: "artifact ETB search legendary creature to hand",
        surface: RecipeSurface::EtbAbility,
        matcher: match_artifact_etb_search_legendary_creature,
        calibration: singleton_calibrations!(
            "The Seriema" => "When The Seriema enters, search your library for a legendary creature card, reveal it, put it into your hand, then shuffle.";
            "When The Seriema enters, search your library for a legendary creature card, put it into your hand, then shuffle.",
            "When The Seriema enters, search your library for a creature card, reveal it, put it into your hand, then shuffle.",
            "When The Seriema enters, search your library for a legendary creature card, reveal it, then shuffle.",
            "When another artifact enters, search your library for a legendary creature card, reveal it, put it into your hand, then shuffle."
        ),
    },
    Recipe {
        id: RecipeId("spell.create_tokens.rat_black_one_one.cant_block.two"),
        label: "create two black Rat tokens that cannot block",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_create_two_rat_tokens_cant_block,
        calibration: singleton_calibrations!(
            "Pest Problem" => "Create two 1/1 black Rat creature tokens with \"This token can't block.\"";
            "Create two 1/1 black Rat creature tokens.",
            "Create two tapped 1/1 black Rat creature tokens with \"This token can't block.\"",
            "Create two 1/1 black Rat creature tokens with \"This creature can't block.\"",
            "Create a 1/1 black Rat creature token with \"This token can't block.\""
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

fn surface_applies(surface: RecipeSurface, is_spell: bool, context: &RecipeContext) -> bool {
    match surface {
        RecipeSurface::KeywordClause => true,
        RecipeSurface::SpellClause => is_spell,
        RecipeSurface::AuraSpellClause => context.source_is_aura,
        RecipeSurface::ModalAssembly | RecipeSurface::ModalMode => false,
        RecipeSurface::ZoneActivatedAbility
        | RecipeSurface::SpellStaticAbility
        | RecipeSurface::CharacteristicAbility => true,
        RecipeSurface::EtbAbility
        | RecipeSurface::TriggeredAbility
        | RecipeSurface::ActivatedAbility
        | RecipeSurface::StaticAbility => !is_spell,
    }
}

fn match_surface_in(
    catalog: &[Recipe],
    input: &str,
    surface: RecipeSurface,
    context: &RecipeContext,
) -> Result<Option<RecipeMatch>, RecipeAmbiguity> {
    let mut matches = catalog
        .iter()
        .filter(|recipe| recipe.surface == surface)
        .filter_map(|recipe| {
            (recipe.matcher)(input, context).map(|emission| RecipeMatch {
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

pub(super) fn match_modal_assembly(
    oracle_text: &str,
    context: &RecipeContext,
) -> Result<Option<RecipeMatch>, RecipeAmbiguity> {
    match_surface_in(CATALOG, oracle_text, RecipeSurface::ModalAssembly, context)
}

pub(super) fn match_modal_mode(
    mode_text: &str,
    context: &RecipeContext,
) -> Result<Option<RecipeMatch>, RecipeAmbiguity> {
    match_surface_in(CATALOG, mode_text, RecipeSurface::ModalMode, context)
}

pub(super) fn reviewed_modal_mode_pair(
    mode_ids: &[RecipeId],
    min_modes: u32,
    max_modes: u32,
) -> bool {
    let ids = mode_ids.iter().map(|id| id.as_str()).collect::<Vec<_>>();
    let expected_bounds = match ids.as_slice() {
        ["modal_mode.damage.creature.three.source", "modal_mode.destroy.artifact"] => Some((1, 1)),
        ["modal_mode.pump.team.plus_one_plus_one", "modal_mode.grant.team.hexproof"] => {
            Some((1, 1))
        }
        ["modal_mode.pump.team.plus_two_power", "modal_mode.create_tokens.goblin_red_one_one.two"] => {
            Some((1, 1))
        }
        ["modal_mode.pump.creature.plus_three_plus_three", "modal_mode.destroy.creature.flying"] => {
            Some((1, 1))
        }
        ["modal_mode.counter.spell.unrestricted", "modal_mode.surveil_two.draw_two"] => {
            Some((1, 1))
        }
        ["modal_mode.counter.spell.unless_four", "modal_mode.draw_two.discard_one"] => Some((1, 1)),
        ["modal_mode.damage.creature.equal_power.controlled_to_opponent", "modal_mode.destroy.artifact"] => {
            Some((1, 1))
        }
        ["modal_mode.damage.each_opponent_creature.one.source", "modal_mode.damage.creature.four.source"] => {
            Some((1, 1))
        }
        ["modal_mode.destroy.artifact_or_enchantment", "modal_mode.put_counter.creature.plus_one_plus_one.indestructible"] => {
            Some((1, 1))
        }
        ["modal_mode.grant.indestructible.artifact_or_creature", "modal_mode.damage.tapped_creature.two.source"] => {
            Some((1, 1))
        }
        ["modal_mode.target_player.draw_two.lose_two", "modal_mode.pump.creature.plus_two_plus_two.lifelink"] => {
            Some((1, 1))
        }
        ["modal_mode.discard.target_opponent.two", "modal_mode.pump.opponents.creatures.minus_one_minus_one"] => {
            Some((1, 1))
        }
        ["modal_mode.discard.target_player.two", "modal_mode.target_player.draw_two.lose_two"] => {
            Some((1, 1))
        }
        ["modal_mode.grant.indestructible.creature", "modal_mode.destroy.creature.toughness_at_least_four"] => {
            Some((1, 1))
        }
        ["modal_mode.destroy.creature.flying", "modal_mode.put_counter.creature.plus_one_plus_one.trample_hexproof"] => {
            Some((1, 1))
        }
        ["modal_mode.pump.creature.minus_one_minus_one", "modal_mode.put_counter.creature.plus_one_plus_one"] => {
            Some((1, 2))
        }
        ["modal_mode.damage.creature.three.source", "modal_mode.destroy.noncreature_artifact"] => {
            Some((1, 2))
        }
        _ => None,
    };
    expected_bounds == Some((min_modes, max_modes))
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
        .filter(|recipe| surface_applies(recipe.surface, is_spell, context))
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
    let context_for = |source_name: &str| -> Result<RecipeContext, String> {
        Ok(RecipeContext {
            source_name: source_name.into(),
            triggered_ability_id: AbilityId::new("triggered_01")?,
            activated_ability_id: AbilityId::new("activated_01")?,
            static_ability_id: AbilityId::new("static_01")?,
            characteristic_ability_id: AbilityId::new("characteristic_01")?,
            presentation: AbilityPresentation::OracleLines(vec![1]),
            source_is_artifact: true,
            source_is_land: true,
            source_is_creature: true,
            source_is_vehicle: true,
            source_is_aura: true,
            source_is_equipment: true,
            source_is_enchantment: true,
            source_is_instant: true,
            source_is_sorcery: true,
        })
    };
    let mut ids = std::collections::BTreeSet::new();
    for recipe in catalog {
        if !ids.insert(recipe.id) {
            return Err(format!("duplicate recipe id {}", recipe.id.as_str()));
        }
        let minimum_positive_cards = recipe.calibration.minimum_positive_cards;
        let positive_card_suffix = if minimum_positive_cards == 1 { "" } else { "s" };
        if minimum_positive_cards == 0
            || recipe.calibration.positive_cards.len() < minimum_positive_cards
        {
            return Err(format!(
                "{} needs at least {minimum_positive_cards} positive calibration card{positive_card_suffix}",
                recipe.id.as_str()
            ));
        }
        let names = recipe
            .calibration
            .positive_cards
            .iter()
            .map(|card| card.name)
            .collect::<std::collections::BTreeSet<_>>();
        let distinct_card_suffix = if minimum_positive_cards == 1 { "" } else { "s" };
        if names.len() < minimum_positive_cards {
            return Err(format!(
                "{} needs at least {minimum_positive_cards} distinct named calibration card{distinct_card_suffix}",
                recipe.id.as_str()
            ));
        }
        if recipe.calibration.negative_near_misses.is_empty() {
            return Err(format!(
                "{} needs reviewed negative near-misses",
                recipe.id.as_str()
            ));
        }

        for positive in recipe.calibration.positive_cards {
            let context = context_for(positive.name)?;
            if (recipe.matcher)(positive.clause, &context).is_none() {
                return Err(format!(
                    "{} positive calibration {} did not match its recipe",
                    recipe.id.as_str(),
                    positive.name
                ));
            }
            let matched = match recipe.surface {
                RecipeSurface::ModalAssembly | RecipeSurface::ModalMode => {
                    match_surface_in(catalog, positive.clause, recipe.surface, &context)
                }
                _ => match_clause_in(
                    catalog,
                    positive.clause,
                    matches!(recipe.surface, RecipeSurface::SpellClause),
                    &context,
                ),
            }
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
            let context = context_for(recipe.calibration.positive_cards[0].name)?;
            if (recipe.matcher)(negative, &context).is_some() {
                return Err(format!(
                    "{} accepted near-miss {negative:?}",
                    recipe.id.as_str()
                ));
            }
            let matched = match recipe.surface {
                RecipeSurface::ModalAssembly | RecipeSurface::ModalMode => {
                    match_surface_in(catalog, negative, recipe.surface, &context)
                }
                _ => match_clause_in(
                    catalog,
                    negative,
                    matches!(recipe.surface, RecipeSurface::SpellClause),
                    &context,
                ),
            };
            match matched {
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
            source_name: "Test Card".into(),
            triggered_ability_id: AbilityId::new("triggered_01").unwrap(),
            activated_ability_id: AbilityId::new("activated_01").unwrap(),
            static_ability_id: AbilityId::new("static_01").unwrap(),
            characteristic_ability_id: AbilityId::new("characteristic_01").unwrap(),
            presentation: AbilityPresentation::OracleLines(vec![1]),
            source_is_artifact: true,
            source_is_land: true,
            source_is_creature: true,
            source_is_vehicle: true,
            source_is_aura: true,
            source_is_equipment: true,
            source_is_enchantment: true,
            source_is_instant: true,
            source_is_sorcery: true,
        }
    }

    #[test]
    fn catalog_has_stable_unique_ids_and_complete_calibration_metadata() {
        validate_catalog().expect("built-in recipe catalog should be valid");
    }

    #[test]
    fn equipment_manifest_dread_attach_is_exact_and_fail_closed() {
        let clause = "When this Equipment enters, manifest dread, then attach this Equipment to that creature.";
        let matched = match_clause(clause, false, &context()).expect("matcher should not error");
        assert!(matches!(
            matched,
            Some(RecipeMatch {
                emission: RecipeEmission::TriggeredAbility(TriggeredAbilityDef {
                    effect,
                    ..
                }),
                ..
            }) if effect == vec![
                SpellEffectKind::ManifestDread,
                SpellEffectKind::AttachEquipment {
                    equipment: EffectSubject::Source,
                    creature: EffectSubject::PreviousEffectObject,
                }
            ]
        ));
        for near_miss in [
            "When this Equipment enters, manifest dread, then attach this Equipment to a creature.",
            "When this Equipment enters, manifest dread, then you may attach this Equipment to that creature.",
            "When this Equipment enters, attach this Equipment to that creature, then manifest dread.",
        ] {
            assert!(match_clause(near_miss, false, &context()).unwrap().is_none());
        }
        let mut non_equipment = context();
        non_equipment.source_is_equipment = false;
        assert!(match_clause(clause, false, &non_equipment)
            .unwrap()
            .is_none());
    }

    #[test]
    fn issue_275_self_enters_optional_discard_then_draw_is_exact() {
        let matched = match_clause(
            "When this creature enters, you may discard a card. If you do, draw a card.",
            false,
            &context(),
        )
        .expect("recipe matching should not be ambiguous")
        .expect("issue 275 ETB recipe should match");
        assert_eq!(
            matched.id.as_str(),
            "triggered.self_enters.optional_discard_then_draw"
        );
        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue 275 recipe must emit a triggered ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::DrawDiscard {
                who: PlayerRecipient::Controller,
                draw_count: 1,
                discard_count: 1,
                order: DrawDiscardOrder::DiscardThenDraw,
                optional: true,
            }]
        );
        for near_miss in [
            "When this creature enters, discard a card. If you do, draw a card.",
            "When this creature enters, you may draw a card, then discard a card.",
            "When this creature enters, you may discard two cards. If you do, draw a card.",
            "When this creature enters, you may discard a card. If you do, draw two cards.",
            "When this creature enters or attacks, you may discard a card. If you do, draw a card.",
        ] {
            assert!(
                match_clause(near_miss, false, &context())
                    .unwrap()
                    .is_none(),
                "matched {near_miss}"
            );
        }
        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(match_clause(
            "When this creature enters, you may discard a card. If you do, draw a card.",
            false,
            &noncreature,
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn issue_282_self_becomes_tapped_loot_templates_are_exact_and_fail_closed() {
        let cases = [
            (
                "Whenever this creature becomes tapped, draw a card, then discard a card.",
                "triggered.self_becomes_tapped.draw_then_discard",
                TriggerCondition::WheneverSelfBecomesTapped,
                DrawDiscardOrder::DrawThenDiscard,
                false,
            ),
            (
                "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card.",
                "triggered.self_becomes_tapped.optional_discard_then_draw",
                TriggerCondition::WheneverSelfBecomesTapped,
                DrawDiscardOrder::DiscardThenDraw,
                true,
            ),
        ];
        for (clause, expected_id, expected_trigger, expected_order, expected_optional) in
            cases.iter()
        {
            let matched = match_clause(clause, false, &context())
                .expect("issue #282 recipe matching should not be ambiguous")
                .unwrap_or_else(|| panic!("issue #282 clause should match: {clause}"));
            assert_eq!(matched.id.as_str(), *expected_id, "{clause}");
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("issue #282 recipe must emit a triggered ability: {clause}");
            };
            assert_eq!(ability.trigger, *expected_trigger, "{clause}");
            assert!(
                !ability.may,
                "optional discard is an effect choice: {clause}"
            );
            assert_eq!(
                ability.effect,
                [SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::Controller,
                    draw_count: 1,
                    discard_count: 1,
                    order: *expected_order,
                    optional: *expected_optional,
                }],
                "{clause}"
            );
        }

        for near_miss in [
            "Whenever this creature becomes tapped, you may draw a card, then discard a card.",
            "Whenever this creature becomes tapped, discard a card, then draw a card.",
            "Whenever this creature becomes tapped, draw two cards, then discard a card.",
            "Whenever this creature becomes tapped, draw a card, then discard two cards.",
            "Whenever this creature attacks, draw a card, then discard a card.",
            "When this creature enters, draw a card, then discard a card.",
            "Whenever enchanted creature becomes tapped, draw a card, then discard a card.",
            "Whenever you tap this creature for a cost, draw a card, then discard a card.",
            "Whenever this creature becomes tapped, draw a card, then discard a card. You gain 1 life.",
            "Whenever this creature becomes tapped, you may discard a card. If you do, draw two cards.",
            "Whenever this creature becomes tapped, discard a card. If you do, draw a card.",
        ] {
            assert!(
                match_self_becomes_tapped_draw_then_discard(near_miss, &context()).is_none()
                    && match_self_becomes_tapped_optional_discard_then_draw(near_miss, &context())
                        .is_none(),
                "new issue #282 recipe matched near-miss {near_miss}"
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        for clause in [
            "Whenever this creature becomes tapped, draw a card, then discard a card.",
            "Whenever this creature becomes tapped, you may discard a card. If you do, draw a card.",
        ] {
            assert!(
                match_clause(clause, false, &noncreature)
                    .expect("source-kind matching should not be ambiguous")
                    .is_none(),
                "noncreature source matched {clause}"
            );
        }
    }

    #[test]
    fn issue_280_increment_is_exact_and_source_scoped() {
        let clause = "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)";
        let matched = match_clause(clause, false, &context())
            .expect("issue #280 recipe matching should not be ambiguous")
            .expect("exact Increment clause should match");
        assert_eq!(matched.id.as_str(), "triggered.increment");
        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #280 recipe must emit a triggered ability");
        };
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter: SpellCastFilter::default(),
                ordinal: None,
                ordinal_scope: Default::default(),
            }
        );
        assert_eq!(
            ability.intervening_if,
            Some(GameCondition::TriggeringSpellManaSpent {
                comparison: SpellManaSpentComparison::GreaterThanSourcePowerOrToughness,
            })
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            }]
        );

        for near_miss in [
            "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put two +1/+1 counters on this creature.)",
            "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power, put a +1/+1 counter on this creature.)",
            "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's toughness, put a +1/+1 counter on this creature.)",
            "Increment (Whenever you cast a spell, if the amount of mana you spent is at least this creature's power or toughness, put a +1/+1 counter on this creature.)",
            "Increment (Whenever you cast a spell, if the spell's mana value is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)",
            "Increment (Whenever you cast a spell, if you spent more mana than this creature's power or toughness, put a +1/+1 counter on this creature.)",
            "Increment (Whenever an opponent casts a spell, if the amount of mana they spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.)",
            "Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.",
            "Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature. Increment",
            "Increment (Whenever you cast a spell, if the amount of mana you spent is greater than this creature's power or toughness, put a +1/+1 counter on this creature.) Whenever this creature attacks, draw a card.",
        ] {
            assert!(
                match_clause(near_miss, false, &context())
                    .expect("near-miss matching should not be ambiguous")
                    .is_none(),
                "issue #280 recipe matched near-miss {near_miss}"
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(match_clause(clause, false, &noncreature)
            .expect("source-kind matching should not be ambiguous")
            .is_none());
    }

    #[test]
    fn issue_277_affinity_for_artifacts_emits_counted_artifact_reduction() {
        let matched = match_clause("Affinity for artifacts", false, &context())
            .expect("affinity-for-artifacts matching should not be ambiguous")
            .expect("issue 277 affinity-for-artifacts recipe should match");
        assert_eq!(
            matched.id.as_str(),
            "static.cost_reduction.affinity_artifacts"
        );
        let RecipeEmission::SpellCostModifier(
            SpellCostModifier::BattlefieldCountGenericReduction {
                amount_per_match,
                filter,
                aggregate,
            },
        ) = matched.emission
        else {
            panic!("issue 277 recipe must emit a battlefield-count spell cost modifier");
        };
        assert_eq!(amount_per_match, 1);
        assert_eq!(filter.controllers, RelativePlayerSet::Controller);
        assert_eq!(filter.card_type, Some(CardTypeFilter::Artifact));
        assert_eq!(aggregate, BattlefieldAggregate::Count);
        for near_miss in [
            "Affinity for creatures",
            "Affinity for artifact",
            "Affinity for artifacts if you control an artifact",
            "Affinity for artifacts. Draw a card.",
        ] {
            assert!(
                match_clause(near_miss, false, &context())
                    .unwrap()
                    .is_none(),
                "matched near-miss: {near_miss}"
            );
        }
    }

    #[test]
    fn raid_etb_draw_is_exact_and_fail_closed() {
        let matched = match_clause(
            "Raid — When this creature enters, if you attacked this turn, draw a card.",
            false,
            &context(),
        )
        .expect("raid matcher should not error")
        .expect("raid ETB draw recipe should match");
        assert_eq!(matched.id.as_str(), "triggered.etb.raid.draw.one");
        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("raid ETB draw must emit a triggered ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.intervening_if,
            Some(GameCondition::AttackedThisTurn {
                players: RelativePlayerSet::Controller,
            })
        );
        assert_eq!(
            ability.effect,
            vec![SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        );
        for near_miss in [
            "Raid — When this creature enters, draw a card.",
            "Raid — When this creature enters, if you attacked this turn, draw two cards.",
            "Raid — When another creature enters, if you attacked this turn, draw a card.",
            "Raid — When this creature enters, if an opponent attacked this turn, draw a card.",
            "Raid — When this creature enters, if you attacked this turn, you may draw a card.",
        ] {
            assert!(
                match_clause(near_miss, false, &context())
                    .unwrap()
                    .is_none(),
                "matched near-miss: {near_miss}"
            );
        }
    }

    #[test]
    fn equipment_target_attach_templates_are_exact_and_fail_closed() {
        let attach = "When this Equipment enters, attach it to target creature you control.";
        let first_strike = "When this Equipment enters, attach it to target creature you control. That creature gains first strike until end of turn.";
        assert!(matches!(
            match_clause(attach, false, &context()).unwrap(),
            Some(RecipeMatch { emission: RecipeEmission::TriggeredAbility(TriggeredAbilityDef { effect, targeting: Some(_), .. }), .. })
                if matches!(&effect[..], [SpellEffectKind::AttachSource { target: TargetFilter { kind: TargetKind::Creature, controller: TargetController::You, .. } }])
        ));
        let Some(RecipeMatch {
            emission:
                RecipeEmission::TriggeredAbility(TriggeredAbilityDef {
                    effect,
                    targeting: Some(_),
                    ..
                }),
            ..
        }) = match_clause(first_strike, false, &context()).unwrap()
        else {
            panic!("first-strike template did not match");
        };
        assert_eq!(effect.len(), 2);
        assert!(matches!(
            effect[0],
            SpellEffectKind::AttachSource {
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..
                }
            }
        ));
        assert!(matches!(
            &effect[1],
            SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(filter), keywords }
                if filter.kind == TargetKind::Creature && filter.controller == TargetController::You && keywords == &[Keyword::FirstStrike]
        ));
        for near_miss in [
            "When this Equipment enters, attach it to target creature.",
            "When this Equipment enters, you may attach it to target creature you control.",
            "When this Equipment enters, attach another Equipment to target creature you control.",
            "When this Equipment enters, attach it to target creature you control. Draw a card.",
            "When this Equipment enters, attach it to target creature you control. That creature gains double strike until end of turn.",
        ] {
            assert!(match_clause(near_miss, false, &context()).unwrap().is_none(), "matched near-miss: {near_miss}");
        }
        let mut non_equipment = context();
        non_equipment.source_is_equipment = false;
        assert!(match_clause(attach, false, &non_equipment)
            .unwrap()
            .is_none());
        assert!(match_clause(first_strike, false, &non_equipment)
            .unwrap()
            .is_none());
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
                minimum_positive_cards: 2,
            },
        }];
        assert_eq!(
            validate_catalog_in(&incomplete),
            Err("synthetic.incomplete needs at least 2 positive calibration cards".into())
        );
    }

    #[test]
    fn calibration_error_reports_configured_minimum() {
        let incomplete = [Recipe {
            id: RecipeId("synthetic.singleton"),
            label: "synthetic singleton",
            surface: RecipeSurface::KeywordClause,
            matcher: duplicate_matcher,
            calibration: RecipeCalibration {
                positive_cards: &[],
                negative_near_misses: &["No"],
                minimum_positive_cards: 1,
            },
        }];
        assert_eq!(
            validate_catalog_in(&incomplete),
            Err("synthetic.singleton needs at least 1 positive calibration card".into())
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
        for (clause, _) in &cases[..4] {
            assert!(
                match_clause(clause, false, &nonland).unwrap().is_none(),
                "{clause}"
            );
        }
        assert!(match_clause(cases[4].0, false, &nonland)
            .expect("paid any-color recipe must remain unambiguous")
            .is_some());
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
    fn issue_263_creature_trigger_clauses_emit_exact_typed_abilities() {
        let cases = [
            (
                "When this creature enters, return up to one other target permanent you control to its owner's hand.",
                "etb.return_to_hand.other_permanent_you_control.up_to_one",
            ),
            (
                "When this creature enters, you may mill two cards.",
                "etb.mill.controller.two.may",
            ),
            (
                "Whenever you cast a noncreature spell, put a +1/+1 counter on this creature.",
                "triggered.controller_casts.noncreature.put_counter.plus_one_plus_one.source.one",
            ),
            (
                "Whenever you draw your second card each turn, put a +1/+1 counter on this creature.",
                "triggered.controller_draws.second.put_counter.plus_one_plus_one.source.one",
            ),
            (
                "Whenever this creature attacks, you gain 2 life.",
                "triggered.self_attacks.gain_life.controller.two",
            ),
            (
                "Whenever this creature attacks, mill a card.",
                "triggered.self_attacks.mill.controller.one",
            ),
        ];

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        noncreature.source_is_enchantment = false;
        for (clause, _) in cases {
            assert!(
                match_clause(clause, false, &noncreature).unwrap().is_none(),
                "issue #263 creature recipe must reject a noncreature source: {clause}"
            );
        }

        let abilities = cases.map(|(clause, expected_id)| {
            let matched = match_clause(clause, false, &context())
                .expect("issue #263 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #263 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("issue #263 recipe must emit a triggered ability: {clause}");
            };
            assert_eq!(ability.ability_id.as_str(), "triggered_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            ability
        });

        assert_eq!(
            abilities[0].trigger,
            TriggerCondition::WhenSelfEntersBattlefield
        );
        assert!(!abilities[0].may);
        assert_eq!(
            abilities[0].effect,
            [SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    controller: TargetController::You,
                    excluded_objects: vec![TargetObjectExclusion::Source],
                    ..TargetFilter::default()
                })),
            }]
        );
        let target_group = &abilities[0]
            .targeting
            .as_ref()
            .expect("optional bounce targeting")
            .groups[0];
        assert_eq!((target_group.min, target_group.max), (0, 1));
        assert_eq!(target_group.effect_indices, [0]);

        assert_eq!(
            abilities[1].trigger,
            TriggerCondition::WhenSelfEntersBattlefield
        );
        assert!(abilities[1].may);
        assert_eq!(
            abilities[1].effect,
            [SpellEffectKind::Mill {
                count: Amount::Fixed(2),
                who: PlayerRecipient::Controller,
            }]
        );

        assert_eq!(
            abilities[2].trigger,
            TriggerCondition::WheneverPlayerCastsSpell {
                caster: CastTriggerPlayer::Controller,
                filter: SpellCastFilter {
                    card_type: Some(CardTypeFilter::Noncreature),
                    ..SpellCastFilter::default()
                },
                ordinal: None,
                ordinal_scope: Default::default(),
            }
        );
        assert_eq!(
            abilities[3].trigger,
            TriggerCondition::WheneverPlayerDrawsNthCard {
                drawer: CastTriggerPlayer::Controller,
                ordinal: 2,
            }
        );
        for ability in &abilities[2..=3] {
            assert_eq!(
                ability.effect,
                [SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }]
            );
            assert!(!ability.may);
        }

        for ability in &abilities[4..=5] {
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0,
                }
            );
            assert!(!ability.may);
        }
        assert_eq!(
            abilities[4].effect,
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),
            }]
        );
        assert_eq!(
            abilities[5].effect,
            [SpellEffectKind::Mill {
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
            }]
        );
    }

    #[test]
    fn issue_264_activated_templates_emit_exact_typed_abilities() {
        let cases = [
            (
                "{T}, Sacrifice this land: Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
                "activated.land.tap_sacrifice.search_basic_land.battlefield_tapped",
            ),
            (
                "{1}: Add one mana of any color. Activate only once each turn.",
                "activated.mana.pay_one.any_color.per_turn_one",
            ),
            (
                "{1}{B}: This creature gets +1/+1 until end of turn.",
                "activated.pump.self.plus_1_plus_1.pay_1b",
            ),
            (
                "{2}{G}: This creature gets +2/+2 until end of turn. Activate only once each turn.",
                "activated.pump.self.plus_2_plus_2.pay_2g.per_turn_one",
            ),
            (
                "{3}{W}: Creatures you control get +1/+1 until end of turn.",
                "activated.pump.creatures_you_control.plus_1_plus_1.fixed_cost",
            ),
            (
                "{4}{W}: Creatures you control get +1/+1 until end of turn.",
                "activated.pump.creatures_you_control.plus_1_plus_1.fixed_cost",
            ),
            (
                "{5}: Creatures you control get +1/+1 until end of turn.",
                "activated.pump.creatures_you_control.plus_1_plus_1.fixed_cost",
            ),
        ];

        let abilities = cases.map(|(clause, expected_id)| {
            let matched = match_clause(clause, false, &context())
                .expect("issue #264 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #264 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("issue #264 recipe must emit an activated ability: {clause}");
            };
            assert_eq!(ability.ability_id.as_str(), "activated_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert!(ability.targeting.is_none());
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert!(ability.conditions.is_empty());
            ability
        });

        assert_eq!(
            abilities[0].costs,
            [AbilityCost::Tap, AbilityCost::SacrificeSelf]
        );
        assert!(matches!(
            abilities[0].effect.as_slice(),
            [SpellEffectKind::SearchLibrary {
                who: PlayerRecipient::Controller,
                optional: false,
                count: 1,
                filter: Some(ZoneCardFilter {
                    card_type: Some(CardTypeFilter::BasicLand),
                    ..
                }),
                destination: SearchDestination::Battlefield { tapped: true },
                shuffle: true,
                reveal: false,
                ..
            }]
        ));

        assert_eq!(
            abilities[1].costs,
            [AbilityCost::Mana(ManaCost::parse("{1}").unwrap())]
        );
        assert_eq!(abilities[1].mana_options().unwrap().len(), 5);
        assert_eq!(
            abilities[1].activation_limit,
            Some(tricerules_cards::primitives::ActivationLimit::PerTurn { max_activations: 1 })
        );

        assert_eq!(
            abilities[2].effect,
            [SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 1,
                scale: None,
                subject: EffectSubject::Source,
            }]
        );
        assert_eq!(abilities[2].activation_limit, None);
        assert_eq!(
            abilities[3].effect,
            [SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 2,
                scale: None,
                subject: EffectSubject::Source,
            }]
        );
        assert_eq!(
            abilities[3].activation_limit,
            Some(tricerules_cards::primitives::ActivationLimit::PerTurn { max_activations: 1 })
        );

        for ability in &abilities[4..] {
            assert_eq!(
                ability.effect,
                [SpellEffectKind::PumpAll {
                    filter: creatures_you_control(),
                    power: 1,
                    toughness: 1,
                }]
            );
            assert_eq!(ability.activation_limit, None);
        }

        let mut nonland = context();
        nonland.source_is_land = false;
        assert!(match_clause(cases[0].0, false, &nonland).unwrap().is_none());
        let mut noncreature = context();
        noncreature.source_is_creature = false;
        for (clause, _) in &cases[1..] {
            assert!(
                match_clause(clause, false, &noncreature).unwrap().is_none(),
                "issue #264 creature recipe must reject a noncreature source: {clause}"
            );
        }
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

    #[test]
    fn issue_266_permanent_trigger_templates_have_stable_exact_recipe_ids() {
        let cases = [
            (
                "Whenever you gain life, put a +1/+1 counter on this creature.",
                "triggered.controller_gains_life.put_counter.plus_one_plus_one.source.one",
            ),
            (
                "Whenever you gain life, each opponent loses 1 life.",
                "triggered.controller_gains_life.each_opponent_loses_one",
            ),
            (
                "When this creature enters or dies, surveil 1.",
                "triggered.self_enters_or_dies.surveil.one",
            ),
            (
                "When this creature enters, target creature an opponent controls gets -2/-0 until end of turn.",
                "etb.pump.opponent_creature.minus_two_zero",
            ),
            (
                "When this creature enters, target opponent discards a card.",
                "etb.discard.target_opponent.one",
            ),
            (
                "When this creature enters, create a Clue token.",
                "etb.create_token.clue.one",
            ),
            (
                "When this creature enters, investigate.",
                "etb.create_token.clue.one",
            ),
            (
                "When this Aura enters, draw a card.",
                "etb.aura.draw.one",
            ),
            (
                "When this creature enters, create a 1/1 white Soldier creature token.",
                "etb.create_token.printed_one_one.one",
            ),
            (
                "When this creature enters, create a 1/1 white and blue Merfolk creature token.",
                "etb.create_token.printed_one_one.one",
            ),
            (
                "When this creature enters, it deals 1 damage to any target.",
                "etb.damage.any_target.one.source",
            ),
            (
                "When this creature enters, put a +1/+1 counter on another target creature you control.",
                "etb.put_counter.plus_one_plus_one.other_creature_you_control.one",
            ),
            (
                "When this creature enters, return up to one other target creature to its owner's hand.",
                "etb.return_to_hand.other_creature.up_to_one",
            ),
        ];

        for (clause, expected_id) in cases {
            let matched = match_clause(clause, false, &context())
                .expect("issue #266 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #266 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
        }
    }

    #[test]
    fn issue_266_permanent_trigger_templates_require_the_printed_source_kind() {
        let mut noncreature = context();
        noncreature.source_is_creature = false;
        for clause in [
            "Whenever you gain life, put a +1/+1 counter on this creature.",
            "Whenever you gain life, each opponent loses 1 life.",
            "When this creature enters or dies, surveil 1.",
            "When this creature enters, target creature an opponent controls gets -2/-0 until end of turn.",
            "When this creature enters, target opponent discards a card.",
            "When this creature enters, create a Clue token.",
            "When this creature enters, investigate.",
            "When this creature enters, create a 1/1 white Soldier creature token.",
            "When this creature enters, it deals 1 damage to any target.",
            "When this creature enters, put a +1/+1 counter on another target creature you control.",
            "When this creature enters, return up to one other target creature to its owner's hand.",
        ] {
            assert_eq!(match_clause(clause, false, &noncreature), Ok(None), "{clause}");
        }

        let mut non_aura = context();
        non_aura.source_is_aura = false;
        assert_eq!(
            match_clause("When this Aura enters, draw a card.", false, &non_aura),
            Ok(None)
        );
    }

    #[test]
    fn issue_262_combat_trick_templates_have_stable_exact_recipe_ids() {
        let cases = [
            (
                "Target creature gets +3/+3 and gains trample until end of turn.",
                "spell.pump_grant.creature.plus_3_plus_3.trample",
            ),
            (
                "Target creature you control gets +0/+3 and gains hexproof until end of turn.",
                "spell.pump_grant.creature_you_control.plus_0_plus_3.hexproof",
            ),
            (
                "Target creature you control gets +1/+1 and gains hexproof until end of turn. Untap it.",
                "spell.pump_grant_untap.creature_you_control.plus_1_plus_1.hexproof",
            ),
            (
                "Target creature gets +1/+3 and gains reach until end of turn. Untap it.",
                "spell.pump_grant_untap.creature.plus_1_plus_3.reach",
            ),
            (
                "Target creature gets -3/-3 until end of turn.",
                "spell.pump.creature.minus_3_minus_3",
            ),
            (
                "Target creature gains deathtouch and indestructible until end of turn.",
                "spell.grant.creature.deathtouch_indestructible",
            ),
            (
                "Target creature gets +3/+0 until end of turn.\nDraw a card.",
                "spell.pump_then_draw.creature.plus_3_plus_0",
            ),
        ];

        for (clause, expected_id) in cases {
            let matched = match_clause(clause, true, &context())
                .expect("issue #262 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #262 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
        }
    }

    #[test]
    fn issue_268_one_shot_spell_templates_have_stable_exact_recipe_ids() {
        let cases = [
            (
                "Target creature you control gains deathtouch and indestructible until end of turn.",
                "spell.grant.creature.deathtouch_indestructible",
            ),
            (
                "Draw three cards, then discard a card.",
                "spell.draw_discard.draw_three.discard_one",
            ),
            (
                "Target creature you control gets +1/+0 until end of turn. It deals damage equal to its power to up to one target creature an opponent controls.",
                "spell.creature_power_damage.controlled_plus_one_to_opponent",
            ),
            (
                "Return target card from your graveyard to your hand.",
                "spell.return_graveyard_card.hand",
            ),
            (
                "Target creature gets +4/+4 and gains trample until end of turn.",
                "spell.pump_grant.creature.plus_4_plus_4.trample",
            ),
            (
                "Boltwave deals 3 damage to each opponent.",
                "spell.damage_player.each_opponent.fixed_three.source",
            ),
            (
                "Bombard deals 4 damage to target creature.",
                "spell.damage.creature.fixed_four.source",
            ),
            (
                "Destroy target artifact or enchantment.",
                "spell.destroy.artifact_or_enchantment",
            ),
            (
                "Destroy target creature or planeswalker.",
                "spell.destroy.creature_or_planeswalker",
            ),
            (
                "Creatures you control get +3/+3 and gain trample until end of turn.",
                "spell.pump_all.creatures_you_control.plus_3_plus_3.trample",
            ),
            (
                "Destroy target creature. You gain 2 life.",
                "spell.destroy.creature_then_gain_life.two",
            ),
            (
                "Target creature gets -2/-2 until end of turn.",
                "spell.pump.creature.minus_2_minus_2",
            ),
            (
                "Destroy target attacking or blocking creature.",
                "spell.destroy.attacking_or_blocking_creature",
            ),
            (
                "Exile target creature.",
                "spell.exile.creature",
            ),
        ];

        for (clause, expected_id) in cases {
            let mut test_context = context();
            if clause.starts_with("Boltwave ") {
                test_context.source_name = "Boltwave".into();
            } else if clause.starts_with("Bombard ") {
                test_context.source_name = "Bombard".into();
            }
            let matched = match_clause(clause, true, &test_context)
                .expect("issue #268 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #268 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
        }
    }

    #[test]
    fn issue_271_templates_have_stable_exact_recipe_ids() {
        validate_catalog().expect("issue #271 calibrations and near-misses must remain exact");
        let spell_cases = [
            (
                "This spell costs {3} less to cast if it targets a tapped creature.",
                "spell.cost_reduction.target_tapped_creature.three",
            ),
            (
                "Surveil 2, then draw two cards. You lose 2 life.",
                "spell.surveil_two.draw_two.lose_two",
            ),
            (
                "Return target nonland permanent to its owner's hand. Surveil 1.",
                "spell.return_nonland_permanent_then_surveil_one",
            ),
            (
                "Target creature you control deals damage equal to its power to target creature an opponent controls.",
                "spell.damage.creature.equal_power.controlled_to_opponent",
            ),
        ];
        for (clause, expected_id) in spell_cases {
            let matched = match_clause(clause, true, &context())
                .expect("issue #271 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #271 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
        }

        let matched = match_clause(
            "When this enchantment enters, exile up to one target nonland permanent an opponent controls until this enchantment leaves the battlefield. You gain 2 life.",
            false,
            &context(),
        )
        .expect("issue #271 ETB clause must not be ambiguous")
        .expect("issue #271 ETB clause must be supported");
        assert_eq!(
            matched.id.as_str(),
            "etb.enchantment.exile_opponent_nonland_up_to_one_until_source_leaves.gain_life_two"
        );

        let mut wrong_spell_type = context();
        wrong_spell_type.source_is_instant = false;
        assert!(match_clause(
            "This spell costs {3} less to cast if it targets a tapped creature.",
            true,
            &wrong_spell_type,
        )
        .unwrap()
        .is_none());
        assert!(match_clause(
            "Return target nonland permanent to its owner's hand. Surveil 1.",
            true,
            &wrong_spell_type,
        )
        .unwrap()
        .is_none());
        assert!(match_clause(
            "Target creature you control deals damage equal to its power to target creature an opponent controls.",
            true,
            &wrong_spell_type,
        )
        .unwrap()
        .is_none());

        let mut instant_not_sorcery = context();
        instant_not_sorcery.source_is_sorcery = false;
        assert!(match_clause(
            "Surveil 2, then draw two cards. You lose 2 life.",
            true,
            &instant_not_sorcery,
        )
        .unwrap()
        .is_none());

        let mut nonenchantment = context();
        nonenchantment.source_is_enchantment = false;
        assert!(match_clause(
            "When this enchantment enters, exile up to one target nonland permanent an opponent controls until this enchantment leaves the battlefield. You gain 2 life.",
            false,
            &nonenchantment,
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn issue_273_enchantment_linked_exile_template_is_supported() {
        let clause = "When this enchantment enters, exile target nonland permanent an opponent controls until this enchantment leaves the battlefield.";
        let matched = match_clause(clause, false, &context())
            .expect("issue #273 ETB clause must not be ambiguous")
            .expect("issue #273 ETB clause must be supported");
        assert_eq!(
            matched.id.as_str(),
            "etb.enchantment.exile_opponent_nonland_until_source_leaves"
        );
        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #273 ETB clause must emit a triggered ability");
        };
        assert_eq!(ability.effect.len(), 1);
        let [SpellEffectKind::ExileUntilSourceLeaves { target }] = ability.effect.as_slice() else {
            panic!("issue #273 must emit linked exile");
        };
        assert_eq!(target.kind, TargetKind::AnyPermanent);
        assert_eq!(target.controller, TargetController::Opponent);
        assert_eq!(target.excluded_permanent_types, [PermanentTypeFilter::Land]);
        let groups = &ability.targeting.as_ref().expect("mandatory target").groups;
        assert_eq!((groups[0].min, groups[0].max), (1, 1));
        assert_eq!(groups[0].effect_indices, [0]);

        let mut nonenchantment = context();
        nonenchantment.source_is_enchantment = false;
        assert_eq!(match_clause(clause, false, &nonenchantment), Ok(None));
        for near_miss in [
            "When this enchantment enters, exile target tapped nonland permanent an opponent controls until this enchantment leaves the battlefield.",
            "When this enchantment enters, exile target nonland permanent with mana value 4 or less an opponent controls until this enchantment leaves the battlefield.",
        ] {
            assert_eq!(match_clause(near_miss, false, &context()), Ok(None), "{near_miss}");
        }
    }

    #[test]
    fn issue_261_attachment_templates_have_stable_ids_and_subtype_gates() {
        let cases = [
            ("Enchant creature", "aura.enchant.creature"),
            ("Equip {2}", "activated.equip.generic_fixed"),
            (
                "Equipped creature gets +1/+1 and has reach and vigilance.",
                "static.attached_modifier.creature.fixed",
            ),
            (
                "When this Aura enters, tap enchanted creature.",
                "etb.aura.tap_attached_creature",
            ),
            (
                "Enchanted creature doesn't untap during its controller's untap step.",
                "static.aura.attached_creature_untap_step",
            ),
        ];
        for (clause, expected_id) in cases {
            let matched = match_clause(clause, false, &context())
                .expect("issue #261 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #261 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
        }

        let mut ordinary_enchantment = context();
        ordinary_enchantment.source_is_aura = false;
        ordinary_enchantment.source_is_equipment = false;
        for clause in [
            "Enchant creature",
            "When this Aura enters, tap enchanted creature.",
            "Enchanted creature gets +2/+2.",
            "Enchanted creature doesn't untap during its controller's untap step.",
            "Equip {2}",
            "Equipped creature has double strike.",
        ] {
            assert_eq!(
                match_clause(clause, false, &ordinary_enchantment),
                Ok(None),
                "{clause} must require its source subtype"
            );
        }
    }

    #[test]
    fn issue_267_static_templates_have_stable_exact_recipe_ids() {
        let cases = [
            (
                "During your turn, this creature has first strike.",
                "static.conditional_self.first_strike.controller_turn",
            ),
            (
                "This creature can't block.",
                "static.self_combat_restriction.cant_block",
            ),
            (
                "Creatures you control get +1/+1.",
                "static.anthem_pt.creatures_you_control.plus_one_plus_one",
            ),
            (
                "This creature enters tapped.",
                "static.enters_tapped.creature.unconditional",
            ),
            (
                "You may play an additional land on each of your turns.",
                "static.extra_land_plays.one",
            ),
            (
                "You may play lands from your graveyard.",
                "static.play_lands_from_own_graveyard",
            ),
            (
                "During your turn, creatures you control with +1/+1 counters on them have first strike.",
                "static.anthem_keyword.countered_creatures.first_strike.controller_turn",
            ),
            (
                "At the beginning of your end step, draw a card.",
                "triggered.controller_end_step.draw.one",
            ),
        ];

        for (clause, expected_id) in cases {
            let matched = match_clause(clause, false, &context())
                .expect("issue #267 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #267 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
        }
    }

    #[test]
    fn issue_267_companion_templates_emit_typed_effects_and_exact_targeting() {
        let cases = [
            (
                "Landfall — Whenever a land you control enters, mill a card.",
                false,
                "triggered.landfall.mill.one",
            ),
            (
                "Whenever this creature attacks, you may discard a card. If you do, draw a card.",
                false,
                "triggered.self_attacks.optional_discard_then_draw",
            ),
            (
                "When this creature enters, return target creature card from your graveyard to your hand.",
                false,
                "triggered.etb.return_creature_card_from_graveyard.hand",
            ),
            (
                "Whenever a creature you control enters, this enchantment deals 1 damage to each opponent.",
                false,
                "triggered.controller_creature_enters.damage.each_opponent.one",
            ),
            (
                "Landfall — Whenever a land you control enters, this creature deals 1 damage to each opponent.",
                false,
                "triggered.landfall.damage.each_opponent.one",
            ),
            (
                "At the beginning of your end step, draw a card.",
                false,
                "triggered.controller_end_step.draw.one",
            ),
            (
                "Search your library for a legendary creature card, reveal it, put it into your hand, then shuffle.",
                true,
                "spell.search.legendary_creature.hand",
            ),
            (
                "Create two 1/1 black Rat creature tokens with \"This token can't block.\"",
                true,
                "spell.create_tokens.rat_black_one_one.cant_block.two",
            ),
        ];

        for (clause, is_spell, expected_id) in cases {
            let matched = match_clause(clause, is_spell, &context())
                .expect("issue #267 companion clause must not be ambiguous")
                .unwrap_or_else(|| {
                    panic!("issue #267 companion clause must be supported: {clause}")
                });
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
        }

        let mut seriema = context();
        seriema.source_name = "The Seriema".into();
        let matched = match_clause(
            "When The Seriema enters, search your library for a legendary creature card, reveal it, put it into your hand, then shuffle.",
            false,
            &seriema,
        )
        .expect("artifact ETB search must not be ambiguous")
        .expect("artifact ETB search must be supported");
        assert_eq!(
            matched.id.as_str(),
            "etb.artifact.search.legendary_creature.hand"
        );

        let RecipeEmission::TriggeredAbility(attack) = match_clause(
            "Whenever this creature attacks, you may discard a card. If you do, draw a card.",
            false,
            &context(),
        )
        .unwrap()
        .unwrap()
        .emission
        else {
            panic!("optional attack recipe must emit a trigger");
        };
        assert_eq!(
            attack.effect,
            [SpellEffectKind::DrawDiscard {
                who: PlayerRecipient::Controller,
                draw_count: 1,
                discard_count: 1,
                order: DrawDiscardOrder::DiscardThenDraw,
                optional: true,
            }]
        );

        let matched = match_clause(
            "Search your library for a legendary creature card, reveal it, put it into your hand, then shuffle.",
            true,
            &context(),
        )
        .unwrap()
        .unwrap();
        let RecipeEmission::SpellEffect(SpellEffectKind::SearchLibrary {
            filter: Some(filter),
            destination,
            shuffle,
            reveal,
            optional,
            ..
        }) = matched.emission
        else {
            panic!("legendary search recipe must emit SearchLibrary");
        };
        assert_eq!(filter.card_type, Some(CardTypeFilter::Creature));
        assert_eq!(filter.required_supertypes, ["Legendary"]);
        assert_eq!(destination, SearchDestination::Hand);
        assert!(shuffle && reveal && !optional);
    }

    #[test]
    fn issue_267_templates_fail_closed_on_source_kinds_and_near_misses() {
        let mut noncreature = context();
        noncreature.source_is_creature = false;
        noncreature.source_is_enchantment = false;
        for clause in [
            "During your turn, this creature has first strike.",
            "This creature can't block.",
            "This creature enters tapped.",
            "You may play an additional land on each of your turns.",
            "You may play lands from your graveyard.",
            "Landfall — Whenever a land you control enters, mill a card.",
            "Landfall — Whenever a land you control enters, this creature deals 1 damage to each opponent.",
            "Whenever this creature attacks, you may discard a card. If you do, draw a card.",
            "When this creature enters, return target creature card from your graveyard to your hand.",
        ] {
            assert_eq!(match_clause(clause, false, &noncreature), Ok(None), "{clause}");
        }

        let mut nonartifact_or_enchantment = context();
        nonartifact_or_enchantment.source_is_artifact = false;
        nonartifact_or_enchantment.source_is_enchantment = false;
        for clause in [
            "Creatures you control get +1/+1.",
            "At the beginning of your end step, draw a card.",
        ] {
            assert_eq!(
                match_clause(clause, false, &nonartifact_or_enchantment),
                Ok(None),
                "{clause}"
            );
        }
        nonartifact_or_enchantment.source_name = "The Seriema".into();
        assert_eq!(
            match_clause(
                "When The Seriema enters, search your library for a legendary creature card, reveal it, put it into your hand, then shuffle.",
                false,
                &nonartifact_or_enchantment,
            ),
            Ok(None),
            "artifact ETB search must reject a non-artifact source"
        );

        let mut nonenchantment = context();
        nonenchantment.source_is_enchantment = false;
        assert_eq!(
            match_clause(
                "Whenever a creature you control enters, this enchantment deals 1 damage to each opponent.",
                false,
                &nonenchantment,
            ),
            Ok(None)
        );

        for (clause, is_spell) in [
            ("This creature has first strike.", false),
            ("This creature can't block or attack.", false),
            ("This creature enters the battlefield tapped.", false),
            ("You may play two additional lands on each of your turns.", false),
            ("Landfall — Whenever a land you control enters, mill two cards.", false),
            ("Whenever a land you control enters, this creature deals 1 damage to each opponent.", false),
            ("Landfall — Whenever a land enters, this creature deals 1 damage to each opponent.", false),
            ("Landfall — Whenever a land you control enters, this creature deals 2 damage to each opponent.", false),
            ("Landfall — Whenever a land you control enters, target opponent takes 1 damage.", false),
            ("Landfall — Whenever a land you control enters, each opponent loses 1 life.", false),
            ("Search your library for a creature card, reveal it, put it into your hand, then shuffle.", true),
            ("Create two 1/1 black Rat creature tokens.", true),
        ] {
            assert_eq!(match_clause(clause, is_spell, &context()), Ok(None), "{clause}");
        }
    }

    #[test]
    fn issue_270_utility_templates_emit_exact_typed_abilities() {
        let cases = [
            (
                "When this artifact enters, draw a card.",
                "etb.artifact.draw.one",
            ),
            (
                "When this artifact enters, scry 2.",
                "etb.artifact.scry.two",
            ),
            (
                "When this artifact enters, create a Food token.",
                "etb.artifact.create_food.one",
            ),
            (
                "{1}, {T}, Sacrifice this artifact: Add one mana of any color.",
                "activated.artifact.pay_one_tap_sacrifice.any_color",
            ),
            (
                "{2}, {T}, Sacrifice this artifact: You gain 3 life.",
                "activated.artifact.pay_two_tap_sacrifice.gain_three",
            ),
            (
                "{2}, {T}, Sacrifice this artifact: You gain 3 life and draw a card.",
                "activated.artifact.pay_two_tap_sacrifice.gain_three_draw_one",
            ),
            (
                "{3}{U}, Sacrifice this artifact: Draw two cards.",
                "activated.artifact.pay_three_u_sacrifice.draw_two",
            ),
            (
                "{2}, Sacrifice this creature: Draw a card.",
                "activated.creature.pay_two_sacrifice.draw_one",
            ),
            (
                "{3}, {T}, Sacrifice this artifact: It deals 3 damage to target creature.",
                "activated.artifact.pay_three_tap_sacrifice.damage_creature_three",
            ),
            (
                "{7}, {T}, Sacrifice this artifact: Destroy target permanent.",
                "activated.artifact.pay_seven_tap_sacrifice.destroy_permanent",
            ),
            (
                "{1}, {T}: Add one mana of any color.",
                "activated.mana.pay_one_tap_any_color",
            ),
        ];

        let emissions = cases.map(|(clause, expected_id)| {
            let matched = match_clause(clause, false, &context())
                .expect("issue #270 clause must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #270 clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id, "{clause}");
            matched.emission
        });

        let RecipeEmission::ActivatedAbility(gain_then_draw) = &emissions[5] else {
            panic!("Candy Trail recipe must emit an activated ability");
        };
        assert_eq!(
            gain_then_draw.costs,
            [
                AbilityCost::Mana(ManaCost::parse("{2}").unwrap()),
                AbilityCost::Tap,
                AbilityCost::SacrificeSelf,
            ]
        );
        assert_eq!(
            gain_then_draw.effect,
            [
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(3),
                },
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                },
            ]
        );

        let RecipeEmission::ActivatedAbility(damage) = &emissions[8] else {
            panic!("Bear Trap recipe must emit an activated ability");
        };
        assert!(matches!(
            damage.effect.as_slice(),
            [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    ..
                },
            }]
        ));
        assert_eq!(
            damage.targeting.as_ref().unwrap().groups[0].effect_indices,
            [0]
        );

        let RecipeEmission::ActivatedAbility(sacrificed_mana) = &emissions[3] else {
            panic!("Omni-Cheese Pizza recipe must emit an activated ability");
        };
        assert_eq!(sacrificed_mana.mana_options().unwrap().len(), 5);
        assert!(sacrificed_mana.targeting.is_none());

        let mut nonartifact = context();
        nonartifact.source_is_artifact = false;
        for (clause, _) in &cases[..7] {
            assert_eq!(
                match_clause(clause, false, &nonartifact),
                Ok(None),
                "{clause} must require an artifact source"
            );
        }
        for (clause, _) in &cases[8..10] {
            assert_eq!(
                match_clause(clause, false, &nonartifact),
                Ok(None),
                "{clause} must require an artifact source"
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert_eq!(match_clause(cases[7].0, false, &noncreature), Ok(None));

        let mut nonland = context();
        nonland.source_is_land = false;
        assert!(match_clause(cases[10].0, false, &nonland)
            .expect("generic paid mana recipe must not be ambiguous")
            .is_some());
    }

    #[test]
    fn issue_281_etb_instant_or_sorcery_return_is_exact_and_fail_closed() {
        let clause =
            "When this creature enters, return target instant or sorcery card from your graveyard to your hand.";
        let matched = match_clause(clause, false, &context())
            .expect("issue #281 clause must not be ambiguous")
            .expect("issue #281 exact ETB recursion clause should match");
        assert_eq!(
            matched.id.as_str(),
            "triggered.etb.return_instant_or_sorcery_card_from_graveyard.hand"
        );
        assert_eq!(
            CATALOG
                .iter()
                .find(|recipe| recipe.id == matched.id)
                .expect("issue #281 recipe is registered")
                .surface,
            RecipeSurface::TriggeredAbility
        );
        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #281 recipe must emit a triggered ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(!ability.may);
        assert!(ability.targeting.is_none());
        assert_eq!(
            ability.effect,
            [SpellEffectKind::MoveGraveyardCards {
                filter: GraveyardFilter {
                    card: Some(ZoneCardFilter {
                        card_type: Some(CardTypeFilter::InstantOrSorcery),
                        ..ZoneCardFilter::default()
                    }),
                    ..GraveyardFilter::default()
                },
                destination: GraveyardDestination::Hand,
                linked_exile_id: None,
            }]
        );

        // The exact "your graveyard" creature filter belongs to the existing creature-card
        // recipe, so assert this recipe rejects it directly. Catalog calibration uses the
        // non-overlapping "a graveyard" variant below so validation remains exact-one globally.
        let creature_near_miss =
            "When this creature enters, return target creature card from your graveyard to your hand.";
        assert!(
            match_etb_return_instant_or_sorcery_card_to_hand(creature_near_miss, &context())
                .is_none()
        );
        assert_eq!(
            match_clause(creature_near_miss, false, &context())
                .expect("existing creature recursion recipe should remain exact")
                .expect("creature near-miss should be consumed by its existing recipe")
                .id
                .as_str(),
            "triggered.etb.return_creature_card_from_graveyard.hand"
        );

        for near_miss in [
            "When this creature enters, return target creature card from a graveyard to your hand.",
            "When this creature enters, return target permanent card from your graveyard to your hand.",
            "When this creature enters, return target card from your graveyard to your hand.",
            "When this creature enters, return target instant or sorcery card from a graveyard to your hand.",
            "When this creature enters, return target instant or sorcery card from an opponent's graveyard to your hand.",
            "When this creature enters, return target instant or sorcery card from your graveyard to the top of your library.",
            "When this creature enters, return target instant or sorcery card from your graveyard to the battlefield.",
            "When this creature enters, return target instant or sorcery card from your graveyard to exile.",
            "When this creature enters, return up to one target instant or sorcery card from your graveyard to your hand.",
            "When this creature enters, choose an instant or sorcery card in your graveyard, then return it to your hand.",
            "When this creature enters, you may return target instant or sorcery card from your graveyard to your hand.",
            "When this creature enters, return two target instant or sorcery cards from your graveyard to your hand.",
            "When this creature enters, return target instant or sorcery card from your graveyard to your hand, then draw a card.",
            "When this creature enters, return target instant or sorcery card from your graveyard to your hand. You gain 1 life.",
        ] {
            assert_eq!(
                match_clause(near_miss, false, &context()),
                Ok(None),
                "issue #281 recipe matched near-miss {near_miss}"
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert_eq!(match_clause(clause, false, &noncreature), Ok(None));
        assert_eq!(match_clause(clause, true, &context()), Ok(None));
    }

    #[test]
    fn issue_276_creature_or_planeswalker_power_damage_template_is_supported() {
        let cases = [
            "Target creature you control deals damage equal to its power to target creature or planeswalker you don't control.",
        ];
        for clause in cases {
            let matched = match_clause(clause, true, &context())
                .expect("issue #276 clause must not be ambiguous")
                .expect("issue #276 clause must be supported");
            assert_eq!(
                matched.id.as_str(),
                "spell.damage.creature.equal_power.controlled_to_noncontroller_permanent"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("issue #276 recipe must publish grouped spell targeting")
            };
            let [SpellEffectKind::CreatureDealsDamageEqualToPower { source, target }] =
                effects.as_slice()
            else {
                panic!("issue #276 recipe must publish one power-damage effect")
            };
            assert_eq!(source.kind, TargetKind::Creature);
            assert_eq!(source.controller, TargetController::You);
            assert_eq!(target.kind, TargetKind::AnyPermanent);
            assert_eq!(target.controller, TargetController::NotYou);
            assert_eq!(
                target.permanent_types,
                [
                    PermanentTypeFilter::Creature,
                    PermanentTypeFilter::Planeswalker
                ]
            );
            assert_eq!(targeting.groups.len(), 2);
            assert_eq!(targeting.groups[0].distinct_from, [1]);
            assert_eq!(targeting.groups[1].distinct_from, [0]);
        }

        let mut instant_only = context();
        instant_only.source_is_sorcery = false;
        assert!(match_clause(cases[0], true, &instant_only)
            .expect("issue #276 instant context must not be ambiguous")
            .is_some());

        let mut sorcery_only = context();
        sorcery_only.source_is_instant = false;
        assert!(match_clause(cases[0], true, &sorcery_only)
            .expect("issue #276 sorcery context must not be ambiguous")
            .is_some());

        for near_miss in [
            "Target creature you control gets +1/+0 until end of turn. Then it deals damage equal to its power to target creature you don't control.",
            "This spell costs {1} less to cast if it targets a Mount or Vehicle you control. Target creature you control deals damage equal to its power to target creature an opponent controls.",
            "Target creature you control deals damage equal to its power to target creature you don't control.",
            "Target creature you control deals damage equal to its power to each of two other target creatures.",
        ] {
            assert_eq!(
                match_clause(near_miss, true, &context()),
                Ok(None),
                "issue #276 near-miss must remain unsupported: {near_miss}"
            );
        }
    }
}
