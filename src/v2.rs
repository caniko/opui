use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::{
    ASSET_KIND_MISMATCH, BREAKPOINT_BOUNDS, BREAKPOINT_OVERLAP, COMPONENT_CYCLE, COMPONENT_ROOT,
    CheckOptions, DUPLICATE_EXPANDED_RUNTIME_ID, Diagnostic, EXPANDED_RUNTIME_ID, ForestEdge,
    LENGTH_CONTEXT, LOCALE_CYCLE, LOCALE_DEFAULT, MASK_ASSET_KIND, MASK_SOURCE, PATH_COMMANDS,
    SLOT_MULTIPLE, SLOT_REQUIRED, TEMPLATE_CYCLE, TEMPLATE_DUPLICATE_CHILD, TEMPLATE_KEY_MISMATCH,
    TEMPLATE_MULTIPLE_PARENTS, TEMPLATE_UNKNOWN_CHILD, TEMPLATE_UNREACHABLE,
    TEXT_PATH_INCOMPATIBLE, TEXT_PATH_UNKNOWN, THEME_CYCLE, THEME_DEFAULT, THEME_VALUE_KIND,
    TOKEN_KIND_MISMATCH, UNKNOWN_BREAKPOINT, UNKNOWN_COMPONENT, UNKNOWN_LOCALE, UNKNOWN_MASK_PATH,
    UNKNOWN_MESSAGE, UNKNOWN_PATH, UNKNOWN_SLOT, UNKNOWN_TEMPLATE_NODE, UNKNOWN_THEME,
    UNKNOWN_TOKEN, UNKNOWN_VARIANT_AXIS, UNKNOWN_VARIANT_VALUE, UNRESOLVED_FONT,
    UNRESOLVED_INLINE_IMAGE, VARIANT_DEFAULT, asset_kind, check_assets, check_fallback_asset,
    check_file_diagnostics, check_fill_stops, check_forest, check_image, check_nodes,
    check_package_name, escape_pointer,
};

pub(crate) fn check(
    root: &Map<String, Value>,
    nodes: Option<&Map<String, Value>>,
    assets: Option<&Map<String, Value>>,
    opts: &CheckOptions,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(nodes) = nodes {
        check_nodes(nodes, assets, diags);
    }
    if let Some(assets) = assets {
        check_assets(assets, opts, diags);
    }
    let extras = nodes.map(slot_edges).unwrap_or_default();
    if let (Some(nodes), Some(entrypoints)) =
        (nodes, root.get("entrypoints").and_then(Value::as_object))
    {
        check_forest(nodes, entrypoints, &extras, diags);
    }
    check_package_name(root, opts, diags);
    check_file_diagnostics(root, nodes, diags);

    let resources = root.get("resources").and_then(Value::as_object);
    let catalogs = Catalogs::from_resources(resources);
    catalogs.check_breakpoints(diags);
    catalogs.check_themes(assets, diags);
    catalogs.check_locales(assets, diags);
    catalogs.check_paths(diags);
    catalogs.check_components(assets, diags);

    if let Some(nodes) = nodes {
        catalogs.walk_concrete(nodes, assets, diags);
    }
}

fn slot_edges(nodes: &Map<String, Value>) -> Vec<ForestEdge<'_>> {
    let mut out = Vec::new();
    for (id, node) in nodes {
        let Some(slots) = node
            .get("component_instance")
            .and_then(|p| p.get("slots"))
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (slot, ids) in slots {
            let Some(ids) = ids.as_array() else {
                continue;
            };
            for (i, child) in ids.iter().enumerate() {
                let Some(cid) = child.as_str() else {
                    continue;
                };
                out.push(ForestEdge {
                    parent: id.as_str(),
                    child: cid,
                    path: format!(
                        "/nodes/{}/component_instance/slots/{}/{}",
                        escape_pointer(id),
                        escape_pointer(slot),
                        i
                    ),
                });
            }
        }
    }
    out
}

struct Catalogs<'a> {
    breakpoints: Option<&'a Map<String, Value>>,
    tokens: Option<&'a Map<String, Value>>,
    themes: Option<&'a Value>,
    localization: Option<&'a Value>,
    paths: Option<&'a Map<String, Value>>,
    components: Option<&'a Map<String, Value>>,
}

impl<'a> Catalogs<'a> {
    fn from_resources(resources: Option<&'a Map<String, Value>>) -> Self {
        Self {
            breakpoints: resources
                .and_then(|r| r.get("breakpoints"))
                .and_then(Value::as_object),
            tokens: resources
                .and_then(|r| r.get("tokens"))
                .and_then(Value::as_object),
            themes: resources.and_then(|r| r.get("themes")),
            localization: resources.and_then(|r| r.get("localization")),
            paths: resources
                .and_then(|r| r.get("paths"))
                .and_then(Value::as_object),
            components: resources
                .and_then(|r| r.get("components"))
                .and_then(Value::as_object),
        }
    }

    fn check_breakpoints(&self, diags: &mut Vec<Diagnostic>) {
        let Some(bps) = self.breakpoints else {
            return;
        };
        let mut rows: Vec<(&str, Option<f64>, Option<f64>)> = Vec::new();
        for (id, bp) in bps {
            let min = num_or_null(bp.get("min_width"));
            let max = num_or_null(bp.get("max_width"));
            if let (Some(lo), Some(hi)) = (min, max)
                && lo >= hi
            {
                diags.push(Diagnostic::error(
                    BREAKPOINT_BOUNDS,
                    format!("/resources/breakpoints/{}", escape_pointer(id)),
                    format!("breakpoint `{id}` min_width must be < max_width"),
                ));
            }
            rows.push((id.as_str(), min, max));
        }
        rows.sort_by(|a, b| {
            rank_min(a.1)
                .partial_cmp(&rank_min(b.1))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });
        for pair in rows.windows(2) {
            let (aid, _, amax) = pair[0];
            let (bid, bmin, _) = pair[1];
            let overlaps = match (amax, bmin) {
                (Some(hi), Some(lo)) => hi > lo,
                (None, _) => true,
                (_, None) => true,
            };
            if overlaps {
                diags.push(Diagnostic::error(
                    BREAKPOINT_OVERLAP,
                    format!("/resources/breakpoints/{}", escape_pointer(bid)),
                    format!("breakpoints `{aid}` and `{bid}` overlap"),
                ));
            }
        }
    }

    fn check_themes(&self, assets: Option<&Map<String, Value>>, diags: &mut Vec<Diagnostic>) {
        let Some(themes) = self.themes.and_then(Value::as_object) else {
            return;
        };
        let items = themes.get("items").and_then(Value::as_object);
        if let Some(default) = themes.get("default").and_then(Value::as_str)
            && items.is_none_or(|m| !m.contains_key(default))
        {
            diags.push(Diagnostic::error(
                THEME_DEFAULT,
                "/resources/themes/default",
                format!("unknown default theme `{default}`"),
            ));
        }
        let Some(items) = items else {
            return;
        };
        for (id, theme) in items {
            if let Some(ext) = theme.get("extends").and_then(Value::as_str)
                && !items.contains_key(ext)
            {
                diags.push(Diagnostic::error(
                    UNKNOWN_THEME,
                    format!("/resources/themes/items/{}/extends", escape_pointer(id)),
                    format!("unknown theme `{ext}`"),
                ));
            }
            if let Some(values) = theme.get("values").and_then(Value::as_object) {
                for (tok, val) in values {
                    match self.token_kind(tok) {
                        None => diags.push(Diagnostic::error(
                            UNKNOWN_TOKEN,
                            format!(
                                "/resources/themes/items/{}/values/{}",
                                escape_pointer(id),
                                escape_pointer(tok)
                            ),
                            format!("unknown token `{tok}`"),
                        )),
                        Some(kind) if !literal_matches_kind(kind, val) => {
                            diags.push(Diagnostic::error(
                                THEME_VALUE_KIND,
                                format!(
                                    "/resources/themes/items/{}/values/{}",
                                    escape_pointer(id),
                                    escape_pointer(tok)
                                ),
                                format!("theme value for `{tok}` must be a {kind} literal"),
                            ))
                        }
                        Some("font") => check_font_id(
                            val.as_str().unwrap_or(""),
                            &format!(
                                "/resources/themes/items/{}/values/{}",
                                escape_pointer(id),
                                escape_pointer(tok)
                            ),
                            None,
                            assets,
                            diags,
                        ),
                        Some(_) => {}
                    }
                }
            }
        }
        if let Some(hit) = graph_cycle(items, |t| {
            t.get("extends")
                .and_then(Value::as_str)
                .into_iter()
                .collect()
        }) {
            diags.push(Diagnostic::error(
                THEME_CYCLE,
                format!("/resources/themes/items/{}", escape_pointer(hit)),
                "theme extends graph is cyclic",
            ));
        }
    }

