# SPDX-License-Identifier: Apache-2.0 OR MIT

cmake_minimum_required(VERSION 3.20.0)

find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
project(Project)

file(READ "${CMAKE_SOURCE_DIR}/VERSION" VERSION_RAW)
string(REPLACE "\r\n" "\n" VERSION_RAW "${VERSION_RAW}")
string(REPLACE "\r"   "\n" VERSION_RAW "${VERSION_RAW}")
string(REGEX REPLACE "\n$" "" VERSION_RAW "${VERSION_RAW}")
string(REPLACE "\n" ";" VERSION_LINES "${VERSION_RAW}")
foreach(line IN LISTS VERSION_LINES)
    string(STRIP "${line}" line)
    if(line MATCHES "^([A-Z_]+)[ ]*=[ ]*(.+)$")
        set(KEY "${CMAKE_MATCH_1}")
        set(VAL "${CMAKE_MATCH_2}")
        message(STATUS "Defining ${KEY}=\"${VAL}\"")
        add_compile_definitions(${KEY}="${VAL}")
    endif()
endforeach()

file(GLOB mongoose_files mongoose/*.c)
target_include_directories(app PRIVATE "${CMAKE_CURRENT_LIST_DIR}/mongoose")

file(GLOB SRC_C_FILES CONFIGURE_DEPENDS "src/*.c" ${mongoose_files})
target_sources(app PRIVATE ${SRC_C_FILES})

rust_cargo_application()

