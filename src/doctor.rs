use crate::{config, db};
use anyhow::{anyhow, Context, Result};
use colored::{ColoredString, Colorize};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use toml::{map::Map as TomlMap, to_string_pretty, Value as TomlValue};

struct DoctorSummary {
    lines: Vec<String>,
    healthy: bool,
}

pub async fn run() -> Result<()> {
    let summary = evaluate().await?;

    for line in &summary.lines {
        println!("{line}");
    }

    if summary.healthy {
        Ok(())
    } else {
        Err(anyhow!("doctor checks failed"))
    }
}

async fn evaluate() -> Result<DoctorSummary> {
    let mut lines = Vec::new();
    let mut healthy = true;

    match check_config() {
        Ok(message) => lines.push(format!("Config: OK - {message}")),
        Err(err) => {
            lines.push(format!("Config: ERR - {err}"));
            healthy = false;
        }
    }

    match check_database().await {
        Ok(message) => lines.push(format!("Database: OK - {message}")),
        Err(err) => {
            lines.push(format!("Database: ERR - {err}"));
            healthy = false;
        }
    }

    if healthy {
        lines.push("Doctor summary: all checks passed".to_string());
    } else {
        lines.push("Doctor summary: issues detected".to_string());
    }

    Ok(DoctorSummary { lines, healthy })
}

fn check_config() -> Result<String> {
    let config_dir = config::get_config_dir().context("finding config directory")?;
    let config_file = config_dir.join("config.toml");

    config::load_config()
        .with_context(|| format!("loading configuration from {}", config_file.display()))?;

    let hosts_path = config::get_hosts_file_path().context("resolving hosts file path")?;
    if !hosts_path.exists() {
        return Err(anyhow!("expected hosts file at {}", hosts_path.display()));
    }

    Ok(format!(
        "configuration file at {} parsed successfully",
        config_file.display()
    ))
}

async fn check_database() -> Result<String> {
    let db_path = db::get_db_path();
    db::init_db(&db_path)
        .await
        .with_context(|| format!("initializing database at {db_path}"))?;

    db::load_env_state(&db_path)
        .await
        .with_context(|| format!("querying env_state table at {db_path}"))?;

    let file_path = PathBuf::from(&db_path);
    Ok(format!("database reachable at {}", file_path.display()))
}

pub fn print_config() -> Result<()> {
    let config_dir = config::get_config_dir()?;
    let config_file = config_dir.join("config.toml");
    let current = load_config_or_default(&config_file)?;
    let default = config::AppConfig::default();

    let merged = merge_with_defaults(&default, &current)?;
    let configured_paths = gather_configured_paths(&config_file)?;
    let annotated = annotate_config_toml(&default, &merged, &configured_paths)?;

    println!("{}\n{}", "Configuration".bold(), annotated);

    Ok(())
}

fn gather_configured_paths(config_file: &Path) -> Result<HashSet<Vec<String>>> {
    if !config_file.exists() {
        return Ok(HashSet::new());
    }

    let contents = fs::read_to_string(config_file)?;
    if contents.trim().is_empty() {
        return Ok(HashSet::new());
    }

    let parsed: TomlValue = toml::from_str(&contents)?;
    let mut paths = HashSet::new();

    if let TomlValue::Table(table) = parsed {
        for (key, value) in table {
            let mut current_path = vec![key];
            collect_configured_paths(&mut current_path, value, &mut paths);
        }
    }

    Ok(paths)
}

fn collect_configured_paths(
    current_path: &mut Vec<String>,
    value: TomlValue,
    paths: &mut HashSet<Vec<String>>,
) {
    match value {
        TomlValue::Table(map) => {
            if !current_path.is_empty() {
                paths.insert(current_path.clone());
            }
            for (child_key, child_value) in map {
                current_path.push(child_key);
                collect_configured_paths(current_path, child_value, paths);
                current_path.pop();
            }
        }
        TomlValue::Array(items) => {
            if !current_path.is_empty() {
                paths.insert(current_path.clone());
            }
            for item in items {
                if let TomlValue::Table(_) = item {
                    // Array of tables: recurse to capture nested fields.
                    collect_configured_paths(current_path, item, paths);
                }
            }
        }
        _ => {
            if !current_path.is_empty() {
                paths.insert(current_path.clone());
            }
        }
    }
}

fn load_config_or_default(path: &Path) -> Result<config::AppConfig> {
    if path.exists() {
        config::load_config()
    } else {
        Ok(config::AppConfig::default())
    }
}

