// notchtap-detect — prints the main screen's top safe-area inset, plus the
// notch cutout's left/right/width, as json and exits (spec §5, plan §3.5).
// the rust core shells out to this; a positive inset means a notch. no
// arguments, no other output.
import AppKit

let inset: Double
var cutoutLeftX = 0.0
var cutoutRightX = 0.0
var cutoutWidthValue = 0.0

if let screen = NSScreen.main {
    inset = screen.safeAreaInsets.top
    if let left = screen.auxiliaryTopLeftArea, let right = screen.auxiliaryTopRightArea {
        cutoutLeftX = left.maxX
        cutoutRightX = right.minX
        cutoutWidthValue = cutoutRightX - cutoutLeftX
    }
    // else: no notch (or one aux area missing) — cutout fields stay 0.0
} else {
    // headless / no display: report no notch rather than failing —
    // the rust side treats any inset <= 0 as hud mode anyway
    inset = 0.0
}

print("{ \"safe_area_top_inset\": \(inset), \"cutout_left_x\": \(cutoutLeftX), \"cutout_right_x\": \(cutoutRightX), \"cutout_width\": \(cutoutWidthValue) }")
