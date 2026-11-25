//! Registry conformance for the queue-v74 01-cycling-control cohort.
//!
//! One removal or recursion spell body plus a hand-zone `cycling` activated ability. Fingerprints
//! are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, EffectSubject, HandCardAction, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::{
    registry, AbilityCost, AbilityPresentation, AbilitySourceZone, CardFace, Layout,
};

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

fn assert_cycling(f: &CardFace, cost: &str, line: u16, id: &str) {
    let ability = f
        .activated_abilities
        .first()
        .unwrap_or_else(|| panic!("{id}: missing cycling ability"));
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![line]),
        "{id}: cycling presentation"
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Hand, "{id}");
    assert!(
        matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(mana), AbilityCost::DiscardSelf]
                if mana.to_string() == cost
        ),
        "{id}: cycling cost {cost}"
    );
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }]
        ),
        "{id}: cycling draws one"
    );
}

#[test]
fn q74_cycling_control_map_definitions() {
    for (id, name, mana, cost, types) in [
        ("flame_jet", "Flame Jet", "{1}{R}", "{2}", &["Sorcery"][..]),
        ("rescind", "Rescind", "{1}{U}{U}", "{2}", &["Instant"][..]),
        ("thornado", "Thornado", "{2}{G}", "{1}{G}", &["Instant"][..]),
        ("unburden", "Unburden", "{1}{B}{B}", "{2}", &["Sorcery"][..]),
        (
            "wander_in_death",
            "Wander in Death",
            "{2}{B}",
            "{2}",
            &["Sorcery"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert!(f.flashback_cost.is_none(), "{id}: no flashback");
        assert!(f.static_abilities.is_empty(), "{id}: no static ability");
        assert_cycling(f, cost, 2, id);
    }

    let flame = face("flame_jet", "Flame Jet", "{1}{R}", &["Sorcery"]);
    assert!(
        matches!(
            flame.spell_effect.as_slice(),
            [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target,
            }] if target.any_of.as_ref().is_some_and(|leaves| {
                leaves.len() == 2
                    && leaves[0].kind == TargetKind::AnyPlayer
                    && leaves[1].kind == TargetKind::AnyPermanent
                    && leaves[1].permanent_types == vec![PermanentTypeFilter::Planeswalker]
            })
        ),
        "Flame Jet damages a player or planeswalker for three"
    );

    let rescind = face("rescind", "Rescind", "{1}{U}{U}", &["Instant"]);
    assert!(
        matches!(
            rescind.spell_effect.as_slice(),
            [SpellEffectKind::ReturnToOwnersHand {
                subject: EffectSubject::Chosen(filter),
            }] if filter.kind == TargetKind::AnyPermanent
        ),
        "Rescind returns any permanent to its owner's hand"
    );

    let thornado = face("thornado", "Thornado", "{2}{G}", &["Instant"]);
    assert!(
        matches!(
            thornado.spell_effect.as_slice(),
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(filter),
            }] if filter.kind == TargetKind::Creature
                && filter.required_keywords.as_slice() == [tricerules_cards::primitives::Keyword::Flying]
        ),
        "Thornado destroys a flying creature"
    );

    let unburden = face("unburden", "Unburden", "{1}{B}{B}", &["Sorcery"]);
    assert!(
        matches!(
            unburden.spell_effect.as_slice(),
            [SpellEffectKind::ChooseHandCards {
                action: HandCardAction::Discard,
                count: 2,
                target,
                ..
            }] if target.kind == TargetKind::AnyPlayer
        ),
        "Unburden makes the chosen player discard two"
    );

    let wander = face("wander_in_death", "Wander in Death", "{2}{B}", &["Sorcery"]);
    let targeting = wander.targeting.as_ref().expect("Wander targeting");
    let group = targeting.groups.first().expect("Wander target group");
    assert_eq!((group.min, group.max), (0, 2));
    assert_eq!(group.effect_indices, [0]);
    assert!(
        format!("{:?}", wander.spell_effect).contains("MoveGraveyardCards")
            && format!("{:?}", wander.spell_effect).contains("Creature")
            && format!("{:?}", wander.spell_effect).contains("Hand"),
        "Wander in Death returns creature cards to hand"
    );
}

#[test]
fn q74_cycling_control_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "flame_jet",
            "Flame Jet",
            "flame_jet",
            "7014c96b494e1aee4cba186079c0f7e44a0af6a6381a971f095514b5a238160c",
        ),
        (
            "rescind",
            "Rescind",
            "rescind",
            "59e6e551700e84f316992b159d6479a929fd2c5919a47a9032d84a5dcdf3f14b",
        ),
        (
            "thornado",
            "Thornado",
            "thornado",
            "3b43759c3ba41f0d24d5a286327bf086423bf718a787d279899bdb1a5a4ae385",
        ),
        (
            "unburden",
            "Unburden",
            "unburden",
            "a60ffa3572fbe46fa1656d95e596232ccaee24bcff444b15717539553d73524f",
        ),
        (
            "wander_in_death",
            "Wander in Death",
            "wander_in_death",
            "d2e573d6ba842f34a2e97ed74488761e50376bad3592bb970b9858e62b307ad4",
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