    fn check_locales(&self, assets: Option<&Map<String, Value>>, diags: &mut Vec<Diagnostic>) {
        let Some(loc) = self.localization.and_then(Value::as_object) else {
            return;
        };
        let locales = loc.get("locales").and_then(Value::as_object);
        if let Some(default) = loc.get("default_locale").and_then(Value::as_str)
            && locales.is_none_or(|m| !m.contains_key(default))
        {
            diags.push(Diagnostic::error(
                LOCALE_DEFAULT,
                "/resources/localization/default_locale",
                format!("unknown default locale `{default}`"),
            ));
        }
        let Some(locales) = locales else {
            return;
        };
        for (id, locale) in locales {
            if let Some(fb) = locale.get("fallback").and_then(Value::as_str)
                && !locales.contains_key(fb)
            {
                diags.push(Diagnostic::error(
                    UNKNOWN_LOCALE,
                    format!(
                        "/resources/localization/locales/{}/fallback",
                        escape_pointer(id)
                    ),
                    format!("unknown locale `{fb}`"),
                ));
            }
        }
        if let Some(hit) = graph_cycle(locales, |l| {
            l.get("fallback")
                .and_then(Value::as_str)
                .into_iter()
                .collect()
        }) {
            diags.push(Diagnostic::error(
                LOCALE_CYCLE,
                format!("/resources/localization/locales/{}", escape_pointer(hit)),
                "locale fallback graph is cyclic",
            ));
        }
        for (lid, locale) in locales {
            if let Some(messages) = locale.get("messages").and_then(Value::as_object) {
                for (key, rt) in messages {
                    walk_rich_text(
                        rt,
                        None,
                        &format!(
                            "/resources/localization/locales/{}/messages/{}",
                            escape_pointer(lid),
                            escape_pointer(key)
                        ),
                        self,
                        assets,
                        diags,
                    );
                }
            }
        }
    }

    fn check_paths(&self, diags: &mut Vec<Diagnostic>) {
        let Some(paths) = self.paths else {
            return;
        };
        for (id, path) in paths {
            let Some(cmds) = path.get("commands").and_then(Value::as_array) else {
                continue;
            };
            check_path_commands(
                cmds,
                &format!("/resources/paths/{}/commands", escape_pointer(id)),
                None,
                diags,
            );
        }
    }

    fn check_components(&self, assets: Option<&Map<String, Value>>, diags: &mut Vec<Diagnostic>) {
        let Some(components) = self.components else {
            return;
        };
        for (cid, def) in components {
            let base = format!("/resources/components/{}", escape_pointer(cid));
            let nodes = def.get("nodes").and_then(Value::as_object);
            if let Some(root) = def.get("root").and_then(Value::as_str)
                && nodes.is_none_or(|n| !n.contains_key(root))
            {
                diags.push(Diagnostic::error(
                    COMPONENT_ROOT,
                    format!("{base}/root"),
                    format!("component `{cid}` root `{root}` is missing"),
                ));
            }
            let Some(nodes) = nodes else {
                continue;
            };
            for (tid, node) in nodes {
                let path = format!("{base}/nodes/{}", escape_pointer(tid));
                if node.get("template_id").and_then(Value::as_str) != Some(tid.as_str()) {
                    diags.push(
                        Diagnostic::error(
                            TEMPLATE_KEY_MISMATCH,
                            format!("{path}/template_id"),
                            "nodes key must equal template_id",
                        )
                        .with_node(tid),
                    );
                }
                self.walk_node(node, Some(tid), &path, Some(nodes), assets, diags);
            }
            check_template_forest(cid, def, nodes, &base, diags);
            check_slot_defs(def, nodes, &base, diags);
            check_variant_axes(def, nodes, &base, self, assets, diags);
        }
        if let Some(hit) = graph_cycle(components, component_deps) {
            diags.push(Diagnostic::error(
                COMPONENT_CYCLE,
                format!("/resources/components/{}", escape_pointer(hit)),
                "component dependency graph is cyclic",
            ));
        }
    }

    fn walk_concrete(
        &self,
        nodes: &Map<String, Value>,
        assets: Option<&Map<String, Value>>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let mut used: HashMap<String, String> = HashMap::new();
        for (id, node) in nodes {
            if let Some(rid) = node.get("runtime_id").and_then(Value::as_str) {
                used.entry(rid.to_string()).or_insert_with(|| id.clone());
            }
        }
        for (id, node) in nodes {
            let path = format!("/nodes/{}", escape_pointer(id));
            self.walk_node(node, Some(id.as_str()), &path, None, assets, diags);
            if node.get("type").and_then(Value::as_str) == Some("component_instance") {
                if let Some(payload) = node.get("component_instance") {
                    self.check_expanded_instance(
                        payload,
                        &path,
                        Some(id),
                        assets,
                        &mut HashSet::new(),
                        diags,
                    );
                    expand_instance(
                        payload,
                        node.get("runtime_id").and_then(Value::as_str),
                        self.components,
                        &mut used,
                        id,
                        &mut HashSet::new(),
                        diags,
                    );
                }
            }
        }
    }

    fn check_expanded_instance(
        &self,
        payload: &Value,
        path: &str,
        node_id: Option<&str>,
        assets: Option<&Map<String, Value>>,
        stack: &mut HashSet<String>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let Some(definition) = payload.get("definition").and_then(Value::as_str) else {
            return;
        };
        if !stack.insert(definition.to_string()) {
            diags.push(err(
                COMPONENT_CYCLE,
                format!("{path}/component_instance/definition"),
                format!("component expansion cycles through `{definition}`"),
                node_id,
            ));
            return;
        }
        let Some((def, nodes)) = instantiate_component(payload, self.components) else {
            stack.remove(definition);
            return;
        };
        let base = format!("{path}/component_instance/expanded");
        check_template_forest(definition, def, &nodes, &base, diags);
        check_slot_defs(def, &nodes, &base, diags);
        for (id, node) in &nodes {
            let node_path = format!("{base}/{}", escape_pointer(id));
            self.walk_node(node, node_id, &node_path, Some(&nodes), assets, diags);
            if node.get("type").and_then(Value::as_str) == Some("component_instance")
                && let Some(nested) = node.get("component_instance")
            {
                self.check_expanded_instance(nested, &node_path, node_id, assets, stack, diags);
            }
        }
        stack.remove(definition);
    }

