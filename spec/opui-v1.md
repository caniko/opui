# openpencil-runtime-ui v1

Frozen interchange format `openpencil-runtime-ui`, `schema_version` integer `1`.
Machine contract: `spec/opui-v1.schema.json` (JSON Schema draft 2020-12).
This document is the semantic contract. Where they conflict, this prose wins
for meaning; the schema wins for structural membership.

v1 is a static, fluid, offline runtime forest. No breakpoints, no animation,
no scripting, no network, no embedded binary.

The manifest contains only engine-neutral runtime data. It MUST NOT contain
Bevy or Rust type names, OpenPencil's internal object serialization,
executable behavior, scripts, game logic, absolute filesystem paths, or
resources that require an automatic network fetch.

## 1. Roles

| Role | Job |
| --- | --- |
| Exporter | Emit a deterministic package: manifest plus sibling asset tree. |
| Checker | Validate schema rules and semantic rules. Report diagnostics. |
| Consumer | Load a checker-clean document and paint it. Ignore unknown extensions. |

A document is **runtime-loadable** only if it is schema-valid, satisfies every
semantic rule, and contains no diagnostic with `severity` `"error"`. Consumers
MUST refuse non-runtime-loadable documents.

## 2. Schema rules vs semantic rules

**Schema rules** are draft 2020-12 constraints in `opui-v1.schema.json`.
They are structural: required keys, enums, closed objects, typed lengths,
node payload unions, empty leaf children, regexes.

**Semantic rules** are checker-enforced and cannot be expressed (or cannot be
expressed completely) in draft 2020-12. They are marked **Semantic** below.

Semantic families:

- Forest: single parent, no cycles, children exist, entrypoints name roots.
- References: node/asset ids resolve; kinds match the referencing field.
- Uniqueness: `runtime_id` unique when present.
- Key equality: map key equals the object's id field; asset id equals the
  constructed `<kind>-<sha256>` string.
- Package stem equals `document.name`.
- Text run ranges after JSON decoding.
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
   hashes.
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
13. Omitted `layout`/`style` fields are not inserted, and explicit default
    values are not stripped. Two documents that differ only by writing or
    omitting a default-valued field are both conforming and are not
    byte-identical.

**Exporter obligation:** produce bytes that already satisfy 1–13.
**Checker-observable:** all of 1–13 on the manifest; 12 also on sidecars.

## 5. Top-level document

All nine top-level keys are required. The object is closed. Exact keys:

