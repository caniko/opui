use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};

const SCHEMA_JSON: &str = include_str!("../spec/opui-v1.schema.json");

pub const SCHEMA: &str = "opui.schema";
pub const ENCODING: &str = "opui.encoding";
pub const DUPLICATE_JSON_KEY: &str = "opui.duplicate_json_key";
pub const KEY_ORDER: &str = "opui.key_order";
pub const INDENT: &str = "opui.indent";
pub const NON_FINITE: &str = "opui.non_finite";
pub const NEGATIVE_ZERO: &str = "opui.negative_zero";
pub const NUMBER_NORMALIZATION: &str = "opui.number_normalization";
pub const DUPLICATE_RUNTIME_ID: &str = "opui.duplicate_runtime_id";
pub const NODE_KEY_MISMATCH: &str = "opui.node_key_mismatch";
pub const ASSET_KEY_MISMATCH: &str = "opui.asset_key_mismatch";
pub const UNKNOWN_CHILD: &str = "opui.unknown_child";
pub const DUPLICATE_CHILD: &str = "opui.duplicate_child";
pub const MULTIPLE_PARENTS: &str = "opui.multiple_parents";
pub const CYCLE: &str = "opui.cycle";
pub const UNREACHABLE_NODE: &str = "opui.unreachable_node";
pub const UNKNOWN_ENTRYPOINT: &str = "opui.unknown_entrypoint";
pub const ENTRYPOINT_NOT_ROOT: &str = "opui.entrypoint_not_root";
pub const UNRESOLVED_ASSET: &str = "opui.unresolved_asset";
pub const UNRESOLVED_FONT: &str = "opui.unresolved_font";
pub const UNRESOLVED_IMAGE: &str = "opui.unresolved_image";
pub const ASSET_KIND_MISMATCH: &str = "opui.asset_kind_mismatch";
pub const ASSET_ID_RULE: &str = "opui.asset_id_rule";
pub const ASSET_URI_RULE: &str = "opui.asset_uri_rule";
pub const ASSET_MIME_MAPPING: &str = "opui.asset_mime_mapping";
pub const ASSET_HASH_MISMATCH: &str = "opui.asset_hash_mismatch";
pub const ASSET_BYTE_LENGTH: &str = "opui.asset_byte_length";
pub const ASSET_MISSING: &str = "opui.asset_missing";
pub const NETWORK_URI: &str = "opui.network_uri";
pub const EMBEDDED_DATA: &str = "opui.embedded_data";
pub const SVG_SCRIPT: &str = "opui.svg_script";
pub const SVG_ANIMATION: &str = "opui.svg_animation";
pub const SVG_EVENT: &str = "opui.svg_event";
pub const SVG_FOREIGN_OBJECT: &str = "opui.svg_foreign_object";
pub const SVG_DTD: &str = "opui.svg_dtd";
pub const SVG_CSS: &str = "opui.svg_css";
pub const SVG_EXTERNAL_REF: &str = "opui.svg_external_ref";
pub const SVG_INVALID: &str = "opui.svg_invalid";
pub const TEXT_RUN_RANGE: &str = "opui.text_run_range";
pub const TEXT_RUN_OVERLAP: &str = "opui.text_run_overlap";
pub const TEXT_RUN_EMPTY: &str = "opui.text_run_empty";
pub const TEXT_RUN_ORDER: &str = "opui.text_run_order";
pub const TEXT_RUN_SCALAR: &str = "opui.text_run_scalar";
pub const NINE_SLICE_BOUNDS: &str = "opui.nine_slice_bounds";
pub const GRADIENT_STOP_ORDER: &str = "opui.gradient_stop_order";
pub const FALLBACK_DIAGNOSTIC_COUNT: &str = "opui.fallback_diagnostic_count";
pub const DIAGNOSTIC_ORDER: &str = "opui.diagnostic_order";
pub const DIAGNOSTIC_STRATEGY: &str = "opui.diagnostic_strategy";
pub const DIAGNOSTIC_NODE: &str = "opui.diagnostic_node";
pub const DIAGNOSTIC_RUNTIME_ID: &str = "opui.diagnostic_runtime_id";
pub const PACKAGE_NAME: &str = "opui.package_name";

const IJSON_MAX: i64 = 9_007_199_254_740_991;

const ASSET_ROWS: &[(&str, &str, &str, &str)] = &[
    ("image", "images", "png", "image/png"),
    ("image", "images", "jpg", "image/jpeg"),
    ("image", "images", "webp", "image/webp"),
    ("font", "fonts", "ttf", "font/ttf"),
    ("font", "fonts", "otf", "font/otf"),
    ("font", "fonts", "woff2", "font/woff2"),
    ("vector", "vectors", "svg", "image/svg+xml"),
    ("raster_fallback", "fallback", "png", "image/png"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: String,
    pub node_id: Option<String>,
    pub runtime_id: Option<String>,
    pub message: String,
    pub strategy: Option<String>,
    pub details: Map<String, Value>,
    pub path: String,
}

impl Diagnostic {
    pub fn error(code: &str, loc: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            severity: "error".into(),
            node_id: None,
            runtime_id: None,
            message: message.into(),
            strategy: Some("none".into()),
            details: Map::new(),
            path: loc.into(),
        }
    }

    fn with_node(mut self, id: impl Into<String>) -> Self {
        self.node_id = Some(id.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct CheckOptions {
    pub check_assets: bool,
    pub check_canonical: bool,
    pub asset_root: Option<PathBuf>,
    pub package_name: Option<String>,
}

impl Default for CheckOptions {
    fn default() -> Self {
        Self {
            check_assets: true,
            check_canonical: true,
            asset_root: None,
            package_name: None,
        }
    }
}

impl CheckOptions {
    pub fn for_path(path: &Path) -> Self {
        Self {
            check_assets: true,
            check_canonical: true,
            asset_root: Some(asset_root_for(path)),
            package_name: package_name_for(path),
        }
    }
}

pub fn asset_root_for(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.parent()
        .unwrap_or(Path::new("."))
        .join(format!("{name}.assets"))
}

pub fn package_name_for(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".opui"))
        .filter(|stem| !stem.is_empty())
        .map(str::to_string)
}

pub fn check_path(path: &Path, opts: &CheckOptions) -> Vec<Diagnostic> {
    let mut opts = opts.clone();
    if opts.asset_root.is_none() {
        opts.asset_root = Some(asset_root_for(path));
    }
    if opts.package_name.is_none() {
        opts.package_name = package_name_for(path);
    }
    let mut diags = match fs::read(path) {
        Ok(bytes) => check_bytes(&bytes, &opts),
        Err(err) => vec![Diagnostic::error(ENCODING, "", err.to_string())],
    };
    if package_name_for(path).is_none() {
        diags.push(Diagnostic::error(
            PACKAGE_NAME,
            "",
            "manifest filename must be <name>.opui",
        ));
        sort_diagnostics(&mut diags);
    }
    diags
}

pub fn check_bytes(bytes: &[u8], opts: &CheckOptions) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    encoding_precheck(bytes, &mut diags);
    let (value, parse_flags) = match parse_strict(bytes) {
        Ok(parsed) => parsed,
        Err(diag) => {
            diags.push(diag);
            sort_diagnostics(&mut diags);
            return diags;
        }
    };
    if parse_flags.unsorted {
        diags.push(Diagnostic::error(
            KEY_ORDER,
            "",
            "object keys are not sorted",
        ));
    }
    if parse_flags.neg_zero {
        diags.push(Diagnostic::error(
            NEGATIVE_ZERO,
            "",
            "negative zero is invalid",
        ));
    }
    if parse_flags.non_finite {
        diags.push(Diagnostic::error(NON_FINITE, "", "non-finite number"));
    }
    schema_check(&value, &mut diags);
    semantic_check(&value, opts, &mut diags);
    if opts.check_canonical {
        canonical_check(bytes, &value, &mut diags);
    }
    sort_diagnostics(&mut diags);
    diags
}

