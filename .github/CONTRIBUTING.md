# Contributing to solid-data-governance-rs

This is Open Source software published by **GENEFOLD AI LTD** under the
Apache License 2.0.

---

## Volunteer basis

Contributions are accepted strictly on a **voluntary basis**.
No contributor may raise any claim — financial, intellectual-property, or
otherwise — arising from work contributed to this repository.

---

## Licence assignment

By submitting a contribution you confirm that:

1. You have the right to submit the work under the Apache License 2.0.
2. You grant **GENEFOLD AI LTD** and all downstream recipients a perpetual,
   worldwide, royalty-free, sublicensable licence to use, reproduce, modify,
   and distribute your contribution under the terms of the
   [Apache License 2.0](../LICENSE).
3. You understand that **no compensation** is due for your contribution.
4. Your contribution does not knowingly infringe any third-party patent,
   copyright, trade secret, or other proprietary right.

---

## How to contribute

1. Fork the repository and create a feature branch (`git checkout -b feat/my-change`).
2. Run the pre-push checks before opening a PR:
   ```bash
   cargo fmt --all
   cargo clippy --all-targets -- -D warnings
   cargo test --all
   ```
3. Open a pull request against `main`. The PR template will prompt you to
   confirm the contributor declaration above.

---

## Code of conduct

Be respectful and constructive. Contributions that are abusive, harassing,
or submitted in bad faith will be closed without comment.

---

## Reporting issues

Please open a GitHub Issue. Include:
- Rust / Cargo version (`rustc --version`, `cargo --version`)
- Steps to reproduce
- Expected vs actual behaviour
- Relevant log output (`RUST_LOG=debug ...`)
