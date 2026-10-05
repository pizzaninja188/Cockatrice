use tricerules_cards::primitives::{CardTypeFilter, SpellEffectKind};
use tricerules_cards::CardRegistry;

#[test]
fn graveyard_return_registers_exact_single_faces_costs_types_and_filters() {
    for (id, name, cost, legendary) in [
        (
            "primevals_glorious_rebirth",
            "Primevals' Glorious Rebirth",
            "{5}{W}{B}",
            true,
        ),
        (
            "triumphant_reckoning",
            "Triumphant Reckoning",
            "{6}{W}{W}{W}",
            false,
        ),
    ] {
        let card = CardRegistry::global().get(id).unwrap();
        assert_eq!(card.name, name);
        assert_eq!(card.faces_iter().count(), 1);
        let face = card.primary_face();
        assert_eq!(face.mana_cost.to_string(), cost);
        assert_eq!(face.types, ["Sorcery"]);
        assert_eq!(face.is_legendary, legendary);
        assert_eq!(face.spell_effect.len(), 1);
        assert!(face.targeting.is_none());
        assert!(
            face.activated_abilities.is_empty()
                && face.triggered_abilities.is_empty()
                && face.static_abilities.is_empty()
        );
        let SpellEffectKind::ReturnAllGraveyardPermanents { filter } = &face.spell_effect[0] else {
            panic!("complete return instruction required");
        };
        if legendary {
            assert_eq!(filter.required_supertypes, ["Legendary"]);
            assert!(filter.any_of.is_none());
        } else {
            let branches = filter.any_of.as_ref().unwrap();
            assert_eq!(
                branches
                    .iter()
                    .map(|filter| filter.card_type.unwrap())
                    .collect::<Vec<_>>(),
                [
                    CardTypeFilter::Artifact,
                    CardTypeFilter::Enchantment,
                    CardTypeFilter::Planeswalker
                ]
            );
        }
    }
}

#[test]
fn all_graveyard_return_rejects_invalid_filter_and_ability_context() {
    let ability = r#"(id: "test", name: "Test", face_id: "test", types: ["Artifact"],
        activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
        costs: [], effect: [ReturnAllGraveyardPermanents(filter: (card_type: Some(Artifact)))])])"#;
    let err = CardRegistry::from_chunks_and_tokens(&[ability], &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("requires a spell instruction"), "{err}");
    let bad_filter = r#"(id: "test", name: "Test", face_id: "test", types: ["Sorcery"],
        spell_effect: [ReturnAllGraveyardPermanents(filter: (any_of: Some([(card_type: Some(Artifact))])))])"#;
    assert!(CardRegistry::from_chunks_and_tokens(&[bad_filter], &[]).is_err());
}
