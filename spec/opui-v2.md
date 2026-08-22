# openpencil-runtime-ui v2

Standalone interchange format `openpencil-runtime-ui`, `schema_version` integer `2`.
Machine contract: `spec/opui-v2.schema.json` (JSON Schema draft 2020-12).
This document is the semantic contract. Where they conflict, this prose wins
for meaning; the schema wins for structural membership.

v2 is a static, fluid, offline runtime forest with data-only catalogs:
breakpoints, tokens, themes, localization, vector paths, and component
templates. No animation, no scripting, no interaction handlers, no network,
no embedded binary, no serialized scroll position.

The manifest contains only engine-neutral runtime data. It MUST NOT contain
Bevy or Rust type names, OpenPencil's internal object serialization,
executable behavior, scripts, game logic, animation, interaction handlers,
absolute filesystem paths, `data:` URIs, network fonts, or resources that
require an automatic network fetch.

This document does not `$ref` v1. v1 concepts are restated where they still
apply. §23 is a conceptual mapping only.

## 1. Roles

| Role | Job |
| --- | --- |
| Exporter | Emit a deterministic package: manifest plus sibling asset tree. |
| Checker | Validate schema rules and semantic rules. Report diagnostics. |
| Consumer | Load a checker-clean document and paint it. Ignore unknown extensions. |

A document is **runtime-loadable** only if it is schema-valid, satisfies every
semantic rule, and contains no diagnostic with `severity` `"error"`. Consumers
MUST refuse non-runtime-loadable documents.

The consumer supplies, at load/layout time, a runtime viewport rectangle and
MAY supply an active theme id and an active locale. Those three inputs are not
manifest fields.

## 2. Schema rules vs semantic rules

**Schema rules** are draft 2020-12 constraints in `opui-v2.schema.json`.
They are structural: required keys, enums, closed objects, typed lengths,
node payload unions, empty leaf children, regexes.

**Semantic rules** are checker-enforced and cannot be expressed (or cannot be
expressed completely) in draft 2020-12. They are marked **Semantic** below.

Semantic families:

- Forest: single parent, no cycles, children exist, entrypoints name roots;
  slot refs are concrete forest edges.
- References: node/asset/token/theme/locale/message/path/breakpoint/component
  /template/slot/variant ids resolve; kinds match the referencing field.
- Uniqueness: `runtime_id` unique when present, including expanded instance
  part ids.
- Key equality: map key equals the object's id field; asset id equals the
  constructed `<kind>-<sha256>` string.
- Package stem equals `document.name`.
- Token/theme type agreement and acyclic theme `extends`.
- Breakpoint bounds and pairwise non-overlap.
- Locale fallback acyclicity; message resolution.
- Component dependency DAG; template forests; slot cardinality; variant
  defaults.
- Rich-text emptiness; text-on-path compatibility.
- Asset bytes, MIME, URI, SVG subset.
- Diagnostic correlation for fallbacks (`node_id` + fallback `asset`).
- Encoding/determinism of the JSON bytes.

## 3. Package

The package is exactly two siblings:

```
<name>.opui
<name>.opui.assets/
  images/
  fonts/
  fallback/
  vectors/
```

`<name>` is `document.name`. The manifest file is UTF-8 JSON named
`<name>.opui`. Asset `uri` values resolve relative to the sibling asset
root `<name>.opui.assets/`, **not** the directory that contains the
manifest. Example: uri `images/<sha>.png` is the file
`<name>.opui.assets/images/<sha>.png`.

No `data:`, `http:`, `https:`, `file:`, protocol-relative, or absolute URIs.

**Semantic:** on-disk stem equals `document.name` (`opui.package_name`).

## 4. Encoding and determinism

These rules apply to the `<name>.opui` bytes. They are **Semantic** except
where a JSON parser rejects the input first.

1. UTF-8 with no BOM.
2. Duplicate object keys are invalid (`opui.duplicate_json_key`).
3. Object keys are sorted by UTF-8 code unit lexicographic order
   (`opui.key_order`).
4. Indentation is exactly two spaces. No tabs. (`opui.indent`)
5. Newlines are LF only. No CR.
6. The file ends with exactly one trailing LF.
7. Arrays are semantically ordered (see each array). Do not sort arrays by
   key unless an array-specific rule says so.
8. No timestamps. No random ids. `generator` has `name` and `version` only.
   `source_id` and `runtime_id` are exporter-stable. Asset ids are content
   hashes. Catalog ids (tokens, themes, breakpoints, paths, components,
   message keys) are exporter-stable semantic names.
9. Values are I-JSON (RFC 7493): finite numbers only; integers that are
   JSON numbers MUST be exactly representable in IEEE 754 binary64
   (`±(2^53-1)`). No `NaN`, `Infinity`, `-Infinity`.
10. Negative zero is invalid (`opui.negative_zero`). Write `0`.
11. Number tokens follow RFC 8785 *JSON Canonicalization Scheme* number
    serialization (no useless exponent, no trailing zeros, no leading zeros).
    Pretty-print remains OPUI-specific: two-space indent and the trailing LF.
    Do not emit RFC 8785 compact (one-line) form. (`opui.number_normalization`)
12. Asset files are the exact hashed bytes. Re-encoding that changes bytes
    changes the id.
13. Omitted `layout`/`style`/catalog fields are not inserted, and explicit
    default values are not stripped. Two documents that differ only by
    writing or omitting a default-valued field are both conforming and are
    not byte-identical.

**Exporter obligation:** produce bytes that already satisfy 1–13.
**Checker-observable:** all of 1–13 on the manifest; 12 also on sidecars.

## 5. Top-level document

All ten top-level keys are required. The object is closed. Exact keys:

`format`, `schema_version`, `generator`, `document`, `entrypoints`,
`assets`, `resources`, `nodes`, `diagnostics`, `extensions`.

There is no top-level `reference_viewport`.

```json
{
  "assets": {},
  "diagnostics": [],
  "document": {
    "name": "home",
    "reference_viewport": { "height": 900, "width": 1440 }
  },
  "entrypoints": { "default": "root" },
  "extensions": {},
  "format": "openpencil-runtime-ui",
  "generator": { "name": "openpencil", "version": "2.0.0" },
  "nodes": { "root": { "...": "..." } },
  "resources": {},
  "schema_version": 2
}
```

| Key | Meaning |
| --- | --- |
| `format` | Const `"openpencil-runtime-ui"`. |
| `schema_version` | Const integer `2`. |
| `generator` | Closed. Required `name`, `version` (non-empty strings). No timestamp. |
| `document` | Closed. Required `name` (package stem) and `reference_viewport`. |
| `entrypoints` | Map of name → `source_id`. `default` is required. |
| `assets` | Map of asset id → asset. Empty if unused. |
| `resources` | Closed catalog object. Empty `{}` is valid. Catalog properties optional. |
| `nodes` | Map of `source_id` → node. Non-empty. |
| `diagnostics` | Exporter-emitted diagnostics, semantically ordered. |
| `extensions` | Reverse-domain map. May be empty. |

`document.reference_viewport` is design-time width/height, both finite `> 0`.
Not a breakpoint. Not the root containing block.

`entrypoints` is an open name map (not a closed object). Values are `source_id`
strings.

**Semantic**

- `nodes` key equals `node.source_id` (`opui.node_key_mismatch`).
- `assets` key equals `<kind>-<sha256>` and `asset.sha256` (`opui.asset_key_mismatch`, `opui.asset_id_rule`).
- Every `entrypoints` value names an existing node (`opui.unknown_entrypoint`).
- Every entrypoint node is a forest root (`opui.entrypoint_not_root`).
- `runtime_id` values that are present are unique, including expanded
  instance part ids (`opui.duplicate_runtime_id`,
  `opui.duplicate_expanded_runtime_id`).

## 6. Resources

`resources` is a required closed object. All catalog properties are optional.
`{}` is valid. Present catalogs use the shapes below.

