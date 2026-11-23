//! Registry conformance for the queue-v60 batch A cohorts.
//!
//! Each included card is a normal-layout single-face, Commander-legal instant or sorcery whose
//! typed definition is one of three reviewed shapes: a destroy-artifact-or-enchantment spell with
//! one printed rider, a targeted combat restriction that prevents blocking this turn, or an exile
//! spell that hits a creature or planeswalker with an optional rider. Each fingerprint is SHA-256
//! of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    Amount, CombatRestrictionScope, EffectSubject, LibraryPartitionKind, PermanentTypeFilter,
    SpellEffectKind, TargetKind,
};
use tricerules_cards::{registry, CardFace, Layout};

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

fn assert_artifact_or_enchantment(subject: &EffectSubject, id: &str) {
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{id}: expected a chosen subject, got {subject:?}");
    };
    let any_of = filter
        .any_of
        .as_ref()
        .expect("artifact-or-enchantment union");
    assert_eq!(any_of.len(), 2, "{id}");
    assert_eq!(any_of[0].kind, TargetKind::AnyPermanent, "{id}");
    assert_eq!(
        any_of[0].permanent_types,
        [PermanentTypeFilter::Artifact],
        "{id}"
    );
    assert_eq!(any_of[1].kind, TargetKind::AnyPermanent, "{id}");
    assert_eq!(
        any_of[1].permanent_types,
        [PermanentTypeFilter::Enchantment],
        "{id}"
    );
}

#[test]
fn q60_artifact_enchantment_map_definitions() {
    for (id, name, mana, types) in [
        (
            "artisans_sorrow",
            "Artisan's Sorrow",
            "{3}{G}",
            &["Instant"][..],
        ),
        ("crack_open", "Crack Open", "{2}{G}", &["Sorcery"][..]),
        (
            "expose_to_daylight",
            "Expose to Daylight",
            "{2}{W}",
            &["Instant"][..],
        ),
        (
            "ray_of_distortion",
            "Ray of Distortion",
            "{3}{W}",
            &["Instant"][..],
        ),
        (
            "slice_in_twain",
            "Slice in Twain",
            "{2}{G}{G}",
            &["Instant"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        let [SpellEffectKind::Destroy { subject }, ..] = f.spell_effect.as_slice() else {
            panic!(
                "{id}: must destroy one chosen permanent: {:?}",
                f.spell_effect
            );
        };
        assert_artifact_or_enchantment(subject, id);
        let groups = &f.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert_eq!(f.triggered_abilities.len(), 0, "{id}");
        assert_eq!(f.static_abilities.len(), 0, "{id}");
    }

    let sorrow = face(
        "artisans_sorrow",
        "Artisan's Sorrow",
        "{3}{G}",
        &["Instant"],
    );
    assert!(matches!(
        sorrow.spell_effect.as_slice(),
        [
            SpellEffectKind::Destroy { .. },
            SpellEffectKind::Scry {
                count: Amount::Fixed(2)
            }
        ]
    ));

    let crack = face("crack_open", "Crack Open", "{2}{G}", &["Sorcery"]);
    assert!(matches!(
        crack.spell_effect.as_slice(),
        [
            SpellEffectKind::Destroy { .. },
            SpellEffectKind::CreateTokens {
                token,
                count: Amount::Fixed(1),
                ..
            }
        ] if token == "treasure"
    ));

    let daylight = face(
        "expose_to_daylight",
        "Expose to Daylight",
        "{2}{W}",
        &["Instant"],
    );
    assert!(matches!(
        daylight.spell_effect.as_slice(),
        [
            SpellEffectKind::Destroy { .. },
            SpellEffectKind::Scry {
                count: Amount::Fixed(1)
            }
        ]
    ));

    let ray = face(
        "ray_of_distortion",
        "Ray of Distortion",
        "{3}{W}",
        &["Instant"],
    );
    assert_eq!(
        ray.flashback_cost
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("{4}{W}{W}")
    );

    let slice = face(
        "slice_in_twain",
        "Slice in Twain",
        "{2}{G}{G}",
        &["Instant"],
    );
    assert!(matches!(
        slice.spell_effect.as_slice(),
        [
            SpellEffectKind::Destroy { .. },
            SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }
        ]
    ));
}

#[test]
fn q60_unblockable_map_definitions() {
    for (id, name, mana, types) in [
        ("infiltrate", "Infiltrate", "{U}", &["Instant"][..]),
        ("trailblazer", "Trailblazer", "{2}{G}{G}", &["Instant"][..]),
        ("artful_dodge", "Artful Dodge", "{U}", &["Sorcery"][..]),
        (
            "touch_of_invisibility",
            "Touch of Invisibility",
            "{3}{U}",
            &["Sorcery"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        let [first, ..] = f.spell_effect.as_slice() else {
            panic!("{id}: expected at least one effect");
        };
        let SpellEffectKind::ApplyCombatRestriction { scope, restriction } = first else {
            panic!(
                "{id}: expected one combat restriction: {:?}",
                f.spell_effect
            );
        };
        let CombatRestrictionScope::Chosen(filter) = scope else {
            panic!("{id}: expected a chosen scope, got {scope:?}");
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        assert!(restriction.cant_be_blocked, "{id}");
        assert!(!restriction.cant_attack && !restriction.cant_block, "{id}");
        let groups = &f.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups[0].effect_indices, [0], "{id}");
    }

    let dodge = face("artful_dodge", "Artful Dodge", "{U}", &["Sorcery"]);
    assert_eq!(
        dodge
            .flashback_cost
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("{U}")
    );

    let touch = face(
        "touch_of_invisibility",
        "Touch of Invisibility",
        "{3}{U}",
        &["Sorcery"],
    );
    assert!(matches!(
        touch.spell_effect.as_slice(),
        [
            SpellEffectKind::ApplyCombatRestriction { .. },
            SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }
        ]
    ));
}

#[test]
fn q60_exile_creature_planeswalker_map_definitions() {
    for (id, name, mana) in [
        ("feed_the_serpent", "Feed the Serpent", "{2}{B}{B}"),
        ("eat_to_extinction", "Eat to Extinction", "{3}{B}"),
        ("vraskas_contempt", "Vraska's Contempt", "{2}{B}{B}"),
    ] {
        let f = face(id, name, mana, &["Instant"]);
        let [SpellEffectKind::Exile { subject, .. }, ..] = f.spell_effect.as_slice() else {
            panic!("{id}: expected exile first: {:?}", f.spell_effect);
        };
        let EffectSubject::Chosen(filter) = subject else {
            panic!("{id}: expected a chosen subject, got {subject:?}");
        };
        assert_eq!(filter.kind, TargetKind::AnyPermanent, "{id}");
        assert_eq!(
            filter.permanent_types,
            [
                PermanentTypeFilter::Creature,
                PermanentTypeFilter::Planeswalker
            ],
            "{id}"
        );
        let groups = &f.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert_eq!(f.triggered_abilities.len(), 0, "{id}");
        assert_eq!(f.static_abilities.len(), 0, "{id}");
    }

    let eat = face(
        "eat_to_extinction",
        "Eat to Extinction",
        "{3}{B}",
        &["Instant"],
    );
    assert!(matches!(
        eat.spell_effect.as_slice(),
        [
            SpellEffectKind::Exile { .. },
            SpellEffectKind::LibraryPartition {
                count: 1,
                kind: LibraryPartitionKind::Surveil,
                ..
            }
        ]
    ));

    let contempt = face(
        "vraskas_contempt",
        "Vraska's Contempt",
        "{2}{B}{B}",
        &["Instant"],
    );
    assert!(matches!(
        contempt.spell_effect.as_slice(),
        [
            SpellEffectKind::Exile { .. },
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),
                ..
            }
        ]
    ));
}

