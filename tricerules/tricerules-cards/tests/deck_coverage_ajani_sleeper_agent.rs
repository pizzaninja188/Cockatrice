//! Exact Oracle 37714eb9-a39d-4534-a2d3-27908c418f8a, including every loyalty clause.
use tricerules_cards::primitives::{AbilityCost, SpellEffectKind, StaticAbilityDef};
use tricerules_cards::AbilityPresentation;

#[test]
fn ajani_exact_identity_compleated_and_all_three_loyalty_abilities() {
    const ID: &str = "ajani,_sleeper_agent";
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(ID)
        .expect("complete actual Ajani must be registered");
    assert_eq!(registry.id_for_name("Ajani, Sleeper Agent"), Some(ID));
    assert!(!card.commander_setup_only);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "ajani_sleeper_agent");
    assert_eq!(face.mana_cost.to_string(), "{1}{G}{G/W/P}{W}");
    assert_eq!(face.mana_cost.mana_value(), 4);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Ajani"]);
    assert_eq!(face.loyalty, Some(4));
    assert!(face
        .static_abilities
        .iter()
        .any(|ability| ability.definition == StaticAbilityDef::Compleated
            && ability.presentation == AbilityPresentation::OracleLines(vec![1])));
    assert_eq!(face.activated_abilities.len(), 3);
    for (index, cost) in [1, -3, -6].into_iter().enumerate() {
        let ability = &face.activated_abilities[index];
        assert_eq!(
            ability.ability_id.as_str(),
            format!("activated_{:02}", index + 1)
        );
        assert_eq!(ability.costs, [AbilityCost::Loyalty(cost)]);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![index as u16 + 2])
        );
        assert!(ability.program.is_none() && ability.modal.is_none());
    }
    let expected: Vec<SpellEffectKind> = ron::from_str(
        r#"[
        RevealTopCardToHandIfMatches(
            filter:(any_of:Some([(card_type:Some(Creature)),(card_type:Some(Planeswalker))])),
            unmatched:MayPutBottom),
    ]"#,
    )
    .unwrap();
    assert_eq!(face.activated_abilities[0].effect, expected);
    assert!(face.activated_abilities[0].targeting.is_none());
    let expected: Vec<SpellEffectKind> = ron::from_str(
        r#"[
        DistributeCounters(counter:PlusOnePlusOne,count:3,subject:Chosen((kind:Creature))),
        GrantKeywords(subject:Chosen((kind:Creature)),keywords:[Vigilance]),
    ]"#,
    )
    .unwrap();
    assert_eq!(face.activated_abilities[1].effect, expected);
    let groups = &face.activated_abilities[1]
        .targeting
        .as_ref()
        .unwrap()
        .groups;
    assert_eq!(groups.len(), 1);
    assert_eq!((groups[0].min, groups[0].max), (0, 3));
    assert_eq!(groups[0].effect_indices, [0, 1]);
    let expected: Vec<SpellEffectKind> = ron::from_str(r#"[
        CreateTriggeredEmblem(emblem_id:"ajani_sleeper_agent_emblem",display_name:"Ajani, Sleeper Agent Emblem",
            ability:(ability_id:"emblem_01",presentation:OracleLines([4]),
                trigger:WheneverPlayerCastsSpell(caster:Controller,
                    filter:(any_of:Some([(card_type:Some(Creature)),(card_type:Some(Planeswalker))])),
                    ordinal:None,ordinal_scope:AllSpells),
                effect:[PutPoisonCountersOnTargetPlayer(count:2,target:(kind:OpponentPlayer))])),
    ]"#).unwrap();
    assert_eq!(face.activated_abilities[2].effect, expected);
    assert!(face.activated_abilities[2].targeting.is_none());
}