fn merge_with_defaults(
    default: &config::AppConfig,
    current: &config::AppConfig,
) -> Result<config::AppConfig> {
    let mut merged = serde_json::to_value(default)?;
    let current_json = serde_json::to_value(current)?;
    deep_merge(&mut merged, &current_json);
    Ok(serde_json::from_value(merged)?)
}

fn deep_merge(target: &mut JsonValue, source: &JsonValue) {
    match (target, source) {
        (JsonValue::Object(target_map), JsonValue::Object(source_map)) => {
            for (key, source_value) in source_map {
                if let Some(target_value) = target_map.get_mut(key) {
                    deep_merge(target_value, source_value);
                } else {
                    target_map.insert(key.clone(), source_value.clone());
                }
            }
        }
        (target_slot, source_value) => {
            *target_slot = source_value.clone();
        }
    }
}

fn annotate_config_toml(
    default: &config::AppConfig,
    current: &config::AppConfig,
    configured_paths: &HashSet<Vec<String>>,
) -> Result<String> {
    let annotations = build_annotation_map(default, current)?;
    highlight_toml_with_annotations(current, &annotations, configured_paths)
}

fn build_annotation_map<T>(default: &T, current: &T) -> Result<BTreeMap<Vec<String>, ValueSnapshot>>
where
    T: Serialize,
{
    let default_json = serde_json::to_value(default)?;
    let current_json = serde_json::to_value(current)?;

    let mut map = BTreeMap::new();
    collect_snapshots(Vec::new(), &current_json, &default_json, &mut map);
    Ok(map)
}

#[derive(Clone)]
struct ValueSnapshot {
    current: JsonValue,
    default: JsonValue,
}

fn collect_snapshots(
    mut path: Vec<String>,
    current: &JsonValue,
    default: &JsonValue,
    map: &mut BTreeMap<Vec<String>, ValueSnapshot>,
) {
    match current {
        JsonValue::Object(obj) => {
            for (key, value) in obj {
                path.push(key.clone());
                let default_child = default.get(key).unwrap_or(&JsonValue::Null);
                collect_snapshots(path.clone(), value, default_child, map);
                path.pop();
            }
        }
        JsonValue::Array(_) => {
            if !path.is_empty() {
                map.insert(
                    path,
                    ValueSnapshot {
                        current: current.clone(),
                        default: default.clone(),
                    },
                );
            }
        }
        _ => {
            if !path.is_empty() {
                map.insert(
                    path,
                    ValueSnapshot {
                        current: current.clone(),
                        default: default.clone(),
                    },
                );
            }
        }
    }
}

struct PendingComment {
    closing: char,
    comment: String,
    kind: ValueKind,
    item_kind: Option<ValueKind>,
    configured: bool,
}

struct RenderedLine {
    text: String,
    deferred: Option<PendingComment>,
}

fn highlight_toml_with_annotations(
    current: &config::AppConfig,
    annotations: &BTreeMap<Vec<String>, ValueSnapshot>,
    configured_paths: &HashSet<Vec<String>>,
) -> Result<String> {
    let toml_string = to_string_pretty(current)?;
    let mut result = String::new();
    let mut table_path: Vec<String> = Vec::new();
    let mut pending_comments: Vec<PendingComment> = Vec::new();

    for line in toml_string.lines() {
        let trimmed = line.trim_start();
        let indent = &line[..line.len() - trimmed.len()];

        if trimmed.is_empty() {
            result.push('\n');
            continue;
        }

        if let Some(path) = parse_table_path(trimmed) {
            table_path = path;
            result.push_str(indent);
            result.push_str(&trimmed.blue().bold().to_string());
            result.push('\n');
            continue;
        }

        if let Some((key_part, value_part)) = trimmed.split_once('=') {
            let key = key_part.trim();
            let value_text = value_part.trim();
            let mut full_path = table_path.clone();
            full_path.push(key.to_string());
            let rendered = render_line(
                indent,
                key,
                value_text,
                annotations.get(&full_path),
                configured_paths.contains(&full_path),
            );
            result.push_str(&rendered.text);
            if let Some(deferred) = rendered.deferred {
                pending_comments.push(deferred);
            }
        } else {
            let mut line_buf = String::new();
            line_buf.push_str(indent);
            let is_closing = pending_comments
                .last()
                .map(|pending| line_starts_with(trimmed, pending.closing))
                .unwrap_or(false);

            if is_closing {
                if let Some(pending) = pending_comments.pop() {
                    let mut closing_repr = colorize_primary(trimmed, pending.kind);
                    if !pending.configured {
                        closing_repr = closing_repr.dimmed();
                    }
                    line_buf.push_str(&closing_repr.to_string());
                    line_buf.push_str(&pending.comment);
                } else {
                    line_buf.push_str(trimmed);
                }
            } else if let Some(pending) = pending_comments.last() {
                if let Some(item_kind) = pending.item_kind {
                    let mut body_repr = colorize_primary(trimmed, item_kind);
                    if !pending.configured {
                        body_repr = body_repr.dimmed();
                    }
                    line_buf.push_str(&body_repr.to_string());
                } else {
                    line_buf.push_str(trimmed);
                }
            } else {
                line_buf.push_str(trimmed);
            }
            line_buf.push('\n');
            result.push_str(&line_buf);
        }
    }

    Ok(result.trim_end().to_string())
}

