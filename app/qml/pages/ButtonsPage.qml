import QtQuick
import QtQuick.Layouts
import "Actions.js" as Actions

Item {
    id: page
    property var device
    property string profileId: ""
    property string selected: ""
    property var pressed: ({})
    property var recentApps: []

    readonly property var profile: {
        var ps = device ? (device.config.profiles || []) : []
        for (var i = 0; i < ps.length; ++i)
            if (ps[i].id === profileId)
                return ps[i]
        return null
    }
    onDeviceChanged: if (profileId !== "" && !profile) profileId = ""

    // Live press highlighting and the gesture trail need the service to report input.
    readonly property bool watching: visible && Window.window !== null && Window.window.active
    onWatchingChanged: Daemon.setMonitoring(watching)
    Component.onCompleted: Daemon.setMonitoring(watching)
    Component.onDestruction: Daemon.setMonitoring(false)

    // Effective binding of every control in the selected profile.
    readonly property var bindings: {
        var out = {}
        if (!device)
            return out
        var base = device.config.bindings || {}
        var own = profile ? profile.bindings : base
        var controls = (device.controls || []).map(function (c) { return c.key })
        for (var i = 0; i < controls.length; ++i) {
            var k = controls[i]
            var mine = own.buttons ? own.buttons[k] : undefined
            if (mine !== undefined)
                out[k] = { action: mine, inherited: false }
            else
                out[k] = { action: (base.buttons && base.buttons[k]) || { type: "default" }, inherited: profile !== null }
        }
        var tw = own.thumbwheel !== undefined && own.thumbwheel !== null ? own.thumbwheel : base.thumbwheel
        var twInherited = profile !== null && (own.thumbwheel === undefined || own.thumbwheel === null)
        out["thumbwheel"] = { action: twAction(tw), inherited: twInherited }
        return out
    }
    function twAction(b) {
        // Shown on the stage as the wheel's mode.
        return { type: "thumbwheel-mode", mode: Actions.thumbwheelModeOf(b) }
    }

    Connections {
        target: Daemon
        function onButtonEvent(id, control, isPressed) {
            if (!page.device || id !== page.device.id)
                return
            var p = Object.assign({}, page.pressed)
            if (isPressed)
                p[control] = true
            else
                delete p[control]
            page.pressed = p
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 28
        anchors.bottomMargin: 20
        spacing: 18

        PageHeader {
            title: "Buttons"
            subtitle: page.profile ? "Changes apply only while " + page.profile.name + " is focused."
                                   : "Click a button to choose what it does. Press it on your mouse to find it."
        }

        ProfileBar {
            Layout.fillWidth: true
            device: page.device
            profileId: page.profileId
            recentApps: page.recentApps
            onSelectProfile: function (id) { page.profileId = id }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 18

            DeviceStage {
                Layout.fillWidth: true
                Layout.fillHeight: true
                device: page.device
                bindings: page.bindings
                selected: page.selected
                pressed: page.pressed
                onControlClicked: function (key) { page.selected = page.selected === key ? "" : key }
            }

            ControlInspector {
                id: inspector
                visible: page.selected !== ""
                Layout.preferredWidth: 400
                Layout.fillHeight: true
                device: page.device
                controlKey: page.selected
                profileId: page.profileId
                profileName: page.profile ? page.profile.name : ""
                onCloseRequested: page.selected = ""
                opacity: visible ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: Theme.anim } }
            }
        }
    }
}
