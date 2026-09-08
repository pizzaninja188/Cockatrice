//! Owned copy values and token construction identity (CR 707 / 608.2h).
use super::*;

pub(super) fn double_faced_values(definition: &CardDefinition) -> Option<Box<DoubleFacedToken>> {
    if !matches!(definition.layout, Layout::Transform | Layout::ModalDfc) {
        return None;
    }
    let face_values = |index| {
        Some(CopiableValues {
            source_card_id: definition.id.clone(),
            source_face_index: index,
            display_name: definition.face_display_name(index)?.to_string(),
            face: definition.face(index)?.clone(),
            room_faces: None,
        })
    };
    Some(Box::new(DoubleFacedToken {
        layout: definition.layout,
        faces: [face_values(0)?, face_values(1)?],
    }))
}

/// Capture both faces only when the physical source actually is double-faced. Copy-layer
/// provenance alone cannot make a Clone double-faced, and face-down identity stays anonymous.
pub(super) fn token_copy_snapshot_from(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Option<TokenCopySnapshot> {
    let object = state.objects.get(&oid)?;
    let values = copiable_values_from(state, registry, oid)?;
    let token_id = if object.face_down {
        "anonymous_creature_token".to_string()
    } else {
        object.card_id.clone()
    };
    let intrinsic_faces = if object.face_down {
        None
    } else if object.is_token() {
        object.token_faces.clone()
    } else {
        registry.get(&object.card_id).and_then(double_faced_values)
    };
    let faces = intrinsic_faces.map(|mut faces| {
        // A copy effect on the double-faced source applies to both of its faces (707.8a).
        // The resulting token freezes those modified values as its own intrinsic faces.
        if object.copiable_values.is_some() {
            faces.faces = [values.clone(), values.clone()];
        }
        faces
    });
    let face_up_index = if faces.is_some() {
        object.face_up_index
    } else {
        0
    };
    Some(TokenCopySnapshot {
        token_id,
        values,
        faces,
        face_up_index,
    })
}

impl GameEngine {
    pub(super) fn source_token_copy_snapshot(&self, item: &StackItem) -> Option<TokenCopySnapshot> {
        let source = item.source_permanent_id?;
        if self.source_is_current_object(item) {
            token_copy_snapshot_from(&self.state, self.registry, source)
        } else {
            self.state
                .last_known_copy_by_generation
                .get(&(source, item.source_zone_change))
                .cloned()
        }
    }
}

pub(super) fn copiable_values_from(
    state: &GameState,
    registry: &'static CardRegistry,
    oid: ObjectId,
) -> Option<CopiableValues> {
    let object = state.objects.get(&oid)?;
    // CR 707.2: layer 1b values are public and copied even when the underlying card is not.
    if object.face_down && object.zone == Zone::Battlefield {
        return Some(CopiableValues {
            source_card_id: String::new(),
            source_face_index: 0,
            face: CardFace {
                types: vec!["Creature".into()],
                is_creature: true,
                power: Some(2),
                toughness: Some(2),
                ..Default::default()
            },
            room_faces: None,
            display_name: "Face-down creature".into(),
        });
    }
    if let Some(values) = object
        .copiable_values
        .as_ref()
        .or(object.token_origin.as_ref())
    {
        return Some(values.clone());
    }
    let definition = registry.get(&object.card_id)?;
    let mut face = definition.face(object.face_up_index)?.clone();
    if definition.layout == Layout::Flip && object.face_up_index > 0 {
        // Flipping replaces only the alternative characteristics; mana cost is retained.
        face.mana_cost = definition.primary_face().mana_cost.clone();
        face.colors_override = Some(definition.primary_face().colors());
    }
    Some(CopiableValues {
        source_card_id: object.card_id.clone(),
        source_face_index: object.face_up_index,
        face,
        room_faces: (definition.layout == Layout::Room).then(|| definition.faces.clone()),
        display_name: definition
            .face_display_name(object.face_up_index)?
            .to_string(),
    })
}
