//! Descriptions of validated target predicates. Alternatives retain their own qualifiers
//! unless all branches share them.

use super::presentation::card_type_label;
use super::{
    Color, CombatRole, GraveyardFilter, GraveyardOwner, PowerComparison, StackSpellFilter,
    TargetBinding, TargetChooser, TargetController, TargetFilter, TargetGroupDef, TargetKind,
    TargetObjectExclusion, TargetOwner, TargetRole, ZoneCardFilter,
};
use std::borrow::Cow;

#[derive(PartialEq, Eq)]
struct Phrase {
    noun: String,
    qualifiers: Vec<String>,
    excludes_source: bool,
}

impl Phrase {
    fn new(noun: impl Into<String>) -> Self {
        Self {
            noun: noun.into(),
            qualifiers: Vec::new(),
            excludes_source: false,
        }
    }

    fn add(&mut self, qualifier: impl Into<String>) {
        self.qualifiers.push(qualifier.into());
    }

    fn text(self) -> String {
        if self.qualifiers.is_empty() {
            self.noun
        } else {
            format!("{} {}", self.noun, self.qualifiers.join(" "))
        }
    }

    fn alternative(self) -> String {
        if self.excludes_source {
            format!("{} other than the source", self.text())
        } else {
            self.text()
        }
    }
}

fn alternatives(branches: Vec<Phrase>, shared_ending: Option<&str>) -> Option<Phrase> {
    let first = branches.first()?;
    let qualifiers = first.qualifiers.clone();
    let excludes_source = first.excludes_source;
    if branches
        .iter()
        .all(|branch| branch.qualifiers == qualifiers && branch.excludes_source == excludes_source)
    {
        let mut names = branches
            .into_iter()
            .map(|branch| branch.noun)
            .collect::<Vec<_>>();
        let ending = shared_ending
            .filter(|ending| names.iter().all(|name| name.ends_with(*ending)))
            .unwrap_or_default();
        for name in &mut names {
            name.truncate(name.len() - ending.len());
        }
        let mut noun = joined(&names, "or");
        noun.push_str(ending);
        return Some(Phrase {
            noun,
            qualifiers,
            excludes_source,
        });
    }
    let names = branches
        .into_iter()
        .map(|branch| format!("({})", branch.alternative()))
        .collect::<Vec<_>>();
    Some(Phrase::new(joined(&names, "or")))
}

fn joined(names: &[String], conjunction: &str) -> String {
    match names {
        [] => String::new(),
        [only] => only.clone(),
        [left, right] => format!("{left} {conjunction} {right}"),
        [front @ .., last] => format!("{}, {conjunction} {last}", front.join(", ")),
    }
}

fn noun(singular: &str, plural: bool) -> String {
    if !plural {
        return singular.into();
    }
    match singular {
        "activated or triggered ability" => "activated or triggered abilities".into(),
        _ => format!("{singular}s"),
    }
}

fn color_name(color: Color) -> &'static str {
    match color {
        Color::White => "white",
        Color::Blue => "blue",
        Color::Black => "black",
        Color::Red => "red",
        Color::Green => "green",
    }
}

fn with_adjectives(adjectives: Vec<String>, subject: String) -> String {
    if adjectives.is_empty() {
        subject
    } else {
        format!("{} {subject}", adjectives.join(" "))
    }
}

fn bounds(name: &str, min: Option<u32>, max: Option<u32>) -> Option<String> {
    Some(match (min, max) {
        (Some(min), Some(max)) if min == max => format!("with {name} {min}"),
        (Some(min), Some(max)) => format!("with {name} from {min} through {max}"),
        (Some(min), None) => format!("with {name} {min} or greater"),
        (None, Some(max)) => format!("with {name} {max} or less"),
        (None, None) => return None,
    })
}

fn comparison(name: &str, comparison: PowerComparison) -> String {
    match comparison {
        PowerComparison::AtLeast(min) => format!("with {name} {min} or greater"),
        PowerComparison::AtMost(max) => format!("with {name} {max} or less"),
    }
}

