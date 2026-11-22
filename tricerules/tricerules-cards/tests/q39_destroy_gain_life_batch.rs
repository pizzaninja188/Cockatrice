//! Registry conformance for the queue-v39 destroy-and-gain-life cohort.
use tricerules_cards::primitives::{
    Amount, Color, EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    life: i32,
    not_color: Option<Color>,
    permanent_type: Option<PermanentTypeFilter>,
    flying: bool,
}

const CARDS: &[Card] = &[
    Card {
        id: "lichs_caress",
        name: "Lich's Caress",
        mana: "{3}{B}{B}",
        life: 3,
        not_color: None,
        permanent_type: None,
        flying: false,
    },
    Card {
        id: "dark_offering",
        name: "Dark Offering",
        mana: "{4}{B}{B}",
        life: 3,
        not_color: Some(Color::Black),
        permanent_type: None,
        flying: false,
    },
    Card {
        id: "poison_arrow",
        name: "Poison Arrow",
        mana: "{4}{B}{B}",
        life: 3,
        not_color: Some(Color::Black),
        permanent_type: None,
        flying: false,
    },
    Card {
        id: "ray_of_dissolution",
        name: "Ray of Dissolution",
        mana: "{2}{W}",
        life: 3,
        not_color: None,
        permanent_type: Some(PermanentTypeFilter::Enchantment),
        flying: false,
    },
    Card {
        id: "cursebreak",
        name: "Cursebreak",
        mana: "{1}{W}",
        life: 2,
        not_color: None,
        permanent_type: Some(PermanentTypeFilter::Enchantment),
        flying: false,
    },
    Card {
        id: "aerial_predation",
        name: "Aerial Predation",
        mana: "{2}{G}",
        life: 2,
        not_color: None,
        permanent_type: None,
        flying: true,
    },
];

#[test]
fn q39_destroy_gain_life_maps_definitions() {
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
        assert!(matches!(f.types.as_slice(), [t] if t == "Sorcery" || t == "Instant"));
        assert!(f.keywords.is_empty());
        assert!(f.targeting.is_some());
        let [destroy, gain] = f.spell_effect.as_slice() else {
            panic!("{}: destroy then gain life", card.id);
        };
        let SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        } = destroy
        else {
            panic!("{}: chosen destroy target", card.id);
        };
        assert_eq!(filter.not_color, card.not_color);
        if let Some(permanent_type) = card.permanent_type {
            assert_eq!(filter.permanent_types, [permanent_type]);
            assert_eq!(filter.kind, TargetKind::AnyPermanent);
        } else {
            assert!(filter.permanent_types.is_empty());
            assert_eq!(filter.kind, TargetKind::Creature);
        }
        assert_eq!(
            filter
                .required_keywords
                .contains(&tricerules_cards::Keyword::Flying),
            card.flying
        );
        assert!(matches!(
            gain,
            SpellEffectKind::GainLife { amount: Amount::Fixed(n), .. } if *n == card.life as u32
        ));
        let target = f.targeting.as_ref().expect("targeting");
        assert_eq!(target.groups.len(), 1);
        assert_eq!(target.groups[0].effect_indices, [0]);
    }
}
