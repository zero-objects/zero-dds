cmake_minimum_required(VERSION 3.16)
project(zerodds-cpp
    VERSION 1.0.0
    DESCRIPTION "ZERODDS CXX bindings" 
    LANGUAGES CXX
)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_WINDOWS_EXPORT_ALL_SYMBOLS ON)

option(BUILD_SHARED_LIBS "Build static or shared library" OFF)
option(HDDS_BUILD_INSTALL "Build install target" OFF)
option(BUILD_EXAMPLES "Build examples" ON)
option(BUILD_TESTS "Build tests" ON)

# Find thread library
# find_package(Threads REQUIRED)

# Interface library being header-only
set(ZERODDS_CXX_HEADERS 
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/cond/Condition.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/policy/CorePolicy.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/Exception.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/Fixed.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/InstanceHandle.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/qos_bridge.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/qos.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/Status.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/core/Time.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/domain/DomainParticipant.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/pub/DataWriterListener.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/pub/Publisher.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/sub/cond/ReadCondition.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/sub/DataReaderListener.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/sub/Subscriber.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/BuiltinTopic.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/ContentFilteredTopic.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/Topic.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/TopicTraits.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/xcdr2_md5.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/topic/xcdr2.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/dds/dds.hpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/include/zerodds/dds.hpp"
)
add_library(zerodds-cpp INTERFACE ${ZERODDS_CXX_HEADERS})
add_library(zerodds::zerodds-cpp ALIAS zerodds-cpp)

target_include_directories(zerodds-cpp
    INTERFACE
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<INSTALL_INTERFACE:include>
)

target_link_libraries(zerodds-cpp INTERFACE zerodds::zerodds-c 
    #PRIVATE Threads::Threads ${CMAKE_DL_LIBS}
)

# Install
if(HDDS_BUILD_INSTALL)
    include(CMakePackageConfigHelpers)

    install(FILES ${CMAKE_CURRENT_SOURCE_DIR}/include DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})

    # Configuration
    configure_package_config_file(
        ${CMAKE_CURRENT_SOURCE_DIR}/cmake/${PROJECT_NAME}Config.cmake.in
        ${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}Config.cmake
        INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
    )
    write_basic_package_version_file(
        ${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}ConfigVersion.cmake
        VERSION ${PROJECT_VERSION}
        COMPATIBILITY SameMajorVersion
    )
    # Package
    install(FILES ${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}Config.cmake
                  ${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}ConfigVersion.cmake
            DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
    )
    install(
        EXPORT ${PROJECT_NAME}Targets
        FILE ${PROJECT_NAME}Targets.cmake
        NAMESPACE ${PROJECT_NAME}::
        DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
    )
    install(
        TARGETS ${PROJECT_NAME}
        EXPORT ${PROJECT_NAME}Targets
        RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
        LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
        ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
    )

    #
    # CPack
    #
    set(CPACK_GENERATOR DEB RPM TXZ TGZ)
    set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
    set(CPACK_PACKAGE_NAME ${PROJECT_NAME})
    set(CPACK_PACKAGE_DESCRIPTION_SUMMARY ${PROJECT_DESCRIPTION})
    set(CPACK_PACKAGE_FILE_NAME
        "${CPACK_PACKAGE_NAME}-${CPACK_PACKAGE_VERSION}.${CMAKE_SYSTEM_PROCESSOR}"
    )
    set(CPACK_PACKAGE_RPM_DIR "${CMAKE_CURRENT_BINARY_DIR}/_CPack_Packages/Linux/RPM")
    include(CPack)
endif()

# Examples
if(BUILD_EXAMPLES)
    add_executable(cpp_smoke ${CMAKE_CURRENT_SOURCE_DIR}/examples/cpp_smoke.cpp)
    target_link_libraries(cpp_smoke PRIVATE zerodds-cpp)
endif()

# Tests
if(BUILD_TESTS)
    add_executable(smoke_dds_psm ${CMAKE_CURRENT_SOURCE_DIR}/tests/smoke_dds_psm.cpp)
    target_link_libraries(smoke_dds_psm PRIVATE zerodds-cpp)
endif()
