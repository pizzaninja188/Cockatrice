//! Registry and presentation conformance for issue #314.

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, ActivationTiming, Amount, PlayerRecipient, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, ManaCost};

#[derive(Clone, Copy)]
struct CohortFixture {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    activation_cost: &'static str,
    fingerprint: &'static str,
}

const COHORT: [CohortFixture; 2] = [
    CohortFixture {
        id: "mystic_archaeologist",
        name: "Mystic Archaeologist",
        mana: "{1}{U}",
        types: &["Creature", "Human", "Wizard"],
        power: 2,
        toughness: 1,
        activation_cost: "{3}{U}{U}",
        fingerprint: "8eb16b53cfccdfa6f91c0372e3e88291e7e9cab02c0bba70f3428f660d876adc",
    },
    CohortFixture {
        id: "oscorp_research_team",
        name: "Oscorp Research Team",
        mana: "{3}{U}",
        types: &["Creature", "Human", "Scientist"],
        power: 1,
        toughness: 5,
        activation_cost: "{6}{U}",
        fingerprint: "48b384f670aa4e73098ea594d00004e18ca421e7ac1de27d533937aa3a07617d",
    },
];

#[test]
fn issue_314_registry_contains_exactly_the_reviewed_creature_draw_two_cohort() {
    let registry = CardRegistry::global();
    for card in COHORT {
        let CohortFixture {
            id,
            name,
            mana,
            types,
            power,
            toughness,
            activation_cost,
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
        assert_eq!(face.colors(), vec![Color::Blue]);
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
            [AbilityCost::Mana(ManaCost::parse(activation_cost).unwrap())]
        );
        assert!(ability.cost_modifiers.is_empty());
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            }]
        );
        assert!(ability.targeting.is_none());
        assert!(ability.conditions.is_empty());
        assert!(ability.activation_limit.is_none());

        let presentation = registry
            .presentation_face(id, face.face_id.as_str())
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}"));
        assert_eq!(presentation.card_name, name);
        assert_eq!(presentation.face_name, name);
        assert_eq!(presentation.oracle_text_sha256, fingerprint);
    }

    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in COHORT {
        let CohortFixture {
            id, fingerprint, ..
        } = card;
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        assert!(
            row.ends_with(fingerprint),
            "fingerprint drift for {id}: {row}"
        );
    }
}
