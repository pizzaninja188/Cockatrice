//! Registry conformance for the reviewed sacrifice-to-destroy-enchantment family.
//!
//! Every included card is a normal-layout white 2/2 creature with one sacrifice-self
//! activated ability that destroys a chosen enchantment. Card names and subtypes are
//! printed data; the shared scenario executes every card. Each fingerprint is SHA-256
//! of the exact pinned Oracle line.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "kami_of_ancient_law",
        "Kami of Ancient Law",
        &["Creature", "Spirit"],
        "ec95633cc2fa16d8e5138c47d45b14540a332f6d5276344683593b54c7285e8e",
    ),
    (
        "keening_apparition",
        "Keening Apparition",
        &["Creature", "Spirit"],
        "ec95633cc2fa16d8e5138c47d45b14540a332f6d5276344683593b54c7285e8e",
    ),
    (
        "ronom_unicorn",
        "Ronom Unicorn",
        &["Creature", "Unicorn"],
        "ec95633cc2fa16d8e5138c47d45b14540a332f6d5276344683593b54c7285e8e",
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
fn ronom_unicorn_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, types, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), "{1}{W}", "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!((f.power, f.toughness), (Some(2), Some(2)), "{id} body");
        assert!(f.keywords.is_empty(), "{id}");
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
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(ability.costs, [AbilityCost::SacrificeSelf], "{id}");
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(target)
            }] if target.kind == TargetKind::AnyPermanent
                && target.permanent_types.as_slice() == [PermanentTypeFilter::Enchantment]
        ));
    }
}

#[test]
fn ronom_unicorn_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
