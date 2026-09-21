//! Registry conformance for the reviewed removal/damage one-shot direct-RON batch.
//!
//! Lightning Strike, Flame Lash, Seismic Rupture, Deathmark, Death in the Family, Epic Downfall,
//! Repel Calamity, Eriette's Lullaby and Grapple with Death were promoted after complete-definition
//! review against the pinned Scryfall snapshot (exact records and `rulings_uri` fetched
//! 2026-09-21). Governance: CR 115 (targets), 120 (damage), 701.8 (destroy), 701.13 (exile),
//! 105.2 (color), 202.3 (mana value), and 119.3 (life gain).

use tricerules_cards::primitives::{
    Amount, EffectSubject, PermanentTypeFilter, PowerComparison, SpellEffectKind, TargetKind,
};
use tricerules_cards::{CardRegistry, Color, Keyword, Layout};

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn issue_removal_batch_maps_definitions() {
    let registry = CardRegistry::global();

    // Identity for the whole batch.
    for (id, name, mana, types) in [
        (
            "lightning_strike",
            "Lightning Strike",
            "{1}{R}",
            &["Instant"][..],
        ),
        ("flame_lash", "Flame Lash", "{3}{R}", &["Instant"][..]),
        (
            "seismic_rupture",
            "Seismic Rupture",
            "{2}{R}",
            &["Sorcery"][..],
        ),
        ("deathmark", "Deathmark", "{B}", &["Sorcery"][..]),
        (
            "death_in_the_family",
            "Death in the Family",
            "{1}{B}",
            &["Instant"][..],
        ),
        ("epic_downfall", "Epic Downfall", "{1}{B}", &["Sorcery"][..]),
        (
            "repel_calamity",
            "Repel Calamity",
            "{1}{W}",
            &["Instant"][..],
        ),
        (
            "eriettes_lullaby",
            "Eriette's Lullaby",
            "{1}{W}",
            &["Sorcery"][..],
        ),
        (
            "grapple_with_death",
            "Grapple with Death",
            "{1}{B}{G}",
            &["Sorcery"][..],
        ),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
    }

    // Any-target damage.
    for (id, amount) in [("lightning_strike", 3u32), ("flame_lash", 4)] {
        let f = face(registry, id);
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id} has one effect");
        };
        assert!(
            matches!(
                effect,
                SpellEffectKind::DamageTarget { amount: a, target }
                    if *a == Amount::Fixed(amount) && target.kind == TargetKind::AnyTarget
            ),
            "{id}: {effect:?}"
        );
    }

    // Seismic Rupture: mass damage excluding flyers.
    let rupture = face(registry, "seismic_rupture");
    let [effect] = rupture.spell_effect.as_slice() else {
        panic!("one effect");
    };
    assert!(
        matches!(
            effect,
            SpellEffectKind::DamageAll { amount, kind, .. }
                if *amount == Amount::Fixed(2)
                    && kind.kind == TargetKind::Creature
                    && kind.excluded_keywords == vec![Keyword::Flying]
        ),
        "{effect:?}"
    );

    // Deathmark: destroy a green or white creature.
    let deathmark = face(registry, "deathmark");
    let [effect] = deathmark.spell_effect.as_slice() else {
        panic!("one effect");
    };
    let SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(filter),
    } = effect
    else {
        panic!("destroy, got {effect:?}");
    };
    let any_of = filter.any_of.as_ref().expect("color branches");
    assert_eq!(any_of.len(), 2);
    assert!(any_of.iter().all(|f| f.kind == TargetKind::Creature));
    assert!(any_of.iter().any(|f| f.is_color == Some(Color::Green)));
    assert!(any_of.iter().any(|f| f.is_color == Some(Color::White)));

    // Mana-value-bounded exile.
    for (id, min, max) in [
        ("death_in_the_family", None, Some(3u32)),
        ("epic_downfall", Some(3), None),
    ] {
        let f = face(registry, id);
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id} one effect");
        };
        let SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(filter),
        } = effect
        else {
            panic!("{id} exile, got {effect:?}");
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        assert_eq!(filter.min_mana_value, min, "{id} min mv");
        assert_eq!(filter.max_mana_value, max, "{id} max mv");
    }

    // Repel Calamity: power or toughness at least 4.
    let repel = face(registry, "repel_calamity");
    let [effect] = repel.spell_effect.as_slice() else {
        panic!("one effect");
    };
    let SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(filter),
    } = effect
    else {
        panic!("destroy, got {effect:?}");
    };
    let any_of = filter.any_of.as_ref().expect("stat branches");
    assert!(any_of
        .iter()
        .any(|f| f.power == Some(PowerComparison::AtLeast(4))));
    assert!(any_of
        .iter()
        .any(|f| f.toughness == Some(PowerComparison::AtLeast(4))));

    // Eriette's Lullaby: destroy a tapped creature, gain two life.
    let lullaby = face(registry, "eriettes_lullaby");
    let [destroy, gain] = lullaby.spell_effect.as_slice() else {
        panic!("destroy then life");
    };
    assert!(
        matches!(destroy, SpellEffectKind::Destroy { subject: EffectSubject::Chosen(filter) }
            if filter.kind == TargetKind::Creature && filter.tapped == Some(true)),
        "{destroy:?}"
    );
    assert_eq!(
        *gain,
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(2)
        }
    );
    assert_eq!(
        lullaby.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    // Grapple with Death: destroy an artifact or creature, gain one life.
    let grapple = face(registry, "grapple_with_death");
    let [destroy, gain] = grapple.spell_effect.as_slice() else {
        panic!("destroy then life");
    };
    let SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(filter),
    } = destroy
    else {
        panic!("destroy, got {destroy:?}");
    };
    let any_of = filter.any_of.as_ref().expect("type branches");
    assert!(any_of.iter().any(|f| f.kind == TargetKind::Creature));
    assert!(any_of
        .iter()
        .any(|f| f.permanent_types.contains(&PermanentTypeFilter::Artifact)));
    assert_eq!(
        *gain,
        SpellEffectKind::GainLife {
            amount: Amount::Fixed(1)
        }
    );
    assert_eq!(
        grapple.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );
}