| Property | Shape |
| --- | --- |
| `breakpoints` | Map of breakpoint id → `{ min_width, max_width }`. |
| `tokens` | Map of token id → `{ kind, value }`. |
| `themes` | Closed `{ default, items }`. |
| `localization` | Closed `{ default_locale, locales }`. |
| `paths` | Map of path id → `{ commands, fill_rule }`. |
| `components` | Map of component id → definition. |

Catalog map keys use the **stable id** grammar
`^[A-Za-z][A-Za-z0-9]*([._-][A-Za-z0-9]+)*$` except template node ids,
which are opaque non-empty strings like `source_id`. Locale ids use the
BCP 47-ish grammar in §6.4.

Typed maps (not extension maps): every catalog map, `themes.items`,
`themes.items.*.values`, `localization.locales`,
`localization.locales.*.messages`, `components.*.nodes`,
`components.*.slots`, `components.*.variant_axes[].values`,
variant patch maps, `responsive`, instance `variants` / `slots` /
`overrides`.

### 6.1 Breakpoints

```json
{
  "sm": { "max_width": 600, "min_width": null },
  "md": { "max_width": null, "min_width": 600 }
}
```

Each entry is closed. `min_width` and `max_width` are required and each is
`null` or a finite number `≥ 0`. At least one bound MUST be non-null
(schema). Widths are CSS pixels of the runtime viewport.

A viewport width `W` **matches** breakpoint `b` iff

```
(b.min_width is null or W >= b.min_width)
and (b.max_width is null or W < b.max_width)
```

`min_width` is inclusive. `max_width` is exclusive.

**Semantic**

- If both bounds are non-null, `min_width < max_width`
  (`opui.breakpoint_bounds`).
- Declared breakpoints are pairwise non-overlapping when ordered by
  `min_width` (`null` sorts as −∞) (`opui.breakpoint_overlap`).
- Every `responsive` key names a declared breakpoint
  (`opui.unknown_breakpoint`).

After the non-overlap rule, at most one breakpoint matches any `W`. If none
match, no responsive override applies.

### 6.2 Tokens

```json
{
  "color.fg": {
    "kind": "color",
    "value": { "a": 1, "b": 0.1, "g": 0.1, "r": 0.1, "space": "srgb" }
  },
  "space.md": { "kind": "length", "value": { "type": "px", "value": 16 } }
}
```

Each token is a closed `{ kind, value }` union:

| `kind` | `value` |
| --- | --- |
| `color` | Color object (§9.1). |
| `number` | Finite number. |
| `length` | A length literal (`auto` / `px` / `percent` / `fill` / `fr` / `none` / `minmax`). |
| `string` | Non-empty string. |
| `font` | `font-<sha256>` asset id. |

Catalog `value` is always a literal of that kind, never a token ref.
Token refs appear only at typed leaves (§6.2.1).

#### 6.2.1 Token refs

Where this spec says a field is a **color**, **number**, **length**,
**string**, or **font** value, the wire form is either the literal or

```json
{ "token": "color.fg" }
```

`token` is a token id. The object is closed.

Practical tokenized fields:

- Color: every color (fills, borders, shadows, strokes, text `color`, tint).
- Number: `opacity`, `flex_grow`, `flex_shrink`, `z_index`, rotation
  `degrees`, scale `x`/`y`, skew `x`/`y`, transform-origin `x`/`y`, filter
  amounts, stroke `miter_limit` / `dash_offset` / dash items, font `weight`.
- Length: every Length field, including stroke `width` and text `size` /
  `letter_spacing`.
- String: text `family`.
- Font: text `font`.

Not tokenized: discriminators (`type`, `kind`, `display`, enums), grid
lines, rich-text structure, ids, booleans, `children`.

**Semantic**

- `token` names a declared token (`opui.unknown_token`).
- Resolved kind matches the field (`opui.token_kind_mismatch`).
- After resolution, Length still obeys the field's type row
  (`opui.length_context`). `z_index` / `weight` resolve to integers in
  range. `font` resolves to a `font` asset (`opui.unresolved_font`).

### 6.3 Themes

When `themes` is present it is closed and required:

```json
{
  "default": "light",
  "items": {
    "dark": {
      "extends": "light",
      "values": {
        "color.fg": { "a": 1, "b": 0.9, "g": 0.9, "r": 0.9, "space": "srgb" }
      }
    },
    "light": { "extends": null, "values": {} }
  }
}
```

| Field | Rule |
| --- | --- |
| `default` | Theme id or `null`. |
| `items` | Map of theme id → `{ extends, values }`. May be empty. |
| `extends` | Theme id or `null`. |
| `values` | Map of token id → untyped JSON. Schema does not type the JSON. |

**Semantic**

- Non-null `default` names an item (`opui.theme_default`).
- `extends` is `null` or names an item (`opui.unknown_theme`).
- `extends` graph is acyclic (`opui.theme_cycle`).
- Each `values` key names a declared token (`opui.unknown_token`).
- Each `values` entry is a literal of that token's `kind`
  (`opui.theme_value_kind`). Not a token ref.

Resolution of token `T` under active theme `A` (or `themes.default` if the
consumer did not select one, or catalog-only if that is `null` / omitted):

1. Start with `tokens.T.value`.
2. Walk the `extends` chain from the root (`extends: null`) to `A`.
3. Each theme's `values.T`, when present, replaces the running value.

If `themes` is omitted, every token ref uses the catalog `value`.

### 6.4 Localization

When `localization` is present it is closed and required:

```json
{
  "default_locale": "en",
  "locales": {
    "de": {
      "fallback": "en",
      "messages": { "cta.label": { "blocks": [{ "...": "..." }] } }
    },
    "en": { "fallback": null, "messages": { "cta.label": { "blocks": [] } } }
  }
}
```

Locale ids match `^[A-Za-z]{2,8}(-[A-Za-z0-9]{1,8})*$` (BCP 47-ish:
primary 2–8 ALPHA, then 1–8 alphanum subtags). Examples: `en`, `en-US`,
`zh-Hans-CN`.

Each locale is closed: `fallback` (`null` or locale id) and `messages`
(map of message key → rich text). Message keys use the stable-id grammar.

Rich text is `{ "blocks": [ ... ] }`. Blocks:

| `type` | Fields |
| --- | --- |
| `paragraph` | `spans` |
| `unordered_list` | `items` — each `{ "spans": [ ... ] }` |
| `ordered_list` | `start` (integer ≥ 1), `items` as above |

List items are span arrays only. Nested lists are out of scope.

Spans:

| `type` | Fields |
| --- | --- |
| `text` | `value` (string). Optional `style` (`textStyleOverrides`). |
| `line_break` | none |
| `inline_image` | `asset` (`image-`/`vector-`), `intrinsic_width`, `intrinsic_height` (≥ 1) |

**Semantic**

- `default_locale` names a locale (`opui.locale_default`).
- `fallback` is `null` or names a locale (`opui.unknown_locale`).
- Fallback graph is acyclic (`opui.locale_cycle`).
- `inline_image.asset` exists and is `image` or `vector`
  (`opui.unresolved_inline_image`, `opui.asset_kind_mismatch`).
- A `type: message` text source resolves via §10 (`opui.unknown_message`).

Empty `blocks: []` is schema-valid and paints nothing
(`opui.rich_text_empty` is not emitted for that alone).

### 6.5 Paths

```json
{
  "icon": {
    "commands": [
      { "type": "move", "x": 0.2, "y": 0.5 },
      { "type": "line", "x": 0.4, "y": 0.7 },
      { "type": "cubic", "c1x": 0.5, "c1y": 0.8, "c2x": 0.6, "c2y": 0.8, "x": 0.8, "y": 0.3 },
      { "type": "close" }
    ],
    "fill_rule": "nonzero"
  }
}
```

Closed `{ commands, fill_rule }`. `fill_rule` is `nonzero` or `evenodd`.
`commands` is a non-empty array of tagged commands. Coordinates are finite
and live in the referencing node's border-box normalized space (`0,0`
top-left, `1,1` bottom-right). Values outside `0..1` are allowed.

