// Plan 171 (tab-notch redesign, spec section 4): the rest-state audio
// indicator — three bars in `--media-mint`, visible only while audio is
// GENUINELY playing (never a generic "media available" state; this is
// deliberately narrower than the icon strip's own music-icon presence
// gate, which the design source leaves to whichever caller wires this
// up in slice K to decide is the right signal). Collapsed to zero width
// when silent so the idle face stays optically centered — width and
// opacity animate on one clock, matching the design source's own
// `.eq`/`.eq.playing` rule pair verbatim.
//
// New component: no existing surface in this app renders an equalizer-
// style indicator (confirmed via grep before writing this) — the
// `.media-bar`/`.media-bar-fill` the idle peek's now-playing row already
// draws is a PROGRESS bar (elapsed/duration), a different concept
// entirely, untouched by this component.
export function EqBars({ playing }: { playing: boolean }) {
  return (
    <span className={`eq${playing ? " playing" : ""}`} aria-hidden="true">
      <i />
      <i />
      <i />
    </span>
  );
}
