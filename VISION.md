# OPUI vision

OPUI (`openpencil-runtime-ui`) is the engine-neutral runtime interchange
contract between OpenPencil exporters and UI consumers. This repository holds
the frozen contract text, JSON Schemas, fixtures, and a Rust checker that
validates and canonicalizes `.opui` packages against a specific contract
version.

## Problem

A design exporter and a runtime renderer otherwise agree on a private,
undocumented shape. Every consumer then re-implements the same guesses about
parenting, asset references, and identity, and a format change silently breaks
one of them.

## Principles

- **Frozen versions, explicit dispatch.** The checker supports v1 and v2 and
  dispatches only on `schema_version`; unknown versions are rejected. Exporters
  and consumers target one version at a specific contract commit.
- **Structural and semantic validation are both required.** Schema rules are
  draft 2020-12 constraints; semantic rules — single parent, no cycles, existing
  children, entrypoints naming roots, concrete slot references — are
  checker-enforced. Consumers must refuse a document that fails either, and
  must refuse any document carrying a diagnostic with severity `error`.
- **The prose contract carries meaning.** Where the prose and schema disagree,
  the prose wins for meaning and the schema wins for structural membership.
- **Engine-neutral data only.** Manifests contain no Bevy or Rust type names, no
  OpenPencil object serialization, no scripts, executable behavior, game logic,
  animation, interaction handlers, absolute paths, `data:` URIs, network fonts,
  or automatically fetched resources.
- **Assets are local and content-addressed.** A package is a manifest plus
  sibling `images/`, `fonts/`, `fallback/`, and `vectors/` sidecars, so a
  consumer never needs the network at load time.
- **Canonical bytes are evidence.** `canonicalize` emits canonical JSON, and the
  valid fixtures are canonical byte-for-byte; the invalid corpus exercises both
  versions and their semantic reference rules.
- **Extensions are namespaced and inert.** Consumers ignore unknown extensions,
  and an extension cannot affect required layout or painting.

## Scope

The v1 and v2 contract documents and schemas, the `check` and `canonicalize`
CLI, package/asset validation, fixture corpora for both versions, and stable
diagnostic codes. v2 adds data-only breakpoints, tokens, themes,
localization, vector paths, and runtime component templates.

## Non-goals

- No painting or layout engine: this project validates and canonicalizes, it
  does not render.
- No animation, scripting, interaction handlers, serialized scroll position, or
  network-loaded resources in the contract.
- No compatibility bridging between versions: v2 does not `$ref` v1, and the
  documented mapping is conceptual only.

## Direction

The two commits establish the versioning pattern directly: freeze OPUI v1, then
add OPUI v2 as a separate frozen contract.

## Place in the fleet

The `openpencil` exporter (`op-runtime-ui`) emits packages against this
contract, and the `opui-integration` harness drives OpenPencil's public CLI
through this checker into the Bevy consumer. The atlas Simit updater lists the
repository but disables its enrollment pending a signed runner-policy draft,
Actions, and exact-source hosted qualification.
