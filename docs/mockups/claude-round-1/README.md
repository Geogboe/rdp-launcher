# Claude Round 1 Mockups

A calmer, more utilitarian direction for the RDP Launch home screen.

## Files

- `home-calm.html` — Home view without inspector. Single-column list with active sessions pinned at top. 620px width.
- `home-with-inspector-calm.html` — Home view with a right-side inspector pane showing detail for the selected row. 680px width.
- `active-session-selected.html` — Active-session variant of the inspector, showing session-specific detail and actions.
- `preset-launch-selection.html` — Idle connection selected with preset choice in the inspector before launch.
- `empty-first-run.html` — No profiles yet; first-run and empty-state behavior.
- `notes.md` — Design rationale, comparison to the balanced mockups, and open questions.

## Key design choices

- Neutral gray palette with Windows-native blue accent instead of warm cream/green.
- Real title bar and status bar to frame the window as a desktop utility.
- No pills, chips, or badges on list rows. Metadata is inline text or deferred to the inspector.
- Active sessions indicated by a thin green left-edge bar, not colored badges.
- Inspector pane uses key-value pairs, not cards — visually lighter and more information-dense.
- Section dividers are small uppercase labels, not full heading blocks.
- Follow-up interaction mockups keep the same calmer shell and answer the remaining questions needed for slice-one implementation.

## How to review

Open the HTML files directly in a browser. Compare them against the earlier compact mockups and the rationale in `notes.md` to evaluate the visual noise reduction and the stronger Windows utility framing.