#[test]
fn issue_removal_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "lightning_strike",
            "Lightning Strike",
            "034d3d8758d78e9cd061ec08cf71bcd1e9f565e222a2626a19f4097b556cc617",
        ),
        (
            "flame_lash",
            "Flame Lash",
            "8dd0e2a3964ace11532b2111ddf6ac1dbb2985ffcaf41ee82549ebce88866467",
        ),
        (
            "seismic_rupture",
            "Seismic Rupture",
            "a1953b097eee822cb1dd7ab3a33e5372fd75eecacdb9e36289962ae35cbb90d0",
        ),
        (
            "deathmark",
            "Deathmark",
            "444722793a04173dbb3b7f51b7d0fb592cbffc9d30e103e53b51cd40d2c8e621",
        ),
        (
            "death_in_the_family",
            "Death in the Family",
            "2ce8eb5f147e66054d35da5f3be32c2fa99de4dbd2ce0ee50ed490a7da6a0d47",
        ),
        (
            "epic_downfall",
            "Epic Downfall",
            "f6ee2548beb628641a8a581d4f52b77a6b5ee3eb1cc3809d8fae92f54994c79d",
        ),
        (
            "repel_calamity",
            "Repel Calamity",
            "0f8c09d5f52a2bd0c502765eba2f26a13ecb177db6e7261499e388200491a3b5",
        ),
        (
            "eriettes_lullaby",
            "Eriette's Lullaby",
            "dd02e890933c16d7b616f120da25ff78dfe5d127b5d49d36ad6ee28348fc9437",
        ),
        (
            "grapple_with_death",
            "Grapple with Death",
            "117a6a3ddcad067d0c99ba83612acb4a671ada7b1d1366063fd1c0c5f4c547cf",
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
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