fn render_line(
    indent: &str,
    key: &str,
    value_text: &str,
    annotation: Option<&ValueSnapshot>,
    is_configured: bool,
) -> RenderedLine {
    let mut line = String::new();
    line.push_str(indent);
    let mut key_repr = key.bold();
    if !is_configured {
        key_repr = key_repr.dimmed();
    }
    line.push_str(&key_repr.to_string());
    line.push_str(" = ");

    let mut deferred = None;

    if let Some(snapshot) = annotation {
        let kind = value_kind(&snapshot.current);
        let mut value_repr = colorize_primary(value_text, kind);
        if !is_configured {
            value_repr = value_repr.dimmed();
        }
        line.push_str(&value_repr.to_string());

        let type_sample = select_type_sample(&snapshot.default, &snapshot.current);
        let type_label = format!("({})", describe_type(type_sample));
        let type_colored = if type_consistent(&snapshot.current, type_sample) {
            type_label.bright_black()
        } else {
            type_label.red()
        };

        let mut comment_parts: Vec<ColoredString> = vec![type_colored];

        if show_default_note(snapshot) {
            let default_display = format!("[{}]", format_value(&snapshot.default));
            comment_parts.push(colorize_secondary(&default_display, kind));
        }

        if !comment_parts.is_empty() {
            let mut comment = String::new();
            comment.push_str("  ");
            comment.push_str(&"#".bright_black().to_string());

            for part in comment_parts {
                comment.push(' ');
                comment.push_str(&part.to_string());
            }

            if let Some(closing) = multiline_closing(kind, value_text) {
                deferred = Some(PendingComment {
                    closing,
                    comment,
                    kind,
                    item_kind: infer_nested_kind(snapshot),
                    configured: is_configured,
                });
            } else {
                line.push_str(&comment);
            }
        }
    } else {
        let mut value_repr = colorize_literal(value_text);
        if !is_configured {
            value_repr = value_repr.dimmed();
        }
        line.push_str(&value_repr.to_string());
    }

    line.push('\n');
    RenderedLine {
        text: line,
        deferred,
    }
}

fn multiline_closing(kind: ValueKind, value_text: &str) -> Option<char> {
    match (kind, value_text) {
        (ValueKind::Array, "[") => Some(']'),
        (ValueKind::Object, "{") => Some('}'),
        _ => None,
    }
}

fn line_starts_with(line: &str, expected: char) -> bool {
    line.starts_with(expected)
}

fn infer_nested_kind(snapshot: &ValueSnapshot) -> Option<ValueKind> {
    match snapshot.current {
        JsonValue::Array(ref items) => items
            .iter()
            .find_map(|item| match value_kind(item) {
                ValueKind::Null => None,
                kind => Some(kind),
            })
            .or(Some(ValueKind::Null)),
        JsonValue::Object(_) => Some(ValueKind::Object),
        _ => None,
    }
}

