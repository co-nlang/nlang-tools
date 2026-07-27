# Work order — the operator gets a name

Arc opened 2026-07-27, after v0.2.45.
Acceptor: project brain. Implementer: model #3.
Expected classification: **增量** if the tree-wide scan below stays clean; the
one language-surface removal is judged by the v0.2.42 `/add_architect` precedent
at acceptance, not assumed here.

This is step **2 of 3** of the cold-start sequence that v0.2.45 opened:

```
A  identity out of the universe root          ← v0.2.45, done
B  a stable identity the operator can declare ← THIS ARC
C  operator declaration: REAL_01 §7.2 privilege tokens, lifecycle, CRL
```

---

## 0. The headline, measured on v0.2.45

Three processes, **one** workspace, three signatures:

```
91f3ff6e…   7b2cf863…   10132278…
```

`Identity::new_random()` at every `Ouroboros::init` (`lib.rs:372/395`), written
nowhere. A committed repository's `.oo/` holds `HEAD` and `objects` and **no key
material of any kind**. The signer of a `#refine` is a party that exists for the
duration of one process and is never seen again.

### The door with no key

v0.2.45 removed the engine's self-appointment, which was correct, and thereby
revealed that the honest configuration is **unreachable**. Measured on a
repository with a HEAD:

```
.oo/architects.json = ["<pubkey observed from process 1>"]

  oo refine --sign …   →  Error: signer 9cb94f64… not in architect_registry
  oo refine …          →  Error: missing %authority on non-bootstrap refine
```

**Both directions refused.** There is no value you can write into that file that
this engine will ever present, because it presents a different one every time.

So SPEC_10 §93's 權威判定 branch has never once been satisfied: before v0.2.45
self-appointment made it always true; after v0.2.45 having no key makes it always
false. The only reachable configuration is the empty whitelist, where everything
is `unverified`.

> A whitelist that cannot be satisfied is not a stricter check than one that
> cannot fail. It is the same check.

### The builtin that persistence would arm

`~%Official./sign_refine` is reachable from an ordinary n/ program with **no
privilege flag**, and returns a real Ed25519 signature by the engine's key,
effect `#io`. Measured: three plain `oo eval` runs, three signatures, no grant.

Harmless today only because the key is worthless. `~%Official` has exactly one
key, and the language has **no interface that constructs a refine commit** — so
the builtin's only possible use is to hand the signature to the program, which
can write it to a file or send it over a socket. The moment the key is stable and
declared, that reads: *any n/ program, including one fetched from a peer, can
obtain an architect's signature authorising an arbitrary CAID → CAID redirect.*

## 1. What the spec says, and the one thing it does not

* **ORDER_01 §7.1**: `~%Governance.@Voter` = `{{ pubkey: b"", weight: @int & 1.., alias: @str? }}`; the trust root `~%Official.architects` is a **set of Voters**. Weights and human aliases: that is a **person**. Not a process, and not a workspace.
* **SPEC_10 §93** (v0.2.45): the whitelist 「**必須**經帶外通道供給」 and the engine 「**不得**自行鑄入任何本地金鑰」.
* **REAL_01 §7.2**: `~/.oo/authorized_keys` is the public-key whitelist for **privilege tokens** — a different mechanism, entirely unimplemented, and step C. The engine reads nothing under `$HOME` at all (measured: no `dirs::home_dir`, no `HOME`, no `authorized_keys`, no CRL).

**The spec never says where the engine's own private key lives.** That is the
gap this arc's spec closure fills.

### Where a secret lives

Discussion 025 split `.oo/` into self-authenticating **objects** (verify, no
permissions) and asserted **pointers** (authenticate, nothing to verify against).
A private key is neither. What it needs is **concealment**, and concealment is
the one property the CAID/lattice framework has nothing to say about.

So the question is not which subdirectory of `.oo/` — it is whether the key
belongs in that tree at all, and it does not. `.oo/` is the thing that gets
**served to peers and copied with the repository**. Relying on "the serve path
happens not to read that file" is exactly the reasoning v0.2.44 punished when
`remote_fetch` never recomputed an address.

### Why minting a key is allowed when appointing was not

v0.2.45 forbade the engine from minting **authority**. A keypair is a **name**,
and names are self-minted in n/ — a CAID needs nobody's permission either.
Authority arrives by **declaration**, and declaration is always out of band.

> The engine may make names. It may not make declarations.

Same degree separation as v0.2.45, one layer up.

## 2. Rulings

### Q1 — the identity is the **operator's**, at `~/.oo/identity`

