# Contributing to Stoneale Contracts

Thanks for helping build Stoneale on Stellar!

## Picking an issue

1. Browse [open issues](https://github.com/stoneale-games/stoneale-contracts/issues). Each one has a complexity label:
   * `complexity: trivial`: docs, small refactors or a single test. Good for a first contribution.
   * `complexity: medium`: a new function or feature with tests.
   * `complexity: high`: a new contract, cross-contract work or anything security-sensitive.
2. Comment on the issue to ask for it to be assigned to you **before** you start, so two people don't do the same work.
3. If you've had no activity on an assigned issue for 7 days, it may be reassigned.

## Development setup

```bash
rustup target add wasm32v1-none
cargo install --locked stellar-cli   # required to build WASM and deploy
cargo test
```

## Before opening a PR

CI runs these checks, so run them locally first:

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
stellar contract build
```

## Guidelines

* **Test every behavior.** New functions need unit tests for both the success path and every error path. Auth checks must be tested, either by asserting `env.auths()` or with `mock_auths`.
* **Never store personal wellness data on-chain.** Store IDs, counts and types only, never mood scores, journal text or anything health-related that could identify someone.
* **Use typed errors.** Add variants to the contract's `Error` enum instead of panicking with strings.
* **Emit events** for every state change that an indexer or the frontend might care about.
* **Manage storage TTLs.** Extend TTLs on writes, and use temporary storage for data that expires naturally.
* Keep PRs focused: one issue per PR. Reference it in the description (`Closes #12`).
* Use [Conventional Commits](https://www.conventionalcommits.org): `feat(rewards): ...`, `fix(badges): ...`, `docs: ...`.

## Security

If you find a vulnerability, **do not open a public issue**. Email **[security@stoneale.com](mailto:security@stoneale.com)** instead.

## License

By contributing, you agree that your contributions will be licensed under [Apache-2.0](LICENSE).
