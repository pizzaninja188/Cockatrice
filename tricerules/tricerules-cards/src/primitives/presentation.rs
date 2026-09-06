//! Presentation-only summaries of completely understood simple instructions.
//!
//! Forest/Llanowar Elves and Clue/Food tokens share these descriptions. Unknown pieces return
//! None as a unit: a plausible but incomplete sentence is worse than an identified fallback.

use super::{
    AbilityCost, Amount, EffectSubject, LibraryPartitionKind, LifeAmount, ManaAmount,
    ManaRetention, PlayerRecipient, SpellEffectKind, TargetController, TargetFilter, TargetKind,
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

fn simple_target(filter: &TargetFilter) -> Option<String> {
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
    let simple = TargetFilter {
        kind: filter.kind.clone(),
        controller: filter.controller,
        ..TargetFilter::default()
    };
    (*filter == simple).then(|| format!("{noun}{controller}"))
}

pub(super) fn simple_costs(costs: &[AbilityCost], source: &str) -> Option<String> {
    costs
        .iter()
        .map(|cost| {
            Some(match cost {
                AbilityCost::Tap => "{T}".into(),
                AbilityCost::Mana(mana) => mana.to_string(),
                AbilityCost::SacrificeSelf => format!("Sacrifice {source}"),
                AbilityCost::Discard => "Discard a card".into(),
                AbilityCost::DiscardSelf => format!("Discard {source}"),
                AbilityCost::ExileSelf => format!("Exile {source}"),
                AbilityCost::PayLife { amount } => format!("Pay {amount} life"),
                AbilityCost::Loyalty(amount) => format!("{amount:+}"),
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
                    options,
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
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                } => "Draw a card.".into(),
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(count),
                } => format!("Draw {count} cards."),
                SpellEffectKind::GainLife {
                    amount: Amount::Fixed(amount),
                } => format!("You gain {amount} life."),
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
                    count: 1,
                } => "Discard a card.".into(),
                SpellEffectKind::Discard {
                    who: PlayerRecipient::Controller,
                    count,
                } => format!("Discard {count} cards."),
                SpellEffectKind::Blight { count } => format!("Blight {count}."),
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
                _ => return None,
            })
        })
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join(" "))
}
