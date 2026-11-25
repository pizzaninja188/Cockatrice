//! Registry conformance for the queue-v59 batch A cohorts.
//!
//! Each included card is a normal-layout single-face, Commander-legal instant or sorcery whose
//! typed definition is one of three reviewed shapes: a controller-chosen reveal-hand discard with
//! an optional rider, a threaten spell with one rider, or a filtered counterspell that draws.

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, Amount, CardTypeFilter, EffectSubject, HandCardAction,
    HandCardChooser, HandChoiceVisibility, Keyword, LibraryPartitionKind, PlayerRecipient,
    SpellEffectKind, TargetKind,
};
use tricerules_cards::{registry, AbilityPresentation, CardFace, Layout};

fn face(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.face_id.as_str(), id, "{id} face id");
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

#[test]
fn q59_hand_discard_map_definitions() {
    for (id, name, mana, target_kind, filtered) in [
        (
            "dark_inquiry",
            "Dark Inquiry",
            "{2}{B}",
            TargetKind::OpponentPlayer,
            true,
        ),
        (
            "distress",
            "Distress",
            "{B}{B}",
            TargetKind::AnyPlayer,
            true,
        ),
        (
            "brainbite",
            "Brainbite",
            "{2}{U}{B}",
            TargetKind::OpponentPlayer,
            false,
        ),
        (
            "thought_erasure",
            "Thought Erasure",
            "{U}{B}",
            TargetKind::OpponentPlayer,
            true,
        ),
    ] {
        let f = face(id, name, mana, &["Sorcery"]);
        let SpellEffectKind::ChooseHandCards {
            action,
            count,
            target,
            chooser,
            card_filter,
            optional,
            visibility,
        } = &f.spell_effect[0]
        else {
            panic!(
                "{id} must choose a revealed hand card: {:?}",
                f.spell_effect
            );
        };
        assert_eq!(*action, HandCardAction::Discard, "{id}");
        assert_eq!(*count, 1, "{id}");
        assert_eq!(target.kind, target_kind, "{id}");
        assert_eq!(*chooser, HandCardChooser::Controller, "{id}");
        assert_eq!(
            *card_filter,
            filtered.then_some(CardTypeFilter::Nonland),
            "{id}"
        );
        assert!(!*optional, "{id}");
        assert_eq!(*visibility, HandChoiceVisibility::PublicReveal, "{id}");
        let group = &f.targeting.as_ref().expect("explicit player group").groups[0];
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(group.effect_indices, [0], "{id}");
    }

    let brainbite = face("brainbite", "Brainbite", "{2}{U}{B}", &["Sorcery"]);
    assert!(matches!(
        brainbite.spell_effect.as_slice(),
        [
            SpellEffectKind::ChooseHandCards { .. },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }
        ]
    ));

    let erasure = face("thought_erasure", "Thought Erasure", "{U}{B}", &["Sorcery"]);
    let [SpellEffectKind::ChooseHandCards { .. }, SpellEffectKind::LibraryPartition {
        count,
        top_min,
        top_max,
        kind,
    }] = erasure.spell_effect.as_slice()
    else {
        panic!(
            "Thought Erasure must discard, then surveil: {:?}",
            erasure.spell_effect
        );
    };
    assert_eq!(*count, 1);
    assert_eq!(*top_min, 0);
    assert_eq!(*top_max, None);
    assert_eq!(*kind, LibraryPartitionKind::Surveil);
}

#[test]
fn q59_threaten_map_definitions() {
    for (id, name, mana, keywords) in [
        (
            "limits_of_solidarity",
            "Limits of Solidarity",
            "{3}{R}",
            vec![Keyword::Haste],
        ),
        (
            "portent_of_betrayal",
            "Portent of Betrayal",
            "{3}{R}",
            vec![Keyword::Haste],
        ),
        (
            "traitorous_blood",
            "Traitorous Blood",
            "{1}{R}{R}",
            vec![Keyword::Trample, Keyword::Haste],
        ),
    ] {
        let f = face(id, name, mana, &["Sorcery"]);
        let [SpellEffectKind::GainControl { duration, .. }, SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(filter),
        }, SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(grant_filter),
            keywords: granted,
        }, ..] = f.spell_effect.as_slice()
        else {
            panic!("{id} must threaten one creature: {:?}", f.spell_effect);
        };
        assert_eq!(
            *duration,
            tricerules_cards::GainControlDuration::UntilEndOfTurn
        );
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        assert_eq!(grant_filter.kind, TargetKind::Creature, "{id}");
        assert_eq!(granted, &keywords, "{id}");
        assert!(f.targeting.is_none(), "{id} uses the default target group");
    }

    let portent = face(
        "portent_of_betrayal",
        "Portent of Betrayal",
        "{3}{R}",
        &["Sorcery"],
    );
    assert!(matches!(
        portent.spell_effect.last(),
        Some(SpellEffectKind::Scry {
            count: Amount::Fixed(1)
        })
    ));

    let limits = face(
        "limits_of_solidarity",
        "Limits of Solidarity",
        "{3}{R}",
        &["Sorcery"],
    );
    let [cycling] = limits.activated_abilities.as_slice() else {
        panic!("Limits of Solidarity has exactly one cycling ability");
    };
    assert_eq!(cycling.ability_id.as_str(), "activated_01");
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
    assert!(matches!(
        &cycling.costs[0],
        AbilityCost::Mana(cost) if cost.to_string() == "{2}"
    ));
    assert!(matches!(&cycling.costs[1], AbilityCost::DiscardSelf));
    assert!(matches!(
        cycling.effect.as_slice(),
        [SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            ..
        }]
    ));
}

