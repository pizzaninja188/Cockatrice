#ifndef RULED_GAME_SESSION_H
#define RULED_GAME_SESSION_H

// Fork-owned synchronous lifecycle for one tricerules sidecar session. RuledGameDriver remains
// the public Server_Game facade; this collaborator owns transport, deck admission, canonical
// command envelopes, and per-session policy state without touching physical card identity.

#include <QByteArray>
#include <QHash>
#include <QList>
#include <QPair>
#include <QString>
#include <QStringList>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <memory>

class RulesRelay;
class Server_Game;

class RuledGameSession
{
    friend class RuledBatchTest;

public:
    enum class StartDisposition
    {
        Blocked,
        Fallback,
        Started
    };

    struct StartResult
    {
        StartDisposition disposition = StartDisposition::Blocked;
        ruled::v1::IpcResponse response;
        QList<QPair<int, QStringList>> deckByPlayer;
        quint64 seed = 0;
        QString cardDataHash;
    };

    explicit RuledGameSession(Server_Game *game);
    ~RuledGameSession();

    bool validateDecksForStart();
    StartResult start();
    void resetForNewGame();
    void end();
    void abort();

    [[nodiscard]] bool isActive() const;
    bool playerCommand(int playerId, const QByteArray &payload, ruled::v1::IpcResponse &response);
    void handleConnectionLost();

    bool cacheAutoPassPolicy(int playerId, const ruled::v1::SetAutoPassPolicy &policy);
    [[nodiscard]] QByteArray canonicalGameplayCommand(int playerId, const ruled::v1::RuledCommand &command) const;

private:
    [[nodiscard]] QList<QPair<int, QStringList>> mainboardNamesByPlayer() const;
    void notifyUnimplementedCards(const QList<QPair<int, QStringList>> &deckByPlayer, const QStringList &missingNames);
    void sendEngineNotice(const QString &title, const QString &message);
    void notifyEngineUnreachable();

    Server_Game *const game;
    std::unique_ptr<RulesRelay> relay;
    quint64 seed = 0;
    bool engineConnectionLost = false;
    QHash<int, ruled::v1::SetAutoPassPolicy> autoPassPolicies;
};

#endif