    fn walk_node(
        &self,
        node: &Value,
        node_id: Option<&str>,
        path: &str,
        template_nodes: Option<&Map<String, Value>>,
        assets: Option<&Map<String, Value>>,
        diags: &mut Vec<Diagnostic>,
    ) {
        if let Some(layout) = node.get("layout") {
            walk_layout(layout, node_id, &format!("{path}/layout"), self, diags);
        }
        if let Some(style) = node.get("style") {
            walk_style(style, node_id, &format!("{path}/style"), self, diags);
        }
        if let Some(resp) = node.get("responsive").and_then(Value::as_object) {
            for (key, ov) in resp {
                if self.breakpoints.is_none_or(|b| !b.contains_key(key)) {
                    diags.push(err(
                        UNKNOWN_BREAKPOINT,
                        format!("{path}/responsive/{}", escape_pointer(key)),
                        format!("unknown breakpoint `{key}`"),
                        node_id,
                    ));
                }
                if let Some(layout) = ov.get("layout") {
                    walk_layout(
                        layout,
                        node_id,
                        &format!("{path}/responsive/{}/layout", escape_pointer(key)),
                        self,
                        diags,
                    );
                }
                if let Some(style) = ov.get("style") {
                    walk_style(
                        style,
                        node_id,
                        &format!("{path}/responsive/{}/style", escape_pointer(key)),
                        self,
                        diags,
                    );
                }
            }
        }
        if let Some(masks) = node.get("masks").and_then(Value::as_array) {
            walk_masks(
                masks,
                node_id,
                &format!("{path}/masks"),
                self,
                assets,
                diags,
            );
        }
        match node.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = node.get("text") {
                    self.walk_text(text, node_id, &format!("{path}/text"), assets, diags);
                }
            }
            Some("image") => {
                if let Some(image) = node.get("image") {
                    if template_nodes.is_some() {
                        check_image(
                            node_id.unwrap_or(""),
                            &format!("{path}/image"),
                            image,
                            assets,
                            diags,
                        );
                    }
                    if let Some(tint) = image.get("tint").filter(|v| !v.is_null()) {
                        resolve_field(
                            tint,
                            "color",
                            node_id,
                            &format!("{path}/image/tint"),
                            self,
                            diags,
                        );
                    }
                }
            }
            Some("fallback") => {
                if template_nodes.is_some()
                    && let (Some(id), Some(fb)) = (node_id, node.get("fallback"))
                {
                    check_fallback_asset(id, &format!("{path}/fallback"), fb, assets, diags);
                }
            }
            Some("vector") => {
                if let Some(vector) = node.get("vector") {
                    walk_vector(vector, node_id, &format!("{path}/vector"), self, diags);
                }
            }
            Some("component_instance") => {
                if let Some(payload) = node.get("component_instance") {
                    self.check_instance(payload, path, node_id, diags);
                    walk_instance_tokens(payload, node_id, path, self, assets, diags);
                }
            }
            _ => {}
        }
    }

    fn walk_text(
        &self,
        text: &Value,
        node_id: Option<&str>,
        path: &str,
        assets: Option<&Map<String, Value>>,
        diags: &mut Vec<Diagnostic>,
    ) {
        if let Some(defaults) = text.get("defaults") {
            walk_text_style(
                defaults,
                node_id,
                &format!("{path}/defaults"),
                self,
                assets,
                diags,
            );
        }
        if let Some(source) = text.get("source") {
            match source.get("type").and_then(Value::as_str) {
                Some("literal") => {
                    if let Some(rt) = source.get("value") {
                        walk_rich_text(
                            rt,
                            node_id,
                            &format!("{path}/source/value"),
                            self,
                            assets,
                            diags,
                        );
                    }
                }
                Some("message") => {
                    if let Some(key) = source.get("key").and_then(Value::as_str) {
                        self.check_message(key, node_id, &format!("{path}/source/key"), diags);
                    }
                }
                _ => {}
            }
        }
        if let Some(tp) = text.get("text_path") {
            self.check_text_path(tp, text, node_id, &format!("{path}/text_path"), diags);
        }
    }

    fn check_message(
        &self,
        key: &str,
        node_id: Option<&str>,
        path: &str,
        diags: &mut Vec<Diagnostic>,
    ) {
        let Some(locales) = self
            .localization
            .and_then(|l| l.get("locales"))
            .and_then(Value::as_object)
        else {
            diags.push(err(
                UNKNOWN_MESSAGE,
                path,
                format!("unknown message `{key}`"),
                node_id,
            ));
            return;
        };
        for (lid, _) in locales {
            if self.resolve_message(lid, key).is_none() {
                diags.push(err(
                    UNKNOWN_MESSAGE,
                    path,
                    format!("message `{key}` missing from locale `{lid}` fallback chain"),
                    node_id,
                ));
            }
        }
    }

    fn resolve_message(&self, locale: &str, key: &str) -> Option<&'a Value> {
        let locales = self
            .localization?
            .get("locales")
            .and_then(Value::as_object)?;
        let mut seen = HashSet::new();
        let mut cur = Some(locale);
        while let Some(id) = cur {
            if !seen.insert(id) {
                return None;
            }
            let loc = locales.get(id)?;
            if let Some(msg) = loc
                .get("messages")
                .and_then(Value::as_object)
                .and_then(|m| m.get(key))
            {
                return Some(msg);
            }
            cur = loc.get("fallback").and_then(Value::as_str);
        }
        None
    }

    fn check_text_path(
        &self,
        tp: &Value,
        text: &Value,
        node_id: Option<&str>,
        path: &str,
        diags: &mut Vec<Diagnostic>,
    ) {
        if let Some(pid) = tp.get("path").and_then(Value::as_str)
            && self.paths.is_none_or(|p| !p.contains_key(pid))
        {
            diags.push(err(
                TEXT_PATH_UNKNOWN,
                format!("{path}/path"),
                format!("unknown path `{pid}`"),
                node_id,
            ));
        }
        let Some(source) = text.get("source") else {
            return;
        };
        match source.get("type").and_then(Value::as_str) {
            Some("literal") => {
                if let Some(rt) = source.get("value") {
                    check_text_path_rich(rt, node_id, path, diags);
                }
            }
            Some("message") => {
                if let Some(key) = source.get("key").and_then(Value::as_str) {
                    if let Some(locales) = self
                        .localization
                        .and_then(|l| l.get("locales"))
                        .and_then(Value::as_object)
                    {
                        let mut seen_rt = HashSet::new();
                        for (lid, _) in locales {
                            if let Some(rt) = self.resolve_message(lid, key) {
                                let ptr = rt as *const Value as usize;
                                if seen_rt.insert(ptr) {
                                    check_text_path_rich(rt, node_id, path, diags);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn check_instance(
        &self,
        payload: &Value,
        path: &str,
        node_id: Option<&str>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let Some(def_id) = payload.get("definition").and_then(Value::as_str) else {
            return;
        };
        let Some(def) = self.components.and_then(|c| c.get(def_id)) else {
            diags.push(err(
                UNKNOWN_COMPONENT,
                format!("{path}/component_instance/definition"),
                format!("unknown component `{def_id}`"),
                node_id,
            ));
            return;
        };
        let tnodes = def.get("nodes").and_then(Value::as_object);
        let slots = def.get("slots").and_then(Value::as_object);
        if let Some(variants) = payload.get("variants").and_then(Value::as_object) {
            let axes = def.get("variant_axes").and_then(Value::as_array);
            for (axis, value) in variants {
                let Some(axes) = axes else {
                    diags.push(err(
                        UNKNOWN_VARIANT_AXIS,
                        format!(
                            "{path}/component_instance/variants/{}",
                            escape_pointer(axis)
                        ),
                        format!("unknown variant axis `{axis}`"),
                        node_id,
                    ));
                    continue;
                };
                let found = axes
                    .iter()
                    .find(|a| a.get("name").and_then(Value::as_str) == Some(axis));
                match found {
                    None => diags.push(err(
                        UNKNOWN_VARIANT_AXIS,
                        format!(
                            "{path}/component_instance/variants/{}",
                            escape_pointer(axis)
                        ),
                        format!("unknown variant axis `{axis}`"),
                        node_id,
                    )),
                    Some(axis_obj) => {
                        let Some(val) = value.as_str() else {
                            continue;
                        };
                        if axis_obj
                            .get("values")
                            .and_then(Value::as_object)
                            .is_none_or(|v| !v.contains_key(val))
                        {
                            diags.push(err(
                                UNKNOWN_VARIANT_VALUE,
                                format!(
                                    "{path}/component_instance/variants/{}",
                                    escape_pointer(axis)
                                ),
                                format!("unknown variant value `{val}`"),
                                node_id,
                            ));
                        }
                    }
                }
            }
        }
        if let Some(overrides) = payload.get("overrides").and_then(Value::as_object) {
            for tid in overrides.keys() {
                if tnodes.is_none_or(|n| !n.contains_key(tid)) {
                    diags.push(err(
                        UNKNOWN_TEMPLATE_NODE,
                        format!(
                            "{path}/component_instance/overrides/{}",
                            escape_pointer(tid)
                        ),
                        format!("unknown template node `{tid}`"),
                        node_id,
                    ));
                }
            }
        }
        if let Some(fills) = payload.get("slots").and_then(Value::as_object) {
            for (name, ids) in fills {
                if slots.is_none_or(|s| !s.contains_key(name)) {
                    diags.push(err(
                        UNKNOWN_SLOT,
                        format!("{path}/component_instance/slots/{}", escape_pointer(name)),
                        format!("unknown slot `{name}`"),
                        node_id,
                    ));
                }
                let len = ids.as_array().map(Vec::len).unwrap_or(0);
                if let Some(slot) = slots.and_then(|s| s.get(name)) {
                    if slot.get("allows_multiple").and_then(Value::as_bool) == Some(false)
                        && len > 1
                    {
                        diags.push(err(
                            SLOT_MULTIPLE,
                            format!("{path}/component_instance/slots/{}", escape_pointer(name)),
                            format!("slot `{name}` does not allow multiple"),
                            node_id,
                        ));
                    }
                }
            }
        }
        if let Some(slots) = slots {
            let fills = payload.get("slots").and_then(Value::as_object);
            for (name, slot) in slots {
                let len = fills
                    .and_then(|f| f.get(name))
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0);
                if slot.get("required").and_then(Value::as_bool) == Some(true) && len == 0 {
                    diags.push(err(
                        SLOT_REQUIRED,
                        format!("{path}/component_instance/slots"),
                        format!("slot `{name}` is required"),
                        node_id,
                    ));
                }
            }
        }
    }

    fn token_kind(&self, id: &str) -> Option<&'a str> {
        self.tokens?.get(id)?.get("kind")?.as_str()
    }

    fn token_catalog(&self, id: &str) -> Option<&'a Value> {
        self.tokens?.get(id)?.get("value")
    }

    fn resolved_values(&self, id: &str) -> Vec<&'a Value> {
        let mut out = Vec::new();
        if let Some(v) = self.token_catalog(id) {
            out.push(v);
        }
        let Some(items) = self
            .themes
            .and_then(|t| t.get("items"))
            .and_then(Value::as_object)
        else {
            return out;
        };
        for tid in items.keys() {
            if let Some(v) = self.resolve_under_theme(id, tid)
                && out.last() != Some(&v)
            {
                out.push(v);
            }
        }
        out
    }

    fn resolve_under_theme(&self, token: &str, theme: &str) -> Option<&'a Value> {
        let items = self.themes?.get("items").and_then(Value::as_object)?;
        let chain = theme_chain(items, theme)?;
        let mut val = self.token_catalog(token);
        for tid in chain {
            if let Some(v) = items
                .get(tid)
                .and_then(|t| t.get("values"))
                .and_then(Value::as_object)
                .and_then(|v| v.get(token))
            {
                val = Some(v);
            }
        }
        val
    }
}

fn walk_layout(
    layout: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    const SIZE: &[&str] = &["auto", "px", "percent", "fill"];
    const MIN: &[&str] = &["auto", "px", "percent"];
    const MAX: &[&str] = &["px", "percent", "none"];
    const PAD: &[&str] = &["px", "percent"];
    const MARGIN: &[&str] = &["auto", "px", "percent"];
    const TRACK: &[&str] = &["px", "percent", "fr", "auto", "minmax"];
    walk_length(
        layout.get("width"),
        SIZE,
        false,
        node_id,
        &format!("{path}/width"),
        cat,
        diags,
    );
    walk_length(
        layout.get("height"),
        SIZE,
        false,
        node_id,
        &format!("{path}/height"),
        cat,
        diags,
    );
    walk_length(
        layout.get("flex_basis"),
        SIZE,
        false,
        node_id,
        &format!("{path}/flex_basis"),
        cat,
        diags,
    );
    walk_length(
        layout.get("min_width"),
        MIN,
        false,
        node_id,
        &format!("{path}/min_width"),
        cat,
        diags,
    );
    walk_length(
        layout.get("min_height"),
        MIN,
        false,
        node_id,
        &format!("{path}/min_height"),
        cat,
        diags,
    );
    walk_length(
        layout.get("max_width"),
        MAX,
        false,
        node_id,
        &format!("{path}/max_width"),
        cat,
        diags,
    );
    walk_length(
        layout.get("max_height"),
        MAX,
        false,
        node_id,
        &format!("{path}/max_height"),
        cat,
        diags,
    );
    walk_edges(
        layout.get("padding"),
        PAD,
        true,
        node_id,
        &format!("{path}/padding"),
        cat,
        diags,
    );
    walk_edges(
        layout.get("gap"),
        PAD,
        true,
        node_id,
        &format!("{path}/gap"),
        cat,
        diags,
    );
    walk_edges(
        layout.get("margin"),
        MARGIN,
        false,
        node_id,
        &format!("{path}/margin"),
        cat,
        diags,
    );
    walk_edges(
        layout.get("inset"),
        MARGIN,
        false,
        node_id,
        &format!("{path}/inset"),
        cat,
        diags,
    );
    resolve_field(
        layout.get("flex_grow").unwrap_or(&Value::Null),
        "number",
        node_id,
        &format!("{path}/flex_grow"),
        cat,
        diags,
    );
    resolve_field(
        layout.get("flex_shrink").unwrap_or(&Value::Null),
        "number",
        node_id,
        &format!("{path}/flex_shrink"),
        cat,
        diags,
    );
    if let Some(z) = layout.get("z_index") {
        check_z_index(z, node_id, &format!("{path}/z_index"), cat, diags);
    }
    for key in ["grid_template_columns", "grid_template_rows"] {
        if let Some(tracks) = layout.get(key).and_then(Value::as_array) {
            for (i, track) in tracks.iter().enumerate() {
                walk_length(
                    Some(track),
                    TRACK,
                    false,
                    node_id,
                    &format!("{path}/{key}/{i}"),
                    cat,
                    diags,
                );
            }
        }
    }
}

fn walk_edges(
    edges: Option<&Value>,
    types: &'static [&'static str],
    nonneg: bool,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(obj) = edges.and_then(Value::as_object) else {
        return;
    };
    for (k, v) in obj {
        walk_length(
            Some(v),
            types,
            nonneg,
            node_id,
            &format!("{path}/{}", escape_pointer(k)),
            cat,
            diags,
        );
    }
}

fn walk_style(
    style: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(op) = style.get("opacity") {
        check_opacity(op, node_id, &format!("{path}/opacity"), cat, diags);
    }
    if let Some(fills) = style.get("fills").and_then(Value::as_array) {
        walk_fills(fills, node_id, &format!("{path}/fills"), cat, diags);
    }
    if let Some(borders) = style.get("borders").and_then(Value::as_object) {
        for (side, border) in borders {
            walk_length(
                border.get("width"),
                &["px"],
                true,
                node_id,
                &format!("{path}/borders/{}/width", escape_pointer(side)),
                cat,
                diags,
            );
            if let Some(c) = border.get("color") {
                resolve_field(
                    c,
                    "color",
                    node_id,
                    &format!("{path}/borders/{}/color", escape_pointer(side)),
                    cat,
                    diags,
                );
            }
        }
    }
    walk_edges(
        style.get("corner_radius"),
        &["px", "percent"],
        true,
        node_id,
        &format!("{path}/corner_radius"),
        cat,
        diags,
    );
    if let Some(shadows) = style.get("shadows").and_then(Value::as_array) {
        for (i, sh) in shadows.iter().enumerate() {
            let sp = format!("{path}/shadows/{i}");
            if let Some(c) = sh.get("color") {
                resolve_field(c, "color", node_id, &format!("{sp}/color"), cat, diags);
            }
            walk_length(
                sh.get("blur"),
                &["px"],
                true,
                node_id,
                &format!("{sp}/blur"),
                cat,
                diags,
            );
            walk_length(
                sh.get("offset_x"),
                &["px"],
                false,
                node_id,
                &format!("{sp}/offset_x"),
                cat,
                diags,
            );
            walk_length(
                sh.get("offset_y"),
                &["px"],
                false,
                node_id,
                &format!("{sp}/offset_y"),
                cat,
                diags,
            );
            walk_length(
                sh.get("spread"),
                &["px"],
                false,
                node_id,
                &format!("{sp}/spread"),
                cat,
                diags,
            );
        }
    }
    if let Some(tr) = style.get("translation") {
        walk_length(
            tr.get("x"),
            &["px"],
            false,
            node_id,
            &format!("{path}/translation/x"),
            cat,
            diags,
        );
        walk_length(
            tr.get("y"),
            &["px"],
            false,
            node_id,
            &format!("{path}/translation/y"),
            cat,
            diags,
        );
    }
    if let Some(deg) = style.get("rotation").and_then(|r| r.get("degrees")) {
        resolve_field(
            deg,
            "number",
            node_id,
            &format!("{path}/rotation/degrees"),
            cat,
            diags,
        );
    }
    for (field, axis) in [("scale", "x"), ("scale", "y"), ("skew", "x"), ("skew", "y")] {
        if let Some(v) = style.get(field).and_then(|o| o.get(axis)) {
            resolve_field(
                v,
                "number",
                node_id,
                &format!("{path}/{field}/{axis}"),
                cat,
                diags,
            );
        }
    }
    if let Some(origin) = style.get("transform_origin") {
        for axis in ["x", "y"] {
            if let Some(v) = origin.get(axis) {
                check_unit_interval(
                    v,
                    node_id,
                    &format!("{path}/transform_origin/{axis}"),
                    cat,
                    diags,
                );
            }
        }
    }
    if let Some(filters) = style.get("filters").and_then(Value::as_array) {
        for (i, f) in filters.iter().enumerate() {
            if let Some(v) = f.get("value") {
                resolve_field(
                    v,
                    "number",
                    node_id,
                    &format!("{path}/filters/{i}/value"),
                    cat,
                    diags,
                );
            }
            if let Some(v) = f.get("degrees") {
                resolve_field(
                    v,
                    "number",
                    node_id,
                    &format!("{path}/filters/{i}/degrees"),
                    cat,
                    diags,
                );
            }
        }
    }
}

fn walk_fills(
    fills: &[Value],
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    for (i, fill) in fills.iter().enumerate() {
        let fp = format!("{path}/{i}");
        if let Some(id) = node_id {
            check_fill_stops(id, &fp, fill, diags);
        }
        if let Some(c) = fill.get("color") {
            resolve_field(c, "color", node_id, &format!("{fp}/color"), cat, diags);
        }
        if let Some(stops) = fill.get("stops").and_then(Value::as_array) {
            for (j, stop) in stops.iter().enumerate() {
                if let Some(c) = stop.get("color") {
                    resolve_field(
                        c,
                        "color",
                        node_id,
                        &format!("{fp}/stops/{j}/color"),
                        cat,
                        diags,
                    );
                }
            }
        }
    }
}

fn walk_vector(
    vector: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(layers) = vector.get("layers").and_then(Value::as_array) else {
        return;
    };
    for (i, layer) in layers.iter().enumerate() {
        let lp = format!("{path}/layers/{i}");
        if let Some(pid) = layer.get("path").and_then(Value::as_str)
            && cat.paths.is_none_or(|p| !p.contains_key(pid))
        {
            diags.push(err(
                UNKNOWN_PATH,
                format!("{lp}/path"),
                format!("unknown path `{pid}`"),
                node_id,
            ));
        }
        if let Some(fills) = layer.get("fills").and_then(Value::as_array) {
            walk_fills(fills, node_id, &format!("{lp}/fills"), cat, diags);
        }
        if let Some(strokes) = layer.get("strokes").and_then(Value::as_array) {
            for (j, stroke) in strokes.iter().enumerate() {
                let sp = format!("{lp}/strokes/{j}");
                if let Some(c) = stroke.get("color") {
                    resolve_field(c, "color", node_id, &format!("{sp}/color"), cat, diags);
                }
                walk_length(
                    stroke.get("width"),
                    &["px"],
                    true,
                    node_id,
                    &format!("{sp}/width"),
                    cat,
                    diags,
                );
                if let Some(m) = stroke.get("miter_limit") {
                    resolve_field(
                        m,
                        "number",
                        node_id,
                        &format!("{sp}/miter_limit"),
                        cat,
                        diags,
                    );
                }
                if let Some(d) = stroke.get("dash_offset") {
                    resolve_field(
                        d,
                        "number",
                        node_id,
                        &format!("{sp}/dash_offset"),
                        cat,
                        diags,
                    );
                }
                if let Some(dash) = stroke.get("dash").and_then(Value::as_array) {
                    for (k, item) in dash.iter().enumerate() {
                        resolve_field(
                            item,
                            "number",
                            node_id,
                            &format!("{sp}/dash/{k}"),
                            cat,
                            diags,
                        );
                    }
                }
            }
        }
    }
}

fn walk_masks(
    masks: &[Value],
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    for (i, mask) in masks.iter().enumerate() {
        let mp = format!("{path}/{i}");
        let path_id = mask.get("path").and_then(Value::as_str);
        let asset_id = mask.get("asset").and_then(Value::as_str);
        match (path_id, asset_id) {
            (None, None) => diags.push(err(
                MASK_SOURCE,
                mp,
                "mask must have path or asset",
                node_id,
            )),
            (Some(pid), None) => {
                if cat.paths.is_none_or(|p| !p.contains_key(pid)) {
                    diags.push(err(
                        UNKNOWN_MASK_PATH,
                        format!("{mp}/path"),
                        format!("unknown mask path `{pid}`"),
                        node_id,
                    ));
                }
            }
            (None, Some(aid)) => match asset_kind(assets, aid) {
                None => diags.push(err(
                    MASK_ASSET_KIND,
                    format!("{mp}/asset"),
                    format!("unknown mask asset `{aid}`"),
                    node_id,
                )),
                Some(kind) if kind != "image" && kind != "vector" => diags.push(err(
                    MASK_ASSET_KIND,
                    format!("{mp}/asset"),
                    "mask asset must be image or vector",
                    node_id,
                )),
                Some(_) => {}
            },
            (Some(_), Some(_)) => diags.push(err(
                MASK_SOURCE,
                mp,
                "mask must not set both path and asset",
                node_id,
            )),
        }
    }
}

fn walk_text_style(
    style: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(c) = style.get("color") {
        resolve_field(c, "color", node_id, &format!("{path}/color"), cat, diags);
    }
    if let Some(f) = style.get("family") {
        resolve_field(f, "string", node_id, &format!("{path}/family"), cat, diags);
    }
    if let Some(font) = style.get("font") {
        check_font_value(font, node_id, &format!("{path}/font"), cat, assets, diags);
    }
    walk_length(
        style.get("size"),
        &["px"],
        true,
        node_id,
        &format!("{path}/size"),
        cat,
        diags,
    );
    walk_length(
        style.get("letter_spacing"),
        &["px"],
        false,
        node_id,
        &format!("{path}/letter_spacing"),
        cat,
        diags,
    );
    if let Some(w) = style.get("weight") {
        check_weight(w, node_id, &format!("{path}/weight"), cat, diags);
    }
}

fn walk_rich_text(
    rt: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(blocks) = rt.get("blocks").and_then(Value::as_array) else {
        return;
    };
    for (i, block) in blocks.iter().enumerate() {
        let bp = format!("{path}/blocks/{i}");
        if let Some(spans) = block.get("spans").and_then(Value::as_array) {
            walk_spans(spans, node_id, &format!("{bp}/spans"), cat, assets, diags);
        }
        if let Some(items) = block.get("items").and_then(Value::as_array) {
            for (j, item) in items.iter().enumerate() {
                if let Some(spans) = item.get("spans").and_then(Value::as_array) {
                    walk_spans(
                        spans,
                        node_id,
                        &format!("{bp}/items/{j}/spans"),
                        cat,
                        assets,
                        diags,
                    );
                }
            }
        }
    }
}

fn walk_spans(
    spans: &[Value],
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    for (i, span) in spans.iter().enumerate() {
        let sp = format!("{path}/{i}");
        if let Some(style) = span.get("style") {
            walk_text_style(style, node_id, &format!("{sp}/style"), cat, assets, diags);
        }
        if span.get("type").and_then(Value::as_str) == Some("inline_image") {
            if let Some(aid) = span.get("asset").and_then(Value::as_str) {
                match asset_kind(assets, aid) {
                    None => diags.push(err(
                        UNRESOLVED_INLINE_IMAGE,
                        format!("{sp}/asset"),
                        format!("unknown inline image `{aid}`"),
                        node_id,
                    )),
                    Some(kind) if kind != "image" && kind != "vector" => diags.push(err(
                        ASSET_KIND_MISMATCH,
                        format!("{sp}/asset"),
                        "inline image must be image or vector",
                        node_id,
                    )),
                    Some(_) => {}
                }
            }
        }
    }
}

fn walk_instance_tokens(
    payload: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(overrides) = payload.get("overrides").and_then(Value::as_object) {
        for (tid, patch) in overrides {
            walk_patch(
                patch,
                node_id,
                &format!(
                    "{path}/component_instance/overrides/{}",
                    escape_pointer(tid)
                ),
                cat,
                assets,
                diags,
            );
        }
    }
}

fn walk_patch(
    patch: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(layout) = patch.get("layout") {
        walk_layout(layout, node_id, &format!("{path}/layout"), cat, diags);
    }
    if let Some(style) = patch.get("style") {
        walk_style(style, node_id, &format!("{path}/style"), cat, diags);
    }
    if let Some(text) = patch.get("text") {
        cat.walk_text(text, node_id, &format!("{path}/text"), assets, diags);
    }
    if let Some(image) = patch.get("image") {
        if let Some(tint) = image.get("tint").filter(|v| !v.is_null()) {
            resolve_field(
                tint,
                "color",
                node_id,
                &format!("{path}/image/tint"),
                cat,
                diags,
            );
        }
        if let (Some(id), Some(_)) = (node_id, image.get("asset")) {
            check_image_asset(id, &format!("{path}/image"), image, assets, diags);
        }
    }
    if let Some(vector) = patch.get("vector") {
        walk_vector(vector, node_id, &format!("{path}/vector"), cat, diags);
    }
    if let Some(inst) = patch.get("component_instance") {
        if let Some(definition) = inst.get("definition").and_then(Value::as_str)
            && cat
                .components
                .is_none_or(|defs| !defs.contains_key(definition))
        {
            diags.push(err(
                UNKNOWN_COMPONENT,
                format!("{path}/component_instance/definition"),
                format!("unknown component `{definition}`"),
                node_id,
            ));
        }
        walk_instance_tokens(inst, node_id, path, cat, assets, diags);
    }
}

fn check_image_asset(
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
                crate::UNRESOLVED_IMAGE,
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
}

fn walk_length(
    value: Option<&Value>,
    types: &'static [&'static str],
    nonneg: bool,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(value) = value.filter(|v| !v.is_null()) else {
        return;
    };
    if let Some(tok) = token_ref(value) {
        if !check_token(tok, "length", node_id, path, cat, diags) {
            return;
        }
        for resolved in cat.resolved_values(tok) {
            if !length_ok(resolved, types, nonneg) {
                diags.push(err(
                    LENGTH_CONTEXT,
                    path,
                    "resolved length is not valid in this field",
                    node_id,
                ));
            }
        }
        return;
    }
    match value.get("type").and_then(Value::as_str) {
        Some("fill") => {
            if let Some(w) = value.get("weight") {
                resolve_field(w, "number", node_id, &format!("{path}/weight"), cat, diags);
            }
        }
        Some("fr") => {
            if let Some(v) = value.get("value") {
                resolve_field(v, "number", node_id, &format!("{path}/value"), cat, diags);
            }
        }
        Some("minmax") => {
            walk_length(
                value.get("min"),
                &["px", "percent", "fr", "auto"],
                false,
                node_id,
                &format!("{path}/min"),
                cat,
                diags,
            );
            walk_length(
                value.get("max"),
                &["px", "percent", "fr", "auto"],
                false,
                node_id,
                &format!("{path}/max"),
                cat,
                diags,
            );
        }
        _ => {}
    }
}

