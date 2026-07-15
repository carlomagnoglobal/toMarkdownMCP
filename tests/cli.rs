//! End-to-end tests for the CLI subcommands, driving the real binary.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_to_markdown_mcp"))
}

#[test]
fn convert_fixture_note_to_stdout() {
    let out = bin()
        .args(["convert", "tests/fixtures/mini_vault/Note A.md"])
        .output()
        .expect("binary runs");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.trim().is_empty(), "conversion produced no output");
}

#[test]
fn convert_writes_output_file() {
    let dir = std::env::temp_dir().join(format!("to_md_cli_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out_file = dir.join("note.md");
    let out = bin()
        .args(["convert", "tests/fixtures/mini_vault/Note A.md", "-o"])
        .arg(&out_file)
        .output()
        .expect("binary runs");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let written = std::fs::read_to_string(&out_file).expect("output file written");
    assert!(!written.trim().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn tools_lists_catalog() {
    let out = bin().arg("tools").output().expect("binary runs");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("convert_file"), "catalog should mention convert_file");
    assert!(stdout.contains("obsidian_search"), "catalog should mention obsidian_search");
}

#[test]
fn search_finds_fixture_content() {
    let out = bin()
        .args(["search", "Note", "--dir", "tests/fixtures/mini_vault"])
        .output()
        .expect("binary runs");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    assert!(!String::from_utf8_lossy(&out.stdout).trim().is_empty());
}

#[test]
fn unknown_subcommand_fails_with_usage_error() {
    let out = bin().arg("definitely-not-a-command").output().expect("binary runs");
    assert!(!out.status.success());
}
