#include "ruled_client_state.h"

#include "ruled_client_host.h"

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
