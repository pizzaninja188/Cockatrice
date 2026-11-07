#include "ruled_pending_cast.h"

// Widget-independent local target staging. The engine continues to author legal choices.

QSet<quint32> ruledPendingGraveyardTargetOids(const PendingRuledSpellCast &spell,
                                              const PendingActivatedAbility &ability,
                                              const RuledClientState &state)
{
    if (ability.valid && ability.waitingForTarget) {
        const auto data = currentRuledPendingAbilityTargetData(ability, state);
        const auto *group = data ? ruledPendingAbilityTargetGroup(ability, *data) : nullptr;
        return group && !group->chosenByOpponent ? group->validGraveyardIds : QSet<quint32>{};
    }
    if (!spell.waitingForTarget || !ruledCastCostGroupsComplete(spell)) {
        return {};
    }
    const auto group = currentRuledSpellTargetGroup(spell, state);
    return group ? group->validGraveyardIds : QSet<quint32>{};
}

namespace
{
std::optional<QVector<int>> counterTargetIndices(const PendingActivatedAbility &ability,
                                                const RuledSpellTargetData &data)
{
    if (!ability.valid || ability.selectedMode || ability.permanentAction || ability.ownedBegin() ||
        ability.transactionId() != 0 || ability.stage() != PendingActivatedAbility::Stage::Announcing ||
        ability.sourceZone != ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD || ability.chosenOpponentTargets ||
        ability.deferredReturnTappedCreature || !data.counterAllocation || !data.counterAllocation->valid ||
        data.counterAllocation->fixedTotal <= 0 || data.isDamageTargets)
        return std::nullopt;
    const auto &allocation = *data.counterAllocation;
    if (allocation.groupIndex < 0 || allocation.groupIndex >= data.groups.size())
        return std::nullopt;
    const auto &group = data.groups.at(allocation.groupIndex);
    if (group.groupIndex != allocation.groupIndex || group.minTargets < 0 || group.maxTargets <= 0 ||
        group.minTargets > group.maxTargets || group.maxTargets > allocation.fixedTotal ||
        group.canTargetSelf || group.canTargetOpponent || group.chosenByOpponent || group.hasXTargetChoices ||
        !group.validStackIds.isEmpty() || !group.validGraveyardIds.isEmpty())
        return std::nullopt;
    QVector<int> indices;
    QSet<quint32> seen;
    for (int i = 0; i < ability.selectedTargets.size(); ++i) {
        const auto &target = ability.selectedTargets.at(i);
        if (target.ref.group_index() != static_cast<quint32>(allocation.groupIndex)) {
            if (target.ref.counter_amount() != 0)
                return std::nullopt;
            continue;
        }
        const auto generation = group.validPermanentGenerations.constFind(target.ref.object_id());
        if (target.ref.kind() != ruled::v1::TARGET_REF_KIND_PERMANENT || target.ref.damage_amount() != 0 ||
            !group.validPermanentIds.contains(target.ref.object_id()) ||
            generation == group.validPermanentGenerations.cend() ||
            !target.ref.has_expected_zone_change_generation() ||
            target.ref.expected_zone_change_generation() != generation.value() ||
            target.zoneChangeGeneration != generation.value() || seen.contains(target.ref.object_id()))
            return std::nullopt;
        seen.insert(target.ref.object_id());
        indices.append(i);
    }
    if (indices.size() < group.minTargets || indices.size() > group.maxTargets)
        return std::nullopt;
    return indices;
}
} // namespace

RuledPendingCast::CounterAllocationStep
RuledPendingCast::prepareAbilityCounterAllocation(const RuledSpellTargetData &data)
{
    if (!data.counterAllocation) {
        return std::any_of(ability.selectedTargets.cbegin(), ability.selectedTargets.cend(),
                           [](const auto &target) { return target.ref.counter_amount() != 0; })
                   ? CounterAllocationStep::Invalid : CounterAllocationStep::Ready;
    }
    const auto indices = counterTargetIndices(ability, data);
    if (!indices)
        return CounterAllocationStep::Invalid;
    if (indices->isEmpty())
        return CounterAllocationStep::Ready;
    if (indices->size() == 1) {
        ability.selectedTargets[indices->front()].ref.set_counter_amount(data.counterAllocation->fixedTotal);
        return CounterAllocationStep::Ready;
    }
    quint64 sum = 0;
    for (int index : *indices) {
        const auto amount = ability.selectedTargets.at(index).ref.counter_amount();
        if (amount == 0)
            return CounterAllocationStep::Allocating;
        sum += amount;
    }
    return sum == static_cast<quint32>(data.counterAllocation->fixedTotal)
               ? CounterAllocationStep::Ready : CounterAllocationStep::Allocating;
}

