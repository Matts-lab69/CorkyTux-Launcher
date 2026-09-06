cmake_minimum_required(VERSION 3.21)
project(corkytux VERSION 2.11.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

find_package(Qt6 REQUIRED COMPONENTS Core Gui Quick Network Concurrent Svg)

qt_standard_project_setup(REQUIRES 6.4)

set(BACKEND_SOURCES
    src/main.cpp
    src/backend/ConfigManager.cpp
    src/backend/GameModel.cpp
    src/backend/ProtonManager.cpp
    src/backend/IntegrationManager.cpp
    src/backend/PluginManager.cpp
    src/backend/ThemeManager.cpp
)

set(BACKEND_HEADERS
    src/backend/ConfigManager.h
    src/backend/GameModel.h
    src/backend/ProtonManager.h
    src/backend/IntegrationManager.h
    src/backend/PluginManager.h
    src/backend/ThemeManager.h
)

qt_add_executable(corkytux ${BACKEND_SOURCES} ${BACKEND_HEADERS})

qt_add_qml_module(corkytux
    URI CorkyTux
    VERSION 1.0
    QML_FILES
        qml/Main.qml
        qml/Sidebar.qml
        qml/GameCard.qml
        qml/DetailsPanel.qml
        qml/AddGameModal.qml
        qml/SettingsModal.qml
        qml/GameSettingsModal.qml
        qml/RemoveModal.qml
        qml/LogModal.qml
        qml/ProtonModal.qml
        qml/components/CButton.qml
        qml/components/CCard.qml
        qml/components/CComboBox.qml
        qml/components/CIcon.qml
        qml/components/CModal.qml
        qml/components/CSwitch.qml
        qml/components/CTextField.qml
)

target_link_libraries(corkytux PRIVATE Qt6::Core Qt6::Gui Qt6::Quick Qt6::Network Qt6::Concurrent Qt6::Svg)

# UI artwork (icons, logo) served as qrc:/assets/...
file(GLOB ASSET_FILES RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}" CONFIGURE_DEPENDS "qml/assets/*.png")
qt_add_resources(corkytux "corkytux_assets"
    PREFIX "/assets"
    FILES ${ASSET_FILES}
)

install(TARGETS corkytux RUNTIME DESTINATION bin)

# Desktop entry + icon (so cmake --install puts CorkyTux in the app menu)
install(FILES release/corkytux.png
        DESTINATION share/icons
        RENAME corkytux.png)

configure_file(release/corkytux.desktop.in
               ${CMAKE_CURRENT_BINARY_DIR}/corkytux.desktop
               @ONLY)
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/corkytux.desktop
        DESTINATION share/applications)
