use tricerules_cards::primitives::SpellEffectKind;
use tricerules_cards::ManaCost;

#[test]
fn eerie_ultimatum_registers_its_exact_card_identity_and_instructions() {
    let card = tricerules_cards::registry::global()
        .get("eerie_ultimatum")
        .expect("complete Eerie Ultimatum definition");

    assert_eq!(card.name, "Eerie Ultimatum");
    assert_eq!(card.faces_iter().count(), 1);
    let face = card.primary_face();
    assert_eq!(
        face.mana_cost,
        ManaCost::parse("{W}{W}{B}{B}{B}{G}{G}").unwrap()
    );
    assert_eq!(face.types, ["Sorcery"]);
    assert!(face.targeting.is_none());
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::ReturnDifferentNamedPermanentsFromGraveyard]
    );
}