fn resolve_field(
    value: &Value,
    kind: &str,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    if value.is_null() {
        return;
    }
    let Some(tok) = token_ref(value) else {
        return;
    };
    check_token(tok, kind, node_id, path, cat, diags);
}

fn check_token(
    tok: &str,
    expected: &str,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) -> bool {
    match cat.token_kind(tok) {
        None => {
            diags.push(err(
                UNKNOWN_TOKEN,
                path,
                format!("unknown token `{tok}`"),
                node_id,
            ));
            false
        }
        Some(kind) if kind != expected => {
            diags.push(err(
                TOKEN_KIND_MISMATCH,
                path,
                format!("token `{tok}` is {kind}, expected {expected}"),
                node_id,
            ));
            false
        }
        Some(_) => true,
    }
}

fn check_font_value(
    value: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(tok) = token_ref(value) {
        if !check_token(tok, "font", node_id, path, cat, diags) {
            return;
        }
        for resolved in cat.resolved_values(tok) {
            if let Some(id) = resolved.as_str() {
                check_font_id(id, path, node_id, assets, diags);
            }
        }
        return;
    }
    if let Some(id) = value.as_str() {
        check_font_id(id, path, node_id, assets, diags);
    }
}

fn check_font_id(
    id: &str,
    path: &str,
    node_id: Option<&str>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    match asset_kind(assets, id) {
        None => diags.push(err(
            UNRESOLVED_FONT,
            path,
            format!("unknown font `{id}`"),
            node_id,
        )),
        Some(kind) if kind != "font" => diags.push(err(
            ASSET_KIND_MISMATCH,
            path,
            "font must reference a font asset",
            node_id,
        )),
        Some(_) => {}
    }
}

