//! Registry conformance for the queue-v32 flying self-untap cohort.
use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    body: (u32, u32),
    activation: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "blizzard_elemental",
        name: "Blizzard Elemental",
        mana_cost: "{5}{U}{U}",
        body: (5, 5),
        activation: "{3}{U}",
        fingerprint: "73874ce9ebf8631581c58eb2c0c4e050c88cad6645ce00bddf468bdcd678418b",
    },
    Card {
        id: "glimmerbell",
        name: "Glimmerbell",
        mana_cost: "{1}{U}",
        body: (1, 3),
        activation: "{1}{U}",
        fingerprint: "9e7973460bda7caed148dbbf953106f85a6d884063ce4b894d14696e733b0108",
    },
    Card {
        id: "vigilant_drake",
        name: "Vigilant Drake",
        mana_cost: "{4}{U}",
        body: (3, 3),
        activation: "{2}{U}",
        fingerprint: "818d5aab56b3b375cb2fdc85b7826f36e818680c4f2ce252f711e644ad522a09",
    },
];

#[test]
fn q32_flying_untap_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        let expected_types: Vec<&str> = match card.id {
            "blizzard_elemental" => vec!["Creature", "Elemental"],
            "glimmerbell" => vec!["Creature", "Elemental", "Jellyfish"],
            "vigilant_drake" => vec!["Creature", "Drake"],
            _ => unreachable!("printed card"),
        };
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            expected_types
        );
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.body.0), Some(card.body.1))
        );
        assert_eq!(f.keywords, [Keyword::Flying]);
        assert!(f.triggered_abilities.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.targeting.is_none());
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: one activated ability", card.id)
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(
            matches!(&ability.costs[..], [AbilityCost::Mana(cost)] if cost.to_string() == card.activation),
            "{}: printed mana cost",
            card.id
        );
        assert!(
            matches!(ability.effect.as_slice(), [SpellEffectKind::Untap { subject }] if subject == &tricerules_cards::primitives::EffectSubject::Source),
            "{}: untap source",
            card.id
        );
    }
}

#[test]
fn q32_flying_untap_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
