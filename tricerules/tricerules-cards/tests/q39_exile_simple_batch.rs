//! Registry conformance for the queue-v39 simple exile cohort.
use tricerules_cards::primitives::{
    EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    kind: TargetKind,
    permanent_type: Option<PermanentTypeFilter>,
    excluded_land: bool,
}

const CARDS: &[Card] = &[
    Card {
        id: "scour_from_existence",
        name: "Scour from Existence",
        mana: "{7}",
        kind: TargetKind::AnyPermanent,
        permanent_type: None,
        excluded_land: false,
    },
    Card {
        id: "caustic_rain",
        name: "Caustic Rain",
        mana: "{2}{B}{B}",
        kind: TargetKind::AnyPermanent,
        permanent_type: Some(PermanentTypeFilter::Land),
        excluded_land: false,
    },
    Card {
        id: "erase",
        name: "Erase",
        mana: "{W}",
        kind: TargetKind::AnyPermanent,
        permanent_type: Some(PermanentTypeFilter::Enchantment),
        excluded_land: false,
    },
    Card {
        id: "utter_end",
        name: "Utter End",
        mana: "{2}{W}{B}",
        kind: TargetKind::AnyPermanent,
        permanent_type: None,
        excluded_land: true,
    },
    Card {
        id: "shattering_blow",
        name: "Shattering Blow",
        mana: "{1}{R/W}",
        kind: TargetKind::AnyPermanent,
        permanent_type: Some(PermanentTypeFilter::Artifact),
        excluded_land: false,
    },
];

#[test]
fn q39_exile_simple_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert!(f.keywords.is_empty());
        assert!(f.targeting.is_some());
        let [exile] = f.spell_effect.as_slice() else {
            panic!("{}: exactly one exile effect", card.id);
        };
        let SpellEffectKind::Exile {
            subject: EffectSubject::Chosen(filter),
            ..
        } = exile
        else {
            panic!("{}: chosen exile target", card.id);
        };
        assert_eq!(filter.kind, card.kind);
        assert_eq!(
            filter.permanent_types,
            card.permanent_type.map_or(Vec::new(), |p| vec![p])
        );
        assert_eq!(
            filter
                .excluded_permanent_types
                .contains(&PermanentTypeFilter::Land),
            card.excluded_land
        );
        let target = f.targeting.as_ref().expect("targeting");
        assert_eq!(target.groups.len(), 1);
        assert_eq!(target.groups[0].effect_indices, [0]);
    }
}
