# Headless client-core tests: the ruled view model compiled on its own, with no Qt widgets and
# no game objects. Only ruled_client_state.cpp / ruled_event_dispatcher.cpp are pulled in —
# `RuledActions` (which does depend on CardItem/Player) deliberately stays out.
add_executable(
  ruled_client_test ruled_client_test.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_client_state.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_event_dispatcher.cpp
)

target_include_directories(ruled_client_test PRIVATE ${CMAKE_SOURCE_DIR}/cockatrice/src)

target_link_libraries(
  ruled_client_test
  PRIVATE libcockatrice_protocol
  PRIVATE Threads::Threads
  PRIVATE ${GTEST_BOTH_LIBRARIES}
  PRIVATE ${TEST_QT_MODULES}
)

add_test(NAME ruled_client_test COMMAND ruled_client_test)

if(NOT GTEST_FOUND)
  add_dependencies(ruled_client_test gtest)
endif()
