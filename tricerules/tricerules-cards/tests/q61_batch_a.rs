//! Registry conformance for the queue-v61 batch A cohorts.
//!
//! Each included card is a normal-layout single-face, Commander-legal card whose typed definition
//! is one of three reviewed shapes: a sorcery that destroys a land and then applies one printed
//! rider, a sorcery that makes one player lose life and gains its controller the same life, or an
//! instant that pumps one creature, grants one keyword and draws a card. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, Amount, CardTypeFilter, EffectSubject, PermanentTypeFilter,
    SpellEffectKind, TargetKind,
};
use tricerules_cards::{registry, CardFace, Keyword, Layout};

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

fn assert_land_target(subject: &EffectSubject, id: &str) {
    let EffectSubject::Chosen(filter) = subject else {
        panic!("{id}: expected a chosen subject, got {subject:?}");
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent, "{id}");
    assert_eq!(filter.permanent_types, [PermanentTypeFilter::Land], "{id}");
}

#[test]
fn q61_land_destruction_map_definitions() {
    for (id, name, mana) in [
        ("earth_rift", "Earth Rift", "{3}{R}"),
        ("frenzied_tilling", "Frenzied Tilling", "{3}{R}{G}"),
        ("implode", "Implode", "{4}{R}"),
        ("lay_waste", "Lay Waste", "{3}{R}"),
        ("rubble_reading", "Rubble Reading", "{3}{R}"),
    ] {
        let f = face(id, name, mana, &["Sorcery"]);
        let [SpellEffectKind::Destroy { subject }, ..] = f.spell_effect.as_slice() else {
            panic!("{id}: must destroy one chosen land: {:?}", f.spell_effect);
        };
        assert_land_target(subject, id);
        let groups = &f.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert_eq!(f.triggered_abilities.len(), 0, "{id}");
        assert_eq!(f.static_abilities.len(), 0, "{id}");
    }

    let rift = face("earth_rift", "Earth Rift", "{3}{R}", &["Sorcery"]);
    assert_eq!(
        rift.flashback_cost
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("{5}{R}{R}")
    );

    let tilling = face(
        "frenzied_tilling",
        "Frenzied Tilling",
        "{3}{R}{G}",
        &["Sorcery"],
    );
    assert!(matches!(
        tilling.spell_effect.as_slice(),
        [
            SpellEffectKind::Destroy { .. },
            SpellEffectKind::SearchLibrary {
                filter: Some(filter),
                count: 1,
                optional: false,
                ..
            },
        ] if filter.card_type == Some(CardTypeFilter::BasicLand)
    ));

    let implode = face("implode", "Implode", "{4}{R}", &["Sorcery"]);
    assert!(matches!(
        implode.spell_effect.as_slice(),
        [
            SpellEffectKind::Destroy { .. },
            SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            },
        ]
    ));

    let rubble = face("rubble_reading", "Rubble Reading", "{3}{R}", &["Sorcery"]);
    assert!(matches!(
        rubble.spell_effect.as_slice(),
        [
            SpellEffectKind::Destroy { .. },
            SpellEffectKind::Scry {
                count: Amount::Fixed(2)
            },
        ]
    ));

    let lay = face("lay_waste", "Lay Waste", "{3}{R}", &["Sorcery"]);
    assert_eq!(lay.spell_effect.len(), 1);
    assert_eq!(lay.activated_abilities.len(), 1);
    let cycling = &lay.activated_abilities[0];
    assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
    assert!(matches!(
        cycling.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"
    ));
    assert!(matches!(
        cycling.effect.as_slice(),
        [SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            ..
        }]
    ));
}

#[test]
fn q61_drain_life_map_definitions() {
    for (id, name, mana, amount, draws) in [
        ("last_caress", "Last Caress", "{2}{B}", 1, true),
        ("soul_feast", "Soul Feast", "{3}{B}{B}", 4, false),
        ("sovereigns_bite", "Sovereign's Bite", "{1}{B}", 3, false),
    ] {
        let f = face(id, name, mana, &["Sorcery"]);
        let [SpellEffectKind::TargetPlayerLosesLife {
            amount: loss,
            target,
        }, SpellEffectKind::GainLife { amount: gain, .. }, ..] = f.spell_effect.as_slice()
        else {
            panic!("{id}: must drain one player: {:?}", f.spell_effect);
        };
        assert_eq!(*loss, amount, "{id}");
        assert_eq!(target.kind, TargetKind::AnyPlayer, "{id}");
        assert_eq!(*gain, Amount::Fixed(amount), "{id}");
        let groups = &f.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(groups[0].effect_indices, [0], "{id}");
        assert_eq!(
            f.spell_effect.len(),
            if draws { 3 } else { 2 },
            "{id}: draw only where printed"
        );
        if draws {
            assert!(matches!(
                f.spell_effect[2],
                SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }
            ));
        }
        assert_eq!(f.triggered_abilities.len(), 0, "{id}");
        assert_eq!(f.static_abilities.len(), 0, "{id}");
    }
}

