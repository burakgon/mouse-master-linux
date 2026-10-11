pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts

// "All applications" plus one chip per application profile.
Flow {
    id: root
    property var device
    property string profileId: ""
    property var recentApps: []
    signal selectProfile(string id)
    spacing: 8

    readonly property var profiles: device ? (device.config.profiles || []) : []

    Chip {
        text: "All applications"
        iconName: "layers"
        selected: root.profileId === ""
        onClicked: root.selectProfile("")
    }
    Repeater {
        model: root.profiles
        delegate: Chip {
            required property var modelData
            readonly property var app: Apps.lookup(modelData.apps.length ? modelData.apps[0] : modelData.name)
            text: modelData.name
            imageSource: "image://theme/" + (app.icon || "application-x-executable")
            selected: root.profileId === modelData.id
            closable: hovered || selected
            onClicked: root.selectProfile(modelData.id)
            onCloseClicked: {
                removeDialog.profile = modelData
                removeDialog.open()
            }
        }
    }
    Chip {
        text: "Add application"
        iconName: "plus"
        onClicked: picker.open()
    }

    AppPickerDialog {
        id: picker
        title: "Customize for an application"
        recent: root.recentApps
        onChosen: function (app) {
            var ids = [app.id]
            if (app.wmClass && ids.indexOf(app.wmClass) < 0)
                ids.push(app.wmClass)
            Daemon.command(root.device.id, { command: "add-profile", name: app.name, apps: ids }, function (err, id) {
                if (err)
                    Notify.show(err, true)
                else
                    root.selectProfile(id)
            })
        }
    }

    Modal {
        id: removeDialog
        property var profile: null
        title: "Remove " + (profile ? profile.name : "") + " settings?"
        subtitle: "The buttons will behave like in all other applications."
        footer: [
            Item { Layout.fillWidth: true },
            AppButton { text: "Cancel"; kind: "ghost"; onClicked: removeDialog.close() },
            AppButton {
                text: "Remove"
                kind: "danger"
                onClicked: {
                    var id = removeDialog.profile.id
                    Daemon.command(root.device.id, { command: "remove-profile", id: id })
                    if (root.profileId === id)
                        root.selectProfile("")
                    removeDialog.close()
                }
            }
        ]
    }
}
