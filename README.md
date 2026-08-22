# OPUI

OPUI is the engine-neutral runtime interchange contract between OpenPencil
exporters and UI consumers.

The checker supports both frozen contract versions and dispatches only on
`schema_version`:

```json
{
  "format": "openpencil-runtime-ui",
  "schema_version": 2
}
```

The normative contracts are:

- v1: [`spec/opui-v1.md`](spec/opui-v1.md) and [`spec/opui-v1.schema.json`](spec/opui-v1.schema.json)
- v2: [`spec/opui-v2.md`](spec/opui-v2.md) and [`spec/opui-v2.schema.json`](spec/opui-v2.schema.json)

Consumers must not load an OPUI document unless both structural and semantic
validation succeed.

## Package

```text
<name>.opui
<name>.opui.assets/
  images/
  fonts/
  fallback/
  vectors/
```

Assets are local, content-addressed sidecars. Both versions contain no scripts,
executable behavior, game logic, absolute paths, embedded binary data, or
automatically fetched network resources. v2 adds data-only breakpoints, tokens,
themes, localization, vector paths, and runtime component templates.

## Conformance

Validate one or more packages:

```bash
cargo run -- check fixtures/minimal.opui
```

Write canonical JSON to stdout or an explicit output path:

```bash
cargo run -- canonicalize input.opui
cargo run -- canonicalize input.opui --output canonical.opui
```

Run the complete schema, semantic, asset, canonicalization, and fixture gate:

```bash
cargo test --all-targets
```

The valid fixtures are canonical byte-for-byte. The invalid corpus covers both
schema versions and their semantic reference rules.

## Versioning

Exporters and consumers must target one schema version at a specific contract
commit. Unknown schema versions are rejected. Extensions are namespaced,
ignored by consumers, and cannot affect required layout or painting.
