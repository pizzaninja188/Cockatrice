#include "ruled_client_state.h"
#include "ruled_diagnostic_values.h"

using RuledDiagnosticValues::value;

QJsonObject RuledClientState::diagnosticSnapshot() const
{
    QJsonObject result{{"privacy", "recipient_only"}, {"format_version", 1}};
    QJsonArray objects;
    auto ids = engineOidToCardId.keys();
    std::sort(ids.begin(), ids.end());
    for (const auto oid : ids) {
        objects.append(QJsonObject{
            {"engine_object_id", value(oid)},
            {"server_card_id", engineOidToCardId.value(oid)},
            {"owner_player_id", engineOidOwner.contains(oid) ? value(engineOidOwner.value(oid)) : QJsonValue()},
            {"zone_change_generation",
             battlefieldGenerationByOid.contains(oid) ? value(battlefieldGenerationByOid.value(oid)) : QJsonValue()}});
    }
    result.insert("battlefield_objects", objects);
#define FIELD(name) result.insert(#name, value(name))
    FIELD(engineCommandPending);
    FIELD(engineCommandIndicatorVisible);
    FIELD(engineCommandGeneration);
    FIELD(choiceWaitingPlayerId);
    FIELD(openingPickSeatIds);
    FIELD(openingMulliganCount);
    FIELD(cleanupDiscardSelectedIndices);
    FIELD(openingBottomSelectedIndices);
    FIELD(ownerCardIdToEngineOid);
    FIELD(engineOidToCardId);
    FIELD(engineOidOwner);
    FIELD(ownedCardToEngineHandSlot);
    FIELD(battlefieldGenerationByOid);
    FIELD(abilitySourceGenerationByOid);
    FIELD(privateFaceDownNameByOwnedCard);
    FIELD(privateFaceDownGenerationByOid);
    FIELD(graveyardOidToPlayerId);
    FIELD(graveyardOidToServerCardId);
    FIELD(exileOidToPlayerId);
    FIELD(exileOidToServerCardId);
    FIELD(pendingCastGraveyardOids);
    FIELD(currentActivePlayerId);
    FIELD(pendingAttackerOids);
    FIELD(currentAttackerOids);
    FIELD(pendingAttackAssignments);
    FIELD(currentAttackAssignments);
    FIELD(remoteAttackPreviewAssignments);
    FIELD(pendingBlocks);
    FIELD(committedBlocks);
    FIELD(remoteBlockPreviewPairs);
    FIELD(requiredAttackerOids);
    FIELD(requiredBlockerOids);
    FIELD(selectableAttackerOids);
    FIELD(legalBlockAttackerOidsByBlocker);
    FIELD(stagedBlockerOids);
    FIELD(attackersSubmittedThisStep);
    FIELD(blockersSubmittedThisStep);
    FIELD(attackerAwaitingDefenderOid);
    FIELD(combatDamagePendingAttackers);
    FIELD(currentCombatDamageAttackerIdx);
    FIELD(committedBlockerGroups);
    FIELD(pendingCombatDamageByBlocker);
    FIELD(stackOidOrder);
    FIELD(triggerOrderCandidateOids);
    FIELD(firstStrikeStepPending);
    FIELD(stackTargetsByStackOid);
    FIELD(stackAnnotationByOid);
    FIELD(stackSourceOidByStackOid);
    FIELD(activatedAbilitiesByOid);
    FIELD(engineOidMarkedDamage);
    FIELD(engineOidBattlefieldPower);
    FIELD(engineOidBattlefieldToughness);
    FIELD(engineOidLoyalty);
    FIELD(engineOidDefense);
    FIELD(engineOidBattleProtector);
    FIELD(lastTriggerSourceOid);
    FIELD(lastTriggerAbilityIndex);
    FIELD(lastTriggerControllerPlayerId);
    FIELD(handActions);
    FIELD(zoneCastActions);
    FIELD(zoneCastSourceByOid);
    FIELD(zoneCastCostsByCastKey);
    FIELD(zoneLandFacesByOid);
    FIELD(zoneLandSourceByOid);
    FIELD(exilePlayPermissionGroups);
    FIELD(engineOidSummoningSick);
    FIELD(engineOidHaste);
    FIELD(engineOidTrample);
    FIELD(engineOidCreature);
    FIELD(permanentActionsByOid);
    FIELD(handAbilityOidBySlot);
    FIELD(zoneAbilitySourceByOid);
    FIELD(zoneAbilityIndicesByOid);
    FIELD(ownedGraveyardCardToEngineOid);
    FIELD(ownedExileCardToEngineOid);
    FIELD(validTargetsByHandSlot);
    FIELD(validTargetsByZoneObject);
    FIELD(validTargetsByAbility);
    FIELD(abilityCostData);
    FIELD(remoteAttackerPreviewOids);
    FIELD(legalAttackAssignmentsByAttacker);
    FIELD(stackTargetKindByStackAndTargetOid);
    FIELD(syntheticAbilityFakeIds);
    FIELD(syntheticAbilityControllerPid);
    FIELD(eligibleRestrictedManaByAbility);
    FIELD(waterbendAbilities);
    FIELD(restrictedManaByPlayer);
#undef FIELD
    result.insert("lastEnginePhaseId", QString::fromStdString(ruled::v1::PhaseId_Name(lastEnginePhaseId)));
    const char *combat[] = {
        "None", "DeclareAttackers", "DeclareBlockers", "AssignCombatDamage", "FirstStrikeDamage", "CombatDamage"};
    result.insert("currentCombatPhase", combat[static_cast<int>(currentCombatPhase)]);
    const char *opening[] = {"None", "ChooseFirst", "MulliganChoice", "BottomLibrary"};
    result.insert("openingUiKind", opening[static_cast<int>(openingUiKind)]);
    result.insert("payment", payment.diagnosticSnapshot());
    QJsonValue pending;
    if (pendingChoice) {
        const auto &choice = *pendingChoice;
        const char *kinds[] = {
            "TriggerTarget", "TriggerMode",    "CopyTarget",        "PermanentChoice",  "CopySource",
            "LegendKeep",    "AuraPermanent",  "AuraPlayer",        "BattleProtector",  "AttackingTokenDefender",
            "CostObjects",   "ResolutionPick", "ResolutionPayment", "ResolutionBranch", "SpecialCast",
            "TriggerOrder", "ReplacementOption"};
        QJsonObject choiceState{{"kind", kinds[static_cast<int>(choice.kind)]}};
#define FIELD(name) choiceState.insert(#name, value(choice.name))
        FIELD(promptText);
        FIELD(mayDecline);
        FIELD(candidateOids);
        FIELD(selectedObjectOids);
        FIELD(combatDefenderOptions);
        FIELD(serverCardIdToOid);
        FIELD(serverCardIdToName);
        FIELD(hasSelectableRestriction);
        FIELD(selectableServerCardIds);
        FIELD(selectedServerCardIds);
        FIELD(reverseSelectionOrder);
        FIELD(min);
        FIELD(max);
        FIELD(uniqueNames);
        FIELD(selectionSlotServerCardIds);
        FIELD(selectionSlotLabels);
        FIELD(viewTitle);
        FIELD(showViewControls);
        FIELD(candidateNames);
        FIELD(candidateAnnotations);
        FIELD(publicReveal);
        FIELD(genericManaCost);
        FIELD(waterbend);
        FIELD(paymentSourceOid);
        FIELD(paymentCurrentlyLegal);
        FIELD(manaCost);
        FIELD(selectedTriggerMode);
        FIELD(triggerTargets);
        FIELD(orderCandidates);
        FIELD(selectedTriggerTargetsByGroup);
        FIELD(activeTriggerTargetGroupPosition);
        FIELD(orderCardIdToOid);
#undef FIELD
        QJsonArray options;
        for (const auto &option : choice.choiceOptions)
            options.append(QJsonObject{{"index", option.index},
                                       {"label", option.label},
                                       {"enabled", option.enabled},
                                       {"needsTarget", option.needsTarget},
                                       {"searchZones", value(option.searchZones)}});
        choiceState.insert("choiceOptions", options);
        pending = choiceState;
    }
    result.insert("pendingChoice", pending);
    result.insert("reveals", value(reveals.entries()));
    result.insert("revealOrder", value(reveals.order()));
    result.insert("revealChoiceId", reveals.choiceId());
    result.insert("revealWindowsSuppressed", reveals.windowsSuppressed());
    return result;
}