bool RuledPendingCast::applyAbilityCounterAllocation(const RuledSpellTargetData &data, const QVector<int> &amounts)
{
    const auto indices = counterTargetIndices(ability, data);
    if (!indices || amounts.size() != indices->size())
        return false;
    qint64 sum = 0;
    for (int amount : amounts) {
        if (amount <= 0)
            return false;
        sum += amount;
        if (sum > data.counterAllocation->fixedTotal)
            return false;
    }
    if (!indices->isEmpty() && sum != data.counterAllocation->fixedTotal)
        return false;
    for (int i = 0; i < indices->size(); ++i)
        ability.selectedTargets[indices->at(i)].ref.set_counter_amount(static_cast<quint32>(amounts.at(i)));
    return true;
}

bool RuledPendingCast::sameAbilityCounterPrompt(const PendingActivatedAbility &captured,
                                               const RuledSpellTargetData &capturedData,
                                               const PendingActivatedAbility &current,
                                               const RuledSpellTargetData &currentData,
                                               const RuledClientState &state)
{
    if (!sameAbilityFlexiblePrompt(captured, current) || captured.waitingForTarget || current.waitingForTarget ||
        !capturedData.counterAllocation || !currentData.counterAllocation ||
        !(*capturedData.counterAllocation == *currentData.counterAllocation) ||
        !counterTargetIndices(captured, capturedData) || !counterTargetIndices(current, currentData) ||
        captured.selectedTargets.size() != current.selectedTargets.size() ||
        capturedData.groups.size() != currentData.groups.size() ||
        state.abilitySourceZone(current.permanentOid) != ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD ||
        !state.abilityActivatable(current.permanentOid, current.abilityIndex) ||
        !ruledPendingAbilitySourceStillCurrent(state, current))
        return false;
    const auto &oldGroup = capturedData.groups.at(capturedData.counterAllocation->groupIndex);
    const auto &newGroup = currentData.groups.at(currentData.counterAllocation->groupIndex);
    if (oldGroup.minTargets != newGroup.minTargets || oldGroup.maxTargets != newGroup.maxTargets)
        return false;
    for (int i = 0; i < captured.selectedTargets.size(); ++i) {
        const auto &oldTarget = captured.selectedTargets.at(i);
        const auto &target = current.selectedTargets.at(i);
        const auto group = std::find_if(currentData.groups.cbegin(), currentData.groups.cend(),
                                       [&target](const auto &group) {
                                           return group.groupIndex == static_cast<int>(target.ref.group_index());
                                       });
        if (group == currentData.groups.cend() ||
            (target.ref.kind() == ruled::v1::TARGET_REF_KIND_PERMANENT &&
             !group->validPermanentIds.contains(target.ref.object_id())) ||
            (target.ref.kind() == ruled::v1::TARGET_REF_KIND_STACK &&
             !group->validStackIds.contains(target.ref.object_id())) ||
            (target.ref.kind() == ruled::v1::TARGET_REF_KIND_GRAVEYARD &&
             !group->validGraveyardIds.contains(target.ref.object_id())))
            return false;
        if (oldTarget.ref.SerializeAsString() != target.ref.SerializeAsString() ||
            oldTarget.zoneChangeGeneration != target.zoneChangeGeneration ||
            (target.ref.kind() == ruled::v1::TARGET_REF_KIND_PERMANENT &&
             (!state.battlefieldGenerationByOid.contains(target.ref.object_id()) ||
              state.battlefieldGenerationByOid.value(target.ref.object_id()) != target.zoneChangeGeneration)))
            return false;
    }
    return true;
}

RuledPendingCast::DamageAllocationStep RuledPendingCast::prepareSpellDamageAllocation()
{
    const int total = pendingDamageTargetsTotal();
    const int numTargets = spell.selectedTargetOids.size();
    // Automatic damage is calculated by the engine on resolution, so the submitted amounts
    // are zero and there is no interactive allocation or minimum of one damage per target.
    if (spell.damageDivision != ruled::v1::DAMAGE_DIVISION_CHOOSE_AT_CAST) {
        spell.selectedTargetDamages.clear();
        for (int i = 0; i < numTargets; ++i)
            spell.selectedTargetDamages.append(0);
    } else if (numTargets > total) {
        return DamageAllocationStep::Invalid;
    } else if (numTargets == 1) {
        spell.selectedTargetDamages.clear();
        spell.selectedTargetDamages.append(static_cast<quint32>(total));
    } else {
        spell.targetDamageAllocations.clear();
        for (int i = 0; i < numTargets; ++i)
            spell.targetDamageAllocations.append(1);
        spell.damageAllocationTotal = total;
        spell.inDamageAllocationMode = true;
        return DamageAllocationStep::Allocating;
    }
    return DamageAllocationStep::Ready;
}

