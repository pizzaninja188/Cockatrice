//! Registry conformance for the queue-v58 batch A cohorts.
//!
//! Each included card is a normal-layout single-face, Commander-legal instant or sorcery whose
//! typed definition is one of three reviewed shapes: an uncounterable spell with one effect, a
//! counterspell with one rider, or a destroy-target-enchantment spell with an optional rider.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    AbilitySourceZone, Amount, CardTypeFilter, EffectSubject, LibraryPartitionKind,
    PermanentTypeFilter, SpellEffectKind, StaticAbilityDef, TargetKind,
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
fn q58_uncounterable_map_definitions() {
    let last_word = face("last_word", "Last Word", "{2}{U}{U}", &["Instant"]);
    assert_eq!(
        last_word.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        last_word.static_abilities[0].definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
    let [SpellEffectKind::CounterTargetSpell { spell_filter, .. }] =
        last_word.spell_effect.as_slice()
    else {
        panic!(
            "Last Word must counter one spell: {:?}",
            last_word.spell_effect
        );
    };
    assert_eq!(spell_filter.card_type, None);

    let veto = face("dovins_veto", "Dovin's Veto", "{W}{U}", &["Instant"]);
    assert_eq!(
        veto.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        veto.static_abilities[0].definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
    let [SpellEffectKind::CounterTargetSpell { spell_filter, .. }] = veto.spell_effect.as_slice()
    else {
        panic!(
            "Dovin's Veto must counter one spell: {:?}",
            veto.spell_effect
        );
    };
    assert_eq!(
        spell_filter.card_type,
        Some(CardTypeFilter::Noncreature),
        "only a noncreature spell is legal"
    );

    let void_rend = face("void_rend", "Void Rend", "{W}{U}{B}", &["Instant"]);
    assert_eq!(
        void_rend.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        void_rend.static_abilities[0].definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
    let [SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(filter),
    }] = void_rend.spell_effect.as_slice()
    else {
        panic!(
            "Void Rend must destroy one chosen permanent: {:?}",
            void_rend.spell_effect
        );
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(
        filter.excluded_permanent_types,
        [PermanentTypeFilter::Land],
        "a land is not a legal target"
    );
    assert!(filter.permanent_types.is_empty());
    assert_eq!(
        void_rend.targeting.as_ref().unwrap().groups[0].effect_indices,
        [0]
    );

    let wreak = face("wreak_havoc", "Wreak Havoc", "{2}{R}{G}", &["Sorcery"]);
    assert_eq!(
        wreak.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        wreak.static_abilities[0].definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
    let [SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(filter),
    }] = wreak.spell_effect.as_slice()
    else {
        panic!(
            "Wreak Havoc must destroy one chosen permanent: {:?}",
            wreak.spell_effect
        );
    };
    let any_of = filter.any_of.as_ref().expect("artifact-or-land union");
    assert_eq!(any_of.len(), 2);
    assert_eq!(any_of[0].kind, TargetKind::AnyPermanent);
    assert_eq!(any_of[0].permanent_types, [PermanentTypeFilter::Artifact]);
    assert_eq!(any_of[1].kind, TargetKind::AnyPermanent);
    assert_eq!(any_of[1].permanent_types, [PermanentTypeFilter::Land]);

    let verdict = face(
        "supreme_verdict",
        "Supreme Verdict",
        "{1}{W}{W}{U}",
        &["Sorcery"],
    );
    assert_eq!(
        verdict.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        verdict.static_abilities[0].definition,
        StaticAbilityDef::SpellCannotBeCountered
    );
    let [SpellEffectKind::DestroyAll {
        kind,
        prevent_regeneration,
    }] = verdict.spell_effect.as_slice()
    else {
        panic!(
            "Supreme Verdict must destroy all creatures: {:?}",
            verdict.spell_effect
        );
    };
    assert_eq!(kind.kind, TargetKind::Creature);
    assert!(!prevent_regeneration);
    assert!(verdict.targeting.is_none());
}

#[test]
fn q58_counter_rider_map_definitions() {
    let denial = face(
        "fervent_denial",
        "Fervent Denial",
        "{3}{U}{U}",
        &["Instant"],
    );
    assert_eq!(
        denial
            .flashback_cost
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("{5}{U}{U}")
    );
    assert!(matches!(
        denial.spell_effect.as_slice(),
        [SpellEffectKind::CounterTargetSpell { .. }]
    ));

    let neutralize = face("neutralize", "Neutralize", "{1}{U}{U}", &["Instant"]);
    assert!(matches!(
        neutralize.spell_effect.as_slice(),
        [SpellEffectKind::CounterTargetSpell { .. }]
    ));
    let [cycling] = neutralize.activated_abilities.as_slice() else {
        panic!("Neutralize has exactly one cycling ability");
    };
    assert_eq!(cycling.ability_id.as_str(), "activated_01");
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
    assert_eq!(cycling.costs.len(), 2);
    assert!(matches!(
        &cycling.costs[0],
        tricerules_cards::primitives::AbilityCost::Mana(cost) if cost.to_string() == "{2}"
    ));
    assert!(matches!(
        cycling.costs[1],
        tricerules_cards::primitives::AbilityCost::DiscardSelf
    ));
    assert!(matches!(
        cycling.effect.as_slice(),
        [SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            ..
        }]
    ));

    let sabotage = face(
        "sinister_sabotage",
        "Sinister Sabotage",
        "{1}{U}{U}",
        &["Instant"],
    );
    let [SpellEffectKind::CounterTargetSpell { .. }, SpellEffectKind::LibraryPartition {
        count,
        top_min,
        top_max,
        kind,
    }] = sabotage.spell_effect.as_slice()
    else {
        panic!(
            "Sinister Sabotage must counter, then surveil 1: {:?}",
            sabotage.spell_effect
        );
    };
    assert_eq!(*count, 1);
    assert_eq!(*top_min, 0);
    assert_eq!(*top_max, None);
    assert_eq!(*kind, LibraryPartitionKind::Surveil);

    let visions = face(
        "traumatic_visions",
        "Traumatic Visions",
        "{3}{U}{U}",
        &["Instant"],
    );
    assert!(matches!(
        visions.spell_effect.as_slice(),
        [SpellEffectKind::CounterTargetSpell { .. }]
    ));
    let [typecycle] = visions.activated_abilities.as_slice() else {
        panic!("Traumatic Visions has exactly one basic landcycling ability");
    };
    assert_eq!(typecycle.ability_id.as_str(), "activated_01");
    assert_eq!(
        typecycle.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(typecycle.source_zone, AbilitySourceZone::Hand);
    assert!(matches!(
        &typecycle.costs[0],
        tricerules_cards::primitives::AbilityCost::Mana(cost) if cost.to_string() == "{1}{U}"
    ));
    assert!(matches!(
        typecycle.costs[1],
        tricerules_cards::primitives::AbilityCost::DiscardSelf
    ));
    let [SpellEffectKind::SearchLibrary {
        filter: Some(filter),
        reveal,
        destination,
        ..
    }] = typecycle.effect.as_slice()
    else {
        panic!(
            "Traumatic Visions must search a basic land: {:?}",
            typecycle.effect
        );
    };
    assert_eq!(filter.card_type, Some(CardTypeFilter::BasicLand));
    assert!(reveal, "basic landcycling reveals the found card");
    assert_eq!(
        *destination,
        tricerules_cards::primitives::SearchDestination::Hand
    );
}

#[test]
fn q58_destroy_enchantment_map_definitions() {
    for (id, name, mana, types) in [
        ("aura_blast", "Aura Blast", "{1}{W}", &["Instant"][..]),
        ("clear", "Clear", "{1}{W}", &["Instant"][..]),
        ("demystify", "Demystify", "{W}", &["Instant"][..]),
        (
            "quiet_purity",
            "Quiet Purity",
            "{W}",
            &["Instant", "Arcane"][..],
        ),
        (
            "ray_of_revelation",
            "Ray of Revelation",
            "{1}{W}",
            &["Instant"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        let Some(SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        }) = f.spell_effect.first()
        else {
            panic!(
                "{id} must destroy one chosen enchantment: {:?}",
                f.spell_effect
            );
        };
        assert_eq!(filter.kind, TargetKind::AnyPermanent, "{id}");
        assert_eq!(
            filter.permanent_types,
            [PermanentTypeFilter::Enchantment],
            "{id}"
        );
        assert_eq!(
            f.targeting.as_ref().unwrap().groups[0].effect_indices,
            [0],
            "{id}"
        );
        assert_eq!(f.triggered_abilities.len(), 0, "{id}");
        assert_eq!(f.static_abilities.len(), 0, "{id}");
    }

    let blast = face("aura_blast", "Aura Blast", "{1}{W}", &["Instant"]);
    assert!(matches!(
        blast.spell_effect.as_slice(),
        [
            SpellEffectKind::Destroy { .. },
            SpellEffectKind::Draw { .. }
        ]
    ));

    let clear = face("clear", "Clear", "{1}{W}", &["Instant"]);
    let [cycling] = clear.activated_abilities.as_slice() else {
        panic!("Clear has exactly one cycling ability");
    };
    assert_eq!(
        cycling.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
    assert!(matches!(
        &cycling.costs[0],
        tricerules_cards::primitives::AbilityCost::Mana(cost) if cost.to_string() == "{2}"
    ));

    let ray = face(
        "ray_of_revelation",
        "Ray of Revelation",
        "{1}{W}",
        &["Instant"],
    );
    assert_eq!(
        ray.flashback_cost
            .as_ref()
            .map(ToString::to_string)
            .as_deref(),
        Some("{G}")
    );
}

#[test]
fn q58_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "dovins_veto",
            "Dovin's Veto",
            "389338a83d9600e656f935c569a4f6ba039f2f86a0e1c064475aa0d61b635fcc",
        ),
        (
            "last_word",
            "Last Word",
            "5d4c0ebda4cdb76a8e329e71091b7eef4c21e4b745b9575cc5f07095dace97eb",
        ),
        (
            "void_rend",
            "Void Rend",
            "9d31dd99151cb873923cf84e345afb5e87341e6f719cfc78a942859d7afa8e59",
        ),
        (
            "wreak_havoc",
            "Wreak Havoc",
            "138a7d56a1568338852b8e0b376ca47cf26afe651c40b3cbfc6306617875104b",
        ),
        (
            "supreme_verdict",
            "Supreme Verdict",
            "d3ee91b20ebd44207ce4fbf187821033717331831cfd27a23fb42a084711b374",
        ),
        (
            "fervent_denial",
            "Fervent Denial",
            "4a2d0e67d53d23ba3bb833dcac79db51722f61570885868cb85665a4bfc36561",
        ),
        (
            "neutralize",
            "Neutralize",
            "443d085595ed92a859a2f5b3f38ba20433223c6d1c1ca6bddc015f9b7070cca7",
        ),
        (
            "sinister_sabotage",
            "Sinister Sabotage",
            "82132cce74ba09f300b016fccbfd3ad3ac2edb54d370dde37109f030636a649b",
        ),
        (
            "traumatic_visions",
            "Traumatic Visions",
            "1c0ddf0afcfdc761b104c51df5e1978ac312404a1bbb3a39dcc6f80d942dfac7",
        ),
        (
            "aura_blast",
            "Aura Blast",
            "fc6947ab7fcce8260d2167408c190ce65109450a96fb1aeb4ad2736b049b0af3",
        ),
        (
            "clear",
            "Clear",
            "e691ffae6235da72bc1a83c156933eb1160085e5b99e676dfa6d017c47d3fec2",
        ),
        (
            "demystify",
            "Demystify",
            "61285fdb4d05d32de93fc8ae58649794ef4ca9ae64a0b0a202243658b4619afb",
        ),
        (
            "quiet_purity",
            "Quiet Purity",
            "61285fdb4d05d32de93fc8ae58649794ef4ca9ae64a0b0a202243658b4619afb",
        ),
        (
            "ray_of_revelation",
            "Ray of Revelation",
            "dbc9748ea31e6618727297f809ec62883a938d2ba1f9326618e670e3e05ae2f4",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[1], name);
        assert_eq!(
            fields[4], fingerprint,
            "pinned Oracle fingerprint drift for {id}"
        );
    }
}
