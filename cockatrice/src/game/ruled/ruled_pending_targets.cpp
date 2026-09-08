#include "ruled_pending_cast.h"

// Widget-independent local target staging. The engine continues to author legal choices.

RuledPendingCast::DamageAllocationStep RuledPendingCast::prepareSpellDamageAllocation()
{
    const int total = pendingDamageTargetsTotal();
    const int numTargets = spell.selectedTargetOids.size();
    // Evenly divided damage is calculated by the engine on resolution, so the submitted amounts
    // are zero and there is no interactive allocation or minimum of one damage per target.
    if (spell.damageDividedEvenly) {
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
    // "Divided evenly" has no per-target minimum — Fireball may legally target more creatures
    // than X (they simply each take 0). Only the engine's own cap applies, if any.
    if (spell.damageDividedEvenly) {
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
        spell.fixedDamage = nextMode.targets.fixedDamage;
        spell.extraManaPerTarget = nextMode.targets.extraManaPerTarget;
        loadCurrentTargetGroup(state);
        spell.waitingForTarget = true;
        return true;
    }
    spell.activeModePosition = -1;
    return false;
}