fn parse_table_path(line: &str) -> Option<Vec<String>> {
    if let Some(inner) = line
        .strip_prefix("[[")
        .and_then(|rest| rest.strip_suffix("]]"))
    {
        return Some(
            inner
                .split('.')
                .map(|segment| segment.trim().to_string())
                .collect(),
        );
    }

    if let Some(inner) = line
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    {
        return Some(
            inner
                .split('.')
                .map(|segment| segment.trim().to_string())
                .collect(),
        );
    }

    None
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ValueKind {
    Null,
    Boolean(bool),
    Integer,
    Float,
    String,
    Array,
    Object,
}

fn value_kind(value: &JsonValue) -> ValueKind {
    match value {
        JsonValue::Null => ValueKind::Null,
        JsonValue::Bool(flag) => ValueKind::Boolean(*flag),
        JsonValue::Number(num) => {
            if num.is_i64() || num.is_u64() {
                ValueKind::Integer
            } else {
                ValueKind::Float
            }
        }
        JsonValue::String(_) => ValueKind::String,
        JsonValue::Array(_) => ValueKind::Array,
        JsonValue::Object(_) => ValueKind::Object,
    }
}

fn colorize_primary(value_text: &str, kind: ValueKind) -> ColoredString {
    let palette = palette_for(kind);
    apply_color(value_text, palette.primary)
}

fn colorize_secondary(text: &str, kind: ValueKind) -> ColoredString {
    let palette = palette_for(kind);
    apply_color(text, palette.secondary)
}

fn colorize_literal(value_text: &str) -> ColoredString {
    value_text.normal()
}

struct Palette {
    primary: (u8, u8, u8),
    secondary: (u8, u8, u8),
}

fn palette_for(kind: ValueKind) -> Palette {
    let primary = match kind {
        ValueKind::Null => (140, 140, 140),
        ValueKind::Boolean(true) => (60, 179, 113),
        ValueKind::Boolean(false) => (229, 88, 88),
        ValueKind::Integer => (215, 166, 47),
        ValueKind::Float => (202, 156, 64),
        ValueKind::String => (108, 182, 255),
        ValueKind::Array => (160, 141, 229),
        ValueKind::Object => (110, 139, 199),
    };

    let secondary = soften(primary);
    Palette { primary, secondary }
}

fn soften(color: (u8, u8, u8)) -> (u8, u8, u8) {
    let (r, g, b) = color;
    (soften_channel(r), soften_channel(g), soften_channel(b))
}

fn soften_channel(channel: u8) -> u8 {
    channel + ((255 - channel) / 2)
}

fn apply_color(text: &str, color: (u8, u8, u8)) -> ColoredString {
    let (r, g, b) = color;
    text.truecolor(r, g, b)
}

fn select_type_sample<'a>(default: &'a JsonValue, current: &'a JsonValue) -> &'a JsonValue {
    if !matches!(value_kind(default), ValueKind::Null) {
        default
    } else {
        current
    }
}

fn describe_type(sample: &JsonValue) -> &'static str {
    match value_kind(sample) {
        ValueKind::Null => "null",
        ValueKind::Boolean(_) => "bool",
        ValueKind::Integer => "int",
        ValueKind::Float => "float",
        ValueKind::String => "string",
        ValueKind::Array => "array",
        ValueKind::Object => "table",
    }
}

fn type_consistent(current: &JsonValue, sample: &JsonValue) -> bool {
    matches!(
        (value_kind(current), value_kind(sample)),
        (_, ValueKind::Null)
            | (ValueKind::Boolean(_), ValueKind::Boolean(_))
            | (ValueKind::Integer, ValueKind::Integer)
            | (ValueKind::Float, ValueKind::Float)
            | (ValueKind::Integer, ValueKind::Float)
            | (ValueKind::Float, ValueKind::Integer)
            | (ValueKind::String, ValueKind::String)
            | (ValueKind::Array, ValueKind::Array)
            | (ValueKind::Object, ValueKind::Object)
    )
}

fn show_default_note(snapshot: &ValueSnapshot) -> bool {
    if snapshot.default.is_null() {
        return false;
    }

    snapshot.current != snapshot.default
}

fn format_value(value: &JsonValue) -> String {
    if let Some(toml_value) = json_to_toml(value) {
        toml_value.to_string()
    } else {
        value.to_string()
    }
}

fn json_to_toml(value: &JsonValue) -> Option<TomlValue> {
    match value {
        JsonValue::Null => None,
        JsonValue::Bool(flag) => Some(TomlValue::Boolean(*flag)),
        JsonValue::Number(num) => {
            if let Some(int) = num.as_i64() {
                Some(TomlValue::Integer(int))
            } else {
                num.as_f64().map(TomlValue::Float)
            }
        }
        JsonValue::String(text) => Some(TomlValue::String(text.clone())),
        JsonValue::Array(values) => {
            let mut items = Vec::with_capacity(values.len());
            for value in values {
                if let Some(converted) = json_to_toml(value) {
                    items.push(converted);
                } else {
                    return None;
                }
            }
            Some(TomlValue::Array(items))
        }
        JsonValue::Object(map) => {
            let mut table = TomlMap::new();
            for (key, value) in map {
                if let Some(converted) = json_to_toml(value) {
                    table.insert(key.clone(), converted);
                }
            }
            Some(TomlValue::Table(table))
        }
    }
}
