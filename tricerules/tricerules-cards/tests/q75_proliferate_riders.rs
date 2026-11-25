//! Registry conformance for the queue-v75 02-proliferate-riders cohort.
//!
//! One spell body plus `Proliferate`, with the printed instruction order preserved. Fingerprints
//! are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, CardTypeFilter, EffectSubject, SpellEffectKind, TargetKind,
};
use tricerules_cards::{registry, CardFace, Layout};

fn face(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
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
fn q75_proliferate_riders_map_definitions() {
    let plan = face(
        "contentious_plan",
        "Contentious Plan",
        "{1}{U}",
        &["Sorcery"],
    );
    assert!(
        matches!(
            plan.spell_effect.as_slice(),
            [
                SpellEffectKind::Proliferate,
                SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }
            ]
        ),
        "Contentious Plan proliferates then draws one"
    );

    let progress = face("steady_progress", "Steady Progress", "{2}{U}", &["Instant"]);
    assert_eq!(
        format!("{:?}", progress.spell_effect),
        format!("{:?}", plan.spell_effect),
        "Steady Progress has the same typed body as Contentious Plan"
    );

    let atomize = face("atomize", "Atomize", "{2}{B}{G}", &["Instant"]);
    assert!(
        matches!(
            atomize.spell_effect.as_slice(),
            [SpellEffectKind::Destroy { subject: EffectSubject::Chosen(filter) }, SpellEffectKind::Proliferate]
                if filter.kind == TargetKind::AnyPermanent
                    && filter.excluded_permanent_types
                        == vec![tricerules_cards::primitives::PermanentTypeFilter::Land]
        ),
        "Atomize destroys a nonland permanent then proliferates"
    );

    let insight = face(
        "vivisurgeons_insight",
        "Vivisurgeon's Insight",
        "{3}{U}{U}",
        &["Sorcery"],
    );
    assert!(
        matches!(
            insight.spell_effect.as_slice(),
            [
                SpellEffectKind::Draw {
                    count: Amount::Fixed(3),
                    ..
                },
                SpellEffectKind::Proliferate
            ]
        ),
        "Vivisurgeon's Insight draws three then proliferates"
    );

    let restoration = face(
        "unnatural_restoration",
        "Unnatural Restoration",
        "{1}{G}",
        &["Sorcery"],
    );
    let [SpellEffectKind::MoveGraveyardCards {
        filter,
        destination,
        ..
    }, SpellEffectKind::Proliferate] = restoration.spell_effect.as_slice()
    else {
        panic!("Unnatural Restoration returns a card then proliferates");
    };
    assert_eq!(
        *destination,
        tricerules_cards::primitives::GraveyardDestination::Hand
    );
    let card = filter.card.as_ref().expect("a permanent-card filter");
    let any_of = card
        .any_of
        .as_ref()
        .expect("Land or nonland permanent card");
    assert_eq!(any_of.len(), 2);
    assert!(any_of
        .iter()
        .any(|leaf| leaf.card_type == Some(CardTypeFilter::Land)));
    assert!(any_of
        .iter()
        .any(|leaf| leaf.card_type == Some(CardTypeFilter::NonlandPermanent)));

    for f in [plan, progress, atomize, insight, restoration] {
        assert!(f.spell_keywords.is_empty(), "no spell keyword");
        assert!(f.flashback_cost.is_none(), "no flashback");
        assert!(f.static_abilities.is_empty(), "no static ability");
        assert!(f.activated_abilities.is_empty(), "no activated ability");
    }
}

#[test]
fn q75_proliferate_riders_target_groups_match_their_bodies() {
    let atomize = face("atomize", "Atomize", "{2}{B}{G}", &["Instant"]);
    let group = atomize
        .targeting
        .as_ref()
        .and_then(|targeting| targeting.groups.first())
        .expect("Atomize has one target group");
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
    assert_eq!(group.prompt, "Choose target nonland permanent");

    let restoration = face(
        "unnatural_restoration",
        "Unnatural Restoration",
        "{1}{G}",
        &["Sorcery"],
    );
    assert!(
        restoration.targeting.is_none(),
        "the graveyard move owns its card target"
    );

    for f in [
        face(
            "contentious_plan",
            "Contentious Plan",
            "{1}{U}",
            &["Sorcery"],
        ),
        face("steady_progress", "Steady Progress", "{2}{U}", &["Instant"]),
        face(
            "vivisurgeons_insight",
            "Vivisurgeon's Insight",
            "{3}{U}{U}",
            &["Sorcery"],
        ),
    ] {
        assert!(f.targeting.is_none(), "{} is untargeted", f.name);
    }
}

#[test]
fn q75_proliferate_riders_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "contentious_plan",
            "Contentious Plan",
            "contentious_plan",
            "221d579f3a64c3a063c048d62a4840c443f1f932ceb38e6c1fa7a7c3595266d6",
        ),
        (
            "steady_progress",
            "Steady Progress",
            "steady_progress",
            "221d579f3a64c3a063c048d62a4840c443f1f932ceb38e6c1fa7a7c3595266d6",
        ),
        (
            "atomize",
            "Atomize",
            "atomize",
            "58be367afff663413b20ed65919e2ecfc41281ca7820b0d69ddecee497b466db",
        ),
        (
            "unnatural_restoration",
            "Unnatural Restoration",
            "unnatural_restoration",
            "a2023cbf14617e2bea25bef51522309ed425bb6feba734a97d094d1fd7dc7fd1",
        ),
        (
            "vivisurgeons_insight",
            "Vivisurgeon's Insight",
            "vivisurgeons_insight",
            "33df0a43ccda890c8fec144064327abe5b0cc32a204a462c440141554938da0c",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id, "face id drift for {id}");
        assert_eq!(fields[3], name, "display name drift for {id}");
        assert_eq!(fields[4], fingerprint, "pinned Oracle fingerprint for {id}");
    }
}
