import QtQuick
import QtQuick.Layouts
import "Actions.js" as Actions

// Click, then press a key combination. Emits `recorded(keys)` with Linux key names.
Rectangle {
    id: root
    property var keys: []
    property bool recording: false
    signal recorded(var keys)

    property var _held: []
    property var _combo: []

    implicitHeight: 52
    implicitWidth: 320
    radius: 12
    color: recording ? Theme.accentAlpha(0.1) : Theme.surfaceAlt
    border.width: recording ? 2 : 1
    border.color: recording ? Theme.accent : Theme.stroke
    Behavior on color { ColorAnimation { duration: Theme.anim } }
    focus: recording
    activeFocusOnTab: true

    SequentialAnimation on border.color {
        running: root.recording
        loops: Animation.Infinite
        ColorAnimation { to: Theme.accentAlpha(0.35); duration: 600 }
        ColorAnimation { to: Theme.accent; duration: 600 }
    }

    function start() {
        _held = []
        _combo = []
        recording = true
        forceActiveFocus()
    }
    function finish() {
        recording = false
        if (_combo.length > 0) {
            keys = Actions.normalizeCombo(_combo)
            recorded(keys)
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 14
        anchors.rightMargin: 10
        spacing: 10
        Icon {
            name: "keyboard"
            size: 18
            color: root.recording ? Theme.accent : Theme.textSecondary
        }
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            KeyCaps {
                anchors.verticalCenter: parent.verticalCenter
                visible: !root.recording && root.keys.length > 0
                keys: root.keys
            }
            KeyCaps {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.recording && root._combo.length > 0
                keys: Actions.normalizeCombo(root._combo)
            }
            Label {
                anchors.verticalCenter: parent.verticalCenter
                visible: (root.recording && root._combo.length === 0) || (!root.recording && root.keys.length === 0)
                text: root.recording ? "Press a key combination…" : "Click to record a shortcut"
                secondary: true
            }
        }
        AppButton {
            compact: true
            kind: root.recording ? "primary" : "secondary"
            text: root.recording ? "Done" : "Record"
            onClicked: root.recording ? root.finish() : root.start()
        }
    }

    MouseArea {
        anchors.fill: parent
        z: -1
        onClicked: if (!root.recording) root.start()
    }

    Keys.onPressed: function (event) {
        if (!root.recording)
            return
        event.accepted = true
        var name = Actions.keyNameFromScanCode(event.nativeScanCode)
        if (!name)
            return
        if (root._held.length === 0)
            root._combo = []
        if (root._held.indexOf(name) < 0)
            root._held = root._held.concat([name])
        if (root._combo.indexOf(name) < 0)
            root._combo = root._combo.concat([name])
    }
    Keys.onReleased: function (event) {
        if (!root.recording || event.isAutoRepeat)
            return
        event.accepted = true
        var name = Actions.keyNameFromScanCode(event.nativeScanCode)
        root._held = root._held.filter(function (k) { return k !== name })
        // Finish once everything is released and a non-modifier key was part of the combo.
        if (root._held.length === 0 && root._combo.some(function (k) { return !Actions.isModifier(k) }))
            root.finish()
    }
    onActiveFocusChanged: if (!activeFocus && recording) finish()
}
