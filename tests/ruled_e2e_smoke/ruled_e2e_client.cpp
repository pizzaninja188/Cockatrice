#include "ruled_e2e_client.h"
namespace ruled_e2e
{
void SmokeClient::log(const QString &line)
{
    transcript->append(QStringLiteral("[%1] %2").arg(userName, line));
}

bool SmokeClient::connectToServer()
{
    sock.connectToHost(QStringLiteral("127.0.0.1"), static_cast<quint16>(kServatricePort));
    if (!sock.waitForConnected(10000)) {
        return false;
    }
    // TCP handshake: the server treats the first CommandContainer *without* a cmd_id as the
    // session-init trigger (see TcpServerSocketInterface::readClient) and silently swallows
    // everything until it arrives.
    CommandContainer hello;
    std::string bytes;
    hello.SerializeToString(&bytes);
    writeFrame(bytes);
    return true;
}

void SmokeClient::writeFrame(const std::string &bytes)
{
    QByteArray frame;
    const quint32 len = static_cast<quint32>(bytes.size());
    frame.append(static_cast<char>((len >> 24) & 0xff));
    frame.append(static_cast<char>((len >> 16) & 0xff));
    frame.append(static_cast<char>((len >> 8) & 0xff));
    frame.append(static_cast<char>(len & 0xff));
    frame.append(bytes.data(), static_cast<int>(bytes.size()));
    sock.write(frame);
    sock.flush();
}

void SmokeClient::sendContainer(CommandContainer &cont)
{
    cont.set_cmd_id(nextCmdId++);
    std::string bytes;
    cont.SerializeToString(&bytes);
    writeFrame(bytes);
}

bool SmokeClient::pump(int waitMs)
{
    if (sock.state() != QAbstractSocket::ConnectedState) {
        return false;
    }
    if (sock.bytesAvailable() == 0) {
        sock.waitForReadyRead(waitMs);
    }
    bool processed = false;
    inBuf.append(sock.readAll());
    for (;;) {
        if (!sawHandshakeGarbage && inBuf.size() >= 4 && inBuf.startsWith("<?xm")) {
            // v14 compatibility preamble some server builds emit; skip its 60 bytes.
            if (inBuf.size() < 60) {
                break;
            }
            inBuf.remove(0, 60);
            sawHandshakeGarbage = true;
            continue;
        }
        if (inBuf.size() < 4) {
            break;
        }
        const quint32 len = (static_cast<quint32>(static_cast<unsigned char>(inBuf[0])) << 24) +
                            (static_cast<quint32>(static_cast<unsigned char>(inBuf[1])) << 16) +
                            (static_cast<quint32>(static_cast<unsigned char>(inBuf[2])) << 8) +
                            static_cast<quint32>(static_cast<unsigned char>(inBuf[3]));
        if (static_cast<quint32>(inBuf.size()) < 4 + len) {
            break;
        }
        ServerMessage msg;
        const bool ok = msg.ParseFromArray(inBuf.constData() + 4, static_cast<int>(len));
        inBuf.remove(0, static_cast<int>(4 + len));
        if (ok) {
            handleServerMessage(msg);
            processed = true;
        }
    }
    return processed;
}

void SmokeClient::handleServerMessage(const ServerMessage &msg)
{
    switch (msg.message_type()) {
        case ServerMessage::RESPONSE:
            responses[msg.response().cmd_id()] = msg.response();
            if (msg.response().response_code() != Response::RespOk && ruledCmdIds.count(msg.response().cmd_id()) > 0) {
                // A rejected ruled command means the policy misjudged legality; the
                // transcript makes the wedge diagnosable if the game then stalls.
                log(QStringLiteral("!! ruled command %1 rejected with code %2")
                        .arg(msg.response().cmd_id())
                        .arg(msg.response().response_code()));
            }
            onResponse(msg.response());
            break;
        case ServerMessage::SESSION_EVENT:
            handleSessionEvent(msg.session_event());
            break;
        case ServerMessage::GAME_EVENT_CONTAINER:
            handleGameEventContainer(msg.game_event_container());
            break;
        case ServerMessage::ROOM_EVENT:
            handleRoomEvent(msg.room_event());
            break;
        default:
            break;
    }
}

void SmokeClient::handleSessionEvent(const SessionEvent &ev)
{
    if (ev.HasExtension(Event_ListRooms::ext)) {
        const auto &lr = ev.GetExtension(Event_ListRooms::ext);
        if (roomId < 0 && lr.room_list_size() > 0) {
            roomId = lr.room_list(0).room_id();
        }
    }
    if (ev.HasExtension(Event_GameJoined::ext)) {
        const auto &gj = ev.GetExtension(Event_GameJoined::ext);
        gameId = gj.game_info().game_id();
        myId = gj.player_id();
        log(QStringLiteral("joined game %1 as player %2").arg(gameId).arg(myId));
    }
    if (ev.HasExtension(Event_NotifyUser::ext)) {
        const auto &nu = ev.GetExtension(Event_NotifyUser::ext);
        if (nu.type() == Event_NotifyUser::CUSTOM) {
            ++notifyCustomCount;
            lastNotifyContent = QString::fromStdString(nu.custom_content());
            log(QStringLiteral("NotifyUser CUSTOM: %1").arg(lastNotifyContent.left(120)));
        }
    }
}

void SmokeClient::handleRoomEvent(const RoomEvent &ev)
{
    if (ev.HasExtension(Event_ListGames::ext)) {
        const auto &lg = ev.GetExtension(Event_ListGames::ext);
        for (int i = 0; i < lg.game_list_size(); ++i) {
            announcedGames.push_back(lg.game_list(i));
        }
    }
}

::testing::AssertionResult SmokeClient::loginAndJoinRoom()
{
    if (!connectToServer()) {
        return ::testing::AssertionFailure() << "could not connect to servatrice";
    }
    CommandContainer cont;
    auto *login = cont.add_session_command()->MutableExtension(Command_Login::ext);
    login->set_user_name(userName.toStdString());
    login->set_password("");
    login->set_clientver("ruled-e2e-smoke");
    const quint64 loginId = nextCmdId;
    sendContainer(cont);
    auto r = pumpUntil([&] { return responses.count(loginId) > 0; }, 10000, "login response");
    if (!r) {
        return r;
    }
    if (responses[loginId].response_code() != Response::RespOk) {
        return ::testing::AssertionFailure() << "login failed with code " << responses[loginId].response_code();
    }
    CommandContainer listCont;
    listCont.add_session_command()->MutableExtension(Command_ListRooms::ext);
    sendContainer(listCont);
    r = pumpUntil([&] { return roomId >= 0; }, 10000, "room list");
    if (!r) {
        return r;
    }
    CommandContainer joinCont;
    auto *join = joinCont.add_session_command()->MutableExtension(Command_JoinRoom::ext);
    join->set_room_id(roomId);
    const quint64 joinId = nextCmdId;
    sendContainer(joinCont);
    r = pumpUntil([&] { return responses.count(joinId) > 0; }, 10000, "join room response");
    if (!r) {
        return r;
    }
    if (responses[joinId].response_code() != Response::RespOk) {
        return ::testing::AssertionFailure() << "join room failed with code " << responses[joinId].response_code();
    }
    return ::testing::AssertionSuccess();
}

::testing::AssertionResult SmokeClient::createRuledGame()
{
    CommandContainer cont;
    cont.set_room_id(roomId);
    auto *create = cont.add_room_command()->MutableExtension(Command_CreateGame::ext);
    create->set_description("ruled e2e smoke");
    create->set_max_players(2);
    create->set_spectators_allowed(false);
    create->set_starting_life_total(20);
    create->set_ruled_game(true);
    sendContainer(cont);
    return pumpUntil([&] { return gameId >= 0 && myId >= 0; }, 10000, "game created/joined");
}

::testing::AssertionResult SmokeClient::joinRuledGame(int targetGameId)
{
    CommandContainer cont;
    cont.set_room_id(roomId);
    auto *join = cont.add_room_command()->MutableExtension(Command_JoinGame::ext);
    join->set_game_id(targetGameId);
    join->set_spectator(false);
    sendContainer(cont);
    return pumpUntil([&] { return gameId == targetGameId && myId >= 0; }, 10000, "game joined");
}

::testing::AssertionResult SmokeClient::selectDeck(const QString &xml)
{
    CommandContainer cont;
    cont.set_game_id(gameId);
    auto *sel = cont.add_game_command()->MutableExtension(Command_DeckSelect::ext);
    sel->set_deck(xml.toStdString());
    const quint64 id = nextCmdId;
    sendContainer(cont);
    auto r = pumpUntil([&] { return responses.count(id) > 0; }, 10000, "deck select response");
    if (!r) {
        return r;
    }
    if (responses[id].response_code() != Response::RespOk) {
        return ::testing::AssertionFailure() << "deck select failed with code " << responses[id].response_code();
    }
    return ::testing::AssertionSuccess();
}

void SmokeClient::sendReady()
{
    CommandContainer cont;
    cont.set_game_id(gameId);
    auto *ready = cont.add_game_command()->MutableExtension(Command_ReadyStart::ext);
    ready->set_ready(true);
    sendContainer(cont);
}

void SmokeClient::sendRuled(const ruled::v1::RuledCommand &cmd, const QString &what)
{
    if (!sendingTranslatedCastCommit && (legacyCastCommit.has_value() || translatedCastCommitInFlight)) {
        commandsAfterTranslatedCast.emplace_back(cmd, what);
        return;
    }
    CommandContainer cont;
    cont.set_game_id(gameId);
    std::string bytes;
    ruled::v1::RuledCommand explicitCommand = cmd;
    if (explicitCommand.has_cast_spell() &&
        explicitCommand.cast_spell().cast_method() == ruled::v1::CAST_METHOD_UNSPECIFIED) {
        explicitCommand.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    }
    const auto translateCast = [this](const ruled::v1::CastSpell &cast,
                                      ruled::v1::SpellCastAnnouncement *announcement) {
        announcement->mutable_targets()->CopyFrom(cast.targets());
        announcement->set_x_value(cast.x_value());
        announcement->mutable_flex_payments()->CopyFrom(cast.flex_payments());
        announcement->set_face_index(cast.face_index());
        announcement->mutable_selected_modes()->CopyFrom(cast.selected_modes());
        if (cast.has_source()) {
            announcement->mutable_source()->CopyFrom(cast.source());
        }
        announcement->mutable_cost_selections()->CopyFrom(cast.cost_selections());
        announcement->mutable_cast_cost_group_selections()->CopyFrom(cast.cast_cost_group_selections());
        announcement->set_cast_method(cast.cast_method());
        if (cast.has_casting_permission_id()) {
            announcement->set_casting_permission_id(cast.casting_permission_id());
        }
        LegacyCastCommit commit;
        commit.hasPayment = cast.has_payment();
        if (commit.hasPayment) {
            commit.payment.CopyFrom(cast.payment());
        }
        commit.restrictedMana.assign(cast.restricted_mana().begin(), cast.restricted_mana().end());
        legacyCastCommit = std::move(commit);
    };
    if (explicitCommand.has_cast_spell()) {
        ruled::v1::RuledCommand translated;
        translateCast(explicitCommand.cast_spell(), translated.mutable_begin_spell_cast()->mutable_announcement());
        explicitCommand = std::move(translated);
    } else if (explicitCommand.has_submit_resolution_choice() &&
               explicitCommand.submit_resolution_choice().has_cast_spell()) {
        ruled::v1::RuledCommand translated;
        auto *choice = translated.mutable_submit_resolution_choice();
        choice->CopyFrom(explicitCommand.submit_resolution_choice());
        choice->clear_cast_spell();
        translateCast(explicitCommand.submit_resolution_choice().cast_spell(),
                      choice->mutable_spell_cast_announcement());
        explicitCommand = std::move(translated);
    }
    explicitCommand.SerializeToString(&bytes);
    cont.add_game_command()->MutableExtension(Command_RuledPayload::ext)->set_payload(bytes);
    ruledCmdIds.insert(nextCmdId);
    sendContainer(cont);
    onRuledCommandSent();
    log(QStringLiteral("-> %1").arg(what));
}

::testing::AssertionResult SmokeClient::publishMain1Stops()
{
    ruled::v1::RuledCommand cmd;
    auto *policy = cmd.mutable_set_auto_pass_policy();
    // The scripted clients need one ordinary priority window in which to play their lands and
    // spells. Stack entries and required combat/cleanup choices stop automatically; every
    // other empty step is intentionally allowed to settle without an inferred legal-action stop.
    policy->add_stop_on_own_turn(ruled::v1::PHASE_ID_MAIN1);
    policy->add_stop_on_opponent_turn(ruled::v1::PHASE_ID_MAIN1);
    CommandContainer cont;
    cont.set_game_id(gameId);
    std::string bytes;
    cmd.SerializeToString(&bytes);
    cont.add_game_command()->MutableExtension(Command_RuledPayload::ext)->set_payload(bytes);
    const quint64 commandId = nextCmdId;
    sendContainer(cont);
    log(QStringLiteral("-> publish Main1 stop policy"));
    auto result = pumpUntil([&] { return responses.count(commandId) > 0; }, 10000, "auto-pass policy response");
    if (!result) {
        return result;
    }
    if (responses[commandId].response_code() != Response::RespOk) {
        return ::testing::AssertionFailure()
               << "auto-pass policy failed with code " << responses[commandId].response_code();
    }
    return ::testing::AssertionSuccess();
}
void SmokeClient::handleGameEventContainer(const GameEventContainer &cont)
{
    for (const auto &ev : cont.event_list()) {
        observePhysicalEvent(ev);
        onPhysicalEvent(ev);
        if (ev.HasExtension(Event_RuledPayload::ext)) {
            ruled::v1::RuledEventBatch batch;
            if (batch.ParseFromString(ev.GetExtension(Event_RuledPayload::ext).payload()))
                applyRuledBatch(batch);
        }
    }
}

void SmokeClient::applyRuledBatch(const ruled::v1::RuledEventBatch &batch)
{
    if (batch.has_payment_preview()) {
        ++paymentPreviewCount;
        paymentPreview = batch.payment_preview();
        onPaymentPreview(batch);
        return;
    }
    const auto localLegal = batch.legal_by_player().find(myId);
    const bool spellAnnouncementBatch =
        localLegal != batch.legal_by_player().end() && localLegal->second.has_pending_spell_cast();
    if (!spellAnnouncementBatch) {
        ++stateVersion;
    }
    onBatchBegin(batch);
    for (const auto &ev : batch.events()) {
        observeRuledEvent(ev, [this](const QString &line) { log(line); });
        onRuledEvent(ev);
    }
    onBatchEventsComplete(batch);
    if (observeLegalActions(batch)) {
        commitTranslatedLegacyCast(batch);
        releaseCommandsAfterTranslatedCast(batch);
        onLegalActions(batch);
    }
}

void SmokeClient::commitTranslatedLegacyCast(const ruled::v1::RuledEventBatch &batch)
{
    if (!legacyCastCommit.has_value()) {
        return;
    }
    const auto legal = batch.legal_by_player().find(myId);
    if (legal == batch.legal_by_player().end() || !legal->second.has_pending_spell_cast()) {
        return;
    }
    const quint64 transactionId = legal->second.pending_spell_cast().transaction_id();
    LegacyCastCommit payment = std::move(*legacyCastCommit);
    legacyCastCommit.reset();
    translatedCastCommitInFlight = true;
    ruled::v1::RuledCommand commit;
    auto *spell = commit.mutable_commit_spell_cast();
    spell->set_transaction_id(transactionId);
    if (payment.hasPayment) {
        spell->mutable_payment()->CopyFrom(payment.payment);
        if (legal->second.pending_spell_cast().has_payment_preview()) {
            const auto &authoritative = legal->second.pending_spell_cast().payment_preview().selection();
            spell->mutable_payment()->set_expected_state_revision(authoritative.expected_state_revision());
            if (authoritative.has_source()) {
                spell->mutable_payment()->mutable_source()->CopyFrom(authoritative.source());
            }
        }
    }
    for (const auto &restricted : payment.restrictedMana) {
        spell->add_restricted_mana()->CopyFrom(restricted);
    }
    sendingTranslatedCastCommit = true;
    sendRuled(commit, QStringLiteral("commit translated spell cast"));
    sendingTranslatedCastCommit = false;
}

void SmokeClient::releaseCommandsAfterTranslatedCast(const ruled::v1::RuledEventBatch &batch)
{
    if (!translatedCastCommitInFlight) {
        return;
    }
    const auto legal = batch.legal_by_player().find(myId);
    if (legal == batch.legal_by_player().end() || legal->second.has_pending_spell_cast()) {
        return;
    }
    translatedCastCommitInFlight = false;
    auto queued = std::move(commandsAfterTranslatedCast);
    commandsAfterTranslatedCast.clear();
    for (const auto &[command, what] : queued) {
        sendRuled(command, what);
    }
}
bool SmokeClient::labelMatching(const QRegularExpression &re, QRegularExpressionMatch *out) const
{
    for (const QString &l : labels) {
        const QRegularExpressionMatch m = re.match(l);
        if (m.hasMatch()) {
            if (out) {
                *out = m;
            }
            return true;
        }
    }
    return false;
}

const ruled::v1::LegalHandAction *SmokeClient::handAction(ruled::v1::HandActionKind kind, const QString &cardName) const
{
    for (const auto &action : latestLegal.hand_actions()) {
        if (action.kind() == kind && (cardName.isEmpty() || QString::fromStdString(action.card_name()) == cardName)) {
            return &action;
        }
    }
    return nullptr;
}

const ruled::v1::LegalZoneAbilityAction *SmokeClient::zoneAbilityAction(const QString &cardName,
                                                                        ruled::v1::AbilitySourceZone sourceZone) const
{
    for (const auto &action : latestLegal.zone_ability_actions()) {
        if (QString::fromStdString(action.card_name()) == cardName && action.source_zone() == sourceZone) {
            return &action;
        }
    }
    return nullptr;
}

QList<const ruled::v1::LegalHandAction *> SmokeClient::handActions(ruled::v1::HandActionKind kind) const
{
    QList<const ruled::v1::LegalHandAction *> result;
    for (const auto &action : latestLegal.hand_actions()) {
        if (action.kind() == kind) {
            result.append(&action);
        }
    }
    return result;
}

int SmokeClient::countOwn(const QString &cardId, bool untappedOnly) const
{
    const auto it = battlefieldByPlayer.find(myId);
    if (it == battlefieldByPlayer.end()) {
        return 0;
    }
    int n = 0;
    for (const Permanent &perm : it->second) {
        if (perm.cardId == cardId && (!untappedOnly || !perm.tapped)) {
            ++n;
        }
    }
    return n;
}

void SmokeClient::setBattlefieldAbilitySource(ruled::v1::ActivateAbility *ability, quint32 oid) const
{
    ability->set_source_object_id(oid);
    ability->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD);
    const auto it = battlefieldByPlayer.find(myId);
    if (it == battlefieldByPlayer.end()) {
        return;
    }
    const auto permanent = std::find_if(it->second.begin(), it->second.end(),
                                        [oid](const Permanent &candidate) { return candidate.oid == oid; });
    if (permanent != it->second.end()) {
        ability->set_expected_zone_change_generation(permanent->generation);
    }
}
} // namespace ruled_e2e
