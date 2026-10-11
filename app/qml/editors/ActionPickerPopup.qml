import QtQuick
import QtQuick.Controls.Basic as C

// The action picker in a popup, for gesture directions and wheel steps.
C.Popup {
    id: root
    property alias action: picker.action
    property alias controlKey: picker.controlKey
    property alias device: picker.device
    property alias allowDefault: picker.allowDefault
    property string heading: "Choose an action"
    signal picked(var action)

    width: 380
    height: Math.min(560, (parent ? parent.height : 700) - 80)
    modal: true
    focus: true
    padding: 16
    closePolicy: C.Popup.CloseOnEscape | C.Popup.CloseOnPressOutside
    parent: C.Overlay.overlay
    x: Math.round((parent.width - width) / 2)
    y: Math.round((parent.height - height) / 2)

    C.Overlay.modal: Rectangle { color: Theme.overlay }
    enter: Transition {
        NumberAnimation { property: "opacity"; from: 0; to: 1; duration: Theme.anim }
        NumberAnimation { property: "scale"; from: 0.97; to: 1; duration: Theme.anim; easing.type: Easing.OutCubic }
    }
    background: Rectangle {
        radius: Theme.radiusLarge
        color: Theme.raised
        border.width: 1
        border.color: Theme.strokeStrong
    }
    contentItem: Column {
        spacing: 12
        Row {
            width: parent.width
            Label {
                text: root.heading
                size: Theme.fontHeadline
                strong: true
                width: parent.width - closeButton.width
                anchors.verticalCenter: parent.verticalCenter
            }
            IconButton {
                id: closeButton
                iconName: "x"
                onClicked: root.close()
            }
        }
        ActionPicker {
            id: picker
            width: parent.width
            height: root.availableHeight - 50
            allowNone: true
            holdActions: false
            onPicked: function (action) {
                root.picked(action)
                root.close()
            }
        }
    }
}
