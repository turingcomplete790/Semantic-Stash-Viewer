//! Regenerate `ui/src/bindings.ts` without launching the app (used by CI's drift check).

fn main() {
    let builder = semantic_stash_viewer::specta_builder();
    if let Err(e) = semantic_stash_viewer::export_bindings(&builder) {
        eprintln!("failed to export TypeScript bindings: {e}");
        std::process::exit(1);
    }
    println!("wrote {}", semantic_stash_viewer::BINDINGS_PATH);
}