`format`, `schema_version`, `generator`, `document`, `entrypoints`,
`assets`, `nodes`, `diagnostics`, `extensions`.

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
  "generator": { "name": "openpencil", "version": "1.0.0" },
  "nodes": { "root": { "...": "..." } },
  "schema_version": 1
}
```

| Key | Meaning |
| --- | --- |
| `format` | Const `"openpencil-runtime-ui"`. |
| `schema_version` | Const integer `1`. |
| `generator` | Closed. Required `name`, `version` (non-empty strings). No timestamp. |
| `document` | Closed. Required `name` (package stem) and `reference_viewport`. |
| `entrypoints` | Map of name → `source_id`. `default` is required. |
| `assets` | Map of asset id → asset. Empty if unused. |
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
- `runtime_id` values that are present are unique (`opui.duplicate_runtime_id`).

## 6. Nodes

Nodes are a map, not a tree-inlined array. Each node is keyed by `source_id`.

Required common fields: `type`, `source_id`, `visible`, `children`, `layout`,
`style`, `extensions`.

Optional: `runtime_id`, `name`, `component`.

`runtime_id` matches `^[A-Za-z][A-Za-z0-9._/-]*$`.

- `source_id` preserves the source OpenPencil node identity. It is opaque to
  consumers and is used for provenance and graph references.
- `runtime_id`, when present, is semantic and stable across equivalent
  exports. It is the only node identifier applications bind behavior to.
- `name` is descriptive only. Applications MUST NOT treat it as a stable
  identifier or binding interface.

Union `type`: `container` | `text` | `image` | `fallback`.

Exactly one type payload object is present, named like the type:

- `container` → `container` (empty closed object)
- `text` → `text`
- `image` → `image`
- `fallback` → `fallback`

Irrelevant payloads MUST be absent (schema).

`children` is an array of `source_id` strings. Order is the parent's child
order (flex/grid order and default paint order).

Text, image, and fallback are leaves: `children` MUST be `[]` (schema).

```json
{
  "children": ["title"],
  "container": {},
  "extensions": {},
  "layout": { "...": "..." },
  "source_id": "root",
  "style": { "...": "..." },
  "type": "container",
  "visible": true
}
```

### 6.1 Forest (**Semantic**)

- Every `children` entry names an existing node (`opui.unknown_child`).
- A node appears in at most one parent's `children` (`opui.multiple_parents`).
- A node appears at most once in one `children` array (`opui.duplicate_child`).
- The directed graph of child edges is acyclic (`opui.cycle`).
- Roots are nodes that appear in no `children` array.
- Nodes not reachable from any entrypoint are invalid (`opui.unreachable_node`).

This is a forest. Multiple roots are allowed. Runtime starts at
`entrypoints.default`.

## 7. Components

`component` is flattened instance metadata only. Not a graph edge.

```json
{
  "component_name": "Button",
  "definition_source_id": "Button/Primary",
  "role": "instance",
  "variant_properties": { "size": "lg" }
}
```

Required: `role` (const `"instance"`), `definition_source_id`,
`component_name`, `variant_properties` (string-to-string map, may be empty).

`definition_source_id` is opaque. It is **not** a `source_id` and MUST NOT
be resolved against `nodes` or `assets`. Instances are already expanded into
ordinary nodes.

## 8. Layout

v1 is **fluid only**. There are no breakpoint objects, media queries, or
variant sets. Design size lives only on `document.reference_viewport`.

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

### 8.2 Visibility vs `display: none`

`visible` is a stored boolean. It is not CSS `visibility` by itself.

**Computed visible** (paint and text/image drawing):

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

Every Length discriminator is exactly `type` (not `unit`).

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
| `border.width` `letter_spacing` `size` (font) `outer_shadows.*.offset_*` `spread` `translation.*` | `px` |
| `outer_shadows.*.blur` | `px` ≥ 0 |
| grid track | `px` `percent` `fr` `auto` `minmax` |

`min_width` / `min_height` with `auto` means `0` in this subset (not the CSS
flex automatic minimum size).

A Length `type` outside the field's row is `opui.length_context`. The
published schema already splits $defs per field.

### 8.5 Layout object

Closed. Internal properties are optional. `{}` is valid. Consumers apply
the omission defaults below before layout. Present nested objects
(`padding`, `margin`, `inset`, `gap`, placements) still require their own
keys. Inapplicable flex/grid fields, when omitted, take these defaults and
are ignored when `display` does not use them.

Exporters SHOULD omit a field when its value equals the default, to keep
packages small. Explicit default values are conforming. Byte
canonicalization does not drop or insert these fields
(`opui.key_order` still applies to keys that are present).

| Property | Omitted default |
| --- | --- |
| `display` | `absolute` (free-position child mode) |
| `position` | `flow` |
| `overflow` | `visible` |
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

```json
{
  "align_content": "start",
  "align_items": "stretch",
  "align_self": "auto",
  "aspect_ratio": { "type": "auto" },
  "display": "flex",
  "flex_basis": { "type": "auto" },
  "flex_direction": "column",
  "flex_grow": 0,
  "flex_shrink": 1,
  "flex_wrap": "nowrap",
  "gap": {
    "column": { "type": "px", "value": 0 },
    "row": { "type": "px", "value": 0 }
  },
  "grid_auto_flow": "row",
  "grid_column": { "end": { "kind": "auto" }, "start": { "kind": "auto" } },
  "grid_row": { "end": { "kind": "auto" }, "start": { "kind": "auto" } },
  "grid_template_columns": [],
  "grid_template_rows": [],
  "height": { "type": "auto" },
  "inset": {
    "bottom": { "type": "auto" },
    "left": { "type": "auto" },
    "right": { "type": "auto" },
    "top": { "type": "auto" }
  },
  "justify_content": "start",
  "justify_items": "stretch",
  "justify_self": "auto",
  "margin": {
    "bottom": { "type": "px", "value": 0 },
    "left": { "type": "px", "value": 0 },
    "right": { "type": "px", "value": 0 },
    "top": { "type": "px", "value": 0 }
  },
  "max_height": { "type": "none" },
  "max_width": { "type": "none" },
  "min_height": { "type": "auto" },
  "min_width": { "type": "auto" },
  "overflow": "visible",
  "padding": {
    "bottom": { "type": "px", "value": 0 },
    "left": { "type": "px", "value": 0 },
    "right": { "type": "px", "value": 0 },
    "top": { "type": "px", "value": 0 }
  },
  "position": "flow",
  "width": { "type": "percent", "value": 100 },
  "z_index": 0
}
```

`overflow`: `visible` | `hidden`. Both axes. v1 has no scroll containers.
`hidden` clips paint and layout overflow to the padding box.

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
`flex-flow` / other shorthands, scrolling, margin collapsing, writing-mode
other than horizontal-tb / ltr.

Alignment keywords are the CSS values with hyphens turned into underscores
(`space_between`). There is no `flex-start`; use `start`.

Child order is `children` array order. There is no `order` property.

### 8.7 Paint order

For siblings that paint:

1. Sort by `z_index` ascending (more negative first).
2. Break ties by `children` index (earlier first).

`display: none` and not-computed-visible nodes and their descendant boxes
do not paint. Outer shadows paint behind the node's border box, in
`outer_shadows` array order (first behind). Transforms apply after layout
about `transform_origin`, in the fixed order translation, then rotation,
then scale. No skew.

## 9. Style

Closed. Internal properties are optional. `{}` is valid. Consumers apply
omission defaults before paint. Present nested objects (`fill`, `border`,
`corner_radius`, shadow items, `translation`, `rotation`, `scale`,
`transform_origin`) still require their own keys.

Exporters SHOULD omit a field when its value equals the default. Explicit
defaults are conforming. Canonicalization does not drop or insert them.

| Property | Omitted default |
| --- | --- |
| `opacity` | `1` |
| `clipping` | `false` |
| `fill` | `{ "type": "none" }` |
| `border` | `style` `none`, `width` `{ "type": "px", "value": 0 }`, color transparent black `{ "space": "srgb", "r": 0, "g": 0, "b": 0, "a": 0 }` |
| `corner_radius` | all corners `{ "type": "px", "value": 0 }` |
| `outer_shadows` | `[]` |
| `translation` | `{ "x": { "type": "px", "value": 0 }, "y": { "type": "px", "value": 0 } }` |
| `rotation` | `{ "degrees": 0 }` |
| `scale` | `{ "x": 1, "y": 1 }` |
| `transform_origin` | `{ "x": 0.5, "y": 0.5 }` (center) |

```json
{
  "border": {
    "color": { "a": 1, "b": 0, "g": 0, "r": 0, "space": "srgb" },
    "style": "none",
    "width": { "type": "px", "value": 0 }
  },
  "clipping": false,
  "corner_radius": {
    "bottom_left": { "type": "px", "value": 0 },
    "bottom_right": { "type": "px", "value": 0 },
    "top_left": { "type": "px", "value": 0 },
    "top_right": { "type": "px", "value": 0 }
  },
  "fill": { "type": "none" },
  "opacity": 1,
  "outer_shadows": [],
  "rotation": { "degrees": 0 },
  "scale": { "x": 1, "y": 1 },
  "transform_origin": { "x": 0.5, "y": 0.5 },
  "translation": {
    "x": { "type": "px", "value": 0 },
    "y": { "type": "px", "value": 0 }
  }
}
```

`clipping: true` clips this node's painting and descendants to the rounded
border box. Combined with `overflow: hidden`, the intersection clips.

### 9.1 Color

Unpremultiplied RGBA. Channels are numbers in `0..1`. Color space is
`srgb` or `srgb-linear`.

```json
{ "a": 1, "b": 0.2, "g": 0.4, "r": 0.8, "space": "srgb" }
```

### 9.2 Fill

Exactly one fill. Types: `none`, `solid`, `linear`, `radial`.

Linear and radial geometry is complete and normalized against the node's
border box: `x`/`y` `0` is the top-left, `1` is the bottom-right. Values
outside `0..1` are allowed (gradient continues).

- Linear: `start`, `end` points, `stops`.
- Radial: `center`, `radius` as an axis-aligned ellipse. `radius.x` and
  `radius.y` are radii in the same normalized space and MUST be `> 0`.

Stops: at least two. Each has `offset` in `0..1` and a color.
**Semantic:** the checker enforces nondecreasing `offset` values
(`opui.gradient_stop_order`). Equal offsets are allowed (hard edge: later
stop wins). Decreasing offsets are invalid.

### 9.3 Border, corner radius, outer shadows, transforms

Border is uniform on all four sides. `style` is `none` or `solid`.
`width` is `{ "type": "px", "value": ≥ 0 }`.

`corner_radius` is per corner, `px` or `percent` ≥ 0. Percent is relative
to the corresponding border-box axis as in CSS.

`outer_shadows` is an array of outer shadows (empty if none). Each item:

```json
{
  "blur": { "type": "px", "value": 0 },
  "color": { "a": 1, "b": 0, "g": 0, "r": 0, "space": "srgb" },
  "offset_x": { "type": "px", "value": 0 },
  "offset_y": { "type": "px", "value": 0 },
  "spread": { "type": "px", "value": 0 }
}
```

No inner shadows. Array order is back-to-front under the border box.

Transforms are named fields, not a list, and not skew:

| Field | Shape |
| --- | --- |
| `translation` | `{ "x": <px Length>, "y": <px Length> }` |
| `rotation` | `{ "degrees": <finite number> }` clockwise |
| `scale` | `{ "x": <finite number>, "y": <finite number> }` |
| `transform_origin` | `{ "x": 0..1, "y": 0..1 }` in border-box fraction; `(0.5, 0.5)` is center |

## 10. Text

```json
{
  "align_horizontal": "start",
  "align_vertical": "start",
  "content": "Hello",
  "defaults": {
    "color": { "a": 1, "b": 0, "g": 0, "r": 0, "space": "srgb" },
    "family": "Inter",
    "font_style": "normal",
    "letter_spacing": { "type": "px", "value": 0 },
    "line_height": { "type": "normal" },
    "size": { "type": "px", "value": 16 },
    "weight": 400
  },
  "overflow": "clip",
  "runs": [],
  "wrap": "wrap"
}
```

`content` is a Unicode string (JSON string after decoding).

`defaults` is a complete text style. `font` is optional; when present it is
a `font-<sha256>` asset id.

Sparse `runs` override defaults on a byte range:

```json
{
  "end": 5,
  "start": 0,
  "style": { "weight": 700 }
}
```

Offsets are UTF-8 byte offsets into `content` **after JSON decoding**,
half-open `[start, end)`.

**Semantic**

- `0 ≤ start < end ≤ utf8_len(content)` (`opui.text_run_range`,
  `opui.text_run_empty`).
- `start` and `end` lie on Unicode scalar boundaries, not in a surrogate
  or mid-code-unit (`opui.text_run_scalar`). JSON has no surrogates after
  decoding; this still forbids splitting a multi-byte UTF-8 scalar.
- Runs are sorted by `start` ascending (`opui.text_run_order`).
- Runs do not overlap (`opui.text_run_overlap`).
- `font` resolves to an asset with `kind` `"font"` (`opui.unresolved_font`).

`line_height`: `{ "type": "normal" }` | `{ "type": "multiple", "value": > 0 }`
| `{ "type": "px", "value": ≥ 0 }`. Not a layout Length.

`align_horizontal`: `start` `center` `end` `justify`.
`align_vertical`: `start` `center` `end`.
`wrap`: `wrap` `nowrap`.
`overflow`: `clip` `ellipsis`.

`style.fill` paints the text node's background. Glyph color comes from
text style `color`. Opacity and clipping are only on common `style`.

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

`tint` is `null` or a color. When set, each unpremultiplied source channel
is multiplied by the tint channel (including alpha).

Opacity and clipping are only on common `style`.

**Semantic:** `asset` exists and `kind` is `image` or `vector`
(`opui.unresolved_image`, `opui.asset_kind_mismatch`).

## 12. Fallback

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

## 13. Assets

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

### 13.1 Deterministic id and URI

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

### 13.2 Static SVG security subset

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

## 14. Unsupported-feature strategy

When the source cannot be represented as a standard `container` / `text` /
`image` node, the diagnostic `strategy` field records the choice.

| `strategy` | Node | Diagnostic | Loadable |
| --- | --- | --- | --- |
| `native` | Existing standard node that approximates the source | `node_id` is that standard node's `source_id`. Typical codes: `opui.unsupported_native`, `opui.mask_unsupported`, `opui.effect_unsupported`, `opui.path_unsupported` | yes if severity is not `error` |
| `raster_fallback` | `type: fallback` plus raster asset | Exactly one `opui.rasterization` diagnostic per fallback: `node_id` is the fallback `source_id`, `strategy` is `raster_fallback`. The raster is that node's `fallback.asset`. `opui.unsupported_raster_fallback` is optional source detail, not a second required diagnostic | yes if severity is not `error` |
| `error` | optional | Unsupported-feature refusal. Requires `severity` `"error"` (schema). Typical code `opui.unsupported_error` | no |
| `none` | n/a | Ordinary validator/exporter notes, not an unsupported-feature choice. May use `severity` `"error"` | no if severity is `error` |

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
non-null they MUST match that node (`opui.diagnostic_runtime_id`).

## 15. Diagnostics

Closed object. Exact required fields:

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

### 15.1 Ordering (**Semantic**)

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

### 15.2 Codes

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

Encoding codes are usually checker-output only (the file may be unparsable).
Printed checker diagnostics for those cases use `node_id`/`runtime_id`
`null` and `strategy` `"none"`, and may add a non-manifest `location`.

## 16. Extensions

Top-level `extensions`, each node's `extensions`, and diagnostic `details`
are namespaced maps. Keys match `^[a-z][a-z0-9-]*(\.[a-z0-9-]+)+$`
(reverse domain, at least one dot). Values are unconstrained JSON.

Unknown keys MUST be ignored by consumers. Extensions MUST NOT change
required layout or paint. Required rendering is fully determined by the
non-extension fields in this spec.

These maps are the only objects that are not `additionalProperties: false`.
`entrypoints`, `nodes`, `assets`, and `variant_properties` are typed maps,
not extension maps.

## 17. Exporter obligations vs checker-observable rules

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
- Computing UTF-8 run offsets from the pre-escape source text correctly
  (the checker only sees the decoded string).
- Not introducing timestamps or random ids (the checker cannot know
  intent; it only checks the id grammar, `generator` shape, and hash rule).
- SVG sanitization that removes more than the forbidden subset (allowed).

## 18. Complete schema shapes

Normative structural membership is `spec/opui-v1.schema.json`. Summary:

- Manifest: closed, nine required keys (§5).
- `generator`: `{ name, version }`.
- `document`: `{ name, reference_viewport: { width, height } }`.
- `entrypoints`: required `default`, additional properties are `source_id`.
- `nodes`: map → node union.
- Node: closed; required common fields; optional `runtime_id` `name`
  `component`; exactly one of `container` `text` `image` `fallback`.
- `component`: `{ role: "instance", definition_source_id, component_name, variant_properties }`.
- `container`: `{}`.
- `text`: `content`, `defaults`, `runs`, `align_horizontal`,
  `align_vertical`, `wrap`, `overflow`.
- `image`: `asset`, `intrinsic_width`, `intrinsic_height`, `fit`,
  `nine_slice`, `tint`.
- `fallback`: `asset`, `intrinsic_width`, `intrinsic_height`.
- `layout`: closed; internal keys optional with omission defaults in §8.5.
  Length discriminator is `type`.
- `style`: closed; internal keys optional with omission defaults in §9.
- `assets`: map → `{ kind, uri, mime_type, sha256, byte_length }`.
- `diagnostics[]`: `{ code, severity, node_id, runtime_id, message, strategy, details }`.
- `extensions` / `details`: reverse-domain maps.

## 19. Out of scope for v1

Breakpoints, scrolling, inner shadows, multiple fills, per-side borders,
filters, blend modes, skew, text-on-path, rich text objects beyond sparse
runs, component graphs, network fonts, `data:` URIs, animation, interaction.
