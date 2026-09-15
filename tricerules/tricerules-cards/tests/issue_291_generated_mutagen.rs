use tricerules_cards::primitives::{EffectSubject, PlayerRecipient, SpellEffectKind, TargetKind};
use tricerules_cards::{
    AbilityCost, ActivationTiming, Amount, CardRegistry, CounterKind, TriggerCondition,
};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    types: &'static [&'static str],
    mana_cost: &'static str,
    power: Option<u32>,
    toughness: Option<u32>,
}

const COHORT: &[ExpectedCard] = &[
    ExpectedCard {
        id: "crustacean_commando",
        name: "Crustacean Commando",
        types: &["Creature", "Crab", "Mutant", "Soldier"],
        mana_cost: "{1}{U}",
        power: Some(0),
        toughness: Some(3),
    },
    ExpectedCard {
        id: "slithering_cryptid",
        name: "Slithering Cryptid",
        types: &["Creature", "Fish", "Mutant"],
        mana_cost: "{2}{G/U}",
        power: Some(2),
        toughness: Some(3),
    },
];

#[test]
fn issue_291_cards_have_exact_allowlisted_etb_mutagen_shape() {
    let registry = CardRegistry::global();
    for expected in COHORT {
        let id = expected.id;
        let name = expected.name;
        let expected_types = expected.types;
        let mana_cost = expected.mana_cost;
        let power = expected.power;
        let toughness = expected.toughness;
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing issue #291 card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            expected_types
        );
        assert_eq!((face.power, face.toughness), (power, toughness));
        assert!(face.activated_abilities.is_empty());
        assert!(face.static_abilities.is_empty());
        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{id} should have exactly one triggered ability");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01");
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id} trigger"
        );
        assert!(!trigger.may);
        assert!(trigger.targeting.is_none());
        assert!(trigger.modal.is_none());
        assert!(trigger.intervening_if.is_none());
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::CreateTokens {
                token: "mutagen".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }],
            "{id} effect"
        );
    }
}

#[test]
fn issue_291_reuses_the_predefined_mutagen_token_contract() {
    let registry = CardRegistry::global();
    let token = registry
        .get("mutagen")
        .expect("Mutagen token")
        .primary_face();
    assert!(registry.is_token("mutagen"));
    assert_eq!(token.types, ["Artifact", "Mutagen"]);
    let [ability] = token.activated_abilities.as_slice() else {
        panic!("Mutagen must have exactly one activated ability");
    };
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap, AbilityCost::SacrificeSelf]
            if cost.to_string() == "{1}"
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Chosen(target),
        }] if target.kind == TargetKind::Creature
    ));
}
