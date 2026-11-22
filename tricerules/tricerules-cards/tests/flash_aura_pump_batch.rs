//! Registry conformance for the queue-v18 flash Aura pump cohort.
//!
//! Every included card is a normal-layout Aura with Flash that enchants a creature
//! and gives the enchanted creature a printed P/T bonus.
use tricerules_cards::primitives::{SpellEffectKind, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    pump: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "feral_invocation",
        name: "Feral Invocation",
        mana_cost: "{2}{G}",
        pump: (2, 2),
        oracle_text_sha256: "f2596958caa37f6d898338eb7e1282b4507a3ed748c5acfd90bb6b33fdc0828c",
    },
    ExpectedCard {
        id: "gift_of_granite",
        name: "Gift of Granite",
        mana_cost: "{W}",
        pump: (0, 2),
        oracle_text_sha256: "e371aef5a175f2674ee292d3e68b7d5beec11e70dec03fe3e57e6a07deb29e77",
    },
    ExpectedCard {
        id: "indomitable_will",
        name: "Indomitable Will",
        mana_cost: "{1}{W}",
        pump: (1, 2),
        oracle_text_sha256: "285c156f75e16a6ce8a27cb6ad8faa7a565aa449e052daf61549f86484ad5568",
    },
    ExpectedCard {
        id: "magetas_boon",
        name: "Mageta's Boon",
        mana_cost: "{1}{W}",
        pump: (1, 2),
        oracle_text_sha256: "950e87ef0d2d0602621401bd8ee71f9105e25e09abfcbd9e9dfc574ef5a144d2",
    },
    ExpectedCard {
        id: "stoneskin",
        name: "Stoneskin",
        mana_cost: "{2}{W}",
        pump: (0, 10),
        oracle_text_sha256: "b05cf6e7ba8cec58710bed706667599ce2bcc288b9973096b7d7d59e5e728372",
    },
];

#[test]
fn flash_aura_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for expected in CARDS {
        let id = expected.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id}");
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
        assert_eq!(face.types, ["Enchantment", "Aura"], "{id}");
        assert_eq!(face.keywords, [Keyword::Flash], "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        let [SpellEffectKind::AuraAttach { target }] = face.spell_effect.as_slice() else {
            panic!("{id}: exactly one AuraAttach");
        };
        assert_eq!(
            target.kind,
            tricerules_cards::primitives::TargetKind::Creature,
            "{id}"
        );
        let [static_ability] = face.static_abilities.as_slice() else {
            panic!("{id}: exactly one static ability");
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{id}"
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::AttachedModifier {
                    delta_power,
                    delta_toughness,
                    keywords,
                    ..
                } if *delta_power == expected.pump.0
                    && *delta_toughness == expected.pump.1
                    && keywords.is_empty()
            ),
            "{id}: attached printed pump"
        );
    }
}

#[test]
fn flash_aura_pump_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for expected in CARDS {
        let id = expected.id;
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], expected.name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
