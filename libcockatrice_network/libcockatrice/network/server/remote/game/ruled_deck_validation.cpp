#include "ruled_deck_validation.h"

#include "../server_abstractuserinterface.h"
#include "../server_response_containers.h"
#include "rules_relay.h"
#include "server_abstract_player.h"
#include "server_game.h"

#include <QStringList>
#include <libcockatrice/deck_list/deck_list.h>
#include <libcockatrice/deck_list/tree/deck_list_card_node.h>
#include <libcockatrice/protocol/pb/event_notify_user.pb.h>
#include <libcockatrice/protocol/pb/response_deck_download.pb.h>
#include <libcockatrice/protocol/pb/session_event.pb.h>
#include <libcockatrice/protocol/ruled_deck_validation_report.h>

namespace RuledDeckValidation
{
bool normalize(const DeckList &deck, int playerId, ruled::v1::PlayerDeck &out, QString &error)
{
    QStringList main, side;
    int total = deck.getCommanders().size();
    for (const auto &zone : {QStringLiteral(DECK_ZONE_MAIN), QStringLiteral(DECK_ZONE_SIDE)}) {
        auto &names = zone == DECK_ZONE_MAIN ? main : side;
        for (const auto *node : deck.getCardNodes({zone})) {
            if (node->getNumber() < 1 || node->getNumber() > 10000 - total || node->getName().trimmed().isEmpty()) {
                error = QStringLiteral("Invalid card quantity or empty card name (maximum 10,000 cards)");
                return false;
            }
            total += node->getNumber();
            for (int copy = 0; copy < node->getNumber(); ++copy) {
                names.append(node->getName());
            }
        }
    }
    for (const auto *zone : deck.getZoneNodes()) {
        if (zone->getName() != DECK_ZONE_MAIN && zone->getName() != DECK_ZONE_SIDE &&
            zone->getName() != DECK_ZONE_TOKENS && zone->getName() != DECK_ZONE_MAYBEBOARD) {
            error = QStringLiteral("Unsupported deck section: %1").arg(zone->getName());
            return false;
        }
    }
    if (total > 10000 || deck.getCurrentSideboardPlan().size() > 10000) {
        error = QStringLiteral("Deck or sideboard plan exceeds the validation limit");
        return false;
    }
    for (const auto &move : deck.getCurrentSideboardPlan()) {
        const auto start = QString::fromStdString(move.start_zone());
        const auto target = QString::fromStdString(move.target_zone());
        if (!((start == DECK_ZONE_MAIN && target == DECK_ZONE_SIDE) ||
              (start == DECK_ZONE_SIDE && target == DECK_ZONE_MAIN))) {
            error = QStringLiteral("Sideboard plans may move cards only between mainboard and sideboard");
            return false;
        }
        auto &from = start == DECK_ZONE_MAIN ? main : side;
        auto &to = target == DECK_ZONE_MAIN ? main : side;
        const auto index = from.indexOf(QString::fromStdString(move.card_name()));
        if (index < 0) {
            error = QStringLiteral("Sideboard move references a card absent from its source");
            return false;
        }
        to.append(from.takeAt(index));
    }
    out.Clear();
    out.set_player_id(playerId);
    for (const auto &name : main) {
        out.add_mainboard_card_name(name.trimmed().toStdString());
    }
    for (const auto &name : side) {
        out.add_sideboard_card_name(name.trimmed().toStdString());
    }
    auto *setup = out.mutable_commander_setup();
    if (deck.getCommanders().isEmpty()) {
        setup->set_no_commander(true);
    } else {
        for (const auto &commander : deck.getCommanders()) {
            setup->mutable_declared()->add_card_name(commander.name.trimmed().toStdString());
        }
    }
    return true;
}

ruled::v1::IpcResponse query(const DeckList &deck, int playerId, ruled::v1::GameFormat format)
{
    ruled::v1::PlayerDeck normalized;
    ruled::v1::IpcResponse result;
    QString error;
    if (normalize(deck, playerId, normalized, error)) {
        RulesRelay relay;
        if (relay.validateDeck(normalized, format, result) && result.has_deck_validation()) {
            return result;
        }
        error = QStringLiteral(
            "The rules engine did not return a format validation report. Check that the matching sidecar is running.");
    }
    result.Clear();
    result.set_error(error.toStdString());
    auto *report = result.mutable_deck_validation();
    report->set_format(format);
    report->set_legality(ruled::v1::DECK_LEGALITY_UNKNOWN);
    auto *issue = report->add_issues();
    issue->set_code("validation_unavailable");
    issue->set_message(error.toStdString());
    return result;
}

void notify(Server_AbstractPlayer *player, const ruled::v1::DeckValidationReport &report)
{
    if (auto *ui = player->getUserInterface()) {
        Event_NotifyUser notify;
        notify.set_type(Event_NotifyUser::CUSTOM);
        notify.set_custom_title("Deck validation");
        notify.set_custom_content(RuledDeckValidationReport::describe(report).toStdString());
        auto *event = Server_AbstractUserInterface::prepareSessionEvent(notify);
        ui->sendProtocolItem(*event);
        delete event;
    }
}

bool admit(Server_AbstractPlayer *player,
           const DeckList &deck,
           ResponseContainer &response,
           ruled::v1::DeckValidationReport *acceptedReport)
{
    // A dedicated local resume server reconstructs only the already accepted, verified plan.
    if (!qEnvironmentVariable("COCKATRICE_RULED_RESUME_PLAN").isEmpty()) {
        return true;
    }
    if (!player->getGame()->getRuledGame() || player->getGame()->getFormat() == ruled::v1::GAME_FORMAT_UNSPECIFIED) {
        return true;
    }
    const auto result = query(deck, player->getPlayerId(), player->getGame()->getFormat());
    if (result.ok() && result.deck_validation().legality() == ruled::v1::DECK_LEGALITY_LEGAL &&
        result.deck_validation().engine_supported()) {
        if (acceptedReport) {
            *acceptedReport = result.deck_validation();
        }
        return true;
    }
    auto *reply = new Response_RuledDeckValidation;
    *reply->mutable_validation() = result.deck_validation();
    response.setResponseExtension(reply);
    notify(player, result.deck_validation());
    return false;
}
} // namespace RuledDeckValidation
