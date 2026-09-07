add_executable(ruled_diagnostics_test ruled_diagnostics_test.cpp)
target_link_libraries(ruled_diagnostics_test PRIVATE libcockatrice_protocol Threads::Threads
                      ${GTEST_BOTH_LIBRARIES} ${TEST_QT_MODULES})
if(NOT GTEST_FOUND)
  add_dependencies(ruled_diagnostics_test gtest)
endif()
add_test(NAME ruled_diagnostics_test COMMAND ruled_diagnostics_test)