#[test]
fn q61_pump_keyword_draw_map_definitions() {
    for (id, name, mana, power, toughness, keyword) in [
        (
            "guided_strike",
            "Guided Strike",
            "{1}{W}",
            1,
            0,
            Keyword::FirstStrike,
        ),
        (
            "moment_of_defiance",
            "Moment of Defiance",
            "{2}{B}",
            2,
            1,
            Keyword::Lifelink,
        ),
        ("wildsize", "Wildsize", "{2}{G}", 2, 2, Keyword::Trample),
    ] {
        let f = face(id, name, mana, &["Instant"]);
        let [SpellEffectKind::PumpTarget {
            power: actual_power,
            toughness: actual_toughness,
            subject: pump_subject,
            ..
        }, SpellEffectKind::GrantKeywords {
            subject: keyword_subject,
            keywords,
        }, SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            ..
        }] = f.spell_effect.as_slice()
        else {
            panic!("{id}: unexpected pump shape: {:?}", f.spell_effect);
        };
        assert_eq!(
            (*actual_power, *actual_toughness),
            (power, toughness),
            "{id}"
        );
        assert_eq!(keywords.as_slice(), [keyword], "{id}");
        for subject in [pump_subject, keyword_subject] {
            let EffectSubject::Chosen(filter) = subject else {
                panic!("{id}: expected a chosen subject, got {subject:?}");
            };
            assert_eq!(filter.kind, TargetKind::Creature, "{id}");
        }
        let groups = &f.targeting.as_ref().expect("targeting").groups;
        assert_eq!(groups.len(), 1, "{id}");
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{id}");
        assert_eq!(groups[0].effect_indices, [0, 1], "{id}");
        assert_eq!(f.triggered_abilities.len(), 0, "{id}");
        assert_eq!(f.static_abilities.len(), 0, "{id}");
    }
}

#[test]
fn q61_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "earth_rift",
            "Earth Rift",
            "a3add86f2aa14db0907027bf6bb34e135660ca219bc79639b8db040eb3bea56b",
        ),
        (
            "frenzied_tilling",
            "Frenzied Tilling",
            "29d544ad59f0c224931900af2bc7f538339843bf850831706cc6b13d29b33e0d",
        ),
        (
            "implode",
            "Implode",
            "b0148b79354d29a6085eba1754770bc515755a8803d31bcbdebe40feb9e21bda",
        ),
        (
            "lay_waste",
            "Lay Waste",
            "b5b308d885910fb5b9de65a3fc4bc150991e22cdb0d83cb5666bafdf6f27c244",
        ),
        (
            "rubble_reading",
            "Rubble Reading",
            "390df0cd18c2fcdb6379bac1b4e7258d321d2b15259912086879d0637504cd07",
        ),
        (
            "last_caress",
            "Last Caress",
            "82b3d1d699e4cef325569b740da47d496e47fff9ca56942f762368fd6ca4af32",
        ),
        (
            "soul_feast",
            "Soul Feast",
            "19b915d22844cff8de932e4ebfc638ec6617ff1e5321e38de76497222fb11a57",
        ),
        (
            "sovereigns_bite",
            "Sovereign's Bite",
            "9a3daca3ce4c4ec22fba75f7f243a357f3319901ac7548b04b1216565a9dd0a5",
        ),
        (
            "guided_strike",
            "Guided Strike",
            "d41a1811ea926a000f8ffba713de3682e24b1447d68c6c81214dca44b706c94b",
        ),
        (
            "moment_of_defiance",
            "Moment of Defiance",
            "6d83ec1c6b42fb35680939789f63ee317c3563e3149b09ea2b4b41a0742a89b8",
        ),
        (
            "wildsize",
            "Wildsize",
            "7ca12c4bd66c779bc2abca92d91d9e1894530ec27dbb3a4724e681d398ecd1da",
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
