use tricerules_cards::primitives::{
    EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityCost, AbilityPresentation, CardRegistry, Keyword};

#[test]
fn issue_251_generated_cards_have_exact_source_data_and_typed_ability() {
    let registry = CardRegistry::global();
    let cases = [
        ("cathar_commando", "{1}{W}", 3, 1, Some(Keyword::Flash), 2),
        (
            "shattered_acolyte",
            "{1}{W}",
            2,
            2,
            Some(Keyword::Lifelink),
            2,
        ),
        ("thrashing_brontodon", "{1}{G}{G}", 3, 4, None, 1),
        (
            "undergrowth_leopard",
            "{1}{G}",
            2,
            2,
            Some(Keyword::Vigilance),
            2,
        ),
        (
            "voracious_varmint",
            "{1}{G}",
            2,
            2,
            Some(Keyword::Vigilance),
            2,
        ),
    ];

    for (id, mana_cost, power, toughness, keyword, oracle_line) in cases {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!((face.power, face.toughness), (Some(power), Some(toughness)));
        assert_eq!(face.keywords, keyword.into_iter().collect::<Vec<_>>());

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id} must have one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert!(matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(cost), AbilityCost::SacrificeSelf] if cost.to_string() == "{1}"
        ));
        let [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(target),
        }] = ability.effect.as_slice()
        else {
            panic!("{id} must destroy one chosen permanent");
        };
        assert_eq!(target.kind, TargetKind::AnyPermanent);
        assert_eq!(
            target.permanent_types,
            [
                PermanentTypeFilter::Artifact,
                PermanentTypeFilter::Enchantment
            ]
        );
    }
}
