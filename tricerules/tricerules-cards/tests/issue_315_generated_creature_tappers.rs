//! Registry and presentation conformance for issue #315.

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivationTiming, EffectSubject, PermanentTypeFilter,
    SpellEffectKind, TargetFilter, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, ManaCost};

#[derive(Clone)]
struct CohortFixture {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    activation_cost: &'static str,
    prompt: &'static str,
    target_filter: TargetFilter,
    fingerprint: &'static str,
}

fn cohort() -> [CohortFixture; 3] {
    [
        CohortFixture {
            id: "coeurl",
            name: "Coeurl",
            mana: "{1}{W}",
            types: &["Creature", "Cat", "Beast"],
            power: 2,
            toughness: 2,
            activation_cost: "{1}{W}",
            prompt: "Choose target nonenchantment creature",
            target_filter: TargetFilter {
                kind: TargetKind::Creature,
                excluded_permanent_types: vec![PermanentTypeFilter::Enchantment],
                ..TargetFilter::default()
            },
            fingerprint: "d2295d4738496a7e194c914e3c20bd99eee465495fe71097d2ff437f2db7515a",
        },
        CohortFixture {
            id: "frostbridge_guard",
            name: "Frostbridge Guard",
            mana: "{1}{W}",
            types: &["Creature", "Elemental", "Soldier"],
            power: 2,
            toughness: 2,
            activation_cost: "{2}{W}",
            prompt: "Choose target creature",
            target_filter: TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            },
            fingerprint: "560c7551471b8089cf3d507a9698525df20b69f3944797c236f5ed4c6af7b480",
        },
        CohortFixture {
            id: "sterling_keykeeper",
            name: "Sterling Keykeeper",
            mana: "{1}{W}",
            types: &["Creature", "Human", "Mercenary"],
            power: 2,
            toughness: 2,
            activation_cost: "{2}",
            prompt: "Choose target non-Mount creature",
            target_filter: TargetFilter {
                kind: TargetKind::Creature,
                excluded_subtypes: vec!["Mount".into()],
                ..TargetFilter::default()
            },
            fingerprint: "47af1810f1ea6ace506cd5dfa83fd2b32b9e3ed8234ad3b8ae6b3e7a77f73193",
        },
    ]
}

#[test]
fn issue_315_registry_contains_exactly_the_reviewed_creature_tapper_cohort() {
    let registry = CardRegistry::global();
    for card in cohort() {
        let CohortFixture {
            id,
            name,
            mana,
            types,
            power,
            toughness,
            activation_cost,
            prompt,
            ref target_filter,
            fingerprint,
        } = card;
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing reviewed card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(definition.face_count(), 1);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana);
        assert_eq!(face.types, types);
        assert_eq!((face.power, face.toughness), (Some(power), Some(toughness)));
        assert_eq!(face.colors(), vec![Color::White]);
        assert!(face.keywords.is_empty());
        assert!(face.triggered_abilities.is_empty());
        assert!(face.static_abilities.is_empty());
        assert!(face.custom_effect.is_none());
        assert!(face.modal_spell.is_none());

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{name} must have exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
        assert_eq!(ability.timing, ActivationTiming::Normal);
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(ManaCost::parse(activation_cost).unwrap()),
                AbilityCost::Tap,
            ]
        );
        assert!(ability.cost_modifiers.is_empty());
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Tap {
                subject: EffectSubject::Chosen(Box::new(target_filter.clone())),
            }]
        );
        assert!(ability.conditions.is_empty());
        assert!(ability.activation_limit.is_none());
        let targeting = ability
            .targeting
            .as_ref()
            .expect("tap ability should target one creature");
        let [group] = targeting.groups.as_slice() else {
            panic!("{name} should have exactly one target group");
        };
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.prompt, prompt);
        assert_eq!(group.effect_indices, [0]);
        assert!(group.distinct_from.is_empty());
        assert!(!group.same_graveyard);
        assert!(group.cast_cost_expansion.is_none());

        let presentation = registry
            .presentation_face(id, face.face_id.as_str())
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}"));
        assert_eq!(presentation.card_name, name);
        assert_eq!(presentation.face_name, name);
        assert_eq!(presentation.oracle_text_sha256, fingerprint);
    }

    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in cohort() {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        assert!(
            row.ends_with(card.fingerprint),
            "fingerprint drift for {}: {row}",
            card.id
        );
    }
}
