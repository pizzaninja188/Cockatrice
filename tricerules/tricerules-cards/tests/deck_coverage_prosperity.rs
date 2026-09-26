use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{CardRegistry, Layout};

#[test]
fn prosperity_registers_its_variable_all_player_draw() {
    let registry = CardRegistry::global();
    let card = registry
        .get("prosperity")
        .expect("Prosperity registry definition");

    assert_eq!(card.name, "Prosperity");
    assert_eq!(registry.id_for_name("Prosperity"), Some("prosperity"));
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "prosperity");
    assert_eq!(face.mana_cost.to_string(), "{X}{U}");
    assert_eq!(face.types, ["Sorcery"]);
    assert!(face.activated_abilities.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert!(matches!(
        face.spell_effect.as_slice(),
        [SpellEffectKind::Draw {
            who: PlayerRecipient::EachPlayer,
            count: Amount::X,
        }]
    ));
}
