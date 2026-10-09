mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{CardTypeFilter, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{Color, Keyword};

const BIRD_TOKEN: &str = "bird_u_2_2_flying";

#[test]
fn swan_song_registers_its_exact_spell_filter_and_bird_token() {
    let face = FaceExpectation {
        id: "swan_song",
        name: "Swan Song",
        face_id: "swan_song",
        mana_cost: "{U}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
    }
    .check();

    let [SpellEffectKind::CounterTargetSpell {
        spell_filter,
        unless_controller_pays,
        unless_controller_pays_by_cast_cost,
    }, SpellEffectKind::CreateTokens {
        token, count, who, ..
    }] = face.spell_effect.as_slice()
    else {
        panic!("Swan Song counters one spell and creates one Bird for that spell's controller");
    };

    let branches = spell_filter
        .any_of
        .as_ref()
        .expect("the three permitted spell types form an OR filter");
    assert_eq!(
        branches
            .iter()
            .map(|branch| branch.card_type)
            .collect::<Vec<_>>(),
        [
            Some(CardTypeFilter::Enchantment),
            Some(CardTypeFilter::Instant),
            Some(CardTypeFilter::Sorcery),
        ]
    );
    assert!(branches.iter().all(|branch| {
        branch.any_of.is_none()
            && branch.is_color.is_none()
            && branch.min_mana_value.is_none()
            && branch.max_mana_value.is_none()
    }));
    assert!(spell_filter.card_type.is_none());
    assert_eq!(*unless_controller_pays, None);
    assert_eq!(*unless_controller_pays_by_cast_cost, None);
    assert_eq!(token, BIRD_TOKEN);
    assert_eq!(*count, tricerules_cards::Amount::Fixed(1));
    assert_eq!(*who, PlayerRecipient::PreviousTargetedSpellController);

    let registry = tricerules_cards::registry::global();
    assert!(registry.is_token(BIRD_TOKEN));
    let bird = registry.get(BIRD_TOKEN).expect("blue Bird token");
    assert_eq!(bird.name, "Bird");
    let bird_face = bird.primary_face();
    assert_eq!(bird_face.types, ["Creature", "Bird"]);
    assert_eq!(bird_face.colors_override.as_ref().unwrap(), &[Color::Blue]);
    assert_eq!((bird_face.power, bird_face.toughness), (Some(2), Some(2)));
    assert_eq!(bird_face.keywords, [Keyword::Flying]);
}
