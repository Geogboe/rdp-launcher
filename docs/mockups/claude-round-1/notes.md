# Claude Round 1 - Design Notes

## What changed relative to the balanced mockups

### Visual language

- **Dropped the warm cream/green palette.** The balanced mockups used an earthy warm scheme (`#efe7db` backgrounds, green accents, 30px border-radius) that read more like a concept prototype than a Windows utility. Round 1 uses neutral grays, a standard Segoe UI type stack, and an accent blue (`#0067b8`) drawn from Windows system colors.
- **Added a real title bar and status bar.** These two framing elements immediately communicate "this is a desktop window" rather than "this is a design mockup floating on a gradient background." The title bar includes the standard minimize/maximize/close glyphs.
- **Reduced border-radius from 30px to 4-8px.** The earlier rounded corners felt like a mobile app or concept render. Tighter radii feel native to Windows.
- **Replaced the gradient body background with flat gray.** The balanced mockups used layered radial gradients behind the window. A plain `#e8e8e8` background reads as a Windows desktop.

### Information density and noise

- **Eliminated all pills/chips from the main list rows.** The balanced mockups had 2-3 chips per row ("Remote Guard", "Clipboard off", "PID 14844"). Round 1 moves this metadata into a single inline text line using middot separators, or defers it to the inspector pane entirely.
- **Status is text, not a colored badge.** "Connected", "Ready", and "Prompt auth" are right-aligned text with semantic color only, no pill background or border.
- **Active sessions use a thin left-edge indicator (3px green bar)** instead of a colored chip. This is quieter and scales better with many rows.
- **Removed per-row action buttons.** "Inspect" and "Launch" buttons on every row added significant visual noise. Actions now live in the inspector or are implied by selection.

### Layout

- **One unified list with section dividers**, not separate `<section>` blocks with individual headings. The section labels ("Active Sessions", "Connections") are small uppercase dividers, not full heading elements.
- **Inspector pane uses a key-value table layout** instead of summary cards with backgrounds and borders. This is visually lighter and conveys more information per pixel.
- **Inspector is narrower (240px fixed) and uses the panel background color**, making it clearly secondary to the main list.

## Why the new direction is calmer

1. **Fewer competing elements.** Each row carries only name, host, status text, and time. No chips, no inline actions, no badge borders.
2. **Consistent rhythm.** All rows share identical structure and height. The eye can scan the list without being interrupted by differently-shaped metadata.
3. **Color is used for meaning only.** Green = connected, amber = warning, blue = accent/action. Everything else is grayscale. The balanced mockups had green chips, green state pills, green accents, and green borders all competing.
4. **The inspector carries the detail burden.** This lets the main list stay lean. You only see connection/display/history details for the row you care about, not spread across every row.
5. **Windows-native framing.** The title bar, status bar, and flat background create an immediate context that says "utility" not "dashboard."

## Which mockup should become the primary baseline

**`home-with-inspector-calm.html`** is the strongest candidate for primary baseline.

Reasons:

- It demonstrates the full interaction model: scan the list, select a row, see details on the right. This is the core use case the product needs to support.
- The inspector makes the list rows leaner, because you know detail is one click away.
- The 680px width with a 240px inspector and ~440px list feels natural — similar proportions to Windows Event Viewer, Credential Manager, or the Services snap-in.
- It shows how both active sessions and idle connections coexist in one continuous list without needing separate visual treatments.

`home-calm.html` is useful as the "inspector closed" state — what the window looks like at 620px before a row is selected. It could also be the default view on first launch when no selection exists yet.

## What still feels unresolved

- **Right-click context menu.** A Windows utility should support right-click on a row for Launch, Edit, Duplicate, Delete. This isn't shown in static HTML but matters for believability.
- **Keyboard navigation.** Up/down to select, Enter to launch, Tab into the inspector. Important for the utility feel but can't be demonstrated in a static mockup.
- **Empty states.** What does the window look like with zero connections? With zero active sessions?
- **Active session row actions.** The current mockup pins sessions at the top, but doesn't show how to bring an MSTSC window to front (Reveal) or disconnect. This probably belongs in the inspector when a session row is selected.
- **Preset switching.** The architecture describes launch presets (daily driver, admin, low bandwidth). Where does preset selection surface? Probably in the inspector, but this needs a separate mockup.
- **Favorites / environment filtering.** The balanced mockups had filter chips (All, Active, Favorites). Round 1 dropped these to reduce noise. This may need to come back as a dropdown or toolbar filter rather than horizontal pills.
- **Multi-select / bulk operations.** Not explored yet. May not be needed for v1.
- **Inspector for active session vs. idle connection.** The inspector content should differ — an active session should show PID, runtime, and session actions, while an idle connection shows launch posture and history. Only the idle case is mocked so far.

## Follow-up interaction set

The next planning pass should treat these as the implementation-facing companions to the baseline:

- `active-session-selected.html`
- `preset-launch-selection.html`
- `empty-first-run.html`

Those files are not broad visual experiments. They exist to answer the remaining interaction questions needed before implementation starts.
