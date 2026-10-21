# Stoneale Contracts

**Soroban smart contracts for [Stoneale](https://stoneale.com): a play-to-learn wellness gaming platform built on Stellar.**

[![CI](https://github.com/stoneale-games/stoneale-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/stoneale-games/stoneale-contracts/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Stoneale rewards healthy habits like breathing sessions, mood check-ins and journaling streaks. This repo holds the on-chain side of those rewards. It runs on **Stellar** and is written as **Soroban** smart contracts in Rust.

| Contract | What it does |
|---|---|
| [`stone-rewards`](contracts/stone-rewards) | Mints **STONE**, a Stellar asset, to players when the game server reports a completed achievement. Each achievement can be claimed once, and each player has a daily cap. |
| [`wellness-badges`](contracts/wellness-badges) | **Soulbound** (non-transferable) NFTs for wellness milestones. Players can always burn their own badges. |

## 🚀 Live on Stellar testnet

| | Address |
|---|---|
| STONE asset (`STONE:GCPX…L5UI`) SAC | [`CCX55XV3JX5JCAK6PU4NVFH47ABA4KXRIKQCQHRVEXXCFWGHSQLTNCBG`](https://stellar.expert/explorer/testnet/contract/CCX55XV3JX5JCAK6PU4NVFH47ABA4KXRIKQCQHRVEXXCFWGHSQLTNCBG) |
| `stone-rewards` | [`CB5CGHND7BPEJZBUKN4AO3UHNHXGG2U2OJ6ASWERKMMW54KIUKXOC6QC`](https://stellar.expert/explorer/testnet/contract/CB5CGHND7BPEJZBUKN4AO3UHNHXGG2U2OJ6ASWERKMMW54KIUKXOC6QC) |
| `wellness-badges` | [`CBXEHWTFJDNWZNZESWLE67EODOBKSJG333F6ECPARPBXAV476J26RCOZ`](https://stellar.expert/explorer/testnet/contract/CBXEHWTFJDNWZNZESWLE67EODOBKSJG333F6ECPARPBXAV476J26RCOZ) |
| Admin / STONE issuer | [`GCPX2276HEZKHLIPMC553SLBB3UJCQ377IU2DRICTSLFNDCXBOCWL5UI`](https://stellar.expert/explorer/testnet/account/GCPX2276HEZKHLIPMC553SLBB3UJCQ377IU2DRICTSLFNDCXBOCWL5UI) |

`stone-rewards` is the admin of the STONE SAC. These values are also in [`deployments/testnet.env`](deployments/testnet.env), ready to drop into the Frontend's `.env`.

Smoke-tested on 2026-10-21. A demo player received 50 STONE through `award`, `total_earned` returned the same amount, a badge was minted, and a duplicate claim was rejected with `AlreadyClaimed (#2)`.

## How it fits together

```
 Player (Freighter wallet)
        │  plays a session in the Stoneale web app
        ▼
 Stoneale game server ── signs ──► stone-rewards.award(player, achievement, amount)
        │                                   │ (rewards contract is STONE's SAC admin)
        │                                   ▼
        │                          STONE Stellar Asset Contract ──► mint to player
        │
        └──── signs ──► wellness-badges.mint(minter, player, badge_type)
```

* **Why a Stellar asset for STONE?** A classic asset wrapped by the Stellar Asset Contract (SAC) works in every Stellar wallet, DEX and anchor from day one. We only write custom code where we need custom rules: who can mint, and how much.
* **Why soulbound badges?** A badge describes a person's progress. Making it tradeable would reward speculation instead of the habit.
* **Privacy first.** Contracts store only achievement IDs and badge types, never mood data or journal content. Players can burn badges to remove them from their public profile.

## Contract reference

### `stone-rewards`

| Function | Auth | Description |
|---|---|---|
| `__constructor(admin, reporter, token, daily_cap)` | n/a | `token` is the STONE SAC address. `daily_cap` is in stroops (7 decimals). |
| `award(player, achievement: Symbol, amount)` | reporter | Mints STONE. Errors: `InvalidAmount`, `AlreadyClaimed`, `DailyCapExceeded`. |
| `is_claimed`, `total_earned`, `earned_today`, `config` | none | Read-only views. |
| `set_reporter`, `set_daily_cap`, `set_admin`, `upgrade` | admin | Admin operations. |
| `release_token_admin(new_admin)` | admin | Hands STONE's SAC admin role to another address, for migration. |

### `wellness-badges`

| Function | Auth | Description |
|---|---|---|
| `__constructor(admin, name, symbol, base_uri)` | n/a | Metadata lives at `{base_uri}{badge_type}.json`. |
| `mint(minter, to, badge_type: u32) -> u32` | approved minter | One badge per type per player. |
| `burn(owner, token_id)` | owner | Permanently removes the badge. |
| `badge`, `owner_of`, `badges_of`, `balance`, `has_badge`, `is_minter` | none | Read-only views. |
| `set_minter`, `set_base_uri`, `set_admin`, `upgrade` | admin | Admin operations. |

There is deliberately no `transfer`.

## Getting started

Prerequisites: [Rust](https://rustup.rs) (stable) and the [Stellar CLI](https://developers.stellar.org/docs/tools/cli).

```bash
rustup target add wasm32v1-none
cargo test                 # run unit tests
stellar contract build     # build optimised WASM into target/wasm32v1-none/release
```

### Deploy to testnet

```bash
stellar keys generate stoneale-admin --network testnet --fund
ADMIN=$(stellar keys address stoneale-admin)

# 1. Issue STONE and deploy its Stellar Asset Contract
stellar contract asset deploy --asset STONE:$ADMIN --source stoneale-admin --network testnet
STONE=$(stellar contract id asset --asset STONE:$ADMIN --network testnet)

# 2. Deploy the rewards contract (1,000 STONE daily cap)
REWARDS=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/stone_rewards.wasm \
  --source stoneale-admin --network testnet -- \
  --admin $ADMIN --reporter $ADMIN --token $STONE --daily_cap 10000000000)

# 3. Make the rewards contract the only STONE minter
stellar contract invoke --id $STONE --source stoneale-admin --network testnet -- \
  set_admin --new_admin $REWARDS

# 4. Deploy badges
stellar contract deploy \
  --wasm target/wasm32v1-none/release/wellness_badges.wasm \
  --source stoneale-admin --network testnet -- \
  --admin $ADMIN --name "Stoneale Wellness Badges" --symbol STNB \
  --base_uri "https://stoneale.com/badges/"
```

> Players need a STONE **trustline** before they can receive STONE. The web app adds one through Freighter.

## Roadmap

- [x] **Phase 1, foundations:** STONE rewards contract, soulbound badges, CI, unit tests.
- [ ] **Phase 2, testnet launch:** ~~deploy to testnet~~ (done, see above), connect the [Frontend](https://github.com/stoneale-games/Frontend) through Freighter, and have the game server sign `award` calls.
- [ ] **Phase 3, player-owned data:** opt-in, encrypted wellness-progress proofs that players can share selectively with therapists or programs.
- [ ] **Phase 4, impact ledger:** on-chain, aggregated, anonymised impact metrics that sponsors and NGOs can verify.
- [ ] **Phase 5, mainnet:** security audit, multisig admin, mainnet deployment.

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and look for issues labelled `good first issue` or by complexity (`complexity: trivial`, `complexity: medium` and `complexity: high`).

## License

[Apache-2.0](LICENSE) © Stoneale
