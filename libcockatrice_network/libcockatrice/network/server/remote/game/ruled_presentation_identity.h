#ifndef RULED_PRESENTATION_IDENTITY_H
#define RULED_PRESENTATION_IDENTITY_H

// The relay's client-facing object handles. The engine, physical synchronizer and replay
// continue to use canonical ObjectIds. A handle is never reused, even after it is retired.
#include <QHash>
#include <QSet>
#include <QString>
#include <QVector>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <optional>

class Server_Game;
namespace google::protobuf { class Message; }

class RuledPresentationIdentity
{
public:
    explicit RuledPresentationIdentity(const Server_Game *game);
    void reset();
    bool beginBatch(const ruled::v1::RuledEventBatch &batch);
    bool project(ruled::v1::RuledEventBatch &batch, int recipient);
    bool projectPayment(ruled::v1::PaymentPreview &preview, int recipient);
    bool canonicalize(ruled::v1::RuledCommand &command, int recipient,
                      const ruled::v1::ResolutionChoiceRequired *choice);

private:
    struct Epoch {
        quint64 firstGeneration = 0;
        quint32 handle = 0;
        quint64 generationMask = 0;
    };
    struct Handle {
        quint32 object = 0;
        int epoch = 0;
    };
    struct SeenIncarnation {
        int zone = 0;
        quint64 generation = 0;
    };
    struct Walk {
        int recipient = -1;
        bool inbound = false;
        const ruled::v1::ResolutionChoiceRequired *choice = nullptr;
        QHash<quint32, int> epochByObject;
        QSet<const google::protobuf::Message *> translatedTargets;
        QSet<quint32> pendingReceipts;
    };

    bool walk(google::protobuf::Message *message, Walk &context);
    bool object(quint32 &value, Walk &context, std::optional<quint64> generation = std::nullopt);
    bool objectAndGeneration(quint32 &object, quint64 &generation, Walk &context);
    bool startEpoch(quint32 object, quint64 generation);
    bool ensureObject(quint32 object);
    bool ensureProtectedObject(quint32 object);
    quint32 allocateHandle();

    const Server_Game *game;
    QHash<quint32, QVector<Epoch>> epochs;
    QHash<quint32, Handle> handles;
    QSet<quint32> usedHandles;
    QHash<int, QSet<quint32>> issued;
    QHash<quint32, int> batchStartEpochs;
    QHash<quint32, SeenIncarnation> visibleIncarnations;
    QHash<quint32, quint64> faceDownExiles;
    QHash<QString, QString> revealOccurrences;
};

#endif
