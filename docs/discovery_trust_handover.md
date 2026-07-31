# 歸屬信任根 / Affiliation trust roots

**Opened** 2026-07-31. **Baseline** `dev 7714977` (engine v0.6.0).
**Probes** `crates/oo/tests/discovery_trust_probe_test.rs` — created and
calibrated before this order was written. Workspace at baseline: **1708 passed,
0 failed, 14 ignored** (11 of those ignored are this arc's reds; 3 are standing).
Every red was run independently and failed; see §8.

**Classification: 增量 (Layer 1).** This arc adds an opt-in, workspace-local
configuration and a CLI surface. With the file absent — every existing
workspace — behaviour is unchanged. It changes no wire format, CAID, language
surface or network policy. Per VERSIONING §5/§6, non-editorial incremental work
uses the minor position; the eventual candidate is therefore expected to be
v0.7.0, not a v0.6 patch.

---

## 1. The defect is a false pointer, not merely missing code

REAL_02 §4.2.8 postponed the question "which operators do I care about?" and
sent its answer to the assertion layer "beside `~/.oo/authorized_keys`". SPEC_13
§4.1.2 obligation #3 uses the same pointer for trust configuration generally.
That pointer is wrong.

REAL_01 §7.0 split privilege into two faces on 2026-07-27:

| face | requester | mechanism |
| :-- | :-- | :-- |
| local one-shot / CLI | the workspace operator | per-invocation capability |
| service / JSON-RPC | a remote party | token + connection authentication |

§7.0.2 then says §7.1/§7.2's token format, lifecycle, CRL and
`~/.oo/authorized_keys` apply **only to the service face**, and are inapplicable
until that face exists. The engine has no service face, no token verifier, no
CRL and no `authorized_keys` reader.

So `authorized_keys` cannot simultaneously mean:

1. issuers whose signatures authorise remote service privilege tokens; and
2. operators whose affiliation claims this local node may later read as
   admission consent.

Those are different questions in REAL_01 §7.6. Sharing a file because both hold
public keys would be the same category error that table was written to prevent.

### 1.1 The active precedent, and why it is not the answer either

The live governance whitelist is workspace-local `.oo/architects.json`:

* load: `crates/interpreter/src/storage.rs::load_architects`
* runtime field: `Ouroboros::architect_registry`
* authority consumer: `authority.rs::verify_refine_authority`

That establishes the **layer and scope** precedent — workspace assertion state —
but not a shared list. At least four distinct authority declarations exist or
are specified:

| declaration | question answered |
| :-- | :-- |
| `~/.oo/authorized_keys` | who may issue service-face privilege tokens? |
| `.oo/architects.json` | who may sign `#refine` in this universe? |
| REAL_02 §6.2 package roots | who may authorise package-blacklist updates? |
| this arc's affiliation roots | whose affiliation may express this node's admission consent? |

Do not call the new one "the third list". Counting is brittle and already
became false; the questions are the invariant.

---

## 2. The six accepted rulings

### A. The root applies to affiliation, not general trust

Canonical spec term: **歸屬信任根 / affiliation root**.
Canonical configuration field:

```nlang
affiliation_roots: [
    "1111111111111111111111111111111111111111111111111111111111111111"
]
```

An entry means only:

> A cryptographically valid affiliation claim signed by this operator MAY, in a
> later arc, be read as this node's consent to admit the claimed node.

It grants none of the following:

* `#refine` / governance authority;
* package or blacklist authority;
* service-token issuance authority;
* a language capability;
* degree-0 correctness or fetch-source preference;
* permission to bypass advert, signature or CAID verification.

The field is deliberately not `trusted_operators`: that name would claim more
than the mechanism says.

### B. Scope is the workspace/node

The path is exactly:

```text
<workspace>/.oo/discovery.n
```

The future effect is admission into **this `Ouroboros` instance's** source set.
A home-global file would make one decision in workspace A silently authorise the
same operator across B, C and D under the same Unix account. That is consent
scope leakage, not convenience.

REAL_02 §5.1 already reserves `.oo/discovery.n` for 「發現節點與信任設定」.
Use it. Do not invent `.oo/trusted_operators.json`, and do not put the new list
under `~/.oo/`.

### C. Absence, empty and unreadable are distinct

| physical state | semantic result |
| :-- | :-- |
| no `.oo/discovery.n` | valid empty set |
| `affiliation_roots: []` | valid empty set |
| present but unreadable, malformed or invalid | named error; operation fails |

Every error must name `discovery.n` (preferably the full path) and the reason.
It must propagate through `Ouroboros::init`, so ordinary commands such as
`oo status` cannot proceed while pretending the declaration is empty.

Reconnaissance measured the opposite precedent in the existing governance
loader:

```rust
.load_architects(base_dir)
.unwrap_or_else(|_| HashSet::new())
```

With `.oo/architects.json` containing `{not-json`, `oo status` exited 0 and
printed `Universe is static (no staged changes).` That is fail-safe but
indistinguishable from absence. Do **not** copy it. Also do **not** repair it in
this arc; governance configuration honesty is a separate debt.

### D. The `.n` file is closed data, never a program

Use the n/ parser, but do not evaluate the result. Accept exactly one top-level
field named `affiliation_roots`, whose value is a literal list of literal
strings. Reject:

* unknown or misspelled top-level fields;
* morphisms, applications, paths, interpolation or any other executable shape;
* non-string list members;
* public keys not exactly 64 lowercase hexadecimal characters.

The public-key declaration is already the trusted out-of-band assertion. Key
validation does not decide whether the operator is trustworthy; it rejects a
name that can never match the Ed25519 key in a verified affiliation claim.

The canonical file writer emits one key per line, sorted lexicographically.
Treat the collection as a set. Do not rely on `HashSet` iteration order.

### E. The management surface is local CLI only

Add this nested command surface:

```text
oo node trust list
oo node trust add <operator-key>
oo node trust remove <operator-key>
```

Required observable behaviour:

* `list`: keys only, one per line, sorted; an empty set prints no key lines;
* `list` with no file: success, empty output, and **does not create the file**;
* successful `add`: output contains `added` and the canonical key;
* successful `remove`: output contains `removed` and the canonical key;
* invalid CLI input: non-zero, explains "64 lowercase hex", writes nothing.

No `--grant` is required. The local operator can already edit `.oo/`; the CLI is
a validated out-of-band editor, not an authority source. No n/ morphism or
builtin may be added. In particular, do not resurrect `~%Official./add_architect`
or create its affiliation analogue.

A canonical rewrite should not expose a partially-written policy. Prefer a
same-directory temporary file plus replacement, and leave no temporary file on
success. This is an implementation property, not permission to add another
persistent format.

### F. This arc establishes the root and does not consume it

Even with a non-empty root, this delivery must not:

* insert anything into `Ouroboros.peers`;
* dial any address;
* create `.oo/peers/`;
* change the durable peer directory;
* retain, rank or evict an advert differently;
* change `#advertise`, `#discover`, FIND_NODE or `#fetch`;
* turn the peer directory into a fetch source;
* accept or reject an advert because of affiliation;
* persist a derived `verified_operator_key` verdict;
* mint an operator or node identity;
* change `.oo/format`;
* write universe objects or move the universe root.

② answers "whose statements could count as consent?". ③ answers "when does a
verified statement cause admission, and how many such admissions may exist?".
They are two different things, so the split is legitimate.

---

## 3. Measured implementation reality

### 3.1 Existing pieces the delivery should reuse

* `.oo/` is already an unconditional language boundary:
  `crates/interpreter/src/builtins/fs_guard.rs` rejects any resolved path with an
  exact `.oo` component. Measured candidate write:

  ```text
  _|_ (%cause: #store_boundary)
  candidate-file-exists=no
  ```

  No new path guard is required. P1 pins both an ordinary-file control and the
  refusal in one run.

* `PeerAdvert.verified_operator_key` already contains the derived, verified
  affiliation operator key and is rebuilt from verbatim `%ad`; it is not
  persisted (`lib.rs:304–306`, `peers.rs::refresh_affiliations`). Do not change
  it in this arc.

* `Ouroboros.peers` is the active fetch-source map. `disc.connect` is the current
  remote writer and v0.6.0 gates it with `--grant connect`. No affiliation path
  writes it yet. Leave it that way.

* `Ouroboros::init` already returns `Result`, so a configuration error can be
  propagated rather than collapsed.

### 3.2 A suitable implementation boundary

Recommended shape (names may follow surrounding idiom, behaviour may not
change):

```text
crates/interpreter/src/discovery_config.rs
    DiscoveryConfig { affiliation_roots: BTreeSet<String> }
    load(base_dir) -> Result<DiscoveryConfig>
    write(base_dir, &DiscoveryConfig) -> Result<()>
    validate_operator_key(&str) -> Result<()>

crates/interpreter/src/lib.rs
    pub mod discovery_config
    Ouroboros gains the loaded affiliation-root set
    new_in_memory => empty
    init => load and propagate errors

crates/oo/src/main.rs
    NodeCmd::Trust { subcommand: TrustCmd }
    TrustCmd::{List, Add { operator_key }, Remove { operator_key }}
```

The runtime field is intentional even though this arc has no consumer: ③ must
consume the same parsed declaration that `list` reports, not independently
re-read or reinterpret the file.

Do not put these functions on `ObjectStore`: affiliation roots are assertion
configuration beside the store, not content-addressed objects.

---

## 4. Probe map

### 4.1 Controls and pins — green at baseline

| test | property |
| :-- | :-- |
| `control_store_initialization_is_live` | status really initialized `.oo/format` and objects |
| `control_existing_node_surface_is_live` | `oo node peers` exists, so missing `trust` is distinguishable from missing `node` |
| `p1_language_boundary_already_covers_discovery_config` | ordinary file write works; `.oo/discovery.n` write is `#store_boundary` and absent |
| `p2_ordinary_work_does_not_create_trust_or_keys` | ordinary init/commit/peers creates neither declaration nor either identity |
| `p3_discovery_config_never_reaches_the_universe_root` | two non-empty committed stores, config in one only, same root digest |
| `p4_a_root_alone_creates_no_peer_source_or_network_identity` | a manually supplied root creates no peer, peer directory or identity |
| `p5_other_authority_lists_remain_separate` | discovery trust neither rewrites architects nor instantiates `authorized_keys` |

P3 first proves both stores and both roots non-empty. P2 pairs every absence with
`.oo/format` and HEAD presence. These are controls, not comments.

### 4.2 Red gate — 11 ignored tests

| test | delivery obligation | calibrated baseline failure |
| :-- | :-- | :-- |
| `red_list_surface_exists_and_missing_means_empty_without_creation` | list surface; absent = empty/no write | `unrecognized subcommand 'trust'` |
| `red_add_persists_the_reserved_literal_config` | add + file + list | `unrecognized subcommand 'trust'` |
| `red_add_remove_round_trip_is_exact_and_sorted` | set semantics, sorting, exact removal | first add command absent |
| `red_malformed_config_is_a_loud_named_error` | parse failure propagates and names file | status succeeds; silently empty |
| `red_short_key_is_a_loud_named_error` | impossible key rejected | status succeeds; silently empty |
| `red_uppercase_key_is_rejected_not_silently_normalized` | one canonical public-key spelling | status succeeds; accepted/ignored |
| `red_unknown_field_is_not_silently_ignored` | closed shape catches typo | status succeeds; silently empty |
| `red_roots_are_workspace_local_even_under_one_home` | two workspaces share one HOME but not roots | trust surface absent in both |
| `red_trust_management_mints_no_keys_and_admits_no_peers` | CLI only edits config; objects/format/keys/peers untouched | trust surface absent |
| `red_invalid_cli_key_is_rejected_before_any_write` | CLI validates before file creation | command absent; required diagnostic missing |
| `red_config_path_that_cannot_be_read_as_a_file_is_loud` | read error distinct from absence | status succeeds; directory silently ignored |

The shared-HOME red genuinely overrides both processes to the **same HOME**;
operator and node key paths remain per-workspace so the test observes only trust
scope rather than causing a key collision.

---

## 5. Files and suites that must be inspected, not blindly edited

### Expected implementation sites

* `crates/interpreter/src/lib.rs`
* a new focused interpreter module for discovery configuration
* `crates/oo/src/main.rs`

### Existing probes constraining durable state

An untruncated scan found all current allow-list walkers:

* `crates/oo/tests/local_gc_probe_test.rs::p4_no_undeclared_durable_state`
* `crates/oo/tests/advert_persistence_probe_test.rs::r2_the_file_appears_where_declared_and_nowhere_else`
* `crates/oo/tests/kademlia_table_probe_test.rs::p4_nothing_persisted`

Do **not** mechanically add `discovery.n` to all three arrays:

* the first two exercise workflows that never configure trust, so the file
  should remain absent and their property still holds without an allow-list
  change;
* the Kademlia pin explicitly says routing creates no undeclared durable state;
  adding an allowance would weaken it for no reason.

Only change an existing probe if the implemented behaviour makes its premise
false and report that collision to the acceptor first. The standing rule is to
grep these pins whenever a durable file is added; it is not permission to widen
them pre-emptively.

### Existing tests that should remain owners of their properties

* `store_boundary_probe_test.rs` owns the generic `.oo` boundary.
* `affiliation_claim_probe_test.rs` owns claim verification, expiry and the
  three arrival paths.
* `connect_consent_probe_test.rs` owns explicit remote-connect capability.
* `wire_says_why_probe_test.rs` owns scan failure classification.

Do not duplicate or refactor them into this arc.

---

## 6. Delivery scope and forbidden scope

### In scope

1. parse/load the closed `.oo/discovery.n` shape;
2. expose the immutable loaded roots on `Ouroboros` for the next arc;
3. propagate malformed/read errors honestly;
4. implement `oo node trust list/add/remove`;
5. canonical, sorted persistence;
6. remove this suite's 11 `#[ignore]` attributes and nothing else in the probe.

### Explicitly out of scope

* automatic admission, source-set insertion or its cap;
* persistence of admitted sources;
* any dial or packet;
* any ranking/preference of fetch sources (SPEC_13 §6.1.1 forbids it at degree 0);
* any change to affiliation claim wire bytes or expiry;
* fixing malformed `.oo/architects.json` being swallowed;
* implementing service tokens, `authorized_keys` or CRL;
* implementing package trust roots;
* changing `.oo/format`;
* spec, CHANGELOG, VERSION or ENGINE_SYNC edits;
* release bump/tag/cut;
* cleanup/refactors unrelated to this declaration.

Spec closure belongs to the **acceptor after delivery**. The delivery must not
edit `nlang-spec`.

---

## 7. Satisfiability check

The order is satisfiable without crossing arc boundaries:

1. Missing config maps to `DiscoveryConfig::default()`.
2. Present config is parsed as AST data, validated and stored as a `BTreeSet`.
3. `Ouroboros::init` loads it with `?`; in-memory engines use empty.
4. The CLI reads/modifies/writes the same `DiscoveryConfig` type.
5. No code consults the set for network behaviour.

Thus all 11 reds can turn green while P4 remains green. There is no requirement
for "A broken while B good" where A and B are structurally inseparable:

* config parsing and network admission are separate functions;
* workspace paths and HOME are independently controlled;
* root content and assertion files are independently observable;
* key validation precedes writing, so failed input and file absence are
  separable.

If any of those statements proves false during implementation, stop and report
it rather than weakening a probe. The affiliation arc's impossible "claim-only
tamper" is the precedent this check exists to prevent.

---

## 8. Calibration record

### 8.1 Suite

```text
cargo test --test discovery_trust_probe_test
18 tests: 7 passed, 0 failed, 11 ignored
```

### 8.2 Every red independently

Each name was enumerated from the complete file scan — no `head`, no truncation —
and run with `--ignored --exact --nocapture`. All 11 failed. The two families of
failure are distinct and expected:

* R1–R3/R8–R10: the CLI surface does not exist;
* R4–R7/R11: existing commands ignore `discovery.n`, so malformed or unreadable
  declarations silently behave as absence.

### 8.3 Workspace

```text
177 suites
1708 passed
0 failed
14 ignored
```

No non-`ok` test-result block occurred. Existing compiler warnings are unchanged
and not in scope. `cargo fmt --all -- --check` is also red before delivery across
many untouched interpreter files; the new probe itself was formatted with
`rustfmt`, so whole-tree formatting is recorded as baseline debt rather than an
impossible delivery gate.

---

## 9. Delivery rules

The delivery agent may:

* add implementation files;
* edit the expected implementation sites;
* remove the 11 `#[ignore = ...]` attributes from
  `discovery_trust_probe_test.rs`.

The delivery agent may **not** otherwise edit that probe. Probe modification
rights belong to the acceptor. If a probe is impossible, wrong or constrains an
implementation spelling rather than the stated property, report it; do not
accommodate or rewrite it.

Also:

* no `git add -A`;
* no stash;
* nlang-tools commit message in English;
* do not commit generated/temp files;
* format every touched Rust file and check those files with `rustfmt --check`;
  `cargo fmt --all -- --check` is already red on untouched baseline files and is
  not a satisfiable gate for this arc;
* run this suite, then the whole workspace;
* commit implementation as one delivery commit on `dev`;
* do not bump the version.

Suggested delivery commit message:

```text
Add workspace-local affiliation trust roots
```

### Delivery record (delivery side)

**Built**

- `crates/interpreter/src/discovery_config.rs`: `DiscoveryConfig` with
  `BTreeSet` roots; `load` / `write` / `validate_operator_key`.
- Closed parse via `parse_program` (data only — no eval). Rejects unknown
  fields, non-string members, short/uppercase keys, unreadable paths.
- Absence / empty list → empty set; present-but-bad → named error containing
  `discovery.n`.
- Canonical write: sorted keys, temp file + rename under `.oo/`.
- `Ouroboros.affiliation_roots` loaded on `init` (`?`); empty for in-memory.
- CLI: `oo node trust list|add|remove` — no `--grant`; no network side effects.

**Not built (ruling F)**

No peer admission, dial, directory change, advert policy, or identity mint.

**Numbers**

| Suite | Result |
| --- | --- |
| discovery_trust | **18/18** |
| workspace | **1719 / 0 / 3** |
| conf | **143/143** |
| genesis | **11/11** |

Existing durable allow-list probes **untouched** (trust file absent in those
workflows). Spec/CHANGELOG not edited.

---

## 10. Acceptance plan (acceptor, after delivery)

1. **Diff purity**: probe changed only by removal of its 11 ignores; no spec or
   version files; no peer/admission implementation.
2. **Independent rerun**: probe suite, full workspace, conformance, genesis.
3. **Malformed matrix**: malformed AST, short key, uppercase key, unknown field,
   directory at path; each names `discovery.n` and does not silently empty.
4. **Workspace scope**: two workspaces under one HOME; add in A, B stays empty.
5. **No manufacture**: list absent creates no file; add/list/remove mint no
   identity, peer directory or object, and `.oo/format` stays byte-identical.
6. **Universe comparison**: both sides committed and non-empty; config in one;
   root digests equal.
7. **Old/new local compatibility**:
   * v0.6.0 opens a workspace containing valid `.oo/discovery.n` (it ignores the
     optional unknown file);
   * new engine opens an old workspace with no file as empty.
8. **Canonical bytes**: add B then A; file and list are sorted; remove A removes
   only A.
9. **Cost**: measure file bytes for 1 and 100 roots. This is a conformance/cost
   measurement for ENGINE_SYNC, not a mathematical argument for the spec.
10. **Adversarial scope**: verify no newly added call site reads the roots from
    network input or makes a dial. This arc has no remote-input parser of its
    own; `discovery.n` is operator input.

---

## 11. Ledger — known and deliberately not fixed here

* malformed `.oo/architects.json` is swallowed to an empty governance set;
* affiliation roots have no effect until ③ automatic admission + cap;
* `Ouroboros.peers` has no source-count cap;
* `random_below` silently returns zero on entropy failure;
* `#success` without `%result` is still recorded as an integrity incident;
* `advert_persistence` P3 compares object counts without a non-empty guard;
* `to_nlang` prints unforced Thunks as Rust `Debug`;
* `reader.read_line` is unbounded;
* `free_port()` is TOCTOU;
* `routing_id_from_digest` zero-pads.

None is permission to expand this delivery.