pub fn format_diagnostics(diags: &[Diagnostic]) -> String {
    let mut lines = String::new();
    for d in diags {
        lines.push_str(&format!(
            "{severity}  {code}  {node}  {runtime}  {strategy}  {path}  {message}\n",
            severity = d.severity,
            code = d.code,
            node = d.node_id.as_deref().unwrap_or("-"),
            runtime = d.runtime_id.as_deref().unwrap_or("-"),
            strategy = d.strategy.as_deref().unwrap_or("-"),
            path = if d.path.is_empty() {
                "-"
            } else {
                d.path.as_str()
            },
            message = d.message
        ));
    }
    lines
}

pub fn is_runtime_loadable(diags: &[Diagnostic]) -> bool {
    !diags.iter().any(|d| d.severity == "error")
}

struct ParseFlags {
    unsorted: bool,
    neg_zero: bool,
    non_finite: bool,
}

struct Strict {
    value: Value,
    flags: ParseFlags,
}

impl Strict {
    fn leaf(value: Value) -> Self {
        Self {
            value,
            flags: ParseFlags {
                unsorted: false,
                neg_zero: false,
                non_finite: false,
            },
        }
    }

    fn merge_flags(&mut self, other: &ParseFlags) {
        self.flags.unsorted |= other.unsorted;
        self.flags.neg_zero |= other.neg_zero;
        self.flags.non_finite |= other.non_finite;
    }
}

#[allow(clippy::result_large_err)]
pub fn parse_strict(bytes: &[u8]) -> Result<(Value, ParseFlagsPublic), Diagnostic> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    match Strict::deserialize(&mut de) {
        Ok(parsed) => de.end().map_err(parse_err).map(|()| {
            (
                parsed.value,
                ParseFlagsPublic {
                    unsorted: parsed.flags.unsorted,
                    neg_zero: parsed.flags.neg_zero,
                    non_finite: parsed.flags.non_finite,
                },
            )
        }),
        Err(err) => Err(parse_err(err)),
    }
}

#[derive(Debug)]
pub struct ParseFlagsPublic {
    pub unsorted: bool,
    pub neg_zero: bool,
    pub non_finite: bool,
}

fn parse_err(err: serde_json::Error) -> Diagnostic {
    let msg = err.to_string();
    if let Some(key) = msg.strip_prefix("duplicate object key ") {
        return Diagnostic::error(
            DUPLICATE_JSON_KEY,
            "",
            format!("duplicate object key {key}"),
        );
    }
    Diagnostic::error(ENCODING, "", msg)
}

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Strict;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        Ok(Strict::leaf(Value::Bool(v)))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        Ok(Strict::leaf(Value::Number(v.into())))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(Strict::leaf(Value::Number(v.into())))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        let mut flags = ParseFlags {
            unsorted: false,
            neg_zero: v == 0.0 && v.is_sign_negative(),
            non_finite: !v.is_finite(),
        };
        if flags.neg_zero {
            return Ok(Strict {
                value: Value::Number(0.into()),
                flags,
            });
        }
        if !v.is_finite() {
            flags.non_finite = true;
            return Ok(Strict {
                value: Value::Null,
                flags,
            });
        }
        let number = Number::from_f64(v).unwrap_or_else(|| 0.into());
        Ok(Strict {
            value: Value::Number(number),
            flags,
        })
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(Strict::leaf(Value::String(v.to_string())))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        Ok(Strict::leaf(Value::String(v)))
    }

    fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(Strict::leaf(Value::Null))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        Ok(Strict::leaf(Value::Null))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        let mut acc = Strict::leaf(Value::Null);
        while let Some(item) = seq.next_element::<Strict>()? {
            acc.merge_flags(&item.flags);
            items.push(item.value);
        }
        acc.value = Value::Array(items);
        Ok(acc)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut obj = Map::new();
        let mut acc = Strict::leaf(Value::Null);
        let mut last: Option<String> = None;
        while let Some(key) = map.next_key::<String>()? {
            if obj.contains_key(&key) {
                return Err(de::Error::custom(format!("duplicate object key {key}")));
            }
            if last
                .as_ref()
                .is_some_and(|prev| key.as_str() < prev.as_str())
            {
                acc.flags.unsorted = true;
            }
            last = Some(key.clone());
            let val = map.next_value::<Strict>()?;
            acc.merge_flags(&val.flags);
            obj.insert(key, val.value);
        }
        acc.value = Value::Object(obj);
        Ok(acc)
    }
}

pub fn canonical_bytes(value: &Value) -> Vec<u8> {
    let mut out = String::new();
    write_canonical(&mut out, value, 0);
    out.push('\n');
    out.into_bytes()
}

fn write_canonical(out: &mut String, value: &Value, indent: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&jcs_number(n)),
        Value::String(s) => write_json_string(out, s),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                out.push('\n');
                push_indent(out, indent + 1);
                write_canonical(out, item, indent + 1);
                if i + 1 != items.len() {
                    out.push(',');
                }
            }
            out.push('\n');
            push_indent(out, indent);
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                out.push('\n');
                push_indent(out, indent + 1);
                write_json_string(out, key);
                out.push_str(": ");
                write_canonical(out, &map[*key], indent + 1);
                if i + 1 != keys.len() {
                    out.push(',');
                }
            }
            out.push('\n');
            push_indent(out, indent);
            out.push('}');
        }
    }
}

