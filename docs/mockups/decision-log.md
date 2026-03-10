# Mockup Decisions

These notes explain what the current mockups are trying to test.

Earlier broad explorations were useful for narrowing the product direction, but the repo now keeps only the current baseline artifacts that still inform implementation.

## Claude Round 1 - Calm Utility Direction

- Tests whether stripping the warm cream palette, rounded corners, and chip-heavy rows in favor of a neutral gray, Windows-native look produces a calmer home screen.
- Replaces per-row pills and badges with inline text metadata (middot-separated) and semantic-colored status text.
- Adds a title bar and status bar to frame the mockup as a real desktop window rather than a floating concept.
- Active sessions use a thin green left-edge bar instead of colored state chips.
- The inspector pane is narrower, uses key-value text rows instead of bordered summary cards, and sits on the panel background to feel clearly secondary.
- This round is testing whether the design can feel serious and native without losing polish — a Windows utility that respects the user's time rather than a concept dashboard that tries to impress.
- `claude-round-1/home-with-inspector-calm.html` is the current preferred baseline for the home view.
- `claude-round-1/home-calm.html` is the companion state when the inspector is closed or nothing is selected.
- Follow-up interaction mockups in the same directory now cover:
  - selected active session behavior
  - preset selection before launch
  - empty and first-run behavior
