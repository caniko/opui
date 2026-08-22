# OPUI

OPUI is the engine-neutral runtime interchange contract between OpenPencil
exporters and UI consumers.

The frozen v1 identifier is:

```json
{
  "format": "openpencil-runtime-ui",
  "schema_version": 1
}
```

The normative contract is [`spec/opui-v1.md`](spec/opui-v1.md) together with
[`spec/opui-v1.schema.json`](spec/opui-v1.schema.json). Consumers must not load
an OPUI document unless both structural and semantic validation succeed.

## Package

```text
<name>.opui
<name>.opui.assets/
  images/
  fonts/
  fallback/
  vectors/
```

Assets are local, content-addressed sidecars. OPUI v1 contains no scripts,
executable behavior, game logic, absolute paths, embedded binary data, or
automatically fetched network resources.

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

The valid fixtures are canonical byte-for-byte. The invalid corpus covers
schema versions, traversal, duplicate runtime IDs, unresolved references,
cycles, malformed text runs, and invalid numeric values.

## Versioning

Agents implementing exporters or consumers must target
`openpencil-runtime-ui` schema version `1` at a specific contract commit.
Unknown schema versions are rejected. Extensions are namespaced, ignored by v1
consumers, and cannot affect required layout or painting.
