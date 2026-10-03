# Contract: Decision record

The spike's lasting output (spec User Story 4, FR-014, SC-012), written to
`specs/006-native-ui-spike/decision.md` at the end of the spike (or at the playback gate on a
no-go, FR-015).

## Required sections

1. **Decision**: `GO` or `NO-GO`, with the reason in two or three plain sentences.
2. **Evidence**: one table, a row per success criterion SC-001 to SC-011:

   | Criterion | Budget | Native build | Web build (same machine, same day) | Pass |
   |---|---|---|---|---|

   The web build's column uses its harness run; "–" where it has no equivalent.
3. **Playback approach**: which path from research R1 was used (DMA-BUF, GL backend, or
   read-back), what was tried and why it was dropped, hardware decoding per codec, and anything
   platform-specific.
4. **What got simpler, what got harder**: calling the core and cache directly versus commands and
   bindings; testing; text input and IME; accessibility; menus, tooltips, and context menus;
   theming; build times and binary size.
5. **Toolkit gaps** found, each with: what's missing, how much it matters for the roadmap (search,
   filters, galleries, editing), and a workaround if one exists.
6. **Glue to move** for a port: the parts of `src-tauri` and the spike's `services.rs` that belong
   in a shared crate.
7. **Next steps**:
   - On GO: the constitution amendment (Technology & Platform Constraints, Principles III and V
     wording, per the 2026-10-03 analysis), and an outline of the port feature (screens in order,
     what carries over unchanged, what the web build keeps doing until the port reaches parity).
   - On NO-GO: what blocked it, whether a different native toolkit could get past it, and the
     WebKit issues the web build should fix instead.
8. **Unknowns**: untested platforms (Windows, macOS, X11), and anything measured only once.
