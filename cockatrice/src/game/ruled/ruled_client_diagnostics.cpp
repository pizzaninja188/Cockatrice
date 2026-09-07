#include "ruled_client_diagnostics.h"

#include "../../client/settings/cache_settings.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include "../abstract_game.h"
#include "../board/card_item.h"
#include "../player/player.h"
#include "../player/player_actions.h"
#include "../player/player_info.h"
#include "../prompt/game_prompt_widget.h"
#include "../replay.h"
#include "ruled_actions.h"
#include "ruled_client_state.h"
#include "ruled_diagnostic_values.h"
#include "ruled_payment_ui.h"

#include <QAbstractButton>
#include <QApplication>
#include <QCryptographicHash>
#include <QGraphicsView>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QScreen>
#include <QTimer>
#include <QUuid>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pending_command.h>
#include <libcockatrice/protocol/ruled_diagnostic_journal.h>
#include <libcockatrice/protocol/ruled_diagnostics.h>

RuledClientDiagnostics::RuledClientDiagnostics(AbstractGame *game, QObject *parent) : QObject(parent), game(game)
{
    qApp->installEventFilter(this);
}
RuledClientDiagnostics::~RuledClientDiagnostics()
{
    if (journal)
        journal->finish();
}
bool RuledClientDiagnostics::ensure()
{
    if (!RuledActions::isRuledGame(game) || qobject_cast<Replay *>(game) ||
        qEnvironmentVariable("COCKATRICE_RULED_CAPTURE") == "0")
        return false;
    if (!journal) {
        const auto root = RuledDiagnosticJournal::defaultRoot("client");
        RuledDiagnosticJournal::prune(root);
        QJsonObject metadata{{"game_id", game->getGameMetaInfo()->gameId()},
                             {"build", RuledDiagnostics::buildInfo()},
                             {"local_player_id", game->getPlayerManager()->getLocalPlayerId()},
                             {"spectator", game->getPlayerManager()->isSpectator()},
                             {"game_info", RuledDiagnostics::decode(game->getGameMetaInfo()->proto())}};
        QFile database(SettingsCache::instance().getCardDatabasePath());
        QCryptographicHash hash(QCryptographicHash::Sha256);
        metadata.insert("display_database_sha256", database.open(QIODevice::ReadOnly) && hash.addData(&database)
                                                       ? QJsonValue(QString::fromLatin1(hash.result().toHex()))
                                                       : QJsonValue());
        if (auto *screen = game->getTab()->screen())
            metadata.insert("display", QJsonObject{{"logical_dpi", screen->logicalDotsPerInch()},
                                                   {"device_pixel_ratio", screen->devicePixelRatio()},
                                                   {"width", screen->size().width()},
                                                   {"height", screen->size().height()}});
        journal = std::make_unique<RuledDiagnosticJournal>(root, "client", "recipient_only", metadata);
        journal->record("client_game_info", game->getGameMetaInfo()->proto());
    }
    return journal->isHealthy();
}
QString RuledClientDiagnostics::directory() const
{
    return journal ? journal->directory() : QString();
}
QString RuledClientDiagnostics::error() const
{
    return journal ? journal->error() : tr("Capture is unavailable for this game.");
}
void RuledClientDiagnostics::received(const GameEventContainer &events)
{
    if (!ensure())
        return;
    QString correlation;
    for (const auto &event : events.event_list()) {
        if (!event.HasExtension(Event_RuledPayload::ext))
            continue;
        const auto &payload = event.GetExtension(Event_RuledPayload::ext);
        if (payload.has_diagnostic_context()) {
            const auto &context = payload.diagnostic_context();
            const auto serverId = QString::fromStdString(context.capture_id());
            if (!serverId.isEmpty() && serverId != lastServerCapture) {
                lastServerCapture = serverId;
                journal->metadata({{"server_capture_id", serverId}});
            }
            correlation = QString::fromStdString(context.client_request_id());
            journal->note("server_context", RuledDiagnostics::decode(context), correlation);
        }
    }
    journal->record("received_game_events", events, correlation);
}
void RuledClientDiagnostics::prepared(PendingCommand *command)
{
    if (!ensure())
        return;
    const auto id = QUuid::createUuid().toString(QUuid::WithoutBraces);
    auto &container = command->getCommandContainer();
    for (auto &gameCommand : *container.mutable_game_command()) {
        if (gameCommand.HasExtension(Command_RuledPayload::ext))
            gameCommand.MutableExtension(Command_RuledPayload::ext)->set_diagnostic_request_id(id.toStdString());
    }
    ++submitted;
    journal->record(
        "client_request", container, id,
        {{"authenticated_actor_unavailable", true}, {"local_player_id", game->getPlayerManager()->getLocalPlayerId()}});
    connect(command, &PendingCommand::finished, this,
            [this, id](const Response &response, const CommandContainer &, const QVariant &) {
                if (journal)
                    journal->record("client_response", response, id);
                QTimer::singleShot(0, this, [this, id] { snapshot(id); });
            });
}
void RuledClientDiagnostics::blocked(const google::protobuf::Message &command, const QString &reason)
{
    if (ensure())
        journal->record("client_command_blocked", command, {}, {{"reason", reason}});
}
QJsonObject RuledClientDiagnostics::currentState() const
{
    auto result = game->getGameEventHandler()->ruled()->diagnosticSnapshot();
    result.insert("local_player_id", game->getPlayerManager()->getLocalPlayerId());
    result.insert("active_player_id", game->getGameState()->getActivePlayer());
    result.insert("priority_player_id", game->getGameState()->getPriorityPlayer());
    result.insert("toolbar_phase", game->getGameState()->getCurrentPhase());
    QJsonArray prompts;
    for (const auto *prompt : game->getTab()->findChildren<GamePromptWidget *>())
        prompts.append(prompt->diagnosticSnapshot());
    result.insert("prompts", prompts);
    QJsonArray cards;
    QJsonArray transactions;
    for (const auto *player : game->getPlayerManager()->getPlayers()) {
        const auto *actions = player->getPlayerActions();
        if (actions && actions->ruledPendingCast) {
            transactions.append(QJsonObject{
                {"player_id", player->getPlayerInfo()->getId()},
                {"spell", RuledDiagnosticValues::value(actions->ruledPendingCast->spell)},
                {"ability", RuledDiagnosticValues::value(actions->ruledPendingCast->ability)},
                {"payment_ui",
                 actions->ruledPayment ? QJsonValue(actions->ruledPayment->diagnosticSnapshot()) : QJsonValue()}});
        }
        for (const auto *zone : player->getZones()) {
            for (const auto *card : zone->getCards()) {
                const auto owner = player->getPlayerInfo()->getId();
                const auto *state = game->getGameEventHandler()->ruled();
                const auto oid =
                    state->ownerCardIdToEngineOid.value(RuledClientState::makeOwnedCardKey(owner, card->getId()));
                cards.append(QJsonObject{{"owner_player_id", owner},
                                         {"server_card_id", card->getId()},
                                         {"zone", zone->getName()},
                                         {"known_name", card->getName()},
                                         {"engine_object_id", oid ? QJsonValue(qint64(oid)) : QJsonValue()},
                                         {"selected", card->isSelected()},
                                         {"visible", card->isVisible()},
                                         {"x", card->pos().x()},
                                         {"y", card->pos().y()}});
            }
        }
    }
    result.insert("physical_cards", cards);
    result.insert("local_transactions", transactions);
    return result;
}
void RuledClientDiagnostics::snapshot(const QString &correlation)
{
    if (ensure())
        journal->state("client", currentState(), correlation);
}
QString RuledClientDiagnostics::markReport(const QString &reportId)
{
    ensure();
    if (!journal)
        return {};
    journal->markReport(reportId);
    snapshot(reportId);
    Command_RuledPayload marker;
    marker.set_diagnostic_report_id(reportId.toStdString());
    game->getGameEventHandler()->sendGameCommand(marker);
    return lastServerCapture;
}
bool RuledClientDiagnostics::eventFilter(QObject *watched, QEvent *event)
{
    if (event->type() != QEvent::MouseButtonPress && event->type() != QEvent::MouseButtonRelease &&
        event->type() != QEvent::KeyPress)
        return false;
    auto *widget = qobject_cast<QWidget *>(watched);
    auto *tab = game->getTab();
    if (!widget || (widget != tab && !tab->isAncestorOf(widget)) || !ensure())
        return false;
    QJsonObject action{{"widget_class", widget->metaObject()->className()},
                       {"widget_name", widget->objectName()},
                       {"enabled", widget->isEnabled()},
                       {"event", event->type() == QEvent::KeyPress           ? "key_press"
                                 : event->type() == QEvent::MouseButtonPress ? "mouse_press"
                                                                             : "mouse_release"}};
    if (auto *button = qobject_cast<QAbstractButton *>(widget))
        action.insert("label", button->text());
    if (event->type() == QEvent::KeyPress) {
        const auto *key = static_cast<QKeyEvent *>(event);
        // Only navigation/action keys. Text entry (chat, search, passwords) is not a keystroke log.
        if (!key->text().isEmpty() && key->key() != Qt::Key_Return && key->key() != Qt::Key_Enter &&
            key->key() != Qt::Key_Space && key->key() != Qt::Key_Escape)
            return false;
        action.insert("key", QKeySequence(key->key()).toString());
        action.insert("modifiers", int(key->modifiers()));
    } else {
        const auto *mouse = static_cast<QMouseEvent *>(event);
        action.insert("x", mouse->position().x());
        action.insert("y", mouse->position().y());
        action.insert("button", int(mouse->button()));
        if (auto *view = qobject_cast<QGraphicsView *>(widget->parentWidget())) {
            auto *item = view->itemAt(mouse->position().toPoint());
            while (item && !dynamic_cast<CardItem *>(item))
                item = item->parentItem();
            if (auto *card = dynamic_cast<CardItem *>(item)) {
                const auto owner = card->getOwner()->getPlayerInfo()->getId();
                action.insert("server_card_id", card->getId());
                action.insert("owner_player_id", owner);
                action.insert("known_name", card->getName());
                action.insert("engine_object_id",
                              qint64(game->getGameEventHandler()->ruled()->ownerCardIdToEngineOid.value(
                                  RuledClientState::makeOwnedCardKey(owner, card->getId()))));
            }
        }
    }
    const auto id = QString("input-%1").arg(journal->sequence() + 1);
    journal->note("ui_input", action, id);
    const auto before = submitted;
    QTimer::singleShot(0, this, [this, before, id] {
        if (!journal)
            return;
        journal->note("ui_input_result",
                      {{"commands_prepared", QString::number(submitted - before)},
                       {"disposition", submitted == before ? "no_command_observed" : "command_prepared"}},
                      id);
        snapshot(id);
    });
    return false;
}