| `type` | Fields |
| --- | --- |
| `move` | `x` `y` |
| `line` | `x` `y` |
| `quadratic` | `cx` `cy` `x` `y` |
| `cubic` | `c1x` `c1y` `c2x` `c2y` `x` `y` |
| `close` | none |

No SVG path strings. No arc command. No execution.

**Semantic**

- First command is `move` (`opui.path_commands`).
- `close` / `line` / curve commands are not issued with no current point
  (`opui.path_commands`).
- Every path/mask/text-path/`vector` layer `path` names a declared path
  (`opui.unknown_path`).

### 6.6 Components

```json
{
  "Button": {
    "nodes": { "btn": { "...": "..." }, "label": { "...": "..." } },
    "root": "btn",
    "slots": { "icon": { "allows_multiple": false, "required": false } },
    "variant_axes": [
      {
        "default": "md",
        "name": "size",
        "values": {
          "md": {},
          "sm": { "label": { "style": { "opacity": 0.9 } } }
        }
      }
    ]
  }
}
```

Each definition is closed and required: `root`, `nodes`, `slots`,
`variant_axes`.

`nodes` is a non-empty map of `template_id` → template node. Template nodes
are a closed union structurally equivalent to runtime nodes, except:

- `template_id` replaces `source_id` (required). Map key equals
  `template_id` (**Semantic**, `opui.template_key_mismatch`).
- Optional `runtime_part_id` (`^[A-Za-z][A-Za-z0-9._-]*$`, no `/`).
- No `source_id`, no `runtime_id`, no v1 `component` metadata.
- `children` lists `template_id`s.
- Types: runtime types plus template-only `slot`.
- Nested `component_instance` is allowed.

`slot` payload is closed `{ "name": <slot name> }`. Slot nodes are leaves
(`children` `[]`).

`slots` maps slot name → closed `{ required, allows_multiple }` (booleans).
Every `slot` node's `name` MUST be a key of `slots` (**Semantic**,
`opui.unknown_slot`). Every `slots` key MUST be referenced by exactly one
slot node in that definition (**Semantic**, `opui.unknown_slot`).

`variant_axes` is an ordered array. Each axis is closed:
`name`, `default`, `values`. `values` maps variant name → patch map.
A patch map maps `template_id` → node patch.

A **node patch** is closed and sparse: optional `visible`, `layout`,
`style`, and the relevant type payload (`container` / `text` / `image` /
`vector` / `fallback` / `component_instance` / `slot`). Payload objects
in a patch are sparse (all internals optional; text `defaults` is
`textStyleOverrides`). Patches MUST NOT contain `type`, ids, `children`,
`name`, `responsive`, `masks`, or `extensions`.
The template-only `slot` patch is necessarily empty: a patch cannot rename a
declared slot. Nested component-instance slot fills name template nodes.

**Semantic**

- `root` names a node in `nodes` (`opui.component_root`).
- Template child edges form a forest rooted at `root`: existing children,
  no duplicates, no multiple parents, acyclic, no unreachable template
  nodes (`opui.template_unknown_child`, `opui.template_duplicate_child`,
  `opui.template_multiple_parents`, `opui.template_cycle`,
  `opui.template_unreachable`).
- Patch map keys name template nodes (`opui.unknown_template_node`).
- `default` is a key of `values` (`opui.variant_default`).
- Nested `component_instance.definition` names a declared component
  (`opui.unknown_component`). The directed graph of component → nested
  definition is acyclic (`opui.component_cycle`).

There are no OpenPencil editor internals in definitions or instances.

## 7. Nodes

Nodes are a map, not a tree-inlined array. Each runtime node is keyed by
`source_id`.

Required common fields: `type`, `source_id`, `visible`, `children`,
`layout`, `style`, `extensions`.

Optional: `runtime_id`, `name`, `responsive`, `masks`.

There is no v1 `component` metadata field.

`runtime_id` matches `^[A-Za-z][A-Za-z0-9._/-]*$`.

- `source_id` preserves the source node identity. Opaque to consumers.
  Used for provenance and graph references.
- `runtime_id`, when present, is semantic and stable across equivalent
  exports. It is the only node identifier applications bind behavior to.
- `name` is descriptive only. Applications MUST NOT treat it as a stable
  identifier or binding interface.

Union `type`: `container` | `text` | `image` | `vector` | `fallback` |
`component_instance`.

Exactly one type payload object is present, named like the type.
Irrelevant payloads MUST be absent (schema).

`children` is an array of `source_id` strings. Order is the parent's child
order (flex/grid order and default paint order).

Text, image, vector, fallback, and `component_instance` are leaves:
`children` MUST be `[]` (schema). Slotted nodes are not listed in
`component_instance.children`; they appear in the instance payload
(§14).

`responsive` is a map of breakpoint id → sparse `{ visible, layout, style }`.
Each present key replaces that common field after variant application
(§20). Omitted override keys keep the post-variant base. Omitted fields
inside `layout` / `style` keep the base, then remaining omissions use v2
defaults. Present nested objects (`padding`, `borders`, …) still require
their own keys and replace the whole nested object.

`masks` is described in §15. Omitted means `[]`.

### 7.1 Forest (**Semantic**)

- Every `children` entry names an existing node (`opui.unknown_child`).
- Every `component_instance.slots[*][*]` entry names an existing node
  (`opui.unknown_child`). Those slot refs are concrete forest edges: the
  instance is the unique parent of each listed node.
- A node appears in at most one parent's `children` or as a slot target
  (`opui.multiple_parents`). A node MUST NOT appear both in some
  `children` array and as a slot target.
- A node appears at most once in one `children` array or one slot array
  (`opui.duplicate_child`).
- The directed graph of child edges plus slot edges is acyclic
  (`opui.cycle`).
- Roots are nodes that appear in no `children` array and no slot array.
- Nodes not reachable from any entrypoint (walking children and slot
  edges) are invalid (`opui.unreachable_node`).

This is a forest. Multiple roots are allowed. Runtime starts at
`entrypoints.default`.

## 8. Layout

v2 is fluid. Breakpoints live in `resources.breakpoints` and per-node
`responsive`. Design size lives on `document.reference_viewport`.

The **root containing block** is the **runtime viewport** supplied by the
consumer at layout time. `document.reference_viewport` is not that box.
Percentages resolve against the used containing block.

### 8.1 `display` vs `position`

These are independent.

| Field | Values | Meaning |
| --- | --- | --- |
| `display` | `flex` `grid` `absolute` `none` | How this node lays out **its children**. |
| `position` | `flow` `absolute` | How this node participates in **its parent**. |

- `display: flex` — in-flow children are flex items (CSS Flexbox 1 subset).
- `display: grid` — in-flow children are grid items (CSS Grid 1 subset).
- `display: absolute` — every child is placed by that child's `inset`
  against this node's padding box. Child `position` does not change that
  placement. Children do not form a flex or grid formatting context.
- `display: none` — this node generates no principal box. It does not
  participate in parent layout. Its descendants are not laid out or painted.

- `position: flow` — the node is in the parent's formatting context when
  the parent `display` is `flex` or `grid`. Under a parent with
  `display: absolute`, the node is still placed by `inset` (parent wins).
- `position: absolute` — the node is out of the parent's flex/grid flow
  and is placed by `inset` against its containing block.

`inset` is ignored for in-flow flex/grid items.

For an expanded `component_instance`, the instance node's `display` /
`position` / other layout fields override the expanded template root
(§14). Slotted nodes are laid out as children of the expanded slot node.

### 8.2 Visibility vs `display: none`

`visible` is a stored boolean. It is not CSS `visibility` by itself.
Responsive may replace it.

**Computed visible** (paint and text/image/vector drawing):

```
computed_visible(n) =
    n.visible
    and n.layout.display != none
    and (n is a root or computed_visible(parent(n)))
```