RuledPendingCast::DamageAllocationChange RuledPendingCast::bumpSpellDamageAllocation(quint32 oid, int delta)
{
    if (!isInSpellDamageAllocationMode())
        return DamageAllocationChange::Unavailable;
    const int idx = spell.selectedTargetOids.indexOf(oid);
    if (idx < 0 || idx >= spell.targetDamageAllocations.size())
        return DamageAllocationChange::Unavailable;
    const int cur = spell.targetDamageAllocations.at(idx);
    const int total = spell.damageAllocationTotal;
    const int othersSum = spellDamageAllocationAssignedTotal() - cur;
    const int next = qBound(1, cur + delta, total - othersSum);
    if (next == cur)
        return DamageAllocationChange::Unchanged;
    spell.targetDamageAllocations[idx] = next;
    return DamageAllocationChange::Changed;
}

bool RuledPendingCast::confirmSpellDamageAllocation()
{
    if (!spellDamageAllocationIsLegal())
        return false;
    spell.selectedTargetDamages.clear();
    for (int v : spell.targetDamageAllocations)
        spell.selectedTargetDamages.append(static_cast<quint32>(v));
    spell.inDamageAllocationMode = false;
    return true;
}

int RuledPendingCast::pendingDamageTargetsTotal() const
{
    return spell.fixedDamage > 0 ? spell.fixedDamage : spell.xValue;
}

int RuledPendingCast::effectiveDamageTargetsMax() const
{
    if (!spell.isDamageTargets) {
        return spell.maxTargets;
    }
    // Automatic damage has no allocation minimum — Fireball may legally target more creatures
    // than X (they simply each take 0). Only the engine's own cap applies, if any.
    if (spell.damageDivision != ruled::v1::DAMAGE_DIVISION_CHOOSE_AT_CAST) {
        return spell.maxTargets;
    }
    const int total = pendingDamageTargetsTotal();
    // CR 601.2d: at least 1 damage per target caps the count at the total damage. Fire caps at
    // min(2, total).
    if (spell.maxTargets > 0) {
        return qMin(spell.maxTargets, total);
    }
    return total;
}

bool RuledPendingCast::isInSpellDamageAllocationMode() const
{
    return spell.valid && spell.inDamageAllocationMode;
}

bool RuledPendingCast::isSpellDamageAllocationDisplayActive() const
{
    return spell.valid && spell.isDamageTargets && !spell.selectedTargetOids.isEmpty();
}

int RuledPendingCast::spellDamageAllocationForOid(quint32 oid) const
{
    if (!isSpellDamageAllocationDisplayActive())
        return 0;
    const int idx = spell.selectedTargetOids.indexOf(oid);
    if (idx < 0)
        return 0;
    if (spell.damageDivision == ruled::v1::DAMAGE_DIVISION_PER_TARGET)
        return pendingDamageTargetsTotal();
    // While interactively allocating, show the in-progress split; once confirmed (and through
    // mana payment) show the amount that will actually be sent with the cast.
    if (spell.inDamageAllocationMode) {
        return idx < spell.targetDamageAllocations.size() ? spell.targetDamageAllocations.at(idx) : 0;
    }
    return idx < spell.selectedTargetDamages.size() ? static_cast<int>(spell.selectedTargetDamages.at(idx)) : 0;
}

int RuledPendingCast::spellDamageAllocationAssignedTotal() const
{
    int sum = 0;
    for (int v : spell.targetDamageAllocations)
        sum += v;
    return sum;
}

int RuledPendingCast::spellDamageAllocationMaxTotal() const
{
    return spell.damageAllocationTotal;
}

bool RuledPendingCast::spellDamageAllocationIsLegal() const
{
    return isInSpellDamageAllocationMode() && spellDamageAllocationAssignedTotal() == spell.damageAllocationTotal;
}

bool RuledPendingCast::isTargetSelectedForPendingSpell(quint32 oid) const
{
    if (!spell.valid) {
        return false;
    }
    return spell.selectedTargetOids.contains(oid) ||
           std::any_of(spell.selectedTargetOidsByGroup.cbegin(), spell.selectedTargetOidsByGroup.cend(),
                       [oid](const auto &group) { return group.contains(oid); });
}

bool RuledPendingCast::isTargetSelectedForPendingAbility(quint32 oid) const
{
    return ability.valid &&
           std::any_of(ability.selectedTargets.cbegin(), ability.selectedTargets.cend(), [oid](const auto &target) {
               return target.ref.kind() != ruled::v1::TARGET_REF_KIND_PLAYER && target.ref.object_id() == oid;
           });
}

