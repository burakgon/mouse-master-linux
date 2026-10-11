find_package(Qt6 6.5 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 DBus Svg)
qt_standard_project_setup(REQUIRES 6.5)

qt_add_executable(mouse-master
    src/main.cpp
    src/daemonclient.cpp src/daemonclient.h
    src/system.cpp src/system.h
    src/apps.cpp src/apps.h
    src/iconprovider.cpp src/iconprovider.h
)
target_compile_definitions(mouse-master PRIVATE MOUSE_MASTER_VERSION="${PROJECT_VERSION}")
target_include_directories(mouse-master PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/src)
set_target_properties(mouse-master PROPERTIES WIN32_EXECUTABLE FALSE)

file(GLOB_RECURSE QML_SOURCES RELATIVE ${CMAKE_CURRENT_SOURCE_DIR} CONFIGURE_DEPENDS qml/*.qml qml/*.js)
set_source_files_properties(qml/Theme.qml qml/Notify.qml PROPERTIES QT_QML_SINGLETON_TYPE TRUE)
# Type names come from file names; keep the folder structure out of the URI.
foreach(file ${QML_SOURCES})
    get_filename_component(name ${file} NAME)
    set_source_files_properties(${file} PROPERTIES QT_RESOURCE_ALIAS ${name})
endforeach()

qt_add_qml_module(mouse-master
    URI MouseMaster
    VERSION 1.0
    QML_FILES ${QML_SOURCES}
)

file(GLOB LUCIDE_ICONS RELATIVE ${CMAKE_CURRENT_SOURCE_DIR} CONFIGURE_DEPENDS icons/lucide/*.svg)
file(GLOB DEVICE_ART RELATIVE ${CMAKE_CURRENT_SOURCE_DIR} CONFIGURE_DEPENDS assets/devices/*.svg)
set_source_files_properties(${CMAKE_SOURCE_DIR}/data/udev/70-mouse-master.rules PROPERTIES QT_RESOURCE_ALIAS data/70-mouse-master.rules)
qt_add_resources(mouse-master "assets"
    PREFIX "/"
    FILES
        ${LUCIDE_ICONS}
        ${DEVICE_ART}
        icons/app/io.github.burakgon.MouseMaster.svg
)
qt_add_resources(mouse-master "rules"
    PREFIX "/"
    BASE ${CMAKE_SOURCE_DIR}
    FILES ${CMAKE_SOURCE_DIR}/data/udev/70-mouse-master.rules
)

target_link_libraries(mouse-master PRIVATE
    Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2 Qt6::DBus Qt6::Svg)

install(TARGETS mouse-master RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
