use tricerules_cards::primitives::{SpellEffectKind, TargetFilter, TargetKind};
use tricerules_cards::{AbilityCost, AbilityPresentation, ActivationTiming, Layout, ManaCost};

const AETHERIC_AMPLIFIER: &str = "aetheric_amplifier";

fn mana_options(
    ability: &tricerules_cards::ActivatedAbilityDef,
) -> Vec<(u32, u32, u32, u32, u32, u32)> {
    ability
        .mana_options()
        .expect("the printed ability is a typed mana ability")
        .iter()
        .map(|mana| (mana.w, mana.u, mana.b, mana.r, mana.g, mana.c))
        .collect()
}

#[test]
fn aetheric_amplifier_registers_exact_activated_mana_and_modal_counter_modes() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(AETHERIC_AMPLIFIER)
        .expect("Aetheric Amplifier must have a complete registered definition");
    assert_eq!(
        registry.id_for_name("Aetheric Amplifier"),
        Some(AETHERIC_AMPLIFIER)
    );
    assert_eq!(card.id, AETHERIC_AMPLIFIER);
    assert_eq!(card.name, "Aetheric Amplifier");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    assert!(!card.commander_setup_only);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "aetheric_amplifier");
    assert_eq!(face.mana_cost, ManaCost::parse("{3}").unwrap());
    assert_eq!(face.types, ["Artifact"]);
    let [mana, modal] = face.activated_abilities.as_slice() else {
        panic!("Aetheric Amplifier has exactly its mana and modal activated abilities");
    };

    assert_eq!(mana.ability_id.as_str(), "activated_01");
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(mana.costs, [AbilityCost::Tap]);
    assert_eq!(
        mana_options(mana),
        [
            (1, 0, 0, 0, 0, 0),
            (0, 1, 0, 0, 0, 0),
            (0, 0, 1, 0, 0, 0),
            (0, 0, 0, 1, 0, 0),
            (0, 0, 0, 0, 1, 0),
        ]
    );
    assert_eq!(mana.timing, ActivationTiming::Normal);
    assert!(mana.modal.is_none());
    assert!(mana.targeting.is_none());

    assert_eq!(modal.ability_id.as_str(), "activated_02");
    assert_eq!(
        modal.presentation,
        AbilityPresentation::OracleLines(vec![2, 3, 4])
    );
    assert_eq!(
        modal.costs,
        [
            AbilityCost::Mana(ManaCost::parse("{4}").unwrap()),
            AbilityCost::Tap,
        ]
    );
    assert_eq!(modal.timing, ActivationTiming::SorcerySpeed);
    assert!(modal.effect.is_empty());
    let modal = modal
        .modal
        .as_ref()
        .expect("the second ability chooses a mode on activation");
    assert_eq!((modal.min_modes, modal.max_modes), (1, 1));
    let [double_permanent, double_player] = modal.modes.as_slice() else {
        panic!("the activated ability has the two printed counter modes");
    };

    assert_eq!(double_permanent.mode_id.as_str(), "double_any_permanent");
    assert_eq!(
        double_permanent.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        double_permanent.effects,
        [SpellEffectKind::DoubleCounters {
            target: TargetFilter {
                kind: TargetKind::AnyPermanent,
                ..TargetFilter::default()
            }
        }]
    );
    let [target_group] = double_permanent
        .targeting
        .as_ref()
        .expect("the permanent-counter mode targets one permanent")
        .groups
        .as_slice()
    else {
        panic!("the permanent-counter mode has one target group");
    };
    assert_eq!((target_group.min, target_group.max), (1, 1));
    assert_eq!(target_group.effect_indices, [0]);

    assert_eq!(double_player.mode_id.as_str(), "double_your_counters");
    assert_eq!(
        double_player.presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
    assert_eq!(double_player.effects, [SpellEffectKind::DoubleYourCounters]);
    assert!(double_player.targeting.is_none());
}
