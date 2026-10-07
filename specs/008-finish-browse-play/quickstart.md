# Quickstart: validating Finish Browse and Play (008)

Prerequisites: the native app built (`cargo build -p semantic-stash-viewer-native`); the Testing
instance on :9998 (its saved profile is `localhost:9998`) for anything that writes play history;
Production (`localhost:9999`) read-only.

Test data on Testing (seeded 2026-10-06 through its GraphQL API in the web UI's own formats; ids
only):

| What | Id(s) |
|---|---|
| Performers "SSV Test Performer A" / "B" (A on scenes 181–190, B on 185–195) | 2 / 3 |
| Ordinary tag (all of scenes 181–195); parent tag; child tag (even-numbered scenes) | 6; 7; 8 |
| `isTrait: 1` tag; `plugtag: 1` tag (scene 187) | 9; 10 |
| Common variant (`variantOfTagId` → 9); `isTrait: 0` tag; `ssvTestFlag: 1` tag (scenes 185–186) | 11; 12; 13 |
| Scene with markers (10–20 s, 60 s, 120 s) | 185 |
| Scene with several files | 23 |
| Scenes with sprites and animated previews generated | 185, 192 (194 queued) |
| Scenes with the custom field `ssvSceneFlag: 1` | 181, 183 |
| Saved filters: "SSV performer A"; "SSV parent tag + 1080p" (depth: all); "SSV scene custom field" | 1; 2; 3 |
| Default scene filter (`defaultFilters.scenes`): performer B, title ascending | – |

The plugin compatibility fixture is `crates/stash-core/tests/fixtures/semantic/vocabulary.json`:
the `SemanticVocabulary` result captured read-only from Production on 2026-10-06, with names and
aliases replaced by `tag-<id>` and only the semantic flags and `variantOfTagId` kept (it includes
real flags set to 0).

## Query budget (run first, automated)

```bash
cargo test -p stash-core --test graphql_operations   # selection snapshots and tier rules
cargo test -p stash-core --test query_budget         # request counts per action (mock server)
cargo run -p perf-harness -- --profile "localhost:9998" --quick   # adds the gql-* audit rows
```

Expected: snapshots and counts pass; every `gql-*` row within its budget
([contracts/query-budget.md](contracts/query-budget.md)): one request per page, one per scene
page, none for cached pickers, none for chips only passed over.

## V1: Find (US1)

Search a known title; add performer and resolution filters; check the count and the first 100
against Testing's web UI for the same criteria. Apply each saved filter; open a new Scenes tab
and check the default filter applies. From a scene page, choose a performer, a tag, and a studio
(and Ctrl+click one). Restart: the tab's search, chips, page, and scroll come back. Open the
filter menu and check the Semantic group shows its placeholders.
The 12 SC-001 queries (checked automatically by
`STASH_TEST_URL=http://localhost:9998 STASH_TEST_API_KEY=… cargo test -p stash-core --test live_stash -- --ignored`,
and by eye against Testing's web UI): search text only; one performer; one tag with sub-tags
(depth); one studio; resolution above 720p; duration between 10 and 30 minutes; not organized;
rating at least 4 stars; one custom field; and the 3 saved filters. On 2026-10-06 all 12 gave the
same count and first 100 as the web UI's request. Check the Tags picker: semantic tags grouped by type, each type
in its default colour, and the `isTrait: 0` tag among ordinary, uncoloured tags. Do the whole of V1 once with the keyboard only. Open the performer picker on
Production (read-only): it opens at once after the first time and narrows as you type.

## V2: Scene page (US2)

Open scenes with many performers, with markers, with several files, and with almost no metadata;
compare with the web UI's scene page. Hover a performer, tag, and studio chip (details card);
sweep across a row of chips quickly (nothing loads). Open "All files" and "Galleries". Choose a
marker (playback starts there). Tag chips show their type's colour; the variant tag shows its
canonical tag's colour.

On Testing only (it writes Stash's UI configuration): open Settings → Tag colours, check the
seven default rules and their tag counts, change the Role colour, add a rule for another tag
flag, reorder two rules, and save. Chips recolour at once; Testing's `configuration { ui }`
holds `semanticStashViewer.tagColours` and nothing else in the UI configuration changed. Reset to
the defaults. Disconnect and save: the page says the colours weren't saved and keeps the old ones.

## V3: Resume and history (US3, Testing only)

Play a scene for 2 minutes, stop; within 5 s the web UI shows the resume point and time watched;
reopen it (resumes there, "start from the beginning" offered). Watch to the end (resume cleared).
End viewings by tab close, scene change, server switch, quit, and pulling the network (then
reconnect): every one reaches Stash. Kill the app mid-viewing: Stash has the history to within a
minute. Turn "Save play history" off: nothing changes in the web UI. Jump between markers with
the chapter keys.

## V4: Previews (US4)

Hover cards (preview starts within the budget, stops on leave); wall mode on a 250 page (stills;
only the hovered one animates); a scene without previews (still stays); hover the seek bar of a
scene with sprites (thumbnail and time).

## V5: Fallback (US5, Testing)

A file mpv can't decode (prepare one on Testing): the error offers "Play transcoded"; it plays
with a "Transcoded" badge, and the log has the scene and reason. Normal scenes never transcode.

## Gate

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, and the harness with every 007 row still passing (SC-009).