fn check_z_index(
    value: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(tok) = token_ref(value) else {
        return;
    };
    if !check_token(tok, "number", node_id, path, cat, diags) {
        return;
    }
    for resolved in cat.resolved_values(tok) {
        if !is_ijson_int(resolved) {
            diags.push(err(
                TOKEN_KIND_MISMATCH,
                path,
                "z_index token must resolve to an integer",
                node_id,
            ));
        }
    }
}

fn check_weight(
    value: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(tok) = token_ref(value) else {
        return;
    };
    if !check_token(tok, "number", node_id, path, cat, diags) {
        return;
    }
    for resolved in cat.resolved_values(tok) {
        let ok = resolved
            .as_i64()
            .or_else(|| resolved.as_u64().and_then(|u| i64::try_from(u).ok()))
            .is_some_and(|n| (1..=1000).contains(&n));
        if !ok {
            diags.push(err(
                TOKEN_KIND_MISMATCH,
                path,
                "weight token must resolve to an integer in 1..=1000",
                node_id,
            ));
        }
    }
}

fn check_opacity(
    value: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(tok) = token_ref(value) else {
        return;
    };
    if !check_token(tok, "number", node_id, path, cat, diags) {
        return;
    }
    for resolved in cat.resolved_values(tok) {
        if resolved.as_f64().is_none_or(|n| !(0.0..=1.0).contains(&n)) {
            diags.push(err(
                TOKEN_KIND_MISMATCH,
                path,
                "opacity token must resolve to 0..=1",
                node_id,
            ));
        }
    }
}

