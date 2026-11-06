//! Presentation-only summaries of completely understood simple instructions.
//!
//! Forest/Llanowar Elves and Clue/Food tokens share these descriptions. Unknown pieces return
//! None as a unit: a plausible but incomplete sentence is worse than an identified fallback.

use super::{
    AbilityCost, Amount, CardTypeFilter, EffectSubject, GraveyardFilter, GraveyardOwner,
    LibraryPartitionKind, LifeAmount, ManaAmount, ManaRetention, PlayerRecipient, SpellEffectKind,
    StackSpellFilter, TargetBinding, TargetChooser, TargetController, TargetFilter, TargetGroupDef,
    TargetKind, TargetRole, ZoneCardFilter,
};
use std::borrow::Cow;

fn mana_text(amount: &ManaAmount) -> String {
    [
        (amount.w, "W"),
        (amount.u, "U"),
        (amount.b, "B"),
        (amount.r, "R"),
        (amount.g, "G"),
        (amount.c, "C"),
    ]
    .into_iter()
    .filter(|(count, _)| *count != 0)
    .map(|(count, symbol)| {
        // Keep descriptions bounded even for a synthetic ability with a large mana amount.
        if count <= 5 {
            format!("{{{symbol}}}").repeat(count as usize)
        } else {
            format!("{count} {{{symbol}}}")
        }
    })
    .collect()
}

fn mana_color_symbol(color: super::Color) -> &'static str {
    match color {
        super::Color::White => "W",
        super::Color::Blue => "U",
        super::Color::Black => "B",
        super::Color::Red => "R",
        super::Color::Green => "G",
    }
}

fn simple_target(filter: &TargetFilter) -> Option<String> {
    if !filter.permanent_types.is_empty() {
        return None;
    }
    let (noun, suffix) = battlefield_target_parts(filter)?;
    Some(format!("{noun}{suffix}"))
}

fn battlefield_target_parts(filter: &TargetFilter) -> Option<(Cow<'static, str>, &'static str)> {
    let noun = match (filter.kind, filter.permanent_types.as_slice()) {
        (TargetKind::AnyTarget, []) => "any target".into(),
        (TargetKind::Creature, []) => "target creature".into(),
        (TargetKind::AnyPlayer, []) => "target player".into(),
        (TargetKind::OpponentPlayer, []) => "target opponent".into(),
        (TargetKind::AnyPermanent, []) => "target permanent".into(),
        (TargetKind::AnyPermanent, [card_type]) => {
            format!("target {}", card_type.as_str().to_ascii_lowercase()).into()
        }
        _ => return None,
    };
    let controller = match (&filter.kind, filter.controller) {
        (_, TargetController::Any) => "",
        (TargetKind::Creature | TargetKind::AnyPermanent, TargetController::You) => " you control",
        (TargetKind::Creature | TargetKind::AnyPermanent, TargetController::Opponent) => {
            " an opponent controls"
        }
        _ => return None,
    };
    // Do not silently drop power, subtype, exclusion, or other constraints from a label.
    let simple = TargetFilter {
        kind: filter.kind,
        controller: filter.controller,
        permanent_types: filter.permanent_types.clone(),
        ..TargetFilter::default()
    };
    (*filter == simple).then_some((noun, controller))
}

pub(super) fn card_type_label(card_type: CardTypeFilter) -> &'static str {
    match card_type {
        CardTypeFilter::BasicLand => "basic land",
        CardTypeFilter::Land => "land",
        CardTypeFilter::Enchantment => "enchantment",
        CardTypeFilter::Instant => "instant",
        CardTypeFilter::Sorcery => "sorcery",
        CardTypeFilter::InstantOrSorcery => "instant or sorcery",
        CardTypeFilter::Creature => "creature",
        CardTypeFilter::Artifact => "artifact",
        CardTypeFilter::Planeswalker => "planeswalker",
        CardTypeFilter::Battle => "battle",
        CardTypeFilter::Nonland => "nonland",
        CardTypeFilter::NonlandPermanent => "nonland permanent",
        CardTypeFilter::Noncreature => "noncreature",
    }
}

fn typed_target(card_type: Option<CardTypeFilter>, noun: &str) -> String {
    match card_type {
        Some(card_type) => format!("target {} {noun}", card_type_label(card_type)),
        None => format!("target {noun}"),
    }
}