Overridable by the environment variable **`OO_IDENTITY`** (absolute path to the
key file) for compartmentalisation and for tests. Rejected: `.oo/identity`,
because a secret must not live inside a shareable artifact, and because
per-workspace identity makes the same human a different signer in every
repository — the per-process failure mode at a slower rate.

### Q2 — mint on first need, persist, print the public key

No `oo identity init` ceremony. `--sign` is already the operator asking. The
minted key is **never** self-inserted into any whitelist (that is v0.2.45's
ruling and it is not reopened).

### Q3 — `~%Official./sign_refine` is retired from the language surface

Follows the v0.2.42 `/add_architect` precedent: the language surface must not own
the refine trust root. Rejected: gating it behind a §6.1.4 capability, because
v0.2.40's lesson (意圖≠授權; authorization must be re-presented through a trusted
channel *at the moment the privileged effect is applied*) is especially sharp
here — `--grant` is intent at process start, the signature happens at an
arbitrary point during evaluation, and there is no trusted channel between them.

**`~%Official` stays mounted and becomes empty.** That is the honest cold-start
shape: the spec names the trust root `~%Official.architects` and nobody has
declared one, so `~%Official.architects → ⊥ #missing_key` is the true answer.
Do **not** leave a dead morphism that always returns ⊥ — that is precisely what
`/add_architect` was retired for.

## 3. Scope

### D1 — persistent operator identity

* Resolution order: `OO_IDENTITY` (absolute path) → `~/.oo/identity`.
* File content: the **PKCS#8 DER bytes**, nothing else. The public key is
  *derived* on load (`Ed25519KeyPair::from_pkcs8`), never stored separately —
  a stored pair can disagree with itself.
* Created with mode **0600**; its parent directory created **0700**.
* **Lazy**: minted only when a signature is actually needed. `oo run` / `status`
  / `evolve` / `commit` / `log` must not bring a key into existence (P5).
  `Ouroboros::init` therefore must not mint eagerly — `identity` becomes a
  lazily-initialised accessor, not an eagerly-filled field.
* `Ouroboros::new_in_memory()` must **never** read or write the operator path.
  An in-memory engine is not an operator session; it keeps an ephemeral key.

### D2 — an unreadable file is refused, never replaced

If the identity file exists but does not parse as PKCS#8, every operation that
needs it **fails with an error naming the identity path**, and the file's bytes
are left untouched. Silently minting over an operator's key is unrecoverable and
would turn "my signatures stopped verifying" into a mystery.

### D3 — the identity path is off the language surface

`fs_guard` must refuse the **resolved identity path** (from `OO_IDENTITY` or the
default) to every filesystem-touching builtin, in addition to the existing `.oo`
component rule. The default path already has a `.oo` component and is covered by
accident; **protection must not be an accident of the name.** Siblings in the
same directory stay readable — the refusal is about the path, not the directory.

### D4 — `oo identity`

Prints the operator's public key (64 hex) and the path it lives at. Minting if
absent (Q2). Keep it minimal: no rotation, no import, no export of the private
key in any form.

### D5 — retire `~%Official./sign_refine`

Remove the language-surface mount (`lib.rs:1075`) and the builtin
(`builtins/engine.rs:267–297`). The Rust function `authority::sign_refine` stays
— it is what `oo refine --sign` calls, and after this arc **`oo refine --sign` is
the only consumer of the private key in the entire engine.** Say that in the code
comment; it is the property that makes the surface auditable.

Tree-wide grep for the retired spelling was run at work-order time. Live call
sites: `lib.rs:1075`, `builtins/engine.rs:267/286/296`, plus the three probe
controls the **acceptor** has already rewritten (see §4). `crates/interpreter/
tests/authority_test.rs` and `refine_test.rs` call the **Rust** function and stay,
but will need updating for the lazy accessor. `docs/implementation-status.md:178`
lists the morphism and must be corrected. `docs/worknotes/phase-*.md` are
historical records — **leave them alone.**

### D6 — an empty closed combo must render as re-parseable source

**Surfaced by D5, not caused by it.** Measured on v0.2.45:

```
oo eval '{{ }}'   →  {{ }        ← one brace short
oo eval '{{ }'    →  Parse error: expected field
```

`oo fmt` is unaffected (source round-trips), so this is the **value renderer**
and fmt v2's freeze is not in question. It is in scope because D5 makes
`~%Official` the first empty closed combo mounted in the system root, so
`oo eval ~%Official` — this arc's own headline artifact — would print invalid
n/ source. R8 requires only that the rendered form parse back and be
idempotent; `{{ }}` vs `{{}}` is delivery's choice.