fn push_indent(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push_str("  ");
    }
}

fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii_control() => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn jcs_number(n: &Number) -> String {
    if let Some(i) = n.as_i64() {
        return i.to_string();
    }
    if let Some(u) = n.as_u64() {
        return u.to_string();
    }
    let f = n.as_f64().unwrap_or(0.0);
    if f == 0.0 {
        return "0".into();
    }
    let mut buf = ryu_js::Buffer::new();
    buf.format_finite(f).to_string()
}

fn encoding_precheck(bytes: &[u8], diags: &mut Vec<Diagnostic>) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        diags.push(Diagnostic::error(ENCODING, "", "UTF-8 BOM is invalid"));
    }
    if std::str::from_utf8(bytes).is_err() {
        diags.push(Diagnostic::error(ENCODING, "", "file is not UTF-8"));
        return;
    }
    if bytes.contains(&b'\r') {
        diags.push(Diagnostic::error(ENCODING, "", "CR is invalid; use LF"));
    }
    if bytes.contains(&b'\t') {
        diags.push(Diagnostic::error(INDENT, "", "tabs are invalid"));
    }
    if bytes.is_empty() || !bytes.ends_with(b"\n") || bytes.ends_with(b"\n\n") {
        diags.push(Diagnostic::error(
            ENCODING,
            "",
            "file must end with exactly one LF",
        ));
    }
}

fn schema_check(value: &Value, diags: &mut Vec<Diagnostic>) {
    let validator = compiled_schema();
    for error in validator.iter_errors(value) {
        diags.push(Diagnostic::error(
            SCHEMA,
            instance_pointer(error.instance_path()),
            error.to_string(),
        ));
    }
}

fn compiled_schema() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR.get_or_init(|| {
        let schema: Value = serde_json::from_str(SCHEMA_JSON).expect("opui schema json");
        jsonschema::draft202012::new(&schema).expect("opui schema")
    })
}

fn instance_pointer(path: impl std::fmt::Display) -> String {
    let text = path.to_string();
    if text.is_empty() || text.starts_with('/') {
        text
    } else {
        format!("/{text}")
    }
}

fn semantic_check(value: &Value, opts: &CheckOptions, diags: &mut Vec<Diagnostic>) {
    walk_ijson_numbers(value, "", diags);
    let Some(root) = value.as_object() else {
        return;
    };
    let nodes = root.get("nodes").and_then(Value::as_object);
    let assets = root.get("assets").and_then(Value::as_object);
    if let Some(nodes) = nodes {
        check_nodes(nodes, assets, diags);
    }
    if let Some(assets) = assets {
        check_assets(assets, opts, diags);
    }
    if let (Some(nodes), Some(entrypoints)) =
        (nodes, root.get("entrypoints").and_then(Value::as_object))
    {
        check_forest(nodes, entrypoints, diags);
    }
    check_package_name(root, opts, diags);
    check_file_diagnostics(root, nodes, diags);
}

fn walk_ijson_numbers(value: &Value, pointer: &str, diags: &mut Vec<Diagnostic>) {
    match value {
        Value::Number(n) => {
            if n.as_f64().is_some_and(|f| !f.is_finite()) {
                diags.push(Diagnostic::error(NON_FINITE, pointer, "non-finite number"));
            } else if let Some(i) = n.as_i64() {
                if i.unsigned_abs() > IJSON_MAX as u64 {
                    diags.push(Diagnostic::error(
                        NUMBER_NORMALIZATION,
                        pointer,
                        "integer exceeds I-JSON range",
                    ));
                }
            } else if let Some(u) = n.as_u64()
                && u > IJSON_MAX as u64
            {
                diags.push(Diagnostic::error(
                    NUMBER_NORMALIZATION,
                    pointer,
                    "integer exceeds I-JSON range",
                ));
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                walk_ijson_numbers(item, &format!("{pointer}/{i}"), diags);
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                walk_ijson_numbers(v, &format!("{pointer}/{}", escape_pointer(k)), diags);
            }
        }
        _ => {}
    }
}

fn escape_pointer(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

fn check_nodes(
    nodes: &Map<String, Value>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    let mut seen_runtime: HashMap<&str, &str> = HashMap::new();
    for (key, node) in nodes {
        let path = format!("/nodes/{}", escape_pointer(key));
        let Some(obj) = node.as_object() else {
            continue;
        };
        if obj.get("source_id").and_then(Value::as_str) != Some(key.as_str()) {
            diags.push(
                Diagnostic::error(
                    NODE_KEY_MISMATCH,
                    format!("{path}/source_id"),
                    "nodes key must equal source_id",
                )
                .with_node(key),
            );
        }
        if let Some(rid) = obj.get("runtime_id").and_then(Value::as_str)
            && let Some(prev) = seen_runtime.insert(rid, key)
        {
            diags.push(
                Diagnostic::error(
                    DUPLICATE_RUNTIME_ID,
                    format!("{path}/runtime_id"),
                    format!("runtime_id `{rid}` already used by `{prev}`"),
                )
                .with_node(key),
            );
        }
        match obj.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = obj.get("text") {
                    check_text(key, &format!("{path}/text"), text, assets, diags);
                }
            }
            Some("image") => {
                if let Some(image) = obj.get("image") {
                    check_image(key, &format!("{path}/image"), image, assets, diags);
                }
            }
            Some("fallback") => {
                if let Some(fb) = obj.get("fallback") {
                    check_fallback_asset(key, &format!("{path}/fallback"), fb, assets, diags);
                }
            }
            _ => {}
        }
        check_gradient_stops(key, &path, obj, diags);
    }
}

fn check_gradient_stops(
    node_id: &str,
    path: &str,
    obj: &Map<String, Value>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(fill) = obj.get("style").and_then(|s| s.get("fill")) else {
        return;
    };
    let ty = fill.get("type").and_then(Value::as_str);
    if !matches!(ty, Some("linear") | Some("radial")) {
        return;
    }
    let Some(stops) = fill.get("stops").and_then(Value::as_array) else {
        return;
    };
    let mut prev: Option<f64> = None;
    for stop in stops {
        let Some(offset) = stop.get("offset").and_then(Value::as_f64) else {
            continue;
        };
        if prev.is_some_and(|p| offset < p) {
            diags.push(
                Diagnostic::error(
                    GRADIENT_STOP_ORDER,
                    format!("{path}/style/fill/stops"),
                    "gradient stops must be nondecreasing by offset",
                )
                .with_node(node_id),
            );
            return;
        }
        prev = Some(offset);
    }
}

