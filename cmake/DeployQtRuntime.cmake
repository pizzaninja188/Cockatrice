include_guard(GLOBAL)

function(deploy_qt_runtime target)
  if(NOT WIN32)
    return()
  endif()
  # Keep local target directories runnable by auto-deploying Qt runtime DLLs/plugins.
  set(_cockatrice_qt_bin_hints "")
  if(DEFINED QTDIR)
    list(APPEND _cockatrice_qt_bin_hints "${QTDIR}/bin")
  elseif(DEFINED ENV{QTDIR})
    list(APPEND _cockatrice_qt_bin_hints "$ENV{QTDIR}/bin")
  endif()
  if(_cockatrice_qt_bin_hints)
    find_program(COCKATRICE_WINDEPLOYQT_EXECUTABLE NAMES windeployqt HINTS ${_cockatrice_qt_bin_hints} NO_DEFAULT_PATH)
  else()
    find_program(COCKATRICE_WINDEPLOYQT_EXECUTABLE NAMES windeployqt)
  endif()
  if(COCKATRICE_WINDEPLOYQT_EXECUTABLE)
    add_custom_command(
      TARGET ${target}
      POST_BUILD
      COMMAND "${COCKATRICE_WINDEPLOYQT_EXECUTABLE}"
              --$<IF:$<CONFIG:Debug>,debug,release>
              --no-translations
              --force
              "$<TARGET_FILE:${target}>"
      COMMENT "Deploying Qt runtime for ${target} target"
      VERBATIM
    )
  else()
    message(WARNING "windeployqt not found; ${target}.exe may fail at runtime until Qt DLLs are manually deployed.")
  endif()
endfunction()
