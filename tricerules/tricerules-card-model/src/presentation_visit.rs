//! Typed walk over every authored presentation node of one card face.
//!
//! Each node is reported with the exact [`PresentationPath`] the engine publishes for it, so the
//! authoring audit, its exception rows and review packets name the identity a client receives.

use crate::card_def::{
    CardFace, IdentifiedCharacteristicDefiningAbility, IdentifiedStaticAbility, ModalDef, ModeDef,
};
use crate::identity::{AbilityId, AbilityPresentation, ChoicePresentation};
use crate::presentation_path::{PresentationPath, PresentationPathComponent, PresentationPathKind};
use crate::primitives::{
    ActivatedAbilityDef, AuthoredBlock, AuthoredChooseBranch, CardProgram, CastCostGroupDef,
    CastCostOptionDef, InstructionKind, ManaSpendingRestriction, ReflexiveTriggeredAbilityDef,
    ResolvingPermanentModifier, SearchSelectionSlot, SpellEffectKind, StaticAbilityDef,
    TriggeredAbilityDef,
};
use serde::Serialize;

/// The authored definition that owns one presentation mapping.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub enum PresentationSubject<'a> {
    Activated(&'a ActivatedAbilityDef),
    Triggered(&'a TriggeredAbilityDef),
    Reflexive(&'a ReflexiveTriggeredAbilityDef),
    Static(&'a IdentifiedStaticAbility),
    CharacteristicDefining(&'a IdentifiedCharacteristicDefiningAbility),
    Mode(&'a ModeDef),
    CastCostGroup(&'a CastCostGroupDef),
    CastCostOption(&'a CastCostOptionDef),
    ChooseBranch(&'a AuthoredChooseBranch),
    /// A `DrawCountChoice` instruction's presentation, shared by every branch it expands to.
    DrawCountChoice(&'a ChoicePresentation),
    SearchSlot(&'a SearchSelectionSlot),
    ManaRestriction(&'a ManaSpendingRestriction),
}

/// One authored mapping, of either presentation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodePresentation<'a> {
    Ability(&'a AbilityPresentation),
    Choice(&'a ChoicePresentation),
}

impl<'a> PresentationSubject<'a> {
    pub fn presentation(self) -> NodePresentation<'a> {
        match self {
            Self::Activated(ability) => NodePresentation::Ability(&ability.presentation),
            Self::Triggered(ability) => NodePresentation::Ability(&ability.presentation),
            Self::Reflexive(ability) => NodePresentation::Ability(&ability.presentation),
            Self::Static(ability) => NodePresentation::Ability(&ability.presentation),
            Self::CharacteristicDefining(ability) => {
                NodePresentation::Ability(&ability.presentation)
            }
            Self::Mode(mode) => NodePresentation::Ability(&mode.presentation),
            Self::CastCostGroup(group) => NodePresentation::Ability(&group.presentation),
            Self::CastCostOption(option) => NodePresentation::Choice(option.presentation()),
            Self::ChooseBranch(branch) => NodePresentation::Choice(&branch.presentation),
            Self::DrawCountChoice(presentation) => NodePresentation::Choice(presentation),
            Self::SearchSlot(slot) => NodePresentation::Choice(&slot.presentation),
            Self::ManaRestriction(restriction) => {
                NodePresentation::Choice(&restriction.presentation)
            }
        }
    }
}

/// One presentation node: the path the engine publishes for it and the definition it maps.
#[derive(Debug, Clone, Copy)]
pub struct PresentationNode<'p, 'a> {
    pub path: &'p PresentationPath,
    pub subject: PresentationSubject<'a>,
}

/// Visits every authored presentation node on one face, in authored order, with the exact
/// [`PresentationPath`] the engine publishes for it. A `DrawCountChoice` is visited once per
/// expanded branch id because the client sees one resolution branch per alternative.
pub fn visit_face_presentations<'a>(
    face: &'a CardFace,
    mut visit: impl FnMut(PresentationNode<'_, 'a>),
) {
    FaceVisitor { visit: &mut visit }.face(face);
}

/// Every presentation path on one face, in visit order (duplicates retained).
pub fn face_presentation_paths(face: &CardFace) -> Vec<PresentationPath> {
    let mut paths = Vec::new();
    visit_face_presentations(face, |node| paths.push(node.path.clone()));
    paths
}

/// The paths a face yields for two different definitions. An empty result means every node on
/// the face has a distinct published identity. Equal definitions reached twice (a
/// `DrawCountChoice` shared by its branches, one ability granted by sibling modes) are one node.
pub fn duplicate_face_presentation_paths(face: &CardFace) -> Vec<PresentationPath> {
    let mut seen =
        std::collections::BTreeMap::<PresentationPath, Vec<PresentationSubject<'_>>>::new();
    let mut duplicates = std::collections::BTreeSet::new();
    visit_face_presentations(face, |node| {
        let subjects = seen.entry(node.path.clone()).or_default();
        if subjects.iter().any(|subject| *subject != node.subject) {
            duplicates.insert(node.path.clone());
        }
        subjects.push(node.subject);
    });
    duplicates.into_iter().collect()
}

struct FaceVisitor<'v, F> {
    visit: &'v mut F,
}

impl<'a, F: FnMut(PresentationNode<'_, 'a>)> FaceVisitor<'_, F> {
    fn emit(&mut self, path: &PresentationPath, subject: PresentationSubject<'a>) {
        (self.visit)(PresentationNode { path, subject });
    }

    fn face(&mut self, face: &'a CardFace) {
        let spell = PresentationPath::spell();
        if let Some(modal) = &face.modal_spell {
            self.modes(&spell, &spell, &modal.modes);
        }
        for group in &face.cast_cost_groups {
            let group_path = spell.child(PresentationPathComponent::CastCostGroup(
                group.group_id.clone(),
            ));
            self.emit(&group_path, PresentationSubject::CastCostGroup(group));
            for option in &group.options {
                let option_path = group_path.child(PresentationPathComponent::CastCostOption(
                    option.option_id().clone(),
                ));
                self.emit(&option_path, PresentationSubject::CastCostOption(option));
            }
        }
        self.effects(&spell, &face.spell_effect);
        if let Some(program) = &face.spell_program {
            self.block(&spell, &program.body);
        }

        let root = PresentationPath::default();
        for ability in &face.characteristic_defining_abilities {
            let path = ability_child(&root, &ability.ability_id);
            self.emit(&path, PresentationSubject::CharacteristicDefining(ability));
        }
        // Class level bars flatten into the face's ability lists, so their abilities publish
        // top-level paths exactly like printed abilities.
        for ability in
            face.activated_abilities
                .iter()
                .chain(face.class_level_bars.iter().flat_map(|bar| {
                    std::iter::once(&bar.level_ability).chain(&bar.activated_abilities)
                }))
        {
            self.activated(&root, ability);
        }
        for ability in face.triggered_abilities.iter().chain(
            face.class_level_bars
                .iter()
                .flat_map(|bar| &bar.triggered_abilities),
        ) {
            self.triggered(&root, ability);
        }
        for ability in face.static_abilities.iter().chain(
            face.class_level_bars
                .iter()
                .flat_map(|bar| &bar.static_abilities),
        ) {
            self.static_ability(ability);
        }
    }

    /// Modes publish under `mode_parent` (the spell or the owning ability). Their bodies resolve
    /// as part of the stack item, whose children hang off `stack_owner`.
    fn modes(
        &mut self,
        mode_parent: &PresentationPath,
        stack_owner: &PresentationPath,
        modes: &'a [ModeDef],
    ) {
        for mode in modes {
            let path = mode_parent.child(PresentationPathComponent::Mode(mode.mode_id.clone()));
            self.emit(&path, PresentationSubject::Mode(mode));
            self.effects(stack_owner, &mode.effects);
            if let Some(program) = &mode.program {
                self.block(stack_owner, &program.body);
            }
        }
    }

    fn activated(&mut self, parent: &PresentationPath, ability: &'a ActivatedAbilityDef) {
        let path = ability_child(parent, &ability.ability_id);
        self.emit(&path, PresentationSubject::Activated(ability));
        self.ability_body(
            &path,
            &ability.effect,
            ability.program.as_ref(),
            ability.modal.as_ref(),
        );
    }

    fn triggered(&mut self, parent: &PresentationPath, ability: &'a TriggeredAbilityDef) {
        let path = ability_child(parent, &ability.ability_id);
        self.emit(&path, PresentationSubject::Triggered(ability));
        self.ability_body(
            &path,
            &ability.effect,
            ability.program.as_ref(),
            ability.modal.as_ref(),
        );
    }

    fn ability_body(
        &mut self,
        path: &PresentationPath,
        effects: &'a [SpellEffectKind],
        program: Option<&'a CardProgram>,
        modal: Option<&'a ModalDef>,
    ) {
        if let Some(modal) = modal {
            self.modes(path, path, &modal.modes);
        }
        self.effects(path, effects);
        if let Some(program) = program {
            self.block(path, &program.body);
        }
    }

    fn static_ability(&mut self, ability: &'a IdentifiedStaticAbility) {
        let path = ability_child(&PresentationPath::default(), &ability.ability_id);
        self.emit(&path, PresentationSubject::Static(ability));
        let (activated, triggered): (&[ActivatedAbilityDef], &[TriggeredAbilityDef]) =
            match &ability.definition {
                StaticAbilityDef::AttachedModifier {
                    activated_abilities,
                    triggered_abilities,
                    ..
                }
                | StaticAbilityDef::ConditionalSelfModifier {
                    activated_abilities,
                    triggered_abilities,
                    ..
                } => (activated_abilities, triggered_abilities),
                StaticAbilityDef::GrantActivatedAbilityToPermanents {
                    activated_abilities,
                    ..
                } => (activated_abilities, &[]),
                StaticAbilityDef::GrantTriggeredAbilityToPermanents {
                    triggered_abilities,
                    ..
                }
                | StaticAbilityDef::ConditionalPermanentTransformation {
                    triggered_abilities,
                    ..
                } => (&[], triggered_abilities),
                _ => (&[], &[]),
            };
        for child in activated {
            self.activated(&path, child);
        }
        for child in triggered {
            self.triggered(&path, child);
        }
    }

    fn block(&mut self, owner: &PresentationPath, block: &'a AuthoredBlock) {
        for instruction in &block.steps {
            match &instruction.kind {
                InstructionKind::ImportCastPayment => {}
                InstructionKind::Perform(effect) => self.effect(owner, effect),
                InstructionKind::If { yes, no, .. } | InstructionKind::IfResult { yes, no, .. } => {
                    self.block(owner, yes);
                    self.block(owner, no);
                }
                InstructionKind::DrawCountChoice {
                    presentation,
                    alternatives,
                    ..
                } => {
                    for alternative in alternatives {
                        let path = owner.child(PresentationPathComponent::ResolutionBranch(
                            alternative.branch_id.clone(),
                        ));
                        self.emit(&path, PresentationSubject::DrawCountChoice(presentation));
                    }
                }
                InstructionKind::Choose {
                    branches,
                    otherwise,
                    ..
                } => {
                    for branch in branches {
                        let path = owner.child(PresentationPathComponent::ResolutionBranch(
                            branch.branch_id.clone(),
                        ));
                        self.emit(&path, PresentationSubject::ChooseBranch(branch));
                        self.block(owner, &branch.body);
                    }
                    self.block(owner, otherwise);
                }
            }
        }
    }

    fn effects(&mut self, owner: &PresentationPath, effects: &'a [SpellEffectKind]) {
        for effect in effects {
            self.effect(owner, effect);
        }
    }

    /// Mirrors where the engine publishes each effect-owned node. Search slots, mana
    /// restrictions, delayed children, reflexive triggers and resolving grants hang off the
    /// resolving stack item, on the face that authored them (never the grant's recipient).
    /// Emblem triggers keep only their immediate parent ability.
    fn effect(&mut self, owner: &PresentationPath, effect: &'a SpellEffectKind) {
        match effect {
            SpellEffectKind::Conditional { effect, .. }
            | SpellEffectKind::ConditionalCastCost { effect, .. } => self.effect(owner, effect),
            SpellEffectKind::MayBehold { if_beheld, .. } => self.effects(owner, if_beheld),
            SpellEffectKind::SearchLibrary { slots, .. } => {
                for slot in slots {
                    let path =
                        owner.child(PresentationPathComponent::SearchSlot(slot.slot_id.clone()));
                    self.emit(&path, PresentationSubject::SearchSlot(slot));
                }
            }
            SpellEffectKind::ProduceMana {
                restriction: Some(restriction),
                ..
            } => {
                let path = owner.child(PresentationPathComponent::ManaRestriction(
                    restriction.restriction_id.clone(),
                ));
                self.emit(&path, PresentationSubject::ManaRestriction(restriction));
            }
            SpellEffectKind::CreateDelayedTrigger { ability, .. }
            | SpellEffectKind::CreateDelayedTriggerThisTurn { ability }
            | SpellEffectKind::CreateWatchedCombatDamageDrawUntilNextTurn { ability, .. } => {
                self.triggered(owner, ability)
            }
            SpellEffectKind::CreateTriggeredEmblem { ability, .. } => {
                let parent = PresentationPath::new(
                    owner
                        .components()
                        .iter()
                        .rev()
                        .find(|component| component.kind() == PresentationPathKind::Ability)
                        .cloned(),
                );
                self.triggered(&parent, ability)
            }
            SpellEffectKind::GrantTriggeredAbility { ability, .. } => {
                self.triggered(owner, ability)
            }
            SpellEffectKind::CreateReflexiveTrigger { ability, .. } => {
                let path = ability_child(owner, &ability.ability_id);
                self.emit(&path, PresentationSubject::Reflexive(ability));
                self.effects(&path, &ability.effect);
            }
            SpellEffectKind::ApplyPermanentModifier { modifier, .. } => {
                self.permanent_modifier(owner, modifier)
            }
            SpellEffectKind::MoveGraveyardCards {
                entry_modifiers, ..
            }
            | SpellEffectKind::ReturnLinkedExiledCards {
                entry_modifiers, ..
            } => {
                for modifier in entry_modifiers {
                    self.permanent_modifier(owner, modifier);
                }
            }
            _ => {}
        }
    }

    fn permanent_modifier(
        &mut self,
        owner: &PresentationPath,
        modifier: &'a ResolvingPermanentModifier,
    ) {
        if let ResolvingPermanentModifier::GrantActivatedAbility(ability) = modifier {
            self.activated(owner, ability);
        }
    }
}

fn ability_child(parent: &PresentationPath, id: &AbilityId) -> PresentationPath {
    parent.child(PresentationPathComponent::Ability(id.clone()))
}