QJsonObject RuledPayment::diagnosticSnapshot() const
{
    QJsonObject result;
#define FIELD(name) result.insert(#name, value(name))
    FIELD(active);
    FIELD(pending);
    FIELD(submitting);
    FIELD(transactionId);
    FIELD(revision);
    FIELD(guardSanitizedPayment);
    FIELD(submissionArmed);
    FIELD(selection);
    FIELD(view);
    FIELD(retiredOptimisticManaCounterIds);
#undef FIELD
    QJsonArray queued, optimistic, restricted;
    for (const auto &entry : queuedMana)
        queued.append(QJsonObject{
            {"symbol", value(entry.symbol)}, {"group_id", value(entry.groupId)}, {"counter_id", entry.counterId}});
    for (const auto &entry : optimisticManaCounters)
        optimistic.append(QJsonObject{
            {"symbol", value(entry.symbol)}, {"group_id", value(entry.groupId)}, {"counter_id", entry.counterId}});
    for (const auto &entry : restrictedMana)
        restricted.append(value(entry));
    result.insert("queuedMana", queued);
    result.insert("optimisticManaCounters", optimistic);
    result.insert("restrictedMana", restricted);
    return result;
}

namespace RuledDiagnosticValues
{
QJsonValue value(const RuledTargetGroupData &v)
{
    QJsonObject result;
    result.insert("groupIndex", value(v.groupIndex));
    result.insert("validPermanentIds", value(v.validPermanentIds));
    result.insert("validStackIds", value(v.validStackIds));
    result.insert("validGraveyardIds", value(v.validGraveyardIds));
    result.insert("canTargetSelf", value(v.canTargetSelf));
    result.insert("canTargetOpponent", value(v.canTargetOpponent));
    result.insert("minTargets", value(v.minTargets));
    result.insert("maxTargets", value(v.maxTargets));
    result.insert("promptText", value(v.promptText));
    result.insert("distinctFromGroupIndices", value(v.distinctFromGroupIndices));
    result.insert("sameGraveyard", value(v.sameGraveyard));
    return result;
}
QJsonValue value(const RuledTargetingCostCandidate &v)
{
    QJsonObject result;
    result.insert("kind", value(v.kind));
    result.insert("oid", value(v.oid));
    return result;
}
QJsonValue value(const RuledTargetingCostApplication &v)
{
    QJsonObject result;
    result.insert("applicationId", value(v.applicationId));
    result.insert("genericMana", value(v.genericMana));
    result.insert("affectedTargets", value(v.affectedTargets));
    return result;
}
QJsonValue value(const RuledTargetedCostReductionApplication &v)
{
    QJsonObject result;
    result.insert("applicationId", value(v.applicationId));
    result.insert("genericMana", value(v.genericMana));
    result.insert("qualifyingTargets", value(v.qualifyingTargets));
    return result;
}
QJsonValue value(const RuledTargetCastCostRequirement &v)
{
    QJsonObject result;
    result.insert("groupIndex", value(v.groupIndex));
    result.insert("costGroupIndex", value(v.costGroupIndex));
    result.insert("costOptionIndex", value(v.costOptionIndex));
    result.insert("affectedTargets", value(v.affectedTargets));
    return result;
}
QJsonValue value(const RuledSpellTargetData &v)
{
    QJsonObject result = value(static_cast<const RuledTargetGroupData &>(v)).toObject();
    result.insert("groups", value(v.groups));
    result.insert("fixedDamage", value(v.fixedDamage));
    result.insert("isDamageTargets", value(v.isDamageTargets));
    result.insert("extraManaPerTarget", value(v.extraManaPerTarget));
    result.insert("damageDividedEvenly", value(v.damageDividedEvenly));
    result.insert("targetingCostApplications", value(v.targetingCostApplications));
    result.insert("targetedCostReductionApplications", value(v.targetedCostReductionApplications));
    result.insert("castCostRequirements", value(v.castCostRequirements));
    return result;
}
QJsonValue value(const RuledChoiceOption &v)
{
    QJsonObject result;
    result.insert("index", value(v.index));
    result.insert("label", value(v.label));
    result.insert("enabled", value(v.enabled));
    result.insert("needsTarget", value(v.needsTarget));
    result.insert("targets", value(v.targets));
    result.insert("searchZones", value(v.searchZones));
    return result;
}
QJsonValue value(const RuledAbilityEntry &v)
{
    return QJsonObject{{"text", value(v.text)},
                       {"manaCost", value(v.manaCost)},
                       {"manaProduced", value(v.manaProduced)},
                       {"costLabel", value(v.costLabel)},
                       {"activatable", value(v.activatable)}};
}
QJsonValue value(const RuledPermanentAction &v)
{
    QJsonObject result;
    result.insert("kind", value(v.kind));
    result.insert("objectId", value(v.objectId));
    result.insert("zoneChangeGeneration", value(v.zoneChangeGeneration));
    result.insert("label", value(v.label));
    result.insert("manaCost", value(v.manaCost));
    result.insert("faceIndex", value(v.faceIndex));
    result.insert("eligibleRestrictedManaGroupIds", value(v.eligibleRestrictedManaGroupIds));
    return result;
}
QJsonValue value(const RuledCounterRemovalOption &v)
{
    QJsonObject result;
    result.insert("optionId", value(v.optionId));
    result.insert("label", value(v.label));
    result.insert("availableCount", value(v.availableCount));
    return result;
}
QJsonValue value(const RuledCostChoice &v)
{
    QJsonObject result;
    result.insert("costIndex", value(v.costIndex));
    result.insert("zone", value(v.zone));
    result.insert("candidateIds", value(v.candidateIds));
    result.insert("min", value(v.min));
    result.insert("max", value(v.max));
    result.insert("kind", value(v.kind));
    result.insert("candidateGenerations", value(v.candidateGenerations));
    result.insert("candidateContributions", value(v.candidateContributions));
    result.insert("aggregateMinimum", value(v.aggregateMinimum));
    result.insert("contributionKind", value(v.contributionKind));
    result.insert("blightCount", value(v.blightCount));
    result.insert("counterSourceId", value(v.counterSourceId));
    result.insert("counterSourceGeneration", value(v.counterSourceGeneration));
    result.insert("counterCount", value(v.counterCount));
    result.insert("counterOptions", value(v.counterOptions));
    return result;
}
QJsonValue value(const RuledCastCostOption &v)
{
    QJsonObject result;
    result.insert("optionIndex", value(v.optionIndex));
    result.insert("label", value(v.label));
    result.insert("kind", value(v.kind));
    result.insert("additionalManaCost", value(v.additionalManaCost));
    result.insert("validHandIndices", value(v.validHandIndices));
    result.insert("validPermanentIds", value(v.validPermanentIds));
    result.insert("validPermanentGenerations", value(v.validPermanentGenerations));
    result.insert("validPermanentGenericReductions", value(v.validPermanentGenericReductions));
    result.insert("candidateContributions", value(v.candidateContributions));
    result.insert("objectMin", value(v.objectMin));
    result.insert("objectMax", value(v.objectMax));
    result.insert("aggregateMinimum", value(v.aggregateMinimum));
    result.insert("contributionKind", value(v.contributionKind));
    result.insert("selectable", value(v.selectable));
    return result;
}
QJsonValue value(const RuledCastCostGroup &v)
{
    QJsonObject result;
    result.insert("groupIndex", value(v.groupIndex));
    result.insert("prompt", value(v.prompt));
    result.insert("min", value(v.min));
    result.insert("max", value(v.max));
    result.insert("options", value(v.options));
    result.insert("skipLabel", value(v.skipLabel));
    return result;
}
QJsonValue value(const RuledCostData &v)
{
    QJsonObject result;
    result.insert("nonManaCostsPayable", value(v.nonManaCostsPayable));
    result.insert("choices", value(v.choices));
    result.insert("castCostGroups", value(v.castCostGroups));
    return result;
}
QJsonValue value(const RuledRestrictedManaGroup &v)
{
    QJsonObject result;
    result.insert("groupId", value(v.groupId));
    result.insert("w", value(v.w));
    result.insert("u", value(v.u));
    result.insert("b", value(v.b));
    result.insert("r", value(v.r));
    result.insert("g", value(v.g));
    result.insert("c", value(v.c));
    result.insert("displayLabel", value(v.displayLabel));
    return result;
}
QJsonValue value(const RuledTriggerOrderCandidate &v)
{
    QJsonObject result;
    result.insert("oid", value(v.oid));
    result.insert("sourceOid", value(v.sourceOid));
    result.insert("cardName", value(v.cardName));
    result.insert("abilityText", value(v.abilityText));
    return result;
}
QJsonValue value(const RuledModalSpellOption &v)
{
    QJsonObject result;
    result.insert("modeIndex", value(v.modeIndex));
    result.insert("label", value(v.label));
    result.insert("selectable", value(v.selectable));
    result.insert("needsTarget", value(v.needsTarget));
    result.insert("targets", value(v.targets));
    result.insert("linkedCastCostGroupIndex", value(v.linkedCastCostGroupIndex));
    result.insert("linkedCastCostOptionIndex", value(v.linkedCastCostOptionIndex));
    return result;
}
QJsonValue value(const RuledCastActionKey &v)
{
    QJsonObject result;
    result.insert("sourceId", value(v.sourceId));
    result.insert("faceIndex", value(v.faceIndex));
    result.insert("source", value(v.source));
    result.insert("method", value(v.method));
    result.insert("castingPermissionId", value(v.castingPermissionId));
    return result;
}
QJsonValue value(const RuledFaceOption &v)
{
    QJsonObject result;
    result.insert("faceIndex", value(v.faceIndex));
    result.insert("faceName", value(v.faceName));
    result.insert("manaCost", value(v.manaCost));
    result.insert("genericCostReduction", value(v.genericCostReduction));
    result.insert("castMethod", value(v.castMethod));
    result.insert("hasConvoke", value(v.hasConvoke));
    result.insert("zoneChangeGeneration", value(v.zoneChangeGeneration));
    result.insert("castingPermissionId", value(v.castingPermissionId));
    result.insert("permissionSourceLabel", value(v.permissionSourceLabel));
    return result;
}
QJsonValue value(const RuledExilePlayPermissionGroup &v)
{
    QJsonObject result;
    result.insert("groupId", value(v.groupId));
    result.insert("sourceLabel", value(v.sourceLabel));
    result.insert("objectIds", value(v.objectIds));
    return result;
}
QJsonValue value(const RuledHandActionSet &v)
{
    QJsonObject result;
    result.insert("handIndices", value(v.handIndices));
    result.insert("faceOptionsByIndex", value(v.faceOptionsByIndex));
    result.insert("needsTargetIndices", value(v.needsTargetIndices));
    result.insert("needsTargetCastKeys", value(v.needsTargetCastKeys));
    result.insert("modalOptionsByCastKey", value(v.modalOptionsByCastKey));
    result.insert("modalMinModesByCastKey", value(v.modalMinModesByCastKey));
    result.insert("modalMaxModesByCastKey", value(v.modalMaxModesByCastKey));
    result.insert("allModesCastCostByCastKey", value(v.allModesCastCostByCastKey));
    result.insert("costDataByCastKey", value(v.costDataByCastKey));
    result.insert("eligibleRestrictedManaByCastKey", value(v.eligibleRestrictedManaByCastKey));
    QJsonArray names;
    auto keys = v.indicesByCardName.uniqueKeys();
    std::sort(keys.begin(), keys.end());
    for (const auto &name : keys) {
        auto engineSlots = v.indicesByCardName.values(name);
        std::sort(engineSlots.begin(), engineSlots.end());
        names.append(QJsonObject{{"known_name", name}, {"engine_hand_slots", value(engineSlots)}});
    }
    result.insert("indicesByCardName", names);
    return result;
}
QJsonValue value(const RuledFlexPip &v)
{
    QJsonObject result;
    result.insert("pipIndex", value(v.pipIndex));
    result.insert("colorA", value(v.colorA));
    result.insert("colorB", value(v.colorB));
    result.insert("generic", value(v.generic));
    result.insert("phyrexian", value(v.phyrexian));
    result.insert("genericPaid", value(v.genericPaid));
    return result;
}
QJsonValue value(const RuledPendingCostSelection &v)
{
    QJsonObject result;
    result.insert("costIndex", value(v.costIndex));
    result.insert("zone", value(v.zone));
    result.insert("selectedIds", value(v.selectedIds));
    result.insert("selectedGenerations", value(v.selectedGenerations));
    result.insert("counterOptionId", value(v.counterOptionId));
    return result;
}
QJsonValue value(const RuledPendingCastCostSelection &v)
{
    QJsonObject result;
    result.insert("groupIndex", value(v.groupIndex));
    result.insert("optionIndex", value(v.optionIndex));
    result.insert("objectKind", value(v.objectKind));
    result.insert("selectedId", value(v.selectedId));
    result.insert("expectedZoneChangeGeneration", value(v.expectedZoneChangeGeneration));
    result.insert("genericCostReduction", value(v.genericCostReduction));
    result.insert("selectedObjectIds", value(v.selectedObjectIds));
    result.insert("selectedObjectGenerations", value(v.selectedObjectGenerations));
    result.insert("selectedObjectContributions", value(v.selectedObjectContributions));
    return result;
}
QJsonValue value(const PendingActivatedAbility &v)
{
    QJsonObject result;
    result.insert("valid", value(v.valid));
    result.insert("permanentAction", value(v.permanentAction));
    result.insert("permanentActionKind", value(v.permanentActionKind));
    result.insert("permanentActionFaceIndex", value(v.permanentActionFaceIndex));
    result.insert("sourceZone", value(v.sourceZone));
    result.insert("expectedZoneChangeGeneration", value(v.expectedZoneChangeGeneration));
    result.insert("permanentOid", value(v.permanentOid));
    result.insert("abilityIndex", value(v.abilityIndex));
    result.insert("manaOptionIndex", value(v.manaOptionIndex));
    result.insert("abilityText", value(v.abilityText));
    result.insert("cardName", value(v.cardName));
    result.insert("needsTarget", value(v.needsTarget));
    result.insert("waitingForTarget", value(v.waitingForTarget));
    result.insert("selectedTargetOid", value(v.selectedTargetOid));
    result.insert("waitingForCost", value(v.waitingForCost));
    result.insert("costChoices", value(v.costChoices));
    result.insert("nextCostChoice", value(v.nextCostChoice));
    result.insert("costSelections", value(v.costSelections));
    result.insert("waitingForMana", value(v.waitingForMana));
    result.insert("remainingCost", value(v.remainingCost));
    result.insert("flexPips", value(v.flexPips));
    result.insert("lifePipIndices", value(v.lifePipIndices));
    result.insert("targetingCostApplied", value(v.targetingCostApplied));
    return result;
}
QJsonValue value(const PendingRuledSpellCast::SelectedMode &v)
{
    QJsonObject result;
    result.insert("modeIndex", value(v.modeIndex));
    result.insert("label", value(v.label));
    result.insert("needsTarget", value(v.needsTarget));
    result.insert("targets", value(v.targets));
    result.insert("selectedTargetOids", value(v.selectedTargetOids));
    result.insert("selectedTargetDamages", value(v.selectedTargetDamages));
    result.insert("selectedTargetOidsByGroup", value(v.selectedTargetOidsByGroup));
    result.insert("selectedTargetDamagesByGroup", value(v.selectedTargetDamagesByGroup));
    result.insert("linkedCastCostGroupIndex", value(v.linkedCastCostGroupIndex));
    result.insert("linkedCastCostOptionIndex", value(v.linkedCastCostOptionIndex));
    return result;
}
QJsonValue value(const PendingRuledSpellCast &v)
{
    QJsonObject result;
    result.insert("hasConvoke", value(v.hasConvoke));
    result.insert("handIndex", value(v.handIndex));
    result.insert("source", value(v.source));
    result.insert("castMethod", value(v.castMethod));
    result.insert("sourceZoneChangeGeneration", value(v.sourceZoneChangeGeneration));
    result.insert("castingPermissionId", value(v.castingPermissionId));
    result.insert("damageDividedEvenly", value(v.damageDividedEvenly));
    result.insert("faceIndex", value(v.faceIndex));
    result.insert("cardName", value(v.cardName));
    result.insert("remainingCost", value(v.remainingCost));
    result.insert("selectedTargetOids", value(v.selectedTargetOids));
    result.insert("selectedTargetDamages", value(v.selectedTargetDamages));
    result.insert("selectedTargetOidsByGroup", value(v.selectedTargetOidsByGroup));
    result.insert("selectedTargetDamagesByGroup", value(v.selectedTargetDamagesByGroup));
    result.insert("activeTargetGroupPosition", value(v.activeTargetGroupPosition));
    result.insert("waitingForTarget", value(v.waitingForTarget));
    result.insert("valid", value(v.valid));
    result.insert("minTargets", value(v.minTargets));
    result.insert("maxTargets", value(v.maxTargets));
    result.insert("fixedDamage", value(v.fixedDamage));
    result.insert("isDamageTargets", value(v.isDamageTargets));
    result.insert("extraManaPerTarget", value(v.extraManaPerTarget));
    result.insert("inDamageAllocationMode", value(v.inDamageAllocationMode));
    result.insert("damageAllocationTotal", value(v.damageAllocationTotal));
    result.insert("targetDamageAllocations", value(v.targetDamageAllocations));
    result.insert("xPips", value(v.xPips));
    result.insert("xValue", value(v.xValue));
    result.insert("genericCostReduction", value(v.genericCostReduction));
    result.insert("castCostGenericReduction", value(v.castCostGenericReduction));
    result.insert("manaCostFinalized", value(v.manaCostFinalized));
    result.insert("flexPips", value(v.flexPips));
    result.insert("lifePipIndices", value(v.lifePipIndices));
    result.insert("waitingForCost", value(v.waitingForCost));
    result.insert("costChoices", value(v.costChoices));
    result.insert("nextCostChoice", value(v.nextCostChoice));
    result.insert("costSelections", value(v.costSelections));
    result.insert("castCostGroups", value(v.castCostGroups));
    result.insert("nextCastCostGroup", value(v.nextCastCostGroup));
    result.insert("waitingForCastCostObject", value(v.waitingForCastCostObject));
    result.insert("activeCastCostOption", value(v.activeCastCostOption));
    result.insert("castCostObjectError", value(v.castCostObjectError));
    result.insert("castCostSelections", value(v.castCostSelections));
    result.insert("modeLinkedCastCosts", value(v.modeLinkedCastCosts));
    result.insert("selectedModeLinkedCastCosts", value(v.selectedModeLinkedCastCosts));
    result.insert("submissionPending", value(v.submissionPending));
    result.insert("selectedModes", value(v.selectedModes));
    result.insert("activeModePosition", value(v.activeModePosition));
    return result;
}
QJsonValue value(const RuledRevealState::Card &v)
{
    QJsonObject result;
    result.insert("objectId", value(v.objectId));
    result.insert("generation", value(v.generation));
    result.insert("cardId", value(v.cardId));
    result.insert("name", value(v.name));
    return result;
}
QJsonValue value(const RuledRevealState::Entry &v)
{
    QJsonObject result;
    result.insert("id", value(v.id));
    result.insert("owner", value(v.owner));
    result.insert("sourceZone", value(v.sourceZone));
    result.insert("sourceObjectId", value(v.sourceObjectId));
    result.insert("sourceDescription", value(v.sourceDescription));
    result.insert("cards", value(v.cards));
    result.insert("phase", value(v.phase));
    result.insert("choiceCardIds", value(v.choiceCardIds));
    return result;
}
} // namespace RuledDiagnosticValues
