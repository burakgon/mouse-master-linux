cmake_minimum_required(VERSION 3.21)
project(mouse-master VERSION 0.1.0 LANGUAGES CXX
        DESCRIPTION "Configure Logitech mice on Linux: buttons, gestures, scrolling and more"
        HOMEPAGE_URL "https://github.com/burakgon/mouse-master-linux")

include(GNUInstallDirs)

option(BUILD_SERVICE "Build the Rust service (mouse-masterd) and CLI (mouse-masterctl) with cargo" ON)
set(UDEV_RULES_DIR "${CMAKE_INSTALL_PREFIX}/lib/udev/rules.d" CACHE PATH "Directory for udev rules")
set(SYSTEMD_USER_UNIT_DIR "${CMAKE_INSTALL_PREFIX}/lib/systemd/user" CACHE PATH "Directory for systemd user units")

add_subdirectory(app)

if(BUILD_SERVICE)
    find_program(CARGO cargo REQUIRED)
    set(CARGO_TARGET_DIR "${CMAKE_BINARY_DIR}/cargo")
    if(CMAKE_BUILD_TYPE STREQUAL "Debug")
        set(CARGO_PROFILE_FLAG "")
        set(CARGO_OUT "${CARGO_TARGET_DIR}/debug")
    else()
        set(CARGO_PROFILE_FLAG "--release")
        set(CARGO_OUT "${CARGO_TARGET_DIR}/release")
    endif()
    add_custom_target(service ALL
        COMMAND ${CMAKE_COMMAND} -E env CARGO_TARGET_DIR=${CARGO_TARGET_DIR}
                ${CARGO} build ${CARGO_PROFILE_FLAG} --locked -p mouse-masterd -p mouse-masterctl
        WORKING_DIRECTORY ${CMAKE_SOURCE_DIR}
        USES_TERMINAL
        COMMENT "Building mouse-masterd and mouse-masterctl")
    install(PROGRAMS "${CARGO_OUT}/mouse-masterd" "${CARGO_OUT}/mouse-masterctl" DESTINATION ${CMAKE_INSTALL_BINDIR})
endif()

configure_file(data/systemd/mouse-master.service.in ${CMAKE_BINARY_DIR}/mouse-master.service @ONLY)
configure_file(data/dbus/io.github.burakgon.MouseMaster.Daemon.service.in
               ${CMAKE_BINARY_DIR}/io.github.burakgon.MouseMaster.Daemon.service @ONLY)

install(FILES data/udev/70-mouse-master.rules DESTINATION ${UDEV_RULES_DIR})
install(FILES data/udev/70-mouse-master.rules DESTINATION ${CMAKE_INSTALL_DATADIR}/mouse-master)
install(FILES ${CMAKE_BINARY_DIR}/mouse-master.service DESTINATION ${SYSTEMD_USER_UNIT_DIR})
install(FILES ${CMAKE_BINARY_DIR}/io.github.burakgon.MouseMaster.Daemon.service DESTINATION ${CMAKE_INSTALL_DATADIR}/dbus-1/services)
install(FILES data/desktop/io.github.burakgon.MouseMaster.desktop DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
install(FILES data/metainfo/io.github.burakgon.MouseMaster.metainfo.xml DESTINATION ${CMAKE_INSTALL_DATADIR}/metainfo)
install(FILES app/icons/app/io.github.burakgon.MouseMaster.svg DESTINATION ${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps)