fn battlefield(
    filter: &TargetFilter,
    plural: bool,
    chooser: TargetChooser,
    per_opponent: bool,
    controlled_by_chooser: bool,
) -> Option<Phrase> {
    let TargetFilter {
        any_of,
        kind,
        tapped,
        combat_role,
        not_color,
        is_color,
        controller,
        owner,
        excluded_objects,
        permanent_types,
        required_subtypes,
        excluded_subtypes,
        required_supertypes,
        excluded_supertypes,
        power,
        toughness,
        required_keywords,
        excluded_keywords,
        token,
        excluded_permanent_types,
        min_mana_value,
        max_mana_value,
        was_dealt_damage_this_turn,
        dealt_damage_this_turn,
        required_counter,
        requires_any_counter,
    } = filter;
    if let Some(branches) = any_of {
        return alternatives(
            branches
                .iter()
                .map(|branch| {
                    battlefield(branch, plural, chooser, per_opponent, controlled_by_chooser)
                })
                .collect::<Option<Vec<_>>>()?,
            None,
        );
    }
    let mut adjectives = Vec::new();
    if let Some(tapped) = tapped {
        adjectives.push(if *tapped { "tapped" } else { "untapped" }.into());
    }
    if let Some(combat_role) = combat_role {
        adjectives.push(
            match combat_role {
                CombatRole::Attacking => "attacking",
                CombatRole::Blocking => "blocking",
                CombatRole::AttackingOrBlocking => "attacking or blocking",
            }
            .into(),
        );
    }
    adjectives.extend(is_color.map(|color| color_name(color).to_owned()));
    adjectives.extend(not_color.map(|color| format!("non{}", color_name(color))));
    adjectives.extend(
        required_supertypes
            .iter()
            .map(|name| name.to_ascii_lowercase()),
    );
    adjectives.extend(
        excluded_supertypes
            .iter()
            .map(|name| format!("non{}", name.to_ascii_lowercase())),
    );
    adjectives.extend(
        excluded_permanent_types
            .iter()
            .map(|kind| format!("non{}", kind.as_str().to_ascii_lowercase())),
    );
    adjectives.extend(excluded_subtypes.iter().map(|name| format!("non-{name}")));
    if *token == Some(false) {
        adjectives.push("nontoken".into());
    }
    let mut types = permanent_types
        .iter()
        .map(|kind| kind.as_str().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let base = match kind {
        TargetKind::AnyTarget => {
            let understood = TargetFilter {
                excluded_objects: excluded_objects.clone(),
                ..TargetFilter::default()
            };
            if *filter != understood {
                return None;
            }
            let mut phrase = Phrase::new(noun("target", plural));
            phrase.excludes_source = excluded_objects.contains(&TargetObjectExclusion::Source);
            if excluded_objects.contains(&TargetObjectExclusion::AttachedObject) {
                phrase.add("other than the attached object");
            }
            return Some(phrase);
        }
        TargetKind::AnyPlayer => "player",
        TargetKind::OpponentPlayer => "opponent",
        TargetKind::Creature => "creature",
        TargetKind::AnyPermanent => "permanent",
    };
    let kind_qualifier = if *kind == TargetKind::Creature {
        match types.as_slice() {
            [only] if only != "creature" => adjectives.push(only.clone()),
            [_, _, ..] if !types.iter().any(|kind| kind == "creature") => (),
            _ => types.clear(),
        }
        if types.len() > 1 {
            Some(format!("with card type {}", joined(&types, "or")))
        } else {
            None
        }
    } else {
        None
    };
    let subject = match (kind, types.as_slice(), required_subtypes.as_slice(), token) {
        (TargetKind::AnyPermanent, [], [], Some(true)) => noun("token", plural),
        (TargetKind::AnyPermanent, [], [subtype], None | Some(false)) if !plural => subtype.clone(),
        (TargetKind::AnyPermanent, types, _, _) if !types.is_empty() => {
            let names = types
                .iter()
                .map(|kind| {
                    let subject = if *token == Some(true) {
                        format!("{kind} {}", noun("token", plural))
                    } else {
                        noun(kind, plural)
                    };
                    with_adjectives(adjectives.clone(), subject)
                })
                .collect::<Vec<_>>();
            adjectives.clear();
            joined(&names, "or")
        }
        (_, _, _, Some(true)) => format!("{base} {}", noun("token", plural)),
        _ => noun(base, plural),
    };
    if plural
        || !matches!(
            (kind, types.as_slice(), required_subtypes.as_slice(), token),
            (TargetKind::AnyPermanent, [], [_], None | Some(false))
        )
    {
        adjectives.extend(required_subtypes.iter().cloned());
    }
    let mut phrase = Phrase::new(with_adjectives(adjectives, subject));
    phrase.qualifiers.extend(kind_qualifier);
    if controlled_by_chooser {
        phrase.add("you control");
    } else {
        match (controller, chooser, per_opponent) {
            (TargetController::Any, _, _) | (TargetController::Opponent, _, true) => (),
            (TargetController::You, TargetChooser::Controller, _) => phrase.add("you control"),
            (TargetController::You, TargetChooser::ChosenOpponent, _) => {
                phrase.add("the ability's controller controls")
            }
            (TargetController::Opponent, TargetChooser::Controller, _) => {
                phrase.add("an opponent controls")
            }
            (TargetController::Opponent, TargetChooser::ChosenOpponent, _) => {
                phrase.add("an opponent of the ability's controller controls")
            }
            (TargetController::NotYou, TargetChooser::Controller, _) => {
                phrase.add("you don't control")
            }
            (TargetController::NotYou, TargetChooser::ChosenOpponent, _) => {
                phrase.add("the ability's controller doesn't control")
            }
            (TargetController::DefendingPlayer, _, _) => {
                phrase.add("the defending player controls")
            }
        }
    }
    if *owner == TargetOwner::You {
        phrase.add(match chooser {
            TargetChooser::Controller => "you own",
            TargetChooser::ChosenOpponent => "the ability's controller owns",
        });
    }
    phrase
        .qualifiers
        .extend(power.map(|value| comparison("power", value)));
    phrase
        .qualifiers
        .extend(toughness.map(|value| comparison("toughness", value)));
    phrase
        .qualifiers
        .extend(bounds("mana value", *min_mana_value, *max_mana_value));
    for keyword in required_keywords {
        phrase.add(format!("with {}", keyword.as_str().to_ascii_lowercase()));
    }
    for keyword in excluded_keywords {
        phrase.add(format!("without {}", keyword.as_str().to_ascii_lowercase()));
    }
    if let Some(counter) = required_counter {
        phrase.add(format!("with a {} counter", counter.label()));
    }
    if *requires_any_counter {
        phrase.add("with a counter");
    }
    if let Some(dealt) = was_dealt_damage_this_turn {
        phrase.add(match (*dealt, plural) {
            (true, true) => "that were dealt damage this turn",
            (true, false) => "that was dealt damage this turn",
            (false, true) => "that weren't dealt damage this turn",
            (false, false) => "that wasn't dealt damage this turn",
        });
    }
    if let Some(dealt) = dealt_damage_this_turn {
        phrase.add(if *dealt {
            "that dealt damage this turn"
        } else {
            "that didn't deal damage this turn"
        });
    }
    phrase.excludes_source = excluded_objects.contains(&TargetObjectExclusion::Source);
    if excluded_objects.contains(&TargetObjectExclusion::AttachedObject) {
        phrase.add("other than the attached object");
    }
    Some(phrase)
}

fn spell(filter: &StackSpellFilter, plural: bool) -> Option<Phrase> {
    let StackSpellFilter {
        any_of,
        card_type,
        is_color,
        min_mana_value,
        max_mana_value,
    } = filter;
    let subject = noun("spell", plural);
    if let Some(branches) = any_of {
        return alternatives(
            branches
                .iter()
                .map(|branch| spell(branch, plural))
                .collect::<Option<Vec<_>>>()?,
            Some(&format!(" {subject}")),
        );
    }
    let adjectives = is_color
        .map(color_name)
        .into_iter()
        .chain(card_type.map(card_type_label))
        .map(str::to_owned)
        .collect();
    let mut phrase = Phrase::new(with_adjectives(adjectives, subject));
    phrase
        .qualifiers
        .extend(bounds("mana value", *min_mana_value, *max_mana_value));
    Some(phrase)
}

fn zone_card(filter: &ZoneCardFilter, plural: bool, location: &str) -> Option<Phrase> {
    let ZoneCardFilter {
        any_of,
        exact_name: None,
        card_type,
        excluded_card_types,
        is_colorless: None,
        min_mana_value,
        max_mana_value,
        required_subtypes,
        excluded_subtypes,
        required_supertypes,
        has_adventure: None,
        printed_power: None,
    } = filter
    else {
        return None;
    };
    if let Some(branches) = any_of {
        let card_types = branches
            .iter()
            .map(|leaf| {
                (*leaf
                    == ZoneCardFilter {
                        card_type: leaf.card_type,
                        ..Default::default()
                    })
                .then_some(leaf.card_type)
                .flatten()
            })
            .collect::<Option<Vec<_>>>();
        if let Some(types) = card_types {
            use super::CardTypeFilter::{Land, NonlandPermanent};
            if matches!(
                types.as_slice(),
                [Land, NonlandPermanent] | [NonlandPermanent, Land]
            ) {
                let mut phrase = Phrase::new(format!("permanent {}", noun("card", plural)));
                phrase.add(location);
                return Some(phrase);
            }
        }
        return alternatives(
            branches
                .iter()
                .map(|branch| zone_card(branch, plural, location))
                .collect::<Option<Vec<_>>>()?,
            None,
        );
    }
    let mut adjectives = required_supertypes
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    use super::CardTypeFilter::{
        BasicLand, Instant, InstantOrSorcery, Noncreature, Nonland, NonlandPermanent, Sorcery,
    };
    let excludes_instants_and_sorceries = matches!(
        excluded_card_types.as_slice(),
        [Instant, Sorcery] | [Sorcery, Instant]
    );
    if !excludes_instants_and_sorceries {
        for kind in excluded_card_types {
            if matches!(
                kind,
                BasicLand | InstantOrSorcery | Noncreature | Nonland | NonlandPermanent
            ) {
                return None;
            }
            adjectives.push(format!("non{}", card_type_label(*kind)));
        }
    }
    adjectives.extend(excluded_subtypes.iter().map(|name| format!("non-{name}")));
    adjectives.extend(required_subtypes.iter().cloned());
    adjectives.extend(card_type.map(|kind| card_type_label(kind).to_owned()));
    let mut phrase = Phrase::new(with_adjectives(adjectives, noun("card", plural)));
    phrase
        .qualifiers
        .extend(bounds("mana value", *min_mana_value, *max_mana_value));
    phrase.add(location);
    if excludes_instants_and_sorceries {
        phrase.add("excluding instants and sorceries");
    }
    Some(phrase)
}

fn graveyard(filter: &GraveyardFilter, plural: bool, same_graveyard: bool) -> Option<Phrase> {
    let GraveyardFilter {
        excluded_objects,
        owner,
        card,
        mana_value_less_than_trigger_object,
    } = filter;
    let location = match (owner, same_graveyard, plural) {
        (GraveyardOwner::Controller, _, _) => "from your graveyard",
        (GraveyardOwner::AnyPlayer, true, _) => "from a single graveyard",
        (GraveyardOwner::AnyPlayer, false, true) => "from graveyards",
        (GraveyardOwner::AnyPlayer, false, false) => "from a graveyard",
        (GraveyardOwner::Opponent, true, _) => "from a single opponent's graveyard",
        (GraveyardOwner::Opponent, false, true) => "from opponents' graveyards",
        (GraveyardOwner::Opponent, false, false) => "from an opponent's graveyard",
    };
    let mut phrase = match card {
        Some(card) => zone_card(card, plural, location)?,
        None => {
            let mut phrase = Phrase::new(noun("card", plural));
            phrase.add(location);
            phrase
        }
    };
    if *mana_value_less_than_trigger_object {
        phrase.add("with mana value lower than the permanent that left the battlefield");
    }
    phrase.excludes_source = excluded_objects.contains(&TargetObjectExclusion::Source);
    Some(phrase)
}

fn constrained_prompt(
    group: &TargetGroupDef,
    bindings: &[TargetBinding<'_>],
    group_index: usize,
    earlier_distinct: &[usize],
    quantity: &str,
    matches_activation_x: bool,
) -> Option<String> {
    let plural = group.max > 1;
    let [first, rest @ ..] = bindings else {
        return None;
    };
    if rest.iter().any(|binding| binding.role != first.role) {
        return None;
    }
    let mut phrase = match first.role {
        TargetRole::Filtered(filter) => battlefield(
            filter,
            plural,
            group.chooser,
            group.for_each_opponent,
            group.controlled_by_chooser,
        )?,
        TargetRole::CreaturePermanent => Phrase::new(noun("creature", plural)),
        TargetRole::StackSpell(filter) => spell(filter, plural)?,
        TargetRole::StackAbility => Phrase::new(noun("activated or triggered ability", plural)),
        TargetRole::GraveyardCard(filter) => graveyard(filter, plural, group.same_graveyard)?,
    };
    if matches_activation_x {
        phrase.add("with mana value X");
    }
    let mut text = format!(
        "Choose {}",
        selection(phrase, quantity, plural, group_index, earlier_distinct)
    );
    if let Some(expansion) = &group.cast_cost_expansion {
        let restricted = battlefield(&expansion.without_cost, plural, group.chooser, false, false)?;
        let restricted = selection(restricted, quantity, plural, group_index, earlier_distinct);
        text.push_str(&format!(
            ". Without the additional cost, choose {restricted}."
        ));
    }
    Some(text)
}

fn selection(
    mut phrase: Phrase,
    quantity: &str,
    plural: bool,
    group_index: usize,
    earlier_distinct: &[usize],
) -> String {
    // Only the second group can say "other" without naming the group it differs from.
    let other_first_target = group_index == 1 && earlier_distinct == [0];
    let other = phrase.excludes_source || other_first_target;
    if !earlier_distinct.is_empty() && (phrase.excludes_source || !other_first_target) {
        let groups = earlier_distinct
            .iter()
            .map(|index| (index + 1).to_string())
            .collect::<Vec<_>>();
        let group_label = if groups.len() == 1 { "group" } else { "groups" };
        phrase.add(format!(
            "different from targets chosen in {group_label} {}",
            joined(&groups, "and")
        ));
    }
    let generic = phrase.noun == if plural { "targets" } else { "target" };
    let prefix = match (quantity, other) {
        ("", true) => "another ".into(),
        ("", false) if generic => "any ".into(),
        ("", false) => "".into(),
        (amount, true) => Cow::Owned(format!("{amount} other ")),
        (amount, false) => Cow::Owned(format!("{amount} ")),
    };
    let target = if generic { "" } else { "target " };
    format!("{prefix}{target}{}", phrase.text())
}

pub(super) fn target_group_prompt<'a>(
    groups: &'a [TargetGroupDef],
    bindings: &[TargetBinding<'_>],
    group_index: usize,
    matches_activation_x: bool,
) -> Result<Cow<'a, str>, String> {
    let group = &groups[group_index];
    if group.controlled_by_chooser && group.chooser != TargetChooser::ChosenOpponent {
        return Err("chooser-controlled targets require an opponent-chosen group".into());
    }
    if !group.prompt.trim().is_empty() {
        return Ok(Cow::Borrowed(&group.prompt));
    }
    let quantity = match (group.min, group.max) {
        (1, 1) => "".into(),
        (0, 1) => "up to one".into(),
        (0, 2) => "up to two".into(),
        (0, 3) => "up to three".into(),
        (1, 2) => "one or two".into(),
        (0, max) => Cow::Owned(format!("up to {max}")),
        (min, max) if min == max => Cow::Owned(min.to_string()),
        (min, max) => Cow::Owned(format!("{min}–{max}")),
    };
    // Explain the relation at the later choice regardless of which group declared it.
    let earlier_distinct = groups[..group_index]
        .iter()
        .enumerate()
        .filter_map(|(index, earlier)| {
            (group.distinct_from.contains(&(index as u32))
                || earlier.distinct_from.contains(&(group_index as u32)))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    constrained_prompt(
        group,
        bindings,
        group_index,
        &earlier_distinct,
        &quantity,
        matches_activation_x,
    )
    .map(Cow::Owned)
    .ok_or_else(|| "target group requires an explicit prompt for its constraints".into())
}