fn check_text(
    node_id: &str,
    path: &str,
    text: &Value,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(obj) = text.as_object() else {
        return;
    };
    let content = obj.get("content").and_then(Value::as_str).unwrap_or("");
    let bytes = content.as_bytes();
    if let Some(defaults) = obj.get("defaults") {
        check_font_ref(
            node_id,
            &format!("{path}/defaults"),
            defaults,
            assets,
            diags,
        );
    }
    let Some(runs) = obj.get("runs").and_then(Value::as_array) else {
        return;
    };
    let mut prev_start: Option<u64> = None;
    let mut prev_end: Option<u64> = None;
    for (i, run) in runs.iter().enumerate() {
        let run_path = format!("{path}/runs/{i}");
        let start = run.get("start").and_then(Value::as_u64).unwrap_or(0);
        let end = run.get("end").and_then(Value::as_u64).unwrap_or(0);
        if start == end {
            diags.push(
                Diagnostic::error(TEXT_RUN_EMPTY, &run_path, "text run is empty")
                    .with_node(node_id),
            );
        } else if start > end || end > bytes.len() as u64 {
            diags.push(
                Diagnostic::error(TEXT_RUN_RANGE, &run_path, "text run is outside content")
                    .with_node(node_id),
            );
        } else {
            if !is_char_boundary(bytes, start as usize) || !is_char_boundary(bytes, end as usize) {
                diags.push(
                    Diagnostic::error(TEXT_RUN_SCALAR, &run_path, "text run splits a UTF-8 scalar")
                        .with_node(node_id),
                );
            }
        }
        if let Some(ps) = prev_start
            && start < ps
        {
            diags.push(
                Diagnostic::error(
                    TEXT_RUN_ORDER,
                    &run_path,
                    "text runs must be sorted by start",
                )
                .with_node(node_id),
            );
        }
        if let Some(pe) = prev_end
            && start < pe
            && start >= prev_start.unwrap_or(0)
        {
            diags.push(
                Diagnostic::error(TEXT_RUN_OVERLAP, &run_path, "text runs overlap")
                    .with_node(node_id),
            );
        }
        prev_start = Some(start);
        prev_end = Some(end);
        if let Some(style) = run.get("style") {
            check_font_ref(node_id, &format!("{run_path}/style"), style, assets, diags);
        }
    }
}

fn is_char_boundary(bytes: &[u8], i: usize) -> bool {
    i == bytes.len() || std::str::from_utf8(bytes).is_ok_and(|s| s.is_char_boundary(i))
}

fn check_font_ref(
    node_id: &str,
    path: &str,
    style: &Value,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(font) = style.get("font").and_then(Value::as_str) else {
        return;
    };
    match asset_kind(assets, font) {
        None => diags.push(
            Diagnostic::error(
                UNRESOLVED_FONT,
                format!("{path}/font"),
                format!("unknown font `{font}`"),
            )
            .with_node(node_id),
        ),
        Some(kind) if kind != "font" => diags.push(
            Diagnostic::error(
                ASSET_KIND_MISMATCH,
                format!("{path}/font"),
                "font must reference a font asset",
            )
            .with_node(node_id),
        ),
        Some(_) => {}
    }
}

fn check_image(
    node_id: &str,
    path: &str,
    image: &Value,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(id) = image.get("asset").and_then(Value::as_str) else {
        return;
    };
    match asset_kind(assets, id) {
        None => diags.push(
            Diagnostic::error(
                UNRESOLVED_IMAGE,
                format!("{path}/asset"),
                format!("unknown image `{id}`"),
            )
            .with_node(node_id),
        ),
        Some(kind) if kind != "image" && kind != "vector" => diags.push(
            Diagnostic::error(
                ASSET_KIND_MISMATCH,
                format!("{path}/asset"),
                "image must reference an image or vector asset",
            )
            .with_node(node_id),
        ),
        Some(_) => {}
    }
    if let Some(ns) = image.get("nine_slice").filter(|v| !v.is_null()) {
        let w = image
            .get("intrinsic_width")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let h = image
            .get("intrinsic_height")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let top = ns.get("top").and_then(Value::as_u64).unwrap_or(0);
        let bottom = ns.get("bottom").and_then(Value::as_u64).unwrap_or(0);
        let left = ns.get("left").and_then(Value::as_u64).unwrap_or(0);
        let right = ns.get("right").and_then(Value::as_u64).unwrap_or(0);
        if top + bottom >= h || left + right >= w {
            diags.push(
                Diagnostic::error(
                    NINE_SLICE_BOUNDS,
                    format!("{path}/nine_slice"),
                    "nine_slice exceeds intrinsic size",
                )
                .with_node(node_id),
            );
        }
    }
}

fn check_fallback_asset(
    node_id: &str,
    path: &str,
    fallback: &Value,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(id) = fallback.get("asset").and_then(Value::as_str) else {
        return;
    };
    match asset_kind(assets, id) {
        None => diags.push(
            Diagnostic::error(
                UNRESOLVED_ASSET,
                format!("{path}/asset"),
                format!("unknown asset `{id}`"),
            )
            .with_node(node_id),
        ),
        Some(kind) if kind != "raster_fallback" => diags.push(
            Diagnostic::error(
                ASSET_KIND_MISMATCH,
                format!("{path}/asset"),
                "fallback must reference a raster_fallback asset",
            )
            .with_node(node_id),
        ),
        Some(_) => {}
    }
}

fn asset_kind<'a>(assets: Option<&'a Map<String, Value>>, id: &str) -> Option<&'a str> {
    assets?.get(id)?.get("kind")?.as_str()
}

