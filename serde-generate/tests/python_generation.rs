// Copyright (c) Facebook, Inc. and its affiliates
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_generate::{python3, test_utils, CodeGeneratorConfig, Encoding, SourceInstaller};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::process::Command;
use tempfile::{tempdir, TempDir};

fn test_that_python_code_parses_with_config(
    config: &CodeGeneratorConfig,
) -> (TempDir, std::path::PathBuf) {
    let registry = test_utils::get_registry().unwrap();
    let dir = tempdir().unwrap();

    let source_path = dir.path().join("test.py");
    let mut source = File::create(&source_path).unwrap();

    let generator = python3::CodeGenerator::new(&config);
    generator.output(&mut source, &registry).unwrap();

    let python_path = format!(
        "{}:runtime/python",
        std::env::var("PYTHONPATH").unwrap_or_default()
    );
    let status = Command::new("python3")
        .arg(source_path.clone())
        .env("PYTHONPATH", python_path)
        .status()
        .unwrap();
    assert!(status.success());

    (dir, source_path.clone())
}

#[test]
fn test_that_python_code_parses() {
    let config = CodeGeneratorConfig::new("testing".to_string());
    test_that_python_code_parses_with_config(&config);
}

#[test]
fn test_that_python_code_parses_without_serialization() {
    let config = CodeGeneratorConfig::new("testing".to_string()).with_serialization(false);
    test_that_python_code_parses_with_config(&config);
}

#[test]
fn test_that_python_code_parses_with_lcs() {
    let config =
        CodeGeneratorConfig::new("testing".to_string()).with_encodings(vec![Encoding::Lcs]);
    test_that_python_code_parses_with_config(&config);
}

#[test]
fn test_that_python_code_parses_with_bincode() {
    let config =
        CodeGeneratorConfig::new("testing".to_string()).with_encodings(vec![Encoding::Bincode]);
    test_that_python_code_parses_with_config(&config);
}

#[test]
fn test_that_python_code_parses_with_comments() {
    let comments = vec![
        (
            vec!["testing".to_string(), "SerdeData".to_string()],
            "Some\ncomments".to_string(),
        ),
        (
            vec![
                "testing".to_string(),
                "List".to_string(),
                "Node".to_string(),
            ],
            "Some other comments".to_string(),
        ),
    ]
    .into_iter()
    .collect();

    let config = CodeGeneratorConfig::new("testing".to_string()).with_comments(comments);
    let (_dir, source_path) = test_that_python_code_parses_with_config(&config);

    // Check that comments were correctly generated.
    let content = std::fs::read_to_string(&source_path).unwrap();
    assert!(content.contains(
        r#"
    """Some
    comments
    """
"#
    ));
    assert!(content.contains(
        r#"
    """Some other comments
    """
"#
    ));
}

#[test]
fn test_python_code_with_external_definitions() {
    let registry = test_utils::get_registry().unwrap();
    let dir = tempdir().unwrap();

    let source_path = dir.path().join("test.py");
    let mut source = File::create(&source_path).unwrap();

    // Pretend that "Tree" is external.
    let mut definitions = BTreeMap::new();
    definitions.insert("pkg.foo".to_string(), vec!["Tree".to_string()]);
    let config =
        CodeGeneratorConfig::new("testing".to_string()).with_external_definitions(definitions);
    let generator = python3::CodeGenerator::new(&config);
    generator.output(&mut source, &registry).unwrap();

    let content = std::fs::read_to_string(&source_path).unwrap();
    assert!(content.contains("from pkg import foo"));
    assert!(content.contains("value: foo.Tree"));
    assert!(!content.contains("value: Tree"));
}

#[test]
fn test_that_installed_python_code_passes_pyre_check() {
    let registry = test_utils::get_registry().unwrap();
    let dir = tempdir().unwrap();

    let config =
        CodeGeneratorConfig::new("testing".to_string()).with_encodings(vec![Encoding::Lcs]);
    let installer = python3::Installer::new(dir.path().join("src"), /* serde package */ None);
    installer.install_module(&config, &registry).unwrap();
    installer.install_serde_runtime().unwrap();
    installer.install_bincode_runtime().unwrap();
    installer.install_lcs_runtime().unwrap();

    // Copy test files manually to type-check them as well.
    // This should go away when python runtimes are properly packaged.
    let status = Command::new("cp")
        .arg("-r")
        .arg("runtime/python/lcs/test_lcs.py")
        .arg(dir.path().join("src/lcs"))
        .status()
        .unwrap();
    assert!(status.success());
    let status = Command::new("cp")
        .arg("-r")
        .arg("runtime/python/bincode/test_bincode.py")
        .arg(dir.path().join("src/bincode"))
        .status()
        .unwrap();
    assert!(status.success());

    // Sadly, we have to manage numpy typeshed manually for now until the next release of numpy.
    let status = Command::new("cp")
        .arg("-r")
        .arg("runtime/python/typeshed")
        .arg(dir.path())
        .status()
        .unwrap();
    assert!(status.success());

    let mut pyre_config = File::create(dir.path().join(".pyre_configuration")).unwrap();
    writeln!(
        &mut pyre_config,
        r#"{{
  "source_directories": [
    "src"
  ],
  "search_path": [
    "typeshed"
  ]
}}"#,
    )
    .unwrap();

    let status = Command::new("pyre")
        .current_dir(dir.path())
        // Work around configuration issue with Pyre 0.0.53
        .arg("--typeshed")
        .arg(
            which::which("pyre")
                .unwrap()
                .parent()
                .unwrap()
                .join("../lib/pyre_check/typeshed"),
        )
        .arg("check")
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn test_python_autotest() -> std::io::Result<()> {
    let status = Command::new("python3")
        .arg("-m")
        .arg("unittest")
        .arg("discover")
        .arg("-s")
        .arg("runtime/python")
        .status()
        .unwrap();
    assert!(status.success());
    Ok(())
}