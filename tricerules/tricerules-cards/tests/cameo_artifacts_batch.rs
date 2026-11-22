//! Registry conformance for the reviewed Cameo mana-artifact cycle.
//!
//! Every included card is a normal-layout `{3}` artifact with one `{T}` mana
//! ability offering its two printed colors and no other ability. Each
//! fingerprint is SHA-256 of the exact pinned Oracle line.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

struct Card {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    options: [Mana; 2],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "bloodstone_cameo",
        name: "Bloodstone Cameo",
        face_id: "bloodstone_cameo",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        fingerprint: "055adf3423a1fcd69b81efacca977fe8f9a3f4a9b919e7d200588ac19b5e4e89",
    },
    Card {
        id: "drake-skull_cameo",
        name: "Drake-Skull Cameo",
        face_id: "drake_skull_cameo",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        fingerprint: "5524cc1abef4e3846b2391eff73fffb94a447f145ff8c7e97f9232a3ceba403d",
    },
    Card {
        id: "seashell_cameo",
        name: "Seashell Cameo",
        face_id: "seashell_cameo",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
        fingerprint: "8a859e91be893576e486566c1497205d3cc6d46fb1ec112b12d5b38cdff54a44",
    },
    Card {
        id: "tigereye_cameo",
        name: "Tigereye Cameo",
        face_id: "tigereye_cameo",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        fingerprint: "c28b9e32c9cb2de23deca06adb0b74279000cd7f045073256c9218993618a55f",
    },
    Card {
        id: "troll-horn_cameo",
        name: "Troll-Horn Cameo",
        face_id: "troll_horn_cameo",
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
        fingerprint: "2d68b3f6c3e8eb5cafdcddd921816f83755a1cff5143fa5b63a87dcdd09ce16c",
    },
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn cameo_artifacts_batch_maps_definitions() {
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
        assert_eq!(f.face_id.as_str(), card.face_id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), "{3}", "{} mana cost", card.id);
        assert_eq!(f.types, ["Artifact"], "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!(f.power, None, "{}", card.id);
        assert_eq!(f.toughness, None, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} presentation",
            card.id
        );
        assert_eq!(ability.costs, [AbilityCost::Tap], "{} cost", card.id);
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 2
                        && [(options[0].w, options[0].u, options[0].b,
                             options[0].r, options[0].g, options[0].c),
                            (options[1].w, options[1].u, options[1].b,
                             options[1].r, options[1].g, options[1].c)] == card.options
            ),
            "{} mana options",
            card.id
        );
        assert!(ability.targeting.is_none(), "{}", card.id);
    }
}

#[test]
fn cameo_artifacts_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.face_id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