| | `visible: true` | `visible: false` |
| --- | --- | --- |
| `display` ≠ `none` | Laid out and painted if ancestors allow. | Laid out; not painted; descendants not painted. Still occupies space. |
| `display: none` | No box. Not painted. Descendants not laid out. | Same. |

A node that is not computed-visible is not painted. `opacity: 0` still
lays out and still counts as computed-visible.

### 8.3 Containing blocks

- Root containing block: the runtime viewport rectangle.
- For an in-flow child: the parent's content box.
- For a child placed by inset (`position: absolute`, or any child of
  `display: absolute`): the padding box of the nearest ancestor that has
  `position: absolute` or `display: absolute`. If none, the root containing
  block.

A `display: none` node is never a containing block.

### 8.4 Lengths

Every Length discriminator is exactly `type` (not `unit`). A token ref may
replace the whole Length object.

| Type | Shape |
| --- | --- |
| `auto` | `{ "type": "auto" }` |
| `px` | `{ "type": "px", "value": <finite number> }` |
| `percent` | `{ "type": "percent", "value": <finite number> }` |
| `fill` | `{ "type": "fill", "weight": <number > 0> }` |
| `fr` | `{ "type": "fr", "value": <number ≥ 0> }` |
| `none` | `{ "type": "none" }` |
| `minmax` | `{ "type": "minmax", "min": <track>, "max": <track> }` |

Percentages: `100` means 100% of the relevant axis of the containing block.

`fill` is a flex-grow-like share of leftover space on that axis. `weight`
is the share. Only legal on `width`, `height`, and `flex_basis`.

| Field | Types |
| --- | --- |
| `width` `height` `flex_basis` | `auto` `px` `percent` `fill` |
| `min_width` `min_height` | `auto` `px` `percent` |
| `max_width` `max_height` | `px` `percent` `none` |
| `padding.*` `gap.*` `corner_radius.*` | `px` `percent`, value ≥ 0 |
| `margin.*` `inset.*` | `auto` `px` `percent` |
| `borders.*.width` `letter_spacing` `size` (font) `shadows.*.offset_*` `spread` `translation.*` `stroke.width` | `px` |
| `shadows.*.blur` | `px` ≥ 0 |
| grid track | `px` `percent` `fr` `auto` `minmax` |

`min_width` / `min_height` with `auto` means `0` in this subset (not the CSS
flex automatic minimum size).

A Length `type` outside the field's row is `opui.length_context` after
token resolution. The published schema already splits $defs per field.

### 8.5 Layout object

Closed. Internal properties are optional. `{}` is valid. Consumers apply
the omission defaults below before layout. Present nested objects
(`padding`, `margin`, `inset`, `gap`, placements, `scrollbar`) still
require their own keys. Inapplicable flex/grid fields, when omitted, take
these defaults and are ignored when `display` does not use them.

There is no v1 `overflow` field.

Exporters SHOULD omit a field when its value equals the default, to keep
packages small. Explicit default values are conforming. Byte
canonicalization does not drop or insert these fields
(`opui.key_order` still applies to keys that are present).

| Property | Omitted default |
| --- | --- |
| `display` | `absolute` (free-position child mode) |
| `position` | `flow` |
| `overflow_x` `overflow_y` | `visible` |
| `scrollbar` | `{ "visibility": "auto", "overscroll": "propagate" }` |
| `width` `height` `min_width` `min_height` | `{ "type": "auto" }` |
| `max_width` `max_height` | `{ "type": "none" }` |
| `padding` | all edges `{ "type": "px", "value": 0 }` |
| `margin` | all edges `{ "type": "px", "value": 0 }` (zero, not `auto`) |
| `inset` | all edges `{ "type": "auto" }` |
| `flex_direction` | `row` |
| `flex_wrap` | `nowrap` |
| `flex_grow` | `0` |
| `flex_shrink` | `1` |
| `flex_basis` | `{ "type": "auto" }` |
| `justify_content` | `start` |
| `justify_items` | `stretch` |
| `justify_self` | `auto` |
| `align_content` | `start` |
| `align_items` | `stretch` |
| `align_self` | `auto` |
| `gap` | row and column `{ "type": "px", "value": 0 }` |
| `grid_template_columns` `grid_template_rows` | `[]` |
| `grid_auto_flow` | `row` |
| `grid_column` `grid_row` | `{ "start": { "kind": "auto" }, "end": { "kind": "auto" } }` |
| `aspect_ratio` | `{ "type": "auto" }` |
| `z_index` | `0` |

`overflow_x` / `overflow_y`: `visible` | `hidden` | `scroll` | `auto`.
Independent axes.

- `visible` — no clip, not a scroll container on that axis.
- `hidden` — clip paint and layout overflow to the padding box on that axis.
- `scroll` — clip and create a scroll container on that axis. Initial
  scroll offset is `0`. Not serialized.
- `auto` — as `scroll` when overflow exists, otherwise as `hidden` for
  clipping purposes (no gutter requirement). Initial offset `0`. Not
  serialized.

`scrollbar` is closed. Present object still requires both keys:

| Field | Values | Default if `scrollbar` omitted |
| --- | --- | --- |
| `visibility` | `auto` `always` `hidden` | `auto` |
| `overscroll` | `propagate` `contain` | `propagate` |

`visibility: hidden` hides the scrollbar but does not disable scrolling.
`overscroll: contain` stops scroll chaining; `propagate` allows it.
`scrollbar` is ignored when both overflow axes are `visible`.

The document MUST NOT contain scroll offsets, scroll snapshots, or scroll
event bindings.

Grid lines: `auto`, `line` with nonzero integer (CSS 1-based; negative from
the end), or `span` with integer ≥ 1.

`aspect_ratio`: `{ "type": "auto" }` or `{ "type": "ratio", "value": > 0 }`
(width / height). This is not a Length.

### 8.6 Flex and grid subset

Unless this section says otherwise, used values follow CSS Flexible Box
Layout Module Level 1 and CSS Grid Layout Module Level 1.

Present:

- Flex: `flex_direction`, `flex_wrap`, `flex_grow`, `flex_shrink`,
  `flex_basis`, `justify_content`, `align_items`, `align_self`,
  `align_content`, `gap`.
- Grid: `grid_template_columns`, `grid_template_rows`, `grid_auto_flow`
  (`row` `column` `row_dense` `column_dense`), `grid_column`, `grid_row`,
  `justify_content`, `justify_items`, `justify_self`, `align_content`,
  `align_items`, `align_self`, `gap`.
- Implicit grid tracks created by auto-placement are `auto` sized.

`width`/`height`/`flex_basis` `{ "type": "fill", "weight": w }` is extra
to CSS: the node takes a `w`-weighted share of leftover space on that axis
after definite sizes. It does not replace `flex_grow` when both are set;
`fill` on the size axis is the used size contribution, and `flex_grow`
applies only when the size is `auto`.

Absent (not in the format): `order`, named lines, `subgrid`, masonry,
`flex-flow` / other shorthands, margin collapsing, writing-mode other than
horizontal-tb / ltr, serialized scroll position.

Alignment keywords are the CSS values with hyphens turned into underscores
(`space_between`). There is no `flex-start`; use `start`.

Child order is `children` array order, then slot-array order for expanded
slot children. There is no `order` property.

### 8.7 Paint order

For siblings that paint:

1. Sort by `z_index` ascending (more negative first).
2. Break ties by `children` index (earlier first). Expanded slot children
   use slot-array index inside that slot's parent.

`display: none` and not-computed-visible nodes and their descendant boxes
do not paint.

Per node, back to front:

1. Outer shadows, array order (first behind).
2. `fills`, array order (first behind).
3. Inner shadows, array order, clipped to the padding box.
4. Borders.
5. In-node content (text, image, vector layers, fallback) then descendant
   boxes.
6. Filters in array order, then `opacity`, then `blend_mode` into the
   parent.

Transforms apply after layout about `transform_origin`, in the fixed order
translation, then rotation, then scale, then skew.

## 9. Style

