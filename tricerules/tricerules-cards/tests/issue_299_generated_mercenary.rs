use tricerules_cards::primitives::{
    EffectSubject, PlayerRecipient, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, ActivationTiming, Amount, CardRegistry, Color, Keyword,
    TriggerCondition,
};

#[test]
fn issue_299_registers_exactly_the_reviewed_dies_mercenary_cohort() {
    let registry = CardRegistry::global();
    let assert_card = |id: &str,
                       name: &str,
                       mana_cost: &str,
                       types: &[&str],
                       stats: (Option<u32>, Option<u32>),
                       keywords: &[Keyword],
                       line: u16| {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing issue #299 card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types.to_vec()
        );
        assert_eq!((face.power, face.toughness), stats);
        assert_eq!(face.keywords, keywords);
        assert!(face.activated_abilities.is_empty());
        assert!(face.static_abilities.is_empty());
        let [trigger] = face.triggered_abilities.as_slice() else {
            panic!("{id} should have exactly one triggered ability");
        };
        assert_eq!(trigger.ability_id.as_str(), "triggered_01");
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
        assert!(!trigger.may);
        assert!(trigger.modal.is_none());
        assert!(trigger.targeting.is_none());
        assert!(trigger.intervening_if.is_none());
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::CreateTokens {
                token: "mercenary_r_1_1".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }]
        );
    };
    assert_card(
        "nezumi_linkbreaker",
        "Nezumi Linkbreaker",
        "{B}",
        &["Creature", "Rat", "Warlock"],
        (Some(1), Some(1)),
        &[],
        1,
    );
    assert_card(
        "wanted_griffin",
        "Wanted Griffin",
        "{3}{W}",
        &["Creature", "Griffin"],
        (Some(3), Some(2)),
        &[Keyword::Flying],
        2,
    );
}

#[test]
fn issue_299_reuses_the_predefined_mercenary_token_contract() {
    let token = CardRegistry::global()
        .get("mercenary_r_1_1")
        .expect("Mercenary token")
        .primary_face();
    assert!(CardRegistry::global().is_token("mercenary_r_1_1"));
    assert_eq!(token.types, ["Creature", "Mercenary"]);
    assert_eq!(token.colors(), [Color::Red]);
    assert_eq!((token.power, token.toughness), (Some(1), Some(1)));
    let [ability] = token.activated_abilities.as_slice() else {
        panic!("Mercenary must have exactly one activated ability");
    };
    assert_eq!(ability.timing, ActivationTiming::SorcerySpeed);
    assert_eq!(ability.costs, [AbilityCost::Tap]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 0,
            scale: None,
            subject: EffectSubject::Chosen(Box::new(tricerules_cards::primitives::TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..Default::default()
            },)),
        }]
    );
}