fn check_forest(
    nodes: &Map<String, Value>,
    entrypoints: &Map<String, Value>,
    diags: &mut Vec<Diagnostic>,
) {
    let mut parent: HashMap<&str, &str> = HashMap::new();
    for (id, node) in nodes {
        let Some(children) = node.get("children").and_then(Value::as_array) else {
            continue;
        };
        let mut seen = HashSet::new();
        for (i, child) in children.iter().enumerate() {
            let Some(cid) = child.as_str() else {
                continue;
            };
            let cpath = format!("/nodes/{}/children/{i}", escape_pointer(id));
            if !seen.insert(cid) {
                diags.push(
                    Diagnostic::error(DUPLICATE_CHILD, cpath, format!("duplicate child `{cid}`"))
                        .with_node(id),
                );
                continue;
            }
            if !nodes.contains_key(cid) {
                diags.push(
                    Diagnostic::error(UNKNOWN_CHILD, cpath, format!("unknown child `{cid}`"))
                        .with_node(id),
                );
                continue;
            }
            if let Some(existing) = parent.insert(cid, id.as_str()) {
                diags.push(
                    Diagnostic::error(
                        MULTIPLE_PARENTS,
                        format!("/nodes/{}", escape_pointer(cid)),
                        format!("`{cid}` already has parent `{existing}`"),
                    )
                    .with_node(cid),
                );
            }
        }
    }

    if let Some(cycle_id) = find_cycle(nodes, &parent) {
        diags.push(
            Diagnostic::error(
                CYCLE,
                format!("/nodes/{}", escape_pointer(cycle_id)),
                "child graph is cyclic",
            )
            .with_node(cycle_id),
        );
    }

    let mut reachable = HashSet::new();
    for (name, target) in entrypoints {
        let epath = format!("/entrypoints/{}", escape_pointer(name));
        let Some(tid) = target.as_str() else {
            continue;
        };
        if !nodes.contains_key(tid) {
            diags.push(Diagnostic::error(
                UNKNOWN_ENTRYPOINT,
                epath,
                format!("unknown entrypoint `{tid}`"),
            ));
            continue;
        }
        if parent.contains_key(tid) {
            diags.push(
                Diagnostic::error(
                    ENTRYPOINT_NOT_ROOT,
                    epath,
                    format!("entrypoint `{tid}` is not a root"),
                )
                .with_node(tid),
            );
        }
        walk_reachable(tid, nodes, &mut reachable);
    }
    for id in nodes.keys() {
        if !reachable.contains(id.as_str()) {
            diags.push(
                Diagnostic::error(
                    UNREACHABLE_NODE,
                    format!("/nodes/{}", escape_pointer(id)),
                    format!("`{id}` is not reachable from an entrypoint"),
                )
                .with_node(id),
            );
        }
    }
}

fn find_cycle<'a>(nodes: &'a Map<String, Value>, _parent: &HashMap<&str, &str>) -> Option<&'a str> {
    let mut state: HashMap<&str, u8> = HashMap::new();
    fn dfs<'a>(
        id: &'a str,
        nodes: &'a Map<String, Value>,
        state: &mut HashMap<&'a str, u8>,
    ) -> Option<&'a str> {
        state.insert(id, 1);
        if let Some(children) = nodes
            .get(id)
            .and_then(|n| n.get("children"))
            .and_then(Value::as_array)
        {
            for child in children {
                let Some(cid) = child.as_str() else {
                    continue;
                };
                match state.get(cid).copied() {
                    Some(1) => return Some(cid),
                    Some(2) => {}
                    _ => {
                        if let Some(hit) = dfs(cid, nodes, state) {
                            return Some(hit);
                        }
                    }
                }
            }
        }
        state.insert(id, 2);
        None
    }
    for id in nodes.keys() {
        if !state.contains_key(id.as_str())
            && let Some(hit) = dfs(id, nodes, &mut state)
        {
            return Some(hit);
        }
    }
    None
}

