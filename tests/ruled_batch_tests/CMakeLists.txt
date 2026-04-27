add_executable(ruled_batch_test ${VERSION_STRING_CPP} ruled_batch_test.cpp)

if(NOT GTEST_FOUND)
  add_dependencies(ruled_batch_test gtest)
endif()

target_link_libraries(
  ruled_batch_test
  PRIVATE libcockatrice_network_server_remote
  PRIVATE libcockatrice_rng
  PRIVATE Threads::Threads
  PRIVATE ${GTEST_BOTH_LIBRARIES}
  PRIVATE ${TEST_QT_MODULES}
)

add_test(NAME ruled_batch_test COMMAND ruled_batch_test)
