#include "rules_relay.h"

#include <QDebug>
#include <QHostAddress>
#include <QtEndian>
#include <QTcpSocket>
#include <google/protobuf/message.h>

#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

RulesRelay::RulesRelay(QObject *parent) : QObject(parent), socket(new QTcpSocket(this))
{
}

RulesRelay::~RulesRelay()
{
    disconnectRelay();
}

QString RulesRelay::engineHost() const
{
    const QByteArray env = qgetenv("TRICERULES_HOST");
    return env.isEmpty() ? QStringLiteral("127.0.0.1") : QString::fromLocal8Bit(env);
}

quint16 RulesRelay::enginePort() const
{
    const QByteArray env = qgetenv("TRICERULES_PORT");
    bool ok = false;
    const int p = env.isEmpty() ? 17381 : QString::fromLocal8Bit(env).toInt(&ok);
    return ok && p > 0 && p < 65536 ? static_cast<quint16>(p) : static_cast<quint16>(17381);
}

bool RulesRelay::connectIfNeeded()
{
    if (socket->state() == QAbstractSocket::ConnectedState) {
        return true;
    }
    socket->connectToHost(QHostAddress(engineHost()), enginePort());
    if (!socket->waitForConnected(3000)) {
        qWarning() << "RulesRelay: failed to connect to tricerules-server:" << socket->errorString();
        return false;
    }
    return true;
}

void RulesRelay::disconnectRelay()
{
    if (socket->state() == QAbstractSocket::ConnectedState) {
        ruled::v1::IpcEnvelope endEnv;
        endEnv.mutable_session_end();
        (void)writeFrame(endEnv);
    }
    socket->abort();
}

bool RulesRelay::writeFrame(const google::protobuf::Message &msg)
{
    std::string data;
    if (!msg.SerializeToString(&data)) {
        return false;
    }
    const quint32 len = qToBigEndian<quint32>(static_cast<quint32>(data.size()));
    if (socket->write(reinterpret_cast<const char *>(&len), sizeof(len)) != sizeof(len)) {
        return false;
    }
    if (socket->write(data.data(), static_cast<qint64>(data.size())) != static_cast<qint64>(data.size())) {
        return false;
    }
    if (!socket->waitForBytesWritten(3000)) {
        return false;
    }
    return true;
}

bool RulesRelay::readFrame(QByteArray &out)
{
    // QIODevice::read may return fewer than requested; always loop until the length prefix
    // and the payload are complete. A partial length read produced truncated frames and
    // short garbage lib_ids_csv.
    char lenLeRaw[4];
    {
        int got = 0;
        while (got < 4) {
            if (socket->bytesAvailable() <= 0 && !socket->waitForReadyRead(5000)) {
                return false;
            }
            const qint64 n = socket->read(lenLeRaw + got, 4 - got);
            if (n <= 0) {
                return false;
            }
            got += static_cast<int>(n);
        }
    }
    const quint32 len = (static_cast<quint32>(static_cast<unsigned char>(lenLeRaw[0])) << 24) |
                        (static_cast<quint32>(static_cast<unsigned char>(lenLeRaw[1])) << 16) |
                        (static_cast<quint32>(static_cast<unsigned char>(lenLeRaw[2])) << 8) |
                        static_cast<quint32>(static_cast<unsigned char>(lenLeRaw[3]));
    // Sane cap (full zone sync for two 60-card decks is a few KB).
    if (len == 0 || len > 16U * 1024U * 1024U) {
        return false;
    }
    out.resize(static_cast<int>(len));
    qint64 got = 0;
    while (got < static_cast<qint64>(len)) {
        if (socket->bytesAvailable() <= 0 && !socket->waitForReadyRead(5000)) {
            return false;
        }
        const qint64 n = socket->read(out.data() + got, static_cast<qint64>(len) - got);
        if (n <= 0) {
            return false;
        }
        got += n;
    }
    return true;
}

bool RulesRelay::sessionStart(quint64 gameId, quint64 seed, const QList<int> &playerIds,
                              const QList<QPair<int, QStringList>> *playerDecks, ruled::v1::IpcResponse &out)
{
    if (!connectIfNeeded()) {
        return false;
    }
    ruled::v1::IpcEnvelope env;
    ruled::v1::SessionStart *ss = env.mutable_session_start();
    ss->set_game_id(gameId);
    ss->set_seed(seed);
    for (int pid : playerIds) {
        ss->add_player_ids(pid);
    }
    if (playerDecks) {
        for (const QPair<int, QStringList> &row : *playerDecks) {
            ruled::v1::PlayerDeck *pd = ss->add_player_decks();
            pd->set_player_id(row.first);
            for (const QString &cid : row.second) {
                pd->add_mainboard_card_id(cid.toStdString());
            }
        }
    }
    if (!writeFrame(env)) {
        return false;
    }
    QByteArray frame;
    if (!readFrame(frame)) {
        return false;
    }
    return out.ParseFromArray(frame.constData(), frame.size());
}

bool RulesRelay::playerCommand(int playerId, const QByteArray &ruledCommandBytes, ruled::v1::IpcResponse &out)
{
    if (!connectIfNeeded()) {
        return false;
    }
    ruled::v1::IpcEnvelope env;
    ruled::v1::PlayerCommand *pc = env.mutable_player_command();
    pc->set_player_id(playerId);
    pc->set_ruled_command(ruledCommandBytes.data(), static_cast<int>(ruledCommandBytes.size()));
    if (!writeFrame(env)) {
        return false;
    }
    QByteArray frame;
    if (!readFrame(frame)) {
        return false;
    }
    return out.ParseFromArray(frame.constData(), frame.size());
}

bool RulesRelay::sessionEnd()
{
    if (socket->state() != QAbstractSocket::ConnectedState) {
        return true;
    }
    ruled::v1::IpcEnvelope env;
    env.mutable_session_end();
    return writeFrame(env);
}
