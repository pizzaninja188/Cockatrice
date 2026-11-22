//! Registry conformance for the reviewed sacrifice-for-two-mana land family.
//!
//! Every included card is a normal-layout land that enters tapped, has one `{T}`
//! mana ability for a single printed color and one `{T}, Sacrifice this land`
//! mana ability for a printed two-mana bundle. Each fingerprint is SHA-256 of
//! the exact pinned Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, EntersTappedAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

struct Card {
    id: &'static str,
    name: &'static str,
    single: Mana,
    pair: Mana,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "ancient_spring",
        name: "Ancient Spring",
        single: (0, 1, 0, 0, 0, 0),
        pair: (1, 0, 1, 0, 0, 0),
        fingerprint: "d1eb97487ccf3916cf0d62c52cd3267fe2352796c0aee3185986a3bd169938c7",
    },
    Card {
        id: "dwarven_ruins",
        name: "Dwarven Ruins",
        single: (0, 0, 0, 1, 0, 0),
        pair: (0, 0, 0, 2, 0, 0),
        fingerprint: "2729dc6bc1ae7867b067b2efe60c8c6a49553ba52d812ccad921fc0996c678da",
    },
    Card {
        id: "ebon_stronghold",
        name: "Ebon Stronghold",
        single: (0, 0, 1, 0, 0, 0),
        pair: (0, 0, 2, 0, 0, 0),
        fingerprint: "05b99c9fdb100f50915efd44ca7415ce156de25edcaeff08767ede248f1d66a2",
    },
    Card {
        id: "geothermal_crevice",
        name: "Geothermal Crevice",
        single: (0, 0, 0, 1, 0, 0),
        pair: (0, 0, 1, 0, 1, 0),
        fingerprint: "1aa9ba79d08254f06d24d47bcd0ba150fa5b3eb3e64b1aa10f4b4fea0332f37a",
    },
    Card {
        id: "havenwood_battleground",
        name: "Havenwood Battleground",
        single: (0, 0, 0, 0, 1, 0),
        pair: (0, 0, 0, 0, 2, 0),
        fingerprint: "5a041e46bd2f4362e13b7e7e9cf79968e939481e9b704ec73f90d771f96005b1",
    },
    Card {
        id: "irrigation_ditch",
        name: "Irrigation Ditch",
        single: (1, 0, 0, 0, 0, 0),
        pair: (0, 1, 0, 0, 1, 0),
        fingerprint: "e531c40c84fbdaadb6e17b2ea079da5a1a08c634de85c70c1b1230c1d327333f",
    },
    Card {
        id: "ruins_of_trokair",
        name: "Ruins of Trokair",
        single: (1, 0, 0, 0, 0, 0),
        pair: (2, 0, 0, 0, 0, 0),
        fingerprint: "0ef4414b768b09cbdbf192f773c0fa9c7f22b3787eb9afd357901db98645d7f1",
    },
    Card {
        id: "sulfur_vent",
        name: "Sulfur Vent",
        single: (0, 0, 1, 0, 0, 0),
        pair: (0, 1, 0, 1, 0, 0),
        fingerprint: "795e7d874d6e62dfad227e3d4a0aff7279a42d740b02dbe75df0bdb436fd7315",
    },
    Card {
        id: "svyelunite_temple",
        name: "Svyelunite Temple",
        single: (0, 1, 0, 0, 0, 0),
        pair: (0, 2, 0, 0, 0, 0),
        fingerprint: "e68e1c53d3cb579c0f3ef81994c7449156d938ae592f8e4ec55627cd6508e7f0",
    },
    Card {
        id: "tinder_farm",
        name: "Tinder Farm",
        single: (0, 0, 0, 0, 1, 0),
        pair: (1, 0, 0, 1, 0, 0),
        fingerprint: "96069adf4f5703f8b43271d9e0c68aa6d1f014e7063ad9a92432dc928417c49a",
    },
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn amount(card: &Card, mana: Mana) -> String {
    format!("{}: {:?}", card.id, mana)
}

#[test]
fn sac_lands_cc_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), "", "{} mana cost", card.id);
        assert_eq!(f.types, ["Land"], "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{} no supertype", card.id);
        assert_eq!(f.power, None, "{} has no power", card.id);
        assert_eq!(f.toughness, None, "{} has no toughness", card.id);
        assert!(f.keywords.is_empty(), "{} has no printed keyword", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);

        let [entry] = f.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", card.id);
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{}", card.id);
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} entry presentation",
            card.id
        );
        assert_eq!(
            entry.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{}",
            card.id
        );

        let [single, pair] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly two activated abilities", card.id);
        };
        assert_eq!(single.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            single.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} single presentation",
            card.id
        );
        assert_eq!(single.costs, [AbilityCost::Tap], "{} single cost", card.id);
        assert!(
            matches!(
                single.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && (options[0].w, options[0].u, options[0].b,
                            options[0].r, options[0].g, options[0].c) == card.single
            ),
            "{}",
            amount(card, card.single)
        );
        assert!(single.targeting.is_none(), "{} single targetless", card.id);

        assert_eq!(pair.ability_id.as_str(), "activated_02", "{}", card.id);
        assert_eq!(
            pair.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{} pair presentation",
            card.id
        );
        assert_eq!(
            pair.costs,
            [AbilityCost::Tap, AbilityCost::SacrificeSelf],
            "{} pair cost",
            card.id
        );
        assert!(
            matches!(
                pair.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && (options[0].w, options[0].u, options[0].b,
                            options[0].r, options[0].g, options[0].c) == card.pair
            ),
            "{}",
            amount(card, card.pair)
        );
        assert!(pair.targeting.is_none(), "{} pair targetless", card.id);
    }
}

#[test]
fn sac_lands_cc_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