fn target_prompt_parts(
    role: TargetRole<'_>,
    same_graveyard: bool,
) -> Option<(Cow<'static, str>, &'static str)> {
    match role {
        TargetRole::Filtered(filter) => battlefield_target_parts(filter),
        TargetRole::CreaturePermanent => Some(("target creature".into(), "")),
        TargetRole::StackAbility => Some(("target activated or triggered ability".into(), "")),
        TargetRole::StackSpell(filter) => {
            let simple = StackSpellFilter {
                card_type: filter.card_type,
                ..StackSpellFilter::default()
            };
            (*filter == simple).then(|| (typed_target(filter.card_type, "spell").into(), ""))
        }
        TargetRole::GraveyardCard(filter) => {
            let card_type = filter.card.as_ref().and_then(|card| card.card_type);
            let simple = GraveyardFilter {
                owner: filter.owner,
                card: filter.card.as_ref().map(|_| ZoneCardFilter {
                    card_type,
                    ..ZoneCardFilter::default()
                }),
                ..GraveyardFilter::default()
            };
            let suffix = match (filter.owner, same_graveyard) {
                (GraveyardOwner::Controller, _) => " from your graveyard",
                (GraveyardOwner::AnyPlayer, false) => " from a graveyard",
                (GraveyardOwner::AnyPlayer, true) => " from a single graveyard",
                (GraveyardOwner::Opponent, false) => " from an opponent's graveyard",
                (GraveyardOwner::Opponent, true) => " from a single opponent's graveyard",
            };
            (*filter == simple).then(|| (typed_target(card_type, "card").into(), suffix))
        }
    }
}

pub(super) fn target_group_prompt<'a>(
    group: &'a TargetGroupDef,
    bindings: &[TargetBinding<'_>],
) -> Result<Cow<'a, str>, String> {
    if !group.prompt.trim().is_empty() {
        return Ok(Cow::Borrowed(&group.prompt));
    }
    let explicit_prompt_required =
        || "target group requires an explicit prompt for its constraints".into();
    let [first, rest @ ..] = bindings else {
        return Err(explicit_prompt_required());
    };
    if group.chooser != TargetChooser::Controller
        || group.for_each_opponent
        || !group.distinct_from.is_empty()
        || group.cast_cost_expansion.is_some()
        || rest.iter().any(|binding| binding.role != first.role)
    {
        return Err(explicit_prompt_required());
    }
    let (noun, suffix) = target_prompt_parts(first.role, group.same_graveyard)
        .ok_or_else(explicit_prompt_required)?;
    if (group.min, group.max) == (1, 1) {
        return Ok(format!("Choose {noun}{suffix}").into());
    }
    let quantity = match (group.min, group.max) {
        (0, 1) => "up to one".into(),
        (0, 2) => "up to two".into(),
        (0, 3) => "up to three".into(),
        (1, 2) => "one or two".into(),
        (0, max) => Cow::Owned(format!("up to {max}")),
        (min, max) if min == max => Cow::Owned(min.to_string()),
        (min, max) => Cow::Owned(format!("{min}–{max}")),
    };
    let noun = noun.strip_prefix("any ").unwrap_or(&noun);
    let noun = match (group.max, first.role) {
        (1, _) => Cow::Borrowed(noun),
        (_, TargetRole::StackAbility) => Cow::Borrowed("target activated or triggered abilities"),
        _ => Cow::Owned(format!("{noun}s")),
    };
    Ok(format!("Choose {quantity} {noun}{suffix}").into())
}

pub(super) fn simple_costs(costs: &[AbilityCost], source: &str) -> Option<String> {
    costs
        .iter()
        .map(|cost| {
            Some(match cost {
                AbilityCost::Tap => "{T}".into(),
                AbilityCost::Mana(mana) => mana.to_string(),
                AbilityCost::RemoveXStorageCountersFromSource => {
                    format!("Remove X storage counters from {source}")
                }
                AbilityCost::SacrificeSelf => format!("Sacrifice {source}"),
                AbilityCost::Discard => "Discard a card".into(),
                AbilityCost::DiscardCard { filter } => format!("Discard a {}", filter.noun()),
                AbilityCost::DiscardSelf => format!("Discard {source}"),
                AbilityCost::ExileSelf => format!("Exile {source}"),
                AbilityCost::PayLife { amount } => format!("Pay {amount} life"),
                AbilityCost::PayCommanderColorIdentityLife => {
                    "Pay life equal to the number of colors in your commanders' color identity"
                        .into()
                }
                AbilityCost::Loyalty(amount) => format!("{amount:+}"),
                AbilityCost::LoyaltyX => "Loyalty X".into(),
                _ => return None,
            })
        })
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join(", "))
}

