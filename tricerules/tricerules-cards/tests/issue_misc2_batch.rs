//! Registry conformance for the reviewed spells direct-RON batch.
//!
//! Playful Shove, Risky Shortcut, Blight Rot, Bounce Off, Plunge into Winter and Mind Spring were
//! promoted after complete-definition review against the pinned Scryfall snapshot (exact records
//! and `rulings_uri` fetched 2026-09-21). Governance: CR 107.3 (X), 115 (targets), 119/120
//! (life and damage), 121.1 (draw), 122.1 (counters), 701.18 (scry), 701.26 (tap), and 400.7
//! (return to owner's hand).

use tricerules_cards::primitives::{
    Amount, EffectSubject, LifeAmount, PermanentTypeFilter, PlayerRecipient, SpellEffectKind,
    TargetKind,
};
use tricerules_cards::{CardRegistry, Layout};

fn primary<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn issue_misc2_batch_maps_definitions() {
    let registry = CardRegistry::global();

    for (id, name, mana, types) in [
        ("playful_shove", "Playful Shove", "{1}{R}", &["Sorcery"][..]),
        (
            "risky_shortcut",
            "Risky Shortcut",
            "{2}{B}",
            &["Sorcery"][..],
        ),
        ("blight_rot", "Blight Rot", "{2}{B}", &["Instant"][..]),
        ("bounce_off", "Bounce Off", "{U}", &["Instant"][..]),
        (
            "plunge_into_winter",
            "Plunge into Winter",
            "{1}{W}",
            &["Instant"][..],
        ),
        ("mind_spring", "Mind Spring", "{X}{U}{U}", &["Sorcery"][..]),
    ] {
        let definition = registry.get(id).expect("registered");
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
    }

    // Playful Shove: 1 damage to any target, then draw a card.
    let shove = primary(registry, "playful_shove");
    assert!(
        matches!(&shove.spell_effect[0], SpellEffectKind::DamageTarget { amount, target }
            if *amount == Amount::Fixed(1) && target.kind == TargetKind::AnyTarget),
        "{:?}",
        shove.spell_effect[0]
    );
    assert!(
        matches!(&shove.spell_effect[1], SpellEffectKind::Draw { count, .. }
            if *count == Amount::Fixed(1)),
        "{:?}",
        shove.spell_effect[1]
    );
    assert_eq!(
        shove.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    // Risky Shortcut: draw two, then each player loses two life.
    let shortcut = primary(registry, "risky_shortcut");
    assert!(
        matches!(&shortcut.spell_effect[0], SpellEffectKind::Draw { count, .. }
            if *count == Amount::Fixed(2)),
        "{:?}",
        shortcut.spell_effect[0]
    );
    assert!(
        matches!(&shortcut.spell_effect[1], SpellEffectKind::LoseLife { amount, who }
            if *amount == LifeAmount::Fixed(2) && *who == PlayerRecipient::EachPlayer),
        "{:?}",
        shortcut.spell_effect[1]
    );

    // Blight Rot: four -1/-1 counters on a target creature.
    let blight = primary(registry, "blight_rot");
    assert!(
        matches!(&blight.spell_effect[0], SpellEffectKind::PutCounters { counter, count, subject: EffectSubject::Chosen(filter) }
            if *counter == tricerules_cards::primitives::CounterKind::MinusOneMinusOne
                && *count == Amount::Fixed(4)
                && filter.kind == TargetKind::Creature),
        "{:?}",
        blight.spell_effect[0]
    );

    // Bounce Off: creature or Vehicle to its owner's hand.
    let bounce = primary(registry, "bounce_off");
    let SpellEffectKind::ReturnToOwnersHand {
        subject: EffectSubject::Chosen(filter),
    } = &bounce.spell_effect[0]
    else {
        panic!("bounce, got {:?}", bounce.spell_effect[0]);
    };
    let any_of = filter.any_of.as_ref().expect("two branches");
    assert!(any_of.iter().any(|f| f.kind == TargetKind::Creature));
    assert!(any_of
        .iter()
        .any(|f| f.required_subtypes.contains(&"Vehicle".to_string())));
    assert!(!any_of
        .iter()
        .any(|f| f.permanent_types.contains(&PermanentTypeFilter::Land)));

    // Plunge into Winter: optional tap, then scry one, then draw a card.
    let plunge = primary(registry, "plunge_into_winter");
    assert!(
        matches!(&plunge.spell_effect[0], SpellEffectKind::Tap { subject: EffectSubject::Chosen(filter) }
            if filter.kind == TargetKind::Creature),
        "{:?}",
        plunge.spell_effect[0]
    );
    assert_eq!(
        plunge.spell_effect[1],
        SpellEffectKind::Scry {
            count: Amount::Fixed(1)
        }
    );
    assert!(
        matches!(&plunge.spell_effect[2], SpellEffectKind::Draw { count, .. }
            if *count == Amount::Fixed(1)),
        "{:?}",
        plunge.spell_effect[2]
    );
    let [group] = plunge.targeting.as_ref().unwrap().groups.as_slice() else {
        panic!("one group");
    };
    assert_eq!((group.min, group.max), (0, 1));
    assert_eq!(group.effect_indices, [0]);

    // Mind Spring: draw X cards.
    let spring = primary(registry, "mind_spring");
    assert!(
        matches!(&spring.spell_effect[0], SpellEffectKind::Draw { count, .. } if *count == Amount::X),
        "{:?}",
        spring.spell_effect[0]
    );
}

#[test]
fn issue_misc2_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "playful_shove",
            "Playful Shove",
            "c06a7216b1d99da8da1a7cb2623de17a7ce32628374b13a82e2b848848ba9dd5",
        ),
        (
            "risky_shortcut",
            "Risky Shortcut",
            "1b33979cb1e25e6f7e8d1533a6548a534b0627e3329b29aeff83e0e00f3bc4fd",
        ),
        (
            "blight_rot",
            "Blight Rot",
            "069005cb7dea0f1cc6b49b6a604f77b56f63e0e6b8f3c5795e1c510822959b46",
        ),
        (
            "bounce_off",
            "Bounce Off",
            "c42dd792d303e186338673e10c8184ee7b8754e1706fcc674880e5176a4a1ecb",
        ),
        (
            "plunge_into_winter",
            "Plunge into Winter",
            "2f25e87ce72997161646a1c7a5f5381afce591d317b363b6bd44de07408f1662",
        ),
        (
            "mind_spring",
            "Mind Spring",
            "8b473e0c57b847ecc575c46c2d8b9ab4b8d204f5e18ddbafa67800ad855d6b6b",
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
