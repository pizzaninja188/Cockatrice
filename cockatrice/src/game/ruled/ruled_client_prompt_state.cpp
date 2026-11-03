#include "ruled_client_state.h"

#include "ruled_client_host.h"

#include <algorithm>
#include <tuple>

bool RuledClientState::isCurrentResolutionPrompt(quint64 promptId) const
{
    return promptId != 0 && resolutionPromptSnapshotValid && resolutionPromptActive &&
           resolutionPromptDeciderPlayerId == host->localPlayerId() && resolutionPromptId == promptId;
}

bool RuledClientState::hasActiveLocalResolutionPrompt() const
{
    return resolutionPromptSnapshotValid && resolutionPromptActive &&
           resolutionPromptDeciderPlayerId == host->localPlayerId();
}

// Prompt and source queries also serve the independently linked cast-menu surface.

const RuledHandActionSet &RuledClientState::handActionSet(RuledHandActionKind kind) const
{
    static const RuledHandActionSet empty;
    const auto it = handActions.constFind(kind);
    return it == handActions.constEnd() ? empty : *it;
}


QVector<RuledFaceOption> RuledClientState::handActionFaceOptions(RuledHandActionKind kind, int handIndex) const
{
    QVector<RuledFaceOption> options = handActionSet(kind).faceOptionsByIndex.value(handIndex);
    std::sort(options.begin(), options.end(), [](const RuledFaceOption &a, const RuledFaceOption &b) {
        return std::tie(a.faceIndex, a.castMethod, a.prototyped) <
               std::tie(b.faceIndex, b.castMethod, b.prototyped);
    });
    return options;
}


std::optional<RuledClientState::SpecialCastSource>
RuledClientState::specialCastSourceForObject(quint32 oid) const
{
    if (!hasPendingChoiceOfKind(ChoiceKind::SpecialCast) || !pendingChoice->candidateOids.contains(oid))
        return std::nullopt;
    const auto source = pendingChoice->specialCastSourceByOid.value(oid, RuledCastSource::Exile);
    if (source == RuledCastSource::Hand) {
        const int serverCardId = pendingChoice->specialCastHandCardIds.value(oid, -1);
        const int handSlot = serverCardId < 0 ? -1 : engineHandSlotForServerCard(host->localPlayerId(), serverCardId);
        if (handSlot < 0)
            return std::nullopt;
        return SpecialCastSource{source, handSlot};
    }
    return SpecialCastSource{source, static_cast<int>(oid)};
}


quint32 RuledClientState::specialCastObjectForSource(int sourceIndex, RuledCastSource source) const
{
    if (!hasPendingChoiceOfKind(ChoiceKind::SpecialCast))
        return 0;
    for (const auto oid : pendingChoice->candidateOids) {
        const auto binding = specialCastSourceForObject(oid);
        if (binding && binding->source == source && binding->sourceIndex == sourceIndex)
            return oid;
    }
    return 0;
}
