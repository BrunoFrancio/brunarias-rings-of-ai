# Agent guidelines

- Write code, identifiers, user-facing text, comments, and documentation in English.
- Record new decisions and issues in `docs/development.md` with the date, rationale, and verification.
- Keep agent signatures, co-author trailers, and session links out of commits and pull requests.
- Verify window changes on a real screen before finishing. On Windows 11, egui and winit can restore the frame when window styles change. See the development log for details.
- Group changes into releases when there is a build to distribute. Routine commits do not require a version bump, tag, or release.
