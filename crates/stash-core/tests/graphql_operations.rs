//! Selection snapshots, tier rules, and the writes rule for every named operation (008 research
//! R2, contracts/query-budget.md "Checks").
//!
//! Regenerate the golden files after an intended change with
//! `UPDATE_GOLDEN=1 cargo test -p stash-core --test graphql_operations`, then review the diff.

use std::path::PathBuf;

use stash_core::adapter::operations::{documents, OperationDocument, Tier};

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("graphql/operations")
}

#[test]
fn every_operation_matches_its_golden_file() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut mismatched = Vec::new();
    for doc in documents() {
        let file = golden_dir().join(format!("{}.graphql", doc.name));
        let text = format!("{}\n", doc.query.trim_end());
        if update {
            std::fs::write(&file, &text).expect("write golden file");
            continue;
        }
        match std::fs::read_to_string(&file) {
            Ok(golden) if golden == text => {}
            Ok(_) => mismatched.push(format!("{} differs from {}", doc.name, file.display())),
            Err(_) => mismatched.push(format!("{} has no golden file", doc.name)),
        }
    }
    assert!(
        mismatched.is_empty(),
        "{}\n(run with UPDATE_GOLDEN=1 and review the diff)",
        mismatched.join("\n")
    );
}

#[test]
fn every_golden_file_belongs_to_an_operation() {
    let names: Vec<&str> = documents().iter().map(|d| d.name).collect();
    for entry in std::fs::read_dir(golden_dir()).expect("operations dir") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "graphql") {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            assert!(names.contains(&stem), "{stem}.graphql has no operation");
        }
    }
}

#[test]
fn card_chip_and_detail_operations_select_no_forbidden_field() {
    for doc in documents() {
        let paths = field_paths(&doc.query);
        for forbidden in forbidden_for(&doc) {
            let hit = paths.iter().find(|p| ends_with_path(p, forbidden));
            assert!(
                hit.is_none(),
                "{} selects `{}` (forbidden for its tier)",
                doc.name,
                hit.map(String::as_str).unwrap_or_default()
            );
        }
    }
}

#[test]
fn only_the_vocabulary_selects_custom_fields() {
    for doc in documents() {
        if doc.name == "SemanticVocabulary" {
            continue;
        }
        let hit = field_paths(&doc.query)
            .into_iter()
            .find(|p| ends_with_path(p, "custom_fields"));
        assert!(hit.is_none(), "{} selects custom_fields", doc.name);
    }
}

#[test]
fn only_play_history_and_tag_colours_write() {
    const WRITES: [&str; 3] = ["SaveActivity", "AddPlay", "SaveTagColours"];
    for doc in documents() {
        let is_mutation = doc.query.trim_start().starts_with("mutation");
        assert_eq!(
            is_mutation,
            WRITES.contains(&doc.name),
            "{} {} a mutation (FR-018)",
            doc.name,
            if is_mutation { "is" } else { "isn't" }
        );
    }
}

/// The fields an operation's tier must not select, as dotted path suffixes.
fn forbidden_for(doc: &OperationDocument) -> Vec<&'static str> {
    match doc.tier {
        Tier::Card | Tier::Chip => {
            // A scene's heavy relations, wherever a scene appears (lists of galleries are fine).
            let mut fields = vec![
                "paths.stream",
                "scenes.sceneStreams",
                "scenes.scene_markers",
                "scenes.galleries",
                "files.path",
                "files.fingerprints",
            ];
            // The performer list carries each picture's version (008 FR-011c, research R13).
            if doc.name != "ChoiceListPerformers" {
                fields.push("image_path");
            }
            if doc.name != "TagCard" {
                fields.push("description");
            }
            fields
        }
        Tier::Detail => vec![
            "findScene.galleries",
            "findScene.sceneStreams",
            "paths.stream",
            "files.path",
        ],
        Tier::Other => vec![],
    }
}

fn ends_with_path(path: &str, suffix: &str) -> bool {
    path == suffix || path.ends_with(&format!(".{suffix}"))
}

/// Every selected field as a dotted path from the operation root (`findScenes.scenes.id`),
/// ignoring arguments, aliases, variables, and fragment spreads.
fn field_paths(query: &str) -> Vec<String> {
    #[derive(PartialEq)]
    enum Token {
        Word(String),
        Open,
        Close,
        Colon,
    }
    let body = query.find('{').map_or("", |i| &query[i..]);
    let mut tokens = Vec::new();
    let mut parens = 0usize;
    let mut word = String::new();
    for c in body.chars() {
        if parens > 0 {
            match c {
                '(' => parens += 1,
                ')' => parens -= 1,
                _ => {}
            }
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        if !word.is_empty() {
            tokens.push(Token::Word(std::mem::take(&mut word)));
        }
        match c {
            '(' => parens = 1,
            '{' => tokens.push(Token::Open),
            '}' => tokens.push(Token::Close),
            ':' => tokens.push(Token::Colon),
            _ => {}
        }
    }

    let mut paths = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut last: Option<String> = None;
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Word(_) if tokens.get(i + 1) == Some(&Token::Colon) => {
                // `alias: field`: skip the alias.
                i += 2;
                continue;
            }
            Token::Word(w) if w == "on" => {
                // `... on Type { … }`: the type name isn't a field.
                i += 2;
                last = None;
                stack.push(String::new());
                // The `{` that follows belongs to this pseudo-level.
                if tokens.get(i) == Some(&Token::Open) {
                    i += 1;
                }
                continue;
            }
            Token::Word(w) => {
                let mut path: Vec<&str> = stack
                    .iter()
                    .map(String::as_str)
                    .filter(|s| !s.is_empty())
                    .collect();
                path.push(w);
                paths.push(path.join("."));
                last = Some(w.clone());
            }
            Token::Open => stack.push(last.take().unwrap_or_default()),
            Token::Close => {
                stack.pop();
                last = None;
            }
            Token::Colon => {}
        }
        i += 1;
    }
    paths
}

#[test]
fn field_paths_reads_nested_selections_with_arguments_and_aliases() {
    let q = "query X($f: F) { a: findScenes(filter: $f) { count scenes { id paths { stream } } } }";
    let paths = field_paths(q);
    assert!(paths.contains(&"findScenes.scenes.paths.stream".to_owned()));
    assert!(paths.contains(&"findScenes.count".to_owned()));
    assert!(ends_with_path(
        "findScenes.scenes.paths.stream",
        "paths.stream"
    ));
    assert!(!ends_with_path("findScenes.scenes.thumbstream", "stream"));
}
