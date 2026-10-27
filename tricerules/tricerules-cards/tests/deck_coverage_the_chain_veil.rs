//! Exact Chain Veil identity and its controller-end-step trigger.
use tricerules_cards::primitives::{
    AbilityCost, CastTriggerPlayer, GameCondition, LifeAmount, PlayerRecipient, SpellEffectKind,
    TriggerCondition,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn chain_veil_exact_identity_and_end_step_ability() {
    let registry = tricerules_cards::registry::global();
    let id = registry
        .id_for_name("The Chain Veil")
        .expect("The Chain Veil must be registered");
    assert_eq!(id, "the_chain_veil");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "The Chain Veil");
    assert_eq!(face.face_id.as_str(), "the_chain_veil");
    assert_eq!(face.mana_cost.to_string(), "{4}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Artifact"]);
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("one end-step trigger")
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: CastTriggerPlayer::Controller
        }
    );
    assert_eq!(
        trigger.intervening_if,
        Some(GameCondition::ControllerHasNotActivatedPlaneswalkerLoyaltyThisTurn)
    );
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(2),
            who: PlayerRecipient::Controller,
        }]
    ));
}

#[test]
fn chain_veil_exact_activated_permission_cost_and_presentation() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("the_chain_veil")
        .expect("The Chain Veil must be registered")
        .primary_face();
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("one activated permission")
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(
        matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap]
        if cost.to_string() == "{4}")
    );
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GrantChainVeilLoyaltyPermission]
    );
    assert!(ability.program.is_none());
}

#[test]
fn chain_veil_permission_is_restricted_to_a_flat_battlefield_activated_effect() {
    const VEIL: &str = include_str!("../data/the_chain_veil.ron");
    tricerules_cards::CardRegistry::from_chunks_and_tokens(&[VEIL], &[])
        .expect("the actual activated ability is admitted");
    let wrapped = VEIL.replace(
        "effect: [GrantChainVeilLoyaltyPermission]",
        "effect: [Conditional(condition: ControlsCreatureTiedForGreatestPower, effect: GrantChainVeilLoyaltyPermission)]",
    );
    let siblings = VEIL.replace(
        "effect: [GrantChainVeilLoyaltyPermission]",
        "effect: [GrantChainVeilLoyaltyPermission, Draw(count: 1)]",
    );
    let graveyard = VEIL.replace(
        "costs: [Mana(\"{4}\"), Tap]",
        "source_zone: Graveyard, costs: [Mana(\"{4}\")]",
    );
    let spell = r#"(id: "veil_spell_fixture", name: "Veil Spell Fixture", face_id: "veil_spell_fixture",
        types: ["Sorcery"], spell_effect: [GrantChainVeilLoyaltyPermission])"#;
    let trigger = r#"(id: "veil_trigger_fixture", name: "Veil Trigger Fixture", face_id: "veil_trigger_fixture",
        types: ["Artifact"], triggered_abilities: [(ability_id: "triggered_01", presentation: Fallback,
        trigger: WhenSelfEntersBattlefield, effect: [GrantChainVeilLoyaltyPermission])])"#;
    let program = r#"(id: "veil_program_fixture", name: "Veil Program Fixture", face_id: "veil_program_fixture",
        types: ["Sorcery"], spell_program: Some((body: (steps: [
        (id: "grant", kind: Perform(GrantChainVeilLoyaltyPermission)),
        ]))))"#;
    let nested = r#"(id: "veil_nested_fixture", name: "Veil Nested Fixture", face_id: "veil_nested_fixture",
        types: ["Sorcery"], spell_program: Some((body: (steps: [
        (id: "delay", kind: Perform(CreateDelayedTrigger(subject: None, ability: (
        ability_id: "delayed_01", presentation: Fallback, trigger: AtBeginningOfNextTurnUpkeep,
        effect: [GrantChainVeilLoyaltyPermission],
        )))),
        ]))))"#;
    for (label, definition) in [
        ("wrapper", wrapped.as_str()),
        ("sibling", siblings.as_str()),
        ("graveyard activation", graveyard.as_str()),
        ("spell", spell),
        ("trigger", trigger),
        ("Program", program),
        ("nested Program child", nested),
    ] {
        match tricerules_cards::CardRegistry::from_chunks_and_tokens(&[definition], &[]) {
            Ok(_) => panic!("{label} unexpectedly admitted the specialized grant"),
            Err(error) => assert!(
                !error.to_string().contains("ron parse:"),
                "{label} must reach validation: {error}",
            ),
        }
    }
}
