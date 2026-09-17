use tricerules_cards::primitives::{
    ActivationLimit, BattlefieldAggregate, BattlefieldPermanentFilter, CardResultAction,
    CardResultFilter, CardResultSource, CardTypeFilter, CombatRestriction, CombatRestrictionScope,
    CombatRole, CountExpression, CreatureScopeController, CreatureScopeFilter, DiscardQuantity,
    DrawDiscardOrder, EffectSubject, EntersTappedAffected, EntryCost, GameCondition,
    GraveyardDestination, GraveyardFilter, GraveyardOwner, HandCardAction, HandCardChooser,
    HandChoiceVisibility, LibraryPlacement, LifeAmount, ObjectContributionKind,
    ObjectPaymentConstraint, PermanentEventFilter, PermanentTypeFilter, PlayerLifeAggregate,
    PlayerRecipient, PowerComparison, PowerToughnessCharacteristic, RelativePlayerSet,
    ResolutionBranchDef, ResolutionBranchRequirement, ResolutionBranchSelection, ResolutionCost,
    SearchDestination, SearchZoneSelection, SpellCastFilter, SpellCostModifier,
    SpellManaSpentComparison, StackSpellFilter, StaticAbilityDef, TargetController, TargetFilter,
    TargetGroupDef, TargetKind, TargetMatchFilter, TargetObjectExclusion, TargetingDef,
    TargetingSourceFilter, TypeLineAddition, ZoneCardFilter,
};
use tricerules_cards::{
    external_oracle_lines, AbilityCost, AbilityId, AbilityPresentation, AbilitySourceZone,
    ActivatedAbilityDef, ActivationTiming, Amount, BasicLandType, CastTriggerPlayer,
    CharacteristicDefiningAbility, ChoiceId, CounterKind, IdentifiedAbility, Keyword,
    LibraryPartitionKind, ManaAmount, ManaCost, SpellEffectKind, TriggerCondition,
    TriggeredAbilityDef,
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
    /// The complete paired Station header and threshold striation. The assembly owns both
    /// lines so an orphan, reordered, duplicated, or appended Station fragment stays unsupported.
    StationAssembly,
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
    /// A printed alternative cast-method cost line (`Warp {cost}`, `Flashback {cost}`) that emits a
    /// cost field on the containing face instead of an ability or resolution effect. Warp is
    /// permanent-face-only (CR 702.185) and Flashback is instant/sorcery-face-only (CR 702.34), so
    /// each matcher owns its own face-type gate rather than relying on [`Self::SpellClause`].
    CastMethodClause,
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
    /// Printing-independent Oracle identity, when matching a real Scryfall card. Unit-test and
    /// catalog-calibration contexts omit this so the clause shape can be exercised in isolation.
    pub(super) oracle_id: Option<String>,
    pub(super) source_is_permanent: bool,
    pub(super) source_is_artifact: bool,
    pub(super) source_is_spacecraft_or_planet: bool,
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
    StationAssembly(StationAssemblyEmission),
    /// CR 702.185: the face-level hand alternative cost printed as `Warp {cost}`.
    WarpCost(ManaCost),
    /// CR 702.34: the face-level graveyard alternative cost printed as `Flashback {cost}`.
    FlashbackCost(ManaCost),
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
pub(super) struct StationAssemblyEmission {
    pub(super) activated_ability: ActivatedAbilityDef,
    pub(super) static_ability: IdentifiedAbility<StaticAbilityDef>,
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

fn match_enchant_creature_you_control(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_aura && text == "Enchant creature you control").then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::AuraAttach {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
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

const ISSUE_298_FIRST_STRIKE_ORACLE_IDS: &[&str] = &[
    "88923ca1-a793-42f0-b9f8-ed9ff9c1185d", // Super Speed
    "89fb21dc-4cf2-4c9a-b0ae-cc6e10277fb6", // Fire-Rim Form
];

const ISSUE_298_HEXPROOF_ORACLE_IDS: &[&str] = &[
    "b9dee727-8ad8-42e0-93c6-5ef91d3f7309", // Aquitect's Defenses
    "d3912c82-37f8-456e-ba49-65c7f5b39d13", // Fae Flight
];

const ISSUE_298_AQUITECT_HEXPROOF_ETB_TEXT: &str = "When this Aura enters, enchanted creature gains hexproof until end of turn. (It can't be the target of spells or abilities your opponents control.)";

const ISSUE_298_HEXPROOF_ETB_TEXT: &str =
    "When this Aura enters, enchanted creature gains hexproof until end of turn.";

/// The four reviewed cards have several independently supported Aura clauses. Keep their
/// complete source surfaces bound together so a future exact-recipe match cannot combine one
/// card's ETB keyword with another card's attachment or static modifier.
pub(super) fn issue_298_card_surface_is_exact(
    oracle_id: &str,
    name: &str,
    mana_cost: &str,
    type_line: &str,
    oracle_text: &str,
) -> bool {
    let expected = match oracle_id {
        "88923ca1-a793-42f0-b9f8-ed9ff9c1185d" => (
            "Super Speed",
            "{R}",
            "Enchantment — Aura",
            "Flash\nEnchant creature\nWhen this Aura enters, enchanted creature gains first strike until end of turn.\nEnchanted creature gets +1/+0 and has haste.",
        ),
        "89fb21dc-4cf2-4c9a-b0ae-cc6e10277fb6" => (
            "Fire-Rim Form",
            "{1}{R}",
            "Enchantment — Aura",
            "Flash\nEnchant creature\nWhen this Aura enters, enchanted creature gains first strike until end of turn.\nEnchanted creature gets +2/+0.",
        ),
        "b9dee727-8ad8-42e0-93c6-5ef91d3f7309" => (
            "Aquitect's Defenses",
            "{1}{U}",
            "Enchantment — Aura",
            "Flash\nEnchant creature you control\nWhen this Aura enters, enchanted creature gains hexproof until end of turn. (It can't be the target of spells or abilities your opponents control.)\nEnchanted creature gets +1/+2.",
        ),
        "d3912c82-37f8-456e-ba49-65c7f5b39d13" => (
            "Fae Flight",
            "{1}{U}",
            "Enchantment — Aura",
            "Flash\nEnchant creature\nWhen this Aura enters, enchanted creature gains hexproof until end of turn.\nEnchanted creature gets +1/+0 and has flying.",
        ),
        _ => return true,
    };
    (name, mana_cost, type_line, oracle_text) == expected
}

fn issue_298_oracle_id_is_reviewed(context: &RecipeContext, reviewed_oracle_ids: &[&str]) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| reviewed_oracle_ids.contains(&oracle_id))
}

fn match_aura_etb_grant_first_strike(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_aura
        && issue_298_oracle_id_is_reviewed(context, ISSUE_298_FIRST_STRIKE_ORACLE_IDS)
        && text
            == "When this Aura enters, enchanted creature gains first strike until end of turn.")
        .then(|| {
            triggered_ability(
                context,
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::AttachedObject,
                    keywords: vec![Keyword::FirstStrike],
                },
            )
        })
}

fn match_aura_etb_grant_hexproof(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    if !context.source_is_aura
        || !issue_298_oracle_id_is_reviewed(context, ISSUE_298_HEXPROOF_ORACLE_IDS)
    {
        return None;
    }
    let exact_text = match context.oracle_id.as_deref() {
        Some("b9dee727-8ad8-42e0-93c6-5ef91d3f7309") => {
            text == ISSUE_298_AQUITECT_HEXPROOF_ETB_TEXT
        }
        Some("d3912c82-37f8-456e-ba49-65c7f5b39d13") => text == ISSUE_298_HEXPROOF_ETB_TEXT,
        None => matches!(
            text,
            ISSUE_298_AQUITECT_HEXPROOF_ETB_TEXT | ISSUE_298_HEXPROOF_ETB_TEXT
        ),
        Some(_) => false,
    };
    exact_text.then(|| {
        triggered_ability(
            context,
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::AttachedObject,
                keywords: vec![Keyword::Hexproof],
            },
        )
    })
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

fn match_equipment_attached_object_attacks_tap_defending_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_equipment
        && text
            == "Whenever equipped creature attacks, tap target creature defending player controls.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::DefendingPlayer,
                ..TargetFilter::default()
            };
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
                context,
                TriggerCondition::WheneverAttachedObjectAttacks,
                vec![SpellEffectKind::Tap {
                    subject: EffectSubject::Chosen(Box::new(target)),
                }],
            ) else {
                unreachable!("triggered_ability_with always returns a triggered ability")
            };
            ability.targeting = Some(exact_targeting(
                1,
                1,
                "Choose target creature defending player controls",
                vec![0],
            ));
            RecipeEmission::TriggeredAbility(ability)
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

const ISSUE_288_REVIEWED_ORACLE_IDS: &[&str] = &[
    "3e7879d5-62ea-4c9a-9fc0-659d70f3a8e1", // Unscrupulous Agent
    "6420e8a0-3ef4-4f95-bb6a-12409eef4d48", // Skullcap Snail
];

fn issue_288_oracle_id_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_288_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

fn match_etb_target_opponent_exiles_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && issue_288_oracle_id_is_reviewed(context)
        && text == "When this creature enters, target opponent exiles a card from their hand.")
        .then(|| {
            targeted_trigger(
                context,
                vec![SpellEffectKind::ChooseHandCards {
                    action: HandCardAction::Exile,
                    count: 1,
                    target: TargetFilter {
                        kind: TargetKind::OpponentPlayer,
                        ..TargetFilter::default()
                    },
                    chooser: HandCardChooser::AffectedPlayer,
                    card_filter: None,
                    optional: false,
                    visibility: HandChoiceVisibility::PrivateLook,
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

fn match_self_enters_optional_search_basic_land_to_top(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.")
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
                vec![SpellEffectKind::ChooseResolutionBranch {
                    chooser: PlayerRecipient::Controller,
                    optional: true,
                    selection: ResolutionBranchSelection::PlayerChoice,
                    branches: vec![ResolutionBranchDef {
                        branch_id: ChoiceId::new("search_for_a_basic_land")
                            .expect("closed search branch uses a valid choice ID"),
                        presentation: AbilityPresentation::Fallback,
                        runtime_fallback: None,
                        cost: ResolutionCost::None,
                        requirement: Default::default(),
                        effects: vec![SpellEffectKind::SearchLibrary {
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
                            destination: SearchDestination::TopOfLibrary,
                            conditional_destination: None,
                            shuffle: true,
                            reveal: true,
                            result_id: None,
                        }],
                    }],
                    otherwise: Vec::new(),
                }],
            )
        })
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

const ISSUE_290_REVIEWED_ORACLE_IDS: &[&str] = &[
    "656fc672-efa5-484a-b5e8-eac262331439", // Sage of Days
    "fde50a0d-9bc6-45f4-873e-3de81a513fac", // Gurmag Nightwatch
];

fn issue_290_oracle_id_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_290_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

const ISSUE_291_REVIEWED_ORACLE_IDS: &[&str] = &[
    "b24a87af-407f-4c58-80b4-caab9c65a233", // Crustacean Commando
    "f570bac8-9987-4963-af02-476d18abc847", // Slithering Cryptid
];

fn issue_291_oracle_id_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_291_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

const ISSUE_291_MUTAGEN_ETB_TEXT: &str = r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#;

fn match_etb_create_mutagen(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && issue_291_oracle_id_is_reviewed(context)
        && text == ISSUE_291_MUTAGEN_ETB_TEXT)
        .then(|| {
            triggered_ability(
                context,
                SpellEffectKind::CreateTokens {
                    token: "mutagen".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                },
            )
        })
}

const ISSUE_297_REVIEWED_ORACLE_IDS: &[&str] = &[
    "ac8cae63-270c-4f73-b29b-50f8f2395fd9", // City Pigeon
];

fn issue_297_oracle_id_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_297_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

const ISSUE_297_FOOD_LEAVES_TEXT: &str = r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#;

fn match_self_leaves_battlefield_create_food(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && issue_297_oracle_id_is_reviewed(context)
        && text == ISSUE_297_FOOD_LEAVES_TEXT)
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WhenSelfLeavesBattlefield,
                vec![SpellEffectKind::CreateTokens {
                    token: "food".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }],
            )
        })
}

const ISSUE_299_REVIEWED_ORACLE_IDS: &[&str] = &[
    "5d24fb6b-7176-407c-b917-6c88a6da40df", // Nezumi Linkbreaker
    "c1348afe-4dc7-41bb-9d3f-1e8751abd7db", // Wanted Griffin
];

fn issue_299_oracle_id_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_299_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

const ISSUE_299_DIES_MERCENARY_TEXT: &str = r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##;

fn match_dies_create_mercenary(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && issue_299_oracle_id_is_reviewed(context)
        && text == ISSUE_299_DIES_MERCENARY_TEXT)
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WhenSelfDies,
                vec![SpellEffectKind::CreateTokens {
                    token: "mercenary_r_1_1".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }],
            )
        })
}

const ISSUE_300_REVIEWED_ORACLE_IDS: &[&str] = &[
    "ce000c0b-db42-4569-855f-f4eae0431c09", // Ripchain Razorkin
    "3b66d2c2-be7a-4296-9888-f0cfb2975e89", // Seismic Monstrosaur
];

fn issue_300_oracle_id_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        // Catalog calibration contexts intentionally omit Oracle identity. Real
        // card parsing supplies Some(oracle_id), including Some("") for malformed
        // input, so unknown identities still fail closed during generation.
        .is_none_or(|oracle_id| ISSUE_300_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

fn match_land_sacrifice_draw_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && issue_300_oracle_id_is_reviewed(context)
        && text == "{2}{R}, Sacrifice a land: Draw a card.")
        .then(|| {
            utility_activated_ability(
                context,
                vec![
                    fixed_mana_cost("{2}{R}"),
                    AbilityCost::SacrificePermanent {
                        filter: TargetFilter {
                            kind: TargetKind::AnyPermanent,
                            controller: TargetController::You,
                            permanent_types: vec![PermanentTypeFilter::Land],
                            ..TargetFilter::default()
                        },
                    },
                ],
                vec![SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                }],
                None,
            )
        })
}

const ISSUE_314_MYSTIC_ORACLE_ID: &str = "b77dfe2a-ebc9-46b0-9134-2ecb2abdd8be";
const ISSUE_314_OSCORP_ORACLE_ID: &str = "4d2e233c-0173-417f-82a0-1e692a400ae1";
const ISSUE_314_REVIEWED_ORACLE_IDS: &[&str] =
    &[ISSUE_314_MYSTIC_ORACLE_ID, ISSUE_314_OSCORP_ORACLE_ID];

fn issue_314_variant(context: &RecipeContext) -> Option<(&'static str, &'static str)> {
    match context.oracle_id.as_deref() {
        Some(ISSUE_314_MYSTIC_ORACLE_ID) if context.source_name == "Mystic Archaeologist" => {
            Some(("Mystic Archaeologist", "{3}{U}{U}"))
        }
        Some(ISSUE_314_OSCORP_ORACLE_ID) if context.source_name == "Oscorp Research Team" => {
            Some(("Oscorp Research Team", "{6}{U}"))
        }
        Some(_) => None,
        None => match context.source_name.as_str() {
            "Mystic Archaeologist" => Some(("Mystic Archaeologist", "{3}{U}{U}")),
            "Oscorp Research Team" => Some(("Oscorp Research Team", "{6}{U}")),
            _ => None,
        },
    }
}

fn issue_314_context_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_314_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

pub(super) fn issue_314_oracle_id_is_reviewed(oracle_id: &str) -> bool {
    ISSUE_314_REVIEWED_ORACLE_IDS.contains(&oracle_id)
}

pub(super) fn issue_314_card_surface_is_exact(
    oracle_id: &str,
    name: &str,
    mana_cost: &str,
    type_line: &str,
    oracle_text: &str,
    power: Option<&str>,
    toughness: Option<&str>,
) -> bool {
    let expected = match oracle_id {
        ISSUE_314_MYSTIC_ORACLE_ID => (
            "Mystic Archaeologist",
            "{1}{U}",
            "Creature — Human Wizard",
            Some("2"),
            Some("1"),
            "{3}{U}{U}: Draw two cards.",
        ),
        ISSUE_314_OSCORP_ORACLE_ID => (
            "Oscorp Research Team",
            "{3}{U}",
            "Creature — Human Scientist",
            Some("1"),
            Some("5"),
            "{6}{U}: Draw two cards.",
        ),
        _ => return true,
    };
    (name, mana_cost, type_line, power, toughness, oracle_text) == expected
}

fn match_creature_pay_draw_two(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    if !context.source_is_creature
        || !issue_314_context_is_reviewed(context)
        || issue_314_variant(context).is_none()
    {
        return None;
    }
    let (_, activation_cost) = issue_314_variant(context)?;
    let expected_text = match activation_cost {
        "{3}{U}{U}" => "{3}{U}{U}: Draw two cards.",
        "{6}{U}" => "{6}{U}: Draw two cards.",
        _ => return None,
    };
    (text == expected_text).then(|| {
        utility_activated_ability(
            context,
            vec![fixed_mana_cost(activation_cost)],
            vec![SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            }],
            None,
        )
    })
}

const ISSUE_315_COEURL_ORACLE_ID: &str = "00d1596a-c3e2-4109-86da-388934a0c652";
const ISSUE_315_FROSTBRIDGE_GUARD_ORACLE_ID: &str = "515c1604-59f0-45b4-91be-d2f20fdd3e1e";
const ISSUE_315_STERLING_KEYKEEPER_ORACLE_ID: &str = "f893d3d6-efef-4394-8e15-e01deed72b4f";
const ISSUE_315_REVIEWED_ORACLE_IDS: &[&str] = &[
    ISSUE_315_COEURL_ORACLE_ID,
    ISSUE_315_FROSTBRIDGE_GUARD_ORACLE_ID,
    ISSUE_315_STERLING_KEYKEEPER_ORACLE_ID,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Issue315Variant {
    activation_cost: &'static str,
    clause: &'static str,
    prompt: &'static str,
    excluded_permanent_type: Option<PermanentTypeFilter>,
    excluded_subtype: Option<&'static str>,
}

fn issue_315_variant(context: &RecipeContext) -> Option<Issue315Variant> {
    match context.oracle_id.as_deref() {
        Some(ISSUE_315_COEURL_ORACLE_ID) if context.source_name == "Coeurl" => {
            Some(Issue315Variant {
                activation_cost: "{1}{W}",
                clause: "{1}{W}, {T}: Tap target nonenchantment creature.",
                prompt: "Choose target nonenchantment creature",
                excluded_permanent_type: Some(PermanentTypeFilter::Enchantment),
                excluded_subtype: None,
            })
        }
        Some(ISSUE_315_FROSTBRIDGE_GUARD_ORACLE_ID)
            if context.source_name == "Frostbridge Guard" =>
        {
            Some(Issue315Variant {
                activation_cost: "{2}{W}",
                clause: "{2}{W}, {T}: Tap target creature.",
                prompt: "Choose target creature",
                excluded_permanent_type: None,
                excluded_subtype: None,
            })
        }
        Some(ISSUE_315_STERLING_KEYKEEPER_ORACLE_ID)
            if context.source_name == "Sterling Keykeeper" =>
        {
            Some(Issue315Variant {
                activation_cost: "{2}",
                clause: "{2}, {T}: Tap target non-Mount creature.",
                prompt: "Choose target non-Mount creature",
                excluded_permanent_type: None,
                excluded_subtype: Some("Mount"),
            })
        }
        Some(_) => None,
        None => match context.source_name.as_str() {
            "Coeurl" => Some(Issue315Variant {
                activation_cost: "{1}{W}",
                clause: "{1}{W}, {T}: Tap target nonenchantment creature.",
                prompt: "Choose target nonenchantment creature",
                excluded_permanent_type: Some(PermanentTypeFilter::Enchantment),
                excluded_subtype: None,
            }),
            "Frostbridge Guard" => Some(Issue315Variant {
                activation_cost: "{2}{W}",
                clause: "{2}{W}, {T}: Tap target creature.",
                prompt: "Choose target creature",
                excluded_permanent_type: None,
                excluded_subtype: None,
            }),
            "Sterling Keykeeper" => Some(Issue315Variant {
                activation_cost: "{2}",
                clause: "{2}, {T}: Tap target non-Mount creature.",
                prompt: "Choose target non-Mount creature",
                excluded_permanent_type: None,
                excluded_subtype: Some("Mount"),
            }),
            _ => None,
        },
    }
}

fn issue_315_context_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_315_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

pub(super) fn issue_315_oracle_id_is_reviewed(oracle_id: &str) -> bool {
    ISSUE_315_REVIEWED_ORACLE_IDS.contains(&oracle_id)
}

pub(super) fn issue_315_card_surface_is_exact(
    oracle_id: &str,
    name: &str,
    mana_cost: &str,
    type_line: &str,
    oracle_text: &str,
    power: Option<&str>,
    toughness: Option<&str>,
) -> bool {
    let expected = match oracle_id {
        ISSUE_315_COEURL_ORACLE_ID => (
            "Coeurl",
            "{1}{W}",
            "Creature — Cat Beast",
            Some("2"),
            Some("2"),
            "{1}{W}, {T}: Tap target nonenchantment creature.",
        ),
        ISSUE_315_FROSTBRIDGE_GUARD_ORACLE_ID => (
            "Frostbridge Guard",
            "{1}{W}",
            "Creature — Elemental Soldier",
            Some("2"),
            Some("2"),
            "{2}{W}, {T}: Tap target creature.",
        ),
        ISSUE_315_STERLING_KEYKEEPER_ORACLE_ID => (
            "Sterling Keykeeper",
            "{1}{W}",
            "Creature — Human Mercenary",
            Some("2"),
            Some("2"),
            "{2}, {T}: Tap target non-Mount creature.",
        ),
        _ => return true,
    };
    (name, mana_cost, type_line, power, toughness, oracle_text) == expected
}

const ISSUE_287_LONG_LAKE_NUISANCE_ORACLE_ID: &str = "a833fdf1-db0c-4846-8452-d3b2059c2355";
const ISSUE_287_PATIENT_INSTRUCTOR_ORACLE_ID: &str = "bddd7e99-ec74-4ca6-9137-155b85695a95";
const ISSUE_287_REVIEWED_ORACLE_IDS: &[&str] = &[
    ISSUE_287_LONG_LAKE_NUISANCE_ORACLE_ID,
    ISSUE_287_PATIENT_INSTRUCTOR_ORACLE_ID,
];

/// CR 701.70: Recruit is draw, then a private mandatory discard, then a nonland-gated token.
/// The engine-owned branch reads the typed discard result emitted by the immediately preceding
/// instruction, so no authoring surface can weaken the land/nonland check.
fn recruit_effects() -> Vec<SpellEffectKind> {
    vec![
        SpellEffectKind::DrawDiscard {
            who: PlayerRecipient::Controller,
            draw_count: 1,
            discard_count: 1,
            order: DrawDiscardOrder::DrawThenDiscard,
            optional: false,
        },
        SpellEffectKind::ChooseResolutionBranch {
            chooser: PlayerRecipient::Controller,
            optional: false,
            selection: ResolutionBranchSelection::FirstApplicable,
            branches: vec![
                ResolutionBranchDef {
                    branch_id: ChoiceId::new("create_a_soldier")
                        .expect("closed Recruit branch uses a valid choice ID"),
                    presentation: AbilityPresentation::Fallback,
                    runtime_fallback: None,
                    cost: ResolutionCost::None,
                    requirement: ResolutionBranchRequirement::CardResultCount {
                        filter: CardResultFilter {
                            source: CardResultSource::PreviousEffect,
                            action: CardResultAction::Discard,
                            players: RelativePlayerSet::Controller,
                            card_type: Some(CardTypeFilter::Nonland),
                        },
                        min: Some(1),
                        max: None,
                    },
                    effects: vec![SpellEffectKind::CreateTokens {
                        token: "human_soldier_w_1_1".into(),
                        count: Amount::Fixed(1),
                        who: PlayerRecipient::Controller,
                        tapped: false,
                        sacrifice_timing: None,
                    }],
                },
                ResolutionBranchDef {
                    branch_id: ChoiceId::new("no_soldier")
                        .expect("closed Recruit fallback uses a valid choice ID"),
                    presentation: AbilityPresentation::Fallback,
                    runtime_fallback: None,
                    cost: ResolutionCost::None,
                    requirement: ResolutionBranchRequirement::Always,
                    effects: Vec::new(),
                },
            ],
            otherwise: Vec::new(),
        },
    ]
}

const ISSUE_287_RECRUIT_CLAUSE: &str = r#"When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"#;

pub(super) fn issue_287_oracle_id_is_reviewed(oracle_id: &str) -> bool {
    ISSUE_287_REVIEWED_ORACLE_IDS.contains(&oracle_id)
}

pub(super) fn issue_287_card_surface_is_exact(
    oracle_id: &str,
    name: &str,
    mana_cost: &str,
    type_line: &str,
    oracle_text: &str,
    power: Option<&str>,
    toughness: Option<&str>,
) -> bool {
    let expected = match oracle_id {
        ISSUE_287_LONG_LAKE_NUISANCE_ORACLE_ID => (
            "Long Lake Nuisance",
            "{3}{U}",
            "Creature — Bird",
            Some("3"),
            Some("1"),
            "Flying\nWhen this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
        ),
        ISSUE_287_PATIENT_INSTRUCTOR_ORACLE_ID => (
            "Patient Instructor",
            "{2}{W/U}",
            "Creature — Human Citizen",
            Some("2"),
            Some("2"),
            "Vigilance\nWhen this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
        ),
        _ => return true,
    };
    (name, mana_cost, type_line, power, toughness, oracle_text) == expected
}

fn match_etb_recruit(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature
        && context
            .oracle_id
            .as_deref()
            .is_none_or(issue_287_oracle_id_is_reviewed)
        && text == ISSUE_287_RECRUIT_CLAUSE)
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
                recruit_effects(),
            )
        })
}

fn match_creature_pay_mana_tap_tap_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    if !context.source_is_permanent
        || !context.source_is_creature
        || !issue_315_context_is_reviewed(context)
    {
        return None;
    }
    let variant = issue_315_variant(context)?;
    if text != variant.clause {
        return None;
    }
    let mut target = TargetFilter {
        kind: TargetKind::Creature,
        ..TargetFilter::default()
    };
    if let Some(permanent_type) = variant.excluded_permanent_type {
        target.excluded_permanent_types.push(permanent_type);
    }
    if let Some(subtype) = variant.excluded_subtype {
        target.excluded_subtypes.push(subtype.into());
    }
    Some(utility_activated_ability(
        context,
        vec![fixed_mana_cost(variant.activation_cost), AbilityCost::Tap],
        vec![SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(Box::new(target)),
        }],
        Some(TargetingDef {
            groups: vec![TargetGroupDef {
                min: 1,
                max: 1,
                prompt: variant.prompt.into(),
                effect_indices: vec![0],
                distinct_from: Vec::new(),
                same_graveyard: false,
                cast_cost_expansion: None,
            }],
        }),
    ))
}

/// Issue #316 exact templates. Each template is a reusable typed surface with at least two real
/// positive calibrations; cohort containment comes from generating against an isolated pinned
/// input, not from identity allowlists. Source-context gating stays on the recipes that require a
/// creature source, and every clause is compared by the complete normalized Oracle line so an
/// appended, reordered, or additional-clause form remains unsupported.
const ISSUE_316_TAPPED_CREATURE_CLAUSE: &str = "Destroy target tapped creature.";
const ISSUE_316_ARTIFACT_OR_ENCHANTMENT_ETB_CLAUSE: &str =
    "When this creature enters, destroy up to one target artifact or enchantment.";
const ISSUE_316_SINGLE_GRAVEYARD_EXILE_ETB_CLAUSE: &str =
    "When this creature enters, exile up to two target cards from a single graveyard.";
const ISSUE_316_NONCREATURE_CAST_PING_CLAUSE: &str =
    "Whenever you cast a noncreature spell, this creature deals 1 damage to each opponent.";
const ISSUE_316_FIRST_STRIKE_PREFIX: &str = "Target creature gets +";
const ISSUE_316_FIRST_STRIKE_SUFFIX: &str = "/+0 and gains first strike until end of turn.";
const ISSUE_316_POWER_BOUND_PREFIX: &str = "Destroy target creature with power ";
const ISSUE_316_POWER_BOUND_SUFFIX: &str = " or less.";

/// CR 115.1 / 701.8: "Destroy target tapped creature" is one mandatory creature target whose
/// current tapped status is an engine target-legality predicate (compare the existing modal
/// "deals 2 damage to target tapped creature" consumer).
fn match_spell_destroy_tapped_creature(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_316_TAPPED_CREATURE_CLAUSE).then(|| RecipeEmission::SpellEffectsWithTargeting {
        effects: vec![SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                tapped: Some(true),
                ..TargetFilter::default()
            })),
        }],
        targeting: exact_targeting(1, 1, "Choose target tapped creature", vec![0]),
    })
}

/// CR 603.2 / 701.8: a creature entry trigger destroys up to one artifact or enchantment. The
/// type union reuses the `spell.destroy.artifact_or_enchantment` predicate, but only in the
/// ability context and only for a creature source, so the spell forms stay with their recipes.
fn match_etb_destroy_up_to_one_artifact_or_enchantment(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_316_ARTIFACT_OR_ENCHANTMENT_ETB_CLAUSE).then(
        || {
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
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
            ) else {
                unreachable!("triggered_ability_with always returns a triggered ability")
            };
            ability.targeting = Some(exact_targeting(
                0,
                1,
                "Choose up to one target artifact or enchantment",
                vec![0],
            ));
            RecipeEmission::TriggeredAbility(ability)
        },
    )
}

/// CR 115.6 / 404.2 / 603.2: a creature entry trigger exiles up to two target cards, and grouped
/// target validation requires every chosen card to belong to one player's graveyard.
fn match_etb_exile_up_to_two_cards_from_single_graveyard(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_316_SINGLE_GRAVEYARD_EXILE_ETB_CLAUSE).then(|| {
        let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
            context,
            TriggerCondition::WhenSelfEntersBattlefield,
            vec![SpellEffectKind::MoveGraveyardCards {
                filter: GraveyardFilter {
                    owner: GraveyardOwner::AnyPlayer,
                    ..GraveyardFilter::default()
                },
                destination: GraveyardDestination::Exile,
                linked_exile_id: None,
            }],
        ) else {
            unreachable!("triggered_ability_with always returns a triggered ability")
        };
        ability.targeting = Some(TargetingDef {
            groups: vec![TargetGroupDef {
                min: 0,
                max: 2,
                prompt: "Choose up to two target cards from a single graveyard".into(),
                effect_indices: vec![0],
                distinct_from: Vec::new(),
                same_graveyard: true,
                cast_cost_expansion: None,
            }],
        });
        RecipeEmission::TriggeredAbility(ability)
    })
}

/// CR 603.2 / 601.2i: reuses the shipped controller-relative noncreature cast trigger and the
/// untargeted per-opponent damage recipient.
fn match_controller_casts_noncreature_ping_each_opponent_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_316_NONCREATURE_CAST_PING_CLAUSE).then(|| {
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
            vec![SpellEffectKind::DamagePlayer {
                amount: Amount::Fixed(1),
                who: PlayerRecipient::EachOpponent,
            }],
        )
    })
}

/// CR 611.2a / 702.7: "gets +N/+0 and gains first strike until end of turn" is parameterized over
/// a positive power amount; a zero power bonus or any nonzero toughness bonus stays unsupported.
fn match_spell_pump_creature_plus_n_zero_first_strike(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    let power = text
        .strip_prefix(ISSUE_316_FIRST_STRIKE_PREFIX)?
        .strip_suffix(ISSUE_316_FIRST_STRIKE_SUFFIX)?
        .parse::<i32>()
        .ok()?;
    (power >= 1).then(|| RecipeEmission::SpellEffectsWithTargeting {
        effects: vec![
            SpellEffectKind::PumpTarget {
                power,
                toughness: 0,
                scale: None,
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            },
            SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                keywords: vec![Keyword::FirstStrike],
            },
        ],
        targeting: exact_targeting(1, 1, "Choose target creature", vec![0, 1]),
    })
}

/// CR 208 / 701.8: "Destroy target creature with power N or less" is parameterized over the
/// inclusive power bound; greater-than, toughness, and up-to-one forms stay unsupported.
fn match_spell_destroy_creature_power_at_most(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    let bound = text
        .strip_prefix(ISSUE_316_POWER_BOUND_PREFIX)?
        .strip_suffix(ISSUE_316_POWER_BOUND_SUFFIX)?
        .parse::<u32>()
        .ok()?;
    (bound >= 1).then(|| RecipeEmission::SpellEffectsWithTargeting {
        effects: vec![SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                power: Some(PowerComparison::AtMost(bound)),
                ..TargetFilter::default()
            })),
        }],
        targeting: exact_targeting(
            1,
            1,
            &format!("Choose target creature with power {bound} or less"),
            vec![0],
        ),
    })
}

/// Issue #317 exact clause templates. Each template is a reusable typed surface with at least two
/// real positive calibrations. Every clause is compared by the complete normalized Oracle line, so
/// an appended, reordered, or additional-clause form remains unsupported, and source-kind gating
/// stays on the recipes whose printed template requires it.
const ISSUE_317_VEHICLE_ATTACK_TREASURE_CLAUSE: &str =
    "Whenever this Vehicle attacks, create a Treasure token.";
const ISSUE_317_RETURN_UP_TO_TWO_GRAVEYARD_CREATURES_CLAUSE: &str =
    "Return up to two target creature cards from your graveyard to your hand.";
const ISSUE_317_TAP_CREATURE_ANY_COLOR_CLAUSE: &str =
    "{T}, Tap an untapped creature you control: Add one mana of any color.";
const ISSUE_317_CANT_BE_BLOCKED_POWER_PREFIX: &str =
    "This creature can't be blocked by creatures with power ";
const ISSUE_317_CANT_BE_BLOCKED_POWER_SUFFIX: &str = " or less.";
const ISSUE_317_CREATE_CLUE_TOKEN_CLAUSE: &str = "Create a Clue token.";
const ISSUE_317_GRAVEYARD_RETURN_SELF_SUFFIX: &str =
    ": Return this card from your graveyard to your hand.";
/// The reviewed printed cost and full clause of the two calibration cards; test-only because the
/// matcher intentionally parameterizes over any exact printed mana cost.
#[cfg(test)]
const ISSUE_317_GRAVEYARD_RETURN_SELF_COST: &str = "{2}{B}";
#[cfg(test)]
const ISSUE_317_GRAVEYARD_RETURN_SELF_TO_HAND_CLAUSE: &str =
    "{2}{B}: Return this card from your graveyard to your hand.";

/// The printed-card creature predicate shared by graveyard recursion shapes: a creature card in
/// the controller's own graveyard (Raise Dead, Macabre Reconstruction, Vampire Soulcaller). Kept
/// as one constructor so the spell and permanent forms cannot drift apart.
fn graveyard_creature_card_filter() -> ZoneCardFilter {
    ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        ..ZoneCardFilter::default()
    }
}

/// CR 508.1 / 603.2: an attack trigger whose printed subject is "this Vehicle". Vehicle sources are
/// not creatures, so the clause is gated on the Vehicle subtype rather than the creature context
/// used by the other self-attack recipes. CR 111.10a: the created Treasure is the registered token.
fn match_self_attacks_vehicle_create_treasure(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_vehicle && text == ISSUE_317_VEHICLE_ATTACK_TREASURE_CLAUSE).then(|| {
        triggered_ability_with(
            context,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0,
            },
            vec![SpellEffectKind::CreateTokens {
                token: "treasure".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }],
        )
    })
}

/// CR 115.1 / 404.2 / 608.2b: "Return up to two target creature cards from your graveyard to your
/// hand" is one optional bounded graveyard-card group (min 0, max 2) sharing one
/// `MoveGraveyardCards` predicate for the controller's own graveyard. The one-card ETB forms keep
/// their own recipes, so this clause matches exactly once.
fn match_spell_return_up_to_two_graveyard_creature_cards(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_317_RETURN_UP_TO_TWO_GRAVEYARD_CREATURES_CLAUSE).then(|| {
        RecipeEmission::SpellEffectsWithTargeting {
            effects: vec![SpellEffectKind::MoveGraveyardCards {
                filter: GraveyardFilter {
                    owner: GraveyardOwner::Controller,
                    card: Some(graveyard_creature_card_filter()),
                    ..GraveyardFilter::default()
                },
                destination: GraveyardDestination::Hand,
                linked_exile_id: None,
            }],
            targeting: exact_targeting(
                0,
                2,
                "Choose up to two target creature cards from your graveyard",
                vec![0],
            ),
        }
    })
}

/// CR 601.2h / 605.1a: "{T}, Tap an untapped creature you control" is two atomic costs on one
/// activated ability: the source tap plus exactly one selected untapped creature the activator
/// controls. The source is excluded from the selection cohort because a creature source paying
/// `{T}` could otherwise be counted twice. The effect reuses the shipped any-color mana shape.
fn match_artifact_tap_creature_add_any_color(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact && text == ISSUE_317_TAP_CREATURE_ANY_COLOR_CLAUSE).then(|| {
        utility_activated_ability(
            context,
            vec![
                AbilityCost::Tap,
                AbilityCost::TapPermanents {
                    constraint: ObjectPaymentConstraint::ExactCount(1),
                    filter: TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::You,
                        ..TargetFilter::default()
                    },
                    exclude_source: true,
                },
            ],
            vec![five_color_mana_effect()],
            None,
        )
    })
}

/// CR 509.1b / 208: "This creature can't be blocked by creatures with power N or less" is a
/// characteristic-based blocking restriction parameterized over the inclusive power bound. The
/// bound must be positive; zero, greater-than, and unparameterized forms stay unsupported.
fn match_self_cannot_be_blocked_by_power_n_or_less(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    let bound = text
        .strip_prefix(ISSUE_317_CANT_BE_BLOCKED_POWER_PREFIX)?
        .strip_suffix(ISSUE_317_CANT_BE_BLOCKED_POWER_SUFFIX)?
        .parse::<u32>()
        .ok()?;
    (context.source_is_creature && bound >= 1).then(|| {
        RecipeEmission::StaticAbility(IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: context.presentation.clone(),
            definition: StaticAbilityDef::SelfCombatRestriction {
                restriction: tricerules_cards::primitives::CombatRestriction {
                    cant_be_blocked_by: vec![TargetFilter {
                        kind: TargetKind::Creature,
                        power: Some(PowerComparison::AtMost(bound)),
                        ..TargetFilter::default()
                    }],
                    ..tricerules_cards::primitives::CombatRestriction::default()
                },
                condition: None,
            },
        })
    })
}

/// CR 111.10f / 701.16a: "Create a Clue token" is the untargeted registered `clue` token. Only the
/// complete sentence matches; Investigate, plural, tapped, and appended-instruction forms keep
/// their own wording or stay unsupported.
fn match_spell_create_clue_token(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_317_CREATE_CLUE_TOKEN_CLAUSE).then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::CreateTokens {
            token: "clue".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        })
    })
}

/// CR 113.6 / 602.2: an activated ability that functions only while the card is in its owner's
/// graveyard. The matcher captures the printed mana cost and emits the public-zone action as a
/// source-relative return with no targeting, matching Merchant of Many Hats by construction.
fn match_graveyard_return_self_to_hand(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    let cost = exact_mana_cost(text.strip_suffix(ISSUE_317_GRAVEYARD_RETURN_SELF_SUFFIX)?)?;
    Some(RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Graveyard,
        costs: vec![AbilityCost::Mana(cost)],
        effect: vec![SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Source,
        }],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    }))
}

/// Issue #318 exact clause templates. Each template is a reusable typed surface with at least two
/// real positive calibrations. Every clause is compared by the complete normalized Oracle line, so
/// an appended, reordered, or additional-clause form remains unsupported, and source-kind gating
/// stays on the recipes whose printed template requires it.
const ISSUE_318_OWNER_CHOICE_PLACEMENT_CLAUSE: &str =
    "Target creature's owner puts it on their choice of the top or bottom of their library.";
const ISSUE_318_ATTACKING_TARGET_REDUCTION_CLAUSE: &str =
    "This spell costs {1} less to cast if it targets an attacking creature.";
const ISSUE_318_ETB_RAT_TOKEN_CLAUSE: &str =
    "When this creature enters, create a 1/1 black Rat creature token with \"This token can't block.\"";
const ISSUE_318_ETB_FLYING_CLAUSE: &str =
    "When this creature enters, target creature gains flying until end of turn.";
const ISSUE_318_SELF_DAMAGE_DRAW_CLAUSE: &str =
    "Whenever this creature deals damage to an opponent, draw a card.";

/// CR 400.3 / 608.2d: "Target creature's owner puts it on their choice of the top or bottom of
/// their library" is one mandatory creature target whose owner announces the logged placement
/// choice at resolution. Bottom-only, top-only, second-from-top, shuffle, noncreature, and
/// appended-instruction forms stay unsupported.
fn match_spell_owner_choice_top_or_bottom_target_creature(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_318_OWNER_CHOICE_PLACEMENT_CLAUSE).then(|| {
        RecipeEmission::SpellEffectsWithTargeting {
            effects: vec![SpellEffectKind::PutInOwnersLibrary {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                placement: LibraryPlacement::OwnerChoiceTopOrBottom,
            }],
            targeting: exact_targeting(1, 1, "Choose target creature", vec![0]),
        }
    })
}

/// CR 601.2f: "This spell costs {1} less to cast if it targets an attacking creature" reduces the
/// generic component once when an announced target is an attacking creature. Tapped, nontoken,
/// union, amount-2, conditionless, and non-instant/sorcery forms stay unsupported.
fn match_spell_attacking_creature_target_reduction_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    ((context.source_is_instant || context.source_is_sorcery)
        && text == ISSUE_318_ATTACKING_TARGET_REDUCTION_CLAUSE)
        .then(|| {
            RecipeEmission::SpellCostModifier(SpellCostModifier::TargetMatchGenericReduction {
                amount: 1,
                filter: TargetMatchFilter::Battlefield(TargetFilter {
                    kind: TargetKind::Creature,
                    combat_role: Some(CombatRole::Attacking),
                    ..TargetFilter::default()
                }),
            })
        })
}

/// CR 111.1 / 701.6 / 603.2: Edgewall Pack and Voracious Vermin share the exact creature ETB
/// template that creates the registered 1/1 black Rat token with its printed "can't block"
/// restriction. This Rat is not one of the CR 111.10 predefined tokens.
fn match_etb_create_rat_token_cant_block(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_318_ETB_RAT_TOKEN_CLAUSE).then(|| {
        triggered_ability(
            context,
            SpellEffectKind::CreateTokens {
                token: "rat_b_1_1_cant_block".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            },
        )
    })
}

/// CR 611.2a / 514.2: the ETB grants flying to one mandatory creature target until end of turn.
/// Controller-restricted, permanent, plural, and pump-union forms stay unsupported.
fn match_etb_target_creature_gains_flying(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_318_ETB_FLYING_CLAUSE).then(|| {
        targeted_trigger(
            context,
            vec![SpellEffectKind::GrantKeywords {
                subject: chosen_creature(TargetController::Any),
                keywords: vec![Keyword::Flying],
            }],
            1,
            1,
            "Choose target creature",
        )
    })
}

/// CR 603.2 / 120.3: "deals damage to an opponent" is the shipped noncombat-or-combat damage
/// trigger documented for Thieving Magpie; only the exact draw-one body is emitted.
fn match_self_damage_to_opponent_draw_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_318_SELF_DAMAGE_DRAW_CLAUSE).then(|| {
        triggered_ability_with(
            context,
            TriggerCondition::WheneverSelfDealsDamageToOpponent,
            vec![SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
        )
    })
}

const ISSUE_301_REVIEWED_ORACLE_IDS: &[&str] = &[
    "295f8b8d-102a-47af-93c2-f8182c5f11ca", // Ascendant Dustspeaker
    "5d46e85f-4a04-48b1-afe9-3a47678041d4", // Startled Relic Sloth
];

fn issue_301_oracle_id_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        // Catalog calibration contexts intentionally omit Oracle identity. Real card parsing
        // supplies Some(oracle_id), so unknown identities fail closed during generation.
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_301_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

fn match_beginning_of_combat_exile_graveyard_card(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && issue_301_oracle_id_is_reviewed(context)
        && text
            == "At the beginning of combat on your turn, exile up to one target card from a graveyard.")
        .then(|| {
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
                context,
                TriggerCondition::AtBeginningOfCombat {
                    player: CastTriggerPlayer::Controller,
                },
                vec![SpellEffectKind::MoveGraveyardCards {
                    filter: GraveyardFilter {
                        owner: GraveyardOwner::AnyPlayer,
                        ..GraveyardFilter::default()
                    },
                    destination: GraveyardDestination::Exile,
                    linked_exile_id: None,
                }],
            ) else {
                unreachable!("triggered_ability_with always returns a triggered ability")
            };
            ability.targeting = Some(TargetingDef {
                groups: vec![TargetGroupDef {
                    min: 0,
                    max: 1,
                    prompt: "Choose up to one target card from a graveyard".into(),
                    effect_indices: vec![0],
                    distinct_from: Vec::new(),
                    same_graveyard: false,
                    cast_cost_expansion: None,
                }],
            });
            RecipeEmission::TriggeredAbility(ability)
        })
}

const ISSUE_309_STATION_HEADER: &str = r#"Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)"#;
const ISSUE_309_STATION_THRESHOLD: &str = "8+ | Flying";
const ISSUE_309_REVIEWED_ORACLE_IDS: &[&str] = &[
    "7b4c37dc-8cb0-4870-929e-11c2a45952a2", // Uthros Scanship
    "db0894e1-2644-48d4-8de4-8cc43e940bc1", // Debris Field Crusher
];

fn issue_309_context_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_309_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

pub(super) fn issue_309_oracle_id_is_reviewed(oracle_id: &str) -> bool {
    ISSUE_309_REVIEWED_ORACLE_IDS.contains(&oracle_id)
}

fn issue_309_variant(context: &RecipeContext) -> Option<(i64, i64, bool)> {
    match context.oracle_id.as_deref() {
        Some("7b4c37dc-8cb0-4870-929e-11c2a45952a2") => Some((4, 4, false)),
        Some("db0894e1-2644-48d4-8de4-8cc43e940bc1") => Some((1, 5, true)),
        Some(_) => None,
        None => match context.source_name.as_str() {
            "Uthros Scanship" => Some((4, 4, false)),
            "Debris Field Crusher" => Some((1, 5, true)),
            _ => None,
        },
    }
}

pub(super) fn issue_309_card_surface_is_exact(
    oracle_id: &str,
    name: &str,
    mana_cost: &str,
    type_line: &str,
    oracle_text: &str,
    power: Option<&str>,
    toughness: Option<&str>,
) -> bool {
    let expected = match oracle_id {
        "7b4c37dc-8cb0-4870-929e-11c2a45952a2" => (
            "Uthros Scanship",
            "{3}{U}",
            "Artifact — Spacecraft",
            Some("4"),
            Some("4"),
            "When this Spacecraft enters, draw two cards, then discard a card.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
        ),
        "db0894e1-2644-48d4-8de4-8cc43e940bc1" => (
            "Debris Field Crusher",
            "{4}{R}",
            "Artifact — Spacecraft",
            Some("1"),
            Some("5"),
            "When this Spacecraft enters, it deals 3 damage to any target.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying\n{1}{R}: This Spacecraft gets +2/+0 until end of turn.",
        ),
        _ => return true,
    };
    (name, mana_cost, type_line, power, toughness, oracle_text) == expected
}

const ISSUE_310_GALVANIZING_ORACLE_ID: &str = "dfe8f77a-cc26-438b-92ae-2ca7a91f813b";
const ISSUE_310_WEDGELIGHT_ORACLE_ID: &str = "03259ab0-caa8-4620-b070-127e2c712252";
const ISSUE_310_REVIEWED_ORACLE_IDS: &[&str] = &[
    ISSUE_310_GALVANIZING_ORACLE_ID,
    ISSUE_310_WEDGELIGHT_ORACLE_ID,
];
const ISSUE_310_GALVANIZING_STATION_HEADER: &str = r#"Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)"#;
const ISSUE_310_GALVANIZING_THRESHOLD_LINE: &str = "3+ | Flying, haste";
const ISSUE_310_WEDGELIGHT_STATION_HEADER: &str = r#"Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)"#;
const ISSUE_310_WEDGELIGHT_THRESHOLD_LINE: &str = "9+ | Flying, first strike";
const ISSUE_310_WEDGELIGHT_ETB_TEXT: &str =
    "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token.";

#[derive(Debug, Clone, Copy)]
struct Issue310StationVariant {
    threshold: u32,
    base_power: i64,
    base_toughness: i64,
    keywords: &'static [Keyword],
    station_header: &'static str,
    threshold_line: &'static str,
}

const ISSUE_310_GALVANIZING_KEYWORDS: &[Keyword] = &[Keyword::Flying, Keyword::Haste];
const ISSUE_310_WEDGELIGHT_KEYWORDS: &[Keyword] = &[Keyword::Flying, Keyword::FirstStrike];

fn issue_310_context_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_310_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

pub(super) fn issue_310_oracle_id_is_reviewed(oracle_id: &str) -> bool {
    ISSUE_310_REVIEWED_ORACLE_IDS.contains(&oracle_id)
}

fn issue_310_variant(context: &RecipeContext) -> Option<Issue310StationVariant> {
    match context.oracle_id.as_deref() {
        Some(ISSUE_310_GALVANIZING_ORACLE_ID) => Some(Issue310StationVariant {
            threshold: 3,
            base_power: 6,
            base_toughness: 5,
            keywords: ISSUE_310_GALVANIZING_KEYWORDS,
            station_header: ISSUE_310_GALVANIZING_STATION_HEADER,
            threshold_line: ISSUE_310_GALVANIZING_THRESHOLD_LINE,
        }),
        Some(ISSUE_310_WEDGELIGHT_ORACLE_ID) => Some(Issue310StationVariant {
            threshold: 9,
            base_power: 3,
            base_toughness: 4,
            keywords: ISSUE_310_WEDGELIGHT_KEYWORDS,
            station_header: ISSUE_310_WEDGELIGHT_STATION_HEADER,
            threshold_line: ISSUE_310_WEDGELIGHT_THRESHOLD_LINE,
        }),
        Some(_) => None,
        None => match context.source_name.as_str() {
            "Galvanizing Sawship" => Some(Issue310StationVariant {
                threshold: 3,
                base_power: 6,
                base_toughness: 5,
                keywords: ISSUE_310_GALVANIZING_KEYWORDS,
                station_header: ISSUE_310_GALVANIZING_STATION_HEADER,
                threshold_line: ISSUE_310_GALVANIZING_THRESHOLD_LINE,
            }),
            "Wedgelight Rammer" => Some(Issue310StationVariant {
                threshold: 9,
                base_power: 3,
                base_toughness: 4,
                keywords: ISSUE_310_WEDGELIGHT_KEYWORDS,
                station_header: ISSUE_310_WEDGELIGHT_STATION_HEADER,
                threshold_line: ISSUE_310_WEDGELIGHT_THRESHOLD_LINE,
            }),
            _ => None,
        },
    }
}

pub(super) fn issue_310_card_surface_is_exact(
    oracle_id: &str,
    name: &str,
    mana_cost: &str,
    type_line: &str,
    oracle_text: &str,
    power: Option<&str>,
    toughness: Option<&str>,
) -> bool {
    let expected = match oracle_id {
        ISSUE_310_GALVANIZING_ORACLE_ID => (
            "Galvanizing Sawship",
            "{5}{R}",
            "Artifact — Spacecraft",
            Some("6"),
            Some("5"),
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
        ),
        ISSUE_310_WEDGELIGHT_ORACLE_ID => (
            "Wedgelight Rammer",
            "{3}{W}",
            "Artifact — Spacecraft",
            Some("3"),
            Some("4"),
            "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
        ),
        _ => return true,
    };
    (name, mana_cost, type_line, power, toughness, oracle_text) == expected
}

const ISSUE_311_PINNACLE_ORACLE_ID: &str = "ad556aee-3dbf-4c8b-9f3a-31947e26c6f5";
const ISSUE_311_WARMAKER_ORACLE_ID: &str = "c947171b-ed9e-4b83-af45-bd595a8d84ee";
const ISSUE_311_REVIEWED_ORACLE_IDS: &[&str] =
    &[ISSUE_311_PINNACLE_ORACLE_ID, ISSUE_311_WARMAKER_ORACLE_ID];
const ISSUE_311_PINNACLE_STATION_HEADER: &str = r#"Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)"#;
const ISSUE_311_PINNACLE_THRESHOLD_LINE: &str = "7+ | Flying";
const ISSUE_311_WARMAKER_STATION_HEADER: &str = r#"Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 6+.)"#;
const ISSUE_311_WARMAKER_THRESHOLD_LINE: &str = "6+ | Flying";
const ISSUE_311_PINNACLE_ETB_TEXT: &str =
    "When this Spacecraft enters, it deals 10 damage to up to one target creature.";
const ISSUE_311_WARMAKER_ETB_TEXT: &str =
    "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to target creature an opponent controls.";

#[derive(Debug, Clone, Copy)]
struct Issue311StationVariant {
    threshold: u32,
    base_power: i64,
    base_toughness: i64,
    station_header: &'static str,
    threshold_line: &'static str,
}

fn issue_311_context_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_311_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

pub(super) fn issue_311_oracle_id_is_reviewed(oracle_id: &str) -> bool {
    ISSUE_311_REVIEWED_ORACLE_IDS.contains(&oracle_id)
}

fn issue_311_variant(context: &RecipeContext) -> Option<Issue311StationVariant> {
    match context.oracle_id.as_deref() {
        Some(ISSUE_311_PINNACLE_ORACLE_ID) => Some(Issue311StationVariant {
            threshold: 7,
            base_power: 7,
            base_toughness: 7,
            station_header: ISSUE_311_PINNACLE_STATION_HEADER,
            threshold_line: ISSUE_311_PINNACLE_THRESHOLD_LINE,
        }),
        Some(ISSUE_311_WARMAKER_ORACLE_ID) => Some(Issue311StationVariant {
            threshold: 6,
            base_power: 4,
            base_toughness: 3,
            station_header: ISSUE_311_WARMAKER_STATION_HEADER,
            threshold_line: ISSUE_311_WARMAKER_THRESHOLD_LINE,
        }),
        Some(_) => None,
        None => match context.source_name.as_str() {
            "Pinnacle Kill-Ship" => Some(Issue311StationVariant {
                threshold: 7,
                base_power: 7,
                base_toughness: 7,
                station_header: ISSUE_311_PINNACLE_STATION_HEADER,
                threshold_line: ISSUE_311_PINNACLE_THRESHOLD_LINE,
            }),
            "Warmaker Gunship" => Some(Issue311StationVariant {
                threshold: 6,
                base_power: 4,
                base_toughness: 3,
                station_header: ISSUE_311_WARMAKER_STATION_HEADER,
                threshold_line: ISSUE_311_WARMAKER_THRESHOLD_LINE,
            }),
            _ => None,
        },
    }
}

pub(super) fn issue_311_card_surface_is_exact(
    oracle_id: &str,
    name: &str,
    mana_cost: &str,
    type_line: &str,
    oracle_text: &str,
    power: Option<&str>,
    toughness: Option<&str>,
) -> bool {
    let expected = match oracle_id {
        ISSUE_311_PINNACLE_ORACLE_ID => (
            "Pinnacle Kill-Ship",
            "{7}",
            "Artifact — Spacecraft",
            Some("7"),
            Some("7"),
            "When this Spacecraft enters, it deals 10 damage to up to one target creature.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
        ),
        ISSUE_311_WARMAKER_ORACLE_ID => (
            "Warmaker Gunship",
            "{2}{R}",
            "Artifact — Spacecraft",
            Some("4"),
            Some("3"),
            "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to target creature an opponent controls.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 6+.)\n6+ | Flying",
        ),
        _ => return true,
    };
    (name, mana_cost, type_line, power, toughness, oracle_text) == expected
}

const ISSUE_313_EXTINGUISHER_ORACLE_ID: &str = "cce3dcc3-57bb-4b95-8b70-337c67bb3c4e";
const ISSUE_313_FELL_ORACLE_ID: &str = "1a82be68-3b74-4dfc-9068-3abea61db709";
const ISSUE_313_REVIEWED_ORACLE_IDS: &[&str] =
    &[ISSUE_313_EXTINGUISHER_ORACLE_ID, ISSUE_313_FELL_ORACLE_ID];
const ISSUE_313_EXTINGUISHER_STATION_HEADER: &str = r#"Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)"#;
const ISSUE_313_EXTINGUISHER_THRESHOLD_LINE: &str = "5+ | Flying, trample";
const ISSUE_313_FELL_STATION_HEADER: &str = r#"Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)"#;
const ISSUE_313_FELL_THRESHOLD_LINE: &str = "8+ | Flying, lifelink";
const ISSUE_313_EXTINGUISHER_ETB_TEXT: &str =
    "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.";
const ISSUE_313_FELL_ETB_TEXT: &str =
    "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.";

#[derive(Debug, Clone, Copy)]
struct Issue313StationVariant {
    threshold: u32,
    base_power: i64,
    base_toughness: i64,
    keywords: &'static [Keyword],
    station_header: &'static str,
    threshold_line: &'static str,
}

const ISSUE_313_EXTINGUISHER_KEYWORDS: &[Keyword] = &[Keyword::Flying, Keyword::Trample];
const ISSUE_313_FELL_KEYWORDS: &[Keyword] = &[Keyword::Flying, Keyword::Lifelink];

fn issue_313_context_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_313_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

pub(super) fn issue_313_oracle_id_is_reviewed(oracle_id: &str) -> bool {
    ISSUE_313_REVIEWED_ORACLE_IDS.contains(&oracle_id)
}

fn issue_313_variant(context: &RecipeContext) -> Option<Issue313StationVariant> {
    match context.oracle_id.as_deref() {
        Some(ISSUE_313_EXTINGUISHER_ORACLE_ID) => Some(Issue313StationVariant {
            threshold: 5,
            base_power: 10,
            base_toughness: 10,
            keywords: ISSUE_313_EXTINGUISHER_KEYWORDS,
            station_header: ISSUE_313_EXTINGUISHER_STATION_HEADER,
            threshold_line: ISSUE_313_EXTINGUISHER_THRESHOLD_LINE,
        }),
        Some(ISSUE_313_FELL_ORACLE_ID) => Some(Issue313StationVariant {
            threshold: 8,
            base_power: 3,
            base_toughness: 2,
            keywords: ISSUE_313_FELL_KEYWORDS,
            station_header: ISSUE_313_FELL_STATION_HEADER,
            threshold_line: ISSUE_313_FELL_THRESHOLD_LINE,
        }),
        Some(_) => None,
        None => match context.source_name.as_str() {
            "Extinguisher Battleship" => Some(Issue313StationVariant {
                threshold: 5,
                base_power: 10,
                base_toughness: 10,
                keywords: ISSUE_313_EXTINGUISHER_KEYWORDS,
                station_header: ISSUE_313_EXTINGUISHER_STATION_HEADER,
                threshold_line: ISSUE_313_EXTINGUISHER_THRESHOLD_LINE,
            }),
            "Fell Gravship" => Some(Issue313StationVariant {
                threshold: 8,
                base_power: 3,
                base_toughness: 2,
                keywords: ISSUE_313_FELL_KEYWORDS,
                station_header: ISSUE_313_FELL_STATION_HEADER,
                threshold_line: ISSUE_313_FELL_THRESHOLD_LINE,
            }),
            _ => None,
        },
    }
}

pub(super) fn issue_313_card_surface_is_exact(
    oracle_id: &str,
    name: &str,
    mana_cost: &str,
    type_line: &str,
    oracle_text: &str,
    power: Option<&str>,
    toughness: Option<&str>,
) -> bool {
    let expected = match oracle_id {
        ISSUE_313_EXTINGUISHER_ORACLE_ID => (
            "Extinguisher Battleship",
            "{8}",
            "Artifact — Spacecraft",
            Some("10"),
            Some("10"),
            "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
        ),
        ISSUE_313_FELL_ORACLE_ID => (
            "Fell Gravship",
            "{2}{B}",
            "Artifact — Spacecraft",
            Some("3"),
            Some("2"),
            "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying, lifelink",
        ),
        _ => return true,
    };
    (name, mana_cost, type_line, power, toughness, oracle_text) == expected
}

fn issue_313_spacecraft_context(context: &RecipeContext) -> bool {
    context.source_is_artifact
        && context.source_is_permanent
        && context.source_is_spacecraft_or_planet
        && issue_313_context_is_reviewed(context)
}

fn issue_313_station_assembly(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    if !issue_313_spacecraft_context(context) {
        return None;
    }
    let variant = issue_313_variant(context)?;
    let lines = external_oracle_lines(text);
    if lines.len() != 2 || lines[0] != variant.station_header || lines[1] != variant.threshold_line
    {
        return None;
    }
    let station_line = match &context.presentation {
        AbilityPresentation::OracleLines(lines) if lines.len() == 1 && lines[0] > 0 => lines[0],
        _ => return None,
    };
    let threshold_line = station_line.checked_add(1)?;
    Some(RecipeEmission::StationAssembly(StationAssemblyEmission {
        activated_ability: ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::ExactCount(1),
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
                exclude_source: true,
            }],
            cost_modifiers: Vec::new(),
            effect: vec![SpellEffectKind::PutCounters {
                counter: CounterKind::Charge,
                count: Amount::Count(CountExpression::CardResultCharacteristicSum {
                    filter: CardResultFilter {
                        source: CardResultSource::Payment,
                        action: CardResultAction::Tap,
                        players: RelativePlayerSet::Controller,
                        card_type: Some(CardTypeFilter::Creature),
                    },
                    characteristic: PowerToughnessCharacteristic::Power,
                }),
                subject: EffectSubject::Source,
            }],
            targeting: None,
            timing: ActivationTiming::SorcerySpeed,
            conditions: Vec::new(),
            activation_limit: None,
        },
        static_ability: IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: AbilityPresentation::OracleLines(vec![threshold_line]),
            definition: StaticAbilityDef::ConditionalSelfModifier {
                condition: GameCondition::SourceCounterCount {
                    counter: CounterKind::Charge,
                    min: Some(variant.threshold),
                    max: None,
                },
                set_types: None,
                add_types: TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Creature],
                    creature_types: Vec::new(),
                },
                base_power: Some(variant.base_power),
                base_toughness: Some(variant.base_toughness),
                delta_power: 0,
                delta_toughness: 0,
                keywords: variant.keywords.to_vec(),
                activated_abilities: Vec::new(),
                triggered_abilities: Vec::new(),
                can_attack_as_though_without_defender: false,
            },
        },
    }))
}

fn issue_313_is_extinguisher(context: &RecipeContext) -> bool {
    match context.oracle_id.as_deref() {
        Some(ISSUE_313_EXTINGUISHER_ORACLE_ID) => true,
        Some(_) => false,
        None => context.source_name == "Extinguisher Battleship",
    }
}

fn issue_313_is_fell(context: &RecipeContext) -> bool {
    match context.oracle_id.as_deref() {
        Some(ISSUE_313_FELL_ORACLE_ID) => true,
        Some(_) => false,
        None => context.source_name == "Fell Gravship",
    }
}

fn match_extinguisher_etb_destroy_then_damage(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (issue_313_spacecraft_context(context)
        && issue_313_is_extinguisher(context)
        && text == ISSUE_313_EXTINGUISHER_ETB_TEXT)
        .then(|| {
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
                vec![
                    SpellEffectKind::Destroy {
                        subject: EffectSubject::Chosen(Box::new(TargetFilter {
                            kind: TargetKind::AnyPermanent,
                            excluded_permanent_types: vec![PermanentTypeFilter::Creature],
                            ..TargetFilter::default()
                        })),
                    },
                    SpellEffectKind::DamageAll {
                        amount: Amount::Fixed(4),
                        players: RelativePlayerSet::All,
                        kind: TargetFilter::default_creature(),
                    },
                ],
            ) else {
                unreachable!("triggered_ability_with always returns a triggered ability")
            };
            ability.targeting = Some(exact_targeting(
                1,
                1,
                "Choose target noncreature permanent",
                vec![0],
            ));
            RecipeEmission::TriggeredAbility(ability)
        })
}

fn match_fell_etb_mill_then_return(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (issue_313_spacecraft_context(context)
        && issue_313_is_fell(context)
        && text == ISSUE_313_FELL_ETB_TEXT)
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WhenSelfEntersBattlefield,
                vec![
                    SpellEffectKind::Mill {
                        count: Amount::Fixed(3),
                        who: PlayerRecipient::Controller,
                    },
                    SpellEffectKind::ChooseGraveyardCard {
                        filter: ZoneCardFilter {
                            any_of: Some(vec![
                                ZoneCardFilter {
                                    card_type: Some(CardTypeFilter::Creature),
                                    ..ZoneCardFilter::default()
                                },
                                ZoneCardFilter {
                                    required_subtypes: vec!["Spacecraft".into()],
                                    ..ZoneCardFilter::default()
                                },
                            ]),
                            ..ZoneCardFilter::default()
                        },
                        destination: GraveyardDestination::Hand,
                        optional: false,
                        from_result: None,
                    },
                ],
            )
        })
}

pub(super) fn match_station_6_7_flying_assembly(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    if !context.source_is_artifact
        || !context.source_is_permanent
        || !context.source_is_spacecraft_or_planet
        || !issue_311_context_is_reviewed(context)
    {
        return None;
    }
    let variant = issue_311_variant(context)?;
    let lines = external_oracle_lines(text);
    if lines.len() != 2 || lines[0] != variant.station_header || lines[1] != variant.threshold_line
    {
        return None;
    }
    let station_line = match &context.presentation {
        AbilityPresentation::OracleLines(lines) if lines.len() == 1 && lines[0] > 0 => lines[0],
        _ => return None,
    };
    let threshold_line = station_line.checked_add(1)?;
    Some(RecipeEmission::StationAssembly(StationAssemblyEmission {
        activated_ability: ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::ExactCount(1),
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
                exclude_source: true,
            }],
            cost_modifiers: Vec::new(),
            effect: vec![SpellEffectKind::PutCounters {
                counter: CounterKind::Charge,
                count: Amount::Count(CountExpression::CardResultCharacteristicSum {
                    filter: CardResultFilter {
                        source: CardResultSource::Payment,
                        action: CardResultAction::Tap,
                        players: RelativePlayerSet::Controller,
                        card_type: Some(CardTypeFilter::Creature),
                    },
                    characteristic: PowerToughnessCharacteristic::Power,
                }),
                subject: EffectSubject::Source,
            }],
            targeting: None,
            timing: ActivationTiming::SorcerySpeed,
            conditions: Vec::new(),
            activation_limit: None,
        },
        static_ability: IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: AbilityPresentation::OracleLines(vec![threshold_line]),
            definition: StaticAbilityDef::ConditionalSelfModifier {
                condition: GameCondition::SourceCounterCount {
                    counter: CounterKind::Charge,
                    min: Some(variant.threshold),
                    max: None,
                },
                set_types: None,
                add_types: TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Creature],
                    creature_types: Vec::new(),
                },
                base_power: Some(variant.base_power),
                base_toughness: Some(variant.base_toughness),
                delta_power: 0,
                delta_toughness: 0,
                keywords: vec![Keyword::Flying],
                activated_abilities: Vec::new(),
                triggered_abilities: Vec::new(),
                can_attack_as_though_without_defender: false,
            },
        },
    }))
}

fn issue_311_is_pinnacle(context: &RecipeContext) -> bool {
    match context.oracle_id.as_deref() {
        Some(ISSUE_311_PINNACLE_ORACLE_ID) => true,
        Some(_) => false,
        None => context.source_name == "Pinnacle Kill-Ship",
    }
}

fn issue_311_is_warmaker(context: &RecipeContext) -> bool {
    match context.oracle_id.as_deref() {
        Some(ISSUE_311_WARMAKER_ORACLE_ID) => true,
        Some(_) => false,
        None => context.source_name == "Warmaker Gunship",
    }
}

fn issue_311_spacecraft_context(context: &RecipeContext) -> bool {
    context.source_is_artifact
        && context.source_is_permanent
        && context.source_is_spacecraft_or_planet
        && issue_311_context_is_reviewed(context)
}

fn match_pinnacle_etb_damage_ten_up_to_one_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (issue_311_spacecraft_context(context)
        && issue_311_is_pinnacle(context)
        && text == ISSUE_311_PINNACLE_ETB_TEXT)
        .then(|| {
            targeted_trigger(
                context,
                vec![SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(10),
                    target: TargetFilter::default_creature(),
                }],
                0,
                1,
                "Choose up to one target creature",
            )
        })
}

fn match_warmaker_etb_damage_artifact_count_opponent_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (issue_311_spacecraft_context(context)
        && issue_311_is_warmaker(context)
        && text == ISSUE_311_WARMAKER_ETB_TEXT)
        .then(|| {
            targeted_trigger(
                context,
                vec![SpellEffectKind::DamageTarget {
                    amount: Amount::Count(CountExpression::BattlefieldPermanents {
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
                    }),
                    target: TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::Opponent,
                        ..TargetFilter::default()
                    },
                }],
                1,
                1,
                "Choose target creature an opponent controls",
            )
        })
}

pub(super) fn match_station_3_or_9_keyword_assembly(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    if !context.source_is_artifact
        || !context.source_is_permanent
        || !context.source_is_spacecraft_or_planet
        || !issue_310_context_is_reviewed(context)
    {
        return None;
    }
    let variant = issue_310_variant(context)?;
    let lines = external_oracle_lines(text);
    if lines.len() != 2 || lines[0] != variant.station_header || lines[1] != variant.threshold_line
    {
        return None;
    }
    let station_line = match &context.presentation {
        AbilityPresentation::OracleLines(lines) if lines.len() == 1 && lines[0] > 0 => lines[0],
        _ => return None,
    };
    let threshold_line = station_line.checked_add(1)?;
    Some(RecipeEmission::StationAssembly(StationAssemblyEmission {
        activated_ability: ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::ExactCount(1),
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
                exclude_source: true,
            }],
            cost_modifiers: Vec::new(),
            effect: vec![SpellEffectKind::PutCounters {
                counter: CounterKind::Charge,
                count: Amount::Count(CountExpression::CardResultCharacteristicSum {
                    filter: CardResultFilter {
                        source: CardResultSource::Payment,
                        action: CardResultAction::Tap,
                        players: RelativePlayerSet::Controller,
                        card_type: Some(CardTypeFilter::Creature),
                    },
                    characteristic: PowerToughnessCharacteristic::Power,
                }),
                subject: EffectSubject::Source,
            }],
            targeting: None,
            timing: ActivationTiming::SorcerySpeed,
            conditions: Vec::new(),
            activation_limit: None,
        },
        static_ability: IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: AbilityPresentation::OracleLines(vec![threshold_line]),
            definition: StaticAbilityDef::ConditionalSelfModifier {
                condition: GameCondition::SourceCounterCount {
                    counter: CounterKind::Charge,
                    min: Some(variant.threshold),
                    max: None,
                },
                set_types: None,
                add_types: TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Creature],
                    creature_types: Vec::new(),
                },
                base_power: Some(variant.base_power),
                base_toughness: Some(variant.base_toughness),
                delta_power: 0,
                delta_toughness: 0,
                keywords: variant.keywords.to_vec(),
                activated_abilities: Vec::new(),
                triggered_abilities: Vec::new(),
                can_attack_as_though_without_defender: false,
            },
        },
    }))
}

fn issue_310_is_wedgelight(context: &RecipeContext) -> bool {
    match context.oracle_id.as_deref() {
        Some(ISSUE_310_WEDGELIGHT_ORACLE_ID) => true,
        Some(_) => false,
        None => context.source_name == "Wedgelight Rammer",
    }
}

fn match_wedgelight_etb_create_robot(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact
        && context.source_is_permanent
        && context.source_is_spacecraft_or_planet
        && issue_310_is_wedgelight(context)
        && text == ISSUE_310_WEDGELIGHT_ETB_TEXT)
        .then(|| {
            triggered_ability(
                context,
                SpellEffectKind::CreateTokens {
                    token: "robot_c_2_2".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                },
            )
        })
}

pub(super) fn match_station_8_flying_assembly(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    if !context.source_is_artifact
        || !context.source_is_spacecraft_or_planet
        || !issue_309_context_is_reviewed(context)
    {
        return None;
    }
    let lines = external_oracle_lines(text);
    let [header, threshold] = lines.as_slice() else {
        return None;
    };
    if header != ISSUE_309_STATION_HEADER || threshold != ISSUE_309_STATION_THRESHOLD {
        return None;
    }
    let (base_power, base_toughness, _) = issue_309_variant(context)?;
    let station_line = match &context.presentation {
        AbilityPresentation::OracleLines(lines) if lines.len() == 1 && lines[0] > 0 => lines[0],
        _ => return None,
    };
    let threshold_line = station_line.checked_add(1)?;
    Some(RecipeEmission::StationAssembly(StationAssemblyEmission {
        activated_ability: ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::ExactCount(1),
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
                exclude_source: true,
            }],
            cost_modifiers: Vec::new(),
            effect: vec![SpellEffectKind::PutCounters {
                counter: CounterKind::Charge,
                count: Amount::Count(CountExpression::CardResultCharacteristicSum {
                    filter: CardResultFilter {
                        source: CardResultSource::Payment,
                        action: CardResultAction::Tap,
                        players: RelativePlayerSet::Controller,
                        card_type: Some(CardTypeFilter::Creature),
                    },
                    characteristic: PowerToughnessCharacteristic::Power,
                }),
                subject: EffectSubject::Source,
            }],
            targeting: None,
            timing: ActivationTiming::SorcerySpeed,
            conditions: Vec::new(),
            activation_limit: None,
        },
        static_ability: IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: AbilityPresentation::OracleLines(vec![threshold_line]),
            definition: StaticAbilityDef::ConditionalSelfModifier {
                condition: GameCondition::SourceCounterCount {
                    counter: CounterKind::Charge,
                    min: Some(8),
                    max: None,
                },
                set_types: None,
                add_types: TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Creature],
                    creature_types: Vec::new(),
                },
                base_power: Some(base_power),
                base_toughness: Some(base_toughness),
                delta_power: 0,
                delta_toughness: 0,
                keywords: vec![Keyword::Flying],
                activated_abilities: Vec::new(),
                triggered_abilities: Vec::new(),
                can_attack_as_though_without_defender: false,
            },
        },
    }))
}

fn issue_309_is_uthros(context: &RecipeContext) -> bool {
    issue_309_variant(context).is_some_and(|(_, _, debris)| !debris)
}

fn issue_309_is_debris(context: &RecipeContext) -> bool {
    issue_309_variant(context).is_some_and(|(_, _, debris)| debris)
}

fn match_uthros_etb_draw_two_discard_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact
        && context.source_is_spacecraft_or_planet
        && issue_309_context_is_reviewed(context)
        && issue_309_is_uthros(context)
        && text == "When this Spacecraft enters, draw two cards, then discard a card.")
        .then(|| {
            triggered_ability(
                context,
                SpellEffectKind::DrawDiscard {
                    who: PlayerRecipient::Controller,
                    draw_count: 2,
                    discard_count: 1,
                    order: DrawDiscardOrder::DrawThenDiscard,
                    optional: false,
                },
            )
        })
}

fn match_debris_etb_damage_three_any_target(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_artifact
        && context.source_is_spacecraft_or_planet
        && issue_309_context_is_reviewed(context)
        && issue_309_is_debris(context)
        && text == "When this Spacecraft enters, it deals 3 damage to any target.")
        .then(|| {
            targeted_trigger(
                context,
                vec![SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(3),
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

fn match_debris_pump_two_power(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_artifact
        && context.source_is_spacecraft_or_planet
        && issue_309_context_is_reviewed(context)
        && issue_309_is_debris(context)
        && text == "{1}{R}: This Spacecraft gets +2/+0 until end of turn.")
        .then(|| {
            utility_activated_ability(
                context,
                vec![fixed_mana_cost("{1}{R}")],
                vec![SpellEffectKind::PumpTarget {
                    power: 2,
                    toughness: 0,
                    scale: None,
                    subject: EffectSubject::Source,
                }],
                None,
            )
        })
}

fn match_etb_look_top_three_optional_top_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && issue_290_oracle_id_is_reviewed(context)
        && text == "When this creature enters, look at the top three cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.")
        .then(|| {
            triggered_ability(
                context,
                SpellEffectKind::LibraryPartition {
                    count: 3,
                    top_min: 0,
                    top_max: Some(1),
                    kind: LibraryPartitionKind::Look,
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

fn match_self_attacks_pump_other_controlled_creature_indestructible(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && text
            == "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn.")
        .then(|| {
            let target = TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                excluded_objects: vec![TargetObjectExclusion::Source],
                ..TargetFilter::default()
            };
            let RecipeEmission::TriggeredAbility(mut ability) = triggered_ability_with(
                context,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0,
                },
                vec![
                    SpellEffectKind::PumpTarget {
                        power: 1,
                        toughness: 0,
                        scale: None,
                        subject: EffectSubject::Chosen(Box::new(target.clone())),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: EffectSubject::Chosen(Box::new(target)),
                        keywords: vec![Keyword::Indestructible],
                    },
                ],
            ) else {
                unreachable!("triggered_ability_with always returns a triggered ability")
            };
            ability.targeting = Some(exact_targeting(
                1,
                1,
                "Choose another target creature you control",
                vec![0, 1],
            ));
            RecipeEmission::TriggeredAbility(ability)
        })
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

fn match_graveyard_card_to_library_bottom(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_permanent
        && text == "{2}: Put target card from your graveyard on the bottom of your library.")
        .then(|| {
            utility_activated_ability(
                context,
                vec![fixed_mana_cost("{2}")],
                vec![SpellEffectKind::MoveGraveyardCards {
                    filter: GraveyardFilter {
                        owner: GraveyardOwner::Controller,
                        ..GraveyardFilter::default()
                    },
                    destination: GraveyardDestination::LibraryBottom,
                    linked_exile_id: None,
                }],
                single_targeting("Choose target card from your graveyard"),
            )
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

const ISSUE_289_SKYRAY_ORACLE_ID: &str = "3a46d85b-ce1a-4842-a342-92a5bddb1053";
const ISSUE_289_MAKO_ORACLE_ID: &str = "e349be42-5f14-44a9-9608-281985c10e2d";
const ISSUE_289_REVIEWED_ORACLE_IDS: &[&str] =
    &[ISSUE_289_SKYRAY_ORACLE_ID, ISSUE_289_MAKO_ORACLE_ID];
/// The exact printing-independent Oracle header of both cohort cards: the discard-batch trigger
/// is bound to reviewed identities so an unreviewed card with the same clause stays fail-closed.
const ISSUE_289_DISCARD_BATCH_CLAUSE: &str =
    "Whenever you discard one or more cards, put that many +1/+1 counters on this creature.";

fn issue_289_context_is_reviewed(context: &RecipeContext) -> bool {
    context
        .oracle_id
        .as_deref()
        .is_none_or(|oracle_id| ISSUE_289_REVIEWED_ORACLE_IDS.contains(&oracle_id))
}

pub(super) fn issue_289_oracle_id_is_reviewed(oracle_id: &str) -> bool {
    ISSUE_289_REVIEWED_ORACLE_IDS.contains(&oracle_id)
}

pub(super) fn issue_289_card_surface_is_exact(
    oracle_id: &str,
    name: &str,
    mana_cost: &str,
    type_line: &str,
    oracle_text: &str,
    power: Option<&str>,
    toughness: Option<&str>,
) -> bool {
    let expected = match oracle_id {
        ISSUE_289_SKYRAY_ORACLE_ID => (
            "Scrounging Skyray",
            "{1}{U}",
            "Creature — Fish Pirate",
            Some("1"),
            Some("2"),
            "Flying\nWhenever you discard one or more cards, put that many +1/+1 counters on this creature.\nCycling {2} ({2}, Discard this card: Draw a card.)",
        ),
        ISSUE_289_MAKO_ORACLE_ID => (
            "Marauding Mako",
            "{R}",
            "Creature — Shark Pirate",
            Some("1"),
            Some("1"),
            "Whenever you discard one or more cards, put that many +1/+1 counters on this creature.\nCycling {2} ({2}, Discard this card: Draw a card.)",
        ),
        _ => return true,
    };
    (name, mana_cost, type_line, power, toughness, oracle_text) == expected
}

/// Scrounging Skyray and Marauding Mako share one exact Oracle clause: the event groups every
/// card a single discard action committed, and the counter amount is that committed count
/// (CR 603.2c, 608.2h, 701.9).
pub(super) fn match_discard_batch_counter_trigger(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature
        && issue_289_context_is_reviewed(context)
        && text == ISSUE_289_DISCARD_BATCH_CLAUSE)
        .then(|| {
            triggered_ability_with(
                context,
                TriggerCondition::WheneverPlayerDiscardsOneOrMoreCards {
                    player: CastTriggerPlayer::Controller,
                },
                vec![SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::EventCount,
                    subject: EffectSubject::Source,
                }],
            )
        })
}

/// Issue #323 exact clause templates. Each template is a reusable typed surface with at least two
/// real positive calibrations. Every clause is compared by the complete normalized Oracle line, so
/// an appended, reordered, or additional-clause form remains unsupported, and source-kind gating
/// stays on the recipes whose printed template requires it.
const ISSUE_323_DESTROY_ALL_CREATURES_CLAUSE: &str = "Destroy all creatures.";
const ISSUE_323_COUNTER_UNLESS_PAYS_TWO_CLAUSE: &str =
    "Counter target spell unless its controller pays {2}.";
const ISSUE_323_DOUBLE_STRIKE_GRANT_CLAUSE: &str =
    "Target creature gains double strike until end of turn.";
const ISSUE_323_CREATE_TREASURE_TOKEN_CLAUSE: &str = "Create a Treasure token.";
const ISSUE_323_SEARCH_BASIC_LAND_TAPPED_CLAUSE: &str =
    "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.";
const ISSUE_323_OTHER_CREATURES_TRAMPLE_CLAUSE: &str = "Other creatures you control have trample.";
const ISSUE_323_CANT_BE_BLOCKED_THIS_TURN_CLAUSE: &str =
    "Target creature can't be blocked this turn.";
const ISSUE_323_MAXIMUM_ONE_BLOCKER_CLAUSE: &str =
    "This creature can't be blocked by more than one creature.";

/// CR 701.7 / 614.1: "Destroy all creatures" is the untargeted mass-destruction instruction
/// (Wrath of God, Day of Judgment) that emits `DestroyAll` with the default `Creature` kind and no
/// target group. Filtered sweeps ("with flying", "you don't control"), the can't-be-regenerated
/// rider, and non-`Creature` sweeps stay unsupported.
fn match_spell_destroy_all_creatures(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_323_DESTROY_ALL_CREATURES_CLAUSE).then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::DestroyAll {
            kind: TargetFilter::default_creature(),
            prevent_regeneration: false,
        })
    })
}

/// CR 701.6 / 118.12a: "unless its controller pays {2}" publishes a fixed generic cost the
/// countered spell's controller may pay. Other amounts, filtered stack targets, and the
/// per-card/permanent variants stay unsupported.
fn match_spell_counter_unless_pays_two(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_323_COUNTER_UNLESS_PAYS_TWO_CLAUSE).then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::CounterTargetSpell {
            spell_filter: StackSpellFilter::default(),
            unless_controller_pays: Some(Amount::Fixed(2)),
            unless_controller_pays_by_cast_cost: None,
        })
    })
}

/// CR 702.4 / 611.2a: "Target creature gains double strike until end of turn" is one mandatory
/// creature target bound to a single keyword grant. Permanent grants, union grants, power pumps,
/// and untap riders stay unsupported.
fn match_spell_grant_double_strike(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_323_DOUBLE_STRIKE_GRANT_CLAUSE).then(|| {
        RecipeEmission::SpellEffectsWithTargeting {
            effects: vec![SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                keywords: vec![Keyword::DoubleStrike],
            }],
            targeting: exact_targeting(1, 1, "Choose target creature", vec![0]),
        }
    })
}

/// CR 111.10a: "Create a Treasure token" makes the registered predefined Treasure with its
/// sacrifice-for-mana ability. Plural, tapped, and appended-instruction forms stay unsupported;
/// the ETB and attack-trigger Treasure recipes are separate surfaces.
fn match_spell_create_treasure_token(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_323_CREATE_TREASURE_TOKEN_CLAUSE).then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::CreateTokens {
            token: "treasure".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        })
    })
}

/// CR 701.23 / 614.1d: search the controller's library for one basic land, put it onto the
/// battlefield tapped, then shuffle. Reveal-to-hand, untapped, nonbasic, up-to-two, and optional
/// forms stay unsupported; the land-sacrifice and landfall recipes are separate surfaces.
fn match_spell_search_basic_land_battlefield_tapped(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_323_SEARCH_BASIC_LAND_TAPPED_CLAUSE).then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::SearchLibrary {
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
        })
    })
}

/// CR 611.3 layer 6: an anthem granting trample to every other creature the source's controller
/// controls, excluding the source itself. "Creatures you control", other keywords, subtype/power
/// scopes, and noncreature sources stay unsupported.
fn match_static_other_creatures_trample(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_323_OTHER_CREATURES_TRAMPLE_CLAUSE).then(|| {
        RecipeEmission::StaticAbility(IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: context.presentation.clone(),
            definition: StaticAbilityDef::AnthemKeyword {
                filter: CreatureScopeFilter {
                    controller: Some(CreatureScopeController::YouControl),
                    exclude_self: true,
                    ..CreatureScopeFilter::default()
                },
                condition: None,
                keyword: Keyword::Trample,
            },
        })
    })
}

/// CR 509.1b / 611.2c: one mandatory creature target becomes unblockable until cleanup through the
/// shared combat-restriction path. "Can't block", "this combat", permanent, and optional/plural
/// forms stay unsupported.
fn match_spell_target_creature_cant_be_blocked(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_323_CANT_BE_BLOCKED_THIS_TURN_CLAUSE).then(|| {
        RecipeEmission::SpellEffectsWithTargeting {
            effects: vec![SpellEffectKind::ApplyCombatRestriction {
                scope: CombatRestrictionScope::Chosen(TargetFilter::default_creature()),
                restriction: CombatRestriction {
                    cant_be_blocked: true,
                    ..CombatRestriction::default()
                },
            }],
            targeting: exact_targeting(1, 1, "Choose target creature", vec![0]),
        }
    })
}

/// CR 509.1b: the source creature can be blocked by at most one creature through the shared
/// combat-restriction path. Total unblockability, other maximums, power/subtype filters, the
/// except-by-two form, and noncreature sources stay unsupported.
fn match_static_self_max_one_blocker(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_323_MAXIMUM_ONE_BLOCKER_CLAUSE).then(|| {
        RecipeEmission::StaticAbility(IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: context.presentation.clone(),
            definition: StaticAbilityDef::SelfCombatRestriction {
                restriction: CombatRestriction {
                    maximum_blockers: Some(1),
                    ..CombatRestriction::default()
                },
                condition: None,
            },
        })
    })
}

/// Issue #327 exact clause templates. Each template is a reusable typed surface with at least two
/// real positive calibrations. Every clause is compared by the complete normalized Oracle line, so
/// an appended, reordered, or additional-clause form remains unsupported, and source-kind gating
/// stays on the recipes whose printed template requires it.
const ISSUE_327_PUT_COUNTER_TARGET_CLAUSE: &str = "Put a +1/+1 counter on target creature.";
const ISSUE_327_MINUS_FOUR_MINUS_FOUR_CLAUSE: &str =
    "Target creature gets -4/-4 until end of turn.";
const ISSUE_327_ATTACKING_ANTHEM_CLAUSE: &str = "Attacking creatures you control get +1/+0.";
const ISSUE_327_END_STEP_SACRIFICE_CLAUSE: &str =
    "At the beginning of the end step, sacrifice this creature.";
const ISSUE_327_TAP_DISCARD_DRAW_CLAUSE: &str = "{T}, Discard a card: Draw a card.";
const ISSUE_327_ETB_MASS_COUNTER_CLAUSE: &str =
    "When this creature enters, put a +1/+1 counter on each other creature you control.";
const ISSUE_327_DIES_COUNTER_CLAUSE: &str =
    "Whenever another creature you control dies, put a +1/+1 counter on this creature.";

/// A single colored mana symbol in Scryfall brace syntax, used by the parameterized self-pump
/// template. Hybrid, generic, colorless, and multi-symbol costs stay unsupported.
fn issue_327_single_colored_symbol(cost: &str) -> Option<ManaCost> {
    let symbol = cost.strip_prefix('{')?.strip_suffix('}')?;
    if symbol.len() != 1 || !matches!(symbol, "W" | "U" | "B" | "R" | "G") {
        return None;
    }
    ManaCost::parse(cost).ok()
}

/// CR 122: "Put a +1/+1 counter on target creature" is one mandatory creature target bound to a
/// single counter placement, the standalone-spell sibling of the modal counter payload. "Each",
/// plural, controlled-only, up-to-one, and -1/-1 forms, and appended instructions stay unsupported.
fn match_spell_put_plus_one_counter_target_creature(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_327_PUT_COUNTER_TARGET_CLAUSE).then(|| {
        RecipeEmission::SpellEffectsWithTargeting {
            effects: vec![SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            }],
            targeting: exact_targeting(1, 1, "Choose target creature", vec![0]),
        }
    })
}

/// CR 602 / 611.2a: a single colored mana symbol buys one +1/+0 pump for the source creature until
/// cleanup. Generic, multi-symbol, hybrid, colorless, and multicolor costs, other pump values,
/// targeted subjects, tap riders, and timing restrictions stay unsupported.
fn match_activated_creature_self_pump_plus_one_zero(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    if !context.source_is_creature {
        return None;
    }
    let cost = text.strip_suffix(": This creature gets +1/+0 until end of turn.")?;
    let mana = issue_327_single_colored_symbol(cost)?;
    Some(RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Mana(mana)],
        effect: vec![SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 0,
            scale: None,
            subject: EffectSubject::Source,
        }],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    }))
}

/// CR 611.2a: a -4/-4 until-cleanup pump on one mandatory creature target, mirroring the shipped
/// -2/-2 recipe. Other values, asymmetric splits, positive pumps, riders, up-to-one, and
/// controlled/opponent-restricted targets stay unsupported.
fn match_spell_creature_minus_four_four(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_327_MINUS_FOUR_MINUS_FOUR_CLAUSE).then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::PumpTarget {
            power: -4,
            toughness: -4,
            scale: None,
            subject: chosen_creature(TargetController::Any),
        })
    })
}

/// CR 611.3 / 613.4c (attacking per CR 508.1k): an enchantment or creature anthem granting +1/+0 to
/// each attacking creature its controller controls. The `attacking` scope re-evaluates
/// continuously. Other values, nonattacking scopes, exclusion, opponent scope, temporary pumps, and
/// other source kinds stay unsupported.
fn match_static_attacking_creatures_anthem_plus_one_zero(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    ((context.source_is_enchantment || context.source_is_creature)
        && text == ISSUE_327_ATTACKING_ANTHEM_CLAUSE)
        .then(|| {
            RecipeEmission::StaticAbility(IdentifiedAbility {
                ability_id: context.static_ability_id.clone(),
                presentation: context.presentation.clone(),
                definition: StaticAbilityDef::AnthemPt {
                    filter: CreatureScopeFilter {
                        controller: Some(CreatureScopeController::YouControl),
                        attacking: true,
                        ..CreatureScopeFilter::default()
                    },
                    condition: None,
                    delta_power: 1,
                    delta_toughness: 0,
                },
            })
        })
}

/// CR 603.2b / 513.2 / 701.21: "at the beginning of the end step" is an ordinary triggered ability
/// whose trigger event is the beginning of an end step, so it triggers at the beginning of any end
/// step while the source is on the battlefield (CR 603.2c); `AnyPlayer` is the faithful relative
/// scope. "At the beginning of the next end step" is instead a delayed one-shot (CR 603.7b) and
/// stays unsupported, as does "your end step"; the rules-equivalent wording "each end step" is also
/// left unsupported rather than folded into this exact printed template. The source is sacrificed
/// as a semantic action, and noncreature sources stay unsupported.
fn match_triggered_end_step_sacrifice_self(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_327_END_STEP_SACRIFICE_CLAUSE).then(|| {
        triggered_ability_with(
            context,
            TriggerCondition::AtBeginningOfEndStep {
                player: CastTriggerPlayer::AnyPlayer,
            },
            vec![SpellEffectKind::Sacrifice {
                subject: EffectSubject::Source,
            }],
        )
    })
}

/// CR 107.5 / 701.9: the tap symbol plus discarding one chosen card as one atomic activation cost,
/// drawing one card. Rummage-order, other draw/discard counts, other costs, sacrifice or mana
/// variants, and timing restrictions stay unsupported.
fn match_activated_tap_discard_draw_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_327_TAP_DISCARD_DRAW_CLAUSE).then(|| {
        RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            cost_modifiers: Vec::new(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::Tap, AbilityCost::Discard],
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

/// CR 122 / 603.6a: a creature ETB that puts one +1/+1 counter on every other creature its
/// controller controls, excluding the source (CR 608.2h snapshot). "Each creature", any-player,
/// targeted, plural, and noncreature-source forms stay unsupported.
fn match_etb_put_counter_each_other_creature(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_327_ETB_MASS_COUNTER_CLAUSE).then(|| {
        triggered_ability_with(
            context,
            TriggerCondition::WhenSelfEntersBattlefield,
            vec![SpellEffectKind::PutCountersAll {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                filter: CreatureScopeFilter {
                    controller: Some(CreatureScopeController::YouControl),
                    exclude_self: true,
                    ..CreatureScopeFilter::default()
                },
            }],
        )
    })
}

/// CR 603.6c / 700.4: whenever another creature the source's controller controls dies, put one
/// +1/+1 counter on the source. Any-creature, self-inclusive, targeted, plural, noncreature
/// sources, and token-creation bodies stay unsupported.
fn match_triggered_another_creature_dies_put_counter_self(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_327_DIES_COUNTER_CLAUSE).then(|| {
        triggered_ability_with(
            context,
            TriggerCondition::WheneverCreatureDies {
                controller: CastTriggerPlayer::Controller,
                filter: PermanentEventFilter {
                    permanent_type: Some(PermanentTypeFilter::Creature),
                    exclude_source: true,
                    ..PermanentEventFilter::default()
                },
            },
            vec![SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Source,
            }],
        )
    })
}

/// Issue #328 exact clause templates. Each template is a reusable typed surface with at least two
/// real positive calibrations. Every clause is compared by the complete normalized Oracle line, so
/// an appended, reordered, or additional-clause form remains unsupported, and source-kind gating
/// stays on the recipes whose printed template requires it.
const ISSUE_328_EXILE_ATTACKING_CREATURE_CLAUSE: &str = "Exile target attacking creature.";
const ISSUE_328_SACRIFICE_SELF_DESTROY_ENCHANTMENT_CLAUSE: &str =
    "Sacrifice this creature: Destroy target enchantment.";
const ISSUE_328_UPKEEP_SELF_DAMAGE_ONE_CLAUSE: &str =
    "At the beginning of your upkeep, this creature deals 1 damage to you.";
const ISSUE_328_TAP_SURVEIL_ONE_CLAUSE: &str = "{T}: Surveil 1.";
const ISSUE_328_TAP_TARGET_CREATURE_GAINS_HASTE_CLAUSE: &str =
    "{T}: Target creature gains haste until end of turn.";
const ISSUE_328_ETB_RETURN_PERMANENT_CARD_CLAUSE: &str =
    "When this creature enters, return target permanent card from your graveyard to your hand.";
const ISSUE_328_UP_TO_TWO_CANT_BLOCK_CLAUSE: &str =
    "Up to two target creatures can't block this turn.";
const ISSUE_328_PUMP_FIRST_STRIKE_SCRY_ONE_CLAUSE: &str =
    "Target creature gets +1/+0 and gains first strike until end of turn. Scry 1.";

/// CR 508.1 / 701.13: "Exile target attacking creature" is the attacking-only sibling of the
/// shipped exile and the destroy-attacking-or-blocking recipes. Blocking-only, attacking-or-
/// blocking, unrestricted, controlled-only, up-to-one, and appended-instruction forms stay
/// unsupported.
fn match_spell_exile_attacking_creature(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_328_EXILE_ATTACKING_CREATURE_CLAUSE).then(|| {
        RecipeEmission::SpellEffect(SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                combat_role: Some(CombatRole::Attacking),
                ..TargetFilter::default()
            })),
        })
    })
}

/// CR 118.12a / 602.2b / 701.21: "Sacrifice this creature" is one atomic `SacrificeSelf` cost that
/// destroys one mandatory enchantment permanent. The shipped Naturalize recipe keeps its own
/// `{1}, Sacrifice … artifact or enchantment` clause; any mana component, artifact or union
/// target, "another creature" cost, exile replacement, and appended instruction stay unsupported.
fn match_activated_sacrifice_self_destroy_enchantment(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_328_SACRIFICE_SELF_DESTROY_ENCHANTMENT_CLAUSE).then(|| {
        RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            cost_modifiers: Vec::new(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::SacrificeSelf],
            effect: vec![SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    permanent_types: vec![PermanentTypeFilter::Enchantment],
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

/// CR 503.1 / 603.2b / 120.3: an upkeep trigger whose source creature deals exactly one damage to
/// its controller as an untargeted `DamagePlayer` recipient. Any-amount, each-upkeep, each-
/// opponent, target-player, end-step, and noncreature-source forms stay unsupported.
fn match_triggered_upkeep_self_damage_controller_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_328_UPKEEP_SELF_DAMAGE_ONE_CLAUSE).then(|| {
        triggered_ability_with(
            context,
            TriggerCondition::AtBeginningOfUpkeep {
                player: CastTriggerPlayer::Controller,
            },
            vec![SpellEffectKind::DamagePlayer {
                amount: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
            }],
        )
    })
}

/// CR 701.25: one tap symbol buys Surveil 1 through the shipped private library-partition effect,
/// mirroring the `{4}, {T}: Surveil 1.` recipe with no mana component. Any mana component,
/// different surveil count, scry, timing restriction, or added life cost stays unsupported.
fn match_activated_tap_surveil_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_328_TAP_SURVEIL_ONE_CLAUSE).then(|| {
        RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            cost_modifiers: Vec::new(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::Tap],
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

/// CR 611.2a / 514.2: one tap symbol grants haste until cleanup to one mandatory creature target.
/// Controlled-only, source-only, mana-component, permanent, plural, pump riders, and appended
/// instructions stay unsupported.
fn match_activated_tap_target_creature_gains_haste(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_328_TAP_TARGET_CREATURE_GAINS_HASTE_CLAUSE).then(|| {
        RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
            ability_id: context.activated_ability_id.clone(),
            presentation: context.presentation.clone(),
            cost_modifiers: Vec::new(),
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![AbilityCost::Tap],
            effect: vec![SpellEffectKind::GrantKeywords {
                subject: chosen_creature(TargetController::Any),
                keywords: vec![Keyword::Haste],
            }],
            targeting: Some(exact_targeting(1, 1, "Choose target creature", vec![0])),
            timing: ActivationTiming::Normal,
            conditions: Vec::new(),
            activation_limit: None,
        })
    })
}

/// CR 110.4a: "a permanent card" is every card that is not an instant or sorcery, which is exactly
/// the engine's own `CardFace::is_permanent` predicate; the schema has no single `Permanent`
/// card-type variant, so the recipe excludes the two nonpermanent card types. The mandatory
/// graveyard-card target revalidates the exact object at resolution. Creature-only, battlefield
/// destination, opponent graveyard, optional, dies-trigger, noncreature-source, and appended
/// instruction forms stay unsupported.
fn match_triggered_etb_return_permanent_card_to_hand(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_328_ETB_RETURN_PERMANENT_CARD_CLAUSE).then(|| {
        triggered_ability(
            context,
            SpellEffectKind::MoveGraveyardCards {
                filter: GraveyardFilter {
                    owner: GraveyardOwner::Controller,
                    card: Some(ZoneCardFilter {
                        excluded_card_types: vec![CardTypeFilter::Instant, CardTypeFilter::Sorcery],
                        ..ZoneCardFilter::default()
                    }),
                    ..GraveyardFilter::default()
                },
                destination: GraveyardDestination::Hand,
                linked_exile_id: None,
            },
        )
    })
}

/// CR 509.1b / 611.2c: "Up to two target creatures can't block this turn" is one bounded (min 0,
/// max 2) creature group bound to a single continuous can't-block restriction. Singular, plural-
/// unbound, controlled-only, attack-restriction, permanent, and appended-instruction forms stay
/// unsupported.
fn match_spell_up_to_two_target_creatures_cant_block(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_328_UP_TO_TWO_CANT_BLOCK_CLAUSE).then(|| {
        RecipeEmission::SpellEffectsWithTargeting {
            effects: vec![SpellEffectKind::ApplyCombatRestriction {
                scope: CombatRestrictionScope::Chosen(TargetFilter::default_creature()),
                restriction: CombatRestriction {
                    cant_block: true,
                    ..CombatRestriction::default()
                },
            }],
            targeting: exact_targeting(0, 2, "Choose up to two target creatures", vec![0]),
        }
    })
}

/// CR 611.2a / 514.2 / 701.22: the shipped #316 `+N/+0` first-strike pump plus a following Scry 1
/// shares one mandatory creature target across the pump and the keyword grant, while scry is
/// untargeted. The plain pump and any other scry count, rider, value, or conditional variant stay
/// unsupported.
fn match_spell_pump_first_strike_scry_one(text: &str, _: &RecipeContext) -> Option<RecipeEmission> {
    (text == ISSUE_328_PUMP_FIRST_STRIKE_SCRY_ONE_CLAUSE).then(|| {
        RecipeEmission::SpellEffectsWithTargeting {
            effects: vec![
                SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 0,
                    scale: None,
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                },
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    keywords: vec![Keyword::FirstStrike],
                },
                SpellEffectKind::Scry {
                    count: Amount::Fixed(1),
                },
            ],
            targeting: exact_targeting(1, 1, "Choose target creature", vec![0, 1]),
        }
    })
}

/// CR 702.185 / 702.34: the shared strict parser for a printed alternative cast-method cost line.
/// The whole remainder after `Keyword ` must be one canonical non-empty mana cost, so a rider,
/// em-dash variant, malformed or non-canonical cost, remaining reminder text, or appended
/// instruction never matches.
fn parse_cast_method_cost(text: &str, keyword: &str) -> Option<ManaCost> {
    let rest = text.strip_prefix(keyword)?.strip_prefix(' ')?;
    if !rest.starts_with('{') {
        return None;
    }
    let cost = ManaCost::parse(rest).ok()?;
    (cost.to_string() == rest && !cost.is_empty()).then_some(cost)
}

/// CR 702.185: Warp is a hand alternative cost for a permanent spell face. The face-type gate is
/// checked before the cost so an instant or sorcery line can never emit `warp_cost`; the registry
/// separately rejects a land face.
fn match_cast_method_warp(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    context
        .source_is_permanent
        .then(|| parse_cast_method_cost(text, "Warp"))?
        .map(RecipeEmission::WarpCost)
}

/// CR 702.34: Flashback is a graveyard alternative cost for an instant or sorcery face. The
/// face-type gate is checked before the cost so a permanent line can never emit `flashback_cost`.
fn match_cast_method_flashback(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_instant || context.source_is_sorcery)
        .then(|| parse_cast_method_cost(text, "Flashback"))?
        .map(RecipeEmission::FlashbackCost)
}

/// Issue #333 exact clause templates. Each template is a reusable typed surface with at least two
/// real positive calibrations. Every clause is compared by the complete normalized Oracle line, so
/// an appended, reordered, or additional-clause form remains unsupported, and source-kind gating
/// stays on the recipes whose printed template requires it.
const ISSUE_333_TARGET_OPPONENT_REVEAL_DISCARD_CLAUSE: &str =
    "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card.";
const ISSUE_333_LANDFALL_GAIN_LIFE_CLAUSE: &str =
    "Landfall — Whenever a land you control enters, you gain 1 life.";
const ISSUE_333_OTHER_CREATURE_ENTERS_PUMP_CLAUSE: &str =
    "Whenever another creature you control enters, this creature gets +1/+1 until end of turn.";
const ISSUE_333_TWO_MANA_ANY_ONE_COLOR_CLAUSE: &str = "{T}: Add two mana of any one color.";
const ISSUE_333_THREE_MANA_ANY_ONE_COLOR_CLAUSE: &str = "{T}: Add three mana of any one color.";
const ISSUE_333_COUNT_SCALED_ARTIFACT_CLAUSE: &str =
    "This creature gets +1/+0 for each artifact you control.";

/// CR 701.9 / 701.20: "target opponent reveals their hand; you choose a nonland card from it; that
/// player discards that card" is one mandatory player target bound to the controller-selected,
/// publicly revealed discard. The shipped Coercion/Thoughtseize payload shares the typed shape,
/// but the nonland filter, opponent restriction, count of one, and absence of a rider are this
/// exact template's contract. Any other reveal/choose/exile/plural/optional wording stays
/// unsupported.
fn match_spell_target_opponent_reveal_discard_nonland(
    text: &str,
    _: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_333_TARGET_OPPONENT_REVEAL_DISCARD_CLAUSE).then(|| {
        RecipeEmission::SpellEffectsWithTargeting {
            effects: vec![SpellEffectKind::ChooseHandCards {
                action: HandCardAction::Discard,
                count: 1,
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    ..TargetFilter::default()
                },
                chooser: HandCardChooser::Controller,
                card_filter: Some(CardTypeFilter::Nonland),
                optional: false,
                visibility: HandChoiceVisibility::PublicReveal,
            }],
            targeting: exact_targeting(1, 1, "Choose target opponent", vec![0]),
        }
    })
}

/// CR 603.6a: the Landfall ability word is presentation, but the printed template includes it, so
/// the exact clause keeps the `Landfall — ` prefix. The trigger watches the controller's lands
/// entering and gains a fixed 1 life. Noncreature sources, other amounts, other effects, and the
/// prefix-less wording stay unsupported.
fn match_landfall_gain_life_one(text: &str, context: &RecipeContext) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_333_LANDFALL_GAIN_LIFE_CLAUSE).then(|| {
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
            vec![SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            }],
        )
    })
}

/// CR 603.6a / 611.2c: another creature the controller controls entering gives the source +1/+1
/// until cleanup; `exclude_source` is the "another" contract (CR 608.2h snapshot semantics).
/// Self-inclusive, all-player, other pump sizes, counters, other ability words, and missing
/// until-end-of-turn wording stay unsupported.
fn match_other_creature_enters_pump_self_plus_one_plus_one(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_333_OTHER_CREATURE_ENTERS_PUMP_CLAUSE).then(|| {
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
            vec![SpellEffectKind::PumpTarget {
                power: 1,
                toughness: 1,
                scale: None,
                subject: EffectSubject::Source,
            }],
        )
    })
}

/// CR 605 / 106.1: five mutually exclusive color bags where every choice produces `per_color` mana
/// of exactly one color. This keeps "of any one color" distinct from a fixed multi-pip cost and
/// from "any color" combination outputs, matching the shipped Sol Ring `(c: 2)` bag shape.
fn any_one_color_mana_options(per_color: u32) -> Vec<ManaAmount> {
    ['W', 'U', 'B', 'R', 'G']
        .into_iter()
        .map(|symbol| {
            let mut amount =
                parse_mana_amount(symbol).expect("five-color recipe uses valid symbols");
            match symbol {
                'W' => amount.w = per_color,
                'U' => amount.u = per_color,
                'B' => amount.b = per_color,
                'R' => amount.r = per_color,
                _ => amount.g = per_color,
            }
            amount
        })
        .collect()
}

/// CR 605.1a: the plain tap symbol produces `per_color` mana of one chosen color. Other costs,
/// sacrifice or mana variants, restrictions, and other multipliers stay unsupported.
fn tap_for_any_one_color_mana(context: &RecipeContext, per_color: u32) -> RecipeEmission {
    RecipeEmission::ActivatedAbility(ActivatedAbilityDef {
        ability_id: context.activated_ability_id.clone(),
        presentation: context.presentation.clone(),
        cost_modifiers: Vec::new(),
        source_zone: AbilitySourceZone::Battlefield,
        costs: vec![AbilityCost::Tap],
        effect: vec![SpellEffectKind::ProduceMana {
            options: any_one_color_mana_options(per_color),
            restriction: None,
            conditional: None,
        }],
        targeting: None,
        timing: ActivationTiming::Normal,
        conditions: Vec::new(),
        activation_limit: None,
    })
}

fn match_tap_for_two_mana_any_one_color(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_333_TWO_MANA_ANY_ONE_COLOR_CLAUSE)
        .then(|| tap_for_any_one_color_mana(context, 2))
}

fn match_tap_for_three_mana_any_one_color(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (text == ISSUE_333_THREE_MANA_ANY_ONE_COLOR_CLAUSE)
        .then(|| tap_for_any_one_color_mana(context, 3))
}

/// CR 613.4c / layer 7c: this creature gets +1/+0 for each artifact its controller controls. The
/// count is a pre-layer-7 battlefield permanent count, matching the shipped `CountScaledSelfPt`
/// vocabulary. Other artifact scopes, creature scopes, pump values, equipped forms, conditions,
/// and noncreature sources stay unsupported.
fn match_static_self_count_scaled_artifact_plus_one_zero(
    text: &str,
    context: &RecipeContext,
) -> Option<RecipeEmission> {
    (context.source_is_creature && text == ISSUE_333_COUNT_SCALED_ARTIFACT_CLAUSE).then(|| {
        RecipeEmission::StaticAbility(IdentifiedAbility {
            ability_id: context.static_ability_id.clone(),
            presentation: context.presentation.clone(),
            definition: StaticAbilityDef::CountScaledSelfPt {
                count: CountExpression::BattlefieldPermanents {
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
                },
                power_per_match: 1,
                toughness_per_match: 0,
            },
        })
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
        id: RecipeId("triggered.discard.one_or_more.plus_one_counters_source"),
        label: "discard one or more cards puts that many +1/+1 counters on this creature",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_discard_batch_counter_trigger,
        calibration: calibrations!(
            "Scrounging Skyray" => "Whenever you discard one or more cards, put that many +1/+1 counters on this creature.",
            "Marauding Mako" => "Whenever you discard one or more cards, put that many +1/+1 counters on this creature.";
            "Whenever an opponent discards one or more cards, put that many +1/+1 counters on this creature.",
            "Whenever you discard a card, put a +1/+1 counter on this creature.",
            "Whenever you discard one or more cards, put that many +1/+1 counters on target creature.",
            "Whenever you discard one or more creature cards, put that many +1/+1 counters on this creature.",
            "Whenever you discard one or more cards, draw that many cards.",
            "Whenever you discard one or more cards, put that many +1/+1 counters on this creature. This ability triggers only once each turn."
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
            "Target creature gets -5/-5 until end of turn.",
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
            "Enchant creature an opponent controls",
            "Enchant tapped creature",
            "Enchant creature or Vehicle",
            "Enchant creature. When this Aura enters, draw a card."
        ),
    },
    Recipe {
        id: RecipeId("aura.enchant.creature_you_control"),
        label: "enchant creature you control",
        surface: RecipeSurface::AuraSpellClause,
        matcher: match_enchant_creature_you_control,
        calibration: calibrations!(
            "Aquitect's Defenses" => "Enchant creature you control",
            "Pitiless Fists" => "Enchant creature you control";
            "Enchant creature an opponent controls",
            "Enchant creature you don't control",
            "Enchant permanent you control",
            "Enchant creature you control. Draw a card."
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
        id: RecipeId("etb.aura.grant_first_strike.attached_until_end_of_turn"),
        label: "Aura ETB grant first strike to enchanted creature until end of turn",
        surface: RecipeSurface::EtbAbility,
        matcher: match_aura_etb_grant_first_strike,
        calibration: calibrations!(
            "Super Speed" => "When this Aura enters, enchanted creature gains first strike until end of turn.",
            "Fire-Rim Form" => "When this Aura enters, enchanted creature gains first strike until end of turn.";
            "When this Aura enters, enchanted creature gains double strike until end of turn.",
            "When this Aura enters, enchanted creature gains first strike permanently.",
            "When this Aura enters, enchanted creature gains first strike until end of combat.",
            "When this Aura enters, enchanted creature gains first strike until end of turn. Draw a card.",
            "When this creature enters, enchanted creature gains first strike until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("etb.aura.grant_hexproof.attached_until_end_of_turn"),
        label: "Aura ETB grant hexproof to enchanted creature until end of turn",
        surface: RecipeSurface::EtbAbility,
        matcher: match_aura_etb_grant_hexproof,
        calibration: calibrations!(
            "Aquitect's Defenses" => "When this Aura enters, enchanted creature gains hexproof until end of turn. (It can't be the target of spells or abilities your opponents control.)",
            "Fae Flight" => "When this Aura enters, enchanted creature gains hexproof until end of turn.";
            "When this Aura enters, enchanted creature gains hexproof permanently.",
            "When this Aura enters, enchanted creature gains shroud until end of turn.",
            "When this Aura enters, enchanted creature gains hexproof until end of combat.",
            "When this Aura enters, enchanted creature gains hexproof until end of turn. Draw a card.",
            "When this Aura enters, target creature gains hexproof until end of turn.",
            "When this enchantment enters, enchanted creature gains hexproof until end of turn."
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
        id: RecipeId("etb.exile.target_opponent_hand.one"),
        label: "creature ETB target opponent exiles one hand card",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_target_opponent_exiles_one,
        calibration: calibrations!(
            "Unscrupulous Agent" => "When this creature enters, target opponent exiles a card from their hand.",
            "Skullcap Snail" => "When this creature enters, target opponent exiles a card from their hand.";
            "When this creature enters, target opponent exiles two cards from their hand.",
            "When this creature enters, each opponent exiles a card from their hand.",
            "When this creature enters, target opponent exiles a card from their hand at random.",
            "When this creature enters, target opponent may exile a card from their hand.",
            "When this creature enters, target opponent reveals a card from their hand.",
            "When this creature enters, target opponent exiles a card from their hand. Draw a card."
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
        id: RecipeId("etb.look.top_three.optional_top_one"),
        label: "ETB look three optionally keep one on top",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_look_top_three_optional_top_one,
        calibration: calibrations!(
            "Sage of Days" => "When this creature enters, look at the top three cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.",
            "Gurmag Nightwatch" => "When this creature enters, look at the top three cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.";
            "When this creature enters, look at the top three cards of your library. Put one of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top three cards of your library. You may put two of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top two cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top four cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top three cards of your library. You may put one of those cards on the bottom of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top three cards of your library. You may put one of those cards into exile. Put the rest into your graveyard.",
            "When this creature enters, look at the top three cards of your library. You may put one of those cards into your hand. Put the rest into your graveyard.",
            "When this creature enters, reveal the top three cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, surveil 3.",
            "When this creature enters, look at the top three cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.create_token.mutagen.one"),
        label: "ETB create Mutagen",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_create_mutagen,
        calibration: calibrations!(
            "Crustacean Commando" => r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            "Slithering Cryptid" => r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#;
            "When this creature enters, create a Mutagen token.",
            r#"When this creature enters, create two Mutagen tokens. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a tapped Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{2}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put two +1/+1 counters on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature you control. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as an instant.")"#,
            r#"When this creature dies, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"Whenever this creature attacks, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.") Draw a card."#
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_enters.recruit"),
        label: "creature ETB recruit",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_recruit,
        calibration: calibrations!(
            "Long Lake Nuisance" => r#"When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"#,
            "Patient Instructor" => r#"When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"#;
            "When this creature enters, recruit.",
            "When this creature enters, you recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
            "When this creature enters, recruit. (Draw a card, then discard a card.)",
            "When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a land card, create a 1/1 white Human Soldier creature token.)",
            "When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 2/2 white Human Soldier creature token.)",
            "When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Soldier creature token.)",
            "When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token. Draw a card.)",
            "When this creature enters, it connives. (Draw a card, then discard a card. If you discarded a nonland card, put a +1/+1 counter on this creature.)",
            "When this creature dies, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
            "Whenever this creature attacks, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
            "When this enchantment enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
            "When an opponent casts their first noncreature spell each turn, you recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_leaves_battlefield.create_token.food.one"),
        label: "leaves the battlefield create Food",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_leaves_battlefield_create_food,
        calibration: calibrations!(
            "City Pigeon" => r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#,
            "Featherbrained Filcher" => r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#;
            r#"When this creature dies, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token."#,
            r#"When this creature leaves the battlefield, create two Food tokens. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a tapped Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{3}, {T}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 2 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.") Draw a card."#,
            r#"When another creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#
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
        id: RecipeId("dies.create_token.mercenary.one"),
        label: "dies create Mercenary",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_dies_create_mercenary,
        calibration: calibrations!(
            "Nezumi Linkbreaker" => r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            "Wanted Griffin" => r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##;
            r##"When this creature leaves the battlefield, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When another creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, you may create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create two 1/1 red Mercenary creature tokens with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a tapped 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as an instant.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +2/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+1 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn.""##,
            r##"When this creature dies, create an attacking 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token. At the beginning of the next end step, sacrifice it."##,
            r##"When this creature dies, create a 1/1 blue Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 2/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Soldier creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{1}, {T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of next turn. Activate only as a sorcery.""##,
            r#"When this creature dies, create a 1/1 red Mercenary creature token."#,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery."" Draw a card."##,
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
        id: RecipeId(
            "triggered.self_attacks.pump_other_creature_you_control.plus_one_indestructible",
        ),
        label: "self-attacks pump another creature you control and grant indestructible",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_attacks_pump_other_controlled_creature_indestructible,
        calibration: calibrations!(
            "Hardened Escort" => "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            "Foot Elite" => "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn.";
            "When this creature enters, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature blocks, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            "At the beginning of combat, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, this creature gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature an opponent controls gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, you may have another target creature you control get +1/+0 and gain indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +2/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+1 and gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains vigilance until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains hexproof until end of turn.",
            "Whenever this creature attacks, put a +1/+1 counter on another target creature you control and it gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn. Draw a card.",
            "Whenever this creature attacks, up to two target creatures you control each get +1/+0 and gain indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn.\nReach",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn, then draw a card."
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
        id: RecipeId("triggered.attached_object_attacks.tap.defending_creature"),
        label: "attached-object attacks tap defending creature",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_equipment_attached_object_attacks_tap_defending_creature,
        calibration: calibrations!(
            "Captain America's Shield" => "Whenever equipped creature attacks, tap target creature defending player controls.",
            "Thunder Lasso" => "Whenever equipped creature attacks, tap target creature defending player controls.";
            "Whenever this Equipment attacks, tap target creature defending player controls.",
            "Whenever this artifact attacks, tap target creature defending player controls.",
            "Whenever equipped creature attacks, tap target creature.",
            "Whenever equipped creature attacks, tap target creature any player controls.",
            "Whenever equipped creature attacks, tap target creature an opponent controls.",
            "Whenever equipped creature attacks, tap target creature attacking player controls.",
            "Whenever equipped creature attacks, tap target permanent defending player controls.",
            "Whenever equipped creature attacks, tap target artifact defending player controls.",
            "Whenever equipped creature attacks, tap target noncreature artifact defending player controls.",
            "Whenever equipped creature attacks, untap target creature defending player controls.",
            "Whenever equipped creature attacks, put a stun counter on target creature defending player controls.",
            "Whenever equipped creature attacks, you may tap target creature defending player controls.",
            "Whenever equipped creature attacks, tap up to one target creature defending player controls.",
            "At the beginning of combat, tap target creature defending player controls.",
            "Whenever equipped creature blocks, tap target creature defending player controls.",
            "Whenever equipped creature attacks, tap two target creatures defending player controls.",
            "Whenever equipped creature attacks, tap target creature defending player controls. Draw a card.",
            "Whenever equipped creature attacks, tap target creature defending player controls.\nReach",
            "Whenever equipped creature attacks, tap target creature defending player controls, then draw a card."
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
        id: RecipeId("activated.graveyard_card_to_library_bottom"),
        label: "put a target card from your graveyard on the bottom of your library",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_graveyard_card_to_library_bottom,
        calibration: calibrations!(
            "Barkform Harvester" => "{2}: Put target card from your graveyard on the bottom of your library.",
            "Tomb Trawler" => "{2}: Put target card from your graveyard on the bottom of your library.";
            "{2}: Put target card from a graveyard on the bottom of your library.",
            "{2}: Put target card from an opponent's graveyard on the bottom of your library.",
            "{2}: Put target creature card from your graveyard on the bottom of your library.",
            "{2}: Put target noncreature card from your graveyard on the bottom of your library.",
            "{2}: Put up to one target card from your graveyard on the bottom of your library.",
            "{2}: Choose a card from your graveyard and put it on the bottom of your library.",
            "{2}: You may put target card from your graveyard on the bottom of your library.",
            "{2}: Put target card from your graveyard on the top of your library.",
            "{2}: Put target card from your graveyard into your hand.",
            "{2}: Exile target card from your graveyard.",
            "{2}: Put two target cards from your graveyard on the bottom of your library.",
            "{2}, {T}: Put target card from your graveyard on the bottom of your library.",
            "{2}, Sacrifice this creature: Put target card from your graveyard on the bottom of your library.",
            "{2}{G}: Put target card from your graveyard on the bottom of your library.",
            "{2}: Put target card from your graveyard on the bottom of your library. Then draw a card.",
            "Put target card from your graveyard on the bottom of your library with {2}:.",
            "{2}: Put target card from your graveyard on the bottom of your library. Put another card there."
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
        id: RecipeId("activated.creature.pay_mana.draw_two"),
        label: "creature activated draw two",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_creature_pay_draw_two,
        calibration: calibrations!(
            "Mystic Archaeologist" => "{3}{U}{U}: Draw two cards.",
            "Oscorp Research Team" => "{6}{U}: Draw two cards.";
            "{3}{U}: Draw two cards.",
            "{4}{U}: Draw two cards.",
            "{3}{U}{U}{U}: Draw two cards.",
            "{5}{U}: Draw two cards.",
            "{7}{U}: Draw two cards.",
            "{3}{G}{G}: Draw two cards.",
            "{3}{U}{U}: Draw one card.",
            "{3}{U}{U}: Draw three cards.",
            "{3}{U}{U}: Draw X cards.",
            "{3}{U}{U}: Target player draws two cards.",
            "{3}{U}{U}: Each player draws two cards.",
            "{3}{U}{U}: You may draw two cards.",
            "{3}{U}{U}: Draw two cards, then discard a card.",
            "{3}{U}{U}: Draw two cards. You lose 2 life.",
            "{3}{U}{U}, {T}: Draw two cards.",
            "{3}{U}{U}, Sacrifice this creature: Draw two cards.",
            "{3}{U}{U}, Discard a card: Draw two cards.",
            "{3}{U}{U}, Pay 1 life: Draw two cards.",
            "{3}{U}{U}: Draw two cards. Activate only as a sorcery.",
            "{3}{U}{U}: Draw two cards. Activate only once each turn.",
            "{3}{U}{U}: If you control another Wizard, draw two cards.",
            "{3}{U}{U}: Draw two cards from your graveyard.",
            "{3}{U}{U}: Draw two cards from your library.",
            "{3}{U}{U}: Draw two cards, then draw another card.",
            "{3}{U}{U}: Draw two cards. This ability can be activated only from your hand."
        ),
    },
    Recipe {
        id: RecipeId("activated.creature.pay_mana_tap.tap_creature"),
        label: "creature activated tap creature",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_creature_pay_mana_tap_tap_creature,
        calibration: calibrations!(
            "Coeurl" => "{1}{W}, {T}: Tap target nonenchantment creature.",
            "Frostbridge Guard" => "{2}{W}, {T}: Tap target creature.",
            "Sterling Keykeeper" => "{2}, {T}: Tap target non-Mount creature.";
            "{1}{W}: Tap target nonenchantment creature.",
            "{1}{W}, {T}: Untap target nonenchantment creature.",
            "{1}{W}, {T}: Tap target nonenchantment creature or player.",
            "{1}{W}, {T}: Tap up to one target nonenchantment creature.",
            "{1}{W}, {T}: Tap two target nonenchantment creatures.",
            "{1}{W}, {T}: Tap target nonenchantment creature you control.",
            "{1}{W}, {T}, Sacrifice this creature: Tap target nonenchantment creature.",
            "{1}{W}, {T}, Untap this creature: Tap target nonenchantment creature.",
            "{1}{W}, {T}, Discard a card: Tap target nonenchantment creature.",
            "{1}{W}, {T}, Pay 1 life: Tap target nonenchantment creature.",
            "{1}{W}, {T}: Destroy target nonenchantment creature.",
            "{1}{W}, {T}: Exile target nonenchantment creature.",
            "{1}{W}, {T}: Tap target noncreature permanent.",
            "{1}{W}, {T}: Tap target artifact creature.",
            "{1}{W}, {T}: Tap target creature. Activate only as a sorcery.",
            "{1}{W}, {T}: Tap target creature. Activate only once each turn.",
            "{1}{W}, {T}: Tap target creature. Activate only if you control an artifact.",
            "{1}{W}, {T}: Tap target creature. Activate only from your hand.",
            "{2}{W}, {T}: Tap target creature.",
            "{2}, {T}: Tap target creature.",
            "{2}, {T}: Tap target Mount creature.",
            "{2}, {T}: Tap target non-Vehicle creature.",
            "{2}, {T}: Tap target creature you control.",
            "{2}, Tap this creature: Tap target non-Mount creature.",
            "{2}, {Q}: Tap target non-Mount creature.",
            "{2}, {T}: Tap target non-Mount creature, then draw a card.",
            "{2}, {T}: Tap target non-Mount creature. Untap this creature.",
            "{2}, {T}, Sacrifice this creature: Tap target non-Mount creature."
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
        id: RecipeId("activated.land.sacrifice.draw_one"),
        label: "land sacrifice draw",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_land_sacrifice_draw_one,
        calibration: calibrations!(
            "Ripchain Razorkin" => "{2}{R}, Sacrifice a land: Draw a card.",
            "Seismic Monstrosaur" => "{2}{R}, Sacrifice a land: Draw a card.";
            "{3}{R}, Sacrifice a land: Draw a card.",
            "{2}{R}, {T}, Sacrifice a land: Draw a card.",
            "{2}{R}, Discard a card, Sacrifice a land: Draw a card.",
            "{2}{R}, Pay 1 life, Sacrifice a land: Draw a card.",
            "{2}{R}, Exile a card from your graveyard, Sacrifice a land: Draw a card.",
            "{2}{R}, Sacrifice a land, Put a +1/+1 counter on this creature: Draw a card.",
            "{2}{R}, Sacrifice this creature: Draw a card.",
            "{2}{R}, Sacrifice a creature: Draw a card.",
            "{2}{R}, Sacrifice an artifact: Draw a card.",
            "{2}{R}, Sacrifice a permanent: Draw a card.",
            "{2}{R}, Sacrifice a Mountain: Draw a card.",
            "{2}{R}, Sacrifice a basic land: Draw a card.",
            "{2}{R}, Sacrifice two lands: Draw a card.",
            "{2}{R}, Sacrifice a land an opponent controls: Draw a card.",
            "{2}{R}, Sacrifice target land: Draw a card.",
            "{2}{R}, You may sacrifice a land: Draw a card.",
            "{2}{R}, Sacrifice a land: You may draw a card.",
            "{2}{R}, Sacrifice a land: Draw zero cards.",
            "{2}{R}, Sacrifice a land: Draw two cards.",
            "{2}{R}, Sacrifice a land: Draw a card, then discard a card.",
            "{2}{R}, Sacrifice a land: Draw a card. Activate only as a sorcery.",
            "{2}{R}, Sacrifice a land: Draw a card. Activate only once each turn.",
            "Sacrifice a land, {2}{R}: Draw a card.",
            "{2}{R}, Sacrifice a land: Draw a card. You gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("triggered.beginning_of_combat.controller.exile_graveyard_card.optional_one"),
        label: "controller beginning-of-combat optional graveyard exile",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_beginning_of_combat_exile_graveyard_card,
        calibration: calibrations!(
            "Ascendant Dustspeaker" => "At the beginning of combat on your turn, exile up to one target card from a graveyard.",
            "Startled Relic Sloth" => "At the beginning of combat on your turn, exile up to one target card from a graveyard.";
            "At the beginning of your upkeep, exile up to one target card from a graveyard.",
            "At the beginning of your end step, exile up to one target card from a graveyard.",
            "At the beginning of each combat, exile up to one target card from a graveyard.",
            "At the beginning of combat on an opponent's turn, exile up to one target card from a graveyard.",
            "At the beginning of combat on your turn, exile one target card from a graveyard.",
            "At the beginning of combat on your turn, exile up to two target cards from a graveyard.",
            "At the beginning of combat on your turn, choose a card from a graveyard, then exile it.",
            "At the beginning of combat on your turn, exile up to one target card from your graveyard.",
            "At the beginning of combat on your turn, exile up to one target card from an opponent's graveyard.",
            "At the beginning of combat on your turn, exile up to one target creature card from a graveyard.",
            "At the beginning of combat on your turn, exile up to one target card from your hand.",
            "At the beginning of combat on your turn, exile up to one target card from a graveyard to your hand.",
            "At the beginning of combat on your turn, exile up to one target card from a graveyard. If you do, draw a card.",
            "At the beginning of combat on your turn, if you control a creature, exile up to one target card from a graveyard.",
            "Whenever this creature attacks, exile up to one target card from a graveyard.",
            "At the beginning of combat on any turn, exile up to one target card from a graveyard.",
            "At the beginning of combat on each player's turn, exile up to one target card from a graveyard.",
            "At the beginning of combat on your turn, exile up to one target nonland card from a graveyard.",
            "At the beginning of combat on your turn, exile up to one target card from a library.",
            "At the beginning of combat on your turn, exile up to one target card from the battlefield.",
            "At the beginning of combat on your turn, exile up to one target card from exile.",
            "At the beginning of combat on your turn, return up to one target card from a graveyard to your hand.",
            "At the beginning of combat on your turn, put up to one target card from a graveyard on top of its owner's library.",
            "At the beginning of combat on your turn, put up to one target card from a graveyard onto the battlefield.",
            "At the beginning of combat on your turn, you may exile target card from a graveyard.",
            "At the beginning of combat on your turn, exile target card from a graveyard. You may exile another target card from a graveyard."
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
        id: RecipeId("triggered.self_enters.optional_search_basic_land_top"),
        label: "self enters optional basic land search to library top",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_enters_optional_search_basic_land_to_top,
        calibration: calibrations!(
            "Campus Guide" => "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.",
            "Spider-Bot" => "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.";
            "When this creature enters, search your library for a basic land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a nonbasic land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a creature card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card into your hand.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card onto the battlefield.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on the bottom.",
            "When this creature enters, you may search your library for a basic land card, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then put that card on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put up to one card on top.",
            "When this creature enters, you may search your library for up to one basic land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for two basic land cards, reveal them, then shuffle and put those cards on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top of an opponent's library.",
            "When this creature enters, target opponent may search their library for a basic land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top. It gains haste.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.\nReach",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top, then draw a card."
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
    Recipe {
        id: RecipeId("station.spacecraft.threshold_6_7_flying"),
        label: "Station 6+/7+ Flying Spacecraft",
        surface: RecipeSurface::StationAssembly,
        matcher: match_station_6_7_flying_assembly,
        calibration: calibrations!(
            "Pinnacle Kill-Ship" => "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Warmaker Gunship" => "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 6+.)\n6+ | Flying";
            "Station",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 6+.)",
            "6+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 11+.)\n11+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as an instant. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7.)\n7 | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying\n7+ | Flying",
            "7+ | Flying\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Planet. Station only as a sorcery.)\n7+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact Vehicle at 7+.)\n7+ | Flying"
        ),
    },
    Recipe {
        id: RecipeId("etb.spacecraft.pinnacle.damage_ten_up_to_one_creature"),
        label: "Pinnacle Kill-Ship ETB damage up to one creature",
        surface: RecipeSurface::EtbAbility,
        matcher: match_pinnacle_etb_damage_ten_up_to_one_creature,
        calibration: singleton_calibrations!(
            "Pinnacle Kill-Ship" => "When this Spacecraft enters, it deals 10 damage to up to one target creature.";
            "When this Spacecraft enters, it deals 9 damage to up to one target creature.",
            "When this Spacecraft enters, it deals 11 damage to up to one target creature.",
            "When this Spacecraft enters, it deals 10 damage to one target creature.",
            "When this Spacecraft enters, it deals 10 damage to up to two target creatures.",
            "When this Spacecraft enters, it deals 10 damage to up to one target player.",
            "When this Spacecraft enters, it deals 10 damage to target creature.",
            "When this Spacecraft enters, you may deal 10 damage to up to one target creature.",
            "When this Spacecraft enters, it deals 10 damage to up to one target creature. You gain 1 life.",
            "Whenever this Spacecraft enters, it deals 10 damage to up to one target creature."
        ),
    },
    Recipe {
        id: RecipeId("etb.spacecraft.warmaker.damage_artifact_count_opponent_creature"),
        label: "Warmaker Gunship ETB artifact-count damage to opponent creature",
        surface: RecipeSurface::EtbAbility,
        matcher: match_warmaker_etb_damage_artifact_count_opponent_creature,
        calibration: singleton_calibrations!(
            "Warmaker Gunship" => "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to target creature an opponent controls.";
            "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to target creature.",
            "When this Spacecraft enters, it deals damage equal to the number of creatures you control to target creature an opponent controls.",
            "When this Spacecraft enters, it deals damage equal to the number of artifacts an opponent controls to target creature an opponent controls.",
            "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to any target.",
            "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to up to one target creature an opponent controls.",
            "When this Spacecraft enters, it deals 3 damage to target creature an opponent controls.",
            "When this Spacecraft enters, you may deal damage equal to the number of artifacts you control to target creature an opponent controls.",
            "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to target creature an opponent controls. You gain 1 life.",
            "Whenever this Spacecraft enters, it deals damage equal to the number of artifacts you control to target creature an opponent controls."
        ),
    },
    Recipe {
        id: RecipeId("station.spacecraft.threshold_5_or_8_keywords"),
        label: "Station 5+/8+ keyword Spacecraft",
        surface: RecipeSurface::StationAssembly,
        matcher: issue_313_station_assembly,
        calibration: calibrations!(
            "Extinguisher Battleship" => "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            "Fell Gravship" => "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying, lifelink";
            "Station",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)",
            "5+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 4+.)\n4+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 6+.)\n6+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, lifelink",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5.)\n5 | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5-8.)\n5-8 | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as an instant. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Trample, flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its toughness on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, vigilance",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample\n5+ | Flying, trample",
            "5+ | Flying, trample\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)",
            "Station (Tap another creature an opponent controls: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            "Station (Tap another creature you control: Put +1/+1 counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its toughness on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            "Station (Tap this artifact: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on that creature. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact Vehicle at 5+.)\n5+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Planet. Station only as a sorcery.)\n5+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample\nDraw a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.spacecraft.extinguisher.destroy_noncreature_then_damage_all_creatures"),
        label: "Extinguisher Battleship ETB destroy noncreature then damage all creatures",
        surface: RecipeSurface::EtbAbility,
        matcher: match_extinguisher_etb_destroy_then_damage,
        calibration: singleton_calibrations!(
            "Extinguisher Battleship" => "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.";
            "When this Spacecraft enters, destroy target creature. Then this Spacecraft deals 4 damage to each creature.",
            "When this Spacecraft enters, destroy target artifact. Then this Spacecraft deals 4 damage to each creature.",
            "When this Spacecraft enters, destroy target land. Then this Spacecraft deals 4 damage to each creature.",
            "When this Spacecraft enters, destroy target permanent. Then this Spacecraft deals 4 damage to each creature.",
            "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 3 damage to each creature.",
            "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 5 damage to each creature.",
            "When this Spacecraft enters, exile target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.",
            "When this Spacecraft enters, return target noncreature permanent to its owner's hand. Then this Spacecraft deals 4 damage to each creature.",
            "When this Spacecraft enters, destroy up to one target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.",
            "When this Spacecraft enters, you may destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.",
            "When this Spacecraft enters, destroy target noncreature permanent. Then it deals 4 damage to each creature.",
            "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each noncreature permanent.",
            "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature your opponents control.",
            "When this Spacecraft enters, this Spacecraft deals 4 damage to each creature. Then destroy target noncreature permanent.",
            "When this Spacecraft enters, destroy target noncreature permanent and deal 4 damage to each creature.",
            "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to target creature.",
            "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature. You gain 1 life.",
            "Whenever this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to each creature."
        ),
    },
    Recipe {
        id: RecipeId("etb.spacecraft.fell.mill_three_then_return_creature_or_spacecraft"),
        label: "Fell Gravship ETB mill three then choose creature or Spacecraft to hand",
        surface: RecipeSurface::EtbAbility,
        matcher: match_fell_etb_mill_then_return,
        calibration: singleton_calibrations!(
            "Fell Gravship" => "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.";
            "When this Spacecraft enters, mill two cards, then return a creature or Spacecraft card from your graveyard to your hand.",
            "When this Spacecraft enters, mill four cards, then return a creature or Spacecraft card from your graveyard to your hand.",
            "When this Spacecraft enters, target player mills three cards, then return a creature or Spacecraft card from your graveyard to your hand.",
            "When this artifact enters, mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.",
            "Whenever this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.",
            "When this Spacecraft enters, mill three cards, then return a creature card from your graveyard to your hand.",
            "When this Spacecraft enters, mill three cards, then return a Spacecraft card from your graveyard to your hand.",
            "When this Spacecraft enters, mill three cards, then return a card from your graveyard to your hand.",
            "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card milled this way from your graveyard to your hand.",
            "When this Spacecraft enters, mill three cards, then return target creature or Spacecraft card from your graveyard to your hand.",
            "When this Spacecraft enters, you may mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.",
            "When this Spacecraft enters, mill three cards, then you may return a creature or Spacecraft card from your graveyard to your hand.",
            "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to the battlefield.",
            "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to exile.",
            "When this Spacecraft enters, return a creature or Spacecraft card from your graveyard to your hand, then mill three cards.",
            "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from an opponent's graveyard to your hand.",
            "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from a graveyard to your hand.",
            "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to your hand. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("station.spacecraft.threshold_3_or_9_keywords"),
        label: "Station 3+ or 9+ keyword Spacecraft",
        surface: RecipeSurface::StationAssembly,
        matcher: match_station_3_or_9_keyword_assembly,
        calibration: calibrations!(
            "Galvanizing Sawship" => "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
            "Wedgelight Rammer" => "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike";
            "Station",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)",
            "3+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 2+.)\n2+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 4+.)\n4+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 10+.)\n10+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3.)\n3 | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at three or more.)\n3+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3-5.)\n3-5 | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Haste, flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, vigilance",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | First strike, flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as an instant. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Activate only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap this artifact: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its toughness on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put +1/+1 counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on that creature. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature an opponent controls: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Planet. Station only as a sorcery.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact Vehicle at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike\nDraw a card.",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n9+ | Flying, haste",
            "3+ | Flying, haste\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste\n3+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
            "Station ({1}, Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste"
        ),
    },
    Recipe {
        id: RecipeId("etb.spacecraft.wedgelight.create_robot"),
        label: "Wedgelight Rammer ETB create one Robot",
        surface: RecipeSurface::EtbAbility,
        matcher: match_wedgelight_etb_create_robot,
        calibration: singleton_calibrations!(
            "Wedgelight Rammer" => "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token.";
            "When this Spacecraft enters, create two 2/2 colorless Robot artifact creature tokens.",
            "When this Spacecraft enters, create a tapped 2/2 colorless Robot artifact creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Robot creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Servo artifact creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token. You may.",
            "When this artifact enters, create a 2/2 colorless Robot artifact creature token.",
            "Whenever this Spacecraft enters, create a 2/2 colorless Robot artifact creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token, then draw a card.",
            "When this Spacecraft enters, create a 3/3 colorless Robot artifact creature token.",
            "When this Spacecraft enters, create a 2/2 white Robot artifact creature token.",
            "When this Spacecraft enters, target player creates a 2/2 colorless Robot artifact creature token.",
            "When this Spacecraft enters, each player creates a 2/2 colorless Robot artifact creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token under your control.",
            "When this Spacecraft enters, you may create a 2/2 colorless Robot artifact creature token."
        ),
    },
    Recipe {
        id: RecipeId("station.spacecraft.threshold_8_flying"),
        label: "Station 8+ Flying Spacecraft",
        surface: RecipeSurface::StationAssembly,
        matcher: match_station_8_flying_assembly,
        calibration: calibrations!(
            "Uthros Scanship" => "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Debris Field Crusher" => "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying";
            "Station",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)",
            "8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8 | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8-10 | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8 or more.)\n8+ | Flying",
            "Station (Tap this artifact: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its toughness on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put +1/+1 counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on that creature. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact Vehicle at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Add a mana cost. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "8+ | Flying\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Vigilance",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying, vigilance",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying\nDraw a card.",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying\n9+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Planet. Station only as a sorcery.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Activate only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as an instant. It's an artifact creature at 8+.)\n8+ | Flying"
        ),
    },
    Recipe {
        id: RecipeId("etb.spacecraft.uthros.draw_two_discard_one"),
        label: "Uthros Scanship ETB draw two then discard one",
        surface: RecipeSurface::EtbAbility,
        matcher: match_uthros_etb_draw_two_discard_one,
        calibration: singleton_calibrations!(
            "Uthros Scanship" => "When this Spacecraft enters, draw two cards, then discard a card.";
            "When this Spacecraft enters, discard a card, then draw two cards.",
            "When this Spacecraft enters, draw two cards, then discard two cards.",
            "When this Spacecraft enters, draw a card, then discard a card.",
            "When this Spacecraft enters, draw two cards, then discard a card. You may.",
            "When this Spacecraft enters, you draw two cards, then discard a card.",
            "When this Spacecraft enters, an opponent draws two cards, then discards a card.",
            "When this Spacecraft enters, draw two cards, then discard a card at random.",
            "When this artifact enters, draw two cards, then discard a card.",
            "Whenever this Spacecraft enters, draw two cards, then discard a card."
        ),
    },
    Recipe {
        id: RecipeId("etb.spacecraft.debris.damage_three_any_target"),
        label: "Debris Field Crusher ETB damage any target",
        surface: RecipeSurface::EtbAbility,
        matcher: match_debris_etb_damage_three_any_target,
        calibration: singleton_calibrations!(
            "Debris Field Crusher" => "When this Spacecraft enters, it deals 3 damage to any target.";
            "When this Spacecraft enters, it deals 2 damage to any target.",
            "When this Spacecraft enters, it deals 3 damage to target creature.",
            "When this Spacecraft enters, it deals 3 damage to target player.",
            "When this Spacecraft enters, it may deal 3 damage to any target.",
            "When this Spacecraft enters, deal 3 damage to any target.",
            "When this artifact enters, it deals 3 damage to any target.",
            "When this Spacecraft enters, it deals 3 damage to any target. Draw a card.",
            "Whenever this Spacecraft enters, it deals 3 damage to any target."
        ),
    },
    Recipe {
        id: RecipeId("activated.spacecraft.debris.pump_two_power"),
        label: "Debris Field Crusher pump +2/+0",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_debris_pump_two_power,
        calibration: singleton_calibrations!(
            "Debris Field Crusher" => "{1}{R}: This Spacecraft gets +2/+0 until end of turn.";
            "{1}{R}: This Spacecraft gets +1/+0 until end of turn.",
            "{1}{R}: This Spacecraft gets +2/+1 until end of turn.",
            "{1}{R}: This Spacecraft gets +2/+0 until your next turn.",
            "{1}{R}: This Spacecraft gets +2/+0 permanently.",
            "{1}{R}, {T}: This Spacecraft gets +2/+0 until end of turn.",
            "{1}{R}: Target Spacecraft gets +2/+0 until end of turn.",
            "{1}{R}: This Spacecraft gets +2/+0. Activate only as a sorcery.",
            "{R}: This Spacecraft gets +2/+0 until end of turn.",
            "{1}{R}: This Spacecraft gets +2/+0 until end of combat."
        ),
    },
    Recipe {
        id: RecipeId("spell.destroy.target_tapped_creature"),
        label: "destroy target tapped creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_destroy_tapped_creature,
        calibration: calibrations!(
            "Push" => "Destroy target tapped creature.",
            "Rip the Seams" => "Destroy target tapped creature.";
            "Destroy target untapped creature.",
            "Destroy target tapped artifact or creature.",
            "Destroy target tapped artifact.",
            "Destroy up to one target tapped creature.",
            "Destroy target tapped creature. You gain 1 life.",
            "Destroy target tapped creature. You gain 2 life.",
            "Destroy target tapped creature",
            "Destroy target tapped permanent.",
            "Destroy two target tapped creatures.",
            "Destroy target tapped creature you control."
        ),
    },
    Recipe {
        id: RecipeId("triggered.etb.destroy_up_to_one_artifact_or_enchantment"),
        label: "destroy up to one target artifact or enchantment on entry",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_destroy_up_to_one_artifact_or_enchantment,
        calibration: calibrations!(
            "Chomping Changeling" => "When this creature enters, destroy up to one target artifact or enchantment.",
            "Disruptive Stormbrood" => "When this creature enters, destroy up to one target artifact or enchantment.";
            "Destroy target artifact or enchantment.",
            "Destroy up to one target artifact or enchantment.",
            "Destroy up to one target artifact.",
            "Destroy up to one target enchantment.",
            "Destroy up to one target creature or enchantment.",
            "Destroy up to one target enchantment or artifact.",
            "When this artifact enters, destroy up to one target artifact or enchantment.",
            "When this creature enters, destroy target artifact or enchantment.",
            "When this creature enters, destroy up to two target artifacts or enchantments.",
            "When this creature enters, you may destroy up to one target artifact or enchantment.",
            "When this creature enters, destroy up to one target artifact or enchantment. You gain 1 life.",
            "Whenever this creature enters, destroy up to one target artifact or enchantment."
        ),
    },
    Recipe {
        id: RecipeId("triggered.etb.exile_up_to_two_target_cards_from_a_single_graveyard"),
        label: "exile up to two target cards from a single graveyard on entry",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_exile_up_to_two_cards_from_single_graveyard,
        calibration: calibrations!(
            "Griffnaut Tracker" => "When this creature enters, exile up to two target cards from a single graveyard.",
            "Feral Deathgorger" => "When this creature enters, exile up to two target cards from a single graveyard.";
            "When this creature enters, exile up to two target cards from a graveyard.",
            "When this creature enters, exile up to one target card from a single graveyard.",
            "When this creature enters, exile up to two target creature cards from a single graveyard.",
            "When this creature enters, exile up to three target cards from a single graveyard.",
            "When this creature enters, exile target card from a single graveyard.",
            "When this creature enters, exile up to two target cards from a single graveyard. If at least one creature card was exiled this way, each opponent loses 2 life and you gain 2 life.",
            "When this creature enters, exile up to two target cards from a single graveyard. You gain 1 life.",
            "Whenever this creature attacks, exile up to two target cards from a single graveyard.",
            "At the beginning of combat on your turn, exile up to two target cards from a single graveyard.",
            "When this creature enters, exile up to two target cards from target opponent's graveyard."
        ),
    },
    Recipe {
        id: RecipeId("triggered.controller_casts_noncreature.ping_each_opponent_one"),
        label: "noncreature cast pings each opponent for one",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_controller_casts_noncreature_ping_each_opponent_one,
        calibration: calibrations!(
            "Firebrand Archer" => "Whenever you cast a noncreature spell, this creature deals 1 damage to each opponent.",
            "Coruscation Mage" => "Whenever you cast a noncreature spell, this creature deals 1 damage to each opponent.";
            "Whenever you cast a spell, this creature deals 1 damage to each opponent.",
            "Whenever an opponent casts a noncreature spell, this creature deals 1 damage to each opponent.",
            "Whenever you cast a creature spell, this creature deals 1 damage to each opponent.",
            "Whenever you cast a noncreature spell, this creature deals 2 damage to each opponent.",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to target opponent.",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to each player.",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to target creature an opponent controls.",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to each opponent. You gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump.target_creature_plus_n_zero_first_strike"),
        label: "creature +N/+0 and first strike",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_pump_creature_plus_n_zero_first_strike,
        calibration: calibrations!(
            "Kindled Fury" => "Target creature gets +1/+0 and gains first strike until end of turn.",
            "Sure Strike" => "Target creature gets +3/+0 and gains first strike until end of turn.";
            "Target creature gets +0/+0 and gains first strike until end of turn.",
            "Target creature gets +0/+1 and gains first strike until end of turn.",
            "Target creature gets +1/+1 and gains first strike until end of turn.",
            "Target creature gets +2/+2 and gains first strike until end of turn.",
            "Target creature gets +1/+0 and gains trample until end of turn.",
            "Target creature gets +1/+0 and gains double strike until end of turn.",
            "Target creature gets +1/+0 and gains first strike until end of combat.",
            "Target creature gets +1/+0 and gains first strike until your next turn.",
            "Target creature you control gets +1/+0 and gains first strike until end of turn.",
            "Creatures you control get +1/+0 and gain first strike until end of turn.",
            "Up to one target creature gets +1/+0 and gains first strike until end of turn.",
            "Target creature gets +1/+0 and gains first strike until end of turn. Investigate.",
            "Target creature gets +1/+0 and gains first strike until end of turn. If this spell was kicked, that creature gains trample until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("spell.destroy.target_creature_power_n_or_less"),
        label: "destroy creature with power N or less",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_destroy_creature_power_at_most,
        calibration: calibrations!(
            "Defeat" => "Destroy target creature with power 2 or less.",
            "Reave Soul" => "Destroy target creature with power 3 or less.";
            "Destroy target creature with power 3 or greater.",
            "Destroy target creature with power 2 or greater.",
            "Destroy target creature with power 0 or less.",
            "Destroy target creature with power three or less.",
            "Destroy target creature with power 3 or less",
            "Destroy up to one target creature with power 3 or less.",
            "Destroy target creature with toughness 3 or less.",
            "Destroy target creature with toughness 4 or greater.",
            "Destroy target artifact creature with power 3 or less.",
            "Destroy two target creatures with power 3 or less.",
            "Destroy target creature with power 3 or less. You gain 1 life."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_attacks.vehicle.create_treasure"),
        label: "Vehicle attack creates a Treasure",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_attacks_vehicle_create_treasure,
        calibration: calibrations!(
            "Careening Mine Cart" => "Whenever this Vehicle attacks, create a Treasure token.",
            "Rocketeer Boostbuggy" => "Whenever this Vehicle attacks, create a Treasure token.";
            "Whenever this creature attacks, create a Treasure token.",
            "Whenever another Vehicle you control attacks, create a Treasure token.",
            "Whenever this Vehicle attacks, you may create a Treasure token.",
            "Whenever this Vehicle deals combat damage to a player, create a Treasure token.",
            "Whenever this Vehicle attacks, create two Treasure tokens.",
            "Whenever this Vehicle attacks, create a tapped Treasure token.",
            "Whenever this Vehicle attacks, create a Treasure token. Draw a card.",
            "Whenever this Vehicle attacks, create a Treasure token"
        ),
    },
    Recipe {
        id: RecipeId("spell.return_graveyard.creature.cards.hand.up_to_two"),
        label: "return up to two target creature cards from your graveyard to your hand",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_return_up_to_two_graveyard_creature_cards,
        calibration: calibrations!(
            "Fight On!" => "Return up to two target creature cards from your graveyard to your hand.",
            "Macabre Reconstruction" => "Return up to two target creature cards from your graveyard to your hand.",
            "Sanguine Indulgence" => "Return up to two target creature cards from your graveyard to your hand.";
            "Return target creature card from your graveyard to your hand.",
            "Return up to one target creature card from your graveyard to your hand.",
            "Return up to two target cards from your graveyard to your hand.",
            "Return up to two target creature cards from a graveyard to your hand.",
            "Return up to two target creature cards from your graveyard to the battlefield.",
            "Return up to two target creature cards from an opponent's graveyard to your hand.",
            "Return up to three target creature cards from your graveyard to your hand.",
            "Return up to two target creature cards from your graveyard to your hand. You gain 2 life.",
            "Return two target creature cards from your graveyard to your hand."
        ),
    },
    Recipe {
        id: RecipeId("activated.tap_creature.add_any_color"),
        label: "tap a creature for any color",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_artifact_tap_creature_add_any_color,
        calibration: calibrations!(
            "Springleaf Drum" => "{T}, Tap an untapped creature you control: Add one mana of any color.",
            "Dragonbroods' Relic" => "{T}, Tap an untapped creature you control: Add one mana of any color.",
            "Scene of the Crime" => "{T}, Tap an untapped creature you control: Add one mana of any color.";
            "{T}, Tap an untapped artifact you control: Add one mana of any color.",
            "{T}, Tap two untapped creatures you control: Add one mana of any color.",
            "{T}, Tap an untapped creature you control: Add {G}.",
            "{T}, Tap an untapped creature you control: Add one mana of any one color.",
            "{T}, Tap an untapped creature you control: Add one mana of any type.",
            "{T}, Exile a creature you control: Add one mana of any color.",
            "{T}, Pay 1 life, Tap an untapped creature you control: Add one mana of any color.",
            "{T}, Tap an untapped creature you control: Add one mana of any color. Draw a card.",
            "{T}, Tap an untapped creature an opponent controls: Add one mana of any color."
        ),
    },
    Recipe {
        id: RecipeId("static.self.cannot_be_blocked_by_power_n_or_less"),
        label: "self cannot be blocked by power N or less",
        surface: RecipeSurface::StaticAbility,
        matcher: match_self_cannot_be_blocked_by_power_n_or_less,
        calibration: calibrations!(
            "Stormkeld Vanguard" => "This creature can't be blocked by creatures with power 2 or less.",
            "Gate Colossus" => "This creature can't be blocked by creatures with power 2 or less.",
            "Bristlebane Outrider" => "This creature can't be blocked by creatures with power 2 or less.",
            "Old Fat Spider" => "This creature can't be blocked by creatures with power 2 or less.";
            "This creature can't be blocked.",
            "This creature can't be blocked by creatures with power 2 or greater.",
            "This creature can't be blocked by creatures with power 0 or less.",
            "This creature can't be blocked by Walls.",
            "This creature can't be blocked except by creatures with power 2 or greater.",
            "This creature can't be blocked by creatures with power 2.",
            "This creature can block only creatures with power 2 or greater.",
            "This creature can't be blocked by creatures with power two or less.",
            "This creature can't be blocked by creatures with toughness 2 or less.",
            "This creature can't be blocked by artifacts with power 2 or less.",
            "This creature can't be blocked by creatures with power 2 or less. It can't be blocked by Walls."
        ),
    },
    Recipe {
        id: RecipeId("spell.create_clue_token"),
        label: "create a Clue",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_create_clue_token,
        calibration: calibrations!(
            "Cunning Maneuver" => "Create a Clue token.",
            "True Ancestry" => "Create a Clue token.",
            "Jet's Brainwashing" => "Create a Clue token.";
            "Create a Clue token. Draw a card.",
            "Create two Clue tokens.",
            "Create a tapped Clue token.",
            "Investigate.",
            "Create a Food token.",
            "Create a tapped Treasure token.",
            "Create a Clue token",
            "Create a colorless Clue token.",
            "Create a 1/1 Clue token.",
            "Create a Clue artifact token."
        ),
    },
    Recipe {
        id: RecipeId("activated.graveyard.return_self_to_hand"),
        label: "graveyard activation returns this card to hand",
        surface: RecipeSurface::ZoneActivatedAbility,
        matcher: match_graveyard_return_self_to_hand,
        calibration: calibrations!(
            "Project Deathlok Soldier" => "{2}{B}: Return this card from your graveyard to your hand.",
            "Abzan Devotee" => "{2}{B}: Return this card from your graveyard to your hand.";
            "{2}{B}: Return this card from your graveyard to the battlefield.",
            "{2}{B}: Return this card from your graveyard to the battlefield tapped.",
            "{2}{B}: Return target creature card from your graveyard to your hand.",
            "{2}{B}: Return this card from your graveyard to your hand. Activate only as a sorcery.",
            "{2}{B}, Exile this card from your graveyard: Return this card from your graveyard to your hand.",
            "{2}{B}: Return this card from the graveyard to your hand.",
            "{2}{B}: Return this card from your graveyard to its owner's hand.",
            "{2}{B}: Return this creature card from your graveyard to your hand.",
            "Return this card from your graveyard to your hand.",
            "{X}{B}: Return this card from your graveyard to your hand."
        ),
    },
    Recipe {
        id: RecipeId("spell.put_in_owners_library.owner_choice_top_or_bottom.target_creature"),
        label: "target creature's owner chooses top or bottom of their library",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_owner_choice_top_or_bottom_target_creature,
        calibration: calibrations!(
            "Misleading Motes" => "Target creature's owner puts it on their choice of the top or bottom of their library.",
            "Run Behind" => "Target creature's owner puts it on their choice of the top or bottom of their library.",
            "Uneasy Partings" => "Target creature's owner puts it on their choice of the top or bottom of their library.",
            "Dire Downdraft" => "Target creature's owner puts it on their choice of the top or bottom of their library.";
            "Put target creature on the bottom of its owner's library.",
            "Put target creature on top of its owner's library.",
            "Target creature's owner puts it on their choice of the top or bottom of their library. Surveil 1.",
            "Target creature's owner shuffles it into their library.",
            "Target nonland permanent's owner puts it on their choice of the top or bottom of their library.",
            "The owner of target spell or nonland permanent puts it on their choice of the top or bottom of their library.",
            "Target artifact's owner puts it on their choice of the top or bottom of their library.",
            "Target creature's owner puts it on the top or bottom of their library.",
            "Target creature's owner puts it on their choice of the top or bottom of their library",
            "Target creature's owner puts it on their choice of the top or bottom of their graveyard.",
            "Target creature's owner puts it on their choice of the top or bottom of their library. Draw a card.",
            "Put target creature on its owner's choice of the top or bottom of their library.",
            "Target creature's owner puts it on their choice of the second from the top or bottom of their library."
        ),
    },
    Recipe {
        id: RecipeId("spell.cost_reduction.target_match_attacking_creature.one"),
        label: "spell costs one less to cast when it targets an attacking creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_attacking_creature_target_reduction_one,
        calibration: calibrations!(
            "Guidance Failure" => "This spell costs {1} less to cast if it targets an attacking creature.",
            "Run Behind" => "This spell costs {1} less to cast if it targets an attacking creature.";
            "This spell costs {1} less to cast if it targets a tapped creature.",
            "This spell costs {1} less to cast if it targets an attacking nontoken creature.",
            "This spell costs {1} less to cast if it targets an attacking or tapped creature.",
            "This spell costs {2} less to cast if it targets an attacking creature.",
            "This spell costs {1} less to cast if you control a creature.",
            "This spell costs {1} less to cast.",
            "This spell costs {1} less to cast if it targets an attacking creature. Draw a card.",
            "This spell costs {1} less to cast if it targets an attacking creature"
        ),
    },
    Recipe {
        id: RecipeId("triggered.etb.create_rat_token.cant_block"),
        label: "creature ETB create one Rat token that can't block",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_create_rat_token_cant_block,
        calibration: calibrations!(
            "Edgewall Pack" => r#"When this creature enters, create a 1/1 black Rat creature token with "This token can't block.""#,
            "Voracious Vermin" => r#"When this creature enters, create a 1/1 black Rat creature token with "This token can't block.""#;
            r#"When this creature dies, create a 1/1 black Rat creature token with "This token can't block.""#,
            "When this creature enters, create a 1/1 black Rat creature token.",
            r#"When this creature enters, create two 1/1 black Rat creature tokens with "This token can't block.""#,
            r#"When this creature enters, create a tapped 1/1 black Rat creature token with "This token can't block.""#,
            r#"When this creature enters, create a 1/1 black Rat creature token with "This creature can't block.""#,
            r#"When this creature enters, create a 1/1 black Rat creature token with "This token can't block." Draw a card."#,
            r#"When this creature enters, create a 1/1 black Rat creature token with "This token can't attack.""#
        ),
    },
    Recipe {
        id: RecipeId("triggered.etb.target_creature_gains_flying"),
        label: "creature ETB grant flying to target creature until end of turn",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_target_creature_gains_flying,
        calibration: calibrations!(
            "Gale Swooper" => "When this creature enters, target creature gains flying until end of turn.",
            "Stratosoarer" => "When this creature enters, target creature gains flying until end of turn.",
            "Nephalia Moondrakes" => "When this creature enters, target creature gains flying until end of turn.";
            "When this creature enters, target creature you control gains flying until end of turn.",
            "When this creature enters, target creature gains flying.",
            "When this creature enters, target creature gains flying and hexproof until end of turn.",
            "When this creature enters, target creature gets +1/+0 and gains flying until end of turn.",
            "When this creature enters, up to one target creature gains flying until end of turn.",
            "When this Equipment enters, attach it to target creature you control. That creature gains flying until end of turn.",
            "When this creature enters, target creature gains flying until end of turn. Draw a card.",
            "When this creature enters, target creature gains first strike until end of turn.",
            "When this creature enters, target creature gains flying until end of combat."
        ),
    },
    Recipe {
        id: RecipeId("triggered.self_damage_to_opponent.draw"),
        label: "self damage to an opponent draws one card",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_self_damage_to_opponent_draw_one,
        calibration: calibrations!(
            "Thieving Magpie" => "Whenever this creature deals damage to an opponent, draw a card.",
            "Thieving Otter" => "Whenever this creature deals damage to an opponent, draw a card.";
            "Whenever this creature deals combat damage to a player, draw a card.",
            "Whenever this creature deals damage to a player, draw a card.",
            "Whenever this creature deals damage to an opponent, you may draw a card.",
            "Whenever this creature deals damage to an opponent, draw two cards.",
            "Whenever this creature deals damage to an opponent, create a Treasure token.",
            "Whenever another creature deals damage to an opponent, draw a card.",
            "Whenever this creature deals damage to an opponent, draw a card. You gain 1 life.",
            "Whenever this creature deals damage to an opponent, draw a card"
        ),
    },
    Recipe {
        id: RecipeId("spell.destroy_all.creatures"),
        label: "destroy all creatures",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_destroy_all_creatures,
        calibration: calibrations!(
            "Day of Judgment" => "Destroy all creatures.",
            "Supreme Verdict" => "Destroy all creatures.",
            "Doomskar" => "Destroy all creatures.";
            "Destroy all nonland permanents.",
            "Destroy all creatures you don't control.",
            "Destroy all creatures with flying.",
            "Destroy all creatures. They can't be regenerated.",
            "Exile all creatures.",
            "Destroy all creatures with power 4 or greater.",
            "Destroy all creatures"
        ),
    },
    Recipe {
        id: RecipeId("spell.counter.target_spell.unless_pays.two"),
        label: "counter target spell unless its controller pays two",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_counter_unless_pays_two,
        calibration: calibrations!(
            "It'll Quench Ya!" => "Counter target spell unless its controller pays {2}.",
            "Quench" => "Counter target spell unless its controller pays {2}.",
            "Miscalculation" => "Counter target spell unless its controller pays {2}.";
            "Counter target spell unless its controller pays {1}.",
            "Counter target spell unless its controller pays {3}.",
            "Counter target noncreature spell unless its controller pays {2}.",
            "Counter target spell unless its controller pays {2} for each card in your graveyard.",
            "Counter target spell unless its controller pays {2}",
            "Counter target spell unless its controller pays {2}. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("spell.grant_keywords.target_creature.double_strike"),
        label: "target creature gains double strike until end of turn",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_grant_double_strike,
        calibration: calibrations!(
            "Two-Headed Hunter // Twice the Rage" => "Target creature gains double strike until end of turn.",
            "Temur Battle Rage" => "Target creature gains double strike until end of turn.",
            "Assault Strobe" => "Target creature gains double strike until end of turn.";
            "Target creature gains double strike until end of turn. Untap it.",
            "Target creature gets +1/+0 and gains double strike until end of turn.",
            "Creatures you control gain double strike until end of turn.",
            "Target creature gains first strike until end of turn.",
            "Target creature gains double strike.",
            "Target creature gains double strike until end of turn. Scry 1.",
            "Target creature gains double strike until end of combat."
        ),
    },
    Recipe {
        id: RecipeId("spell.create_treasure_token"),
        label: "create one Treasure token",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_create_treasure_token,
        calibration: calibrations!(
            "Ancestors' Aid" => "Create a Treasure token.",
            "Prizefight" => "Create a Treasure token.",
            "Strike It Rich" => "Create a Treasure token.";
            "Create two Treasure tokens.",
            "Create a tapped Treasure token.",
            "Create a Treasure token. You gain 1 life.",
            "Create a Treasure token, then draw a card.",
            "Create a Food token.",
            "Create a Treasure token"
        ),
    },
    Recipe {
        id: RecipeId("spell.search_library.basic_land.battlefield_tapped"),
        label: "search library for a basic land onto the battlefield tapped",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_search_basic_land_battlefield_tapped,
        calibration: calibrations!(
            "Shared Roots" => "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "Thunderherd Migration" => "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "Natural Connection" => "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.";
            "Search your library for a basic land card, reveal it, put it into your hand, then shuffle.",
            "Search your library for a basic land card, put it onto the battlefield, then shuffle.",
            "Search your library for a land card, put it onto the battlefield tapped, then shuffle.",
            "Search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle.",
            "You may search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle"
        ),
    },
    Recipe {
        id: RecipeId("static.other_creatures_you_control.have_trample"),
        label: "other creatures you control have trample",
        surface: RecipeSurface::StaticAbility,
        matcher: match_static_other_creatures_trample,
        calibration: calibrations!(
            "Aggressive Mammoth" => "Other creatures you control have trample.",
            "Nylea's Forerunner" => "Other creatures you control have trample.",
            "Khenra Charioteer" => "Other creatures you control have trample.";
            "Creatures you control have trample.",
            "Other creatures you control get +1/+1.",
            "Other creatures you control have trample and haste.",
            "Other attacking creatures you control have trample.",
            "Other creatures you control with power 4 or greater have trample.",
            "Other creatures you control have trample"
        ),
    },
    Recipe {
        id: RecipeId("spell.target_creature_cant_be_blocked_this_turn"),
        label: "target creature can't be blocked this turn",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_target_creature_cant_be_blocked,
        calibration: calibrations!(
            "Enter the Enigma" => "Target creature can't be blocked this turn.",
            "Infiltrate" => "Target creature can't be blocked this turn.",
            "Artful Dodge" => "Target creature can't be blocked this turn.";
            "Target creature can't block this turn.",
            "Target creature can't be blocked this combat.",
            "Target creature is unblockable this turn.",
            "Up to one target creature can't be blocked this turn.",
            "Target creature can't be blocked this turn. Draw a card.",
            "Target creature can't be blocked this turn"
        ),
    },
    Recipe {
        id: RecipeId("static.self.cannot_be_blocked_by_more_than_one_creature"),
        label: "self can't be blocked by more than one creature",
        surface: RecipeSurface::StaticAbility,
        matcher: match_static_self_max_one_blocker,
        calibration: calibrations!(
            "Professional Wrestler" => "This creature can't be blocked by more than one creature.",
            "Charging Rhino" => "This creature can't be blocked by more than one creature.",
            "Stalking Tiger" => "This creature can't be blocked by more than one creature.";
            "This creature can't be blocked.",
            "This creature can't be blocked by more than two creatures.",
            "This creature can't be blocked by creatures with power 2 or greater.",
            "This creature can't be blocked except by two or more creatures.",
            "This creature can block only creatures with power 2 or less.",
            "This creature can't be blocked by more than one creature"
        ),
    },
    Recipe {
        id: RecipeId("spell.put_counter.target_creature.one"),
        label: "put a +1/+1 counter on target creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_put_plus_one_counter_target_creature,
        calibration: calibrations!(
            "Honor" => "Put a +1/+1 counter on target creature.",
            "Battlegrowth" => "Put a +1/+1 counter on target creature.",
            "Guiding Voice" => "Put a +1/+1 counter on target creature.";
            "Put a +1/+1 counter on each creature you control.",
            "Put two +1/+1 counters on target creature.",
            "Put a +1/+1 counter on up to one target creature.",
            "Put a +1/+1 counter on target creature you control.",
            "Put a -1/-1 counter on target creature.",
            "Put a +1/+1 counter on target creature. Draw a card.",
            "Put a +1/+1 counter on target creature"
        ),
    },
    Recipe {
        id: RecipeId("activated.self_pump.plus_one_zero"),
        label: "single colored mana self-pump plus one zero",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_activated_creature_self_pump_plus_one_zero,
        calibration: calibrations!(
            "Shivan Dragon" => "{R}: This creature gets +1/+0 until end of turn.",
            "Inferno Titan" => "{R}: This creature gets +1/+0 until end of turn.",
            "Scourge of Valkas" => "{R}: This creature gets +1/+0 until end of turn.";
            "{1}{R}: This creature gets +1/+0 until end of turn.",
            "{R}: This creature gets +2/+0 until end of turn.",
            "{R}: This creature gets +1/+1 until end of turn.",
            "{R}: This creature gets +1/+0 until your next turn.",
            "{R}: Target creature gets +1/+0 until end of turn.",
            "{R}, {T}: This creature gets +1/+0 until end of turn.",
            "{R}: This creature gets +1/+0 until end of turn. Activate only as a sorcery.",
            "{R}: This creature gets +1/+0 until end of turn"
        ),
    },
    Recipe {
        id: RecipeId("spell.pump.creature.minus_four_minus_four"),
        label: "target creature minus four minus four",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_creature_minus_four_four,
        calibration: calibrations!(
            "Dark Deed" => "Target creature gets -4/-4 until end of turn.",
            "Grasp of Darkness" => "Target creature gets -4/-4 until end of turn.",
            "Flatten" => "Target creature gets -4/-4 until end of turn.";
            "Target creature gets -5/-5 until end of turn.",
            "Target creature gets -4/-0 until end of turn.",
            "Creatures you control get -4/-4 until end of turn.",
            "Target creature gets -4/-4 until end of combat.",
            "Target creature gets -4/-4 until end of turn. You gain 1 life.",
            "Up to one target creature gets -4/-4 until end of turn.",
            "Target creature an opponent controls gets -4/-4 until end of turn.",
            "Target creature gets -4/-4 until end of turn"
        ),
    },
    Recipe {
        id: RecipeId("static.anthem.attacking_creatures_you_control.plus_one_zero"),
        label: "attacking creatures you control get plus one zero",
        surface: RecipeSurface::StaticAbility,
        matcher: match_static_attacking_creatures_anthem_plus_one_zero,
        calibration: calibrations!(
            "Goblin Oriflamme" => "Attacking creatures you control get +1/+0.",
            "Orcish Oriflamme" => "Attacking creatures you control get +1/+0.",
            "Warded Battlements" => "Attacking creatures you control get +1/+0.";
            "Creatures you control get +1/+0.",
            "Attacking creatures you control get +1/+1.",
            "Attacking creatures you control get +2/+0.",
            "Other attacking creatures you control get +1/+0.",
            "Attacking creatures an opponent controls get +1/+0.",
            "Attacking creatures you control get +1/+0 until end of turn.",
            "Attacking creatures you control get +1/+0"
        ),
    },
    Recipe {
        id: RecipeId("triggered.end_step.sacrifice_self"),
        label: "end step sacrifice this creature",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_triggered_end_step_sacrifice_self,
        calibration: calibrations!(
            "Ball Lightning" => "At the beginning of the end step, sacrifice this creature.",
            "Spark Elemental" => "At the beginning of the end step, sacrifice this creature.",
            "Hell's Thunder" => "At the beginning of the end step, sacrifice this creature.";
            "At the beginning of your end step, sacrifice this creature.",
            "At the beginning of the next end step, sacrifice this creature.",
            "At the beginning of each end step, sacrifice this creature.",
            "At the beginning of your upkeep, sacrifice this creature.",
            "At the beginning of the end step, sacrifice this artifact.",
            "At the beginning of the end step, sacrifice another creature.",
            "At the beginning of the end step, sacrifice this creature"
        ),
    },
    Recipe {
        id: RecipeId("activated.tap_discard.draw_one"),
        label: "tap and discard a card to draw a card",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_activated_tap_discard_draw_one,
        calibration: calibrations!(
            "Charging Strifeknight" => "{T}, Discard a card: Draw a card.",
            "Rummaging Goblin" => "{T}, Discard a card: Draw a card.",
            "Mad Prophet" => "{T}, Discard a card: Draw a card.";
            "{T}: Draw a card, then discard a card.",
            "{T}, Discard a card: Draw two cards.",
            "{T}, Discard two cards: Draw a card.",
            "{1}, Discard a card: Draw a card.",
            "{T}, Discard a card: Draw a card. Activate only as a sorcery.",
            "{T}, Sacrifice this creature: Draw a card.",
            "{T}, Discard a card: Draw a card"
        ),
    },
    Recipe {
        id: RecipeId("triggered.etb.put_counter.each_other_creature"),
        label: "creature ETB put a +1/+1 counter on each other controlled creature",
        surface: RecipeSurface::EtbAbility,
        matcher: match_etb_put_counter_each_other_creature,
        calibration: calibrations!(
            "Web-Warriors" => "When this creature enters, put a +1/+1 counter on each other creature you control.",
            "Ridgescale Tusker" => "When this creature enters, put a +1/+1 counter on each other creature you control.",
            "Primeval Protector" => "When this creature enters, put a +1/+1 counter on each other creature you control.";
            "When this creature enters, put a +1/+1 counter on each creature you control.",
            "When this creature enters, put a +1/+1 counter on each other creature.",
            "When this creature enters, put two +1/+1 counters on each other creature you control.",
            "When this creature enters, put a +1/+1 counter on each other creature you don't control.",
            "When another creature enters, put a +1/+1 counter on each other creature you control.",
            "When this creature enters, put a +1/+1 counter on each other creature you control"
        ),
    },
    Recipe {
        id: RecipeId("triggered.another_creature_you_control_dies.put_counter_self"),
        label: "another controlled creature dies put a +1/+1 counter on this creature",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_triggered_another_creature_dies_put_counter_self,
        calibration: calibrations!(
            "Voracious Vermin" => "Whenever another creature you control dies, put a +1/+1 counter on this creature.",
            "Rot Shambler" => "Whenever another creature you control dies, put a +1/+1 counter on this creature.",
            "Unruly Mob" => "Whenever another creature you control dies, put a +1/+1 counter on this creature.";
            "Whenever a creature you control dies, put a +1/+1 counter on this creature.",
            "Whenever another creature dies, put a +1/+1 counter on this creature.",
            "Whenever another creature you control dies, put two +1/+1 counters on this creature.",
            "Whenever another creature you control dies, put a +1/+1 counter on target creature.",
            r#"Whenever another creature you control dies, create a 1/1 black Rat creature token with "This token can't block.""#,
            "Whenever another creature you control dies, put a +1/+1 counter on this creature"
        ),
    },
    Recipe {
        id: RecipeId("spell.exile.target_attacking_creature"),
        label: "exile target attacking creature",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_exile_attacking_creature,
        calibration: calibrations!(
            "Not on My Watch" => "Exile target attacking creature.",
            "Resounding Silence" => "Exile target attacking creature.",
            "Second Thoughts" => "Exile target attacking creature.";
            "Exile target attacking or blocking creature.",
            "Exile target blocking creature.",
            "Exile target attacking creature you control.",
            "Exile up to one target attacking creature.",
            "Exile target attacking creature. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("activated.sacrifice_self.destroy_target_enchantment"),
        label: "sacrifice this creature to destroy target enchantment",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_activated_sacrifice_self_destroy_enchantment,
        calibration: calibrations!(
            "Felidar Cub" => "Sacrifice this creature: Destroy target enchantment.",
            "Kami of Ancient Law" => "Sacrifice this creature: Destroy target enchantment.",
            "Ronom Unicorn" => "Sacrifice this creature: Destroy target enchantment.";
            "{1}, Sacrifice this creature: Destroy target enchantment.",
            "Sacrifice this creature: Destroy target artifact.",
            "Sacrifice this creature: Destroy target artifact or enchantment.",
            "Sacrifice another creature: Destroy target enchantment.",
            "Sacrifice this creature: Exile target enchantment.",
            "Sacrifice this creature: Destroy target enchantment. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.upkeep.self_damage_controller_one"),
        label: "upkeep self damage one to controller",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_triggered_upkeep_self_damage_controller_one,
        calibration: calibrations!(
            "Ravenous Giant" => "At the beginning of your upkeep, this creature deals 1 damage to you.",
            "Nettletooth Djinn" => "At the beginning of your upkeep, this creature deals 1 damage to you.",
            "Serendib Efreet" => "At the beginning of your upkeep, this creature deals 1 damage to you.";
            "At the beginning of your upkeep, this creature deals 2 damage to you.",
            "At the beginning of each upkeep, this creature deals 1 damage to you.",
            "At the beginning of your upkeep, this creature deals 1 damage to each opponent.",
            "At the beginning of your upkeep, this creature deals 1 damage to target player.",
            "At the beginning of your end step, this creature deals 1 damage to you.",
            "At the beginning of your upkeep, this creature deals 1 damage to you. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("activated.tap.surveil_one"),
        label: "tap to surveil one",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_activated_tap_surveil_one,
        calibration: calibrations!(
            "Rune-Sealed Wall" => "{T}: Surveil 1.",
            "Sinister Starfish" => "{T}: Surveil 1.",
            "Microscope" => "{T}: Surveil 1.";
            "{1}, {T}: Surveil 1.",
            "{T}: Surveil 2.",
            "{T}: Scry 1.",
            "{T}: Surveil 1. Activate only as a sorcery.",
            "{T}, Pay 1 life: Surveil 1.",
            "{T}: Surveil 1. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("activated.tap.target_creature_gains_haste"),
        label: "tap to grant target creature haste until end of turn",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_activated_tap_target_creature_gains_haste,
        calibration: calibrations!(
            "Axgard Cavalry" => "{T}: Target creature gains haste until end of turn.",
            "Akki Drillmaster" => "{T}: Target creature gains haste until end of turn.",
            "Bloodlust Inciter" => "{T}: Target creature gains haste until end of turn.";
            "{T}: Target creature you control gains haste until end of turn.",
            "{T}: This creature gains haste until end of turn.",
            "{1}, {T}: Target creature gains haste until end of turn.",
            "{T}: Target creature gains haste.",
            "{T}: Target creature gains haste and trample until end of turn.",
            "{T}: Target creature gets +1/+0 and gains haste until end of turn.",
            "{T}: Target creature gains haste until end of turn. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.etb.return_target_permanent_card_to_hand"),
        label: "creature ETB return target permanent card to hand",
        surface: RecipeSurface::EtbAbility,
        matcher: match_triggered_etb_return_permanent_card_to_hand,
        calibration: calibrations!(
            "Elvish Regrower" => "When this creature enters, return target permanent card from your graveyard to your hand.",
            "Gloomshrieker" => "When this creature enters, return target permanent card from your graveyard to your hand.",
            "Golgari Findbroker" => "When this creature enters, return target permanent card from your graveyard to your hand.";
            "When this creature enters, return target creature card from a graveyard to your hand.",
            "When this creature enters, return target permanent card from your graveyard to the battlefield.",
            "When this creature enters, return target permanent card from an opponent's graveyard to your hand.",
            "When this creature enters, you may return target permanent card from your graveyard to your hand.",
            "When this creature dies, return target permanent card from your graveyard to your hand.",
            "When this creature enters, return target permanent card from your graveyard to your hand. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("spell.up_to_two_target_creatures_cant_block"),
        label: "up to two target creatures can't block this turn",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_up_to_two_target_creatures_cant_block,
        calibration: calibrations!(
            "Bellowing Bruiser // Beat a Path" => "Up to two target creatures can't block this turn.",
            "Abandon the Post" => "Up to two target creatures can't block this turn.",
            "Nightbird's Clutches" => "Up to two target creatures can't block this turn.";
            "Target creature can't block this turn.",
            "Up to two target creatures you control can't block this turn.",
            "Up to two target creatures can't attack this turn.",
            "Up to two target creatures can't block.",
            "Up to one target creature can't block this turn.",
            "Up to two target creatures can't block this turn. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("spell.pump.target_creature_plus_one_zero_first_strike_scry_one"),
        label: "creature +1/+0 and first strike then scry one",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_pump_first_strike_scry_one,
        calibration: calibrations!(
            "Kindled Heroism" => "Target creature gets +1/+0 and gains first strike until end of turn. Scry 1.",
            "Coming In Hot" => "Target creature gets +1/+0 and gains first strike until end of turn. Scry 1.",
            "Storm Strike" => "Target creature gets +1/+0 and gains first strike until end of turn. Scry 1.";
            "Target creature gets +1/+0 and gains first strike until end of turn. Scry 2.",
            "Target creature gets +1/+0 and gains first strike until end of turn. Investigate.",
            "Target creature gets +1/+1 and gains first strike until end of turn. Scry 1.",
            "Target creature gains first strike until end of turn. Scry 1.",
            "Target creature gets +1/+0 and gains first strike until end of turn. Scry 1. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("cast_method.warp"),
        label: "warp alternative cast cost",
        surface: RecipeSurface::CastMethodClause,
        matcher: match_cast_method_warp,
        calibration: calibrations!(
            "Bygone Colossus" => "Warp {3}",
            "Germinating Wurm" => "Warp {1}{G}",
            "Red Tiger Mechan" => "Warp {1}{R}",
            "Starbreach Whale" => "Warp {1}{U}";
            "Warp {1}{G} with a rider",
            "Warp — {1}{G}",
            "Warp  {1}{G}",
            "Warp {1}{G}.",
            "Warp {1}{G} and draw a card.",
            "Warp {01}{G}",
            "{G}: Warp this creature."
        ),
    },
    Recipe {
        id: RecipeId("cast_method.flashback"),
        label: "flashback alternative cast cost",
        surface: RecipeSurface::CastMethodClause,
        matcher: match_cast_method_flashback,
        calibration: calibrations!(
            "Think Twice" => "Flashback {2}{U}",
            "Auron's Inspiration" => "Flashback {2}{W}{W}",
            "Daydream" => "Flashback {2}{W}";
            "Flashback {2}{U} with a rider",
            "Flashback — {2}{U}",
            "Flashback  {2}{U}",
            "Flashback {2}{U}.",
            "Flashback {2}{U}. Draw a card.",
            "Flashback 2",
            "Flashback {02}{U}"
        ),
    },
    Recipe {
        id: RecipeId("spell.target_opponent_reveal.discard_nonland"),
        label: "target opponent reveals and you choose a nonland card to discard",
        surface: RecipeSurface::SpellClause,
        matcher: match_spell_target_opponent_reveal_discard_nonland,
        calibration: calibrations!(
            "Pilfer" => "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card.",
            "Render Speechless" => "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card.",
            "Dark Inquiry" => "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card.";
            "Target player reveals their hand. You choose a nonland card from it. That player discards that card.",
            "Target opponent reveals their hand. You choose a card from it. That player discards that card.",
            "Target opponent reveals their hand. You choose a nonland card from it. That player exiles that card.",
            "Target opponent reveals their hand. You choose two nonland cards from it. That player discards those cards.",
            "Target opponent reveals their hand. You may choose a nonland card from it. That player discards that card.",
            "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card. Draw a card.",
            "Target opponent reveals their hand. You choose a nonland card from it.",
            "You choose a nonland card from target opponent's revealed hand. That player discards that card."
        ),
    },
    Recipe {
        id: RecipeId("triggered.landfall.gain_life_one"),
        label: "landfall gain one life",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_landfall_gain_life_one,
        calibration: calibrations!(
            "Eumidian Terrabotanist" => "Landfall — Whenever a land you control enters, you gain 1 life.",
            "Jaddi Offshoot" => "Landfall — Whenever a land you control enters, you gain 1 life.",
            "Kazandu Nectarpot" => "Landfall — Whenever a land you control enters, you gain 1 life.";
            "Landfall — Whenever a land you control enters, you gain 2 life.",
            "Landfall — Whenever a land you control enters, draw a card.",
            "Whenever a land you control enters, you gain 1 life.",
            "Landfall — Whenever a land enters, you gain 1 life.",
            "Landfall — Whenever a land you control enters, you gain 1 life and draw a card.",
            "Landfall — Whenever a land you control enters, you gain 1 life. Scry 1.",
            "Landfall — Whenever a land you control enters, each opponent loses 1 life."
        ),
    },
    Recipe {
        id: RecipeId("triggered.other_creature_enters.pump_self_plus_one_plus_one"),
        label: "another creature enters and pumps the source plus one plus one",
        surface: RecipeSurface::TriggeredAbility,
        matcher: match_other_creature_enters_pump_self_plus_one_plus_one,
        calibration: calibrations!(
            "Loporrit Scout" => "Whenever another creature you control enters, this creature gets +1/+1 until end of turn.",
            "Griffin Protector" => "Whenever another creature you control enters, this creature gets +1/+1 until end of turn.",
            "Kinsbaile Aspirant" => "Whenever another creature you control enters, this creature gets +1/+1 until end of turn.";
            "Whenever a creature you control enters, this creature gets +1/+1 until end of turn.",
            "Whenever another creature you control enters, this creature gets +2/+2 until end of turn.",
            "Whenever another creature enters, this creature gets +1/+1 until end of turn.",
            "Whenever another creature you control enters, put a +1/+1 counter on this creature.",
            "Whenever another creature you control enters, this creature gets +1/+1.",
            "Alliance — Whenever another creature you control enters, this creature gets +1/+1 until end of turn.",
            "Whenever another creature you control enters, this creature gets +1/+1 until end of turn. Draw a card.",
            "Whenever another creature an opponent controls enters, this creature gets +1/+1 until end of turn."
        ),
    },
    Recipe {
        id: RecipeId("activated.tap.add_two_mana_any_one_color"),
        label: "tap for two mana of any one color",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_tap_for_two_mana_any_one_color,
        calibration: calibrations!(
            "Transdimensional Bovine" => "{T}: Add two mana of any one color.",
            "Khalni Gem" => "{T}: Add two mana of any one color.",
            "Zaxara, the Exemplary" => "{T}: Add two mana of any one color.";
            "{1}, {T}: Add two mana of any one color.",
            "{T}, Sacrifice this artifact: Add two mana of any one color.",
            "{T}: Add two mana of any color.",
            "{T}: Add {G}{G}.",
            "{T}: Add two mana of any one color. Spend this mana only to cast artifact spells.",
            "{T}: Add two mana in any combination of colors.",
            "{T}, Pay 1 life: Add two mana of any one color.",
            "{T}: Add two mana of any one color. Activate only once each turn."
        ),
    },
    Recipe {
        id: RecipeId("activated.tap.add_three_mana_any_one_color"),
        label: "tap for three mana of any one color",
        surface: RecipeSurface::ActivatedAbility,
        matcher: match_tap_for_three_mana_any_one_color,
        calibration: calibrations!(
            "Gilded Lotus" => "{T}: Add three mana of any one color.",
            "Lotus Field" => "{T}: Add three mana of any one color.",
            "Coveted Jewel" => "{T}: Add three mana of any one color.";
            "{T}, Sacrifice this artifact: Add three mana of any one color.",
            "{T}: Add three mana of any color.",
            "{T}: Add three mana in any combination of colors.",
            "{T}: Add {C}{C}{C}.",
            "{T}: Add three mana of any one color. Spend this mana only to cast artifact spells.",
            "{2}, {T}: Add three mana of any one color.",
            "{T}: Add four mana of any one color.",
            "{T}: Add three mana of any one color. Draw a card."
        ),
    },
    Recipe {
        id: RecipeId("static.self.count_scaled.artifact.plus_one_zero"),
        label: "this creature gets plus one plus zero per artifact its controller controls",
        surface: RecipeSurface::StaticAbility,
        matcher: match_static_self_count_scaled_artifact_plus_one_zero,
        calibration: calibrations!(
            "Guidelight Synergist" => "This creature gets +1/+0 for each artifact you control.",
            "Nim Lasher" => "This creature gets +1/+0 for each artifact you control.",
            "Storm-Kiln Artist" => "This creature gets +1/+0 for each artifact you control.";
            "This creature gets +1/+1 for each artifact you control.",
            "This creature gets +1/+0 for each creature you control.",
            "This creature gets +2/+0 for each artifact you control.",
            "Equipped creature gets +1/+0 for each artifact you control.",
            "This creature gets +1/+0 for each artifact an opponent controls.",
            "This creature gets +1/+0 for each artifact you control as long as you control a Robot.",
            "This creature gets +0/+1 for each artifact you control.",
            "This creature gets +1/+0 for each artifact you control. It can't block."
        ),
    },
];

fn surface_applies(surface: RecipeSurface, is_spell: bool, context: &RecipeContext) -> bool {
    match surface {
        RecipeSurface::KeywordClause => true,
        RecipeSurface::SpellClause => is_spell,
        RecipeSurface::AuraSpellClause => context.source_is_aura,
        RecipeSurface::ModalAssembly
        | RecipeSurface::ModalMode
        | RecipeSurface::StationAssembly => false,
        RecipeSurface::ZoneActivatedAbility
        | RecipeSurface::SpellStaticAbility
        | RecipeSurface::CharacteristicAbility => true,
        // Warp and Flashback print on permanent or instant/sorcery faces respectively; each matcher
        // performs its own face-type gate so a wrong-type line always fails closed.
        RecipeSurface::CastMethodClause => true,
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

pub(super) fn match_station_assembly(
    oracle_text: &str,
    context: &RecipeContext,
) -> Result<Option<RecipeMatch>, RecipeAmbiguity> {
    match_surface_in(
        CATALOG,
        oracle_text,
        RecipeSurface::StationAssembly,
        context,
    )
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
            oracle_id: None,
            source_is_permanent: true,
            source_is_artifact: true,
            source_is_spacecraft_or_planet: true,
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
                RecipeSurface::ModalAssembly
                | RecipeSurface::ModalMode
                | RecipeSurface::StationAssembly => {
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
                RecipeSurface::ModalAssembly
                | RecipeSurface::ModalMode
                | RecipeSurface::StationAssembly => {
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
            oracle_id: None,
            source_is_permanent: true,
            source_is_artifact: true,
            source_is_spacecraft_or_planet: true,
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
    fn issue_301_matches_only_the_reviewed_beginning_of_combat_graveyard_clause() {
        const CLAUSE: &str =
            "At the beginning of combat on your turn, exile up to one target card from a graveyard.";
        const RECIPE_ID: &str =
            "triggered.beginning_of_combat.controller.exile_graveyard_card.optional_one";
        for oracle_id in ISSUE_301_REVIEWED_ORACLE_IDS {
            let mut reviewed = context();
            reviewed.oracle_id = Some((*oracle_id).into());
            let matched = match_clause(CLAUSE, false, &reviewed)
                .expect("reviewed clause should not be ambiguous")
                .expect("reviewed clause should match");
            assert_eq!(matched.id.as_str(), RECIPE_ID);
        }

        for oracle_id in [
            "00000000-0000-0000-0000-000000000000",
            "",
            "unreviewed-identical-clause",
        ] {
            let mut unreviewed = context();
            unreviewed.oracle_id = Some(oracle_id.into());
            assert!(
                match_clause(CLAUSE, false, &unreviewed)
                    .expect("unreviewed clause should not be ambiguous")
                    .is_none(),
                "identical text must remain unsupported for an unreviewed Oracle identity: {oracle_id:?}"
            );
        }

        let mut reviewed = context();
        reviewed.oracle_id = Some(ISSUE_301_REVIEWED_ORACLE_IDS[0].into());
        for negative in [
            "At the beginning of your upkeep, exile up to one target card from a graveyard.",
            "At the beginning of your end step, exile up to one target card from a graveyard.",
            "At the beginning of combat on your turn, exile up to one target card from a graveyard. It attacks this combat if able.",
            "At the beginning of each combat, exile up to one target card from a graveyard.",
            "At the beginning of combat on an opponent's turn, exile up to one target card from a graveyard.",
            "At the beginning of combat on your turn, exile one target card from a graveyard.",
            "At the beginning of combat on your turn, exile up to two target cards from a graveyard.",
            "At the beginning of combat on your turn, exile a card from a graveyard.",
            "At the beginning of combat on your turn, choose a card from a graveyard, then exile it.",
            "At the beginning of combat on your turn, exile up to one target card from your graveyard.",
            "At the beginning of combat on your turn, exile up to one target card from an opponent's graveyard.",
            "At the beginning of combat on your turn, exile up to one target creature card from a graveyard.",
            "At the beginning of combat on your turn, exile up to one target card from your hand.",
            "At the beginning of combat on your turn, exile up to one target card from a graveyard to your hand.",
            "At the beginning of combat on your turn, exile up to one target card from a graveyard. If you do, draw a card.",
            "At the beginning of combat on your turn, if you control a creature, exile up to one target card from a graveyard.",
            "Whenever this creature attacks, exile up to one target card from a graveyard.",
            "At the beginning of combat on any turn, exile up to one target card from a graveyard.",
            "At the beginning of combat on each player's turn, exile up to one target card from a graveyard.",
            "At the beginning of combat on your turn, exile up to one target nonland card from a graveyard.",
            "At the beginning of combat on your turn, exile up to one target card from a library.",
            "At the beginning of combat on your turn, exile up to one target card from the battlefield.",
            "At the beginning of combat on your turn, exile up to one target card from exile.",
            "At the beginning of combat on your turn, return up to one target card from a graveyard to your hand.",
            "At the beginning of combat on your turn, put up to one target card from a graveyard on top of its owner's library.",
            "At the beginning of combat on your turn, put up to one target card from a graveyard onto the battlefield.",
            "At the beginning of combat on your turn, you may exile target card from a graveyard.",
            "At the beginning of combat on your turn, exile target card from a graveyard. You may exile another target card from a graveyard.",
        ] {
            assert!(
                match_clause(negative, false, &reviewed)
                    .expect("near-miss should not be ambiguous")
                    .is_none(),
                "near-miss was accepted: {negative}"
            );
        }

        let mut noncreature = context();
        noncreature.oracle_id = Some(ISSUE_301_REVIEWED_ORACLE_IDS[0].into());
        noncreature.source_is_creature = false;
        assert!(
            match_clause(CLAUSE, false, &noncreature)
                .expect("source-kind check should not be ambiguous")
                .is_none(),
            "the exact clause must remain bound to creature sources"
        );
        assert!(
            match_clause(CLAUSE, true, &context())
                .expect("surface check should not be ambiguous")
                .is_none(),
            "the exact clause must remain bound to triggered abilities"
        );
    }

    #[test]
    fn issue_288_target_opponent_exiles_one_hand_card_is_exact_and_typed() {
        let clause = "When this creature enters, target opponent exiles a card from their hand.";
        let matched = match_clause(clause, false, &context())
            .expect("issue #288 recipe matching should not be ambiguous")
            .expect("issue #288 ETB exile should match");
        assert_eq!(matched.id.as_str(), "etb.exile.target_opponent_hand.one");

        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #288 must emit a triggered ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(!ability.may);
        assert!(ability.modal.is_none());
        assert!(ability.intervening_if.is_none());
        assert_eq!(
            ability.effect,
            [SpellEffectKind::ChooseHandCards {
                action: HandCardAction::Exile,
                count: 1,
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    ..TargetFilter::default()
                },
                chooser: tricerules_cards::primitives::HandCardChooser::AffectedPlayer,
                card_filter: None,
                optional: false,
                visibility: tricerules_cards::primitives::HandChoiceVisibility::PrivateLook,
            }]
        );
        let targeting = ability.targeting.as_ref().expect("opponent target group");
        let [group] = targeting.groups.as_slice() else {
            panic!("issue #288 must have exactly one target group");
        };
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.prompt, "Choose target opponent");
        assert_eq!(group.effect_indices, [0]);
        assert!(group.distinct_from.is_empty());

        for oracle_id in ISSUE_288_REVIEWED_ORACLE_IDS {
            let mut reviewed = context();
            reviewed.oracle_id = Some((*oracle_id).into());
            assert!(
                match_clause(clause, false, &reviewed)
                    .expect("reviewed Oracle ID must not be ambiguous")
                    .is_some(),
                "reviewed Oracle ID must match: {oracle_id}"
            );
        }
        let mut unreviewed = context();
        unreviewed.oracle_id = Some("00000000-0000-0000-0000-000000000000".into());
        assert!(
            match_clause(clause, false, &unreviewed)
                .expect("unreviewed Oracle ID must not be ambiguous")
                .is_none(),
            "an unreviewed Oracle ID must fail closed"
        );

        for near_miss in [
            "When this creature enters, target opponent exiles two cards from their hand.",
            "When this creature enters, each opponent exiles a card from their hand.",
            "When this creature enters, target opponent exiles a card from their hand at random.",
            "When this creature enters, target opponent may exile a card from their hand.",
            "When this creature enters, target opponent reveals a card from their hand.",
            "When this creature enters, target opponent exiles a card from their hand. Draw a card.",
        ] {
            assert!(
                match_etb_target_opponent_exiles_one(near_miss, &context()).is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }
        assert!(
            match_etb_target_opponent_exiles_one(
                "When this creature enters, target opponent discards a card.",
                &context(),
            )
            .is_none(),
            "discard wording must not match the exile recipe"
        );

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(clause, false, &noncreature)
                .expect("source-kind check must not be ambiguous")
                .is_none(),
            "the creature ETB recipe must reject noncreature sources"
        );
        assert!(
            match_clause(clause, true, &context())
                .expect("surface check must not be ambiguous")
                .is_none(),
            "the ETB recipe must reject spell clauses"
        );
    }

    #[test]
    fn issue_290_optional_top_one_library_partition_is_exact_and_typed() {
        let clause = "When this creature enters, look at the top three cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.";
        let matched = match_clause(clause, false, &context())
            .expect("issue #290 recipe matching should not be ambiguous")
            .expect("issue #290 ETB library partition should match");
        assert_eq!(matched.id.as_str(), "etb.look.top_three.optional_top_one");

        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #290 must emit a triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(!ability.may);
        assert!(ability.modal.is_none());
        assert!(ability.targeting.is_none());
        assert!(ability.intervening_if.is_none());
        assert_eq!(
            ability.effect,
            [SpellEffectKind::LibraryPartition {
                count: 3,
                top_min: 0,
                top_max: Some(1),
                kind: LibraryPartitionKind::Look,
            }]
        );

        for oracle_id in [
            "656fc672-efa5-484a-b5e8-eac262331439",
            "fde50a0d-9bc6-45f4-873e-3de81a513fac",
        ] {
            let mut reviewed = context();
            reviewed.oracle_id = Some(oracle_id.into());
            assert!(
                match_clause(clause, false, &reviewed)
                    .expect("reviewed Oracle ID must not be ambiguous")
                    .is_some(),
                "reviewed Oracle ID must match: {oracle_id}"
            );
        }
        for oracle_id in ["00000000-0000-0000-0000-000000000000", ""] {
            let mut unreviewed = context();
            unreviewed.oracle_id = Some(oracle_id.into());
            assert!(
                match_clause(clause, false, &unreviewed)
                    .expect("unreviewed Oracle ID must not be ambiguous")
                    .is_none(),
                "unreviewed Oracle ID must fail closed: {oracle_id:?}"
            );
        }

        for near_miss in [
            "When this creature enters, look at the top three cards of your library. Put one of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top three cards of your library. You may put two of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top two cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top four cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top three cards of your library. You may put one of those cards on the bottom of your library. Put the rest into your graveyard.",
            "When this creature enters, look at the top three cards of your library. You may put one of those cards into exile. Put the rest into your graveyard.",
            "When this creature enters, look at the top three cards of your library. You may put one of those cards into your hand. Put the rest into your graveyard.",
            "When this creature enters, reveal the top three cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard.",
            "When this creature enters, surveil 3.",
            "When this creature enters, look at the top three cards of your library. You may put one of those cards back on top of your library. Put the rest into your graveyard. Draw a card.",
        ] {
            assert!(
                match_clause(near_miss, false, &context())
                    .expect("issue #290 near-miss matching should not be ambiguous")
                    .is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(clause, false, &noncreature)
                .expect("source-kind check must not be ambiguous")
                .is_none(),
            "the creature ETB recipe must reject noncreature sources"
        );
        assert!(
            match_clause(clause, true, &context())
                .expect("surface check must not be ambiguous")
                .is_none(),
            "the ETB recipe must reject spell clauses"
        );
    }

    #[test]
    fn issue_291_mutagen_etb_recipe_is_exact_and_allowlisted() {
        let clause = r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#;
        let mut reviewed = context();
        reviewed.oracle_id = Some("b24a87af-407f-4c58-80b4-caab9c65a233".into());
        let matched = match_clause(clause, false, &reviewed)
            .expect("issue #291 recipe matching should not be ambiguous")
            .expect("reviewed Crustacean Commando should match");
        assert_eq!(matched.id.as_str(), "etb.create_token.mutagen.one");

        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #291 must emit a triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::CreateTokens {
                token: "mutagen".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }]
        );
        assert!(ability.targeting.is_none());
        assert!(!ability.may);
        assert!(ability.modal.is_none());
        assert!(ability.intervening_if.is_none());

        for oracle_id in [
            "b24a87af-407f-4c58-80b4-caab9c65a233",
            "f570bac8-9987-4963-af02-476d18abc847",
        ] {
            let mut allowlisted = context();
            allowlisted.oracle_id = Some(oracle_id.into());
            assert!(
                match_clause(clause, false, &allowlisted)
                    .expect("allowlisted Oracle ID must not be ambiguous")
                    .is_some(),
                "allowlisted Oracle ID must match: {oracle_id}"
            );
        }
        for oracle_id in ["00000000-0000-0000-0000-000000000000", ""] {
            let mut unreviewed = context();
            unreviewed.oracle_id = Some(oracle_id.into());
            assert!(
                match_clause(clause, false, &unreviewed)
                    .expect("unreviewed Oracle ID must not be ambiguous")
                    .is_none(),
                "unreviewed Oracle ID must fail closed: {oracle_id:?}"
            );
        }

        for near_miss in [
            r#"When this creature enters, create a Mutagen token."#,
            r#"When this creature enters, create two Mutagen tokens. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a tapped Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{2}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put two +1/+1 counters on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature you control. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as an instant.")"#,
            r#"When this creature dies, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"Whenever this creature attacks, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.")"#,
            r#"When this creature enters, create a Mutagen token. (It's an artifact with "{1}, {T}, Sacrifice this token: Put a +1/+1 counter on target creature. Activate only as a sorcery.") Draw a card."#,
        ] {
            assert!(
                match_clause(near_miss, false, &reviewed)
                    .expect("issue #291 near-miss matching should not be ambiguous")
                    .is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }

        let mut noncreature = reviewed.clone();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(clause, false, &noncreature)
                .expect("source-kind check must not be ambiguous")
                .is_none(),
            "the creature ETB recipe must reject noncreature sources"
        );
        assert!(
            match_clause(clause, true, &reviewed)
                .expect("surface check must not be ambiguous")
                .is_none(),
            "the ETB recipe must reject spell clauses"
        );
    }

    #[test]
    fn issue_297_food_on_leave_recipe_is_exact_and_allowlisted() {
        let clause = r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#;
        let mut city_pigeon = context();
        city_pigeon.oracle_id = Some("ac8cae63-270c-4f73-b29b-50f8f2395fd9".into());
        let matched = match_clause(clause, false, &city_pigeon)
            .expect("issue #297 recipe matching should not be ambiguous")
            .expect("allowlisted City Pigeon should match");
        assert_eq!(
            matched.id.as_str(),
            "triggered.self_leaves_battlefield.create_token.food.one"
        );

        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #297 must emit a triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfLeavesBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::CreateTokens {
                token: "food".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }]
        );
        assert!(ability.targeting.is_none());
        assert!(!ability.may);
        assert!(ability.modal.is_none());
        assert!(ability.intervening_if.is_none());

        let mut featherbrained_filcher = context();
        featherbrained_filcher.oracle_id = Some("79a9fc1c-a6a9-483a-9ba7-d09fe41760c3".into());
        assert!(
            match_clause(clause, false, &featherbrained_filcher)
                .expect("calibration ID must not be ambiguous")
                .is_none(),
            "Featherbrained Filcher must remain calibration-only"
        );

        let recipe = CATALOG
            .iter()
            .find(|recipe| {
                recipe.id.as_str() == "triggered.self_leaves_battlefield.create_token.food.one"
            })
            .expect("issue #297 recipe catalog entry");
        let featherbrained_calibration = recipe
            .calibration
            .positive_cards
            .iter()
            .find(|card| card.name == "Featherbrained Filcher")
            .expect("Featherbrained Filcher calibration");
        assert!(
            match_clause(featherbrained_calibration.clause, false, &context())
                .expect("calibration context must not be ambiguous")
                .is_some(),
            "the catalog calibration must still prove the exact template"
        );

        for oracle_id in [
            "00000000-0000-0000-0000-000000000000",
            "79a9fc1c-a6a9-483a-9ba7-d09fe41760c3-unrelated",
            "",
        ] {
            let mut unreviewed = context();
            unreviewed.oracle_id = Some(oracle_id.into());
            assert!(
                match_clause(clause, false, &unreviewed)
                    .expect("unreviewed Oracle ID must not be ambiguous")
                    .is_none(),
                "unreviewed Oracle ID must fail closed: {oracle_id:?}"
            );
        }

        for near_miss in [
            r#"When this creature dies, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create two Food tokens. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a tapped Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{3}, {T}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 2 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.") Draw a card."#,
            r#"When another creature leaves the battlefield, create a Food token. (It's an artifact with "{2}, {T}, Sacrifice this token: You gain 3 life.")"#,
            r#"When this creature leaves the battlefield, create a Food token."#,
        ] {
            assert!(
                match_clause(near_miss, false, &city_pigeon)
                    .expect("issue #297 near-miss matching should not be ambiguous")
                    .is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }

        let mut noncreature = city_pigeon.clone();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(clause, false, &noncreature)
                .expect("source-kind check must not be ambiguous")
                .is_none(),
            "the creature leave recipe must reject noncreature sources"
        );
        assert!(
            match_clause(clause, true, &city_pigeon)
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the creature leave recipe must reject spell clauses"
        );
    }

    #[test]
    fn issue_299_dies_create_mercenary_recipe_is_exact_and_allowlisted() {
        let clause = r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##;
        let reviewed_ids = [
            "5d24fb6b-7176-407c-b917-6c88a6da40df",
            "c1348afe-4dc7-41bb-9d3f-1e8751abd7db",
        ];
        for oracle_id in reviewed_ids {
            let mut reviewed = context();
            reviewed.oracle_id = Some(oracle_id.into());
            let matched = match_clause(clause, false, &reviewed)
                .expect("issue #299 recipe matching should not be ambiguous")
                .expect("reviewed dies-to-Mercenary card should match");
            assert_eq!(matched.id.as_str(), "dies.create_token.mercenary.one");
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("issue #299 must emit a triggered ability");
            };
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfDies);
            assert!(!ability.may);
            assert!(ability.modal.is_none());
            assert!(ability.targeting.is_none());
            assert!(ability.intervening_if.is_none());
            assert_eq!(
                ability.effect,
                [SpellEffectKind::CreateTokens {
                    token: "mercenary_r_1_1".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }]
            );
        }

        for oracle_id in [
            "00000000-0000-0000-0000-000000000000",
            "",
            "unreviewed-identical-text",
        ] {
            let mut unreviewed = context();
            unreviewed.oracle_id = Some(oracle_id.into());
            assert!(
                match_clause(clause, false, &unreviewed)
                    .expect("unreviewed recipe matching should not be ambiguous")
                    .is_none(),
                "unreviewed Oracle ID must fail closed: {oracle_id:?}"
            );
        }

        for near_miss in [
            r##"When this creature leaves the battlefield, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When another creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, you may create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create two 1/1 red Mercenary creature tokens with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a tapped 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as an instant.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +2/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+1 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn.""##,
            r##"When this creature dies, create an attacking 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token. At the beginning of the next end step, sacrifice it."##,
            r##"When this creature dies, create a 1/1 blue Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 2/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Soldier creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{1}, {T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery.""##,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of next turn. Activate only as a sorcery.""##,
            r#"When this creature dies, create a 1/1 red Mercenary creature token."#,
            r##"When this creature dies, create a 1/1 red Mercenary creature token with "{T}: Target creature you control gets +1/+0 until end of turn. Activate only as a sorcery."" Draw a card."##,
        ] {
            let mut reviewed = context();
            reviewed.oracle_id = Some(reviewed_ids[0].into());
            assert!(
                match_clause(near_miss, false, &reviewed)
                    .expect("issue #299 near-miss matching should not be ambiguous")
                    .is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }

        let mut noncreature = context();
        noncreature.oracle_id = Some(reviewed_ids[0].into());
        noncreature.source_is_creature = false;
        assert!(
            match_clause(clause, false, &noncreature)
                .expect("source-kind check should not be ambiguous")
                .is_none(),
            "the exact clause must remain bound to creature sources"
        );
        assert!(
            match_clause(clause, true, &context())
                .expect("surface check should not be ambiguous")
                .is_none(),
            "the exact clause must remain bound to triggered abilities"
        );
    }

    #[test]
    fn issue_300_land_sacrifice_draw_recipe_is_exact_and_allowlisted() {
        let clause = "{2}{R}, Sacrifice a land: Draw a card.";
        let reviewed_ids = [
            "ce000c0b-db42-4569-855f-f4eae0431c09",
            "3b66d2c2-be7a-4296-9888-f0cfb2975e89",
        ];
        for oracle_id in reviewed_ids {
            let mut reviewed = context();
            reviewed.oracle_id = Some(oracle_id.into());
            let matched = match_clause(clause, false, &reviewed)
                .expect("issue #300 recipe matching should not be ambiguous")
                .expect("reviewed land-sacrifice draw card should match");
            assert_eq!(matched.id, RecipeId("activated.land.sacrifice.draw_one"));
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("issue #300 must emit an activated ability");
            };
            assert_eq!(ability.ability_id.as_str(), "activated_01");
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert_eq!(
                ability.costs,
                [
                    AbilityCost::Mana(ManaCost::parse("{2}{R}").unwrap()),
                    AbilityCost::SacrificePermanent {
                        filter: TargetFilter {
                            kind: TargetKind::AnyPermanent,
                            controller: TargetController::You,
                            permanent_types: vec![PermanentTypeFilter::Land],
                            ..TargetFilter::default()
                        }
                    }
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
            assert!(ability.conditions.is_empty());
            assert!(ability.activation_limit.is_none());
        }

        for oracle_id in [
            "00000000-0000-0000-0000-000000000000",
            "",
            "unreviewed-identical-text",
        ] {
            let mut unreviewed = context();
            unreviewed.oracle_id = Some(oracle_id.into());
            assert!(
                match_clause(clause, false, &unreviewed)
                    .expect("unreviewed recipe matching should not be ambiguous")
                    .is_none(),
                "unreviewed Oracle ID must fail closed: {oracle_id:?}"
            );
        }

        for near_miss in [
            "{3}{R}, Sacrifice a land: Draw a card.",
            "{2}{R}, {T}, Sacrifice a land: Draw a card.",
            "{2}{R}, Discard a card, Sacrifice a land: Draw a card.",
            "{2}{R}, Sacrifice this creature: Draw a card.",
            "{2}{R}, Sacrifice a creature: Draw a card.",
            "{2}{R}, Sacrifice an artifact: Draw a card.",
            "{2}{R}, Sacrifice a permanent: Draw a card.",
            "{2}{R}, Sacrifice a Mountain: Draw a card.",
            "{2}{R}, Sacrifice two lands: Draw a card.",
            "{2}{R}, Sacrifice a land an opponent controls: Draw a card.",
            "{2}{R}, Sacrifice target land: Draw a card.",
            "{2}{R}, You may sacrifice a land: Draw a card.",
            "{2}{R}, Sacrifice a land: You may draw a card.",
            "{2}{R}, Sacrifice a land: Draw two cards.",
            "{2}{R}, Sacrifice a land: Draw a card, then discard a card.",
            "{2}{R}, Sacrifice a land: Draw a card. Activate only as a sorcery.",
            "Sacrifice a land, {2}{R}: Draw a card.",
            "{2}{R}, Sacrifice a land: Draw a card. You gain 1 life.",
        ] {
            let mut reviewed = context();
            reviewed.oracle_id = Some(reviewed_ids[0].into());
            assert!(
                match_clause(near_miss, false, &reviewed)
                    .expect("issue #300 near-miss matching should not be ambiguous")
                    .is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }

        let mut noncreature = context();
        noncreature.oracle_id = Some(reviewed_ids[0].into());
        noncreature.source_is_creature = false;
        assert!(
            match_clause(clause, false, &noncreature)
                .expect("source-kind check should not be ambiguous")
                .is_none(),
            "the exact clause must remain bound to creature sources"
        );
        assert!(
            match_clause(clause, true, &context())
                .expect("spell surface check should not be ambiguous")
                .is_none(),
            "the exact clause must remain bound to activated abilities"
        );
    }

    #[test]
    fn issue_283_graveyard_to_library_bottom_activation_is_supported() {
        let matched = match_clause(
            "{2}: Put target card from your graveyard on the bottom of your library.",
            false,
            &context(),
        )
        .expect("issue #283 recipe matching should not be ambiguous")
        .expect("issue #283 activation should match");
        assert_eq!(
            matched.id.as_str(),
            "activated.graveyard_card_to_library_bottom"
        );
        let recipe = CATALOG
            .iter()
            .find(|recipe| recipe.id == matched.id)
            .expect("issue #283 recipe should remain registered");
        assert_eq!(recipe.surface, RecipeSurface::ActivatedAbility);
    }

    #[test]
    fn issue_283_emits_targeted_controller_bottom_move_and_rejects_near_misses() {
        let clause = "{2}: Put target card from your graveyard on the bottom of your library.";
        let matched = match_clause(clause, false, &context())
            .expect("issue #283 recipe matching should not be ambiguous")
            .expect("issue #283 activation should match");
        let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
            panic!("issue #283 must emit an activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(
            ability.costs,
            vec![AbilityCost::Mana(ManaCost::parse("{2}").unwrap())]
        );
        assert_eq!(
            ability.effect,
            vec![SpellEffectKind::MoveGraveyardCards {
                filter: GraveyardFilter {
                    owner: GraveyardOwner::Controller,
                    ..GraveyardFilter::default()
                },
                destination: GraveyardDestination::LibraryBottom,
                linked_exile_id: None,
            }]
        );
        let targeting = ability
            .targeting
            .as_ref()
            .expect("issue #283 requires explicit targeting");
        assert_eq!(targeting.groups.len(), 1);
        assert_eq!(targeting.groups[0].min, 1);
        assert_eq!(targeting.groups[0].max, 1);
        assert_eq!(
            targeting.groups[0].prompt,
            "Choose target card from your graveyard"
        );
        assert_eq!(targeting.groups[0].effect_indices, vec![0]);

        for near_miss in [
            "{2}: Put target card from a graveyard on the bottom of your library.",
            "{2}: Put target card from an opponent's graveyard on the bottom of your library.",
            "{2}: Put target creature card from your graveyard on the bottom of your library.",
            "{2}: Put target noncreature card from your graveyard on the bottom of your library.",
            "{2}: Put up to one target card from your graveyard on the bottom of your library.",
            "{2}: Choose a card from your graveyard and put it on the bottom of your library.",
            "{2}: You may put target card from your graveyard on the bottom of your library.",
            "{2}: Put target card from your graveyard on the top of your library.",
            "{2}: Put target card from your graveyard into your hand.",
            "{2}: Exile target card from your graveyard.",
            "{2}: Put two target cards from your graveyard on the bottom of your library.",
            "{2}, {T}: Put target card from your graveyard on the bottom of your library.",
            "{2}, Sacrifice this creature: Put target card from your graveyard on the bottom of your library.",
            "{2}{G}: Put target card from your graveyard on the bottom of your library.",
            "{2}: Put target card from your graveyard on the bottom of your library. Then draw a card.",
            "Put target card from your graveyard on the bottom of your library with {2}:.",
            "{2}: Put target card from your graveyard on the bottom of your library. Put another card there.",
            "{2}: Put target card on the bottom of your library from your graveyard.",
        ] {
            assert!(
                match_clause(near_miss, false, &context()).unwrap().is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }

        let mut nonpermanent = context();
        nonpermanent.source_is_permanent = false;
        assert!(
            match_clause(clause, false, &nonpermanent)
                .unwrap()
                .is_none(),
            "the permanent-source recipe must reject nonpermanent sources"
        );
        assert!(
            match_clause(&format!("{clause}\nReach"), false, &context())
                .unwrap()
                .is_none(),
            "the exact recipe must reject a reordered/appended full-card clause"
        );
    }

    #[test]
    fn issue_284_optional_basic_land_to_top_etb_is_exact_and_typed() {
        let clause = "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.";
        let matched = match_clause(clause, false, &context())
            .expect("issue #284 recipe matching should not be ambiguous")
            .expect("issue #284 ETB search should match");
        assert_eq!(
            matched.id.as_str(),
            "triggered.self_enters.optional_search_basic_land_top"
        );
        let recipe = CATALOG
            .iter()
            .find(|recipe| recipe.id == matched.id)
            .expect("issue #284 recipe should remain registered");
        assert_eq!(recipe.surface, RecipeSurface::TriggeredAbility);

        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #284 must emit a triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(!ability.may, "optional search is represented by its branch");
        assert!(ability.targeting.is_none());
        let [SpellEffectKind::ChooseResolutionBranch {
            chooser,
            optional,
            selection,
            branches,
            otherwise,
        }] = ability.effect.as_slice()
        else {
            panic!("issue #284 must emit one optional resolution branch");
        };
        assert_eq!(*chooser, PlayerRecipient::Controller);
        assert!(*optional);
        assert_eq!(
            *selection,
            tricerules_cards::primitives::ResolutionBranchSelection::PlayerChoice
        );
        assert!(otherwise.is_empty());
        let [branch] = branches.as_slice() else {
            panic!("issue #284 must emit one search branch");
        };
        assert_eq!(branch.branch_id.as_str(), "search_for_a_basic_land");
        assert_eq!(branch.presentation, AbilityPresentation::Fallback);
        assert_eq!(
            branch.cost,
            tricerules_cards::primitives::ResolutionCost::None
        );
        assert_eq!(
            branch.requirement,
            tricerules_cards::primitives::ResolutionBranchRequirement::Always
        );
        let [SpellEffectKind::SearchLibrary {
            who,
            optional: search_optional,
            count,
            count_by_cast_cost,
            filter: Some(filter),
            slots,
            zones,
            destination,
            conditional_destination,
            shuffle,
            reveal,
            result_id,
        }] = branch.effects.as_slice()
        else {
            panic!("issue #284 branch must search the library");
        };
        assert_eq!(*who, PlayerRecipient::Controller);
        assert!(!search_optional);
        assert_eq!(*count, 1);
        assert!(count_by_cast_cost.is_none());
        assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand));
        assert!(filter.any_of.is_none());
        assert!(filter.required_subtypes.is_empty());
        assert!(slots.is_empty());
        assert_eq!(
            zones,
            &tricerules_cards::primitives::SearchZoneSelection::default()
        );
        assert_eq!(*destination, SearchDestination::TopOfLibrary);
        assert!(conditional_destination.is_none());
        assert!(*shuffle);
        assert!(*reveal);
        assert!(result_id.is_none());

        for near_miss in [
            "When this creature enters, search your library for a basic land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for up to one basic land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a nonbasic land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a creature card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card into your hand.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card onto the battlefield.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on the bottom.",
            "When this creature enters, you may search your library for a basic land card, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then put that card on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put up to one card on top.",
            "When this creature enters, you may search your library for a basic land card, reveal them, then shuffle and put those cards on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top of an opponent's library.",
            "When this creature enters, target opponent may search their library for a basic land card, reveal it, then shuffle and put that card on top.",
            "When this creature enters, you may search your library for two basic land cards, reveal them, then shuffle and put those cards on top.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top. It gains haste.",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top.\nReach",
            "When this creature enters, you may search your library for a basic land card, reveal it, then shuffle and put that card on top, then draw a card.",
        ] {
            assert!(
                match_clause(near_miss, false, &context()).unwrap().is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(clause, false, &noncreature).unwrap().is_none(),
            "the creature-source recipe must reject noncreature sources"
        );
    }

    #[test]
    fn issue_285_self_attacks_pump_and_indestructible_is_exact_and_typed() {
        let clause = "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn.";
        let matched = match_clause(clause, false, &context())
            .expect("issue #285 recipe matching should not be ambiguous")
            .expect("issue #285 attack pump should match");
        assert_eq!(
            matched.id.as_str(),
            "triggered.self_attacks.pump_other_creature_you_control.plus_one_indestructible"
        );
        let recipe = CATALOG
            .iter()
            .find(|recipe| recipe.id == matched.id)
            .expect("issue #285 recipe should remain registered");
        assert_eq!(recipe.surface, RecipeSurface::TriggeredAbility);

        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #285 must emit a triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0,
            }
        );
        assert!(!ability.may);
        assert!(ability.modal.is_none());
        assert!(ability.intervening_if.is_none());
        let target = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            excluded_objects: vec![TargetObjectExclusion::Source],
            ..TargetFilter::default()
        };
        assert_eq!(
            ability.effect,
            [
                SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 0,
                    scale: None,
                    subject: EffectSubject::Chosen(Box::new(target.clone())),
                },
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(Box::new(target.clone())),
                    keywords: vec![Keyword::Indestructible],
                },
            ]
        );
        let targeting = ability
            .targeting
            .as_ref()
            .expect("issue #285 requires explicit target group");
        assert_eq!(targeting.groups.len(), 1);
        let group = &targeting.groups[0];
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.prompt, "Choose another target creature you control");
        assert_eq!(group.effect_indices, [0, 1]);

        for near_miss in [
            "When this creature enters, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature blocks, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            "At the beginning of combat, another target creature you control gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, this creature gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature an opponent controls gets +1/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, you may have another target creature you control get +1/+0 and gain indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +2/+0 and gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+1 and gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains vigilance until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains hexproof until end of turn.",
            "Whenever this creature attacks, put a +1/+1 counter on another target creature you control and it gains indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn. Draw a card.",
            "Whenever this creature attacks, up to two target creatures you control each get +1/+0 and gain indestructible until end of turn.",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn.\nReach",
            "Whenever this creature attacks, another target creature you control gets +1/+0 and gains indestructible until end of turn, then draw a card.",
        ] {
            assert!(
                match_clause(near_miss, false, &context()).unwrap().is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(clause, false, &noncreature).unwrap().is_none(),
            "the creature-source recipe must reject noncreature sources"
        );
        assert!(
            match_clause(clause, true, &context()).unwrap().is_none(),
            "the triggered recipe must reject spell clauses"
        );
    }

    #[test]
    fn issue_286_equipment_attack_tap_defending_creature_is_exact_and_typed() {
        let clause =
            "Whenever equipped creature attacks, tap target creature defending player controls.";
        let matched = match_clause(clause, false, &context())
            .expect("issue #286 recipe matching should not be ambiguous")
            .expect("issue #286 Equipment attack tap should match");
        assert_eq!(
            matched.id.as_str(),
            "triggered.attached_object_attacks.tap.defending_creature"
        );
        let recipe = CATALOG
            .iter()
            .find(|recipe| recipe.id == matched.id)
            .expect("issue #286 recipe should remain registered");
        assert_eq!(recipe.surface, RecipeSurface::TriggeredAbility);

        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("issue #286 must emit a triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverAttachedObjectAttacks
        );
        assert!(!ability.may);
        assert!(ability.modal.is_none());
        assert!(ability.intervening_if.is_none());
        let target = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::DefendingPlayer,
            ..TargetFilter::default()
        };
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(Box::new(target.clone())),
            }]
        );
        let targeting = ability
            .targeting
            .as_ref()
            .expect("issue #286 requires explicit target group");
        assert_eq!(targeting.groups.len(), 1);
        let [group] = targeting.groups.as_slice() else {
            panic!("issue #286 must have one target group");
        };
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(
            group.prompt,
            "Choose target creature defending player controls"
        );
        assert_eq!(group.effect_indices, [0]);
        assert!(group.distinct_from.is_empty());

        for near_miss in [
            "Whenever this Equipment attacks, tap target creature defending player controls.",
            "Whenever this artifact attacks, tap target creature defending player controls.",
            "Whenever equipped creature attacks, tap target creature.",
            "Whenever equipped creature attacks, tap target creature any player controls.",
            "Whenever equipped creature attacks, tap target creature an opponent controls.",
            "Whenever equipped creature attacks, tap target creature attacking player controls.",
            "Whenever equipped creature attacks, tap target permanent defending player controls.",
            "Whenever equipped creature attacks, tap target artifact defending player controls.",
            "Whenever equipped creature attacks, tap target noncreature artifact defending player controls.",
            "Whenever equipped creature attacks, untap target creature defending player controls.",
            "Whenever equipped creature attacks, put a stun counter on target creature defending player controls.",
            "Whenever equipped creature attacks, you may tap target creature defending player controls.",
            "Whenever equipped creature attacks, tap up to one target creature defending player controls.",
            "At the beginning of combat, tap target creature defending player controls.",
            "Whenever equipped creature blocks, tap target creature defending player controls.",
            "Whenever equipped creature attacks, tap two target creatures defending player controls.",
            "Whenever equipped creature attacks, tap target creature defending player controls. Draw a card.",
            "Whenever equipped creature attacks, tap target creature defending player controls.\nReach",
            "Whenever equipped creature attacks, tap target creature defending player controls, then draw a card.",
        ] {
            assert!(
                match_clause(near_miss, false, &context()).unwrap().is_none(),
                "near-miss unexpectedly matched: {near_miss}"
            );
        }

        let mut non_equipment = context();
        non_equipment.source_is_equipment = false;
        assert!(
            match_clause(clause, false, &non_equipment)
                .unwrap()
                .is_none(),
            "the attachment trigger recipe must reject Aura and creature sources"
        );
        let mut aura = context();
        aura.source_is_equipment = false;
        aura.source_is_aura = true;
        assert!(
            match_clause(clause, false, &aura).unwrap().is_none(),
            "the attachment trigger recipe must reject Aura sources"
        );
        let mut creature = context();
        creature.source_is_equipment = false;
        creature.source_is_aura = false;
        creature.source_is_creature = true;
        assert!(
            match_clause(clause, false, &creature).unwrap().is_none(),
            "the attachment trigger recipe must reject creature sources"
        );
        assert!(
            match_clause(clause, true, &context()).unwrap().is_none(),
            "the triggered recipe must reject spell clauses"
        );
    }

    #[test]
    fn issue_287_self_enters_recruit_is_exact_and_typed() {
        const LONG_LAKE_NUISANCE_ID: &str = "a833fdf1-db0c-4846-8452-d3b2059c2355";
        const PATIENT_INSTRUCTOR_ID: &str = "bddd7e99-ec74-4ca6-9137-155b85695a95";
        const RECRUIT_CLAUSE: &str = r#"When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)"#;
        const RECIPE_ID: &str = "triggered.self_enters.recruit";

        for oracle_id in [LONG_LAKE_NUISANCE_ID, PATIENT_INSTRUCTOR_ID] {
            let mut reviewed = context();
            reviewed.oracle_id = Some(oracle_id.into());
            let matched = match_clause(RECRUIT_CLAUSE, false, &reviewed)
                .expect("issue #287 clause should not be ambiguous")
                .expect("issue #287 ETB recruit should match");
            assert_eq!(matched.id.as_str(), RECIPE_ID);
            let recipe = CATALOG
                .iter()
                .find(|recipe| recipe.id == matched.id)
                .expect("issue #287 recipe should remain registered");
            assert_eq!(recipe.surface, RecipeSurface::EtbAbility);

            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("issue #287 must emit a triggered ability");
            };
            assert_eq!(ability.ability_id.as_str(), "triggered_01");
            assert_eq!(
                ability.presentation,
                AbilityPresentation::OracleLines(vec![1])
            );
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(!ability.may);
            assert!(ability.modal.is_none());
            assert!(ability.targeting.is_none());
            assert!(ability.intervening_if.is_none());
            let [SpellEffectKind::DrawDiscard {
                who,
                draw_count,
                discard_count,
                order,
                optional,
            }, SpellEffectKind::ChooseResolutionBranch {
                chooser,
                optional: branch_optional,
                selection,
                branches,
                otherwise,
            }] = ability.effect.as_slice()
            else {
                panic!(
                    "issue #287 must emit a draw/discard continuation followed by the result-gated token"
                );
            };
            assert_eq!(*who, PlayerRecipient::Controller);
            assert_eq!((*draw_count, *discard_count), (1, 1));
            assert_eq!(*order, DrawDiscardOrder::DrawThenDiscard);
            assert!(!*optional);
            assert_eq!(*chooser, PlayerRecipient::Controller);
            assert!(!*branch_optional);
            assert_eq!(*selection, ResolutionBranchSelection::FirstApplicable);
            assert!(otherwise.is_empty());
            let [soldier, fallback] = branches.as_slice() else {
                panic!("issue #287 must emit exactly the token branch and its applicable fallback");
            };
            assert_eq!(soldier.branch_id.as_str(), "create_a_soldier");
            assert_eq!(soldier.presentation, AbilityPresentation::Fallback);
            assert_eq!(soldier.cost, ResolutionCost::None);
            assert_eq!(
                soldier.requirement,
                tricerules_cards::primitives::ResolutionBranchRequirement::CardResultCount {
                    filter: CardResultFilter {
                        source: CardResultSource::PreviousEffect,
                        action: CardResultAction::Discard,
                        players: RelativePlayerSet::Controller,
                        card_type: Some(CardTypeFilter::Nonland),
                    },
                    min: Some(1),
                    max: None,
                }
            );
            assert_eq!(
                soldier.effects,
                [SpellEffectKind::CreateTokens {
                    token: "human_soldier_w_1_1".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }]
            );
            assert_eq!(fallback.branch_id.as_str(), "no_soldier");
            assert_eq!(fallback.presentation, AbilityPresentation::Fallback);
            assert_eq!(
                fallback.requirement,
                tricerules_cards::primitives::ResolutionBranchRequirement::Always
            );
            assert!(fallback.effects.is_empty());
            assert_eq!(
                SpellEffectKind::validate_list(&ability.effect),
                Ok(()),
                "the engine-authored Recruit assembly must pass list validation"
            );
        }

        for oracle_id in [
            "00000000-0000-0000-0000-000000000000",
            "",
            "unreviewed-identical-clause",
        ] {
            let mut unreviewed = context();
            unreviewed.oracle_id = Some(oracle_id.into());
            assert!(
                match_clause(RECRUIT_CLAUSE, false, &unreviewed)
                    .expect("unreviewed clause should not be ambiguous")
                    .is_none(),
                "identical text must remain unsupported for an unreviewed Oracle identity: {oracle_id:?}"
            );
        }

        let mut reviewed = context();
        reviewed.oracle_id = Some(LONG_LAKE_NUISANCE_ID.into());
        for negative in [
            "When this creature enters, recruit.",
            "When this creature enters, you recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
            "When this creature enters, recruit. (Draw a card, then discard a card.)",
            "When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a land card, create a 1/1 white Human Soldier creature token.)",
            "When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 2/2 white Human Soldier creature token.)",
            "When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Soldier creature token.)",
            "When this creature enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token. Draw a card.)",
            "When this creature enters, it connives. (Draw a card, then discard a card. If you discarded a nonland card, put a +1/+1 counter on this creature.)",
            "When this creature dies, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
            "Whenever this creature attacks, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
            "When this enchantment enters, recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
            "When an opponent casts their first noncreature spell each turn, you recruit. (Draw a card, then discard a card. If you discarded a nonland card, create a 1/1 white Human Soldier creature token.)",
        ] {
            assert!(
                match_clause(negative, false, &reviewed)
                    .expect("near-miss should not be ambiguous")
                    .is_none(),
                "near-miss was accepted: {negative}"
            );
        }

        let mut noncreature = context();
        noncreature.oracle_id = Some(LONG_LAKE_NUISANCE_ID.into());
        noncreature.source_is_creature = false;
        assert!(
            match_clause(RECRUIT_CLAUSE, false, &noncreature)
                .expect("source-kind check should not be ambiguous")
                .is_none(),
            "the exact clause must remain bound to creature sources"
        );
        assert!(
            match_clause(RECRUIT_CLAUSE, true, &context())
                .expect("surface check should not be ambiguous")
                .is_none(),
            "the EtbAbility recipe must reject spell clauses"
        );
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
    fn issue_298_aura_etb_keyword_grants_and_controller_enchant_are_exact() {
        let cases = [
            (
                "88923ca1-a793-42f0-b9f8-ed9ff9c1185d",
                "When this Aura enters, enchanted creature gains first strike until end of turn.",
                "etb.aura.grant_first_strike.attached_until_end_of_turn",
                Keyword::FirstStrike,
            ),
            (
                "89fb21dc-4cf2-4c9a-b0ae-cc6e10277fb6",
                "When this Aura enters, enchanted creature gains first strike until end of turn.",
                "etb.aura.grant_first_strike.attached_until_end_of_turn",
                Keyword::FirstStrike,
            ),
            (
                "b9dee727-8ad8-42e0-93c6-5ef91d3f7309",
                "When this Aura enters, enchanted creature gains hexproof until end of turn. (It can't be the target of spells or abilities your opponents control.)",
                "etb.aura.grant_hexproof.attached_until_end_of_turn",
                Keyword::Hexproof,
            ),
            (
                "d3912c82-37f8-456e-ba49-65c7f5b39d13",
                "When this Aura enters, enchanted creature gains hexproof until end of turn.",
                "etb.aura.grant_hexproof.attached_until_end_of_turn",
                Keyword::Hexproof,
            ),
        ];
        for (oracle_id, clause, expected_id, keyword) in cases {
            let mut aura = context();
            aura.oracle_id = Some(oracle_id.into());
            let matched = match_clause(clause, false, &aura)
                .expect("issue #298 ETB recipe must not be ambiguous")
                .unwrap_or_else(|| panic!("issue #298 ETB clause must be supported: {clause}"));
            assert_eq!(matched.id.as_str(), expected_id);
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("issue #298 must emit a triggered ability");
            };
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(!ability.may);
            assert!(ability.targeting.is_none());
            assert!(ability.intervening_if.is_none());
            assert_eq!(
                ability.effect,
                [SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::AttachedObject,
                    keywords: vec![keyword],
                }]
            );
        }

        let mut aquitect = context();
        aquitect.oracle_id = Some("b9dee727-8ad8-42e0-93c6-5ef91d3f7309".into());
        let matched = match_clause("Enchant creature you control", false, &aquitect)
            .expect("issue #298 controller-only AuraAttach must not be ambiguous")
            .expect("issue #298 controller-only AuraAttach must be supported");
        assert_eq!(matched.id.as_str(), "aura.enchant.creature_you_control");
        assert_eq!(
            matched.emission,
            RecipeEmission::SpellEffect(SpellEffectKind::AuraAttach {
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                }
            })
        );
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

    #[test]
    fn issue_309_station_assembly_is_exact_and_allowlisted() {
        let pair = "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying";
        let mut uthros = context();
        uthros.source_name = "Uthros Scanship".into();
        let matched = match_station_assembly(pair, &uthros)
            .expect("Station assembly must not be ambiguous")
            .expect("reviewed Station pair should match");
        assert_eq!(matched.id.as_str(), "station.spacecraft.threshold_8_flying");
        let RecipeEmission::StationAssembly(assembly) = matched.emission else {
            panic!("Station pair must emit the paired activated and static abilities")
        };
        assert_eq!(
            assembly.activated_ability.costs,
            [AbilityCost::TapPermanents {
                constraint: ObjectPaymentConstraint::ExactCount(1),
                filter: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    ..TargetFilter::default()
                },
                exclude_source: true,
            }]
        );
        assert_eq!(
            assembly.activated_ability.timing,
            ActivationTiming::SorcerySpeed
        );
        assert_eq!(
            assembly.static_ability.definition,
            StaticAbilityDef::ConditionalSelfModifier {
                condition: GameCondition::SourceCounterCount {
                    counter: CounterKind::Charge,
                    min: Some(8),
                    max: None,
                },
                set_types: None,
                add_types: TypeLineAddition {
                    card_types: vec![PermanentTypeFilter::Creature],
                    creature_types: Vec::new(),
                },
                base_power: Some(4),
                base_toughness: Some(4),
                delta_power: 0,
                delta_toughness: 0,
                keywords: vec![Keyword::Flying],
                activated_abilities: Vec::new(),
                triggered_abilities: Vec::new(),
                can_attack_as_though_without_defender: false,
            }
        );

        for near_miss in [
            "Station",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)",
            "8+ | Flying",
            "Station (Tap this artifact: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its toughness on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8 | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8-10 | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8 or more.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "8+ | Flying\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying\nDraw a card.",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. Add a mana cost. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put +1/+1 counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on that creature. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact Vehicle at 8+.)\n8+ | Flying",
            "Station (Tap another creature an opponent controls: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Planet. Station only as a sorcery.)\n8+ | Flying",
        ] {
            assert_eq!(
                match_station_assembly(near_miss, &uthros),
                Ok(None),
                "Station near-miss must remain unsupported: {near_miss}"
            );
        }

        let mut unreviewed = uthros.clone();
        unreviewed.oracle_id = Some("00000000-0000-0000-0000-000000000000".into());
        assert_eq!(
            match_station_assembly(pair, &unreviewed),
            Ok(None),
            "an unreviewed identity must fail closed"
        );
        let mut nonstation = uthros.clone();
        nonstation.source_is_spacecraft_or_planet = false;
        assert_eq!(
            match_station_assembly(pair, &nonstation),
            Ok(None),
            "non-Spacecraft/Planet sources must fail closed"
        );
    }

    #[test]
    fn issue_309_station_assembly_rejects_catalog_ambiguity() {
        let pair = "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying";
        let mut uthros = context();
        uthros.source_name = "Uthros Scanship".into();
        let duplicate_catalog = [
            Recipe {
                id: RecipeId("test.station.duplicate.one"),
                label: "test Station duplicate one",
                surface: RecipeSurface::StationAssembly,
                matcher: match_station_8_flying_assembly,
                calibration: RecipeCalibration {
                    positive_cards: &[],
                    negative_near_misses: &[],
                    minimum_positive_cards: 0,
                },
            },
            Recipe {
                id: RecipeId("test.station.duplicate.two"),
                label: "test Station duplicate two",
                surface: RecipeSurface::StationAssembly,
                matcher: match_station_8_flying_assembly,
                calibration: RecipeCalibration {
                    positive_cards: &[],
                    negative_near_misses: &[],
                    minimum_positive_cards: 0,
                },
            },
        ];
        let ambiguity = match_surface_in(
            &duplicate_catalog,
            pair,
            RecipeSurface::StationAssembly,
            &uthros,
        )
        .expect_err("two matching Station recipes must be ambiguous");
        assert_eq!(
            ambiguity.recipe_ids,
            [
                RecipeId("test.station.duplicate.one"),
                RecipeId("test.station.duplicate.two")
            ]
        );
    }

    #[test]
    fn issue_310_station_variants_are_supported_for_both_reviewed_identities() {
        let cases = [
            (
                "Galvanizing Sawship",
                "dfe8f77a-cc26-438b-92ae-2ca7a91f813b",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
            ),
            (
                "Wedgelight Rammer",
                "03259ab0-caa8-4620-b070-127e2c712252",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            ),
        ];
        for (name, oracle_id, text) in cases {
            let mut reviewed = context();
            reviewed.source_name = name.into();
            reviewed.oracle_id = Some(oracle_id.into());
            let matched = match_station_assembly(text, &reviewed)
                .expect("issue #310 Station variant must not be ambiguous")
                .unwrap_or_else(|| {
                    panic!("issue #310 reviewed Station variant should match: {name}")
                });
            assert_eq!(
                matched.id.as_str(),
                "station.spacecraft.threshold_3_or_9_keywords"
            );
            let RecipeEmission::StationAssembly(assembly) = matched.emission else {
                panic!("issue #310 Station variant must emit a Station assembly: {name}");
            };
            let expected = if name == "Galvanizing Sawship" {
                (3, 6, 5, vec![Keyword::Flying, Keyword::Haste])
            } else {
                (9, 3, 4, vec![Keyword::Flying, Keyword::FirstStrike])
            };
            assert_eq!(
                assembly.static_ability.definition,
                StaticAbilityDef::ConditionalSelfModifier {
                    condition: GameCondition::SourceCounterCount {
                        counter: CounterKind::Charge,
                        min: Some(expected.0),
                        max: None,
                    },
                    set_types: None,
                    add_types: TypeLineAddition {
                        card_types: vec![PermanentTypeFilter::Creature],
                        creature_types: Vec::new(),
                    },
                    base_power: Some(expected.1),
                    base_toughness: Some(expected.2),
                    delta_power: 0,
                    delta_toughness: 0,
                    keywords: expected.3,
                    activated_abilities: Vec::new(),
                    triggered_abilities: Vec::new(),
                    can_attack_as_though_without_defender: false,
                }
            );
        }

        let mut wedgelight = context();
        wedgelight.source_name = "Wedgelight Rammer".into();
        wedgelight.oracle_id = Some(ISSUE_310_WEDGELIGHT_ORACLE_ID.into());
        let etb = match_clause(ISSUE_310_WEDGELIGHT_ETB_TEXT, false, &wedgelight)
            .expect("Wedgelight ETB must not be ambiguous")
            .expect("Wedgelight ETB must match");
        assert_eq!(etb.id.as_str(), "etb.spacecraft.wedgelight.create_robot");
        let RecipeEmission::TriggeredAbility(ability) = etb.emission else {
            panic!("Wedgelight ETB must emit a triggered ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::CreateTokens {
                token: "robot_c_2_2".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }]
        );
    }

    #[test]
    fn issue_310_station_variants_reject_near_misses_and_unreviewed_surfaces() {
        let galvanizing_pair = "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste";
        let wedgelight_pair = "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike";
        let mut galvanizing = context();
        galvanizing.source_name = "Galvanizing Sawship".into();
        galvanizing.oracle_id = Some(ISSUE_310_GALVANIZING_ORACLE_ID.into());
        let mut wedgelight = context();
        wedgelight.source_name = "Wedgelight Rammer".into();
        wedgelight.oracle_id = Some(ISSUE_310_WEDGELIGHT_ORACLE_ID.into());

        for near_miss in [
            "Station",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)",
            "3+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 2+.)\n2+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 4+.)\n4+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 8+.)\n8+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 10+.)\n10+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3.)\n3 | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Haste, flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, vigilance",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as an instant. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n9+ | Flying, haste",
            "3+ | Flying, haste\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste\n3+ | Flying, haste",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
            "Station ({1}, Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
            "Station (Tap this artifact: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its toughness on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put +1/+1 counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on that creature. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature an opponent controls: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Planet. Station only as a sorcery.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact Vehicle at 9+.)\n9+ | Flying, first strike",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike\nDraw a card.",
        ] {
            assert_eq!(
                match_station_assembly(near_miss, &galvanizing),
                Ok(None),
                "Station near-miss must remain unsupported for Galvanizing: {near_miss}"
            );
            assert_eq!(
                match_station_assembly(near_miss, &wedgelight),
                Ok(None),
                "Station near-miss must remain unsupported for Wedgelight: {near_miss}"
            );
        }

        let mut unreviewed = galvanizing.clone();
        unreviewed.oracle_id = Some("00000000-0000-0000-0000-000000000000".into());
        assert_eq!(
            match_station_assembly(galvanizing_pair, &unreviewed),
            Ok(None)
        );
        assert_eq!(
            match_station_assembly(wedgelight_pair, &galvanizing),
            Ok(None)
        );
        assert_eq!(
            match_station_assembly(galvanizing_pair, &wedgelight),
            Ok(None)
        );

        let duplicate_catalog = [
            Recipe {
                id: RecipeId("test.station310.duplicate.one"),
                label: "test Station #310 duplicate one",
                surface: RecipeSurface::StationAssembly,
                matcher: match_station_3_or_9_keyword_assembly,
                calibration: RecipeCalibration {
                    positive_cards: &[],
                    negative_near_misses: &[],
                    minimum_positive_cards: 0,
                },
            },
            Recipe {
                id: RecipeId("test.station310.duplicate.two"),
                label: "test Station #310 duplicate two",
                surface: RecipeSurface::StationAssembly,
                matcher: match_station_3_or_9_keyword_assembly,
                calibration: RecipeCalibration {
                    positive_cards: &[],
                    negative_near_misses: &[],
                    minimum_positive_cards: 0,
                },
            },
        ];
        let ambiguity = match_surface_in(
            &duplicate_catalog,
            galvanizing_pair,
            RecipeSurface::StationAssembly,
            &galvanizing,
        )
        .expect_err("two matching #310 Station recipes must be ambiguous");
        assert_eq!(
            ambiguity.recipe_ids,
            [
                RecipeId("test.station310.duplicate.one"),
                RecipeId("test.station310.duplicate.two")
            ]
        );

        for (mut invalid, label) in [
            (galvanizing.clone(), "non-artifact"),
            (galvanizing.clone(), "non-permanent"),
            (galvanizing.clone(), "non-spacecraft"),
        ] {
            match label {
                "non-artifact" => invalid.source_is_artifact = false,
                "non-permanent" => invalid.source_is_permanent = false,
                "non-spacecraft" => invalid.source_is_spacecraft_or_planet = false,
                _ => unreachable!(),
            }
            assert_eq!(
                match_station_assembly(galvanizing_pair, &invalid),
                Ok(None),
                "{label} Station source must fail closed"
            );
        }

        for near_miss in [
            "When this Spacecraft enters, create two 2/2 colorless Robot artifact creature tokens.",
            "When this Spacecraft enters, create a tapped 2/2 colorless Robot artifact creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Robot creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Servo artifact creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token. You may.",
            "When this artifact enters, create a 2/2 colorless Robot artifact creature token.",
            "Whenever this Spacecraft enters, create a 2/2 colorless Robot artifact creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token, then draw a card.",
            "When this Spacecraft enters, create a 3/3 colorless Robot artifact creature token.",
            "When this Spacecraft enters, create a 2/2 white Robot artifact creature token.",
            "When this Spacecraft enters, target player creates a 2/2 colorless Robot artifact creature token.",
            "When this Spacecraft enters, each player creates a 2/2 colorless Robot artifact creature token.",
            "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token under your control.",
            "When this Spacecraft enters, you may create a 2/2 colorless Robot artifact creature token.",
        ] {
            assert_eq!(
                match_clause(near_miss, false, &wedgelight),
                Ok(None),
                "Wedgelight ETB near-miss must remain unsupported: {near_miss}"
            );
        }

        let mut nonspacecraft = wedgelight.clone();
        nonspacecraft.source_is_spacecraft_or_planet = false;
        assert_eq!(
            match_clause(ISSUE_310_WEDGELIGHT_ETB_TEXT, false, &nonspacecraft),
            Ok(None)
        );
    }

    #[test]
    fn issue_310_card_surfaces_require_exact_identity_and_printed_fields() {
        assert!(issue_310_card_surface_is_exact(
            ISSUE_310_GALVANIZING_ORACLE_ID,
            "Galvanizing Sawship",
            "{5}{R}",
            "Artifact — Spacecraft",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
            Some("6"),
            Some("5"),
        ));
        assert!(issue_310_card_surface_is_exact(
            ISSUE_310_WEDGELIGHT_ORACLE_ID,
            "Wedgelight Rammer",
            "{3}{W}",
            "Artifact — Spacecraft",
            "When this Spacecraft enters, create a 2/2 colorless Robot artifact creature token.\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 9+.)\n9+ | Flying, first strike",
            Some("3"),
            Some("4"),
        ));
        for (name, oracle_id, mana, type_line, text, power, toughness) in [
            (
                "Wrong Name",
                ISSUE_310_GALVANIZING_ORACLE_ID,
                "{5}{R}",
                "Artifact — Spacecraft",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
                Some("6"),
                Some("5"),
            ),
            (
                "Galvanizing Sawship",
                ISSUE_310_GALVANIZING_ORACLE_ID,
                "{4}{R}",
                "Artifact — Spacecraft",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
                Some("6"),
                Some("5"),
            ),
            (
                "Galvanizing Sawship",
                ISSUE_310_GALVANIZING_ORACLE_ID,
                "{5}{R}",
                "Artifact — Vehicle",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
                Some("6"),
                Some("5"),
            ),
            (
                "Galvanizing Sawship",
                ISSUE_310_GALVANIZING_ORACLE_ID,
                "{5}{R}",
                "Artifact — Spacecraft",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, vigilance",
                Some("6"),
                Some("5"),
            ),
            (
                "Galvanizing Sawship",
                ISSUE_310_GALVANIZING_ORACLE_ID,
                "{5}{R}",
                "Artifact — Spacecraft",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste",
                Some("7"),
                Some("5"),
            ),
            (
                "Galvanizing Sawship",
                ISSUE_310_GALVANIZING_ORACLE_ID,
                "{5}{R}",
                "Artifact — Spacecraft",
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 3+.)\n3+ | Flying, haste\nDraw a card.",
                Some("6"),
                Some("5"),
            ),
        ] {
            assert!(!issue_310_card_surface_is_exact(
                oracle_id, name, mana, type_line, text, power, toughness
            ));
        }
        assert!(issue_310_card_surface_is_exact(
            "00000000-0000-0000-0000-000000000000",
            "Unreviewed",
            "{1}",
            "Artifact",
            "unreviewed",
            None,
            None,
        ));
    }

    #[test]
    fn issue_311_station_variants_emit_exact_threshold_characteristics() {
        let cases = [
            (
                "Pinnacle Kill-Ship",
                ISSUE_311_PINNACLE_ORACLE_ID,
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
                7,
                7,
                7,
            ),
            (
                "Warmaker Gunship",
                ISSUE_311_WARMAKER_ORACLE_ID,
                "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 6+.)\n6+ | Flying",
                6,
                4,
                3,
            ),
        ];
        for (name, oracle_id, pair, threshold, power, toughness) in cases {
            let mut reviewed = context();
            reviewed.source_name = name.into();
            reviewed.oracle_id = Some(oracle_id.into());
            let matched = match_station_assembly(pair, &reviewed)
                .expect("#311 Station pair must not be ambiguous")
                .expect("#311 reviewed Station pair should match");
            assert_eq!(
                matched.id.as_str(),
                "station.spacecraft.threshold_6_7_flying"
            );
            let RecipeEmission::StationAssembly(assembly) = matched.emission else {
                panic!("#311 Station pair must emit an assembly")
            };
            assert_eq!(
                assembly.activated_ability.timing,
                ActivationTiming::SorcerySpeed
            );
            assert_eq!(
                assembly.activated_ability.costs,
                [AbilityCost::TapPermanents {
                    constraint: ObjectPaymentConstraint::ExactCount(1),
                    filter: TargetFilter {
                        kind: TargetKind::Creature,
                        controller: TargetController::You,
                        ..TargetFilter::default()
                    },
                    exclude_source: true,
                }]
            );
            assert_eq!(
                assembly.static_ability.definition,
                StaticAbilityDef::ConditionalSelfModifier {
                    condition: GameCondition::SourceCounterCount {
                        counter: CounterKind::Charge,
                        min: Some(threshold),
                        max: None,
                    },
                    set_types: None,
                    add_types: TypeLineAddition {
                        card_types: vec![PermanentTypeFilter::Creature],
                        creature_types: Vec::new(),
                    },
                    base_power: Some(power),
                    base_toughness: Some(toughness),
                    delta_power: 0,
                    delta_toughness: 0,
                    keywords: vec![Keyword::Flying],
                    activated_abilities: Vec::new(),
                    triggered_abilities: Vec::new(),
                    can_attack_as_though_without_defender: false,
                }
            );
        }
    }

    #[test]
    fn issue_311_targeted_etbs_emit_exact_typed_effects_and_targeting() {
        let mut pinnacle = context();
        pinnacle.source_name = "Pinnacle Kill-Ship".into();
        pinnacle.oracle_id = Some(ISSUE_311_PINNACLE_ORACLE_ID.into());
        let pinnacle_match = match_clause(ISSUE_311_PINNACLE_ETB_TEXT, false, &pinnacle)
            .expect("Pinnacle ETB must not be ambiguous")
            .expect("Pinnacle ETB must match");
        assert_eq!(
            pinnacle_match.id.as_str(),
            "etb.spacecraft.pinnacle.damage_ten_up_to_one_creature"
        );
        let RecipeEmission::TriggeredAbility(pinnacle_ability) = pinnacle_match.emission else {
            panic!("Pinnacle ETB must emit a triggered ability")
        };
        assert!(!pinnacle_ability.may);
        assert_eq!(
            pinnacle_ability.effect,
            [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(10),
                target: TargetFilter::default_creature(),
            }]
        );
        assert_eq!(
            pinnacle_ability.targeting,
            Some(exact_targeting(
                0,
                1,
                "Choose up to one target creature",
                vec![0]
            ))
        );

        let mut warmaker = context();
        warmaker.source_name = "Warmaker Gunship".into();
        warmaker.oracle_id = Some(ISSUE_311_WARMAKER_ORACLE_ID.into());
        let warmaker_match = match_clause(ISSUE_311_WARMAKER_ETB_TEXT, false, &warmaker)
            .expect("Warmaker ETB must not be ambiguous")
            .expect("Warmaker ETB must match");
        assert_eq!(
            warmaker_match.id.as_str(),
            "etb.spacecraft.warmaker.damage_artifact_count_opponent_creature"
        );
        let RecipeEmission::TriggeredAbility(warmaker_ability) = warmaker_match.emission else {
            panic!("Warmaker ETB must emit a triggered ability")
        };
        assert_eq!(
            warmaker_ability.effect,
            [SpellEffectKind::DamageTarget {
                amount: Amount::Count(CountExpression::BattlefieldPermanents {
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
                }),
                target: TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::Opponent,
                    ..TargetFilter::default()
                },
            }]
        );
        assert_eq!(
            warmaker_ability
                .targeting
                .as_ref()
                .expect("Warmaker target group")
                .groups[0]
                .min,
            1
        );
        assert_eq!(
            warmaker_ability
                .targeting
                .as_ref()
                .expect("Warmaker target group")
                .groups[0]
                .max,
            1
        );
    }

    #[test]
    fn issue_311_recipes_reject_near_misses_cross_card_mixing_and_ambiguity() {
        let mut pinnacle = context();
        pinnacle.source_name = "Pinnacle Kill-Ship".into();
        pinnacle.oracle_id = Some(ISSUE_311_PINNACLE_ORACLE_ID.into());
        let station = "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying";
        for near_miss in [
            "Station",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)",
            "7+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 11+.)\n11+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as an instant. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7 | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying\n7+ | Flying",
            "7+ | Flying\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)",
            "Station ({1}, Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap this artifact: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its toughness on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature you control: Put +1/+1 counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on that creature. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature an opponent controls: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 7+.)\n7+ | Flying\nDraw a card.",
        ] {
            assert_eq!(
                match_station_assembly(near_miss, &pinnacle),
                Ok(None),
                "Station near-miss must fail closed: {near_miss}"
            );
        }
        let mut unreviewed = pinnacle.clone();
        unreviewed.oracle_id = Some("00000000-0000-0000-0000-000000000000".into());
        assert_eq!(match_station_assembly(station, &unreviewed), Ok(None));
        let warmaker_station = "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 6+.)\n6+ | Flying";
        assert_eq!(
            match_station_assembly(warmaker_station, &pinnacle),
            Ok(None)
        );
        let mut warmaker = pinnacle.clone();
        warmaker.source_name = "Warmaker Gunship".into();
        warmaker.oracle_id = Some(ISSUE_311_WARMAKER_ORACLE_ID.into());
        assert_eq!(
            match_clause(ISSUE_311_PINNACLE_ETB_TEXT, false, &warmaker),
            Ok(None),
            "cross-card ETB variants must not mix"
        );
        for (context, text) in [
            (
                &pinnacle,
                "When this Spacecraft enters, it deals 9 damage to up to one target creature.",
            ),
            (
                &pinnacle,
                "When this Spacecraft enters, it deals 11 damage to up to one target creature.",
            ),
            (
                &pinnacle,
                "When this Spacecraft enters, it deals 10 damage to one target creature.",
            ),
            (
                &pinnacle,
                "When this Spacecraft enters, it deals 10 damage to up to one target player.",
            ),
            (
                &warmaker,
                "When this Spacecraft enters, it deals damage equal to the number of artifacts an opponent controls to target creature an opponent controls.",
            ),
            (
                &warmaker,
                "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to any target.",
            ),
            (
                &warmaker,
                "When this Spacecraft enters, it deals damage equal to the number of creatures you control to target creature an opponent controls.",
            ),
            (
                &warmaker,
                "When this Spacecraft enters, it deals damage equal to its power to target creature an opponent controls.",
            ),
            (
                &warmaker,
                "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to up to one target creature an opponent controls.",
            ),
            (
                &warmaker,
                "When this Spacecraft enters, it deals damage equal to the number of artifacts you control to target creature an opponent controls. You gain 1 life.",
            ),
            (
                &pinnacle,
                "When this Spacecraft enters, it deals 10 damage to up to one target planeswalker.",
            ),
            (
                &pinnacle,
                "When this Spacecraft enters, it deals 10 damage to up to one target player.",
            ),
            (
                &pinnacle,
                "When this Spacecraft enters, you may deal 10 damage to up to one target creature.",
            ),
            (
                &pinnacle,
                "Whenever this Spacecraft enters, it deals 10 damage to up to one target creature.",
            ),
            (
                &warmaker,
                "When this Spacecraft enters, it deals damage equal to the number of artifacts an opponent controls to target creature an opponent controls.",
            ),
        ] {
            assert_eq!(match_clause(text, false, context), Ok(None), "near-miss: {text}");
        }

        for (mut invalid, exact_text) in [
            (pinnacle.clone(), ISSUE_311_PINNACLE_ETB_TEXT),
            (warmaker.clone(), ISSUE_311_WARMAKER_ETB_TEXT),
        ] {
            invalid.source_is_artifact = false;
            assert_eq!(match_clause(exact_text, false, &invalid), Ok(None));
            invalid.source_is_artifact = true;
            invalid.source_is_permanent = false;
            assert_eq!(match_clause(exact_text, false, &invalid), Ok(None));
            invalid.source_is_permanent = true;
            invalid.source_is_spacecraft_or_planet = false;
            assert_eq!(match_clause(exact_text, false, &invalid), Ok(None));
        }

        let duplicate_catalog = [
            Recipe {
                id: RecipeId("test.station311.duplicate.one"),
                label: "test Station #311 duplicate one",
                surface: RecipeSurface::StationAssembly,
                matcher: match_station_6_7_flying_assembly,
                calibration: RecipeCalibration {
                    positive_cards: &[],
                    negative_near_misses: &[],
                    minimum_positive_cards: 0,
                },
            },
            Recipe {
                id: RecipeId("test.station311.duplicate.two"),
                label: "test Station #311 duplicate two",
                surface: RecipeSurface::StationAssembly,
                matcher: match_station_6_7_flying_assembly,
                calibration: RecipeCalibration {
                    positive_cards: &[],
                    negative_near_misses: &[],
                    minimum_positive_cards: 0,
                },
            },
        ];
        let ambiguity = match_surface_in(
            &duplicate_catalog,
            station,
            RecipeSurface::StationAssembly,
            &pinnacle,
        )
        .expect_err("duplicate #311 Station recipes must be ambiguous");
        assert_eq!(
            ambiguity.recipe_ids,
            [
                RecipeId("test.station311.duplicate.one"),
                RecipeId("test.station311.duplicate.two")
            ]
        );
    }

    #[test]
    fn issue_311_card_surfaces_require_exact_identity_and_printed_fields() {
        let cases = [
            (
                ISSUE_311_PINNACLE_ORACLE_ID,
                "Pinnacle Kill-Ship",
                "{7}",
                "Artifact — Spacecraft",
                ISSUE_311_PINNACLE_ETB_TEXT.to_string()
                    + "\n"
                    + ISSUE_311_PINNACLE_STATION_HEADER
                    + "\n"
                    + ISSUE_311_PINNACLE_THRESHOLD_LINE,
                Some("7"),
                Some("7"),
            ),
            (
                ISSUE_311_WARMAKER_ORACLE_ID,
                "Warmaker Gunship",
                "{2}{R}",
                "Artifact — Spacecraft",
                ISSUE_311_WARMAKER_ETB_TEXT.to_string()
                    + "\n"
                    + ISSUE_311_WARMAKER_STATION_HEADER
                    + "\n"
                    + ISSUE_311_WARMAKER_THRESHOLD_LINE,
                Some("4"),
                Some("3"),
            ),
        ];
        for (oracle_id, name, mana, type_line, text, power, toughness) in cases {
            assert!(issue_311_card_surface_is_exact(
                oracle_id, name, mana, type_line, &text, power, toughness
            ));
            assert!(!issue_311_card_surface_is_exact(
                oracle_id, name, mana, "Artifact", &text, power, toughness
            ));
            assert!(!issue_311_card_surface_is_exact(
                oracle_id,
                name,
                mana,
                type_line,
                &text,
                Some("0"),
                toughness
            ));
            assert!(!issue_311_card_surface_is_exact(
                oracle_id,
                "Wrong Name",
                mana,
                type_line,
                &text,
                power,
                toughness
            ));
            assert!(!issue_311_card_surface_is_exact(
                oracle_id, name, "{1}", type_line, &text, power, toughness
            ));
            assert!(!issue_311_card_surface_is_exact(
                oracle_id,
                name,
                mana,
                type_line,
                &text,
                power,
                Some("0")
            ));
            assert!(!issue_311_card_surface_is_exact(
                oracle_id,
                name,
                mana,
                type_line,
                "changed complete Oracle surface",
                power,
                toughness
            ));
        }
        assert!(issue_311_card_surface_is_exact(
            "unreviewed",
            "Unreviewed",
            "{1}",
            "Artifact",
            "anything",
            None,
            None
        ));
    }

    #[test]
    fn issue_313_station_variants_emit_exact_threshold_characteristics() {
        let cases = [
            (
                "Extinguisher Battleship",
                ISSUE_313_EXTINGUISHER_ORACLE_ID,
                ISSUE_313_EXTINGUISHER_STATION_HEADER,
                ISSUE_313_EXTINGUISHER_THRESHOLD_LINE,
                5,
                10,
                10,
                vec![Keyword::Flying, Keyword::Trample],
            ),
            (
                "Fell Gravship",
                ISSUE_313_FELL_ORACLE_ID,
                ISSUE_313_FELL_STATION_HEADER,
                ISSUE_313_FELL_THRESHOLD_LINE,
                8,
                3,
                2,
                vec![Keyword::Flying, Keyword::Lifelink],
            ),
        ];
        for (name, oracle_id, header, threshold_line, threshold, power, toughness, keywords) in
            cases
        {
            let mut reviewed = context();
            reviewed.source_name = name.into();
            reviewed.oracle_id = Some(oracle_id.into());
            let pair = format!("{header}\n{threshold_line}");
            let matched = match_station_assembly(&pair, &reviewed)
                .expect("#313 Station pair must not be ambiguous")
                .expect("#313 reviewed Station pair should match");
            assert_eq!(
                matched.id.as_str(),
                "station.spacecraft.threshold_5_or_8_keywords"
            );
            let RecipeEmission::StationAssembly(assembly) = matched.emission else {
                panic!("#313 Station pair must emit an assembly")
            };
            assert_eq!(
                assembly.activated_ability.timing,
                ActivationTiming::SorcerySpeed
            );
            assert_eq!(
                assembly.static_ability.definition,
                StaticAbilityDef::ConditionalSelfModifier {
                    condition: GameCondition::SourceCounterCount {
                        counter: CounterKind::Charge,
                        min: Some(threshold),
                        max: None,
                    },
                    set_types: None,
                    add_types: TypeLineAddition {
                        card_types: vec![PermanentTypeFilter::Creature],
                        creature_types: Vec::new(),
                    },
                    base_power: Some(power),
                    base_toughness: Some(toughness),
                    delta_power: 0,
                    delta_toughness: 0,
                    keywords,
                    activated_abilities: Vec::new(),
                    triggered_abilities: Vec::new(),
                    can_attack_as_though_without_defender: false,
                }
            );
        }
    }

    #[test]
    fn issue_313_etbs_emit_exact_typed_effects_and_resolution_contracts() {
        let mut extinguisher = context();
        extinguisher.source_name = "Extinguisher Battleship".into();
        extinguisher.oracle_id = Some(ISSUE_313_EXTINGUISHER_ORACLE_ID.into());
        let matched = match_clause(ISSUE_313_EXTINGUISHER_ETB_TEXT, false, &extinguisher)
            .expect("Extinguisher ETB must not be ambiguous")
            .expect("Extinguisher ETB must match");
        assert_eq!(
            matched.id.as_str(),
            "etb.spacecraft.extinguisher.destroy_noncreature_then_damage_all_creatures"
        );
        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("Extinguisher ETB must emit a triggered ability")
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(!ability.may);
        assert_eq!(
            ability.effect,
            [
                SpellEffectKind::Destroy {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        excluded_permanent_types: vec![PermanentTypeFilter::Creature],
                        ..TargetFilter::default()
                    })),
                },
                SpellEffectKind::DamageAll {
                    amount: Amount::Fixed(4),
                    players: RelativePlayerSet::All,
                    kind: TargetFilter::default_creature(),
                },
            ]
        );
        let [group] = ability.targeting.as_ref().unwrap().groups.as_slice() else {
            panic!("Extinguisher target group")
        };
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.effect_indices, [0]);

        let mut fell = context();
        fell.source_name = "Fell Gravship".into();
        fell.oracle_id = Some(ISSUE_313_FELL_ORACLE_ID.into());
        let matched = match_clause(ISSUE_313_FELL_ETB_TEXT, false, &fell)
            .expect("Fell ETB must not be ambiguous")
            .expect("Fell ETB must match");
        assert_eq!(
            matched.id.as_str(),
            "etb.spacecraft.fell.mill_three_then_return_creature_or_spacecraft"
        );
        let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
            panic!("Fell ETB must emit a triggered ability")
        };
        assert!(ability.targeting.is_none());
        assert_eq!(
            ability.effect,
            [
                SpellEffectKind::Mill {
                    count: Amount::Fixed(3),
                    who: PlayerRecipient::Controller,
                },
                SpellEffectKind::ChooseGraveyardCard {
                    filter: ZoneCardFilter {
                        any_of: Some(vec![
                            ZoneCardFilter {
                                card_type: Some(CardTypeFilter::Creature),
                                ..ZoneCardFilter::default()
                            },
                            ZoneCardFilter {
                                required_subtypes: vec!["Spacecraft".into()],
                                ..ZoneCardFilter::default()
                            },
                        ]),
                        ..ZoneCardFilter::default()
                    },
                    destination: GraveyardDestination::Hand,
                    optional: false,
                    from_result: None,
                },
            ]
        );
    }

    #[test]
    fn issue_313_recipes_reject_near_misses_cross_card_mixing_and_context_mutations() {
        let mut extinguisher = context();
        extinguisher.source_name = "Extinguisher Battleship".into();
        extinguisher.oracle_id = Some(ISSUE_313_EXTINGUISHER_ORACLE_ID.into());
        let station = format!(
            "{}\n{}",
            ISSUE_313_EXTINGUISHER_STATION_HEADER, ISSUE_313_EXTINGUISHER_THRESHOLD_LINE
        );
        for near_miss in [
            "Station",
            ISSUE_313_EXTINGUISHER_STATION_HEADER,
            ISSUE_313_EXTINGUISHER_THRESHOLD_LINE,
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 4+.)\n4+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 6+.)\n6+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying, trample\n5+ | Flying, trample",
            "5+ | Flying, trample\nStation (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as an instant. It's an artifact creature at 5+.)\n5+ | Flying, trample",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Trample, flying",
            "Station (Tap another creature you control: Put charge counters equal to its power on this Spacecraft. Station only as a sorcery. It's an artifact creature at 5+.)\n5+ | Flying",
        ] {
            assert_eq!(
                match_station_assembly(near_miss, &extinguisher),
                Ok(None),
                "Station near-miss must fail closed: {near_miss}"
            );
        }
        let mut fell_station = extinguisher.clone();
        fell_station.source_name = "Fell Gravship".into();
        fell_station.oracle_id = Some(ISSUE_313_FELL_ORACLE_ID.into());
        let fell_station_pair = format!(
            "{}\n{}",
            ISSUE_313_FELL_STATION_HEADER, ISSUE_313_FELL_THRESHOLD_LINE
        );
        assert_eq!(
            match_station_assembly(&station, &fell_station),
            Ok(None),
            "cross-card Station variants must not mix"
        );
        assert_eq!(
            match_clause(ISSUE_313_FELL_ETB_TEXT, false, &extinguisher),
            Ok(None),
            "Fell's ETB must not enter the Extinguisher recipe"
        );
        assert_eq!(
            match_clause(ISSUE_313_EXTINGUISHER_ETB_TEXT, false, &fell_station),
            Ok(None),
            "Extinguisher's ETB must not enter the Fell recipe"
        );
        for mut invalid in [extinguisher.clone(), fell_station.clone()] {
            invalid.source_is_artifact = false;
            assert_eq!(
                match_station_assembly(
                    if invalid.source_name == "Extinguisher Battleship" {
                        station.as_str()
                    } else {
                        fell_station_pair.as_str()
                    },
                    &invalid
                ),
                Ok(None)
            );
        }

        for (context, exact, near_misses) in [
            (
                extinguisher.clone(),
                ISSUE_313_EXTINGUISHER_ETB_TEXT,
                vec![
                    "When this Spacecraft enters, destroy target creature. Then this Spacecraft deals 4 damage to each creature.",
                    "When this Spacecraft enters, destroy target artifact. Then this Spacecraft deals 4 damage to each creature.",
                    "When this Spacecraft enters, destroy target land. Then this Spacecraft deals 4 damage to each creature.",
                    "When this Spacecraft enters, destroy target permanent. Then this Spacecraft deals 4 damage to each creature.",
                    "When this Spacecraft enters, destroy up to one target noncreature permanent. Then this Spacecraft deals 4 damage to each creature.",
                    "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 3 damage to each creature.",
                    "When this Spacecraft enters, destroy target noncreature permanent. Then this Spacecraft deals 4 damage to target creature.",
                    "When this Spacecraft enters, this Spacecraft deals 4 damage to each creature. Then destroy target noncreature permanent.",
                    "When this Spacecraft enters, destroy target noncreature permanent. Then it deals 4 damage to each creature.",
                ],
            ),
            (
                fell_station.clone(),
                ISSUE_313_FELL_ETB_TEXT,
                vec![
                    "When this Spacecraft enters, mill two cards, then return a creature or Spacecraft card from your graveyard to your hand.",
                    "When this Spacecraft enters, mill four cards, then return a creature or Spacecraft card from your graveyard to your hand.",
                    "When this Spacecraft enters, target player mills three cards, then return a creature or Spacecraft card from your graveyard to your hand.",
                    "When this artifact enters, mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.",
                    "Whenever this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.",
                    "When this Spacecraft enters, mill three cards, then return a creature card from your graveyard to your hand.",
                    "When this Spacecraft enters, mill three cards, then return a Spacecraft card from your graveyard to your hand.",
                    "When this Spacecraft enters, mill three cards, then return a card from your graveyard to your hand.",
                    "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card milled this way from your graveyard to your hand.",
                    "When this Spacecraft enters, mill three cards, then return target creature or Spacecraft card from your graveyard to your hand.",
                    "When this Spacecraft enters, you may mill three cards, then return a creature or Spacecraft card from your graveyard to your hand.",
                    "When this Spacecraft enters, mill three cards, then you may return a creature or Spacecraft card from your graveyard to your hand.",
                    "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to the battlefield.",
                    "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from your graveyard to exile.",
                    "When this Spacecraft enters, return a creature or Spacecraft card from your graveyard to your hand, then mill three cards.",
                    "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from an opponent's graveyard to your hand.",
                    "When this Spacecraft enters, mill three cards, then return a creature or Spacecraft card from a graveyard to your hand.",
                ],
            ),
        ] {
            assert!(
                match_clause(exact, false, &context)
                    .expect("exact ETB must not be ambiguous")
                    .is_some()
            );
            for near_miss in near_misses {
                assert_eq!(
                    match_clause(near_miss, false, &context),
                    Ok(None),
                    "ETB near-miss must fail closed: {near_miss}"
                );
            }
            let mut unreviewed = context.clone();
            unreviewed.oracle_id = Some("00000000-0000-0000-0000-000000000000".into());
            assert_eq!(match_clause(exact, false, &unreviewed), Ok(None));
            let mut nonartifact = context.clone();
            nonartifact.source_is_artifact = false;
            assert_eq!(match_clause(exact, false, &nonartifact), Ok(None));
            let mut nonpermanent = context.clone();
            nonpermanent.source_is_permanent = false;
            assert_eq!(match_clause(exact, false, &nonpermanent), Ok(None));
            let mut nonspacecraft = context.clone();
            nonspacecraft.source_is_spacecraft_or_planet = false;
            assert_eq!(match_clause(exact, false, &nonspacecraft), Ok(None));
        }

        let duplicate_catalog = [
            Recipe {
                id: RecipeId("test.station313.duplicate.one"),
                label: "test Station #313 duplicate one",
                surface: RecipeSurface::StationAssembly,
                matcher: issue_313_station_assembly,
                calibration: RecipeCalibration {
                    positive_cards: &[],
                    negative_near_misses: &[],
                    minimum_positive_cards: 0,
                },
            },
            Recipe {
                id: RecipeId("test.station313.duplicate.two"),
                label: "test Station #313 duplicate two",
                surface: RecipeSurface::StationAssembly,
                matcher: issue_313_station_assembly,
                calibration: RecipeCalibration {
                    positive_cards: &[],
                    negative_near_misses: &[],
                    minimum_positive_cards: 0,
                },
            },
        ];
        let ambiguity = match_surface_in(
            &duplicate_catalog,
            &station,
            RecipeSurface::StationAssembly,
            &extinguisher,
        )
        .expect_err("duplicate #313 Station recipes must be ambiguous");
        assert_eq!(
            ambiguity.recipe_ids,
            [
                RecipeId("test.station313.duplicate.one"),
                RecipeId("test.station313.duplicate.two")
            ]
        );
    }

    #[test]
    fn issue_313_cross_card_etb_surfaces_reject_both_directions() {
        let mut extinguisher = context();
        extinguisher.source_name = "Extinguisher Battleship".into();
        extinguisher.oracle_id = Some(ISSUE_313_EXTINGUISHER_ORACLE_ID.into());

        let mut fell = context();
        fell.source_name = "Fell Gravship".into();
        fell.oracle_id = Some(ISSUE_313_FELL_ORACLE_ID.into());

        assert_eq!(
            match_clause(ISSUE_313_FELL_ETB_TEXT, false, &extinguisher),
            Ok(None),
            "Fell ETB text must not match Extinguisher identity/context"
        );
        assert_eq!(
            match_clause(ISSUE_313_EXTINGUISHER_ETB_TEXT, false, &fell),
            Ok(None),
            "Extinguisher ETB text must not match Fell identity/context"
        );
    }

    #[test]
    fn issue_313_card_surfaces_require_exact_identity_and_printed_fields() {
        let cases = [
            (
                ISSUE_313_EXTINGUISHER_ORACLE_ID,
                "Extinguisher Battleship",
                "{8}",
                "Artifact — Spacecraft",
                ISSUE_313_EXTINGUISHER_ETB_TEXT.to_string()
                    + "\n"
                    + ISSUE_313_EXTINGUISHER_STATION_HEADER
                    + "\n"
                    + ISSUE_313_EXTINGUISHER_THRESHOLD_LINE,
                Some("10"),
                Some("10"),
            ),
            (
                ISSUE_313_FELL_ORACLE_ID,
                "Fell Gravship",
                "{2}{B}",
                "Artifact — Spacecraft",
                ISSUE_313_FELL_ETB_TEXT.to_string()
                    + "\n"
                    + ISSUE_313_FELL_STATION_HEADER
                    + "\n"
                    + ISSUE_313_FELL_THRESHOLD_LINE,
                Some("3"),
                Some("2"),
            ),
        ];
        for (oracle_id, name, mana, type_line, text, power, toughness) in cases {
            assert!(issue_313_card_surface_is_exact(
                oracle_id, name, mana, type_line, &text, power, toughness
            ));
            assert!(!issue_313_card_surface_is_exact(
                oracle_id, name, mana, "Artifact", &text, power, toughness
            ));
            assert!(!issue_313_card_surface_is_exact(
                oracle_id,
                name,
                mana,
                type_line,
                &text,
                Some("0"),
                toughness
            ));
            assert!(!issue_313_card_surface_is_exact(
                oracle_id,
                name,
                mana,
                type_line,
                &text,
                power,
                Some("0")
            ));
            assert!(!issue_313_card_surface_is_exact(
                oracle_id,
                "Wrong Name",
                mana,
                type_line,
                &text,
                power,
                toughness
            ));
            assert!(!issue_313_card_surface_is_exact(
                oracle_id, name, "{1}", type_line, &text, power, toughness
            ));
            assert!(!issue_313_card_surface_is_exact(
                oracle_id,
                name,
                mana,
                type_line,
                "changed complete Oracle surface",
                power,
                toughness
            ));
        }
        assert!(issue_313_card_surface_is_exact(
            "unreviewed",
            "Unreviewed",
            "{1}",
            "Artifact",
            "anything",
            None,
            None
        ));
    }

    #[test]
    fn issue_314_activated_draw_two_recipe_is_exact_and_identity_bound() {
        let mut mystic = context();
        mystic.source_name = "Mystic Archaeologist".into();
        let matched = match_clause("{3}{U}{U}: Draw two cards.", false, &mystic)
            .expect("Mystic recipe must not be ambiguous")
            .expect("Mystic recipe must match");
        assert_eq!(matched.id.as_str(), "activated.creature.pay_mana.draw_two");
        let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
            panic!("Mystic recipe must emit an activated ability");
        };
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(ability.timing, ActivationTiming::Normal);
        assert_eq!(
            ability.costs,
            [AbilityCost::Mana(ManaCost::parse("{3}{U}{U}").unwrap())]
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            }]
        );
        assert!(ability.targeting.is_none());
        assert!(ability.conditions.is_empty());
        assert!(ability.activation_limit.is_none());

        let mut osc = context();
        osc.source_name = "Oscorp Research Team".into();
        osc.oracle_id = Some(ISSUE_314_OSCORP_ORACLE_ID.into());
        let matched = match_clause("{6}{U}: Draw two cards.", false, &osc)
            .expect("Oscorp recipe must not be ambiguous")
            .expect("Oscorp recipe must match");
        let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
            panic!("Oscorp recipe must emit an activated ability");
        };
        assert_eq!(
            ability.costs,
            [AbilityCost::Mana(ManaCost::parse("{6}{U}").unwrap())]
        );
        let oscrop_swapped_cost = osc.clone();
        assert_eq!(
            match_clause("{3}{U}{U}: Draw two cards.", false, &oscrop_swapped_cost),
            Ok(None),
            "Oscorp must reject Mystic's activation cost"
        );
        assert_eq!(
            match_clause("{3}{U}{U}: Draw two cards.", true, &mystic),
            Ok(None),
            "the creature recipe must not collide with spell surfaces"
        );

        let mut unreviewed = mystic.clone();
        unreviewed.oracle_id = Some("00000000-0000-0000-0000-000000000000".into());
        assert_eq!(
            match_clause("{3}{U}{U}: Draw two cards.", false, &unreviewed),
            Ok(None),
            "an unreviewed Oracle ID must fail closed"
        );
        let mut mismatched_name = mystic.clone();
        mismatched_name.oracle_id = Some(ISSUE_314_MYSTIC_ORACLE_ID.into());
        mismatched_name.source_name = "Oscorp Research Team".into();
        assert_eq!(
            match_clause("{3}{U}{U}: Draw two cards.", false, &mismatched_name),
            Ok(None),
            "an Oracle ID and name from different identities must not mix"
        );
        let mut noncreature = mystic.clone();
        noncreature.source_is_creature = false;
        assert_eq!(
            match_clause("{3}{U}{U}: Draw two cards.", false, &noncreature),
            Ok(None),
            "the recipe is bound to creature sources"
        );
        for near_miss in [
            "{3}{U}: Draw two cards.",
            "{3}{U}{U}{U}: Draw two cards.",
            "{6}{U}: Draw two cards.",
            "{3}{U}{U}: Draw one card.",
            "{3}{U}{U}: Draw three cards.",
            "{3}{U}{U}: Target player draws two cards.",
            "{3}{U}{U}: You may draw two cards.",
            "{3}{U}{U}: Draw two cards, then discard a card.",
            "{3}{U}{U}, {T}: Draw two cards.",
            "{3}{U}{U}: Draw two cards. Activate only as a sorcery.",
        ] {
            assert_eq!(
                match_clause(near_miss, false, &mystic),
                Ok(None),
                "near-miss must fail closed: {near_miss}"
            );
        }
        assert!(issue_314_card_surface_is_exact(
            ISSUE_314_MYSTIC_ORACLE_ID,
            "Mystic Archaeologist",
            "{1}{U}",
            "Creature — Human Wizard",
            "{3}{U}{U}: Draw two cards.",
            Some("2"),
            Some("1"),
        ));
        assert!(issue_314_card_surface_is_exact(
            ISSUE_314_OSCORP_ORACLE_ID,
            "Oscorp Research Team",
            "{3}{U}",
            "Creature — Human Scientist",
            "{6}{U}: Draw two cards.",
            Some("1"),
            Some("5"),
        ));
        assert!(!issue_314_card_surface_is_exact(
            ISSUE_314_OSCORP_ORACLE_ID,
            "Oscorp Research Team",
            "{3}{U}",
            "Creature — Human Scientist",
            "{6}{U}: Draw two cards.",
            Some("3"),
            Some("4"),
        ));
    }

    #[test]
    fn issue_315_creature_tapper_recipe_matches_the_reviewed_cohort() {
        const RECIPE_ID: &str = "activated.creature.pay_mana_tap.tap_creature";
        for (oracle_id, name, clause, activation_cost, expected_filter) in [
            (
                "00d1596a-c3e2-4109-86da-388934a0c652",
                "Coeurl",
                "{1}{W}, {T}: Tap target nonenchantment creature.",
                "{1}{W}",
                TargetFilter {
                    kind: TargetKind::Creature,
                    excluded_permanent_types: vec![PermanentTypeFilter::Enchantment],
                    ..TargetFilter::default()
                },
            ),
            (
                "515c1604-59f0-45b4-91be-d2f20fdd3e1e",
                "Frostbridge Guard",
                "{2}{W}, {T}: Tap target creature.",
                "{2}{W}",
                TargetFilter {
                    kind: TargetKind::Creature,
                    ..TargetFilter::default()
                },
            ),
            (
                "f893d3d6-efef-4394-8e15-e01deed72b4f",
                "Sterling Keykeeper",
                "{2}, {T}: Tap target non-Mount creature.",
                "{2}",
                TargetFilter {
                    kind: TargetKind::Creature,
                    excluded_subtypes: vec!["Mount".into()],
                    ..TargetFilter::default()
                },
            ),
        ] {
            let mut reviewed = context();
            reviewed.oracle_id = Some(oracle_id.into());
            reviewed.source_name = name.into();
            let matched = match_clause(clause, false, &reviewed)
                .expect("reviewed tapper clause must not be ambiguous")
                .expect("reviewed tapper clause must match");
            assert_eq!(matched.id.as_str(), RECIPE_ID);
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("reviewed tapper clause must emit an activated ability");
            };
            assert_eq!(
                ability.costs,
                vec![
                    AbilityCost::Mana(ManaCost::parse(activation_cost).unwrap()),
                    AbilityCost::Tap,
                ]
            );
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::Tap {
                    subject: EffectSubject::Chosen(Box::new(expected_filter)),
                }]
            );
            let targeting = ability
                .targeting
                .expect("reviewed tapper must target one creature");
            assert_eq!(targeting.groups.len(), 1);
            assert_eq!((targeting.groups[0].min, targeting.groups[0].max), (1, 1));
            assert_eq!(targeting.groups[0].effect_indices, vec![0]);
            assert!(targeting.groups[0].distinct_from.is_empty());
            assert!(!targeting.groups[0].same_graveyard);
            assert!(targeting.groups[0].cast_cost_expansion.is_none());
        }

        let mut unreviewed = context();
        unreviewed.source_name = "Coeurl".into();
        unreviewed.oracle_id = Some("00000000-0000-0000-0000-000000000000".into());
        assert!(
            match_clause(
                "{1}{W}, {T}: Tap target nonenchantment creature.",
                false,
                &unreviewed
            )
            .expect("unreviewed identity must not be ambiguous")
            .is_none(),
            "an unknown Oracle identity must not borrow the exact Coeurl clause"
        );

        let mut noncreature = context();
        noncreature.source_name = "Coeurl".into();
        noncreature.oracle_id = Some(ISSUE_315_COEURL_ORACLE_ID.into());
        noncreature.source_is_creature = false;
        assert!(
            match_clause(
                "{1}{W}, {T}: Tap target nonenchantment creature.",
                false,
                &noncreature
            )
            .expect("noncreature source must not be ambiguous")
            .is_none(),
            "the recipe is creature-source-only"
        );

        let mut nonpermanent = context();
        nonpermanent.source_name = "Coeurl".into();
        nonpermanent.oracle_id = Some(ISSUE_315_COEURL_ORACLE_ID.into());
        nonpermanent.source_is_permanent = false;
        assert!(
            match_clause(
                "{1}{W}, {T}: Tap target nonenchantment creature.",
                false,
                &nonpermanent
            )
            .expect("nonpermanent source must not be ambiguous")
            .is_none(),
            "the recipe is battlefield-permanent-only"
        );

        let mut reviewed = context();
        reviewed.source_name = "Coeurl".into();
        reviewed.oracle_id = Some(ISSUE_315_COEURL_ORACLE_ID.into());
        for negative in [
            "{1}{W}: Tap target nonenchantment creature.",
            "{1}{W}, {T}: Untap target nonenchantment creature.",
            "{1}{W}, {T}: Tap target nonenchantment creature or player.",
            "{1}{W}, {T}: Tap up to one target nonenchantment creature.",
            "{1}{W}, {T}: Tap two target nonenchantment creatures.",
            "{1}{W}, {T}: Tap target nonenchantment creature you control.",
            "{1}{W}, {T}, Sacrifice this creature: Tap target nonenchantment creature.",
            "{1}{W}, {T}, Untap this creature: Tap target nonenchantment creature.",
            "{1}{W}, {T}, Discard a card: Tap target nonenchantment creature.",
            "{1}{W}, {T}, Pay 1 life: Tap target nonenchantment creature.",
            "{1}{W}, {T}: Destroy target nonenchantment creature.",
            "{1}{W}, {T}: Exile target nonenchantment creature.",
            "{1}{W}, {T}: Tap target noncreature permanent.",
            "{1}{W}, {T}: Tap target artifact creature.",
            "{1}{W}, {T}: Tap target nonenchantment creature. Activate only as a sorcery.",
            "{1}{W}, {T}: Tap target nonenchantment creature. Activate only once each turn.",
            "{1}{W}, {T}: Tap target nonenchantment creature. Activate only if you control an artifact.",
            "{1}{W}, {T}: Tap target nonenchantment creature. Activate only from your hand.",
            "{2}{W}, {T}: Tap target creature.",
            "{2}, {T}: Tap target creature.",
            "{2}, {T}: Tap target Mount creature.",
            "{2}, {T}: Tap target non-Vehicle creature.",
            "{2}, {T}: Tap target creature you control.",
            "{2}, Tap this creature: Tap target non-Mount creature.",
            "{2}, {Q}: Tap target non-Mount creature.",
            "{2}, {T}: Tap target non-Mount creature, then draw a card.",
            "{2}, {T}: Tap target non-Mount creature. Untap this creature.",
            "{2}, {T}, Sacrifice this creature: Tap target non-Mount creature.",
        ] {
            assert!(
                match_clause(negative, false, &reviewed)
                    .expect("negative tapper clause must not be ambiguous")
                    .is_none(),
                "unsupported tapper near-miss must remain unmatched: {negative}"
            );
        }
    }

    #[test]
    fn issue_289_discard_batch_recipe_is_exact_and_allowlisted() {
        let clause = ISSUE_289_DISCARD_BATCH_CLAUSE;
        for (name, oracle_id) in [
            ("Scrounging Skyray", ISSUE_289_SKYRAY_ORACLE_ID),
            ("Marauding Mako", ISSUE_289_MAKO_ORACLE_ID),
        ] {
            let mut reviewed = context();
            reviewed.source_name = name.into();
            reviewed.oracle_id = Some(oracle_id.into());
            let matched = match_clause(clause, false, &reviewed)
                .expect("reviewed discard-batch clause must not be ambiguous")
                .expect("reviewed discard-batch clause must match");
            assert_eq!(
                matched.id.as_str(),
                "triggered.discard.one_or_more.plus_one_counters_source"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("discard-batch clause must emit one triggered ability")
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverPlayerDiscardsOneOrMoreCards {
                    player: CastTriggerPlayer::Controller,
                }
            );
            assert_eq!(
                ability.effect,
                [SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::EventCount,
                    subject: EffectSubject::Source,
                }]
            );
        }

        let mut unreviewed = context();
        unreviewed.source_name = "Unreviewed Batcher".into();
        unreviewed.oracle_id = Some("00000000-0000-0000-0000-000000000000".into());
        assert!(
            match_clause(clause, false, &unreviewed)
                .expect("unreviewed identity must not be ambiguous")
                .is_none(),
            "an unknown Oracle identity must not borrow the cohort clause"
        );

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(clause, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the recipe is creature-source-only"
        );

        for negative in [
            "Whenever an opponent discards one or more cards, put that many +1/+1 counters on this creature.",
            "Whenever you discard a card, put a +1/+1 counter on this creature.",
            "Whenever you discard one or more cards, put that many +1/+1 counters on target creature.",
            "Whenever you discard one or more creature cards, put that many +1/+1 counters on this creature.",
            "Whenever you discard one or more cards, draw that many cards.",
            "Whenever you discard one or more cards, put that many +1/+1 counters on this creature. This ability triggers only once each turn.",
            "Whenever you discard one or more cards, put that many +1/+1 counters on this creature",
        ] {
            assert!(
                match_clause(negative, false, &context())
                    .expect("discard-batch near-miss must not be ambiguous")
                    .is_none(),
                "unsupported discard-batch near-miss must remain unmatched: {negative}"
            );
        }
    }

    fn issue_316_recipe(id: &str) -> &'static Recipe {
        CATALOG
            .iter()
            .find(|recipe| recipe.id.as_str() == id)
            .unwrap_or_else(|| panic!("missing recipe {id}"))
    }

    fn issue_316_match_spell(clause: &str, source_name: &str) -> RecipeMatch {
        let mut reviewed = context();
        reviewed.source_name = source_name.into();
        match_clause(clause, true, &reviewed)
            .unwrap_or_else(|ambiguity| panic!("{source_name} is ambiguous: {ambiguity}"))
            .unwrap_or_else(|| panic!("{source_name} should match exactly one recipe"))
    }

    fn issue_316_match_non_spell(clause: &str, source_name: &str) -> RecipeMatch {
        let mut reviewed = context();
        reviewed.source_name = source_name.into();
        match_clause(clause, false, &reviewed)
            .unwrap_or_else(|ambiguity| panic!("{source_name} is ambiguous: {ambiguity}"))
            .unwrap_or_else(|| panic!("{source_name} should match exactly one recipe"))
    }

    fn issue_316_assert_unmatched(clause: &str, is_spell: bool) {
        assert!(
            match_clause(clause, is_spell, &context())
                .unwrap_or_else(|ambiguity| {
                    panic!("near-miss must not be ambiguous: {clause}: {ambiguity}")
                })
                .is_none(),
            "unsupported near-miss must remain unmatched: {clause}"
        );
    }

    #[test]
    fn issue_316_recipes_have_stable_ids_and_surfaces() {
        for (id, surface) in [
            (
                "spell.destroy.target_tapped_creature",
                RecipeSurface::SpellClause,
            ),
            (
                "triggered.etb.destroy_up_to_one_artifact_or_enchantment",
                RecipeSurface::EtbAbility,
            ),
            (
                "triggered.etb.exile_up_to_two_target_cards_from_a_single_graveyard",
                RecipeSurface::EtbAbility,
            ),
            (
                "triggered.controller_casts_noncreature.ping_each_opponent_one",
                RecipeSurface::TriggeredAbility,
            ),
            (
                "spell.pump.target_creature_plus_n_zero_first_strike",
                RecipeSurface::SpellClause,
            ),
            (
                "spell.destroy.target_creature_power_n_or_less",
                RecipeSurface::SpellClause,
            ),
        ] {
            assert_eq!(
                issue_316_recipe(id).surface,
                surface,
                "{id} surface drifted"
            );
        }
    }

    #[test]
    fn issue_316_tapped_creature_destroy_is_exact_and_typed() {
        for source_name in ["Push", "Rip the Seams"] {
            let matched = issue_316_match_spell(ISSUE_316_TAPPED_CREATURE_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "spell.destroy.target_tapped_creature");
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("tapped-creature destroy must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![SpellEffectKind::Destroy {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::Creature,
                        tapped: Some(true),
                        ..TargetFilter::default()
                    })),
                }]
            );
            assert_eq!(targeting.groups.len(), 1);
            let group = &targeting.groups[0];
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target tapped creature");
            assert_eq!(group.effect_indices, vec![0]);
            assert!(group.distinct_from.is_empty());
            assert!(!group.same_graveyard);
            assert!(group.cast_cost_expansion.is_none());
        }

        let plain = issue_316_match_spell("Destroy target creature.", "Near Miss");
        assert_eq!(plain.id.as_str(), "spell.destroy.creature");
    }

    #[test]
    fn issue_316_tapped_creature_destroy_rejects_near_misses() {
        for negative in [
            "Destroy target untapped creature.",
            "Destroy target tapped artifact or creature.",
            "Destroy target tapped artifact.",
            "Destroy up to one target tapped creature.",
            "Destroy target tapped creature. You gain 1 life.",
            "Destroy target tapped creature. You gain 2 life.",
            "Destroy target tapped creature",
            "Destroy target tapped permanent.",
            "Destroy two target tapped creatures.",
            "Destroy target tapped creature you control.",
        ] {
            issue_316_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_316_etb_artifact_or_enchantment_destroy_is_creature_gated() {
        for source_name in ["Chomping Changeling", "Disruptive Stormbrood"] {
            let matched = issue_316_match_non_spell(
                ISSUE_316_ARTIFACT_OR_ENCHANTMENT_ETB_CLAUSE,
                source_name,
            );
            assert_eq!(
                matched.id.as_str(),
                "triggered.etb.destroy_up_to_one_artifact_or_enchantment"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("artifact-or-enchantment ETB must emit one triggered ability");
            };
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert!(!ability.may);
            assert!(ability.intervening_if.is_none());
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::Destroy {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        permanent_types: vec![
                            PermanentTypeFilter::Artifact,
                            PermanentTypeFilter::Enchantment,
                        ],
                        ..TargetFilter::default()
                    })),
                }]
            );
            let targeting = ability.targeting.expect("ETB must target optionally");
            assert_eq!(targeting.groups.len(), 1);
            let group = &targeting.groups[0];
            assert_eq!((group.min, group.max), (0, 1));
            assert_eq!(
                group.prompt,
                "Choose up to one target artifact or enchantment"
            );
            assert_eq!(group.effect_indices, vec![0]);
            assert!(!group.same_graveyard);
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(
                ISSUE_316_ARTIFACT_OR_ENCHANTMENT_ETB_CLAUSE,
                false,
                &noncreature
            )
            .expect("noncreature source must not be ambiguous")
            .is_none(),
            "the ETB destroy recipe is creature-source-only"
        );
    }

    #[test]
    fn issue_316_etb_artifact_or_enchantment_destroy_rejects_near_misses() {
        for negative in [
            "Destroy target artifact or enchantment.",
            "Destroy up to one target artifact or enchantment.",
            "Destroy up to one target artifact.",
            "Destroy up to one target enchantment.",
            "Destroy up to one target creature or enchantment.",
            "Destroy up to one target enchantment or artifact.",
            "When this artifact enters, destroy up to one target artifact or enchantment.",
            "When this creature enters, destroy target artifact or enchantment.",
            "When this creature enters, destroy up to two target artifacts or enchantments.",
            "When this creature enters, you may destroy up to one target artifact or enchantment.",
            "When this creature enters, destroy up to one target artifact or enchantment. You gain 1 life.",
            "Whenever this creature enters, destroy up to one target artifact or enchantment.",
        ] {
            issue_316_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_316_etb_single_graveyard_exile_is_creature_gated() {
        for source_name in ["Griffnaut Tracker", "Feral Deathgorger"] {
            let matched =
                issue_316_match_non_spell(ISSUE_316_SINGLE_GRAVEYARD_EXILE_ETB_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.etb.exile_up_to_two_target_cards_from_a_single_graveyard"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("single-graveyard exile must emit one triggered ability");
            };
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::MoveGraveyardCards {
                    filter: GraveyardFilter {
                        owner: GraveyardOwner::AnyPlayer,
                        ..GraveyardFilter::default()
                    },
                    destination: GraveyardDestination::Exile,
                    linked_exile_id: None,
                }]
            );
            let targeting = ability.targeting.expect("exile must target optionally");
            assert_eq!(targeting.groups.len(), 1);
            let group = &targeting.groups[0];
            assert_eq!((group.min, group.max), (0, 2));
            assert_eq!(
                group.prompt,
                "Choose up to two target cards from a single graveyard"
            );
            assert_eq!(group.effect_indices, vec![0]);
            assert!(group.same_graveyard);
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(
                ISSUE_316_SINGLE_GRAVEYARD_EXILE_ETB_CLAUSE,
                false,
                &noncreature
            )
            .expect("noncreature source must not be ambiguous")
            .is_none(),
            "the graveyard exile recipe is creature-source-only"
        );
    }

    #[test]
    fn issue_316_etb_single_graveyard_exile_rejects_near_misses() {
        for negative in [
            "When this creature enters, exile up to two target cards from a graveyard.",
            "When this creature enters, exile up to one target card from a single graveyard.",
            "When this creature enters, exile up to two target creature cards from a single graveyard.",
            "When this creature enters, exile up to three target cards from a single graveyard.",
            "When this creature enters, exile target card from a single graveyard.",
            "When this creature enters, exile up to two target cards from a single graveyard. If at least one creature card was exiled this way, each opponent loses 2 life and you gain 2 life.",
            "When this creature enters, exile up to two target cards from a single graveyard. You gain 1 life.",
            "Whenever this creature attacks, exile up to two target cards from a single graveyard.",
            "At the beginning of combat on your turn, exile up to two target cards from a single graveyard.",
            "When this creature enters, exile up to two target cards from target opponent's graveyard.",
        ] {
            issue_316_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_316_noncreature_cast_ping_is_creature_gated() {
        for source_name in ["Firebrand Archer", "Coruscation Mage"] {
            let matched =
                issue_316_match_non_spell(ISSUE_316_NONCREATURE_CAST_PING_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.controller_casts_noncreature.ping_each_opponent_one"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("noncreature cast ping must emit one triggered ability");
            };
            assert_eq!(
                ability.trigger,
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
                ability.effect,
                vec![SpellEffectKind::DamagePlayer {
                    amount: Amount::Fixed(1),
                    who: PlayerRecipient::EachOpponent,
                }]
            );
            assert!(ability.targeting.is_none());
        }

        let counter = issue_316_match_non_spell(
            "Whenever you cast a noncreature spell, put a +1/+1 counter on this creature.",
            "Near Miss",
        );
        assert_eq!(
            counter.id.as_str(),
            "triggered.controller_casts.noncreature.put_counter.plus_one_plus_one.source.one"
        );

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_316_NONCREATURE_CAST_PING_CLAUSE, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the cast-ping recipe is creature-source-only"
        );
    }

    #[test]
    fn issue_316_noncreature_cast_ping_rejects_near_misses() {
        for negative in [
            "Whenever you cast a spell, this creature deals 1 damage to each opponent.",
            "Whenever an opponent casts a noncreature spell, this creature deals 1 damage to each opponent.",
            "Whenever you cast a creature spell, this creature deals 1 damage to each opponent.",
            "Whenever you cast a noncreature spell, this creature deals 2 damage to each opponent.",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to target opponent.",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to each player.",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to target creature an opponent controls.",
            "Whenever you cast a noncreature spell, this creature deals 1 damage to each opponent. You gain 1 life.",
        ] {
            issue_316_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_316_first_strike_pump_is_parameterized_and_exact() {
        for (source_name, clause, power) in [
            (
                "Kindled Fury",
                "Target creature gets +1/+0 and gains first strike until end of turn.",
                1,
            ),
            (
                "Sure Strike",
                "Target creature gets +3/+0 and gains first strike until end of turn.",
                3,
            ),
        ] {
            let matched = issue_316_match_spell(clause, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.pump.target_creature_plus_n_zero_first_strike"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("first-strike pump must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![
                    SpellEffectKind::PumpTarget {
                        power,
                        toughness: 0,
                        scale: None,
                        subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                        keywords: vec![Keyword::FirstStrike],
                    },
                ]
            );
            assert_eq!(targeting.groups.len(), 1);
            let group = &targeting.groups[0];
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target creature");
            assert_eq!(group.effect_indices, vec![0, 1]);
            assert!(!group.same_graveyard);
        }
    }

    #[test]
    fn issue_316_first_strike_pump_rejects_near_misses() {
        for negative in [
            "Target creature gets +0/+0 and gains first strike until end of turn.",
            "Target creature gets +0/+1 and gains first strike until end of turn.",
            "Target creature gets +1/+1 and gains first strike until end of turn.",
            "Target creature gets +2/+2 and gains first strike until end of turn.",
            "Target creature gets +1/+0 and gains trample until end of turn.",
            "Target creature gets +1/+0 and gains double strike until end of turn.",
            "Target creature gets +1/+0 and gains first strike until end of combat.",
            "Target creature gets +1/+0 and gains first strike until your next turn.",
            "Target creature you control gets +1/+0 and gains first strike until end of turn.",
            "Creatures you control get +1/+0 and gain first strike until end of turn.",
            "Up to one target creature gets +1/+0 and gains first strike until end of turn.",
            "Target creature gets +1/+0 and gains first strike until end of turn. Investigate.",
            "Target creature gets +1/+0 and gains first strike until end of turn. If this spell was kicked, that creature gains trample until end of turn.",
        ] {
            issue_316_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_316_power_bounded_destroy_is_parameterized_and_exact() {
        for (source_name, clause, bound) in [
            ("Defeat", "Destroy target creature with power 2 or less.", 2),
            (
                "Reave Soul",
                "Destroy target creature with power 3 or less.",
                3,
            ),
            (
                "Petty Revenge",
                "Destroy target creature with power 3 or less.",
                3,
            ),
        ] {
            let matched = issue_316_match_spell(clause, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.destroy.target_creature_power_n_or_less"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("power-bounded destroy must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![SpellEffectKind::Destroy {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::Creature,
                        power: Some(PowerComparison::AtMost(bound)),
                        ..TargetFilter::default()
                    })),
                }]
            );
            assert_eq!(targeting.groups.len(), 1);
            let group = &targeting.groups[0];
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(
                group.prompt,
                format!("Choose target creature with power {bound} or less")
            );
            assert_eq!(group.effect_indices, vec![0]);
        }

        let plain = issue_316_match_spell("Destroy target creature.", "Near Miss");
        assert_eq!(plain.id.as_str(), "spell.destroy.creature");
    }

    #[test]
    fn issue_316_power_bounded_destroy_rejects_near_misses() {
        for negative in [
            "Destroy target creature with power 3 or greater.",
            "Destroy target creature with power 2 or greater.",
            "Destroy target creature with power 0 or less.",
            "Destroy target creature with power three or less.",
            "Destroy target creature with power 3 or less",
            "Destroy up to one target creature with power 3 or less.",
            "Destroy target creature with toughness 3 or less.",
            "Destroy target creature with toughness 4 or greater.",
            "Destroy target artifact creature with power 3 or less.",
            "Destroy two target creatures with power 3 or less.",
            "Destroy target creature with power 3 or less. You gain 1 life.",
        ] {
            issue_316_assert_unmatched(negative, true);
        }
    }

    fn issue_317_recipe(id: &str) -> &'static Recipe {
        CATALOG
            .iter()
            .find(|recipe| recipe.id.as_str() == id)
            .unwrap_or_else(|| panic!("missing recipe {id}"))
    }

    fn issue_317_match_spell(clause: &str, source_name: &str) -> RecipeMatch {
        let mut source = context();
        source.source_name = source_name.into();
        match_clause(clause, true, &source)
            .unwrap_or_else(|ambiguity| panic!("{source_name} is ambiguous: {ambiguity}"))
            .unwrap_or_else(|| panic!("{source_name} should match exactly one recipe"))
    }

    fn issue_317_match_non_spell(clause: &str, source_name: &str) -> RecipeMatch {
        let mut source = context();
        source.source_name = source_name.into();
        match_clause(clause, false, &source)
            .unwrap_or_else(|ambiguity| panic!("{source_name} is ambiguous: {ambiguity}"))
            .unwrap_or_else(|| panic!("{source_name} should match exactly one recipe"))
    }

    fn issue_317_assert_unmatched(clause: &str, is_spell: bool) {
        assert!(
            match_clause(clause, is_spell, &context())
                .unwrap_or_else(|ambiguity| {
                    panic!("near-miss must not be ambiguous: {clause}: {ambiguity}")
                })
                .is_none(),
            "unsupported near-miss must remain unmatched: {clause}"
        );
    }

    #[test]
    fn issue_317_recipes_have_stable_ids_and_surfaces() {
        for (id, surface) in [
            (
                "triggered.self_attacks.vehicle.create_treasure",
                RecipeSurface::TriggeredAbility,
            ),
            (
                "spell.return_graveyard.creature.cards.hand.up_to_two",
                RecipeSurface::SpellClause,
            ),
            (
                "activated.tap_creature.add_any_color",
                RecipeSurface::ActivatedAbility,
            ),
            (
                "static.self.cannot_be_blocked_by_power_n_or_less",
                RecipeSurface::StaticAbility,
            ),
            ("spell.create_clue_token", RecipeSurface::SpellClause),
            (
                "activated.graveyard.return_self_to_hand",
                RecipeSurface::ZoneActivatedAbility,
            ),
        ] {
            assert_eq!(
                issue_317_recipe(id).surface,
                surface,
                "{id} surface drifted"
            );
        }
    }

    #[test]
    fn issue_317_vehicle_attack_treasure_is_vehicle_gated() {
        for source_name in ["Careening Mine Cart", "Rocketeer Boostbuggy"] {
            let matched =
                issue_317_match_non_spell(ISSUE_317_VEHICLE_ATTACK_TREASURE_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.self_attacks.vehicle.create_treasure"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("vehicle attack Treasure must emit one triggered ability");
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverSelfAttacks {
                    minimum_other_attackers: 0,
                }
            );
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::CreateTokens {
                    token: "treasure".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }]
            );
            assert!(ability.targeting.is_none());
            assert!(!ability.may);
            assert!(ability.intervening_if.is_none());
        }

        let mut nonvehicle = context();
        nonvehicle.source_is_vehicle = false;
        assert!(
            match_clause(ISSUE_317_VEHICLE_ATTACK_TREASURE_CLAUSE, false, &nonvehicle)
                .expect("nonvehicle source must not be ambiguous")
                .is_none(),
            "the attack Treasure recipe is Vehicle-source-only"
        );
    }

    #[test]
    fn issue_317_vehicle_attack_treasure_rejects_near_misses() {
        for negative in [
            "Whenever this creature attacks, create a Treasure token.",
            "Whenever another Vehicle you control attacks, create a Treasure token.",
            "Whenever this Vehicle attacks, you may create a Treasure token.",
            "Whenever this Vehicle deals combat damage to a player, create a Treasure token.",
            "Whenever this Vehicle attacks, create two Treasure tokens.",
            "Whenever this Vehicle attacks, create a tapped Treasure token.",
            "Whenever this Vehicle attacks, create a Treasure token. Draw a card.",
            "Whenever this Vehicle attacks, create a Treasure token",
        ] {
            issue_317_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_317_return_up_to_two_graveyard_creatures_is_exact_and_typed() {
        for source_name in ["Fight On!", "Macabre Reconstruction", "Sanguine Indulgence"] {
            let matched = issue_317_match_spell(
                ISSUE_317_RETURN_UP_TO_TWO_GRAVEYARD_CREATURES_CLAUSE,
                source_name,
            );
            assert_eq!(
                matched.id.as_str(),
                "spell.return_graveyard.creature.cards.hand.up_to_two"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("graveyard return must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![SpellEffectKind::MoveGraveyardCards {
                    filter: GraveyardFilter {
                        owner: GraveyardOwner::Controller,
                        card: Some(graveyard_creature_card_filter()),
                        ..GraveyardFilter::default()
                    },
                    destination: GraveyardDestination::Hand,
                    linked_exile_id: None,
                }]
            );
            let [group] = targeting.groups.as_slice() else {
                panic!("graveyard return must have exactly one target group");
            };
            assert_eq!((group.min, group.max), (0, 2));
            assert_eq!(
                group.prompt,
                "Choose up to two target creature cards from your graveyard"
            );
            assert_eq!(group.effect_indices, vec![0]);
            assert!(group.distinct_from.is_empty());
            assert!(!group.same_graveyard);
        }

        let plain = issue_317_match_spell(
            "Return target card from your graveyard to your hand.",
            "Near Miss",
        );
        assert_eq!(plain.id.as_str(), "spell.return_graveyard_card.hand");
    }

    #[test]
    fn issue_317_return_up_to_two_graveyard_creatures_rejects_near_misses() {
        for negative in [
            "Return target creature card from your graveyard to your hand.",
            "Return up to one target creature card from your graveyard to your hand.",
            "Return up to two target cards from your graveyard to your hand.",
            "Return up to two target creature cards from a graveyard to your hand.",
            "Return up to two target creature cards from your graveyard to the battlefield.",
            "Return up to two target creature cards from an opponent's graveyard to your hand.",
            "Return up to three target creature cards from your graveyard to your hand.",
            "Return up to two target creature cards from your graveyard to your hand. You gain 2 life.",
            "Return two target creature cards from your graveyard to your hand.",
        ] {
            issue_317_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_317_tap_creature_any_color_is_artifact_gated() {
        for source_name in [
            "Springleaf Drum",
            "Dragonbroods' Relic",
            "Scene of the Crime",
        ] {
            let matched =
                issue_317_match_non_spell(ISSUE_317_TAP_CREATURE_ANY_COLOR_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "activated.tap_creature.add_any_color");
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("tap-creature any-color must emit one activated ability");
            };
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(
                ability.costs,
                vec![
                    AbilityCost::Tap,
                    AbilityCost::TapPermanents {
                        constraint: ObjectPaymentConstraint::ExactCount(1),
                        filter: TargetFilter {
                            kind: TargetKind::Creature,
                            controller: TargetController::You,
                            ..TargetFilter::default()
                        },
                        exclude_source: true,
                    },
                ]
            );
            assert_eq!(ability.effect, vec![five_color_mana_effect()]);
            assert!(ability.targeting.is_none());
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert!(ability.activation_limit.is_none());
        }

        let plain = issue_317_match_non_spell("{T}: Add one mana of any color.", "Near Miss");
        assert_eq!(plain.id.as_str(), "activated.mana.tap_any_color");

        let mut nonartifact = context();
        nonartifact.source_is_artifact = false;
        assert!(
            match_clause(ISSUE_317_TAP_CREATURE_ANY_COLOR_CLAUSE, false, &nonartifact)
                .expect("nonartifact source must not be ambiguous")
                .is_none(),
            "the tap-creature mana recipe is Artifact-source-only"
        );
    }

    #[test]
    fn issue_317_tap_creature_any_color_rejects_near_misses() {
        for negative in [
            "{T}, Tap an untapped artifact you control: Add one mana of any color.",
            "{T}, Tap two untapped creatures you control: Add one mana of any color.",
            "{T}, Tap an untapped creature you control: Add {G}.",
            "{T}, Tap an untapped creature you control: Add one mana of any one color.",
            "{T}, Tap an untapped creature you control: Add one mana of any type.",
            "{T}, Exile a creature you control: Add one mana of any color.",
            "{T}, Pay 1 life, Tap an untapped creature you control: Add one mana of any color.",
            "{T}, Tap an untapped creature you control: Add one mana of any color. Draw a card.",
            "{T}, Tap an untapped creature an opponent controls: Add one mana of any color.",
        ] {
            issue_317_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_317_power_bounded_self_block_restriction_is_creature_gated() {
        for (source_name, bound) in [
            ("Stormkeld Vanguard", 2u32),
            ("Gate Colossus", 2),
            ("Bristlebane Outrider", 2),
            ("Old Fat Spider", 2),
        ] {
            let matched = issue_317_match_non_spell(
                &format!("This creature can't be blocked by creatures with power {bound} or less."),
                source_name,
            );
            assert_eq!(
                matched.id.as_str(),
                "static.self.cannot_be_blocked_by_power_n_or_less"
            );
            let RecipeEmission::StaticAbility(ability) = matched.emission else {
                panic!("power-bounded self block restriction must emit one static ability");
            };
            assert_eq!(
                ability.definition,
                StaticAbilityDef::SelfCombatRestriction {
                    restriction: tricerules_cards::primitives::CombatRestriction {
                        cant_be_blocked_by: vec![TargetFilter {
                            kind: TargetKind::Creature,
                            power: Some(PowerComparison::AtMost(bound)),
                            ..TargetFilter::default()
                        }],
                        ..tricerules_cards::primitives::CombatRestriction::default()
                    },
                    condition: None,
                }
            );
        }

        let one = issue_317_match_non_spell(
            "This creature can't be blocked by creatures with power 1 or less.",
            "Near Miss",
        );
        assert_eq!(
            one.id.as_str(),
            "static.self.cannot_be_blocked_by_power_n_or_less"
        );

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(
                "This creature can't be blocked by creatures with power 2 or less.",
                false,
                &noncreature,
            )
            .expect("noncreature source must not be ambiguous")
            .is_none(),
            "the block-restriction recipe is creature-source-only"
        );
    }

    #[test]
    fn issue_317_power_bounded_self_block_restriction_rejects_near_misses() {
        for negative in [
            "This creature can't be blocked.",
            "This creature can't be blocked by creatures with power 2 or greater.",
            "This creature can't be blocked by creatures with power 0 or less.",
            "This creature can't be blocked by Walls.",
            "This creature can't be blocked except by creatures with power 2 or greater.",
            "This creature can't be blocked by creatures with power 2.",
            "This creature can block only creatures with power 2 or greater.",
            "This creature can't be blocked by creatures with power two or less.",
            "This creature can't be blocked by creatures with toughness 2 or less.",
            "This creature can't be blocked by artifacts with power 2 or less.",
            "This creature can't be blocked by creatures with power 2 or less. It can't be blocked by Walls.",
        ] {
            issue_317_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_317_spell_create_clue_token_is_exact_and_typed() {
        for source_name in ["Cunning Maneuver", "True Ancestry", "Jet's Brainwashing"] {
            let matched = issue_317_match_spell(ISSUE_317_CREATE_CLUE_TOKEN_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "spell.create_clue_token");
            assert_eq!(
                matched.emission,
                RecipeEmission::SpellEffect(SpellEffectKind::CreateTokens {
                    token: "clue".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                })
            );
        }
    }

    #[test]
    fn issue_317_spell_create_clue_token_rejects_near_misses() {
        for negative in [
            "Create a Clue token. Draw a card.",
            "Create two Clue tokens.",
            "Create a tapped Clue token.",
            "Investigate.",
            "Create a Food token.",
            "Create a tapped Treasure token.",
            "Create a Clue token",
            "Create a colorless Clue token.",
            "Create a 1/1 Clue token.",
            "Create a Clue artifact token.",
        ] {
            issue_317_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_317_graveyard_return_self_to_hand_is_typed() {
        for source_name in ["Project Deathlok Soldier", "Abzan Devotee"] {
            let matched = issue_317_match_non_spell(
                ISSUE_317_GRAVEYARD_RETURN_SELF_TO_HAND_CLAUSE,
                source_name,
            );
            assert_eq!(
                matched.id.as_str(),
                "activated.graveyard.return_self_to_hand"
            );
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("graveyard return must emit one activated ability");
            };
            assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
            assert_eq!(
                ability.costs,
                vec![AbilityCost::Mana(
                    ManaCost::parse(ISSUE_317_GRAVEYARD_RETURN_SELF_COST)
                        .expect("fixture mana cost")
                )]
            );
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::ReturnToOwnersHand {
                    subject: EffectSubject::Source,
                }]
            );
            assert!(ability.targeting.is_none());
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert!(ability.activation_limit.is_none());
        }

        let other_cost = issue_317_match_non_spell(
            "{B}: Return this card from your graveyard to your hand.",
            "Near Miss",
        );
        let RecipeEmission::ActivatedAbility(ability) = other_cost.emission else {
            panic!("graveyard return must emit one activated ability");
        };
        assert_eq!(
            ability.costs,
            vec![AbilityCost::Mana(
                ManaCost::parse("{B}").expect("fixture cost")
            )]
        );
    }

    #[test]
    fn issue_317_graveyard_return_self_to_hand_rejects_near_misses() {
        for negative in [
            "{2}{B}: Return this card from your graveyard to the battlefield.",
            "{2}{B}: Return this card from your graveyard to the battlefield tapped.",
            "{2}{B}: Return target creature card from your graveyard to your hand.",
            "{2}{B}: Return this card from your graveyard to your hand. Activate only as a sorcery.",
            "{2}{B}, Exile this card from your graveyard: Return this card from your graveyard to your hand.",
            "{2}{B}: Return this card from the graveyard to your hand.",
            "{2}{B}: Return this card from your graveyard to its owner's hand.",
            "{2}{B}: Return this creature card from your graveyard to your hand.",
            "Return this card from your graveyard to your hand.",
            "{X}{B}: Return this card from your graveyard to your hand.",
        ] {
            issue_317_assert_unmatched(negative, false);
        }
    }

    fn issue_318_recipe(id: &str) -> &'static Recipe {
        CATALOG
            .iter()
            .find(|recipe| recipe.id.as_str() == id)
            .unwrap_or_else(|| panic!("missing recipe {id}"))
    }

    fn issue_318_match_spell(clause: &str, source_name: &str) -> RecipeMatch {
        let mut source = context();
        source.source_name = source_name.into();
        match_clause(clause, true, &source)
            .unwrap_or_else(|ambiguity| panic!("{source_name} is ambiguous: {ambiguity}"))
            .unwrap_or_else(|| panic!("{source_name} should match exactly one recipe"))
    }

    fn issue_318_match_non_spell(clause: &str, source_name: &str) -> RecipeMatch {
        let mut source = context();
        source.source_name = source_name.into();
        match_clause(clause, false, &source)
            .unwrap_or_else(|ambiguity| panic!("{source_name} is ambiguous: {ambiguity}"))
            .unwrap_or_else(|| panic!("{source_name} should match exactly one recipe"))
    }

    fn issue_318_assert_unmatched(clause: &str, is_spell: bool) {
        assert!(
            match_clause(clause, is_spell, &context())
                .unwrap_or_else(|ambiguity| {
                    panic!("near-miss must not be ambiguous: {clause}: {ambiguity}")
                })
                .is_none(),
            "unsupported near-miss must remain unmatched: {clause}"
        );
    }

    #[test]
    fn issue_318_recipes_have_stable_ids_and_surfaces() {
        for (id, surface) in [
            (
                "spell.put_in_owners_library.owner_choice_top_or_bottom.target_creature",
                RecipeSurface::SpellClause,
            ),
            (
                "spell.cost_reduction.target_match_attacking_creature.one",
                RecipeSurface::SpellClause,
            ),
            (
                "triggered.etb.create_rat_token.cant_block",
                RecipeSurface::EtbAbility,
            ),
            (
                "triggered.etb.target_creature_gains_flying",
                RecipeSurface::EtbAbility,
            ),
            (
                "triggered.self_damage_to_opponent.draw",
                RecipeSurface::TriggeredAbility,
            ),
        ] {
            assert_eq!(
                issue_318_recipe(id).surface,
                surface,
                "{id} surface drifted"
            );
        }
    }

    #[test]
    fn issue_318_owner_choice_placement_is_exact_and_typed() {
        for source_name in [
            "Misleading Motes",
            "Run Behind",
            "Uneasy Partings",
            "Dire Downdraft",
        ] {
            let matched =
                issue_318_match_spell(ISSUE_318_OWNER_CHOICE_PLACEMENT_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.put_in_owners_library.owner_choice_top_or_bottom.target_creature"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("owner-choice placement must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![SpellEffectKind::PutInOwnersLibrary {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    placement: LibraryPlacement::OwnerChoiceTopOrBottom,
                }]
            );
            let [group] = targeting.groups.as_slice() else {
                panic!("owner-choice placement must own exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target creature");
            assert_eq!(group.effect_indices, vec![0]);
            assert!(group.distinct_from.is_empty());
            assert!(!group.same_graveyard);
            assert!(group.cast_cost_expansion.is_none());
        }

        assert!(
            match_clause(ISSUE_318_OWNER_CHOICE_PLACEMENT_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "the placement clause is a spell clause and must not match permanent text"
        );
    }

    #[test]
    fn issue_318_owner_choice_placement_rejects_near_misses() {
        for negative in [
            "Put target creature on the bottom of its owner's library.",
            "Put target creature on top of its owner's library.",
            "Target creature's owner puts it on their choice of the top or bottom of their library. Surveil 1.",
            "Target creature's owner shuffles it into their library.",
            "Target nonland permanent's owner puts it on their choice of the top or bottom of their library.",
            "The owner of target spell or nonland permanent puts it on their choice of the top or bottom of their library.",
            "Target artifact's owner puts it on their choice of the top or bottom of their library.",
            "Target creature's owner puts it on the top or bottom of their library.",
            "Target creature's owner puts it on their choice of the top or bottom of their library",
            "Target creature's owner puts it on their choice of the top or bottom of their graveyard.",
            "Target creature's owner puts it on their choice of the top or bottom of their library. Draw a card.",
            "Put target creature on its owner's choice of the top or bottom of their library.",
            "Target creature's owner puts it on their choice of the second from the top or bottom of their library.",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_318_attacking_target_reduction_is_exact_and_typed() {
        for source_name in ["Guidance Failure", "Run Behind"] {
            let matched =
                issue_318_match_spell(ISSUE_318_ATTACKING_TARGET_REDUCTION_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.cost_reduction.target_match_attacking_creature.one"
            );
            assert_eq!(
                matched.emission,
                RecipeEmission::SpellCostModifier(SpellCostModifier::TargetMatchGenericReduction {
                    amount: 1,
                    filter: TargetMatchFilter::Battlefield(TargetFilter {
                        kind: TargetKind::Creature,
                        combat_role: Some(CombatRole::Attacking),
                        ..TargetFilter::default()
                    }),
                })
            );
        }

        let tapped = issue_318_match_spell(
            "This spell costs {3} less to cast if it targets a tapped creature.",
            "Near Miss",
        );
        assert_eq!(
            tapped.id.as_str(),
            "spell.cost_reduction.target_tapped_creature.three"
        );

        let mut non_spell = context();
        non_spell.source_is_instant = false;
        non_spell.source_is_sorcery = false;
        assert!(
            match_clause(
                ISSUE_318_ATTACKING_TARGET_REDUCTION_CLAUSE,
                true,
                &non_spell
            )
            .expect("non-instant/sorcery source must not be ambiguous")
            .is_none(),
            "the attacking-target reduction is instant/sorcery-only"
        );
    }

    #[test]
    fn issue_318_attacking_target_reduction_rejects_near_misses() {
        for negative in [
            "This spell costs {1} less to cast if it targets a tapped creature.",
            "This spell costs {1} less to cast if it targets an attacking nontoken creature.",
            "This spell costs {1} less to cast if it targets an attacking or tapped creature.",
            "This spell costs {2} less to cast if it targets an attacking creature.",
            "This spell costs {1} less to cast if you control a creature.",
            "This spell costs {1} less to cast.",
            "This spell costs {1} less to cast if it targets an attacking creature. Draw a card.",
            "This spell costs {1} less to cast if it targets an attacking creature",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_318_etb_rat_token_is_creature_gated() {
        for source_name in ["Edgewall Pack", "Voracious Vermin"] {
            let matched = issue_318_match_non_spell(ISSUE_318_ETB_RAT_TOKEN_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.etb.create_rat_token.cant_block"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("rat ETB must emit one triggered ability");
            };
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::CreateTokens {
                    token: "rat_b_1_1_cant_block".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                }]
            );
            assert!(ability.targeting.is_none());
            assert!(!ability.may);
            assert!(ability.intervening_if.is_none());
        }

        let soldier = issue_318_match_non_spell(
            "When this creature enters, create a 1/1 white Soldier creature token.",
            "Near Miss",
        );
        assert_eq!(soldier.id.as_str(), "etb.create_token.printed_one_one.one");

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_318_ETB_RAT_TOKEN_CLAUSE, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the rat ETB recipe is creature-source-only"
        );
        assert!(
            match_clause(ISSUE_318_ETB_RAT_TOKEN_CLAUSE, true, &context())
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the rat ETB recipe must reject spell clauses"
        );
    }

    #[test]
    fn issue_318_etb_rat_token_rejects_near_misses() {
        for negative in [
            r#"When this creature dies, create a 1/1 black Rat creature token with "This token can't block.""#,
            "When this creature enters, create a 1/1 black Rat creature token.",
            r#"When this creature enters, create two 1/1 black Rat creature tokens with "This token can't block.""#,
            r#"When this creature enters, create a tapped 1/1 black Rat creature token with "This token can't block.""#,
            r#"When this creature enters, create a 1/1 black Rat creature token with "This creature can't block.""#,
            r#"When this creature enters, create a 1/1 black Rat creature token with "This token can't block." Draw a card."#,
            r#"When this creature enters, create a 1/1 black Rat creature token with "This token can't attack.""#,
            "When this creature enters, create a 1/1 black Rat creature token with flying.",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_318_etb_flying_grant_is_creature_gated() {
        for source_name in ["Gale Swooper", "Stratosoarer", "Nephalia Moondrakes"] {
            let matched = issue_318_match_non_spell(ISSUE_318_ETB_FLYING_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.etb.target_creature_gains_flying"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("flying ETB must emit one triggered ability");
            };
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    keywords: vec![Keyword::Flying],
                }]
            );
            let targeting = ability.targeting.expect("flying ETB must target");
            let [group] = targeting.groups.as_slice() else {
                panic!("flying ETB must own exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target creature");
            assert_eq!(group.effect_indices, vec![0]);
            assert!(!ability.may);
            assert!(ability.intervening_if.is_none());
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_318_ETB_FLYING_CLAUSE, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the flying ETB recipe is creature-source-only"
        );
        assert!(
            match_clause(ISSUE_318_ETB_FLYING_CLAUSE, true, &context())
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the flying ETB recipe must reject spell clauses"
        );
    }

    #[test]
    fn issue_318_etb_flying_grant_rejects_near_misses() {
        for negative in [
            "When this creature enters, target creature you control gains flying until end of turn.",
            "When this creature enters, target creature gains flying.",
            "When this creature enters, target creature gains flying and hexproof until end of turn.",
            "When this creature enters, target creature gets +1/+0 and gains flying until end of turn.",
            "When this creature enters, up to one target creature gains flying until end of turn.",
            "When this Equipment enters, attach it to target creature you control. That creature gains flying until end of turn.",
            "When this creature enters, target creature gains flying until end of turn. Draw a card.",
            "When this creature enters, target creature gains first strike until end of turn.",
            "When this creature enters, target creature gains flying until end of combat.",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_318_self_damage_to_opponent_draw_is_creature_gated() {
        for source_name in ["Thieving Magpie", "Thieving Otter"] {
            let matched = issue_318_match_non_spell(ISSUE_318_SELF_DAMAGE_DRAW_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.self_damage_to_opponent.draw"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("self damage draw must emit one triggered ability");
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverSelfDealsDamageToOpponent
            );
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                }]
            );
            assert!(ability.targeting.is_none());
            assert!(!ability.may);
            assert!(ability.intervening_if.is_none());
        }

        let combat_food = issue_318_match_non_spell(
            "Whenever this creature deals combat damage to a player, create a Food token.",
            "Near Miss",
        );
        assert_eq!(
            combat_food.id.as_str(),
            "triggered.self_combat_damage_to_player.create_token.food.one"
        );

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_318_SELF_DAMAGE_DRAW_CLAUSE, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the self damage draw recipe is creature-source-only"
        );
        assert!(
            match_clause(ISSUE_318_SELF_DAMAGE_DRAW_CLAUSE, true, &context())
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the self damage draw recipe must reject spell clauses"
        );
    }

    #[test]
    fn issue_318_self_damage_to_opponent_draw_rejects_near_misses() {
        for negative in [
            "Whenever this creature deals combat damage to a player, draw a card.",
            "Whenever this creature deals damage to a player, draw a card.",
            "Whenever this creature deals damage to an opponent, you may draw a card.",
            "Whenever this creature deals damage to an opponent, draw two cards.",
            "Whenever this creature deals damage to an opponent, create a Treasure token.",
            "Whenever another creature deals damage to an opponent, draw a card.",
            "Whenever this creature deals damage to an opponent, draw a card. You gain 1 life.",
            "Whenever this creature deals damage to an opponent, draw a card",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_323_recipes_have_stable_ids_and_surfaces() {
        for (id, surface) in [
            ("spell.destroy_all.creatures", RecipeSurface::SpellClause),
            (
                "spell.counter.target_spell.unless_pays.two",
                RecipeSurface::SpellClause,
            ),
            (
                "spell.grant_keywords.target_creature.double_strike",
                RecipeSurface::SpellClause,
            ),
            ("spell.create_treasure_token", RecipeSurface::SpellClause),
            (
                "spell.search_library.basic_land.battlefield_tapped",
                RecipeSurface::SpellClause,
            ),
            (
                "static.other_creatures_you_control.have_trample",
                RecipeSurface::StaticAbility,
            ),
            (
                "spell.target_creature_cant_be_blocked_this_turn",
                RecipeSurface::SpellClause,
            ),
            (
                "static.self.cannot_be_blocked_by_more_than_one_creature",
                RecipeSurface::StaticAbility,
            ),
        ] {
            assert_eq!(
                issue_318_recipe(id).surface,
                surface,
                "{id} surface drifted"
            );
        }
    }

    #[test]
    fn issue_323_destroy_all_creatures_is_exact_and_typed() {
        for source_name in ["Day of Judgment", "Supreme Verdict", "Doomskar"] {
            let matched =
                issue_318_match_spell(ISSUE_323_DESTROY_ALL_CREATURES_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "spell.destroy_all.creatures");
            assert_eq!(
                matched.emission,
                RecipeEmission::SpellEffect(SpellEffectKind::DestroyAll {
                    kind: TargetFilter::default_creature(),
                    prevent_regeneration: false,
                })
            );
        }
        assert!(
            match_clause(ISSUE_323_DESTROY_ALL_CREATURES_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "destroy-all creatures is a spell clause and must not match permanent text"
        );
    }

    #[test]
    fn issue_323_destroy_all_creatures_rejects_near_misses() {
        for negative in [
            "Destroy all nonland permanents.",
            "Destroy all creatures you don't control.",
            "Destroy all creatures with flying.",
            "Destroy all creatures. They can't be regenerated.",
            "Exile all creatures.",
            "Destroy all creatures with power 4 or greater.",
            "Destroy all creatures",
            "Destroy all creatures. Draw a card.",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_323_counter_unless_pays_two_is_exact_and_typed() {
        for source_name in ["It'll Quench Ya!", "Quench", "Miscalculation"] {
            let matched =
                issue_318_match_spell(ISSUE_323_COUNTER_UNLESS_PAYS_TWO_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.counter.target_spell.unless_pays.two"
            );
            assert_eq!(
                matched.emission,
                RecipeEmission::SpellEffect(SpellEffectKind::CounterTargetSpell {
                    spell_filter: StackSpellFilter::default(),
                    unless_controller_pays: Some(Amount::Fixed(2)),
                    unless_controller_pays_by_cast_cost: None,
                })
            );
        }
        assert!(
            match_clause(ISSUE_323_COUNTER_UNLESS_PAYS_TWO_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "the unless-pays counter clause is a spell clause"
        );
    }

    #[test]
    fn issue_323_counter_unless_pays_two_rejects_near_misses() {
        for negative in [
            "Counter target spell unless its controller pays {1}.",
            "Counter target spell unless its controller pays {3}.",
            "Counter target noncreature spell unless its controller pays {2}.",
            "Counter target spell unless its controller pays {2} for each card in your graveyard.",
            "Counter target spell unless its controller pays {2}",
            "Counter target spell unless its controller pays {2}. Draw a card.",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_323_double_strike_grant_is_exact_and_typed() {
        for source_name in [
            "Two-Headed Hunter // Twice the Rage",
            "Temur Battle Rage",
            "Assault Strobe",
        ] {
            let matched = issue_318_match_spell(ISSUE_323_DOUBLE_STRIKE_GRANT_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.grant_keywords.target_creature.double_strike"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("double strike grant must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    keywords: vec![Keyword::DoubleStrike],
                }]
            );
            let [group] = targeting.groups.as_slice() else {
                panic!("double strike grant must own exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target creature");
            assert_eq!(group.effect_indices, vec![0]);
            assert!(group.distinct_from.is_empty());
        }
        assert!(
            match_clause(ISSUE_323_DOUBLE_STRIKE_GRANT_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "the double strike grant is a spell clause"
        );
    }

    #[test]
    fn issue_323_double_strike_grant_rejects_near_misses() {
        for negative in [
            "Target creature gains double strike until end of turn. Untap it.",
            "Target creature gets +1/+0 and gains double strike until end of turn.",
            "Creatures you control gain double strike until end of turn.",
            "Target creature gains first strike until end of turn.",
            "Target creature gains double strike.",
            "Target creature gains double strike until end of turn. Scry 1.",
            "Target creature gains double strike until end of combat.",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_323_create_treasure_token_is_exact_and_typed() {
        for source_name in ["Ancestors' Aid", "Prizefight", "Strike It Rich"] {
            let matched =
                issue_318_match_spell(ISSUE_323_CREATE_TREASURE_TOKEN_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "spell.create_treasure_token");
            assert_eq!(
                matched.emission,
                RecipeEmission::SpellEffect(SpellEffectKind::CreateTokens {
                    token: "treasure".into(),
                    count: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                    tapped: false,
                    sacrifice_timing: None,
                })
            );
        }
        assert!(
            match_clause(ISSUE_323_CREATE_TREASURE_TOKEN_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "the create-Treasure clause is a spell clause; ETB/attack forms use their own surface"
        );
    }

    #[test]
    fn issue_323_create_treasure_token_rejects_near_misses() {
        for negative in [
            "Create two Treasure tokens.",
            "Create a tapped Treasure token.",
            "Create a Treasure token. You gain 1 life.",
            "Create a Treasure token, then draw a card.",
            "Create a Food token.",
            "Create a Treasure token",
            "Create a Treasure token.\nDraw a card.",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_323_search_basic_land_is_exact_and_typed() {
        for source_name in [
            "Shared Roots",
            "Thunderherd Migration",
            "Natural Connection",
        ] {
            let matched =
                issue_318_match_spell(ISSUE_323_SEARCH_BASIC_LAND_TAPPED_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.search_library.basic_land.battlefield_tapped"
            );
            assert_eq!(
                matched.emission,
                RecipeEmission::SpellEffect(SpellEffectKind::SearchLibrary {
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
                })
            );
        }
        assert!(
            match_clause(ISSUE_323_SEARCH_BASIC_LAND_TAPPED_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "the basic-land search is a spell clause"
        );
    }

    #[test]
    fn issue_323_search_basic_land_rejects_near_misses() {
        for negative in [
            "Search your library for a basic land card, reveal it, put it into your hand, then shuffle.",
            "Search your library for a basic land card, put it onto the battlefield, then shuffle.",
            "Search your library for a land card, put it onto the battlefield tapped, then shuffle.",
            "Search your library for up to two basic land cards, put them onto the battlefield tapped, then shuffle.",
            "You may search your library for a basic land card, put it onto the battlefield tapped, then shuffle.",
            "Search your library for a basic land card, put it onto the battlefield tapped, then shuffle",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_323_other_creatures_trample_is_creature_gated_and_excludes_self() {
        for source_name in [
            "Aggressive Mammoth",
            "Nylea's Forerunner",
            "Khenra Charioteer",
        ] {
            let matched =
                issue_318_match_non_spell(ISSUE_323_OTHER_CREATURES_TRAMPLE_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "static.other_creatures_you_control.have_trample"
            );
            let RecipeEmission::StaticAbility(ability) = matched.emission else {
                panic!("the trample anthem must emit one static ability");
            };
            assert_eq!(ability.ability_id.as_str(), "static_01");
            assert_eq!(
                ability.definition,
                StaticAbilityDef::AnthemKeyword {
                    filter: CreatureScopeFilter {
                        controller: Some(CreatureScopeController::YouControl),
                        exclude_self: true,
                        ..CreatureScopeFilter::default()
                    },
                    condition: None,
                    keyword: Keyword::Trample,
                }
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(
                ISSUE_323_OTHER_CREATURES_TRAMPLE_CLAUSE,
                false,
                &noncreature
            )
            .expect("noncreature source must not be ambiguous")
            .is_none(),
            "the trample anthem is creature-source-only"
        );
        assert!(
            match_clause(ISSUE_323_OTHER_CREATURES_TRAMPLE_CLAUSE, true, &context())
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the trample anthem must reject spell clauses"
        );
    }

    #[test]
    fn issue_323_other_creatures_trample_rejects_near_misses() {
        for negative in [
            "Creatures you control have trample.",
            "Other creatures you control get +1/+1.",
            "Other creatures you control have trample and haste.",
            "Other attacking creatures you control have trample.",
            "Other creatures you control with power 4 or greater have trample.",
            "Other creatures you control have trample",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_323_target_creature_cant_be_blocked_is_exact_and_typed() {
        for source_name in ["Enter the Enigma", "Infiltrate", "Artful Dodge"] {
            let matched =
                issue_318_match_spell(ISSUE_323_CANT_BE_BLOCKED_THIS_TURN_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.target_creature_cant_be_blocked_this_turn"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("the unblockable grant must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![SpellEffectKind::ApplyCombatRestriction {
                    scope: CombatRestrictionScope::Chosen(TargetFilter::default_creature()),
                    restriction: CombatRestriction {
                        cant_be_blocked: true,
                        ..CombatRestriction::default()
                    },
                }]
            );
            let [group] = targeting.groups.as_slice() else {
                panic!("the unblockable grant must own exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target creature");
            assert_eq!(group.effect_indices, vec![0]);
        }
        assert!(
            match_clause(
                ISSUE_323_CANT_BE_BLOCKED_THIS_TURN_CLAUSE,
                false,
                &context()
            )
            .expect("non-spell surface check must not be ambiguous")
            .is_none(),
            "the unblockable grant is a spell clause"
        );
    }

    #[test]
    fn issue_323_target_creature_cant_be_blocked_rejects_near_misses() {
        for negative in [
            "Target creature can't block this turn.",
            "Target creature can't be blocked this combat.",
            "Target creature is unblockable this turn.",
            "Up to one target creature can't be blocked this turn.",
            "Target creature can't be blocked this turn. Draw a card.",
            "Target creature can't be blocked this turn",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_323_self_max_one_blocker_is_creature_gated_and_typed() {
        for source_name in ["Professional Wrestler", "Charging Rhino", "Stalking Tiger"] {
            let matched =
                issue_318_match_non_spell(ISSUE_323_MAXIMUM_ONE_BLOCKER_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "static.self.cannot_be_blocked_by_more_than_one_creature"
            );
            let RecipeEmission::StaticAbility(ability) = matched.emission else {
                panic!("the maximum-blocker restriction must emit one static ability");
            };
            assert_eq!(
                ability.definition,
                StaticAbilityDef::SelfCombatRestriction {
                    restriction: CombatRestriction {
                        maximum_blockers: Some(1),
                        ..CombatRestriction::default()
                    },
                    condition: None,
                }
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_323_MAXIMUM_ONE_BLOCKER_CLAUSE, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the maximum-blocker restriction is creature-source-only"
        );
        assert!(
            match_clause(ISSUE_323_MAXIMUM_ONE_BLOCKER_CLAUSE, true, &context())
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the maximum-blocker restriction must reject spell clauses"
        );
    }

    #[test]
    fn issue_323_self_max_one_blocker_rejects_near_misses() {
        for negative in [
            "This creature can't be blocked.",
            "This creature can't be blocked by more than two creatures.",
            "This creature can't be blocked by creatures with power 2 or greater.",
            "This creature can't be blocked except by two or more creatures.",
            "This creature can block only creatures with power 2 or less.",
            "This creature can't be blocked by more than one creature",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_327_recipes_have_stable_ids_and_surfaces() {
        for (id, surface) in [
            (
                "spell.put_counter.target_creature.one",
                RecipeSurface::SpellClause,
            ),
            (
                "activated.self_pump.plus_one_zero",
                RecipeSurface::ActivatedAbility,
            ),
            (
                "spell.pump.creature.minus_four_minus_four",
                RecipeSurface::SpellClause,
            ),
            (
                "static.anthem.attacking_creatures_you_control.plus_one_zero",
                RecipeSurface::StaticAbility,
            ),
            (
                "triggered.end_step.sacrifice_self",
                RecipeSurface::TriggeredAbility,
            ),
            (
                "activated.tap_discard.draw_one",
                RecipeSurface::ActivatedAbility,
            ),
            (
                "triggered.etb.put_counter.each_other_creature",
                RecipeSurface::EtbAbility,
            ),
            (
                "triggered.another_creature_you_control_dies.put_counter_self",
                RecipeSurface::TriggeredAbility,
            ),
        ] {
            assert_eq!(
                issue_318_recipe(id).surface,
                surface,
                "{id} surface drifted"
            );
        }
    }

    #[test]
    fn issue_327_put_counter_target_creature_is_exact_and_typed() {
        for source_name in ["Honor", "Battlegrowth", "Guiding Voice"] {
            let matched = issue_318_match_spell(ISSUE_327_PUT_COUNTER_TARGET_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "spell.put_counter.target_creature.one");
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("the counter placement must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                }]
            );
            let [group] = targeting.groups.as_slice() else {
                panic!("the counter placement must own exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target creature");
            assert_eq!(group.effect_indices, vec![0]);
            assert!(group.distinct_from.is_empty());
        }
        assert!(
            match_clause(ISSUE_327_PUT_COUNTER_TARGET_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "the standalone counter placement is a spell clause, not the modal payload"
        );
    }

    #[test]
    fn issue_327_put_counter_target_creature_rejects_near_misses() {
        for negative in [
            "Put a +1/+1 counter on each creature you control.",
            "Put two +1/+1 counters on target creature.",
            "Put a +1/+1 counter on up to one target creature.",
            "Put a +1/+1 counter on target creature you control.",
            "Put a -1/-1 counter on target creature.",
            "Put a +1/+1 counter on target creature. Draw a card.",
            "Put a +1/+1 counter on target creature",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_327_self_pump_plus_one_zero_is_parameterized_and_creature_gated() {
        for source_name in ["Shivan Dragon", "Inferno Titan", "Scourge of Valkas"] {
            let matched = issue_318_match_non_spell(
                "{R}: This creature gets +1/+0 until end of turn.",
                source_name,
            );
            assert_eq!(matched.id.as_str(), "activated.self_pump.plus_one_zero");
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("the self-pump must emit an activated ability");
            };
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(
                ability.costs,
                vec![AbilityCost::Mana(ManaCost::parse("{R}").unwrap())]
            );
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 0,
                    scale: None,
                    subject: EffectSubject::Source,
                }]
            );
            assert!(ability.targeting.is_none());
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert!(ability.activation_limit.is_none());
        }

        for color in ["W", "U", "B", "R", "G"] {
            let clause = format!("{{{color}}}: This creature gets +1/+0 until end of turn.");
            let matched = issue_318_match_non_spell(&clause, "Color Calibration");
            assert_eq!(matched.id.as_str(), "activated.self_pump.plus_one_zero");
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("the self-pump must emit an activated ability");
            };
            assert_eq!(
                ability.costs,
                vec![AbilityCost::Mana(
                    ManaCost::parse(&format!("{{{color}}}")).unwrap()
                )]
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(
                "{R}: This creature gets +1/+0 until end of turn.",
                false,
                &noncreature
            )
            .expect("noncreature source must not be ambiguous")
            .is_none(),
            "the self-pump recipe is creature-source-only"
        );
    }

    #[test]
    fn issue_327_self_pump_plus_one_zero_rejects_near_misses() {
        for negative in [
            "{1}{R}: This creature gets +1/+0 until end of turn.",
            "{R}: This creature gets +2/+0 until end of turn.",
            "{R}: This creature gets +1/+1 until end of turn.",
            "{R}: This creature gets +1/+0 until your next turn.",
            "{R}: Target creature gets +1/+0 until end of turn.",
            "{R}, {T}: This creature gets +1/+0 until end of turn.",
            "{R}: This creature gets +1/+0 until end of turn. Activate only as a sorcery.",
            "{R}: This creature gets +1/+0 until end of turn",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_327_minus_four_minus_four_is_exact_and_typed() {
        for source_name in ["Dark Deed", "Grasp of Darkness", "Flatten"] {
            let matched =
                issue_318_match_spell(ISSUE_327_MINUS_FOUR_MINUS_FOUR_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.pump.creature.minus_four_minus_four"
            );
            assert_eq!(
                matched.emission,
                RecipeEmission::SpellEffect(SpellEffectKind::PumpTarget {
                    power: -4,
                    toughness: -4,
                    scale: None,
                    subject: chosen_creature(TargetController::Any),
                })
            );
        }
        assert!(
            match_clause(ISSUE_327_MINUS_FOUR_MINUS_FOUR_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "the -4/-4 pump is a spell clause"
        );
    }

    #[test]
    fn issue_327_minus_four_minus_four_rejects_near_misses() {
        for negative in [
            "Target creature gets -5/-5 until end of turn.",
            "Target creature gets -4/-0 until end of turn.",
            "Creatures you control get -4/-4 until end of turn.",
            "Target creature gets -4/-4 until end of combat.",
            "Target creature gets -4/-4 until end of turn. You gain 1 life.",
            "Up to one target creature gets -4/-4 until end of turn.",
            "Target creature an opponent controls gets -4/-4 until end of turn.",
            "Target creature gets -4/-4 until end of turn",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_327_attacking_anthem_is_enchantment_or_creature_gated() {
        for source_name in ["Goblin Oriflamme", "Orcish Oriflamme", "Warded Battlements"] {
            let matched = issue_318_match_non_spell(ISSUE_327_ATTACKING_ANTHEM_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "static.anthem.attacking_creatures_you_control.plus_one_zero"
            );
            let RecipeEmission::StaticAbility(ability) = matched.emission else {
                panic!("the attacking anthem must emit a static ability");
            };
            assert_eq!(
                ability.definition,
                StaticAbilityDef::AnthemPt {
                    filter: CreatureScopeFilter {
                        controller: Some(CreatureScopeController::YouControl),
                        attacking: true,
                        ..CreatureScopeFilter::default()
                    },
                    condition: None,
                    delta_power: 1,
                    delta_toughness: 0,
                }
            );
        }

        let mut other_source = context();
        other_source.source_is_enchantment = false;
        other_source.source_is_creature = false;
        assert!(
            match_clause(ISSUE_327_ATTACKING_ANTHEM_CLAUSE, false, &other_source)
                .expect("source-kind check must not be ambiguous")
                .is_none(),
            "the attacking anthem is bound to enchantment or creature sources"
        );
        assert!(
            match_clause(ISSUE_327_ATTACKING_ANTHEM_CLAUSE, true, &context())
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the attacking anthem must reject spell clauses"
        );
    }

    #[test]
    fn issue_327_attacking_anthem_rejects_near_misses() {
        for negative in [
            "Creatures you control get +1/+0.",
            "Attacking creatures you control get +1/+1.",
            "Attacking creatures you control get +2/+0.",
            "Other attacking creatures you control get +1/+0.",
            "Attacking creatures an opponent controls get +1/+0.",
            "Attacking creatures you control get +1/+0 until end of turn.",
            "Attacking creatures you control get +1/+0",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_327_end_step_sacrifice_is_creature_gated_and_typed() {
        for source_name in ["Ball Lightning", "Spark Elemental", "Hell's Thunder"] {
            let matched =
                issue_318_match_non_spell(ISSUE_327_END_STEP_SACRIFICE_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "triggered.end_step.sacrifice_self");
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("the end-step sacrifice must emit a triggered ability");
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::AtBeginningOfEndStep {
                    player: CastTriggerPlayer::AnyPlayer,
                }
            );
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::Sacrifice {
                    subject: EffectSubject::Source,
                }]
            );
            assert!(ability.targeting.is_none());
            assert!(ability.intervening_if.is_none());
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_327_END_STEP_SACRIFICE_CLAUSE, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the end-step sacrifice recipe is creature-source-only"
        );
    }

    #[test]
    fn issue_327_end_step_sacrifice_rejects_near_misses() {
        for negative in [
            "At the beginning of your end step, sacrifice this creature.",
            "At the beginning of the next end step, sacrifice this creature.",
            "At the beginning of each end step, sacrifice this creature.",
            "At the beginning of your upkeep, sacrifice this creature.",
            "At the beginning of the end step, sacrifice this artifact.",
            "At the beginning of the end step, sacrifice another creature.",
            "At the beginning of the end step, sacrifice this creature",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_327_tap_discard_draw_is_exact_and_typed() {
        for source_name in ["Charging Strifeknight", "Rummaging Goblin", "Mad Prophet"] {
            let matched = issue_318_match_non_spell(ISSUE_327_TAP_DISCARD_DRAW_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "activated.tap_discard.draw_one");
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("the loot activation must emit an activated ability");
            };
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(ability.costs, vec![AbilityCost::Tap, AbilityCost::Discard]);
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                }]
            );
            assert!(ability.targeting.is_none());
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert!(ability.activation_limit.is_none());
        }
    }

    #[test]
    fn issue_327_tap_discard_draw_rejects_near_misses() {
        for negative in [
            "{T}: Draw a card, then discard a card.",
            "{T}, Discard a card: Draw two cards.",
            "{T}, Discard two cards: Draw a card.",
            "{1}, Discard a card: Draw a card.",
            "{T}, Discard a card: Draw a card. Activate only as a sorcery.",
            "{T}, Sacrifice this creature: Draw a card.",
            "{T}, Discard a card: Draw a card",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_327_etb_mass_counter_is_creature_gated_and_excludes_self() {
        for source_name in ["Web-Warriors", "Ridgescale Tusker", "Primeval Protector"] {
            let matched = issue_318_match_non_spell(ISSUE_327_ETB_MASS_COUNTER_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.etb.put_counter.each_other_creature"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("the mass counter ETB must emit a triggered ability");
            };
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::PutCountersAll {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    filter: CreatureScopeFilter {
                        controller: Some(CreatureScopeController::YouControl),
                        exclude_self: true,
                        ..CreatureScopeFilter::default()
                    },
                }]
            );
            assert!(ability.targeting.is_none());
            assert!(!ability.may);
            assert!(ability.intervening_if.is_none());
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_327_ETB_MASS_COUNTER_CLAUSE, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the mass counter ETB recipe is creature-source-only"
        );
        assert!(
            match_clause(ISSUE_327_ETB_MASS_COUNTER_CLAUSE, true, &context())
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the mass counter ETB must reject spell clauses"
        );
    }

    #[test]
    fn issue_327_etb_mass_counter_rejects_near_misses() {
        for negative in [
            "When this creature enters, put a +1/+1 counter on each creature you control.",
            "When this creature enters, put a +1/+1 counter on each other creature.",
            "When this creature enters, put two +1/+1 counters on each other creature you control.",
            "When this creature enters, put a +1/+1 counter on each other creature you don't control.",
            "When another creature enters, put a +1/+1 counter on each other creature you control.",
            "When this creature enters, put a +1/+1 counter on each other creature you control",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_327_dies_counter_is_creature_gated_and_scoped() {
        for source_name in ["Voracious Vermin", "Rot Shambler", "Unruly Mob"] {
            let matched = issue_318_match_non_spell(ISSUE_327_DIES_COUNTER_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.another_creature_you_control_dies.put_counter_self"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("the dies counter must emit a triggered ability");
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverCreatureDies {
                    controller: CastTriggerPlayer::Controller,
                    filter: PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Creature),
                        exclude_source: true,
                        ..PermanentEventFilter::default()
                    },
                }
            );
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }]
            );
            assert!(ability.targeting.is_none());
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_327_DIES_COUNTER_CLAUSE, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the dies counter recipe is creature-source-only"
        );
    }

    #[test]
    fn issue_327_dies_counter_rejects_near_misses() {
        for negative in [
            "Whenever a creature you control dies, put a +1/+1 counter on this creature.",
            "Whenever another creature dies, put a +1/+1 counter on this creature.",
            "Whenever another creature you control dies, put two +1/+1 counters on this creature.",
            "Whenever another creature you control dies, put a +1/+1 counter on target creature.",
            r#"Whenever another creature you control dies, create a 1/1 black Rat creature token with "This token can't block.""#,
            "Whenever another creature you control dies, put a +1/+1 counter on this creature",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_328_cohort_clauses_match_their_exact_recipes() {
        for (clause, is_spell, expected) in [
            (
                "Exile target attacking creature.",
                true,
                "spell.exile.target_attacking_creature",
            ),
            (
                "Sacrifice this creature: Destroy target enchantment.",
                false,
                "activated.sacrifice_self.destroy_target_enchantment",
            ),
            (
                "At the beginning of your upkeep, this creature deals 1 damage to you.",
                false,
                "triggered.upkeep.self_damage_controller_one",
            ),
            (
                "{T}: Surveil 1.",
                false,
                "activated.tap.surveil_one",
            ),
            (
                "{T}: Target creature gains haste until end of turn.",
                false,
                "activated.tap.target_creature_gains_haste",
            ),
            (
                "When this creature enters, return target permanent card from your graveyard to your hand.",
                false,
                "triggered.etb.return_target_permanent_card_to_hand",
            ),
            (
                "Up to two target creatures can't block this turn.",
                true,
                "spell.up_to_two_target_creatures_cant_block",
            ),
            (
                "Target creature gets +1/+0 and gains first strike until end of turn. Scry 1.",
                true,
                "spell.pump.target_creature_plus_one_zero_first_strike_scry_one",
            ),
        ] {
            let matched = match_clause(clause, is_spell, &context())
                .unwrap_or_else(|ambiguity| panic!("{clause}: {ambiguity}"))
                .unwrap_or_else(|| panic!("{clause} should match exactly one recipe"));
            assert_eq!(matched.id.as_str(), expected, "{clause}");
        }
    }

    #[test]
    fn issue_328_recipes_have_stable_ids_and_surfaces() {
        for (id, surface) in [
            (
                "spell.exile.target_attacking_creature",
                RecipeSurface::SpellClause,
            ),
            (
                "activated.sacrifice_self.destroy_target_enchantment",
                RecipeSurface::ActivatedAbility,
            ),
            (
                "triggered.upkeep.self_damage_controller_one",
                RecipeSurface::TriggeredAbility,
            ),
            ("activated.tap.surveil_one", RecipeSurface::ActivatedAbility),
            (
                "activated.tap.target_creature_gains_haste",
                RecipeSurface::ActivatedAbility,
            ),
            (
                "triggered.etb.return_target_permanent_card_to_hand",
                RecipeSurface::EtbAbility,
            ),
            (
                "spell.up_to_two_target_creatures_cant_block",
                RecipeSurface::SpellClause,
            ),
            (
                "spell.pump.target_creature_plus_one_zero_first_strike_scry_one",
                RecipeSurface::SpellClause,
            ),
        ] {
            assert_eq!(
                issue_318_recipe(id).surface,
                surface,
                "{id} surface drifted"
            );
        }
    }

    #[test]
    fn issue_328_exile_attacking_creature_is_exact_and_typed() {
        for source_name in ["Not on My Watch", "Resounding Silence", "Second Thoughts"] {
            let matched =
                issue_318_match_spell(ISSUE_328_EXILE_ATTACKING_CREATURE_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "spell.exile.target_attacking_creature");
            let RecipeEmission::SpellEffect(effect) = matched.emission else {
                panic!("the attacking exile must emit one spell effect");
            };
            assert_eq!(
                effect,
                SpellEffectKind::Exile {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::Creature,
                        combat_role: Some(CombatRole::Attacking),
                        ..TargetFilter::default()
                    })),
                }
            );
        }
        assert!(
            match_clause(ISSUE_328_EXILE_ATTACKING_CREATURE_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "the attacking exile is a spell clause"
        );
    }

    #[test]
    fn issue_328_exile_attacking_creature_rejects_near_misses() {
        for negative in [
            "Exile target attacking or blocking creature.",
            "Exile target blocking creature.",
            "Exile target creature.",
            "Exile target attacking creature you control.",
            "Exile up to one target attacking creature.",
            "Exile target attacking creature. Draw a card.",
        ] {
            // The unrestricted "Exile target creature." clause stays owned by the shipped
            // exile-creature recipe; the attacking-only template must not claim it.
            let outcome = match_clause(negative, true, &context())
                .unwrap_or_else(|ambiguity| panic!("{negative}: {ambiguity}"));
            match outcome {
                None => {}
                Some(matched) => assert_eq!(
                    matched.id.as_str(),
                    "spell.exile.creature",
                    "{negative} must not be claimed by the attacking-exile recipe"
                ),
            }
        }
    }

    #[test]
    fn issue_328_sacrifice_self_destroy_enchantment_is_typed() {
        for source_name in ["Felidar Cub", "Kami of Ancient Law", "Ronom Unicorn"] {
            let matched = issue_318_match_non_spell(
                ISSUE_328_SACRIFICE_SELF_DESTROY_ENCHANTMENT_CLAUSE,
                source_name,
            );
            assert_eq!(
                matched.id.as_str(),
                "activated.sacrifice_self.destroy_target_enchantment"
            );
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("sacrifice-to-destroy must emit an activated ability");
            };
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(ability.costs, vec![AbilityCost::SacrificeSelf]);
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::Destroy {
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        permanent_types: vec![PermanentTypeFilter::Enchantment],
                        ..TargetFilter::default()
                    })),
                }]
            );
            assert!(ability.targeting.is_none());
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert!(ability.activation_limit.is_none());
        }
        assert!(
            match_clause(
                ISSUE_328_SACRIFICE_SELF_DESTROY_ENCHANTMENT_CLAUSE,
                true,
                &context()
            )
            .expect("spell surface check must not be ambiguous")
            .is_none(),
            "sacrifice-to-destroy is an activated ability, not a spell clause"
        );
    }

    #[test]
    fn issue_328_sacrifice_self_destroy_enchantment_rejects_near_misses() {
        for negative in [
            "{1}, Sacrifice this creature: Destroy target enchantment.",
            "Sacrifice this creature: Destroy target artifact.",
            "Sacrifice this creature: Destroy target artifact or enchantment.",
            "Sacrifice another creature: Destroy target enchantment.",
            "Sacrifice this creature: Exile target enchantment.",
            "Sacrifice this creature: Destroy target enchantment. Draw a card.",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_328_upkeep_self_damage_is_creature_gated_and_typed() {
        for source_name in ["Ravenous Giant", "Nettletooth Djinn", "Serendib Efreet"] {
            let matched =
                issue_318_match_non_spell(ISSUE_328_UPKEEP_SELF_DAMAGE_ONE_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.upkeep.self_damage_controller_one"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("the upkeep self-damage must emit a triggered ability");
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::AtBeginningOfUpkeep {
                    player: CastTriggerPlayer::Controller,
                }
            );
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::DamagePlayer {
                    amount: Amount::Fixed(1),
                    who: PlayerRecipient::Controller,
                }]
            );
            assert!(ability.targeting.is_none());
            assert!(!ability.may);
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_328_UPKEEP_SELF_DAMAGE_ONE_CLAUSE, false, &noncreature)
                .expect("noncreature source must not be ambiguous")
                .is_none(),
            "the upkeep self-damage recipe is creature-source-only"
        );
        assert!(
            match_clause(ISSUE_328_UPKEEP_SELF_DAMAGE_ONE_CLAUSE, true, &context())
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the upkeep self-damage must reject spell clauses"
        );
    }

    #[test]
    fn issue_328_upkeep_self_damage_rejects_near_misses() {
        for negative in [
            "At the beginning of your upkeep, this creature deals 2 damage to you.",
            "At the beginning of each upkeep, this creature deals 1 damage to you.",
            "At the beginning of your upkeep, this creature deals 1 damage to each opponent.",
            "At the beginning of your upkeep, this creature deals 1 damage to target player.",
            "At the beginning of your end step, this creature deals 1 damage to you.",
            "At the beginning of your upkeep, this creature deals 1 damage to you. Draw a card.",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_328_tap_surveil_one_is_typed_and_exact() {
        for source_name in ["Rune-Sealed Wall", "Sinister Starfish", "Microscope"] {
            let matched = issue_318_match_non_spell(ISSUE_328_TAP_SURVEIL_ONE_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "activated.tap.surveil_one");
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("the surveil activation must emit an activated ability");
            };
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(ability.costs, vec![AbilityCost::Tap]);
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::LibraryPartition {
                    count: 1,
                    top_min: 0,
                    top_max: None,
                    kind: LibraryPartitionKind::Surveil,
                }]
            );
            assert!(ability.targeting.is_none());
            assert_eq!(ability.timing, ActivationTiming::Normal);
        }
        assert!(
            match_clause(ISSUE_328_TAP_SURVEIL_ONE_CLAUSE, true, &context())
                .expect("spell surface check must not be ambiguous")
                .is_none(),
            "the surveil activation is not a spell clause"
        );
    }

    #[test]
    fn issue_328_tap_surveil_one_rejects_near_misses() {
        for negative in [
            "{4}, {T}: Surveil 1.",
            "{1}, {T}: Surveil 1.",
            "{T}: Surveil 2.",
            "{T}: Scry 1.",
            "{T}: Surveil 1. Activate only as a sorcery.",
            "{T}, Pay 1 life: Surveil 1.",
            "{T}: Surveil 1. Draw a card.",
        ] {
            assert!(
                match_clause(negative, false, &context())
                    .unwrap_or_else(|ambiguity| panic!("{negative}: {ambiguity}"))
                    .is_none()
                    || matches!(
                        match_clause(negative, false, &context())
                            .unwrap_or_else(|ambiguity| panic!("{negative}: {ambiguity}")),
                        Some(matched) if matched.id.as_str()
                            == "activated.land.pay_four_tap.surveil_one"
                    ),
                "the {negative:?} near-miss must not be claimed by the tap-surveil recipe"
            );
        }
    }

    #[test]
    fn issue_328_tap_target_creature_gains_haste_is_typed() {
        for source_name in ["Axgard Cavalry", "Akki Drillmaster", "Bloodlust Inciter"] {
            let matched = issue_318_match_non_spell(
                ISSUE_328_TAP_TARGET_CREATURE_GAINS_HASTE_CLAUSE,
                source_name,
            );
            assert_eq!(
                matched.id.as_str(),
                "activated.tap.target_creature_gains_haste"
            );
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("the haste grant must emit an activated ability");
            };
            assert_eq!(ability.costs, vec![AbilityCost::Tap]);
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::GrantKeywords {
                    subject: chosen_creature(TargetController::Any),
                    keywords: vec![Keyword::Haste],
                }]
            );
            assert_eq!(
                ability.targeting,
                Some(exact_targeting(1, 1, "Choose target creature", vec![0]))
            );
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert!(ability.activation_limit.is_none());
        }
        assert!(
            match_clause(
                ISSUE_328_TAP_TARGET_CREATURE_GAINS_HASTE_CLAUSE,
                true,
                &context()
            )
            .expect("spell surface check must not be ambiguous")
            .is_none(),
            "the haste grant is an activated ability, not a spell clause"
        );
    }

    #[test]
    fn issue_328_tap_target_creature_gains_haste_rejects_near_misses() {
        for negative in [
            "{T}: Target creature you control gains haste until end of turn.",
            "{T}: This creature gains haste until end of turn.",
            "{1}, {T}: Target creature gains haste until end of turn.",
            "{T}: Target creature gains haste.",
            "{T}: Target creature gains haste and trample until end of turn.",
            "{T}: Target creature gets +1/+0 and gains haste until end of turn.",
            "{T}: Target creature gains haste until end of turn. Draw a card.",
        ] {
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_328_etb_return_permanent_card_is_creature_gated_and_typed() {
        for source_name in ["Elvish Regrower", "Gloomshrieker", "Golgari Findbroker"] {
            let matched =
                issue_318_match_non_spell(ISSUE_328_ETB_RETURN_PERMANENT_CARD_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.etb.return_target_permanent_card_to_hand"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("the permanent-card return must emit a triggered ability");
            };
            assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
            assert_eq!(
                ability.effect,
                vec![SpellEffectKind::MoveGraveyardCards {
                    filter: GraveyardFilter {
                        owner: GraveyardOwner::Controller,
                        card: Some(ZoneCardFilter {
                            excluded_card_types: vec![
                                CardTypeFilter::Instant,
                                CardTypeFilter::Sorcery,
                            ],
                            ..ZoneCardFilter::default()
                        }),
                        ..GraveyardFilter::default()
                    },
                    destination: GraveyardDestination::Hand,
                    linked_exile_id: None,
                }]
            );
            assert!(ability.targeting.is_none());
            assert!(!ability.may);
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(
                ISSUE_328_ETB_RETURN_PERMANENT_CARD_CLAUSE,
                false,
                &noncreature
            )
            .expect("noncreature source must not be ambiguous")
            .is_none(),
            "the permanent-card return recipe is creature-source-only"
        );
    }

    #[test]
    fn issue_328_etb_return_permanent_card_rejects_near_misses() {
        for negative in [
            "When this creature enters, return target creature card from your graveyard to your hand.",
            "When this creature enters, return target permanent card from your graveyard to the battlefield.",
            "When this creature enters, return target permanent card from an opponent's graveyard to your hand.",
            "When this creature enters, you may return target permanent card from your graveyard to your hand.",
            "When this creature dies, return target permanent card from your graveyard to your hand.",
            "When this creature enters, return target permanent card from your graveyard to your hand. Draw a card.",
        ] {
            // The exact creature-card clause is consumed by the existing creature recursion
            // recipe, which is the correct single match; every other near-miss stays unmatched.
            let outcome = match_clause(negative, false, &context())
                .unwrap_or_else(|ambiguity| panic!("{negative}: {ambiguity}"));
            match outcome {
                None => {}
                Some(matched) => assert_eq!(
                    matched.id.as_str(),
                    "triggered.etb.return_creature_card_from_graveyard.hand",
                    "{negative} must not be claimed by the permanent-card recipe"
                ),
            }
        }
    }

    #[test]
    fn issue_328_up_to_two_cant_block_is_bounded_and_typed() {
        for source_name in [
            "Bellowing Bruiser // Beat a Path",
            "Abandon the Post",
            "Nightbird's Clutches",
        ] {
            let matched = issue_318_match_spell(ISSUE_328_UP_TO_TWO_CANT_BLOCK_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.up_to_two_target_creatures_cant_block"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("the can't-block restriction must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![SpellEffectKind::ApplyCombatRestriction {
                    scope: CombatRestrictionScope::Chosen(TargetFilter::default_creature()),
                    restriction: CombatRestriction {
                        cant_block: true,
                        ..CombatRestriction::default()
                    },
                }]
            );
            let [group] = targeting.groups.as_slice() else {
                panic!("the can't-block restriction must own exactly one target group");
            };
            assert_eq!((group.min, group.max), (0, 2));
            assert_eq!(group.prompt, "Choose up to two target creatures");
            assert_eq!(group.effect_indices, vec![0]);
        }
        assert!(
            match_clause(ISSUE_328_UP_TO_TWO_CANT_BLOCK_CLAUSE, false, &context())
                .expect("non-spell surface check must not be ambiguous")
                .is_none(),
            "the can't-block restriction is a spell clause"
        );
    }

    #[test]
    fn issue_328_up_to_two_cant_block_rejects_near_misses() {
        for negative in [
            "Target creature can't block this turn.",
            "Up to two target creatures you control can't block this turn.",
            "Up to two target creatures can't attack this turn.",
            "Up to two target creatures can't block.",
            "Up to one target creature can't block this turn.",
            "Up to two target creatures can't block this turn. Draw a card.",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_328_pump_first_strike_scry_one_shares_one_target_group() {
        for source_name in ["Kindled Heroism", "Coming In Hot", "Storm Strike"] {
            let matched =
                issue_318_match_spell(ISSUE_328_PUMP_FIRST_STRIKE_SCRY_ONE_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.pump.target_creature_plus_one_zero_first_strike_scry_one"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("the pump/scry template must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![
                    SpellEffectKind::PumpTarget {
                        power: 1,
                        toughness: 0,
                        scale: None,
                        subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                    },
                    SpellEffectKind::GrantKeywords {
                        subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
                        keywords: vec![Keyword::FirstStrike],
                    },
                    SpellEffectKind::Scry {
                        count: Amount::Fixed(1),
                    },
                ]
            );
            let [group] = targeting.groups.as_slice() else {
                panic!("the pump/scry template must own exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target creature");
            assert_eq!(
                group.effect_indices,
                vec![0, 1],
                "the single group must cover the pump and the keyword grant, not the scry"
            );
        }
        assert!(
            match_clause(
                ISSUE_328_PUMP_FIRST_STRIKE_SCRY_ONE_CLAUSE,
                false,
                &context()
            )
            .expect("non-spell surface check must not be ambiguous")
            .is_none(),
            "the pump/scry template is a spell clause"
        );
    }

    #[test]
    fn issue_328_pump_first_strike_scry_one_rejects_near_misses() {
        for negative in [
            "Target creature gets +1/+0 and gains first strike until end of turn. Scry 2.",
            "Target creature gets +1/+0 and gains first strike until end of turn. Investigate.",
            "Target creature gets +1/+1 and gains first strike until end of turn. Scry 1.",
            "Target creature gains first strike until end of turn. Scry 1.",
            "Target creature gets +1/+0 and gains first strike until end of turn. Scry 1. Draw a card.",
        ] {
            issue_318_assert_unmatched(negative, true);
        }
    }

    #[test]
    fn issue_328_previously_shipped_clauses_keep_their_existing_recipes() {
        for (clause, is_spell, expected) in [
            ("Exile target creature.", true, "spell.exile.creature"),
            (
                "{4}, {T}: Surveil 1.",
                false,
                "activated.land.pay_four_tap.surveil_one",
            ),
            (
                "When this creature enters, return target creature card from your graveyard to your hand.",
                false,
                "triggered.etb.return_creature_card_from_graveyard.hand",
            ),
            (
                "Target creature gets +1/+0 and gains first strike until end of turn.",
                true,
                "spell.pump.target_creature_plus_n_zero_first_strike",
            ),
        ] {
            let matched = match_clause(clause, is_spell, &context())
                .unwrap_or_else(|ambiguity| panic!("{clause}: {ambiguity}"))
                .unwrap_or_else(|| panic!("{clause} should stay consumed by its shipped recipe"));
            assert_eq!(matched.id.as_str(), expected, "{clause}");
        }
    }

    #[test]
    fn issue_329_warp_and_flashback_clauses_match_their_exact_recipes() {
        for (clause, is_spell, expected) in [
            ("Warp {3}", false, "cast_method.warp"),
            ("Warp {1}{G}", false, "cast_method.warp"),
            ("Warp {1}{R}", false, "cast_method.warp"),
            ("Flashback {2}{U}", true, "cast_method.flashback"),
            ("Flashback {2}{W}{W}", true, "cast_method.flashback"),
        ] {
            let matched = match_clause(clause, is_spell, &context())
                .unwrap_or_else(|ambiguity| panic!("{clause}: {ambiguity}"))
                .unwrap_or_else(|| panic!("{clause} should match exactly one recipe"));
            assert_eq!(matched.id.as_str(), expected, "{clause}");
        }
    }

    #[test]
    fn issue_329_recipes_have_stable_ids_and_surfaces() {
        for id in ["cast_method.warp", "cast_method.flashback"] {
            assert_eq!(
                issue_318_recipe(id).surface,
                RecipeSurface::CastMethodClause,
                "{id} surface drifted"
            );
        }
    }

    #[test]
    fn issue_329_warp_emits_the_printed_cost_on_permanent_faces_only() {
        for (source_name, clause, expected) in [
            ("Bygone Colossus", "Warp {3}", "{3}"),
            ("Germinating Wurm", "Warp {1}{G}", "{1}{G}"),
            ("Red Tiger Mechan", "Warp {1}{R}", "{1}{R}"),
            ("Starbreach Whale", "Warp {1}{U}", "{1}{U}"),
        ] {
            let matched = issue_318_match_non_spell(clause, source_name);
            assert_eq!(matched.id.as_str(), "cast_method.warp");
            let RecipeEmission::WarpCost(cost) = matched.emission else {
                panic!("Warp must emit the first-class face cost emission");
            };
            assert_eq!(cost.to_string(), expected, "{source_name}");
        }

        let mut spell_face = context();
        spell_face.source_is_permanent = false;
        assert!(
            match_clause("Warp {1}{G}", true, &spell_face)
                .expect("spell-face check must not be ambiguous")
                .is_none(),
            "Warp must fail closed on a non-permanent face"
        );
        assert!(match_cast_method_warp("Warp {1}{G}", &spell_face).is_none());
    }

    #[test]
    fn issue_329_flashback_emits_the_printed_cost_on_instant_or_sorcery_faces_only() {
        for (source_name, clause, expected) in [
            ("Think Twice", "Flashback {2}{U}", "{2}{U}"),
            ("Auron's Inspiration", "Flashback {2}{W}{W}", "{2}{W}{W}"),
            ("Daydream", "Flashback {2}{W}", "{2}{W}"),
        ] {
            let matched = issue_318_match_spell(clause, source_name);
            assert_eq!(matched.id.as_str(), "cast_method.flashback");
            let RecipeEmission::FlashbackCost(cost) = matched.emission else {
                panic!("Flashback must emit the first-class face cost emission");
            };
            assert_eq!(cost.to_string(), expected, "{source_name}");
        }

        for (instant, sorcery) in [(true, false), (false, true)] {
            let mut spell_face = context();
            spell_face.source_is_instant = instant;
            spell_face.source_is_sorcery = sorcery;
            assert!(
                match_cast_method_flashback("Flashback {2}{U}", &spell_face).is_some(),
                "Flashback must accept an instant/sorcery face"
            );
        }
        let mut permanent = context();
        permanent.source_is_instant = false;
        permanent.source_is_sorcery = false;
        assert!(
            match_clause("Flashback {2}{U}", false, &permanent)
                .expect("permanent-face check must not be ambiguous")
                .is_none(),
            "Flashback must fail closed on a permanent face"
        );
        assert!(match_cast_method_flashback("Flashback {2}{U}", &permanent).is_none());
    }

    #[test]
    fn issue_329_cast_method_recipes_reject_near_misses() {
        for negative in [
            "Warp {1}{G} with a rider",
            "Warp — {1}{G}",
            "Warp  {1}{G}",
            "Warp {1}{G}.",
            "Warp {1}{G} and draw a card.",
            "Warp {01}{G}",
            "Warp {1}{G",
            "{1}{G}: Warp this creature.",
        ] {
            assert!(
                match_cast_method_warp(negative, &context()).is_none(),
                "Warp accepted near-miss {negative:?}"
            );
            issue_318_assert_unmatched(negative, false);
        }

        for negative in [
            "Flashback {2}{U} with a rider",
            "Flashback — {2}{U}",
            "Flashback  {2}{U}",
            "Flashback {2}{U}.",
            "Flashback {2}{U}. Draw a card.",
            "Flashback 2",
            "Flashback {02}{U}",
            "Flashback {2}{U",
        ] {
            assert!(
                match_cast_method_flashback(negative, &context()).is_none(),
                "Flashback accepted near-miss {negative:?}"
            );
            issue_318_assert_unmatched(negative, true);
        }

        // Each cast-method line must be claimed only by its own recipe.
        assert!(match_cast_method_warp("Flashback {1}{G}", &context()).is_none());
        assert!(match_cast_method_flashback("Warp {1}{G}", &context()).is_none());
        assert_eq!(
            match_clause("Flashback {1}{G}", false, &context())
                .expect("cross-method check must not be ambiguous")
                .expect("the Flashback line matches its own recipe")
                .id
                .as_str(),
            "cast_method.flashback"
        );
        assert_eq!(
            match_clause("Warp {1}{G}", true, &context())
                .expect("cross-method check must not be ambiguous")
                .expect("the Warp line matches its own recipe")
                .id
                .as_str(),
            "cast_method.warp"
        );
    }

    #[test]
    fn issue_329_cast_method_clauses_do_not_disturb_shipped_recipes() {
        for (clause, is_spell, expected) in [
            ("Draw a card.", true, "spell.draw.fixed"),
            ("Flying", false, "keyword.supported_set"),
            ("Haste", false, "keyword.supported_set"),
            (
                "When this creature enters, you gain 2 life.",
                false,
                "etb.gain_life.fixed",
            ),
            (
                "When this creature enters, surveil 2.",
                false,
                "etb.surveil.two",
            ),
        ] {
            let matched = match_clause(clause, is_spell, &context())
                .unwrap_or_else(|ambiguity| panic!("{clause}: {ambiguity}"))
                .unwrap_or_else(|| panic!("{clause} should stay consumed by its shipped recipe"));
            assert_eq!(matched.id.as_str(), expected, "{clause}");
        }
    }

    // Adjudicated deviation from the issue text: the printed grammar is `{<mana cost>}`, and
    // `{2}` is a well-formed generic mana cost exactly like the accepted `Warp {3}` positive.
    // The issue's negative list wrongly named `Flashback {2}`; it is not a near-miss and must
    // keep matching. `Flashback 2` (no braces) and `Flashback {02}` remain the real negatives.
    #[test]
    fn issue_329_generic_only_cast_method_costs_are_accepted() {
        let warp = match_cast_method_warp("Warp {3}", &context())
            .expect("generic-only Warp cost must match");
        let RecipeEmission::WarpCost(warp_cost) = warp else {
            panic!("Warp must emit the first-class face cost emission");
        };
        assert_eq!(warp_cost.to_string(), "{3}");

        let flashback = match_cast_method_flashback("Flashback {2}", &context())
            .expect("generic-only Flashback cost must match");
        let RecipeEmission::FlashbackCost(flashback_cost) = flashback else {
            panic!("Flashback must emit the first-class face cost emission");
        };
        assert_eq!(flashback_cost.to_string(), "{2}");
    }

    #[test]
    fn issue_333_six_recipe_templates_match_their_exact_recipes() {
        for (clause, is_spell, expected) in [
            (
                "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card.",
                true,
                "spell.target_opponent_reveal.discard_nonland",
            ),
            (
                "Landfall — Whenever a land you control enters, you gain 1 life.",
                false,
                "triggered.landfall.gain_life_one",
            ),
            (
                "Whenever another creature you control enters, this creature gets +1/+1 until end of turn.",
                false,
                "triggered.other_creature_enters.pump_self_plus_one_plus_one",
            ),
            (
                "{T}: Add two mana of any one color.",
                false,
                "activated.tap.add_two_mana_any_one_color",
            ),
            (
                "{T}: Add three mana of any one color.",
                false,
                "activated.tap.add_three_mana_any_one_color",
            ),
            (
                "This creature gets +1/+0 for each artifact you control.",
                false,
                "static.self.count_scaled.artifact.plus_one_zero",
            ),
        ] {
            let matched = match_clause(clause, is_spell, &context())
                .unwrap_or_else(|ambiguity| panic!("{clause}: {ambiguity}"))
                .unwrap_or_else(|| panic!("{clause} should match exactly one recipe"));
            assert_eq!(matched.id.as_str(), expected, "{clause}");
        }
    }

    #[test]
    fn issue_333_recipes_have_stable_ids_and_surfaces() {
        for (id, surface) in [
            (
                "spell.target_opponent_reveal.discard_nonland",
                RecipeSurface::SpellClause,
            ),
            (
                "triggered.landfall.gain_life_one",
                RecipeSurface::TriggeredAbility,
            ),
            (
                "triggered.other_creature_enters.pump_self_plus_one_plus_one",
                RecipeSurface::TriggeredAbility,
            ),
            (
                "activated.tap.add_two_mana_any_one_color",
                RecipeSurface::ActivatedAbility,
            ),
            (
                "activated.tap.add_three_mana_any_one_color",
                RecipeSurface::ActivatedAbility,
            ),
            (
                "static.self.count_scaled.artifact.plus_one_zero",
                RecipeSurface::StaticAbility,
            ),
        ] {
            assert_eq!(
                issue_318_recipe(id).surface,
                surface,
                "{id} surface drifted"
            );
        }
    }

    #[test]
    fn issue_333_target_opponent_reveal_discard_is_exact_and_typed() {
        for source_name in ["Pilfer", "Render Speechless", "Dark Inquiry"] {
            let matched =
                issue_318_match_spell(ISSUE_333_TARGET_OPPONENT_REVEAL_DISCARD_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "spell.target_opponent_reveal.discard_nonland"
            );
            let RecipeEmission::SpellEffectsWithTargeting { effects, targeting } = matched.emission
            else {
                panic!("{source_name} must emit an explicitly targeted spell");
            };
            assert_eq!(
                effects,
                vec![SpellEffectKind::ChooseHandCards {
                    action: HandCardAction::Discard,
                    count: 1,
                    target: TargetFilter {
                        kind: TargetKind::OpponentPlayer,
                        ..TargetFilter::default()
                    },
                    chooser: HandCardChooser::Controller,
                    card_filter: Some(CardTypeFilter::Nonland),
                    optional: false,
                    visibility: HandChoiceVisibility::PublicReveal,
                }]
            );
            let [group] = targeting.groups.as_slice() else {
                panic!("{source_name} must own exactly one target group");
            };
            assert_eq!((group.min, group.max), (1, 1));
            assert_eq!(group.prompt, "Choose target opponent");
            assert_eq!(group.effect_indices, vec![0]);
            assert!(group.distinct_from.is_empty());
            assert!(!group.same_graveyard);
            assert!(group.cast_cost_expansion.is_none());
        }

        assert!(
            match_clause(
                ISSUE_333_TARGET_OPPONENT_REVEAL_DISCARD_CLAUSE,
                false,
                &context()
            )
            .expect("non-spell surface check must not be ambiguous")
            .is_none(),
            "the reveal-discard template is a spell clause and must not match permanent text"
        );
    }

    #[test]
    fn issue_333_landfall_gain_life_is_exact_and_typed() {
        for source_name in [
            "Eumidian Terrabotanist",
            "Jaddi Offshoot",
            "Kazandu Nectarpot",
        ] {
            let matched =
                issue_318_match_non_spell(ISSUE_333_LANDFALL_GAIN_LIFE_CLAUSE, source_name);
            assert_eq!(matched.id.as_str(), "triggered.landfall.gain_life_one");
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("{source_name} must emit a triggered ability");
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverPermanentEntersBattlefield {
                    controller: CastTriggerPlayer::Controller,
                    filter: PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Land),
                        ..PermanentEventFilter::default()
                    },
                    creature_filter: None,
                }
            );
            assert_eq!(
                ability.effect,
                [SpellEffectKind::GainLife {
                    amount: Amount::Fixed(1),
                }]
            );
            assert!(!ability.may);
            assert!(ability.targeting.is_none());
            assert!(ability.intervening_if.is_none());
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_333_LANDFALL_GAIN_LIFE_CLAUSE, false, &noncreature)
                .expect("source-kind check must not be ambiguous")
                .is_none(),
            "the landfall template must remain bound to creature sources"
        );
    }

    #[test]
    fn issue_333_other_creature_enters_pump_is_exact_and_typed() {
        for source_name in ["Loporrit Scout", "Griffin Protector", "Kinsbaile Aspirant"] {
            let matched =
                issue_318_match_non_spell(ISSUE_333_OTHER_CREATURE_ENTERS_PUMP_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "triggered.other_creature_enters.pump_self_plus_one_plus_one"
            );
            let RecipeEmission::TriggeredAbility(ability) = matched.emission else {
                panic!("{source_name} must emit a triggered ability");
            };
            assert_eq!(
                ability.trigger,
                TriggerCondition::WheneverPermanentEntersBattlefield {
                    controller: CastTriggerPlayer::Controller,
                    filter: PermanentEventFilter {
                        permanent_type: Some(PermanentTypeFilter::Creature),
                        exclude_source: true,
                        ..PermanentEventFilter::default()
                    },
                    creature_filter: None,
                }
            );
            assert_eq!(
                ability.effect,
                [SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 1,
                    scale: None,
                    subject: EffectSubject::Source,
                }]
            );
            assert!(!ability.may);
            assert!(ability.targeting.is_none());
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(
                ISSUE_333_OTHER_CREATURE_ENTERS_PUMP_CLAUSE,
                false,
                &noncreature
            )
            .expect("source-kind check must not be ambiguous")
            .is_none(),
            "the creature-enters pump template must remain bound to creature sources"
        );
    }

    fn assert_any_one_color_options(options: &[ManaAmount], per_color: u32) {
        assert_eq!(
            options,
            [
                ManaAmount {
                    w: per_color,
                    ..ManaAmount::default()
                },
                ManaAmount {
                    u: per_color,
                    ..ManaAmount::default()
                },
                ManaAmount {
                    b: per_color,
                    ..ManaAmount::default()
                },
                ManaAmount {
                    r: per_color,
                    ..ManaAmount::default()
                },
                ManaAmount {
                    g: per_color,
                    ..ManaAmount::default()
                },
            ]
        );
    }

    #[test]
    fn issue_333_two_and_three_mana_any_one_color_are_exact_and_typed() {
        for (clause, source_name, expected_id, per_color) in [
            (
                ISSUE_333_TWO_MANA_ANY_ONE_COLOR_CLAUSE,
                "Transdimensional Bovine",
                "activated.tap.add_two_mana_any_one_color",
                2,
            ),
            (
                ISSUE_333_TWO_MANA_ANY_ONE_COLOR_CLAUSE,
                "Khalni Gem",
                "activated.tap.add_two_mana_any_one_color",
                2,
            ),
            (
                ISSUE_333_TWO_MANA_ANY_ONE_COLOR_CLAUSE,
                "Zaxara, the Exemplary",
                "activated.tap.add_two_mana_any_one_color",
                2,
            ),
            (
                ISSUE_333_THREE_MANA_ANY_ONE_COLOR_CLAUSE,
                "Gilded Lotus",
                "activated.tap.add_three_mana_any_one_color",
                3,
            ),
            (
                ISSUE_333_THREE_MANA_ANY_ONE_COLOR_CLAUSE,
                "Lotus Field",
                "activated.tap.add_three_mana_any_one_color",
                3,
            ),
            (
                ISSUE_333_THREE_MANA_ANY_ONE_COLOR_CLAUSE,
                "Coveted Jewel",
                "activated.tap.add_three_mana_any_one_color",
                3,
            ),
        ] {
            let matched = issue_318_match_non_spell(clause, source_name);
            assert_eq!(matched.id.as_str(), expected_id, "{source_name}");
            let RecipeEmission::ActivatedAbility(ability) = matched.emission else {
                panic!("{source_name} must emit an activated ability");
            };
            assert_eq!(ability.costs, vec![AbilityCost::Tap]);
            assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
            assert_eq!(ability.timing, ActivationTiming::Normal);
            assert!(ability.targeting.is_none());
            assert!(ability.activation_limit.is_none());
            let [SpellEffectKind::ProduceMana {
                options,
                restriction,
                conditional,
            }] = ability.effect.as_slice()
            else {
                panic!("{source_name} must produce mana as its sole effect");
            };
            assert!(restriction.is_none());
            assert!(conditional.is_none());
            assert_any_one_color_options(options, per_color);
        }
    }

    #[test]
    fn issue_333_count_scaled_artifact_is_exact_and_typed() {
        for source_name in ["Guidelight Synergist", "Nim Lasher", "Storm-Kiln Artist"] {
            let matched =
                issue_318_match_non_spell(ISSUE_333_COUNT_SCALED_ARTIFACT_CLAUSE, source_name);
            assert_eq!(
                matched.id.as_str(),
                "static.self.count_scaled.artifact.plus_one_zero"
            );
            let RecipeEmission::StaticAbility(ability) = matched.emission else {
                panic!("{source_name} must emit a static ability");
            };
            assert_eq!(
                ability.definition,
                StaticAbilityDef::CountScaledSelfPt {
                    count: CountExpression::BattlefieldPermanents {
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
                    },
                    power_per_match: 1,
                    toughness_per_match: 0,
                }
            );
        }

        let mut noncreature = context();
        noncreature.source_is_creature = false;
        assert!(
            match_clause(ISSUE_333_COUNT_SCALED_ARTIFACT_CLAUSE, false, &noncreature)
                .expect("source-kind check must not be ambiguous")
                .is_none(),
            "the count-scaled template must remain bound to creature sources"
        );
    }

    #[test]
    fn issue_333_recipes_reject_near_misses() {
        for negative in [
            "Target player reveals their hand. You choose a nonland card from it. That player discards that card.",
            "Target opponent reveals their hand. You choose a card from it. That player discards that card.",
            "Target opponent reveals their hand. You choose a nonland card from it. That player exiles that card.",
            "Target opponent reveals their hand. You choose two nonland cards from it. That player discards those cards.",
            "Target opponent reveals their hand. You may choose a nonland card from it. That player discards that card.",
            "Target opponent reveals their hand. You choose a nonland card from it. That player discards that card. Draw a card.",
            "Target opponent reveals their hand. You choose a nonland card from it.",
        ] {
            assert!(
                match_spell_target_opponent_reveal_discard_nonland(negative, &context()).is_none(),
                "reveal-discard accepted near-miss {negative:?}"
            );
            issue_318_assert_unmatched(negative, true);
        }

        for negative in [
            "Landfall — Whenever a land you control enters, you gain 2 life.",
            "Landfall — Whenever a land you control enters, draw a card.",
            "Whenever a land you control enters, you gain 1 life.",
            "Landfall — Whenever a land enters, you gain 1 life.",
            "Landfall — Whenever a land you control enters, you gain 1 life and draw a card.",
            "Landfall — Whenever a land you control enters, each opponent loses 1 life.",
        ] {
            assert!(
                match_landfall_gain_life_one(negative, &context()).is_none(),
                "landfall accepted near-miss {negative:?}"
            );
            issue_318_assert_unmatched(negative, false);
        }

        for negative in [
            "Whenever a creature you control enters, this creature gets +1/+1 until end of turn.",
            "Whenever another creature you control enters, this creature gets +2/+2 until end of turn.",
            "Whenever another creature enters, this creature gets +1/+1 until end of turn.",
            "Whenever another creature you control enters, put a +1/+1 counter on this creature.",
            "Whenever another creature you control enters, this creature gets +1/+1.",
            "Alliance — Whenever another creature you control enters, this creature gets +1/+1 until end of turn.",
            "Whenever another creature an opponent controls enters, this creature gets +1/+1 until end of turn.",
        ] {
            assert!(
                match_other_creature_enters_pump_self_plus_one_plus_one(negative, &context())
                    .is_none(),
                "creature-enters pump accepted near-miss {negative:?}"
            );
            issue_318_assert_unmatched(negative, false);
        }

        for negative in [
            "{1}, {T}: Add two mana of any one color.",
            "{T}, Sacrifice this artifact: Add two mana of any one color.",
            "{T}: Add two mana of any color.",
            "{T}: Add {G}{G}.",
            "{T}: Add two mana of any one color. Spend this mana only to cast artifact spells.",
            "{T}: Add two mana in any combination of colors.",
            "{T}: Add four mana of any one color.",
        ] {
            assert!(
                match_tap_for_two_mana_any_one_color(negative, &context()).is_none(),
                "two-mana recipe accepted near-miss {negative:?}"
            );
            issue_318_assert_unmatched(negative, false);
        }

        for negative in [
            "{T}, Sacrifice this artifact: Add three mana of any one color.",
            "{T}: Add three mana of any color.",
            "{T}: Add three mana in any combination of colors.",
            "{T}: Add {C}{C}{C}.",
            "{T}: Add three mana of any one color. Spend this mana only to cast artifact spells.",
            "{2}, {T}: Add three mana of any one color.",
        ] {
            assert!(
                match_tap_for_three_mana_any_one_color(negative, &context()).is_none(),
                "three-mana recipe accepted near-miss {negative:?}"
            );
            issue_318_assert_unmatched(negative, false);
        }

        for negative in [
            "This creature gets +1/+1 for each artifact you control.",
            "This creature gets +1/+0 for each creature you control.",
            "This creature gets +2/+0 for each artifact you control.",
            "Equipped creature gets +1/+0 for each artifact you control.",
            "This creature gets +1/+0 for each artifact an opponent controls.",
            "This creature gets +1/+0 for each artifact you control as long as you control a Robot.",
            "This creature gets +0/+1 for each artifact you control.",
        ] {
            assert!(
                match_static_self_count_scaled_artifact_plus_one_zero(negative, &context())
                    .is_none(),
                "count-scaled recipe accepted near-miss {negative:?}"
            );
            issue_318_assert_unmatched(negative, false);
        }
    }

    #[test]
    fn issue_333_recipes_do_not_disturb_shipped_recipes() {
        for (clause, is_spell, expected) in [
            (
                "{T}: Add one mana of any color.",
                false,
                "activated.mana.tap_any_color",
            ),
            ("{T}: Add {G}.", false, "activated.mana.tap_one"),
            (
                "{T}: Add {B} or {R}.",
                false,
                "activated.mana.tap_two_or_three_colors",
            ),
            (
                "Landfall — Whenever a land you control enters, mill a card.",
                false,
                "triggered.landfall.mill.one",
            ),
            (
                "Whenever another creature you control enters, you gain 1 life.",
                false,
                "triggered.other_controlled_creature_etb.gain_life.one",
            ),
            ("Draw a card.", true, "spell.draw.fixed"),
        ] {
            let matched = match_clause(clause, is_spell, &context())
                .unwrap_or_else(|ambiguity| panic!("{clause}: {ambiguity}"))
                .unwrap_or_else(|| panic!("{clause} should stay consumed by its shipped recipe"));
            assert_eq!(matched.id.as_str(), expected, "{clause}");
        }

        // Each multi-mana template is claimed only by its own recipe, and neither claims the
        // shipped one-mana any-color template.
        assert!(match_tap_for_two_mana_any_one_color(
            ISSUE_333_THREE_MANA_ANY_ONE_COLOR_CLAUSE,
            &context()
        )
        .is_none());
        assert!(match_tap_for_three_mana_any_one_color(
            ISSUE_333_TWO_MANA_ANY_ONE_COLOR_CLAUSE,
            &context()
        )
        .is_none());
        assert!(match_tap_for_two_mana_any_one_color(
            "{T}: Add one mana of any color.",
            &context()
        )
        .is_none());
        assert!(match_tap_for_three_mana_any_one_color(
            "{T}: Add one mana of any color.",
            &context()
        )
        .is_none());
        assert!(match_landfall_gain_life_one(
            "Landfall — Whenever a land you control enters, mill a card.",
            &context()
        )
        .is_none());
    }
}