fn walk_reachable<'a>(id: &'a str, nodes: &'a Map<String, Value>, seen: &mut HashSet<&'a str>) {
    if !seen.insert(id) {
        return;
    }
    let Some(children) = nodes
        .get(id)
        .and_then(|n| n.get("children"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for child in children {
        if let Some(cid) = child.as_str() {
            walk_reachable(cid, nodes, seen);
        }
    }
}

fn check_assets(assets: &Map<String, Value>, opts: &CheckOptions, diags: &mut Vec<Diagnostic>) {
    for (key, asset) in assets {
        let path = format!("/assets/{}", escape_pointer(key));
        let Some(obj) = asset.as_object() else {
            continue;
        };
        let kind = obj.get("kind").and_then(Value::as_str).unwrap_or("");
        let sha = obj.get("sha256").and_then(Value::as_str).unwrap_or("");
        let uri = obj.get("uri").and_then(Value::as_str).unwrap_or("");
        let mime = obj.get("mime_type").and_then(Value::as_str).unwrap_or("");
        let expected_id = format!("{kind}-{sha}");
        if key != &expected_id {
            diags.push(Diagnostic::error(
                ASSET_KEY_MISMATCH,
                path.clone(),
                "assets key must be <kind>-<sha256>",
            ));
        }
        if !sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) || sha.len() != 64 {
            diags.push(Diagnostic::error(
                ASSET_ID_RULE,
                format!("{path}/sha256"),
                "sha256 must be 64 lowercase hex",
            ));
        }
        if uri.starts_with("data:") {
            diags.push(Diagnostic::error(
                EMBEDDED_DATA,
                format!("{path}/uri"),
                "data URIs are forbidden",
            ));
        } else if uri.contains("://")
            || uri.starts_with("//")
            || uri.starts_with("file:")
            || uri.starts_with("http:")
        {
            diags.push(Diagnostic::error(
                NETWORK_URI,
                format!("{path}/uri"),
                "network URIs are forbidden",
            ));
        }
        if uri.contains('\0') || uri.contains('\\') || uri.starts_with('/') || uri.contains("..") {
            diags.push(Diagnostic::error(
                ASSET_URI_RULE,
                format!("{path}/uri"),
                "unsafe asset URI",
            ));
        }
        let parsed = parse_asset_uri(uri);
        match parsed {
            None => {
                if !uri.is_empty() && !uri.starts_with("data:") && !uri.contains("://") {
                    diags.push(Diagnostic::error(
                        ASSET_URI_RULE,
                        format!("{path}/uri"),
                        "URI must be <dir>/<sha256>.<ext>",
                    ));
                }
            }
            Some((dir, file_sha, ext)) => {
                if file_sha != sha {
                    diags.push(Diagnostic::error(
                        ASSET_ID_RULE,
                        format!("{path}/uri"),
                        "URI hash must equal sha256",
                    ));
                }
                let row = ASSET_ROWS
                    .iter()
                    .find(|row| row.0 == kind && row.1 == dir && row.2 == ext);
                if row.is_none() {
                    diags.push(Diagnostic::error(
                        ASSET_URI_RULE,
                        format!("{path}/uri"),
                        "kind/dir/ext do not match",
                    ));
                } else if row.is_some_and(|row| row.3 != mime) {
                    diags.push(Diagnostic::error(
                        ASSET_MIME_MAPPING,
                        format!("{path}/mime_type"),
                        "mime_type does not match kind/ext",
                    ));
                }
            }
        }
        if opts.check_assets
            && let Some(root) = &opts.asset_root
        {
            check_asset_bytes(root, key, obj, &path, diags);
        }
    }
}

fn parse_asset_uri(uri: &str) -> Option<(&str, &str, &str)> {
    let (dir, rest) = uri.split_once('/')?;
    let (stem, ext) = rest.rsplit_once('.')?;
    if rest.contains('/') {
        return None;
    }
    Some((dir, stem, ext))
}

fn check_asset_bytes(
    asset_root: &Path,
    key: &str,
    obj: &Map<String, Value>,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(uri) = obj.get("uri").and_then(Value::as_str) else {
        return;
    };
    if uri.is_empty() || uri.contains('\0') || uri.contains("://") || uri.starts_with("data:") {
        return;
    }
    let joined = asset_root.join(uri);
    if !joined.starts_with(asset_root)
        || joined.strip_prefix(asset_root).is_ok_and(|rel| {
            rel.components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        })
    {
        diags.push(Diagnostic::error(
            ASSET_MISSING,
            format!("{path}/uri"),
            "sidecar is not under the asset root",
        ));
        return;
    }
    match fs::symlink_metadata(asset_root) {
        Ok(m) if m.file_type().is_symlink() => {
            diags.push(Diagnostic::error(
                ASSET_MISSING,
                format!("{path}/uri"),
                "asset root must not be a symlink",
            ));
            return;
        }
        Ok(_) => {}
        Err(_) => {
            diags.push(Diagnostic::error(
                ASSET_MISSING,
                format!("{path}/uri"),
                "sidecar file is missing",
            ));
            return;
        }
    }
    let Ok(canon_root) = fs::canonicalize(asset_root) else {
        diags.push(Diagnostic::error(
            ASSET_MISSING,
            format!("{path}/uri"),
            "sidecar file is missing",
        ));
        return;
    };
    let meta = match fs::symlink_metadata(&joined) {
        Ok(m) => m,
        Err(_) => {
            diags.push(Diagnostic::error(
                ASSET_MISSING,
                format!("{path}/uri"),
                "sidecar file is missing",
            ));
            return;
        }
    };
    if meta.file_type().is_symlink() || !meta.is_file() {
        diags.push(Diagnostic::error(
            ASSET_MISSING,
            format!("{path}/uri"),
            "sidecar must be a regular file, not a symlink",
        ));
        return;
    }
    let Ok(canon_file) = fs::canonicalize(&joined) else {
        diags.push(Diagnostic::error(
            ASSET_MISSING,
            format!("{path}/uri"),
            "sidecar file is unreadable",
        ));
        return;
    };
    if !canon_file.starts_with(&canon_root) {
        diags.push(Diagnostic::error(
            ASSET_MISSING,
            format!("{path}/uri"),
            "sidecar is not under the asset root",
        ));
        return;
    }
    let Ok(bytes) = fs::read(&joined) else {
        diags.push(Diagnostic::error(
            ASSET_MISSING,
            format!("{path}/uri"),
            "sidecar file is unreadable",
        ));
        return;
    };
    let expected_len = obj.get("byte_length").and_then(Value::as_u64).unwrap_or(0);
    if bytes.len() as u64 != expected_len {
        diags.push(Diagnostic::error(
            ASSET_BYTE_LENGTH,
            format!("{path}/byte_length"),
            "byte_length does not match file",
        ));
    }
    let digest = hex_lower(&Sha256::digest(&bytes));
    if obj.get("sha256").and_then(Value::as_str) != Some(digest.as_str()) {
        diags.push(Diagnostic::error(
            ASSET_HASH_MISMATCH,
            format!("{path}/sha256"),
            "SHA-256 does not match file",
        ));
    }
    if obj.get("kind").and_then(Value::as_str) == Some("vector") {
        match std::str::from_utf8(&bytes) {
            Ok(text) => check_svg(text, key, path, diags),
            Err(_) => {
                diags.push(Diagnostic::error(
                    SVG_INVALID,
                    path,
                    "vector asset is not UTF-8",
                ));
            }
        }
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

fn check_svg(text: &str, _asset_id: &str, path: &str, diags: &mut Vec<Diagnostic>) {
    let lower = text.to_ascii_lowercase();
    if lower.contains("<!doctype") || lower.contains("<!entity") || has_forbidden_entity(text) {
        diags.push(Diagnostic::error(
            SVG_DTD,
            path,
            "SVG contains DTD or custom entities",
        ));
    }
    if lower.contains("<?xml-stylesheet") {
        diags.push(Diagnostic::error(SVG_CSS, path, "SVG contains CSS"));
    }
    let Ok(doc) = roxmltree::Document::parse(text) else {
        diags.push(Diagnostic::error(
            SVG_INVALID,
            path,
            "vector asset is not well-formed XML",
        ));
        return;
    };
    let mut saw_script = false;
    let mut saw_anim = false;
    let mut saw_event = false;
    let mut saw_foreign = false;
    let mut saw_css = false;
    let mut saw_ext = false;
    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let tag = node.tag_name().name().to_ascii_lowercase();
        if !saw_script && tag == "script" {
            saw_script = true;
            diags.push(Diagnostic::error(SVG_SCRIPT, path, "SVG contains script"));
        }
        if !saw_anim && (tag == "set" || tag == "mpath" || tag.starts_with("animate")) {
            saw_anim = true;
            diags.push(Diagnostic::error(
                SVG_ANIMATION,
                path,
                "SVG contains animation",
            ));
        }
        if !saw_foreign && tag == "foreignobject" {
            saw_foreign = true;
            diags.push(Diagnostic::error(
                SVG_FOREIGN_OBJECT,
                path,
                "SVG contains foreignObject",
            ));
        }
        if !saw_css && tag == "style" {
            saw_css = true;
            diags.push(Diagnostic::error(SVG_CSS, path, "SVG contains CSS"));
        }
        for attr in node.attributes() {
            let an = attr.name().to_ascii_lowercase();
            if !saw_event && an.starts_with("on") && an.len() > 2 {
                saw_event = true;
                diags.push(Diagnostic::error(
                    SVG_EVENT,
                    path,
                    "SVG contains event attributes",
                ));
            }
            if !saw_css && an == "style" {
                saw_css = true;
                diags.push(Diagnostic::error(SVG_CSS, path, "SVG contains CSS"));
            }
            if an == "href" {
                let v = attr.value().trim();
                if v.to_ascii_lowercase().starts_with("javascript:") {
                    if !saw_script {
                        saw_script = true;
                        diags.push(Diagnostic::error(SVG_SCRIPT, path, "SVG contains script"));
                    }
                } else if !saw_ext && !is_fragment_ref(v) {
                    saw_ext = true;
                    diags.push(Diagnostic::error(
                        SVG_EXTERNAL_REF,
                        path,
                        "SVG has a non-fragment reference",
                    ));
                }
            }
            if !saw_ext {
                for url in css_urls(attr.value()) {
                    if !is_fragment_ref(&url) {
                        saw_ext = true;
                        diags.push(Diagnostic::error(
                            SVG_EXTERNAL_REF,
                            path,
                            "SVG has a non-fragment reference",
                        ));
                        break;
                    }
                }
            }
        }
    }
}

fn has_forbidden_entity(text: &str) -> bool {
    let mut rest = text;
    while let Some(idx) = rest.find('&') {
        rest = &rest[idx + 1..];
        if rest.starts_with('#') {
            continue;
        }
        let end = rest.find(';').unwrap_or(rest.len());
        let name = &rest[..end];
        if !matches!(name, "lt" | "gt" | "amp" | "apos" | "quot") {
            return true;
        }
    }
    false
}

fn css_urls(value: &str) -> Vec<String> {
    let lower = value.to_ascii_lowercase();
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        if lower.as_bytes()[i..].starts_with(b"url")
            && (i == 0 || !bytes[i - 1].is_ascii_alphabetic())
        {
            let mut j = i + 3;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'(' {
                j += 1;
                let rest = &value[j..];
                let inner = rest.split(')').next().unwrap_or(rest);
                out.push(inner.trim().trim_matches(['\'', '"']).to_string());
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn is_fragment_ref(value: &str) -> bool {
    let v = value.trim();
    v.starts_with('#') && !v[1..].contains([':', '/'])
}

fn check_package_name(root: &Map<String, Value>, opts: &CheckOptions, diags: &mut Vec<Diagnostic>) {
    let Some(expected) = opts.package_name.as_deref() else {
        return;
    };
    let actual = root
        .get("document")
        .and_then(|d| d.get("name"))
        .and_then(Value::as_str);
    if actual != Some(expected) {
        diags.push(Diagnostic::error(
            PACKAGE_NAME,
            "/document/name",
            format!("document.name must equal package stem `{expected}`"),
        ));
    }
}

fn check_file_diagnostics(
    root: &Map<String, Value>,
    nodes: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(list) = root.get("diagnostics").and_then(Value::as_array) else {
        return;
    };
    if !diagnostics_sorted(list) {
        diags.push(Diagnostic::error(
            DIAGNOSTIC_ORDER,
            "/diagnostics",
            "diagnostics must be sorted by (severity, code, node_id, runtime_id, strategy, message)",
        ));
    }
    let mut raster_for_fallback: HashMap<&str, usize> = HashMap::new();
    for (i, item) in list.iter().enumerate() {
        let path = format!("/diagnostics/{i}");
        let code = item.get("code").and_then(Value::as_str).unwrap_or("");
        let severity = item.get("severity").and_then(Value::as_str).unwrap_or("");
        let strategy = item.get("strategy").and_then(Value::as_str);
        let node_id = opt_str(item.get("node_id"));
        let runtime_id = opt_str(item.get("runtime_id"));
        if severity == "error" {
            diags.push(from_file_diag(item));
        }
        let node = node_id.and_then(|id| nodes.and_then(|n| n.get(id)));
        if node_id.is_some() && node.is_none() {
            diags.push(
                Diagnostic::error(
                    DIAGNOSTIC_NODE,
                    format!("{path}/node_id"),
                    "diagnostic node_id must name an existing node",
                )
                .with_node(node_id.unwrap_or("")),
            );
        }
        match (node_id, runtime_id) {
            (None, Some(_)) => {
                diags.push(Diagnostic::error(
                    DIAGNOSTIC_RUNTIME_ID,
                    format!("{path}/runtime_id"),
                    "diagnostic runtime_id requires node_id",
                ));
            }
            (Some(id), Some(rid)) => {
                let node_rid = node
                    .and_then(|n| n.get("runtime_id"))
                    .and_then(Value::as_str);
                if node_rid != Some(rid) {
                    diags.push(
                        Diagnostic::error(
                            DIAGNOSTIC_RUNTIME_ID,
                            format!("{path}/runtime_id"),
                            "diagnostic runtime_id must match the referenced node",
                        )
                        .with_node(id),
                    );
                }
            }
            _ => {}
        }
        let node_ty = node.and_then(|n| n.get("type")).and_then(Value::as_str);
        match strategy {
            Some("native") => {
                if node_id.is_none() || node_ty.is_none() || node_ty == Some("fallback") {
                    diags.push(
                        Diagnostic::error(
                            DIAGNOSTIC_STRATEGY,
                            format!("{path}/node_id"),
                            "native strategy must reference an existing non-fallback node",
                        )
                        .with_node(node_id.unwrap_or("")),
                    );
                }
            }
            Some("raster_fallback") => {
                if node_ty != Some("fallback") {
                    diags.push(
                        Diagnostic::error(
                            DIAGNOSTIC_STRATEGY,
                            format!("{path}/node_id"),
                            "raster_fallback strategy must reference a fallback node",
                        )
                        .with_node(node_id.unwrap_or("")),
                    );
                }
            }
            Some("error") if severity != "error" => {
                diags.push(Diagnostic::error(
                    DIAGNOSTIC_STRATEGY,
                    format!("{path}/severity"),
                    "error strategy requires severity error",
                ));
            }
            _ => {}
        }
        if code != "opui.rasterization" {
            continue;
        }
        let Some(id) = node_id else {
            diags.push(Diagnostic::error(
                FALLBACK_DIAGNOSTIC_COUNT,
                path,
                "opui.rasterization must set node_id",
            ));
            continue;
        };
        *raster_for_fallback.entry(id).or_insert(0) += 1;
        let node_rid = node
            .and_then(|n| n.get("runtime_id"))
            .and_then(Value::as_str);
        if runtime_id != node_rid {
            diags.push(
                Diagnostic::error(
                    DIAGNOSTIC_RUNTIME_ID,
                    format!("{path}/runtime_id"),
                    "opui.rasterization runtime_id must equal the fallback node runtime_id",
                )
                .with_node(id),
            );
        }
        if strategy != Some("raster_fallback") || node_ty != Some("fallback") {
            diags.push(
                Diagnostic::error(
                    FALLBACK_DIAGNOSTIC_COUNT,
                    path,
                    "opui.rasterization must use strategy raster_fallback on that fallback node",
                )
                .with_node(id),
            );
        }
    }
    if let Some(nodes) = nodes {
        for (id, node) in nodes {
            if node.get("type").and_then(Value::as_str) != Some("fallback") {
                continue;
            }
            let count = raster_for_fallback.get(id.as_str()).copied().unwrap_or(0);
            if count != 1 {
                diags.push(
                    Diagnostic::error(
                        FALLBACK_DIAGNOSTIC_COUNT,
                        format!("/nodes/{}", escape_pointer(id)),
                        format!(
                            "fallback `{id}` must have exactly one opui.rasterization diagnostic"
                        ),
                    )
                    .with_node(id),
                );
            }
        }
    }
}

fn opt_str(v: Option<&Value>) -> Option<&str> {
    match v {
        Some(Value::Null) | None => None,
        Some(v) => v.as_str(),
    }
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "error" => 0,
        "warning" => 1,
        "info" => 2,
        _ => 9,
    }
}

fn diagnostics_sorted(list: &[Value]) -> bool {
    list.windows(2)
        .all(|w| stored_key(&w[0]) <= stored_key(&w[1]))
}

fn stored_key(item: &Value) -> (u8, &str, &str, &str, &str, &str) {
    (
        severity_rank(item.get("severity").and_then(Value::as_str).unwrap_or("")),
        item.get("code").and_then(Value::as_str).unwrap_or(""),
        opt_str(item.get("node_id")).unwrap_or(""),
        opt_str(item.get("runtime_id")).unwrap_or(""),
        item.get("strategy").and_then(Value::as_str).unwrap_or(""),
        item.get("message").and_then(Value::as_str).unwrap_or(""),
    )
}

fn from_file_diag(item: &Value) -> Diagnostic {
    Diagnostic {
        code: item
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("opui.unsupported_error")
            .to_string(),
        severity: "error".into(),
        node_id: opt_str(item.get("node_id")).map(str::to_string),
        runtime_id: opt_str(item.get("runtime_id")).map(str::to_string),
        message: item
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("document contains an error diagnostic")
            .to_string(),
        strategy: item
            .get("strategy")
            .and_then(Value::as_str)
            .map(str::to_string),
        details: Map::new(),
        path: String::new(),
    }
}

fn canonical_check(bytes: &[u8], value: &Value, diags: &mut Vec<Diagnostic>) {
    if bytes == canonical_bytes(value).as_slice() {
        return;
    }
    if !diags.iter().any(|d| {
        matches!(
            d.code.as_str(),
            KEY_ORDER | INDENT | ENCODING | NUMBER_NORMALIZATION | NEGATIVE_ZERO
        )
    }) {
        diags.push(Diagnostic::error(
            ENCODING,
            "",
            "JSON bytes are not canonical",
        ));
    }
}

fn sort_diagnostics(diags: &mut [Diagnostic]) {
    diags.sort_by(|a, b| {
        severity_rank(&a.severity)
            .cmp(&severity_rank(&b.severity))
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| {
                a.node_id
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.node_id.as_deref().unwrap_or(""))
            })
            .then_with(|| {
                a.runtime_id
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.runtime_id.as_deref().unwrap_or(""))
            })
            .then_with(|| {
                a.strategy
                    .as_deref()
                    .unwrap_or("")
                    .cmp(b.strategy.as_deref().unwrap_or(""))
            })
            .then_with(|| a.message.cmp(&b.message))
            .then_with(|| a.path.cmp(&b.path))
    });
}

pub fn default_schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("spec/opui-v1.schema.json")
}