Closed. Internal properties are optional. `{}` is valid. Consumers apply
omission defaults before paint. Present nested objects (`borders` sides,
`corner_radius`, shadow items, `translation`, `rotation`, `scale`,
`skew`, `transform_origin`) still require their own keys.

There are no v1 fields `fill`, `border`, or `outer_shadows`.

Exporters SHOULD omit a field when its value equals the default. Explicit
defaults are conforming. Canonicalization does not drop or insert them.

| Property | Omitted default |
| --- | --- |
| `opacity` | `1` |
| `clipping` | `false` |
| `fills` | `[]` (no fill) |
| `borders` | no sides (equivalent to all `style` `none`) |
| `corner_radius` | all corners `{ "type": "px", "value": 0 }` |
| `shadows` | `[]` |
| `translation` | `{ "x": { "type": "px", "value": 0 }, "y": { "type": "px", "value": 0 } }` |
| `rotation` | `{ "degrees": 0 }` |
| `scale` | `{ "x": 1, "y": 1 }` |
| `skew` | `{ "x": 0, "y": 0 }` (degrees) |
| `transform_origin` | `{ "x": 0.5, "y": 0.5 }` (center) |
| `filters` | `[]` |
| `blend_mode` | `normal` |

`clipping: true` clips this node's painting and descendants to the rounded
border box. Combined with non-`visible` overflow, the intersection clips.

### 9.1 Color

Unpremultiplied RGBA. Channels are numbers in `0..1`. Color space is
`srgb` or `srgb-linear`. Or a color token ref.

```json
{ "a": 1, "b": 0.2, "g": 0.4, "r": 0.8, "space": "srgb" }
```

### 9.2 Fills

`fills` is an ordered array. Empty means none. There is no `type: none`
item. Item types: `solid`, `linear`, `radial`. First item is behind.

Linear and radial geometry is complete and normalized against the node's
border box: `x`/`y` `0` is the top-left, `1` is the bottom-right. Values
outside `0..1` are allowed (gradient continues).

- Linear: `start`, `end` points, `stops`.
- Radial: `center`, `radius` as an axis-aligned ellipse. `radius.x` and
  `radius.y` are radii in the same normalized space and MUST be `> 0`.

Stops: at least two. Each has `offset` in `0..1` and a color (or token).
**Semantic:** the checker enforces nondecreasing `offset` values
(`opui.gradient_stop_order`). Equal offsets are allowed (hard edge: later
stop wins). Decreasing offsets are invalid.

### 9.3 Borders, corner radius, shadows, transforms, filters

`borders` is closed with optional sides `top` `right` `bottom` `left`.
An omitted side has no border. A present side is a complete border:
`style` `none` or `solid`, `width` `{ "type": "px", "value": ≥ 0 }` (or
token), `color` (or token).

`corner_radius` is per corner, `px` or `percent` ≥ 0. Percent is relative
to the corresponding border-box axis as in CSS.

`shadows` is an ordered array (empty if none). Each item:

```json
{
  "blur": { "type": "px", "value": 0 },
  "color": { "a": 1, "b": 0, "g": 0, "r": 0, "space": "srgb" },
  "kind": "outer",
  "offset_x": { "type": "px", "value": 0 },
  "offset_y": { "type": "px", "value": 0 },
  "spread": { "type": "px", "value": 0 }
}
```

`kind` is `outer` or `inner`. Outer paints behind the border box. Inner
paints above fills, clipped to the padding box. Array order is
back-to-front within each kind's paint step.

Transforms:

| Field | Shape |
| --- | --- |
| `translation` | `{ "x": <px Length>, "y": <px Length> }` |
| `rotation` | `{ "degrees": <finite number> }` clockwise |
| `scale` | `{ "x": <finite number>, "y": <finite number> }` |
| `skew` | `{ "x": <finite number>, "y": <finite number> }` degrees |
| `transform_origin` | `{ "x": 0..1, "y": 0..1 }` in border-box fraction; `(0.5, 0.5)` is center |

`filters` is an ordered array. Each item is one of:

| `type` | Fields | Range |
| --- | --- | --- |
| `blur` | `value` | ≥ 0 (CSS px) |
| `brightness` | `value` | ≥ 0 (`1` = unchanged) |
| `contrast` | `value` | ≥ 0 (`1` = unchanged) |
| `saturate` | `value` | ≥ 0 (`1` = unchanged) |
| `hue_rotate` | `degrees` | finite |
| `opacity` | `value` | `0..1` |

Amounts may be number tokens. Applied in array order to the node's
flattened rendering (this node plus descendants) after shadows, fills,
borders, and content.

`blend_mode` is a CSS Compositing Level 1 mix-blend-mode keyword with
hyphens turned into underscores:

`normal` `multiply` `screen` `overlay` `darken` `lighten` `color_dodge`
`color_burn` `hard_light` `soft_light` `difference` `exclusion` `hue`
`saturation` `color` `luminosity`.

No `plus_darker` / `plus_lighter`.

## 10. Text

```json
{
  "align_horizontal": "start",
  "align_vertical": "start",
  "defaults": {
    "color": { "token": "color.fg" },
    "family": { "token": "font.ui" },
    "font_style": "normal",
    "letter_spacing": { "type": "px", "value": 0 },
    "line_height": { "type": "normal" },
    "size": { "type": "px", "value": 16 },
    "weight": 400
  },
  "overflow": "clip",
  "source": {
    "type": "literal",
    "value": {
      "blocks": [
        {
          "spans": [{ "type": "text", "value": "Hello" }],
          "type": "paragraph"
        }
      ]
    }
  },
  "wrap": "wrap"
}
```

Required: `source`, `defaults`, `align_horizontal`, `align_vertical`,
`wrap`, `overflow`. Optional: `text_path`.

There is no v1 `content` / `runs` pair and no `legacy` source. v1 runs
map to span `style` overrides (§23).

`source` is exactly one of:

| `type` | Fields |
| --- | --- |
| `literal` | `value` — rich text |
| `message` | `key` — message key |

`defaults` is a complete text style. `font` is optional; when present it
is a `font-<sha256>` asset id or a font token. `family` / `color` /
`size` / `letter_spacing` / `weight` accept the matching token kinds.

`line_height`: `{ "type": "normal" }` | `{ "type": "multiple", "value": > 0 }`
| `{ "type": "px", "value": ≥ 0 }`. Not a layout Length. Not tokenized.

`align_horizontal`: `start` `center` `end` `justify`.
`align_vertical`: `start` `center` `end`.
`wrap`: `wrap` `nowrap`.
`overflow`: `clip` `ellipsis`.

`style.fills` paint the text node's background. Glyph color comes from
text style `color`. Opacity and clipping are only on common `style`.

### 10.1 Message resolution

Active locale `L` is the consumer locale if it names a declared locale,
else `localization.default_locale`. If `localization` is omitted, a
`type: message` source is `opui.unknown_message`.

Lookup `key` in `L.messages`. If missing, follow `fallback` until found
or the chain ends (`opui.unknown_message`).

### 10.2 Text on path

```json
{
  "alignment": "start",
  "direction": "forward",
  "path": "icon",
  "start_offset": 0
}
```

Closed. Required when present: `path`, `start_offset`, `alignment`
(`start` `center` `end`), `direction` (`forward` `reverse`).
`start_offset` is a finite number: fraction of path length (`0` = start,
`1` = end). Values outside `0..1` are allowed.

**Semantic** (`opui.text_path_incompatible` unless noted)

- `path` is declared (`opui.text_path_unknown` / `opui.unknown_path`).
- After message resolution, rich text is exactly one `paragraph` block.
- That paragraph's spans are only `text` and `line_break` (no lists, no
  `inline_image`).

## 11. Image

```json
{
  "asset": "image-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "fit": "contain",
  "intrinsic_height": 32,
  "intrinsic_width": 32,
  "nine_slice": null,
  "tint": null
}
```

`asset` is `image-<sha256>` or `vector-<sha256>`.
`intrinsic_width` / `intrinsic_height` are integers ≥ 1 (CSS pixels of the
source).