### Out of scope, deliberately

* REAL_01 §7.2 privilege tokens / lifecycle / CRL — that is step C.
* A node identity for peers (non-repudiation of what a node served). Peers
  authenticate nothing today; that is a separate arc and it needs D1 first.
* Key **rotation** — it interacts with existing signatures and with whitelists
  that name the old key. Not now.
* Migrating existing universes whose roots still carry `architects`.
* Restoring `~%Official.architects` as a readable view of the provisioned
  whitelist. It is tempting (it would give `~%Official` a member again and the
  spec already owns the name) but it must not become a **data field**, or the
  root CAID starts depending on local configuration and v0.2.45 §4.1.2
  obligation #3 breaks. Deferred, with that constraint recorded.

## 4. Probes — pre-committed, do not modify

`crates/oo/tests/identity_persistence_probe_test.rs`, calibrated at baseline:
**8 red (`#[ignore]`), 6 green.** Delivery removes **only** the `#[ignore]`
attributes. Every invocation pins `OO_IDENTITY`, so the suite never touches the
developer's real `~/.oo/`.

| | gate | baseline failure |
| :-- | :--- | :--- |
| R1 | identity stable across processes | `d6e90822…` vs `12ad13ec…` in one workspace |
| R2 | persisted at the operator path, 0600, no copy in `.oo/` | `signing did not persist an identity` |
| R3 | a provisioned whitelist can finally verify **(paired: a foreign key must still refuse)** | `signer … not in architect_registry` |
| R4 | `/sign_refine` off the language surface **(paired: `~%Config.fuel` + `~%Official` mounted)** | a real `signature_hex` came back |
| R5 | identity file unreadable from n/ **even outside any `.oo` component** | `the language layer can read the operator's private key` |
| R6 | `oo identity` prints the key signing actually uses **(paired against the commit object)** | `unrecognized subcommand 'identity'` |
| R7 | corrupt identity refused, bytes unchanged | `a corrupt identity was stepped over and something else signed` |
| R8 | empty closed combo re-parses **(paired: non-empty already does)** | `the rendered form does not parse back: "{{ }"` |

| | pin | must stay green |
| :-- | :--- | :--- |
| P1 | root CAID independent of process, workspace **and identity** | v0.2.45 §4.1.2 #1 |
| P2 | empty whitelist + `--sign` → `unverified` | SPEC_10 §93 |
| P3 | empty whitelist, unsigned → accepted, `unverified` | honest cold start |
| P4 | store boundary still refuses `.oo` paths | SPEC_08 §6.3 |
| P5 | ordinary work mints no identity | laziness (D1) |
| P6 | `~%Engine` still resolves calls | retirement blast radius |

**Note on controls — read this before assuming a red is yours.** Three existing
probe controls named `/sign_refine` and would have become false reds for a
delivery that was *asked* to retire it. The acceptor rewrote them, in this same
commit:

* `universe_determinism_probe_test.rs` — control in `red_engine_does_not_mint_a_governance_root`, and `pin_official_module_still_signs` → `pin_official_module_stays_mounted`
* `store_boundary_probe_test.rs` — control in `red_add_architect_is_off_the_language_surface`

All three now assert `~%Official` contains `{{`. **Not** "is not bottom":
measured, a module removed from the (open) system root evaluates to `_`, not
`_|_`, so "not bottom" would pass for a module that had vanished and would not be
a control at all. Both suites re-run green after the rewrite (20/20 and 12/12).

## 5. Acceptance

1. Diff purity — only files this order names.
2. Four numbers: workspace suite, `identity_persistence` 14/14,
   `universe_determinism` 12/12, `store_boundary` 20/20, conformance 143/143,
   genesis 11/11.
3. Adversarial pass, at minimum: an identity path that is a directory; a
   read-only parent; `OO_IDENTITY` set to a relative path; two concurrent
   processes minting at once (the file must not end up half-written or
   inconsistent between them); a zero-byte identity file.
4. A/B against v0.2.45 — the seven reds must be red on the previous binary for
   the same reasons recorded above.
5. Tree-wide grep for `sign_refine` re-run over probes, corpus and conformance;
   classification decided from what it finds, not assumed.

## 6. Delivery record (delivery side)

Fill in below: files touched, the four numbers, anything refused and why.
If a probe looks wrong, **say so instead of accommodating it** — that has
happened twice and both times the probe was the defect.