bool RuledPendingCast::isPlayerSelectedAsPendingAbilityTarget(int playerId) const
{
    return ability.valid &&
           std::any_of(ability.selectedTargets.cbegin(), ability.selectedTargets.cend(), [playerId](const auto &target) {
               return target.ref.kind() == ruled::v1::TARGET_REF_KIND_PLAYER &&
                      target.ref.object_id() == static_cast<quint32>(playerId);
           });
}

bool RuledPendingCast::isCastCostPermanentSelected(quint32 oid) const
{
    return spell.valid &&
           std::any_of(spell.castCostSelections.cbegin(), spell.castCostSelections.cend(),
                       [oid](const auto &selection) {
                           return selection.objectKind == RuledPendingCastCostSelection::ObjectKind::Permanent &&
                                  (selection.selectedId == oid || selection.selectedObjectIds.contains(oid));
                       });
}

void RuledPendingCast::loadCurrentTargetGroup(const RuledClientState &state)
{
    const auto data = currentRuledSpellTargetData(spell, state);
    if (!data.has_value() || spell.activeTargetGroupPosition < 0 ||
        spell.activeTargetGroupPosition >= data->groups.size()) {
        return;
    }
    const auto &group = data->groups.at(spell.activeTargetGroupPosition);
    spell.minTargets = group.minTargets;
    spell.maxTargets = group.maxTargets;
    spell.selectedTargetOids = spell.selectedTargetOidsByGroup.value(spell.activeTargetGroupPosition);
    spell.selectedTargetDamages = spell.selectedTargetDamagesByGroup.value(spell.activeTargetGroupPosition);
}

bool RuledPendingCast::storeCurrentTargetGroupAndAdvance(const RuledClientState &state)
{
    const int current = spell.activeTargetGroupPosition;
    const auto data = currentRuledSpellTargetData(spell, state);
    if (!data.has_value() || current < 0 || current >= data->groups.size()) {
        return false;
    }
    while (spell.selectedTargetOidsByGroup.size() < data->groups.size()) {
        spell.selectedTargetOidsByGroup.append(QVector<quint32>{});
    }
    while (spell.selectedTargetDamagesByGroup.size() < data->groups.size()) {
        spell.selectedTargetDamagesByGroup.append(QVector<quint32>{});
    }
    spell.selectedTargetOidsByGroup[current] = spell.selectedTargetOids;
    spell.selectedTargetDamagesByGroup[current] = spell.selectedTargetDamages;

    if (current + 1 >= data->groups.size()) {
        return false;
    }
    spell.activeTargetGroupPosition = current + 1;
    loadCurrentTargetGroup(state);
    spell.waitingForTarget = true;
    return true;
}

bool RuledPendingCast::storeCurrentModalTargetsAndAdvance(const RuledClientState &state)
{
    const int current = spell.activeModePosition;
    if (current < 0 || current >= spell.selectedModes.size()) {
        return false;
    }
    auto &mode = spell.selectedModes[current];
    mode.selectedTargetOids = spell.selectedTargetOids;
    mode.selectedTargetDamages = spell.selectedTargetDamages;
    mode.selectedTargetOidsByGroup = spell.selectedTargetOidsByGroup;
    mode.selectedTargetDamagesByGroup = spell.selectedTargetDamagesByGroup;

    for (int next = current + 1; next < spell.selectedModes.size(); ++next) {
        const auto &nextMode = spell.selectedModes.at(next);
        if (!nextMode.needsTarget) {
            continue;
        }
        spell.activeModePosition = next;
        spell.activeTargetGroupPosition = 0;
        spell.selectedTargetOidsByGroup = nextMode.selectedTargetOidsByGroup;
        spell.selectedTargetDamagesByGroup = nextMode.selectedTargetDamagesByGroup;
        while (spell.selectedTargetOidsByGroup.size() < nextMode.targets.groups.size()) {
            spell.selectedTargetOidsByGroup.append(QVector<quint32>{});
        }
        while (spell.selectedTargetDamagesByGroup.size() < nextMode.targets.groups.size()) {
            spell.selectedTargetDamagesByGroup.append(QVector<quint32>{});
        }
        spell.isDamageTargets = nextMode.targets.isDamageTargets;
        spell.damageDivision = nextMode.targets.damageDivision;
        spell.fixedDamage = nextMode.targets.fixedDamage;
        spell.extraManaPerTarget = nextMode.targets.extraManaPerTarget;
        loadCurrentTargetGroup(state);
        spell.waitingForTarget = true;
        return true;
    }
    spell.activeModePosition = -1;
    return false;
}