fn check_unit_interval(
    value: &Value,
    node_id: Option<&str>,
    path: &str,
    cat: &Catalogs<'_>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(tok) = token_ref(value) else {
        return;
    };
    if !check_token(tok, "number", node_id, path, cat, diags) {
        return;
    }
    for resolved in cat.resolved_values(tok) {
        if resolved.as_f64().is_none_or(|n| !(0.0..=1.0).contains(&n)) {
            diags.push(err(
                TOKEN_KIND_MISMATCH,
                path,
                "value must resolve to 0..=1",
                node_id,
            ));
        }
    }
}

fn check_path_commands(
    cmds: &[Value],
    path: &str,
    node_id: Option<&str>,
    diags: &mut Vec<Diagnostic>,
) {
    let mut current = false;
    for (i, cmd) in cmds.iter().enumerate() {
        match cmd.get("type").and_then(Value::as_str) {
            Some("move") => current = true,
            Some("line" | "quadratic" | "cubic" | "close") if !current || i == 0 => {
                diags.push(err(
                    PATH_COMMANDS,
                    format!("{path}/{i}"),
                    "path command requires a current point; first command must be move",
                    node_id,
                ));
                return;
            }
            _ => {}
        }
    }
}

fn check_text_path_rich(
    rt: &Value,
    node_id: Option<&str>,
    path: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(blocks) = rt.get("blocks").and_then(Value::as_array) else {
        return;
    };
    if blocks.len() != 1 || blocks[0].get("type").and_then(Value::as_str) != Some("paragraph") {
        diags.push(err(
            TEXT_PATH_INCOMPATIBLE,
            path,
            "text_path rich text must be exactly one paragraph",
            node_id,
        ));
        return;
    }
    let Some(spans) = blocks[0].get("spans").and_then(Value::as_array) else {
        return;
    };
    if spans.iter().any(|s| {
        !matches!(
            s.get("type").and_then(Value::as_str),
            Some("text") | Some("line_break")
        )
    }) {
        diags.push(err(
            TEXT_PATH_INCOMPATIBLE,
            path,
            "text_path spans must be only text or line_break",
            node_id,
        ));
    }
}

fn check_template_forest(
    _cid: &str,
    def: &Value,
    nodes: &Map<String, Value>,
    base: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let mut parent: HashMap<&str, &str> = HashMap::new();
    for (id, node) in nodes {
        let mut seen = HashSet::new();
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for (i, child) in children.iter().enumerate() {
                if let Some(cid) = child.as_str() {
                    record_template_edge(
                        id,
                        cid,
                        &format!("{base}/nodes/{}/children/{i}", escape_pointer(id)),
                        nodes,
                        &mut seen,
                        &mut parent,
                        diags,
                    );
                }
            }
        }
        if let Some(slots) = node
            .get("component_instance")
            .and_then(|instance| instance.get("slots"))
            .and_then(Value::as_object)
        {
            for (slot, children) in slots {
                let Some(children) = children.as_array() else {
                    continue;
                };
                for (i, child) in children.iter().enumerate() {
                    if let Some(cid) = child.as_str() {
                        record_template_edge(
                            id,
                            cid,
                            &format!(
                                "{base}/nodes/{}/component_instance/slots/{}/{i}",
                                escape_pointer(id),
                                escape_pointer(slot)
                            ),
                            nodes,
                            &mut seen,
                            &mut parent,
                            diags,
                        );
                    }
                }
            }
        }
    }
    if let Some(hit) = template_cycle(nodes) {
        diags.push(
            Diagnostic::error(
                TEMPLATE_CYCLE,
                format!("{base}/nodes/{}", escape_pointer(hit)),
                "template child graph is cyclic",
            )
            .with_node(hit),
        );
    }
    if let Some(root) = def.get("root").and_then(Value::as_str)
        && nodes.contains_key(root)
    {
        let mut reach = HashSet::new();
        walk_template(root, nodes, &mut reach);
        for id in nodes.keys() {
            if !reach.contains(id.as_str()) {
                diags.push(
                    Diagnostic::error(
                        TEMPLATE_UNREACHABLE,
                        format!("{base}/nodes/{}", escape_pointer(id)),
                        format!("`{id}` is not reachable from the component root"),
                    )
                    .with_node(id),
                );
            }
        }
    }
}

fn record_template_edge<'a>(
    parent_id: &'a str,
    child_id: &'a str,
    path: &str,
    nodes: &Map<String, Value>,
    seen: &mut HashSet<&'a str>,
    parent: &mut HashMap<&'a str, &'a str>,
    diags: &mut Vec<Diagnostic>,
) {
    if !seen.insert(child_id) {
        diags.push(
            Diagnostic::error(
                TEMPLATE_DUPLICATE_CHILD,
                path,
                format!("duplicate template child `{child_id}`"),
            )
            .with_node(parent_id),
        );
    } else if !nodes.contains_key(child_id) {
        diags.push(
            Diagnostic::error(
                TEMPLATE_UNKNOWN_CHILD,
                path,
                format!("unknown template child `{child_id}`"),
            )
            .with_node(parent_id),
        );
    } else if let Some(existing) = parent.insert(child_id, parent_id) {
        diags.push(
            Diagnostic::error(
                TEMPLATE_MULTIPLE_PARENTS,
                path,
                format!("`{child_id}` already has parent `{existing}`"),
            )
            .with_node(child_id),
        );
    }
}

fn template_edges(node: &Value) -> impl Iterator<Item = &str> {
    let children = node
        .get("children")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str);
    let slots = node
        .get("component_instance")
        .and_then(|instance| instance.get("slots"))
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(Map::values)
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_str);
    children.chain(slots)
}