#[test]
fn q59_counter_filter_map_definitions() {
    for (id, name, mana, filter) in [
        (
            "halt_order",
            "Halt Order",
            "{2}{U}",
            Some(CardTypeFilter::Artifact),
        ),
        (
            "scatter_arc",
            "Scatter Arc",
            "{3}{U}",
            Some(CardTypeFilter::Noncreature),
        ),
        (
            "deny_entry",
            "Deny Entry",
            "{2}{U}",
            Some(CardTypeFilter::Creature),
        ),
    ] {
        let f = face(id, name, mana, &["Instant"]);
        let [SpellEffectKind::CounterTargetSpell { spell_filter, .. }, SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }, ..] = f.spell_effect.as_slice()
        else {
            panic!("{id} must counter, then draw: {:?}", f.spell_effect);
        };
        assert_eq!(spell_filter.card_type, filter, "{id}");
        assert!(spell_filter.min_mana_value.is_none(), "{id}");
        assert!(spell_filter.max_mana_value.is_none(), "{id}");
        assert!(f.targeting.is_none(), "{id} uses the default counter group");
    }

    let deny = face("deny_entry", "Deny Entry", "{2}{U}", &["Instant"]);
    assert!(matches!(
        deny.spell_effect.as_slice(),
        [
            SpellEffectKind::CounterTargetSpell { .. },
            SpellEffectKind::Draw { .. },
            SpellEffectKind::Discard {
                who: PlayerRecipient::Controller,
                quantity: tricerules_cards::primitives::DiscardQuantity::Exact(1),
                ..
            }
        ]
    ));
}

#[test]
fn q59_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, digest) in [
        (
            "dark_inquiry",
            "Dark Inquiry",
            "daed12d3769f725a8a9bbf9c97b1378198645e001fc39c4cda66a374cfc13584",
        ),
        (
            "distress",
            "Distress",
            "74aac589008393539be5f00dadd75da92c8b70a953029cf90e1c09e8400ac845",
        ),
        (
            "brainbite",
            "Brainbite",
            "04e079f120f7bc1449278ecade7358e438f77a9b8936565cc24bc800aeabb51a",
        ),
        (
            "thought_erasure",
            "Thought Erasure",
            "48098e75c7db15b1b8e07915f26b6579671e80e7816876f24a5d5cd7bf4c6993",
        ),
        (
            "limits_of_solidarity",
            "Limits of Solidarity",
            "873dfa4fc6b0ca2e60a8a7f864599107149b0661200dde7d9ccf22e26af8c511",
        ),
        (
            "portent_of_betrayal",
            "Portent of Betrayal",
            "05f0c0f495cc34e0ae79ba80b6eed6dd7917474077205cf6bba0368f0da69edc",
        ),
        (
            "traitorous_blood",
            "Traitorous Blood",
            "70812a1919bb0c04081f3b9189671bf8f02347ba934436898f81bc64be490ce3",
        ),
        (
            "halt_order",
            "Halt Order",
            "ac1741fa5c36d32349fcf77dc78d76f07f3251a7442cf2144ace26a567c4d42f",
        ),
        (
            "scatter_arc",
            "Scatter Arc",
            "77f3b57ce5250a510e1af36112f821ce39e7f1e787e8192670368b7ba9d66d73",
        ),
        (
            "deny_entry",
            "Deny Entry",
            "54ba6483bc755a01bcf83c491e8efed33395c9e6765a244e6a9af3b558c69f02",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields[1], name, "{id} name");
        assert_eq!(fields[4], digest, "{id} fingerprint");
    }
}
