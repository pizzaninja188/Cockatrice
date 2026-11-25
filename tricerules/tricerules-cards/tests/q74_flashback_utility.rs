//! Registry conformance for the queue-v74 02-flashback-utility cohort.
//!
//! One utility spell body plus a printed `flashback_cost`. Fingerprints are checked after the
//! presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, CombatRole, EffectSubject, Keyword, SpellEffectKind, TargetKind,
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
fn q74_flashback_utility_map_definitions() {
    for (id, name, mana, flashback, types) in [
        (
            "burning_oil",
            "Burning Oil",
            "{1}{R}",
            "{3}{W}",
            &["Instant"][..],
        ),
        (
            "deep_reconnaissance",
            "Deep Reconnaissance",
            "{2}{G}",
            "{4}{G}",
            &["Sorcery"][..],
        ),
        (
            "defy_gravity",
            "Defy Gravity",
            "{U}",
            "{U}",
            &["Instant"][..],
        ),
        (
            "moments_peace",
            "Moment's Peace",
            "{1}{G}",
            "{2}{G}",
            &["Instant"][..],
        ),
        (
            "strike_it_rich",
            "Strike It Rich",
            "{R}",
            "{2}{R}",
            &["Sorcery"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!(
            f.flashback_cost.as_ref().map(ToString::to_string),
            Some(flashback.to_string()),
            "{id} flashback"
        );
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert!(
            f.activated_abilities.is_empty(),
            "{id}: no activated ability"
        );
        assert!(f.static_abilities.is_empty(), "{id}: no static ability");
    }

    let burning = face("burning_oil", "Burning Oil", "{1}{R}", &["Instant"]);
    assert!(
        matches!(
            burning.spell_effect.as_slice(),
            [SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target,
            }] if target.kind == TargetKind::Creature
                && target.combat_role == Some(CombatRole::AttackingOrBlocking)
        ),
        "Burning Oil damages an attacking or blocking creature for three"
    );
    let group = burning
        .targeting
        .as_ref()
        .expect("Burning Oil targeting")
        .groups
        .first()
        .expect("Burning Oil group");
    assert_eq!((group.min, group.max), (1, 1));

    let deep = face(
        "deep_reconnaissance",
        "Deep Reconnaissance",
        "{2}{G}",
        &["Sorcery"],
    );
    assert!(
        format!("{:?}", deep.spell_effect).contains("SearchLibrary")
            && format!("{:?}", deep.spell_effect).contains("BasicLand")
            && format!("{:?}", deep.spell_effect).contains("tapped: true"),
        "Deep Reconnaissance searches a tapped basic land"
    );

    let defy = face("defy_gravity", "Defy Gravity", "{U}", &["Instant"]);
    assert!(
        matches!(
            defy.spell_effect.as_slice(),
            [SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(filter),
                keywords,
            }] if filter.kind == TargetKind::Creature && keywords == &[Keyword::Flying]
        ),
        "Defy Gravity grants flying"
    );
    let group = defy
        .targeting
        .as_ref()
        .expect("Defy Gravity targeting")
        .groups
        .first()
        .expect("Defy Gravity group");
    assert_eq!((group.min, group.max), (1, 1));

    let peace = face("moments_peace", "Moment's Peace", "{1}{G}", &["Instant"]);
    assert!(
        matches!(
            peace.spell_effect.as_slice(),
            [SpellEffectKind::PreventAllCombatDamageTurn]
        ),
        "Moment's Peace prevents all combat damage this turn"
    );

    let strike = face("strike_it_rich", "Strike It Rich", "{R}", &["Sorcery"]);
    assert!(
        matches!(
            strike.spell_effect.as_slice(),
            [SpellEffectKind::CreateTokens { token, count: Amount::Fixed(1), .. }] if token == "treasure"
        ),
        "Strike It Rich creates one Treasure"
    );
}

#[test]
fn q74_flashback_utility_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "burning_oil",
            "Burning Oil",
            "burning_oil",
            "acb524633161c8ba13cdeb29ffe70b719a6df62d6da82bb5b158ca94207e07c2",
        ),
        (
            "deep_reconnaissance",
            "Deep Reconnaissance",
            "deep_reconnaissance",
            "a3cee7d35e34871cff2b89ff791a8f79e3aa99f9ad00ce268543c13d329eebb0",
        ),
        (
            "defy_gravity",
            "Defy Gravity",
            "defy_gravity",
            "d92e810740db2f298d1a98d9b10594ddb7e0c4871485472f8acda70699bc29a4",
        ),
        (
            "moments_peace",
            "Moment's Peace",
            "moments_peace",
            "23f1f0e323a3c009bef5ce574b877e580a337c8f6961ce81e1415b50f6dab136",
        ),
        (
            "strike_it_rich",
            "Strike It Rich",
            "strike_it_rich",
            "4379c0635e6a380ac7491a4ff115a6130d2edd040bad6277c15153560ca4c1c3",
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
