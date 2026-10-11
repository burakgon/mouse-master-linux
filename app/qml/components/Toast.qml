import QtQuick

// Transient message shown at the bottom of the window.
Rectangle {
    id: root
    property string message
    property bool error: false

    function show(text, isError) {
        message = text
        error = !!isError
        opacity = 1
        hideTimer.restart()
    }

    anchors.horizontalCenter: parent.horizontalCenter
    anchors.bottom: parent.bottom
    anchors.bottomMargin: opacity > 0 ? 24 : 4
    Behavior on anchors.bottomMargin { NumberAnimation { duration: Theme.anim; easing.type: Easing.OutCubic } }
    width: Math.min(parent.width - 48, row.implicitWidth + 36)
    height: 44
    radius: 22
    color: Theme.dark ? "#2a2c34" : "#1f2026"
    opacity: 0
    visible: opacity > 0
    Behavior on opacity { NumberAnimation { duration: Theme.anim } }
    z: 1000

    Row {
        id: row
        anchors.centerIn: parent
        spacing: 10
        Icon {
            name: root.error ? "circle-alert" : "circle-check"
            color: root.error ? "#ff8a96" : "#5fe0a5"
            size: 18
            anchors.verticalCenter: parent.verticalCenter
        }
        Text {
            text: root.message
            color: "#f2f3f6"
            font.pixelSize: Theme.fontBody
            anchors.verticalCenter: parent.verticalCenter
            elide: Text.ElideRight
            width: Math.min(implicitWidth, root.parent ? root.parent.width - 120 : 400)
        }
    }
    Timer {
        id: hideTimer
        interval: root.error ? 5000 : 2600
        onTriggered: root.opacity = 0
    }
}