fn template_cycle(nodes: &Map<String, Value>) -> Option<&str> {
    let mut state: HashMap<&str, u8> = HashMap::new();
    fn dfs<'a>(
        id: &'a str,
        nodes: &'a Map<String, Value>,
        state: &mut HashMap<&'a str, u8>,
    ) -> Option<&'a str> {
        state.insert(id, 1);
        if let Some(node) = nodes.get(id) {
            for cid in template_edges(node) {
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

fn walk_template<'a>(id: &'a str, nodes: &'a Map<String, Value>, seen: &mut HashSet<&'a str>) {
    if !seen.insert(id) {
        return;
    }
    let Some(node) = nodes.get(id) else {
        return;
    };
    for cid in template_edges(node) {
        walk_template(cid, nodes, seen);
    }
}

fn check_slot_defs(
    def: &Value,
    nodes: &Map<String, Value>,
    base: &str,
    diags: &mut Vec<Diagnostic>,
) {
    let slots = def.get("slots").and_then(Value::as_object);
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for (tid, node) in nodes {
        if node.get("type").and_then(Value::as_str) != Some("slot") {
            continue;
        }
        let Some(name) = node
            .get("slot")
            .and_then(|s| s.get("name"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if slots.is_none_or(|s| !s.contains_key(name)) {
            diags.push(
                Diagnostic::error(
                    UNKNOWN_SLOT,
                    format!("{base}/nodes/{}/slot/name", escape_pointer(tid)),
                    format!("unknown slot `{name}`"),
                )
                .with_node(tid),
            );
        }
        *counts.entry(name).or_insert(0) += 1;
    }
    if let Some(slots) = slots {
        for name in slots.keys() {
            if counts.get(name.as_str()).copied().unwrap_or(0) != 1 {
                diags.push(Diagnostic::error(
                    UNKNOWN_SLOT,
                    format!("{base}/slots/{}", escape_pointer(name)),
                    format!("slot `{name}` must be referenced by exactly one slot node"),
                ));
            }
        }
    }
}

fn check_variant_axes(
    def: &Value,
    nodes: &Map<String, Value>,
    base: &str,
    catalogs: &Catalogs<'_>,
    assets: Option<&Map<String, Value>>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(axes) = def.get("variant_axes").and_then(Value::as_array) else {
        return;
    };
    let mut names = HashSet::new();
    for (i, axis) in axes.iter().enumerate() {
        let ap = format!("{base}/variant_axes/{i}");
        if let Some(name) = axis.get("name").and_then(Value::as_str)
            && !names.insert(name)
        {
            diags.push(Diagnostic::error(
                UNKNOWN_VARIANT_AXIS,
                format!("{ap}/name"),
                format!("duplicate variant axis `{name}`"),
            ));
        }
        let values = axis.get("values").and_then(Value::as_object);
        if let Some(default) = axis.get("default").and_then(Value::as_str)
            && values.is_none_or(|v| !v.contains_key(default))
        {
            diags.push(Diagnostic::error(
                VARIANT_DEFAULT,
                format!("{ap}/default"),
                format!("variant default `{default}` is not a value"),
            ));
        }
        if let Some(values) = values {
            for (vname, patch_map) in values {
                let Some(patch_map) = patch_map.as_object() else {
                    continue;
                };
                for tid in patch_map.keys() {
                    if !nodes.contains_key(tid) {
                        diags.push(Diagnostic::error(
                            UNKNOWN_TEMPLATE_NODE,
                            format!(
                                "{ap}/values/{}/{}",
                                escape_pointer(vname),
                                escape_pointer(tid)
                            ),
                            format!("unknown template node `{tid}`"),
                        ));
                    }
                }
                let path = format!("{ap}/values/{}", escape_pointer(vname));
                let mut patched_nodes = nodes.clone();
                apply_patch_map(&mut patched_nodes, patch_map);
                check_template_forest("", def, &patched_nodes, &path, diags);
                check_slot_defs(def, &patched_nodes, &path, diags);
                for tid in patch_map.keys().filter(|tid| nodes.contains_key(*tid)) {
                    let path = format!(
                        "{ap}/values/{}/{}",
                        escape_pointer(vname),
                        escape_pointer(tid)
                    );
                    catalogs.walk_node(
                        &patched_nodes[tid],
                        Some(tid),
                        &path,
                        Some(&patched_nodes),
                        assets,
                        diags,
                    );
                }
            }
        }
    }
}

fn component_deps(def: &Value) -> Vec<&str> {
    fn collect<'a>(value: &'a Value, out: &mut Vec<&'a str>) {
        match value {
            Value::Object(obj) => {
                if let Some(payload) = obj.get("component_instance")
                    && let Some(id) = payload.get("definition").and_then(Value::as_str)
                {
                    out.push(id);
                }
                for value in obj.values() {
                    collect(value, out);
                }
            }
            Value::Array(items) => {
                for value in items {
                    collect(value, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    collect(def, &mut out);
    out
}

fn instantiate_component<'a>(
    payload: &Value,
    components: Option<&'a Map<String, Value>>,
) -> Option<(&'a Value, Map<String, Value>)> {
    let definition = payload.get("definition")?.as_str()?;
    let def = components?.get(definition)?;
    let mut nodes = def.get("nodes")?.as_object()?.clone();
    let selected = payload.get("variants").and_then(Value::as_object);
    if let Some(axes) = def.get("variant_axes").and_then(Value::as_array) {
        for axis in axes {
            let name = axis.get("name")?.as_str()?;
            let value = selected
                .and_then(|values| values.get(name))
                .and_then(Value::as_str)
                .or_else(|| axis.get("default").and_then(Value::as_str))?;
            if let Some(patches) = axis
                .get("values")
                .and_then(Value::as_object)
                .and_then(|values| values.get(value))
                .and_then(Value::as_object)
            {
                apply_patch_map(&mut nodes, patches);
            }
        }
    }
    if let Some(overrides) = payload.get("overrides").and_then(Value::as_object) {
        apply_patch_map(&mut nodes, overrides);
    }
    Some((def, nodes))
}

fn apply_patch_map(nodes: &mut Map<String, Value>, patches: &Map<String, Value>) {
    for (id, patch) in patches {
        if let Some(node) = nodes.get_mut(id) {
            apply_node_patch(node, patch);
        }
    }
}

fn apply_node_patch(node: &mut Value, patch: &Value) {
    let (Some(node), Some(patch)) = (node.as_object_mut(), patch.as_object()) else {
        return;
    };
    for (key, value) in patch {
        if matches!(
            key.as_str(),
            "container" | "text" | "image" | "vector" | "fallback" | "component_instance" | "slot"
        ) {
            let target = node.entry(key.clone()).or_insert_with(json_object);
            merge_payload(target, value, key == "text");
        } else {
            node.insert(key.clone(), value.clone());
        }
    }
}

fn merge_payload(target: &mut Value, patch: &Value, merge_text_defaults: bool) {
    let (Some(target), Some(patch)) = (target.as_object_mut(), patch.as_object()) else {
        return;
    };
    for (key, value) in patch {
        if merge_text_defaults && key == "defaults" {
            let defaults = target.entry(key.clone()).or_insert_with(json_object);
            if let (Some(defaults), Some(changes)) = (defaults.as_object_mut(), value.as_object()) {
                defaults.extend(changes.clone());
                continue;
            }
        }
        target.insert(key.clone(), value.clone());
    }
}

fn json_object() -> Value {
    Value::Object(Map::new())
}

fn expand_instance(
    payload: &Value,
    base: Option<&str>,
    components: Option<&Map<String, Value>>,
    used: &mut HashMap<String, String>,
    origin: &str,
    stack: &mut HashSet<String>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(def_id) = payload.get("definition").and_then(Value::as_str) else {
        return;
    };
    if !stack.insert(def_id.to_string()) {
        return;
    }
    let Some((_def, tnodes)) = instantiate_component(payload, components) else {
        stack.remove(def_id);
        return;
    };
    for (_tid, tnode) in &tnodes {
        let part = tnode.get("runtime_part_id").and_then(Value::as_str);
        let expanded = match (base, part) {
            (Some(b), Some(p)) => Some(format!("{b}/{p}")),
            _ => None,
        };
        if let Some(eid) = expanded.as_deref() {
            if !valid_runtime_id(eid) {
                diags.push(
                    Diagnostic::error(
                        EXPANDED_RUNTIME_ID,
                        format!("/nodes/{}", escape_pointer(origin)),
                        format!("expanded runtime_id `{eid}` is invalid"),
                    )
                    .with_node(origin),
                );
            }
            if let Some(prev) = used.insert(eid.to_string(), origin.to_string()) {
                diags.push(
                    Diagnostic::error(
                        DUPLICATE_EXPANDED_RUNTIME_ID,
                        format!("/nodes/{}", escape_pointer(origin)),
                        format!("expanded runtime_id `{eid}` already used by `{prev}`"),
                    )
                    .with_node(origin),
                );
            }
        }
        if tnode.get("type").and_then(Value::as_str) == Some("component_instance")
            && let Some(nested) = tnode.get("component_instance")
        {
            expand_instance(
                nested,
                expanded.as_deref(),
                components,
                used,
                origin,
                stack,
                diags,
            );
        }
    }
    stack.remove(def_id);
}

fn valid_runtime_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some('A'..='Z' | 'a'..='z'))
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
}

fn token_ref(value: &Value) -> Option<&str> {
    let obj = value.as_object()?;
    if obj.len() == 1 {
        obj.get("token")?.as_str()
    } else {
        None
    }
}

fn literal_matches_kind(kind: &str, value: &Value) -> bool {
    if token_ref(value).is_some() {
        return false;
    }
    match kind {
        "color" => {
            value.get("space").is_some()
                && value.get("r").is_some()
                && value.get("g").is_some()
                && value.get("b").is_some()
                && value.get("a").is_some()
        }
        "number" => value.is_number(),
        "length" => matches!(
            value.get("type").and_then(Value::as_str),
            Some("auto" | "px" | "percent" | "fill" | "fr" | "none" | "minmax")
        ),
        "string" => value.as_str().is_some_and(|s| !s.is_empty()),
        "font" => value
            .as_str()
            .is_some_and(|s| s.starts_with("font-") && s.len() == 69),
        _ => false,
    }
}

fn length_ok(value: &Value, types: &[&str], nonneg: bool) -> bool {
    let Some(ty) = value.get("type").and_then(Value::as_str) else {
        return false;
    };
    if !types.contains(&ty) {
        return false;
    }
    if nonneg
        && let Some(n) = value.get("value").and_then(Value::as_f64)
        && n < 0.0
    {
        return false;
    }
    if ty == "fill"
        && value
            .get("weight")
            .and_then(Value::as_f64)
            .is_none_or(|w| w <= 0.0)
    {
        return false;
    }
    true
}

fn is_ijson_int(value: &Value) -> bool {
    value.as_i64().is_some() || value.as_u64().is_some_and(|u| u <= 9_007_199_254_740_991)
}

fn num_or_null(v: Option<&Value>) -> Option<f64> {
    match v {
        None | Some(Value::Null) => None,
        Some(v) => v.as_f64(),
    }
}

fn rank_min(min: Option<f64>) -> f64 {
    min.unwrap_or(f64::NEG_INFINITY)
}

fn theme_chain<'a>(items: &'a Map<String, Value>, start: &'a str) -> Option<Vec<&'a str>> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut cur = Some(start);
    while let Some(id) = cur {
        if !seen.insert(id) {
            return None;
        }
        let theme = items.get(id)?;
        chain.push(items.get_key_value(id)?.0.as_str());
        cur = theme.get("extends").and_then(Value::as_str);
    }
    chain.reverse();
    Some(chain)
}

fn graph_cycle<'a>(
    items: &'a Map<String, Value>,
    mut deps: impl FnMut(&'a Value) -> Vec<&'a str>,
) -> Option<&'a str> {
    let mut state: HashMap<&str, u8> = HashMap::new();
    fn dfs<'a>(
        id: &'a str,
        items: &'a Map<String, Value>,
        deps: &mut dyn FnMut(&'a Value) -> Vec<&'a str>,
        state: &mut HashMap<&'a str, u8>,
    ) -> Option<&'a str> {
        state.insert(id, 1);
        if let Some(item) = items.get(id) {
            for next in deps(item) {
                if !items.contains_key(next) {
                    continue;
                }
                match state.get(next).copied() {
                    Some(1) => return Some(next),
                    Some(2) => {}
                    _ => {
                        if let Some(hit) = dfs(next, items, deps, state) {
                            return Some(hit);
                        }
                    }
                }
            }
        }
        state.insert(id, 2);
        None
    }
    for id in items.keys() {
        if !state.contains_key(id.as_str())
            && let Some(hit) = dfs(id, items, &mut deps, &mut state)
        {
            return Some(hit);
        }
    }
    None
}

fn err(
    code: &str,
    path: impl Into<String>,
    message: impl Into<String>,
    node_id: Option<&str>,
) -> Diagnostic {
    let d = Diagnostic::error(code, path, message);
    match node_id {
        Some(id) => d.with_node(id),
        None => d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn codes_from(resources: Value, nodes: Value) -> Vec<String> {
        let mut root = Map::new();
        root.insert("resources".into(), resources);
        root.insert("nodes".into(), nodes);
        root.insert("entrypoints".into(), json!({"default": "root"}));
        root.insert("assets".into(), json!({}));
        root.insert("diagnostics".into(), json!([]));
        root.insert("document".into(), json!({"name": "t"}));
        let nodes = root.get("nodes").and_then(Value::as_object);
        let assets = root.get("assets").and_then(Value::as_object);
        let mut diags = Vec::new();
        check(
            &root,
            nodes,
            assets,
            &CheckOptions {
                check_assets: false,
                check_canonical: false,
                asset_root: None,
                package_name: None,
            },
            &mut diags,
        );
        diags.into_iter().map(|d| d.code).collect()
    }

    #[test]
    fn breakpoint_overlap_and_bounds() {
        let resources = json!({
            "breakpoints": {
                "a": {"min_width": 0, "max_width": 100},
                "b": {"min_width": 50, "max_width": 200}
            }
        });
        let codes = codes_from(resources, json!({}));
        assert!(codes.contains(&BREAKPOINT_OVERLAP.to_string()));

        let resources = json!({
            "breakpoints": {
                "bad": {"min_width": 10, "max_width": 10}
            }
        });
        let codes = codes_from(resources, json!({}));
        assert!(codes.contains(&BREAKPOINT_BOUNDS.to_string()));
    }

    #[test]
    fn theme_cycle_and_type_mismatch() {
        let resources = json!({
            "tokens": {
                "color.fg": {
                    "kind": "color",
                    "value": {"a": 1, "b": 0, "g": 0, "r": 0, "space": "srgb"}
                }
            },
            "themes": {
                "default": "a",
                "items": {
                    "a": {"extends": "b", "values": {}},
                    "b": {"extends": "a", "values": {"color.fg": 1}}
                }
            }
        });
        let codes = codes_from(resources, json!({}));
        assert!(codes.contains(&THEME_CYCLE.to_string()));
        assert!(codes.contains(&THEME_VALUE_KIND.to_string()));
    }

    #[test]
    fn locale_cycle_and_missing_message() {
        let resources = json!({
            "localization": {
                "default_locale": "en",
                "locales": {
                    "en": {"fallback": "de", "messages": {}},
                    "de": {"fallback": "en", "messages": {}}
                }
            }
        });
        let nodes = json!({
            "root": {
                "type": "text",
                "source_id": "root",
                "text": {"source": {"type": "message", "key": "missing"}}
            }
        });
        let codes = codes_from(resources, nodes);
        assert!(codes.contains(&LOCALE_CYCLE.to_string()));
        assert!(codes.contains(&UNKNOWN_MESSAGE.to_string()));
    }

    #[test]
    fn path_sequencing() {
        let resources = json!({
            "paths": {
                "bad": {
                    "fill_rule": "nonzero",
                    "commands": [{"type": "line", "x": 1, "y": 1}]
                }
            }
        });
        let codes = codes_from(resources, json!({}));
        assert!(codes.contains(&PATH_COMMANDS.to_string()));
    }

    #[test]
    fn component_cycle_slot_and_expanded_id() {
        let resources = json!({
            "components": {
                "A": {
                    "root": "a",
                    "nodes": {
                        "a": {
                            "template_id": "a",
                            "type": "component_instance",
                            "runtime_part_id": "part",
                            "children": [],
                            "component_instance": {
                                "definition": "B",
                                "variants": {},
                                "slots": {},
                                "overrides": {}
                            }
                        }
                    },
                    "slots": {},
                    "variant_axes": []
                },
                "B": {
                    "root": "b",
                    "nodes": {
                        "b": {
                            "template_id": "b",
                            "type": "component_instance",
                            "children": [],
                            "component_instance": {
                                "definition": "A",
                                "variants": {},
                                "slots": {},
                                "overrides": {}
                            }
                        }
                    },
                    "slots": {},
                    "variant_axes": []
                },
                "Btn": {
                    "root": "rootn",
                    "nodes": {
                        "rootn": {
                            "template_id": "rootn",
                            "type": "slot",
                            "runtime_part_id": "icon",
                            "children": [],
                            "slot": {"name": "icon"}
                        }
                    },
                    "slots": {
                        "icon": {"required": true, "allows_multiple": false}
                    },
                    "variant_axes": []
                }
            }
        });
        let nodes = json!({
            "root": {
                "type": "component_instance",
                "source_id": "root",
                "runtime_id": "cta",
                "children": [],
                "component_instance": {
                    "definition": "Btn",
                    "variants": {},
                    "slots": {"icon": []},
                    "overrides": {}
                }
            }
        });
        let codes = codes_from(resources, nodes);
        assert!(codes.contains(&COMPONENT_CYCLE.to_string()));
        assert!(codes.contains(&SLOT_REQUIRED.to_string()));
    }

    #[test]
    fn expanded_id_collision() {
        let resources = json!({
            "components": {
                "Btn": {
                    "root": "lab",
                    "nodes": {
                        "lab": {
                            "template_id": "lab",
                            "type": "container",
                            "runtime_part_id": "label",
                            "children": [],
                            "container": {}
                        }
                    },
                    "slots": {},
                    "variant_axes": []
                }
            }
        });
        let nodes = json!({
            "root": {
                "type": "component_instance",
                "source_id": "root",
                "runtime_id": "cta",
                "children": [],
                "component_instance": {
                    "definition": "Btn",
                    "variants": {},
                    "slots": {},
                    "overrides": {}
                }
            },
            "other": {
                "type": "container",
                "source_id": "other",
                "runtime_id": "cta/label",
                "children": [],
                "container": {}
            }
        });
        let codes = codes_from(resources, nodes);
        assert!(codes.contains(&DUPLICATE_EXPANDED_RUNTIME_ID.to_string()));
    }

    #[test]
    fn sparse_patches_are_checked_after_merge() {
        let image = format!("image-{}", "a".repeat(64));
        let resources = json!({
            "components": {
                "Card": {
                    "root": "root",
                    "nodes": {
                        "root": {
                            "template_id": "root",
                            "type": "container",
                            "children": ["image", "text"],
                            "container": {}
                        },
                        "image": {
                            "template_id": "image",
                            "type": "image",
                            "children": [],
                            "image": {
                                "asset": image,
                                "intrinsic_width": 10,
                                "intrinsic_height": 10,
                                "nine_slice": null
                            }
                        },
                        "text": {
                            "template_id": "text",
                            "type": "text",
                            "children": [],
                            "text": {
                                "source": {
                                    "type": "literal",
                                    "value": {
                                        "blocks": [{"type": "unordered_list", "items": []}]
                                    }
                                }
                            }
                        }
                    },
                    "slots": {},
                    "variant_axes": [{
                        "name": "state",
                        "default": "bad",
                        "values": {
                            "bad": {
                                "image": {
                                    "image": {
                                        "nine_slice": {"top": 6, "right": 6, "bottom": 5, "left": 5}
                                    }
                                },
                                "text": {
                                    "text": {
                                        "text_path": {"path": "line"}
                                    }
                                }
                            }
                        }
                    }]
                }
            },
            "paths": {
                "line": {
                    "commands": [{"type": "move", "x": 0, "y": 0}],
                    "fill_rule": "nonzero"
                }
            }
        });
        let nodes = json!({
            "root": {
                "type": "component_instance",
                "source_id": "root",
                "children": [],
                "component_instance": {
                    "definition": "Card",
                    "variants": {},
                    "slots": {},
                    "overrides": {}
                }
            }
        });
        let codes = codes_from(resources, nodes);
        assert!(codes.contains(&crate::NINE_SLICE_BOUNDS.to_string()));
        assert!(codes.contains(&TEXT_PATH_INCOMPATIBLE.to_string()));
    }

    #[test]
    fn themed_fill_weight_is_checked_after_resolution() {
        let resources = json!({
            "tokens": {
                "size": {"kind": "length", "value": {"type": "px", "value": 1}}
            },
            "themes": {
                "default": "bad",
                "items": {
                    "bad": {
                        "extends": null,
                        "values": {"size": {"type": "fill", "weight": 0}}
                    }
                }
            }
        });
        let nodes = json!({
            "root": {
                "type": "container",
                "source_id": "root",
                "children": [],
                "container": {},
                "layout": {"width": {"token": "size"}}
            }
        });
        let codes = codes_from(resources, nodes);
        assert!(codes.contains(&LENGTH_CONTEXT.to_string()));
    }

    #[test]
    fn token_ref_unknown_and_kind() {
        let resources = json!({
            "tokens": {
                "space.md": {"kind": "length", "value": {"type": "px", "value": 8}}
            }
        });
        let nodes = json!({
            "root": {
                "type": "container",
                "source_id": "root",
                "children": [],
                "container": {},
                "style": {"opacity": {"token": "space.md"}},
                "layout": {"width": {"token": "missing"}}
            }
        });
        let codes = codes_from(resources, nodes);
        assert!(codes.contains(&TOKEN_KIND_MISMATCH.to_string()));
        assert!(codes.contains(&UNKNOWN_TOKEN.to_string()));
    }
}