#[cfg(test)]
mod svg_tests {
    use super::*;

    fn codes(text: &str) -> Vec<String> {
        let mut diags = Vec::new();
        check_svg(text, "vector-x", "/assets/vector-x", &mut diags);
        diags.into_iter().map(|d| d.code).collect()
    }

    #[test]
    fn malformed_svg_is_invalid() {
        assert!(codes("<svg").contains(&SVG_INVALID.to_string()));
    }

    #[test]
    fn namespaced_script_is_caught() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg"><s:script>1</s:script></svg>"#;
        assert!(codes(svg).contains(&SVG_SCRIPT.to_string()));
    }

    #[test]
    fn namespaced_style_is_caught() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg"><s:style>x{}</s:style></svg>"#;
        assert!(codes(svg).contains(&SVG_CSS.to_string()));
    }

    #[test]
    fn href_with_spaces_around_equals_is_external() {
        let svg =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><a href = "https://example.test/x"/></svg>"#;
        assert!(codes(svg).contains(&SVG_EXTERNAL_REF.to_string()));
    }

    #[test]
    fn css_url_with_space_before_paren_is_external() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect fill="url (https://example.test/x)"/></svg>"#;
        assert!(codes(svg).contains(&SVG_EXTERNAL_REF.to_string()));
    }
}

