#include "ruled_game_mode.h"

#include "abstract_game.h"
#include "game_meta_info.h"
namespace RuledActions
{
bool isRuledGame(const AbstractGame *game)
{
    return game && const_cast<AbstractGame *>(game)->getGameMetaInfo() &&
           const_cast<AbstractGame *>(game)->getGameMetaInfo()->proto().ruled_game();
}
} // namespace RuledActions
