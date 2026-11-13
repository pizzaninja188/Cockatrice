//! Registry conformance for the reviewed "{T}: Add {C}; {1}, {T}: Add one mana
//! of any color" family.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "mana_prism",
        "Mana Prism",
        "{3}",
        &["Artifact"],
        "ba3523af97773aaa16fe88311cd588b9b91019e0c194c1925863cdeb303ba728",
    ),
    (
        "painted_bluffs",
        "Painted Bluffs",
        "",
        &["Land", "Desert"],
        "ba3523af97773aaa16fe88311cd588b9b91019e0c194c1925863cdeb303ba728",
    ),
    (
        "shimmering_grotto",
        "Shimmering Grotto",
        "",
        &["Land"],
        "ba3523af97773aaa16fe88311cd588b9b91019e0c194c1925863cdeb303ba728",
    ),
    (
        "unknown_shores",
        "Unknown Shores",
        "",
        &["Land"],
        "ba3523af97773aaa16fe88311cd588b9b91019e0c194c1925863cdeb303ba728",
    ),
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
fn mana_any_color_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert_eq!((f.power, f.toughness), (None, None), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        let [colorless, any_color] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly two activated abilities");
        };
        assert_eq!(colorless.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            colorless.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(colorless.costs, [AbilityCost::Tap], "{id}");
        assert!(
            matches!(
                colorless.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && options[0].w == 0
                        && options[0].u == 0
                        && options[0].b == 0
                        && options[0].r == 0
                        && options[0].g == 0
                        && options[0].c == 1
            ),
            "{id}: one colorless option"
        );
        assert_eq!(any_color.ability_id.as_str(), "activated_02", "{id}");
        assert_eq!(
            any_color.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert_eq!(any_color.costs.len(), 2, "{id}");
        assert!(
            matches!(&any_color.costs[0], AbilityCost::Mana(cost) if cost.to_string() == "{1}"),
            "{id}: the any-color ability costs {{1}}"
        );
        assert_eq!(any_color.costs[1], AbilityCost::Tap, "{id}");
        assert!(
            matches!(
                any_color.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 5
                        && options.iter().all(|amount| {
                            [amount.w, amount.u, amount.b, amount.r, amount.g]
                                .iter()
                                .filter(|color| **color == 1)
                                .count()
                                == 1
                        })
            ),
            "{id}: five single-color options"
        );
    }
}

#[test]
fn mana_any_color_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
