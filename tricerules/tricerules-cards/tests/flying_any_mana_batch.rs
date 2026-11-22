//! Registry conformance for the queue-v23 flying `{T}: Add one mana of any color` cohort.
//!
//! Every included card is a normal-layout creature with Flying, printed data and
//! exactly one tap mana ability offering one mana of each color. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    (u32, u32),
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "birds_of_paradise",
        "Birds of Paradise",
        "{G}",
        &["Creature", "Bird"],
        (0, 1),
        "c59d430350b655c6f9bbc5ac2978fc67564982083d70cd9475b66848ce589746",
    ),
    (
        "lotus_guardian",
        "Lotus Guardian",
        "{7}",
        &["Artifact", "Creature", "Dragon"],
        (4, 4),
        "c59d430350b655c6f9bbc5ac2978fc67564982083d70cd9475b66848ce589746",
    ),
    (
        "ornithopter_of_paradise",
        "Ornithopter of Paradise",
        "{2}",
        &["Artifact", "Creature", "Thopter"],
        (0, 2),
        "c59d430350b655c6f9bbc5ac2978fc67564982083d70cd9475b66848ce589746",
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

fn check_card(id: &str, name: &str, mana: &str, types: &[&str], body: (u32, u32)) {
    let registry = tricerules_cards::registry::global();
    let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(definition.name, name, "{id}");
    assert_eq!(registry.id_for_name(name), Some(id), "{id}");
    let f = face(registry, id);
    assert_eq!(f.face_id.as_str(), id, "{id} face id");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    assert!(f.supertypes.is_empty(), "{id} supertypes");
    assert_eq!(
        (f.power, f.toughness),
        (Some(body.0), Some(body.1)),
        "{id} body"
    );
    assert_eq!(f.keywords, [Keyword::Flying], "{id} flying");
    assert!(f.triggered_abilities.is_empty(), "{id}");
    assert!(f.static_abilities.is_empty(), "{id}");
    assert!(f.spell_effect.is_empty(), "{id}");
    assert!(f.targeting.is_none(), "{id}");

    let [ability] = f.activated_abilities.as_slice() else {
        panic!("{id}: exactly one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "{id} presentation"
    );
    assert_eq!(ability.costs, [AbilityCost::Tap], "{id} tap cost");
    assert!(ability.is_mana_ability(), "{id}: mana ability");
    assert!(
        matches!(
            ability.effect.as_slice(),
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

#[test]
fn flying_any_mana_batch_canary_cards_match_their_typed_shape() {
    for &(id, name, mana, types, body, _) in CARDS
        .iter()
        .filter(|(id, ..)| matches!(*id, "birds_of_paradise" | "lotus_guardian"))
    {
        check_card(id, name, mana, types, body);
    }
}

#[test]
fn flying_any_mana_batch_maps_definitions() {
    for &(id, name, mana, types, body, _) in CARDS {
        check_card(id, name, mana, types, body);
    }
}

#[test]
fn flying_any_mana_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, ..) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], CARDS.iter().find(|c| c.0 == id).unwrap().5);
    }
}
