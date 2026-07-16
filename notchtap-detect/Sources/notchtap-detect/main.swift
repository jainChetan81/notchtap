// notchtap-detect — prints the main screen's top safe-area inset as json
// and exits (spec §5). the rust core shells out to this; a positive inset
// means a notch. no arguments, no other output.
import AppKit

let inset: Double
if let screen = NSScreen.main {
    inset = screen.safeAreaInsets.top
} else {
    // headless / no display: report no notch rather than failing —
    // the rust side treats any inset <= 0 as hud mode anyway
    inset = 0.0
}

print("{ \"safe_area_top_inset\": \(inset) }")
