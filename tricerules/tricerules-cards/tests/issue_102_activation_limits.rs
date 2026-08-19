use tricerules_cards::primitives::{ActivationLimit, SpellEffectKind};
use tricerules_cards::{AbilityCost, CardRegistry, Keyword, ManaAmount, TriggerCondition};

struct ExpectedDevotee {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    mana_options: &'static [ManaAmount],
}

const DEVOTEES: &[ExpectedDevotee] = &[
    ExpectedDevotee {
        id: "temur_devotee",
        name: "Temur Devotee",
        mana_cost: "{1}{U}",
        types: &["Creature", "Human", "Druid"],
        power: 3,
        toughness: 3,
        keywords: &[Keyword::Defender],
        mana_options: &[
            ManaAmount {
                g: 1,
                w: 0,
                u: 0,
                b: 0,
                r: 0,
                c: 0,
            },
            ManaAmount {
                u: 1,
                w: 0,
                b: 0,
                r: 0,
                g: 0,
                c: 0,
            },
            ManaAmount {
                r: 1,
                w: 0,
                u: 0,
                b: 0,
                g: 0,
                c: 0,
            },
        ],
    },
    ExpectedDevotee {
        id: "sultai_devotee",
        name: "Sultai Devotee",
        mana_cost: "{1}{G}",
        types: &["Creature", "Zombie", "Snake", "Druid"],
        power: 2,
        toughness: 1,
        keywords: &[Keyword::Deathtouch],
        mana_options: &[
            ManaAmount {
                b: 1,
                w: 0,
                u: 0,
                r: 0,
                g: 0,
                c: 0,
            },
            ManaAmount {
                g: 1,
                w: 0,
                u: 0,
                b: 0,
                r: 0,
                c: 0,
            },
            ManaAmount {
                u: 1,
                w: 0,
                b: 0,
                r: 0,
                g: 0,
                c: 0,
            },
        ],
    },
    ExpectedDevotee {
        id: "mardu_devotee",
        name: "Mardu Devotee",
        mana_cost: "{W}",
        types: &["Creature", "Human", "Scout"],
        power: 1,
        toughness: 2,
        keywords: &[],
        mana_options: &[
            ManaAmount {
                r: 1,
                w: 0,
                u: 0,
                b: 0,
                g: 0,
                c: 0,
            },
            ManaAmount {
                w: 1,
                u: 0,
                b: 0,
                r: 0,
                g: 0,
                c: 0,
            },
            ManaAmount {
                b: 1,
                w: 0,
                u: 0,
                r: 0,
                g: 0,
                c: 0,
            },
        ],
    },
];

#[test]
fn devotee_card_data_matches_oracle() {
    for expected in DEVOTEES {
        let definition = CardRegistry::global()
            .get(expected.id)
            .unwrap_or_else(|| panic!("{} must be registered", expected.id));
        assert_eq!(definition.name, expected.name);
        assert!(definition.partial.is_none());
        let face = definition.primary_face();
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost);
        assert_eq!(face.types, expected.types);
        assert_eq!(face.power, Some(expected.power));
        assert_eq!(face.toughness, Some(expected.toughness));
        assert_eq!(face.keywords, expected.keywords);
        assert_eq!(face.activated_abilities.len(), 1);

        let ability = &face.activated_abilities[0];
        assert!(matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost)] if cost.to_string() == "{1}"
        ));
        assert_eq!(
            ability.activation_limit,
            Some(ActivationLimit::PerTurn { max_activations: 1 })
        );
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::ProduceMana { options, restriction: None, conditional: None }]
                if options == expected.mana_options
        ));
        assert!(ability.text.ends_with("Activate only once each turn."));
    }
}

#[test]
fn mardu_devotee_reuses_the_generic_scry_trigger() {
    let face = CardRegistry::global()
        .get("mardu_devotee")
        .expect("Mardu Devotee must be registered")
        .primary_face();
    assert_eq!(face.triggered_abilities.len(), 1);
    let trigger = &face.triggered_abilities[0];
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(trigger.effect, [SpellEffectKind::Scry { count: 2 }]);
    assert_eq!(trigger.text, "When this creature enters, scry 2.");
}
