# Release Flow

This repository uses Release Please for a single repo-level product release.

## Version Source Of Truth

- The canonical product version lives in `Cargo.toml` under `[workspace.package].version`.
- Member crates inherit that version via `version.workspace = true`.
- Release Please updates the root workspace version and changelog for each release PR.

## Workflow

1. Land changes on `main` with Conventional Commit messages.
2. After the `CI` workflow succeeds on `main`, the `Release Please` workflow opens or updates the release PR.
3. Merge the release PR manually once it looks correct.
4. After that merge reaches `main` and `CI` passes again, `Release Please` creates the GitHub release and tag, then the workflow attaches Windows binaries and the SBOM.

## Notes

- Keep commit subjects Conventional Commit compatible or Release Please will ignore them for versioning and changelog generation.
- This repo intentionally avoids workflow-side auto-approval or auto-merge for the release PR.
