#ifndef RULED_DEV_CONSOLE_GAME_H
#define RULED_DEV_CONSOLE_GAME_H

class AbstractGame;
class RuledDevConsoleWidget;

/// Attach the console to the existing command transport and session/connection lifecycle.
void connectRuledDevConsole(RuledDevConsoleWidget *console, AbstractGame *game);

#endif