`fit`: `contain` `cover` `fill` `none` (CSS `object-fit`).

`nine_slice` is `null` or `{ "top", "right", "bottom", "left" }` integers ≥ 0.
**Semantic:** `top + bottom < intrinsic_height` and
`left + right < intrinsic_width` (`opui.nine_slice_bounds`).

`tint` is `null` or a color (or token). When set, each unpremultiplied
source channel is multiplied by the tint channel (including alpha).

Opacity and clipping are only on common `style`.

**Semantic:** `asset` exists and `kind` is `image` or `vector`
(`opui.unresolved_image`, `opui.asset_kind_mismatch`).

## 12. Vector

```json
{
  "layers": [
    {
      "fills": [{ "color": { "token": "color.fg" }, "type": "solid" }],
      "path": "icon",
      "strokes": [
        {
          "cap": "butt",
          "color": { "a": 1, "b": 0, "g": 0, "r": 0, "space": "srgb" },
          "dash": [],
          "dash_offset": 0,
          "join": "miter",
          "miter_limit": 4,
          "width": { "type": "px", "value": 1 }
        }
      ]
    }
  ]
}
```

Required `layers` (may be empty). Each layer is closed and required:
`path`, `fills`, `strokes`. `fills` uses the style fill-item union (no
`none`; empty means unfilled). `path` names a `resources.paths` entry.

Each stroke is closed and required when present:

| Field | Rule | Typical default if an exporter omits the whole stroke |
| --- | --- | --- |
| `color` | Color or token | opaque black |
| `width` | px Length ≥ 0 or token | `1` |
| `cap` | `butt` `round` `square` | `butt` |
| `join` | `miter` `round` `bevel` | `miter` |
| `miter_limit` | number ≥ 1 or token | `4` |
| `dash` | array of ≥ 0 numbers (or number tokens) | `[]` (solid) |
| `dash_offset` | finite number or token | `0` |

Layer order is back to front. Within a layer, fills then strokes.

No arbitrary SVG execution. `vector` assets remain the v1 static SVG
subset (§16.2) and are referenced only from `image` / mask `asset` /
`inline_image`, not from this payload.

## 13. Fallback

```json
{
  "asset": "raster_fallback-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "intrinsic_height": 64,
  "intrinsic_width": 128
}
```

A fallback is a leaf that paints one `raster_fallback` PNG stored under
the `fallback/` asset directory. Children are empty (schema).

**Semantic**

- `asset` exists and `kind` is `raster_fallback`.
- Every fallback node is pointed to by exactly one diagnostic with
  `code` `"opui.rasterization"`, `strategy` `"raster_fallback"`, and
  `node_id` equal to that node's `source_id` (`opui.fallback_diagnostic_count`).
- The raster file is the fallback node's `asset` (no asset list on the
  diagnostic).
- `runtime_id` on that diagnostic is the node's `runtime_id` if present,
  else `null` (`opui.diagnostic_runtime_id`).

## 14. Component instances

Runtime node `type: component_instance`. Payload is closed and required:

```json
{
  "definition": "Button",
  "overrides": {
    "label": { "visible": true }
  },
  "slots": { "icon": ["ctaIcon"] },
  "variants": { "size": "sm" }
}
```

| Field | Rule |
| --- | --- |
| `definition` | Component id. |
| `variants` | Map of axis name → variant name. Missing axes use the axis `default`. |
| `slots` | Map of slot name → array of concrete `source_id`s. |
| `overrides` | Map of `template_id` → node patch (§6.6). |

`children` on the instance node is `[]`. Slot arrays are the concrete
forest edges (§7.1).

The instance node's `layout` and `style` (after responsive merge) override
the expanded template root's `layout` and `style` (instance wins per
present key, then defaults). Instance `visible` / `masks` / `responsive`
apply to the instance box, which is the expanded root box.

### 14.1 Expansion (**Semantic**)

Deterministic expansion (§20):

1. Deep-instantiate `definition.nodes` (copy). Nested instances expand
   recursively. Cycles are `opui.component_cycle`.
2. Apply `variant_axes` in array order. For each axis, the selected
   name is `variants[axis.name]` if present, else `axis.default`.
   Unknown axis (`opui.unknown_variant_axis`) or unknown value
   (`opui.unknown_variant_value`) is an error. Each selected patch map
   is applied in unspecified key order; patches commute because they
   replace whole listed fields.
3. Apply `overrides` the same way.
4. Bind slots: each slot name in `definition.slots` looks up
   `payload.slots[name]` (missing → `[]`). `required` and length 0 is
   `opui.slot_required`. `allows_multiple: false` and length > 1 is
   `opui.slot_multiple`. Unknown slot name is `opui.unknown_slot`.
   Bound nodes become children of the unique slot template node, in
   array order.
5. Assign expanded `runtime_id`s. If the instance has a `runtime_id` and
   a template node has `runtime_part_id` `P`, the expanded id is
   `<instance runtime_id>/<P>`. Nested instances append:
   `<outer>/<inner_part>/<P>`. If the instance has no `runtime_id`,
   expanded parts have no `runtime_id`. The constructed string MUST
   match the `runtime_id` grammar (`opui.expanded_runtime_id`) and MUST
   be unique across the document (`opui.duplicate_expanded_runtime_id`).

Unknown `definition` is `opui.unknown_component`.

## 15. Masks

Optional common array on runtime and template nodes. Omitted means `[]`.
Each entry is closed and sources **exactly one** of `path` (path id) or
`asset` (`image-` / `vector-`).

```json
{
  "asset": null,
  "invert": false,
  "mode": "alpha",
  "operation": "intersect",
  "path": "icon"
}
```

Schema: one of `path` / `asset` is present and the other is absent (not
`null`). The example above is illustrative; do not write `"asset": null`.

| Field | Values |
| --- | --- |
| `mode` | `alpha` `luminance` |
| `operation` | `intersect` `add` `subtract` |
| `invert` | boolean |

Array order is application order. First mask is applied to the node's
alpha (and descendants). `intersect` is destination-in, `add` is
destination-over, `subtract` is destination-out, then invert if set.

**Semantic**

- `path` is declared (`opui.unknown_mask_path` / `opui.unknown_path`).
- `asset` exists and is `image` or `vector` (`opui.mask_asset_kind`).
- `vector` assets still obey §16.2. No SVG execution as a mask program
  (`opui.mask_source` for any other construct).

## 16. Assets

```json
{
  "byte_length": 123,
  "kind": "image",
  "mime_type": "image/png",
  "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "uri": "images/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png"
}
```

| Field | Rule |
| --- | --- |
| `kind` | `image` `font` `vector` `raster_fallback` |
| `uri` | `<dir>/<sha256>.<ext>` relative to `<name>.opui.assets/` |
| `mime_type` | closed enum below |
| `sha256` | 64 lowercase hex chars |
| `byte_length` | integer ≥ 0, I-JSON range |

### 16.1 Deterministic id and URI

```
id  = <kind> + "-" + sha256
uri = <dir> + "/" + sha256 + "." + ext
```

| kind | dir | allowed ext | mime |
| --- | --- | --- | --- |
| `image` | `images` | `png` | `image/png` |
| `image` | `images` | `jpg` | `image/jpeg` |
| `image` | `images` | `webp` | `image/webp` |
| `font` | `fonts` | `ttf` | `font/ttf` |
| `font` | `fonts` | `otf` | `font/otf` |
| `font` | `fonts` | `woff2` | `font/woff2` |
| `vector` | `vectors` | `svg` | `image/svg+xml` |
| `raster_fallback` | `fallback` | `png` | `image/png` |

JPEG extension is `jpg`, not `jpeg`. Directory for rasters of unsupported
nodes is `fallback`, not `raster_fallbacks`.