#[test]
fn q60_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "artisans_sorrow",
            "Artisan's Sorrow",
            "19484d1fe22485b96acf65fda2af46ce3ef6cec1992bd4fcbe36893fff7020d7",
        ),
        (
            "crack_open",
            "Crack Open",
            "feb33bc13e8fbcb13feacabea4ecbcee75e2f91481064805d4043342df11ca41",
        ),
        (
            "expose_to_daylight",
            "Expose to Daylight",
            "d4f24bcd99e1d9bcb642e4fa5a26bfa11d290f9bfe252b2d3fe5d65c56bc55ff",
        ),
        (
            "ray_of_distortion",
            "Ray of Distortion",
            "220e7505cba1b12286d2dddcfb0d4e34e5e888ac079cfd8042a8c5cd3ad9ec25",
        ),
        (
            "slice_in_twain",
            "Slice in Twain",
            "2ef7176fb0cb8dffbfb9dfb54555ab6b63b26e5fa53ca6d1fa8fd3014f936daf",
        ),
        (
            "infiltrate",
            "Infiltrate",
            "f438deb8a737b9848a1211ea1c33830ef7bf37a23f98aa8cf18baa3178a59658",
        ),
        (
            "trailblazer",
            "Trailblazer",
            "f438deb8a737b9848a1211ea1c33830ef7bf37a23f98aa8cf18baa3178a59658",
        ),
        (
            "artful_dodge",
            "Artful Dodge",
            "a1986d08e07cd853562edd8acce114c28a9f3616eae364fcb7e0de3e05f878a6",
        ),
        (
            "touch_of_invisibility",
            "Touch of Invisibility",
            "a1e42ed155804fd7ed91d2d354eee8fa4d80b8b6480bb4583f6478ea8be92f72",
        ),
        (
            "feed_the_serpent",
            "Feed the Serpent",
            "d1589dd92b376a13ec9b87d874be04f33f3766c1360a98cab329f6031ba1feed",
        ),
        (
            "eat_to_extinction",
            "Eat to Extinction",
            "16ac852109b35ce991a3d9c0880302fa8d30a75f4784c998ec084cdd5002329d",
        ),
        (
            "vraskas_contempt",
            "Vraska's Contempt",
            "fe46c92a6626d586e05618e345eb59d834c0ddc56efe4a453d5c3ea275f087b1",
        ),
    ] {
        let row = format!("{id}\t{name}\t{id}\t{name}\t{fingerprint}");
        assert!(
            fingerprints.lines().any(|line| line == row),
            "{id}: missing pinned fingerprint row"
        );
        assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    }
}
