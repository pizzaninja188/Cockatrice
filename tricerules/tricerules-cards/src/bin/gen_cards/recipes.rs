use tricerules_cards::primitives::{
    CardTypeFilter, DiscardQuantity, EffectSubject, EntersTappedAffected, EntryCost,
    PlayerRecipient, SpellCastFilter, StackSpellFilter, StaticAbilityDef, TargetFilter,
};
use tricerules_cards::{
    AbilityCost, AbilityId, AbilityPresentation, AbilitySourceZone, ActivatedAbilityDef,
    ActivationTiming, Amount, CastTriggerPlayer, IdentifiedAbility, Keyword, ManaAmount,
    SpellEffectKind, TriggerCondition, TriggeredAbilityDef,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RecipeSurface {
    KeywordClause,
    SpellClause,
    EtbAbility,
    TriggeredAbility,
    ActivatedAbility,
    StaticAbility,
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
    pub(super) presentation: AbilityPresentation,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RecipeEmission {
    Keywords(Vec<Keyword>),
    SpellEffect(SpellEffectKind),
    TriggeredAbility(TriggeredAbilityDef),
    ActivatedAbility(ActivatedAbilityDef),
    StaticAbility(IdentifiedAbility<StaticAbilityDef>),
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
    RecipeEmission::TriggeredAbility(TriggeredAbilityDef {
        ability_id: context.triggered_ability_id.clone(),
        presentation: context.presentation.clone(),
        trigger: TriggerCondition::WhenSelfEntersBattlefield,
        effect: vec![effect],
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
            "Ward {2}",
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
        id: RecipeId("static.enters_tapped.unless_pay_life_2"),
        label: "shockland entry payment",
        surface: RecipeSurface::StaticAbility,
        matcher: match_shockland_entry_payment,
        calibration: calibrations!(
            "Blood Crypt" => "As this land enters, you may pay 2 life. If you don't, it enters tapped.",
            "Breeding Pool" => "As this land enters, you may pay 2 life. If you don't, it enters tapped.";
            "This land enters tapped.",
            "This land enters tapped unless you control two or more other lands.",
            "As this land enters, you may pay 3 life. If you don't, it enters tapped.",
            "As this land enters, you may pay 2 life. If you don't, it enters tapped. When this land enters, draw a card."
        ),
    },
];

fn surface_applies(surface: RecipeSurface, is_spell: bool) -> bool {
    match surface {
        RecipeSurface::KeywordClause => true,
        RecipeSurface::SpellClause => is_spell,
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
        presentation: AbilityPresentation::OracleLines(vec![1]),
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
            presentation: AbilityPresentation::OracleLines(vec![1]),
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
}