Kind, directory, extension, and MIME must match one row. Cross-kind
combinations are schema-invalid (`oneOf` rows). Hash equality of sidecar
bytes to `sha256` remains **Semantic** (`opui.asset_hash_mismatch`), as do
missing files (`opui.asset_missing`) and `byte_length`
(`opui.asset_byte_length`). `opui.asset_uri_rule` and
`opui.asset_mime_mapping` remain for checker reports that restate a
schema failure.

No base64 payloads. No network fetch by exporter, checker, or consumer
(`opui.network_uri`, `opui.embedded_data`).

### 16.2 Static SVG security subset

Applies to every `vector` asset. Consumer network and script execution are
disabled. The file MUST be a sanitized static SVG.

Malformed XML or non-UTF-8 bytes are `opui.svg_invalid` (**Semantic**).

Forbidden (**Semantic**, one code each):

- Script: `script` elements, `javascript:` URLs (`opui.svg_script`).
- Animation: SMIL, CSS animation/transition, `animate*` elements
  (`opui.svg_animation`).
- Event attributes: `on*` (`opui.svg_event`).
- `foreignObject` (`opui.svg_foreign_object`).
- DTD, entity declarations, entity references other than the five XML
  predefined ones (`opui.svg_dtd`).
- CSS: `<style>`, `style=""` presentation attributes that embed CSS,
  `@import`, `<?xml-stylesheet` (`opui.svg_css`). Presentation attributes
  that are not CSS stylesheets (`fill`, `stroke`, `transform`, …) are
  allowed.
- `href` / `xlink:href` / `url(...)` that are not same-document fragments
  (`#id`) (`opui.svg_external_ref`).

**Exporter obligation:** emit only the subset. Prefer dropping the
construct or rasterizing rather than shipping a forbidden SVG.

Native vector drawing uses `resources.paths` + `type: vector` (§12), not
SVG execution.

## 17. Unsupported-feature strategy

When the source cannot be represented as a standard `container` / `text` /
`image` / `vector` / `component_instance` node, the diagnostic `strategy`
field records the choice.

| `strategy` | Node | Diagnostic | Loadable |
| --- | --- | --- | --- |
| `native` | Existing standard node that approximates the source | `node_id` is that standard node's `source_id`. Typical codes: `opui.unsupported_native`, `opui.mask_unsupported`, `opui.effect_unsupported`, `opui.path_unsupported` | yes if severity is not `error` |
| `raster_fallback` | `type: fallback` plus raster asset | Exactly one `opui.rasterization` diagnostic per fallback: `node_id` is the fallback `source_id`, `strategy` is `raster_fallback`. The raster is that node's `fallback.asset`. `opui.unsupported_raster_fallback` is optional source detail, not a second required diagnostic | yes if severity is not `error` |
| `error` | optional | Unsupported-feature refusal. Requires `severity` `"error"` (schema). Typical code `opui.unsupported_error` | no |
| `none` | n/a | Ordinary validator/exporter notes, not an unsupported-feature choice. May use `severity` `"error"` | no if severity is `error` |

v2 represents paths, masks, inner shadows, multiple fills, per-side
borders, filters, blend modes, skew, text-on-path, rich text, and
component graphs natively. `opui.path_unsupported` / `opui.mask_unsupported`
/ `opui.effect_unsupported` remain for leftover source constructs (live
path effects, animated masks, unsupported filter types).

Correlation:

- Native diagnostics point at a standard existing node via `node_id`,
  never at a fallback.
- Raster fallback is recorded by the required `opui.rasterization`
  diagnostic (`node_id` = fallback `source_id`). The asset is read from
  that node. `opui.unsupported_raster_fallback` may add source-specific
  detail; it is not mandatory.
- Every fallback has exactly one `opui.rasterization` diagnostic.
- `strategy: "error"` requires `severity: "error"`. The converse is false:
  ordinary errors may use `strategy: "none"` with `severity: "error"`.
- Any diagnostic with `severity: "error"` makes the manifest
  non-runtime-loadable, regardless of `strategy`.

If `node_id` is non-null it MUST name an existing node
(`opui.diagnostic_node`). If both `runtime_id` and `node_id` are
non-null they MUST match that node (`opui.diagnostic_runtime_id`), using
the expanded id when the node is an expanded part that was also written
as a concrete node. Diagnostics do not point at template ids.

## 18. Diagnostics

Closed object. Exact required fields, same as v1:

`code`, `severity`, `node_id`, `runtime_id`, `message`, `strategy`, `details`.

There are no `nodes`, `assets`, or `path` arrays.

```json
{
  "code": "opui.rasterization",
  "details": {},
  "message": "rasterized unsupported vector effect",
  "node_id": "fb1",
  "runtime_id": null,
  "severity": "warning",
  "strategy": "raster_fallback"
}
```

| Field | Rule |
| --- | --- |
| `code` | One of the frozen `opui.*` codes in the schema enum. |
| `severity` | `error` `warning` `info` |
| `node_id` | `source_id` or `null`. |
| `runtime_id` | `runtime_id` or `null`. |
| `message` | Non-empty string. Not a random id. |
| `strategy` | `native` `raster_fallback` `error` `none` |
| `details` | Reverse-domain map. Ignored by required rendering. |

### 18.1 Ordering (**Semantic**)

Sort `diagnostics` by:

1. `severity`: `error`, then `warning`, then `info`
2. `code` (UTF-8 lexicographic)
3. `node_id` (`null` sorts as `""`)
4. `runtime_id` (`null` sorts as `""`)
5. `strategy`
6. `message`

Checker-generated output may include an internal JSON Pointer `location`
that is not a manifest field and MUST NOT be stored in `diagnostics[]`.
That output is not the seven-field wire object. Manifest diagnostics stay
the seven required fields. When the checker prints stored diagnostics it
uses this sort; `location` is not a sort key.

### 18.2 Codes

v1 codes are retained. v2 adds the codes in the last block.

Encoding / schema: `opui.schema`, `opui.encoding`, `opui.duplicate_json_key`,
`opui.key_order`, `opui.indent`, `opui.non_finite`, `opui.negative_zero`,
`opui.number_normalization`.

Identity / forest: `opui.duplicate_runtime_id`, `opui.node_key_mismatch`,
`opui.asset_key_mismatch`, `opui.unknown_child`, `opui.duplicate_child`,
`opui.multiple_parents`, `opui.cycle`, `opui.unreachable_node`,
`opui.unknown_entrypoint`, `opui.entrypoint_not_root`, `opui.package_name`.

Refs / assets: `opui.unresolved_asset`, `opui.unresolved_font`,
`opui.unresolved_image`, `opui.asset_kind_mismatch`, `opui.asset_id_rule`,
`opui.asset_uri_rule`, `opui.asset_mime_mapping`, `opui.asset_hash_mismatch`,
`opui.asset_byte_length`, `opui.asset_missing`, `opui.network_uri`,
`opui.embedded_data`.

SVG: `opui.svg_invalid`, `opui.svg_script`, `opui.svg_animation`,
`opui.svg_event`, `opui.svg_foreign_object`, `opui.svg_dtd`,
`opui.svg_css`, `opui.svg_external_ref`.

Text / layout / style: `opui.text_run_range`, `opui.text_run_overlap`,
`opui.text_run_empty`, `opui.text_run_order`, `opui.text_run_scalar`,
`opui.font_unavailable`, `opui.length_context`, `opui.nine_slice_bounds`,
`opui.gradient_stop_order`.

Unsupported / fallback: `opui.unsupported_native`,
`opui.unsupported_raster_fallback`, `opui.unsupported_error`,
`opui.mask_unsupported`, `opui.effect_unsupported`, `opui.path_unsupported`,
`opui.rasterization`, `opui.fallback_diagnostic_count`,
`opui.diagnostic_node`, `opui.diagnostic_runtime_id`,
`opui.diagnostic_order`, `opui.diagnostic_strategy`.