#[cfg(test)]
mod gradient_tests {
    use super::*;

    #[test]
    fn decreasing_gradient_stops_are_reported() {
        let doc = br#"{
  "assets": {},
  "diagnostics": [],
  "document": {"name": "x", "reference_viewport": {"height": 1, "width": 1}},
  "entrypoints": {"default": "a"},
  "extensions": {},
  "format": "openpencil-runtime-ui",
  "generator": {"name": "t", "version": "1"},
  "nodes": {
    "a": {
      "children": [],
      "source_id": "a",
      "style": {
        "fill": {
          "end": {"x": 1, "y": 0},
          "start": {"x": 0, "y": 0},
          "stops": [
            {"color": {"a": 1, "b": 0, "g": 0, "r": 1, "space": "srgb"}, "offset": 1},
            {"color": {"a": 1, "b": 0, "g": 0, "r": 0, "space": "srgb"}, "offset": 0}
          ],
          "type": "linear"
        }
      },
      "type": "container"
    }
  },
  "schema_version": 1
}
"#;
        let diags = check_bytes(
            doc,
            &CheckOptions {
                check_assets: false,
                check_canonical: false,
                asset_root: None,
                package_name: None,
            },
        );
        assert!(diags.iter().any(|d| d.code == GRADIENT_STOP_ORDER));
    }
}
