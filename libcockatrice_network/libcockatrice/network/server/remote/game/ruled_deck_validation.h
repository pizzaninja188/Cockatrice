#pragma once

#include <QString>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

class DeckList;
class Server_AbstractPlayer;
class ResponseContainer;

namespace RuledDeckValidation
{
// One effective deck ordering for construction validation and engine session creation. Physical
// setup applies the same ordered one-copy sideboard moves; tokens/maybeboard never enter play.
bool normalize(const DeckList &deck, int playerId, ruled::v1::PlayerDeck &out, QString &error);
ruled::v1::IpcResponse query(const DeckList &deck, int playerId, ruled::v1::GameFormat format);
bool admit(Server_AbstractPlayer *player,
           const DeckList &deck,
           ResponseContainer &response,
           ruled::v1::DeckValidationReport *acceptedReport = nullptr);
void notify(Server_AbstractPlayer *player, const ruled::v1::DeckValidationReport &report);
} // namespace RuledDeckValidation
