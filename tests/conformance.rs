use std::fs;
use std::path::PathBuf;

use opui::{
    ASSET_URI_RULE, CYCLE, CheckOptions, DUPLICATE_JSON_KEY, DUPLICATE_RUNTIME_ID, KEY_ORDER,
    SCHEMA, TEXT_RUN_SCALAR, UNKNOWN_CHILD, canonical_bytes, check_bytes, check_path, parse_strict,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures() -> PathBuf {
    root().join("fixtures")
}

fn valid_opui() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(fixtures())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "opui"))
        .collect();
    files.sort();
    files
}

#[test]
fn valid_fixtures_are_clean_and_canonical() {
    let files = valid_opui();
    assert_eq!(
        files.len(),
        6,
        "expected six top-level valid .opui fixtures"
    );
    for path in files {
        let diags = check_path(&path, &CheckOptions::for_path(&path));
        assert!(diags.is_empty(), "{}: {diags:#?}", path.display());
        let bytes = fs::read(&path).unwrap();
        let (value, _) = parse_strict(&bytes).unwrap();
        assert_eq!(
            canonical_bytes(&value),
            bytes,
            "{} is not canonical",
            path.display()
        );
    }
}

#[test]
fn example_is_schema_valid() {
    let bytes = fs::read(root().join("spec/opui-v1.example.json")).unwrap();
    let diags = check_bytes(
        &bytes,
        &CheckOptions {
            check_assets: true,
            check_canonical: true,
            asset_root: None,
            package_name: None,
        },
    );
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn invalid_fixtures_emit_primary_code() {
    let cases = [
        ("unsupported-schema-version.opui", SCHEMA),
        ("traversal-path.opui", ASSET_URI_RULE),
        ("duplicate-runtime-ids.opui", DUPLICATE_RUNTIME_ID),
        ("missing-references.opui", UNKNOWN_CHILD),
        ("cyclic-hierarchy.opui", CYCLE),
        ("malformed-text-runs.opui", TEXT_RUN_SCALAR),
        ("invalid-numeric-values.opui", SCHEMA),
    ];
    for (name, code) in cases {
        let path = fixtures().join("invalid").join(name);
        let diags = check_path(&path, &CheckOptions::for_path(&path));
        assert!(
            diags.iter().any(|d| d.code == code),
            "{} missing {code}: {diags:#?}",
            path.display()
        );
    }
}

#[test]
fn duplicate_keys_rejected() {
    let err = parse_strict(b"{\"a\":1,\"a\":2}\n").unwrap_err();
    assert_eq!(err.code, DUPLICATE_JSON_KEY);
}

#[test]
fn canonical_sorts_keys_and_pretty_prints() {
    let (value, _) = parse_strict(br#"{"b":1,"a":{"z":true,"m":[1,2]}}"#).unwrap();
    let out = String::from_utf8(canonical_bytes(&value)).unwrap();
    assert_eq!(
        out,
        "{\n  \"a\": {\n    \"m\": [\n      1,\n      2\n    ],\n    \"z\": true\n  },\n  \"b\": 1\n}\n"
    );
}

#[test]
fn rfc8785_number_tokens() {
    let cases = [
        ("333333333.33333329", "333333333.3333333"),
        ("1E30", "1e+30"),
        ("4.50", "4.5"),
        ("2e-3", "0.002"),
        ("1e-27", "1e-27"),
        ("42", "42"),
    ];
    for (src, expect) in cases {
        let (value, _) = parse_strict(src.as_bytes()).unwrap();
        let out = String::from_utf8(canonical_bytes(&value)).unwrap();
        assert_eq!(out, format!("{expect}\n"), "src={src}");
    }
}

#[test]
fn unsorted_keys_are_reported() {
    let diags = check_bytes(br#"{"b":1,"a":2}"#, &CheckOptions::default());
    assert!(diags.iter().any(|d| d.code == KEY_ORDER));
}

#[test]
fn raster_fallback_verifies_asset_bytes() {
    let path = fixtures().join("raster-fallback.opui");
    assert!(path.exists());
    let opts = CheckOptions::for_path(&path);
    assert!(opts.check_assets);
    assert!(
        opts.asset_root
            .as_ref()
            .is_some_and(|p| p.ends_with("raster-fallback.opui.assets"))
    );
    assert!(check_path(&path, &opts).is_empty());
}
