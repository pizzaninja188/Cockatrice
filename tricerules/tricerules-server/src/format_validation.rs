//! Pure deck construction validation. Executable-card support is an independent result.
use crate::format_catalog::{normalize, Card, CardLegality, Catalog, Partner};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Standard,
    Commander,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Legality {
    Legal,
    Illegal,
    Unknown,
}
#[derive(Default, Clone, Debug)]
pub struct Deck {
    pub mainboard: Vec<String>,
    pub sideboard: Vec<String>,
    pub commanders: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct Problem {
    pub code: String,
    pub section: String,
    pub card_name: String,
    pub quantity: u32,
    pub message: String,
    pub unknown: bool,
}
#[derive(Clone, Debug)]
pub struct Report {
    pub legality: Legality,
    pub engine_supported: bool,
    pub unsupported_cards: Vec<String>,
    pub problems: Vec<Problem>,
}
impl Report {
    fn issue(
        &mut self,
        code: &str,
        section: &str,
        card: &str,
        quantity: usize,
        message: String,
        unknown: bool,
    ) {
        self.problems.push(Problem {
            code: code.into(),
            section: section.into(),
            card_name: card.into(),
            quantity: quantity as u32,
            message,
            unknown,
        });
    }
}
fn compatible(a: &Card, b: &Card) -> bool {
    a.partners.iter().any(|left| match left {
        Partner::Ordinary => a.legendary && b.legendary && b.partners.contains(left),
        Partner::Family(_) => a.legendary && b.legendary && b.partners.contains(left),
        Partner::Named(other) => {
            a.legendary
                && b.legendary
                && *other == normalize(&b.name)
                && b.partners.contains(&Partner::Named(normalize(&a.name)))
        }
        Partner::Background => a.legendary && b.background,
        Partner::Doctor => a.legendary && a.creature && b.doctor,
    }) || b.partners.iter().any(|right| {
        matches!(right, Partner::Background) && b.legendary && a.background
            || matches!(right, Partner::Doctor) && b.legendary && b.creature && a.doctor
    })
}
pub fn validate(
    catalog: &Catalog,
    format: Format,
    deck: &Deck,
    implemented: impl Fn(&str) -> bool,
) -> Report {
    let mut report = Report {
        legality: Legality::Legal,
        engine_supported: true,
        unsupported_cards: vec![],
        problems: vec![],
    };
    let total = deck.mainboard.len() + deck.sideboard.len() + deck.commanders.len();
    if total > 10_000 {
        report.issue(
            "request_limit",
            "deck",
            "",
            total,
            "Deck exceeds the server's 10,000-card validation limit".into(),
            true,
        );
        report.legality = Legality::Unknown;
        report.engine_supported = false;
        return report;
    }
    match format {
        Format::Standard => {
            if deck.mainboard.len() < 60 {
                report.issue(
                    "deck_size",
                    "mainboard",
                    "",
                    deck.mainboard.len(),
                    format!(
                        "Standard mainboard has {} cards; at least 60 are required",
                        deck.mainboard.len()
                    ),
                    false,
                );
            }
            if deck.sideboard.len() > 15 {
                report.issue(
                    "sideboard_size",
                    "sideboard",
                    "",
                    deck.sideboard.len(),
                    "Standard permits at most 15 sideboard cards".into(),
                    false,
                );
            }
            if !deck.commanders.is_empty() {
                report.issue(
                    "unexpected_commander",
                    "commander",
                    "",
                    deck.commanders.len(),
                    "Standard does not use commander declarations".into(),
                    false,
                );
            }
        }
        Format::Commander => {
            let count = deck.mainboard.len() + deck.commanders.len();
            if count != 100 {
                report.issue("deck_size","deck","",count,format!("Commander deck has {count} cards including commanders; exactly 100 are required"),false);
            }
            if !deck.sideboard.is_empty() {
                report.issue(
                    "sideboard_size",
                    "sideboard",
                    "",
                    deck.sideboard.len(),
                    "Commander does not use an ordinary sideboard".into(),
                    false,
                );
            }
            if !(1..=2).contains(&deck.commanders.len()) {
                report.issue(
                    "commander_count",
                    "commander",
                    "",
                    deck.commanders.len(),
                    "Declare one commander or a permitted pair".into(),
                    false,
                );
            }
        }
    }
    let mut union = BTreeSet::new();
    let mut commanders = vec![];
    for name in &deck.commanders {
        if let Some((_, card)) = catalog.find(name) {
            union.extend(card.colors.iter().cloned());
            commanders.push(card);
        }
    }
    let identity_known = !commanders.is_empty()
        && commanders.len() == deck.commanders.len()
        && commanders.iter().all(|c| !c.unknown_commander_rule);
    if format == Format::Commander {
        if commanders.len() == 1
            && !commanders[0].commander_eligible
            && !commanders[0].unknown_commander_rule
        {
            report.issue(
                "commander_eligibility",
                "commander",
                &commanders[0].name,
                1,
                "This card cannot be your commander".into(),
                false,
            );
        }
        if commanders.len() == 2 && identity_known && !compatible(commanders[0], commanders[1]) {
            report.issue(
                "commander_pair",
                "commander",
                "",
                2,
                "These cards do not form a permitted commander pair".into(),
                false,
            );
        }
        for card in &commanders {
            if card.unknown_commander_rule {
                report.issue(
                    "unsupported_pregame_rule",
                    "commander",
                    &card.name,
                    1,
                    "This commander's pregame construction choice cannot yet be verified".into(),
                    true,
                );
            }
        }
    }
    let mut counts: BTreeMap<&str, (&Card, usize)> = BTreeMap::new();
    let mut unsupported = BTreeSet::new();
    for (section, names) in [
        ("mainboard", &deck.mainboard),
        ("sideboard", &deck.sideboard),
        ("commander", &deck.commanders),
    ] {
        let mut section_counts = BTreeMap::<&str, usize>::new();
        for name in names {
            *section_counts.entry(name).or_default() += 1;
        }
        for (name, quantity) in section_counts {
            if !implemented(name) {
                unsupported.insert(name.trim().to_owned());
            }
            let Some((id, card)) = catalog.find(name) else {
                report.issue(
                    "unknown_card",
                    section,
                    name,
                    quantity,
                    "Card identity is absent or ambiguous in the format catalogue".into(),
                    true,
                );
                continue;
            };
            let entry = counts.entry(id).or_insert((card, 0));
            entry.1 += quantity;
            let status = match format {
                Format::Standard => card.standard,
                Format::Commander => card.commander,
            };
            if matches!(status, CardLegality::NotLegal | CardLegality::Banned) {
                report.issue(
                    if status == CardLegality::Banned {
                        "banned"
                    } else {
                        "not_legal"
                    },
                    section,
                    &card.name,
                    quantity,
                    if status == CardLegality::Banned {
                        "Card is banned in the selected format".into()
                    } else {
                        "Card is not legal in the selected tabletop format".into()
                    },
                    false,
                );
            } else if status == CardLegality::Restricted {
                report.issue(
                    "unknown_restriction",
                    section,
                    &card.name,
                    quantity,
                    "Unexpected restricted status for this format".into(),
                    true,
                );
            }
            if format == Format::Commander && identity_known {
                if !card.colors.is_subset(&union) {
                    report.issue(
                        "color_identity",
                        section,
                        &card.name,
                        quantity,
                        "Card is outside the commanders' combined color identity".into(),
                        false,
                    );
                }
                if !card.land_colors.is_subset(&union) {
                    report.issue(
                        "basic_land_identity",
                        section,
                        &card.name,
                        quantity,
                        "Basic land types produce colors outside the commanders' identity".into(),
                        false,
                    );
                }
            }
        }
    }
    for (_, (card, count)) in counts {
        if card.unknown_copy_rule {
            report.issue(
                "unsupported_copy_rule",
                "deck",
                &card.name,
                count,
                "This card's construction exception cannot yet be verified".into(),
                true,
            );
            continue;
        }
        let limit = card.copy_limit.unwrap_or(if card.basic {
            u32::MAX
        } else if format == Format::Standard {
            4
        } else {
            1
        });
        if count > limit as usize {
            report.issue(
                "copy_limit",
                "deck",
                &card.name,
                count,
                format!("Deck contains {count} copies; the limit is {limit}"),
                false,
            );
        }
    }
    report.unsupported_cards = unsupported.into_iter().collect();
    report.engine_supported = report.unsupported_cards.is_empty();
    report.legality = if report.problems.iter().any(|p| !p.unknown) {
        Legality::Illegal
    } else if !report.problems.is_empty() {
        Legality::Unknown
    } else {
        Legality::Legal
    };
    report
}