pub(super) fn simple_effects(effects: &[SpellEffectKind]) -> Option<String> {
    if effects.is_empty() {
        return None;
    }
    effects
        .iter()
        .map(|effect| {
            Some(match effect {
                SpellEffectKind::ProduceMana {
                    commander_color_identity: true,
                    ..
                } => "Add one mana of any color in your commander's color identity.".into(),
                SpellEffectKind::ProduceManaFromOpponentLands { .. } => {
                    "Add one mana of any color a land an opponent controls could produce.".into()
                }
                SpellEffectKind::ProduceMana {
                    options,
                    commander_color_identity: false,
                    restriction,
                    conditional: None,
                } if !options.is_empty() => {
                    let any_color = options.len() == 5
                        && ['W', 'U', 'B', 'R', 'G'].into_iter().all(|color| {
                            options
                                .iter()
                                .any(|amount| mana_text(amount) == format!("{{{color}}}"))
                        });
                    let output = if any_color {
                        "one mana of any color".into()
                    } else {
                        options
                            .iter()
                            .map(mana_text)
                            .collect::<Vec<_>>()
                            .join(" or ")
                    };
                    let mut text = format!("Add {output}.");
                    if let Some(restriction) = restriction {
                        text.push_str(&format!(" {}.", restriction.fallback_label()));
                    }
                    text
                }
                SpellEffectKind::ProduceManaPerSourceCounter { counter, options }
                    if !options.is_empty() =>
                {
                    let any_color = options.len() == 5
                        && ['W', 'U', 'B', 'R', 'G'].into_iter().all(|color| {
                            options
                                .iter()
                                .any(|amount| mana_text(amount) == format!("{{{color}}}"))
                        });
                    if any_color {
                        format!(
                            "Choose a color. Add one mana of that color for each {} counter on this source.",
                            counter.label()
                        )
                    } else {
                        let output =
                        options
                            .iter()
                            .map(mana_text)
                            .collect::<Vec<_>>()
                            .join(" or ");
                        format!("Add {output} for each {} counter on this source.", counter.label())
                    }
                }
                SpellEffectKind::ProduceSplitManaFromRemovedStorageCounters {
                    first_color,
                    second_color,
                } => format!(
                    "Add X mana in any combination of {{{}}} and {{{}}}.",
                    mana_color_symbol(*first_color),
                    mana_color_symbol(*second_color)
                ),
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                } => "Draw a card.".into(),
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(count),
                } => format!("Draw {count} cards."),
                SpellEffectKind::Draw {
                    who: PlayerRecipient::AffectedPlayer,
                    count: Amount::Fixed(0),
                } => "The affected player draws no cards.".into(),
                SpellEffectKind::Draw {
                    who: PlayerRecipient::AffectedPlayer,
                    count: Amount::Fixed(1),
                } => "The affected player draws a card.".into(),
                SpellEffectKind::Draw {
                    who: PlayerRecipient::AffectedPlayer,
                    count: Amount::Fixed(count),
                } => format!("The affected player draws {count} cards."),
                SpellEffectKind::TakeExtraTurns { count: 1 } => {
                    "Take an extra turn after this one.".into()
                }
                SpellEffectKind::TakeExtraTurns { count } if *count > 1 => {
                    format!("Take {count} extra turns after this one.")
                }
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(amount),
                    who: PlayerRecipient::Controller,
                } => format!("You gain {amount} life."),
                SpellEffectKind::PutPoisonCountersOnTargetPlayer { count, target }
                    if *count > 0 && target.is_player() =>
                {
                    let mut target = simple_target(target)?;
                    target[..1].make_ascii_uppercase();
                    format!("{target} gets {count} poison counter{}.", if *count == 1 { "" } else { "s" })
                }
                SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(amount),
                    target,
                } => {
                    format!("Deal {amount} damage to {}.", simple_target(target)?)
                }
                SpellEffectKind::Explore {
                    subject: EffectSubject::Chosen(target),
                } => {
                    let mut target = simple_target(target)?;
                    target[..1].make_ascii_uppercase();
                    format!("{target} explores.")
                }
                SpellEffectKind::Scry {
                    count: Amount::Fixed(count),
                } => format!("Scry {count}."),
                SpellEffectKind::LibraryPartition {
                    count,
                    top_min: 0,
                    top_max: None,
                    kind: LibraryPartitionKind::Surveil,
                } => format!("Surveil {count}."),
                SpellEffectKind::AddMana { amount, retention } => format!(
                    "Add {}.{}",
                    mana_text(amount),
                    if *retention == ManaRetention::EndOfCombat {
                        " This mana lasts until end of combat."
                    } else {
                        ""
                    }
                ),
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(amount),
                    who: PlayerRecipient::Controller,
                } => format!("You lose {amount} life."),
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(amount),
                    who: PlayerRecipient::EachOpponent,
                } => format!("Each opponent loses {amount} life."),
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(amount),
                    who: PlayerRecipient::EachPlayer,
                } => format!("Each player loses {amount} life."),
                SpellEffectKind::Discard {
                    who: PlayerRecipient::Controller,
                    quantity: crate::primitives::DiscardQuantity::Exact(1),
                } => "Discard a card.".into(),
                SpellEffectKind::Discard {
                    who: PlayerRecipient::Controller,
                    quantity: super::DiscardQuantity::Exact(count),
                } => format!("Discard {count} cards."),
                SpellEffectKind::Discard {
                    who: PlayerRecipient::Controller,
                    quantity: super::DiscardQuantity::All,
                } => "Discard your hand.".into(),
                SpellEffectKind::Discard {
                    who: PlayerRecipient::Controller,
                    quantity: super::DiscardQuantity::UpTo(count),
                } => format!("Discard up to {count} cards."),
                SpellEffectKind::Discard {
                    who: PlayerRecipient::Controller,
                    quantity: super::DiscardQuantity::AnyNumber,
                } => "Discard any number of cards.".into(),
                SpellEffectKind::Proliferate => "Proliferate.".into(),
                SpellEffectKind::Blight { count } => format!("Blight {count}."),
                SpellEffectKind::SearchLibrary { .. } => "Search".into(),
                SpellEffectKind::ReturnAllToOwnersHand { kind } => {
                    let color = kind.is_color?;
                    let exact = TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        is_color: Some(color),
                        ..TargetFilter::default()
                    };
                    if *kind != exact {
                        return None;
                    }
                    let color = match color {
                        super::Color::White => "white",
                        super::Color::Blue => "blue",
                        super::Color::Black => "black",
                        super::Color::Red => "red",
                        super::Color::Green => "green",
                    };
                    format!("Return all {color} permanents to their owners' hands.")
                }
                SpellEffectKind::Sacrifice {
                    subject: EffectSubject::Source,
                } => "Sacrifice this permanent.".into(),
                SpellEffectKind::PutCounters {
                    counter,
                    count: Amount::Fixed(count),
                    subject: EffectSubject::Source,
                } => {
                    if *count == 1 {
                        format!("Put a {} counter on this permanent.", counter.label())
                    } else {
                        format!(
                            "Put {count} {} counters on this permanent.",
                            counter.label()
                        )
                    }
                }
                SpellEffectKind::PutCountersAllPlaneswalkers {
                    counter,
                    count: Amount::Fixed(1),
                    exclude_self: false,
                    subtype: Some(subtype),
                } => format!(
                    "Put a {} counter on each {} planeswalker you control.",
                    counter.label(), subtype.as_str()
                ),
                SpellEffectKind::RemoveCounters {
                    counter,
                    count,
                    subject: EffectSubject::Source,
                } => {
                    if *count == 1 {
                        format!("Remove a {} counter from this permanent.", counter.label())
                    } else {
                        format!(
                            "Remove {count} {} counters from this permanent.",
                            counter.label()
                        )
                    }
                }
                _ => return None,
            })
        })
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join(" "))
}