v2 catalogs / expansion / rich text: `opui.unknown_breakpoint`,
`opui.breakpoint_bounds`, `opui.breakpoint_overlap`,
`opui.unknown_token`, `opui.token_kind_mismatch`, `opui.token_cycle`,
`opui.unknown_theme`, `opui.theme_cycle`, `opui.theme_value_kind`,
`opui.theme_default`, `opui.unknown_locale`, `opui.locale_cycle`,
`opui.unknown_message`, `opui.locale_default`, `opui.unknown_path`,
`opui.path_commands`, `opui.unknown_mask_path`, `opui.mask_source`,
`opui.mask_asset_kind`, `opui.unknown_component`, `opui.component_cycle`,
`opui.unknown_template_node`, `opui.template_key_mismatch`,
`opui.component_root`, `opui.unknown_slot`, `opui.slot_required`,
`opui.slot_multiple`, `opui.unknown_variant_axis`,
`opui.unknown_variant_value`, `opui.variant_default`,
`opui.expanded_runtime_id`, `opui.duplicate_expanded_runtime_id`,
`opui.template_unreachable`, `opui.template_cycle`,
`opui.template_unknown_child`, `opui.template_duplicate_child`,
`opui.template_multiple_parents`, `opui.rich_text_empty`,
`opui.text_path_incompatible`, `opui.text_path_unknown`,
`opui.unresolved_inline_image`.

`opui.token_cycle` is reserved if a future catalog allows token-to-token
aliases. v2 catalog values are literals, so only theme `extends` cycles
occur (`opui.theme_cycle`).

Encoding codes are usually checker-output only (the file may be unparsable).
Printed checker diagnostics for those cases use `node_id`/`runtime_id`
`null` and `strategy` `"none"`, and may add a non-manifest `location`.

## 19. Extensions

Top-level `extensions`, each node's `extensions` (runtime and template),
and diagnostic `details` are namespaced maps. Keys match
`^[a-z][a-z0-9-]*(\.[a-z0-9-]+)+$` (reverse domain, at least one dot).
Values are unconstrained JSON.

Unknown keys MUST be ignored by consumers. Extensions MUST NOT change
required layout or paint. Required rendering is fully determined by the
non-extension fields in this spec.

These maps are the only objects that are not `additionalProperties: false`.
All other maps listed in §6 are typed maps, not extension maps.

## 20. Resolution order

Normative, deterministic. Consumers MUST apply this order. No other
pass may change required rendering.

1. **Expand templates.** Instantiate every reachable `component_instance`
   (§14.1 steps 1, 4–5 for structure and ids; variants and overrides are
   the next two steps). Nested instances expand depth-first, parent before
   interpreting the child's payload.
2. **Variant axes.** Declared array order on the definition. Missing
   instance keys use axis `default`.
3. **Instance overrides.** `overrides` patches on template parts.
4. **One matching breakpoint.** Using the consumer viewport width, select
   the unique matching breakpoint (§6.1) and shallow-merge that
   `responsive` entry's `visible` / `layout` / `style`. If none match,
   keep the post-override base.
5. **Resolve theme tokens.** Determine the active theme (§6.3). Replace
   every token ref with the resolved literal. Kind and length-context
   checks apply here.
6. **Resolve locale.** Determine the active locale (§10.1). Replace
   `type: message` sources with the resolved rich text.
7. **Layout / scroll.** Compute boxes. Scroll containers start at offset
   `(0, 0)`. The document does not store or restore a position.
8. **Paint / composite.** §8.7 and §9.

Extensions are not consulted.

## 21. Exporter obligations vs checker-observable rules

**Checker-observable** (must fail closed):

- Schema validity.
- All **Semantic** rules in this document that inspect JSON, asset bytes,
  or SVG text.
- Deterministic encoding of the manifest.
- Fallback ↔ `opui.rasterization` bijection via `node_id`.
- Error diagnostics ⇒ not runtime-loadable.

**Exporter obligations** (not fully observable, or observable only as
bytes the checker already hashes):

- Faithfulness to the source design.
- Choosing native vs raster vs error for unsupported features.
- Producing a particular rasterization (pixels are not compared).
- Choosing catalog ids and `runtime_part_id`s that stay stable.
- Not introducing timestamps or random ids (the checker cannot know
  intent; it only checks the id grammar, `generator` shape, and hash rule).
- SVG sanitization that removes more than the forbidden subset (allowed).

## 22. Complete schema shapes

Normative structural membership is `spec/opui-v2.schema.json`. Summary:

- Manifest: closed, ten required keys (§5).
- `generator`: `{ name, version }`.
- `document`: `{ name, reference_viewport: { width, height } }`.
- `entrypoints`: required `default`, additional properties are `source_id`.
- `resources`: closed; optional `breakpoints` `tokens` `themes`
  `localization` `paths` `components`.
- `nodes`: map → node union.
- Node: closed; required common fields; optional `runtime_id` `name`
  `responsive` `masks`; exactly one of `container` `text` `image`
  `vector` `fallback` `component_instance`.
- Template node: as node, with `template_id`, optional `runtime_part_id`,
  plus `slot`.
- `container`: `{}`.
- `text`: `source`, `defaults`, `align_horizontal`, `align_vertical`,
  `wrap`, `overflow`; optional `text_path`.
- `image`: `asset`, `intrinsic_width`, `intrinsic_height`, `fit`,
  `nine_slice`, `tint`.
- `vector`: `layers`.
- `fallback`: `asset`, `intrinsic_width`, `intrinsic_height`.
- `component_instance`: `definition`, `variants`, `slots`, `overrides`.
- `layout`: closed; internal keys optional with omission defaults in §8.5.
  Length discriminator is `type`. `overflow_x` / `overflow_y` /
  `scrollbar` replace v1 `overflow`.
- `style`: closed; internal keys optional with omission defaults in §9.
  `fills` `borders` `shadows` `filters` `blend_mode` `skew`. No `fill`,
  `border`, `outer_shadows`.
- `assets`: map → `{ kind, uri, mime_type, sha256, byte_length }`.
- `diagnostics[]`: `{ code, severity, node_id, runtime_id, message, strategy, details }`.
- `extensions` / `details`: reverse-domain maps.

## 23. v1-to-v2 conceptual mapping

Not a mechanical rewriter. Exporters that still emit v1 stay on
`schema_version` `1`.

| v1 | v2 |
| --- | --- |
| `schema_version` `1` | `2` |
| nine top-level keys | same nine plus required `resources` (`{}` if unused) |
| `layout.overflow` | `overflow_x` and `overflow_y` (same keyword if it was `visible`/`hidden`) |
| `style.fill` `type: none` | omit `fills` or `fills: []` |
| `style.fill` other | `fills: [ <that fill without a none sibling> ]` |
| `style.border` | `borders: { top, right, bottom, left }` all equal; omit if `style: none` |
| `style.outer_shadows` | `shadows` items with `kind: "outer"` |
| `text.content` + `runs` | `source: { type: "literal", value: { blocks: [{ type: "paragraph", spans: [...] }] } }` — one `text` span per run, `style` from the run |
| `component` metadata | drop, or rewrite as `component_instance` if a definition is exported |
| flattened instance tree | optional: keep as ordinary nodes, or emit a template + instance |
| no tokens | optional token refs; literals remain valid |
| no breakpoints | optional `resources.breakpoints` + `responsive` |

Package layout, asset id/URI/MIME rules, SVG subset, encoding, diagnostic
seven-field wire object and sort, and extension grammar are unchanged.

## 24. Out of scope / forbidden

Deliberately absent:

- Animation, transitions, timelines, playback.
- Interaction handlers, events, gestures, hit-behavior objects.
- Scripts, expressions, game logic, bindings that execute.
- `data:` URIs, `http:` / `https:` / `file:` / protocol-relative / absolute URIs.
- Network fonts and any automatic network fetch.
- Serialized scroll position or scroll snapshots.
- OpenPencil editor internals, Bevy/Rust type names.
- Nested rich-text lists, token-to-token catalog aliases, SVG path
  execution, live boolean path ops beyond the mask array.

v1 leftover codes (`opui.text_run_*`) stay in the enum so a checker can
report a mistaken v1 fragment; they are not produced for valid v2 text.
