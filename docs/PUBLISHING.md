# Publishing to crates.io

How to turn this repository into a crate that anyone can add with `cargo add`, and how to
keep publishing it after the first release. Everything below was checked against this
working tree on 2026-09-22 with cargo 1.97.1.

The workspace ships **two** packages and both have to be published:

| Package | Path | Why it is published |
|---|---|---|
| `archunit-macros` | `archunit-macros/` | proc macros (`#[analyze_classes]`, `#[arch_test]`, …), re-exported from `archunit::harness` |
| `archunit` | `.` (repository root) | the library itself; depends on `archunit-macros` |

Users only ever add one dependency; the macro crate is an implementation detail, but
crates.io still needs it published because a published crate may not contain path
dependencies.

---

## 1. Settle the published name first

`docs/PLAN.md` says the package is `archunit`, with `archunit-rs` as the fallback if the
name is taken. **It is taken.** As of 2026-09-22 crates.io serves:

```
archunit 0.0.1 — "Architecture testing for Rust"
owner: LukasNiessen — https://github.com/LukasNiessen/ArchUnitRust
```

published 2026-09-20. Names on crates.io are first-come and are not reassigned except
through the [name-dispute process](https://doc.rust-lang.org/cargo/reference/publishing.html),
which is for squatting, not for a name that an active project is using. Check the current
state before doing anything else:

```sh
curl -s -A "you@example.com" https://crates.io/api/v1/crates/archunit | head -c 200
# {"errors":[{"detail":"crate `archunit` does not exist"}]}   → free
```

`archunit-rs` and `archunit-macros` were both free at the same moment.

### Publishing as `archunit-rs` without touching a line of code

The package name and the library name are separate. Keep `archunit` as the *library* name
so every `use archunit::prelude::*;` in the README, the examples and the docs stays
correct, and only change what crates.io sees:

```toml
[package]
name = "archunit-rs"          # what crates.io stores; `cargo add archunit-rs`

[lib]
name = "archunit"             # what users type: `use archunit::…`
path = "src/lib.rs"
```

Consumers then write either of these, and both give them `archunit::`:

```toml
[dev-dependencies]
archunit-rs = "0.1"
# or, to be explicit about the rename:
archunit = { package = "archunit-rs", version = "0.1" }
```

The only edits needed for the rename are: `[package] name` plus the `[lib]` block in the
root `Cargo.toml`, the install snippet in `README.md` (line 77), the package-name decision
bullet in `docs/PLAN.md` (§ Decisions), and the `archunit-macros` dependency line if that
crate is renamed too. `archunit-macros` is free, so it needs no rename — but if you would
rather keep the pair symmetrical, publish `archunit-rs-macros` and point the dependency at
it with `archunit-macros = { package = "archunit-rs-macros", … }`, which keeps the
generated code in the proc macros working unchanged.

Decide this before the first publish. Renaming after release means publishing a new crate
and leaving the old name as a stub that nobody can delete.

---

## 2. Accounts and tokens (one-time)

1. Sign in at <https://crates.io> with GitHub and confirm the verification email —
   crates.io refuses to publish until the address is verified.
2. Turn on 2FA for the GitHub account that owns the crate; crates.io inherits that login.
3. Create an API token under <https://crates.io/settings/tokens> with the narrowest scope
   that works: `publish-new` for the very first release of each package, `publish-update`
   afterwards. Give it an expiry.
4. Store it:

   ```sh
   cargo login                      # prompts, writes ~/.cargo/credentials.toml
   # or, for one-shot / CI use:
   export CARGO_REGISTRY_TOKEN=cio...
   ```

   Never commit the token; `~/.cargo/credentials.toml` is outside this repository for a
   reason.

For CI, skip long-lived tokens entirely — see §8.

---

## 3. Manifest readiness

Both manifests already carry the fields crates.io requires (`name`, `version`,
`description`, `license`, plus `repository` and `readme`). Worth adding before the first
publish:

```toml
[package]
# … existing fields …
homepage = "https://github.com/cwoodruff/ArchUnitRust"
documentation = "https://docs.rs/archunit-rs"      # optional; docs.rs is the default anyway
authors = ["Chris Woodruff <christopherlwoodruff@gmail.com>"]
exclude = ["tests/", "examples/expected/", "examples/frozen/", ".idea/"]

[package.metadata.docs.rs]
all-features = true
rustdoc-args = ["--cfg", "docsrs"]
```

Notes on the specific values:

* **`keywords`** — crates.io allows at most 5, each ≤ 20 characters, alphanumeric or `-`.
  The current five (`architecture`, `testing`, `archunit`, `dependencies`, `layers`) are
  valid as-is.
* **`categories`** — must match the [published slug list](https://crates.io/category_slugs)
  exactly; `development-tools::testing` is valid. Up to 5.
* **`rust-version = "1.85"`** is right for edition 2024 and makes crates.io and `cargo add`
  warn users on older toolchains. Treat raising it as a minor-version bump.
* **`archunit-macros`** is depended on as `{ path = "archunit-macros", version = "0.1.0" }`.
  Keep **both** keys: cargo strips the path on publish and the `version` is what the
  published crate resolves against. The two versions must move together.
* **`exclude`** is optional — see §4 for what it changes.
* The `archunit-macros` manifest has no `readme`; add `readme = "../README.md"` or a short
  one of its own, otherwise its crates.io page is bare.

---

## 4. What actually ends up in the `.crate` file

Inspect before uploading — this is the single most useful pre-publish command:

```sh
cargo package -p archunit --list
```

Today that is 191 files, 1.2 MiB (247.8 KiB compressed), well under the 10 MiB limit,
broken down as:

```
69  examples/expected/…      golden reports for the ported archunit-example rules
21  tests/expected/…         golden failure reports
 2  tests/fixtures/…         only the two data files; see below
51  src/…
 …  README.md, LICENSE, NOTICE, Cargo.toml, Cargo.lock
```

The fixture crates under `tests/fixtures/` are **automatically left out**: each has its own
`Cargo.toml`, and cargo never packages a nested package. Only `tests/fixtures/config/archunit.toml`
and `tests/fixtures/plantuml/layered_app.puml` survive, because they are plain data.

That means the integration tests in the published `.crate` cannot run — their fixtures are
missing. This does not affect users (nobody runs a dependency's tests) and does not affect
docs.rs (it builds the library only), but it is a reason to add the `exclude` from §3 and
ship only `src/`, the README, the licences and the manifest. Excluding `tests/` and
`examples/` cuts the package to roughly a fifth of its size and removes 90 files that can
only fail if someone tries to build them from the registry tarball.

Verify the package builds from the tarball, exactly as crates.io will:

```sh
cargo package --workspace
```

This packages both crates, unpacks `archunit-macros` into a temporary registry and compiles
`archunit` against it — the same trick that lets the two crates be published together. It
currently succeeds.

---

## 5. Release checklist

Run from a clean tree on `main`:

```sh
# 1. the three gates from CLAUDE.md
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --examples

# 2. docs build without warnings (docs.rs will run this)
cargo doc --no-deps --all-features

# 3. set the version in BOTH manifests, in lockstep
#    Cargo.toml: version = "0.1.0"
#    Cargo.toml: archunit-macros = { path = "…", version = "0.1.0" }
#    archunit-macros/Cargo.toml: version = "0.1.0"
cargo update -w            # refresh Cargo.lock with the new versions
git commit -am "chore(release): 0.1.0"

# 4. inspect and verify the tarballs
cargo package -p archunit --list
cargo package --workspace

# 5. rehearse the upload
cargo publish --workspace --dry-run

# 6. publish for real
cargo publish --workspace

# 7. tag
git tag -a v0.1.0 -m "archunit 0.1.0"
git push origin main --tags
```

`cargo publish --workspace` (stable since cargo 1.90) works out the order itself: it
uploads `archunit-macros`, waits for the index, then uploads `archunit`. If you prefer to
do it by hand, the order is mandatory and the wait is real:

```sh
cargo publish -p archunit-macros
# wait until https://crates.io/crates/archunit-macros shows the version
cargo publish -p archunit
```

A publish is **permanent**. Versions cannot be deleted, only yanked, and a version number
can never be reused. Rehearse with `--dry-run`.

---

## 6. After the first publish

* **docs.rs** builds automatically within minutes. Watch
  `https://docs.rs/crate/<name>/<version>/builds` — a failed docs build is invisible on
  crates.io but is the first thing users hit. It builds with `--all-features` only if
  `[package.metadata.docs.rs] all-features = true` is set (§3).
* **Smoke-test as a consumer**, from a scratch directory outside this repository:

  ```sh
  cargo new /tmp/archunit-smoke && cd /tmp/archunit-smoke
  cargo add --dev archunit-rs
  # paste the README "Getting started" test, then:
  cargo test
  ```

  This catches the classic first-release bug: an item used in the README or in macro-generated
  code that is not actually `pub` from the crate root.
* **Add the owners** who should be able to publish:

  ```sh
  cargo owner --add <github-user>
  cargo owner --add github:<org>:<team>
  ```
* **GitHub release** — create one from the tag with the changelog section for that version.
* **Badges in the README** are conventional and cheap:

  ```markdown
  [![crates.io](https://img.shields.io/crates/v/archunit-rs.svg)](https://crates.io/crates/archunit-rs)
  [![docs.rs](https://docs.rs/archunit-rs/badge.svg)](https://docs.rs/archunit-rs)
  ```

---

## 7. Versioning, changelog, fixing mistakes

* Follow SemVer as cargo interprets it: pre-1.0, `0.x.y` → `0.(x+1).0` is the breaking bump
  and `0.x.(y+1)` is compatible. The fluent DSL makes almost any signature change
  user-visible, so expect early breakage and stay on `0.x` until the API has settled
  against real crates.
* Keep a `CHANGELOG.md` (Keep a Changelog format). Because this crate promises parity with
  ArchUnit, note in each entry which ArchUnit APIs and report formats the release covers —
  the status column in `docs/MAPPING.md` is the source for that.
* Check for accidental breakage before releasing:

  ```sh
  cargo install cargo-semver-checks
  cargo semver-checks check-release
  ```
* Consider `cargo install cargo-release` once the cadence is regular; it does the
  version bump in both manifests, the commit, the tag and the publish in one step.
* **Yanking** stops new dependents from picking a version up; it does not remove it and
  does not break existing `Cargo.lock` files:

  ```sh
  cargo yank --version 0.1.0 archunit-rs
  cargo yank --version 0.1.0 --undo archunit-rs
  ```

  A broken release is fixed by yanking it *and* publishing `0.1.1` — never by trying to
  replace the tarball.
* If a token or secret is ever published inside a `.crate`, yank first, then rotate the
  secret, then mail <help@crates.io>.

---

## 8. Automating it

### CI on every push (there is no `.github/` in this repository yet)

```yaml
# .github/workflows/ci.yml
name: ci
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo test --all-features
      - run: cargo test --examples
      - run: cargo package --workspace
  msrv:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.85.0
      - run: cargo check --all-features
```

The `cargo package --workspace` step is what keeps the crate publishable: it fails the
moment someone adds a path dependency without a version, or a file the tarball needs.

### Publish on tag, with Trusted Publishing

crates.io supports GitHub OIDC, so no token has to live in repository secrets. Configure it
once per crate under *Settings → Trusted Publishing* on the crates.io page (repository
`cwoodruff/ArchUnitRust`, workflow `release.yml`), for **both** packages, then:

```yaml
# .github/workflows/release.yml
name: release
on:
  push:
    tags: ["v*"]
permissions:
  contents: read
  id-token: write          # required for the OIDC exchange
jobs:
  publish:
    runs-on: ubuntu-latest
    environment: release
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: rust-lang/crates-io-auth-action@v1
        id: auth
      - run: cargo publish --workspace
        env:
          CARGO_REGISTRY_TOKEN: ${{ steps.auth.outputs.token }}
```

The first release of each package still has to be done from a laptop with a `publish-new`
token, because Trusted Publishing can only be configured on a crate that already exists.

---

## 9. Attribution obligations to carry into the release

This crate is a port of an Apache-2.0 project and deliberately reproduces ArchUnit's names,
rule descriptions and report wording. Two things must stay true in every published tarball:

* `LICENSE` (Apache-2.0) and `NOTICE` are included — they are, by default, because both sit
  at the package root and no `exclude` entry touches them. If you add an `exclude` list,
  re-run `cargo package --list` and confirm both are still there.
* `NOTICE` keeps naming TNG Technology Consulting GmbH and stating that no Java source is
  included. The crates.io description and README should keep calling the crate a port
  rather than implying an affiliation with the ArchUnit project.

`license = "Apache-2.0"` in both manifests is what crates.io displays; it is already set.

---

## 10. Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `error: no matching package named 'archunit-macros' found` during publish | the macro crate is not on the index yet — publish it first, or use `cargo publish --workspace` |
| `error: all dependencies must have a version specified when publishing` | a `path` dependency lost its `version` key |
| `the name 'archunit' is already in use` | see §1 |
| `error: 1 files in the working directory contain changes…` | commit first, or `--allow-dirty` (prefer committing — the tarball should match a tag) |
| docs.rs build failed but crates.io is fine | read the build log at `docs.rs/crate/<name>/<version>/builds`; usually a feature or a `cfg` that only exists locally |
| package is over 10 MiB | add `exclude` (§3); check that no fixture `target/` directory is tracked |
| integration tests fail when run from the published tarball | expected — the fixture crates are not packaged (§4) |
