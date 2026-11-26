//! Presentation-only summaries of completely understood simple instructions.
//!
//! Forest/Llanowar Elves and Clue/Food tokens share these descriptions. Unknown pieces return
//! None as a unit: a plausible but incomplete sentence is worse than an identified fallback.

use super::{
    AbilityCost, Amount, CardTypeFilter, EffectSubject, LibraryPartitionKind, LifeAmount,
    ManaAmount, ManaRetention, ObjectFilter, PlayerRecipient, SearchDestination,
    SearchZoneSelection, SpellEffectKind, TargetController, TargetKind, ZoneCardFilter,
};

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

fn simple_target(filter: &ObjectFilter) -> Option<String> {
    let noun = match filter.kind {
        TargetKind::AnyTarget => "any target",
        TargetKind::Creature => "target creature",
        TargetKind::AnyPlayer => "target player",
        TargetKind::OpponentPlayer => "target opponent",
        TargetKind::AnyPermanent => "target permanent",
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
    let simple = ObjectFilter {
        kind: filter.kind,
        controller: filter.controller,
        ..ObjectFilter::default()
    };
    (*filter == simple).then(|| format!("{noun}{controller}"))
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

/// A single-card library tutor whose every field is the plain default except the card type and
/// a hand or battlefield destination. Lander and Evolving Wilds style searches; anything else
/// (optional, counted, slotted, revealed, other zones, conditional placement) is None.
fn simple_search(effect: &SpellEffectKind) -> Option<String> {
    let SpellEffectKind::SearchLibrary {
        who: PlayerRecipient::Controller,
        optional: false,
        count: 1,
        count_by_cast_cost: None,
        filter,
        selection_constraint: None,
        slots,
        zones,
        destination,
        conditional_destination: None,
        shuffle: true,
        reveal: false,
        selection_bounds: None,
    } = effect
    else {
        return None;
    };
    if !slots.is_empty() || *zones != SearchZoneSelection::default() {
        return None;
    }
    let card = match filter {
        None => "a card".to_string(),
        Some(filter) => {
            let card_type = filter.card_type?;
            let only_type = ZoneCardFilter {
                card_type: Some(card_type),
                ..ZoneCardFilter::default()
            };
            if *filter != only_type {
                return None;
            }
            let label = card_type_label(card_type);
            let article = if label.starts_with(['a', 'e', 'i', 'o', 'u']) {
                "an"
            } else {
                "a"
            };
            format!("{article} {label} card")
        }
    };
    let placement = match destination {
        SearchDestination::Hand => "put it into your hand",
        SearchDestination::Battlefield { tapped: false } => "put it onto the battlefield",
        SearchDestination::Battlefield { tapped: true } => "put it onto the battlefield tapped",
        _ => return None,
    };
    Some(format!(
        "Search your library for {card}, {placement}, then shuffle."
    ))
}

/// A resolution-branch button label. A branch that only searches is the option "Search" (issue
/// #284): a button names the choice, while ability text must stay a complete sentence.
pub(super) fn branch_label(effects: &[SpellEffectKind]) -> Option<String> {
    if let [SpellEffectKind::SearchLibrary { .. }] = effects {
        return Some("Search".into());
    }
    simple_effects(effects)
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
                SpellEffectKind::Investigate {
                    count: Amount::Fixed(count),
                    who: PlayerRecipient::Controller,
                } => match count {
                    1 => "Investigate.".into(),
                    2 => "Investigate twice.".into(),
                    count => format!("Investigate {count} times."),
                },
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
                SpellEffectKind::SearchLibrary { .. } => simple_search(effect)?,
                SpellEffectKind::ReturnAllToOwnersHand { kind } => {
                    let color = kind.is_color?;
                    let exact = ObjectFilter {
                        kind: TargetKind::AnyPermanent,
                        is_color: Some(color),
                        ..ObjectFilter::default()
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
                // CR 701.19: only creatures can be regenerated, so a Source subject is always
                // "this creature" (printed abilities and Aura-granted ones alike).
                SpellEffectKind::Regenerate {
                    subject: EffectSubject::Source,
                } => "Regenerate this creature.".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn search(ron_text: &str) -> Option<String> {
        let effect: SpellEffectKind = ron::from_str(ron_text).expect("search fixture parses");
        simple_effects(&[effect])
    }

    #[test]
    fn simple_search_renders_a_complete_sentence() {
        assert_eq!(
            search("SearchLibrary(filter: Some((card_type: Some(BasicLand))), destination: Battlefield(tapped: true))"),
            Some("Search your library for a basic land card, put it onto the battlefield tapped, then shuffle.".into())
        );
        assert_eq!(
            search("SearchLibrary(filter: Some((card_type: Some(Artifact))))"),
            Some(
                "Search your library for an artifact card, put it into your hand, then shuffle."
                    .into()
            )
        );
        assert_eq!(
            search("SearchLibrary()"),
            Some("Search your library for a card, put it into your hand, then shuffle.".into())
        );
    }

    #[test]
    fn search_that_cannot_be_fully_described_is_none_not_a_bare_verb() {
        for unsupported in [
            "SearchLibrary(optional: true, filter: Some((card_type: Some(BasicLand))))",
            "SearchLibrary(count: 2, filter: Some((card_type: Some(BasicLand))))",
            "SearchLibrary(reveal: true, filter: Some((card_type: Some(Creature))))",
            "SearchLibrary(shuffle: false)",
            "SearchLibrary(destination: Graveyard)",
            "SearchLibrary(destination: TopOfLibrary)",
            "SearchLibrary(filter: Some((card_type: Some(Creature), max_mana_value: Some(3))))",
            "SearchLibrary(filter: Some((required_subtypes: [\"Forest\"])))",
        ] {
            assert_eq!(search(unsupported), None, "{unsupported}");
        }
    }
}
