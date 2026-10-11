// Device artwork and where each control sits on it (artwork coordinates).
.pragma library

var layouts = {
    "mx-master": {
        art: "qrc:/assets/devices/mx-master.svg",
        width: 400, height: 560,
        spots: {
            "right": { x: 284, y: 108, side: "right" },
            "middle": { x: 213, y: 132, side: "right" },
            "mode-shift": { x: 213, y: 201, side: "right" },
            "left": { x: 158, y: 118, side: "left" },
            "thumbwheel": { x: 96, y: 280, side: "left" },
            "forward": { x: 78, y: 323, side: "left" },
            "back": { x: 80, y: 355, side: "left" },
            "haptic-panel": { x: 92, y: 395, side: "left" },
            "gesture": { x: 66, y: 426, side: "left" }
        }
    },
    "mx-anywhere": {
        art: "qrc:/assets/devices/mx-anywhere.svg",
        width: 400, height: 560,
        spots: {
            "right": { x: 256, y: 150, side: "right" },
            "middle": { x: 200, y: 142, side: "right" },
            "mode-shift": { x: 200, y: 213, side: "right" },
            "left": { x: 146, y: 160, side: "left" },
            "forward": { x: 113, y: 283, side: "left" },
            "back": { x: 113, y: 335, side: "left" },
            "gesture": { x: 200, y: 400, side: "right" }
        }
    },
    "generic": {
        art: "qrc:/assets/devices/generic-mouse.svg",
        width: 400, height: 560,
        spots: {
            "left": { x: 146, y: 150, side: "left" },
            "right": { x: 256, y: 150, side: "right" },
            "middle": { x: 200, y: 128, side: "right" },
            "forward": { x: 100, y: 305, side: "left" },
            "back": { x: 100, y: 353, side: "left" },
            "gesture": { x: 200, y: 400, side: "right" },
            "mode-shift": { x: 200, y: 205, side: "right" },
            "thumbwheel": { x: 100, y: 250, side: "left" },
            "dpi": { x: 200, y: 215, side: "right" }
        }
    }
}

function forDevice(device) {
    var name = (device && device.name ? device.name : "").toLowerCase()
    if (name.indexOf("mx master") >= 0)
        return layouts["mx-master"]
    if (name.indexOf("mx anywhere") >= 0)
        return layouts["mx-anywhere"]
    return layouts["generic"]
}
