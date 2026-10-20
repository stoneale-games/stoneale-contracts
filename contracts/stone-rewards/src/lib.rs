//! # STONE Rewards
//!
//! Pays players in STONE, a Stellar asset, when they complete in-game
//! wellness achievements (finishing a breathing session, a 7-day mood streak,
//! etc).
//!
//! STONE is issued as a classic Stellar asset so it shows up natively in
//! Freighter and every other Stellar wallet. This contract is set as the
//! admin of STONE's Stellar Asset Contract (SAC), which makes it the only
//! party able to mint new STONE. Minting is gated by:
//!
//! * a trusted **reporter** (the Stoneale game server) that must sign every
//!   `award` call,
//! * a one-time claim per `(player, achievement)` pair, and
//! * a per-player **daily cap** so a compromised reporter key cannot drain
//!   unlimited supply in a single day.
#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, panic_with_error, token,
    Address, BytesN, ContractExecutable, Env, Symbol,
};

const DAY_IN_SECONDS: u64 = 86_400;
const DAY_IN_LEDGERS: u32 = 17_280;
const PERSISTENT_BUMP: u32 = 90 * DAY_IN_LEDGERS;
const PERSISTENT_THRESHOLD: u32 = PERSISTENT_BUMP - DAY_IN_LEDGERS;
const TEMPORARY_TTL: u32 = 2 * DAY_IN_LEDGERS;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Reporter,
    Token,
    DailyCap,
    Claimed(Address, Symbol),
    Earned(Address),
    DayTotal(Address, u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    InvalidAmount = 1,
    AlreadyClaimed = 2,
    DailyCapExceeded = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub reporter: Address,
    pub token: Address,
    pub daily_cap: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Awarded {
    #[topic]
    pub player: Address,
    #[topic]
    pub achievement: Symbol,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigUpdated {
    pub config: Config,
}

#[contract]
pub struct StoneRewards;

#[contractimpl]
impl StoneRewards {
    /// * `admin` - can rotate the reporter, change the cap and upgrade.
    /// * `reporter` - game server key that signs every `award`.
    /// * `token` - address of the STONE Stellar Asset Contract. This
    ///   contract must be made the SAC admin before `award` can mint.
    /// * `daily_cap` - max STONE (in stroops) one player can earn per UTC day.
    pub fn __constructor(
        env: Env,
        admin: Address,
        reporter: Address,
        token: Address,
        daily_cap: i128,
    ) {
        if daily_cap <= 0 {
            panic_with_error!(&env, Error::InvalidAmount);
        }
        let s = env.storage().instance();
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::Reporter, &reporter);
        s.set(&DataKey::Token, &token);
        s.set(&DataKey::DailyCap, &daily_cap);
    }

    /// Mint `amount` STONE to `player` for completing `achievement`.
    /// Each achievement can only be rewarded once per player.
    pub fn award(
        env: Env,
        player: Address,
        achievement: Symbol,
        amount: i128,
    ) -> Result<(), Error> {
        let config = Self::config(env.clone());
        config.reporter.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let claimed_key = DataKey::Claimed(player.clone(), achievement.clone());
        if env.storage().persistent().has(&claimed_key) {
            return Err(Error::AlreadyClaimed);
        }

        let day = env.ledger().timestamp() / DAY_IN_SECONDS;
        let day_key = DataKey::DayTotal(player.clone(), day);
        let today: i128 = env.storage().temporary().get(&day_key).unwrap_or(0);
        let today = today.checked_add(amount).ok_or(Error::InvalidAmount)?;
        if today > config.daily_cap {
            return Err(Error::DailyCapExceeded);
        }

        let earned_key = DataKey::Earned(player.clone());
        let earned: i128 = env.storage().persistent().get(&earned_key).unwrap_or(0);

        env.storage().persistent().set(&claimed_key, &true);
        bump(&env, &claimed_key);
        env.storage()
            .persistent()
            .set(&earned_key, &(earned + amount));
        bump(&env, &earned_key);
        env.storage().temporary().set(&day_key, &today);
        env.storage()
            .temporary()
            .extend_ttl(&day_key, TEMPORARY_TTL, TEMPORARY_TTL);
        env.storage()
            .instance()
            .extend_ttl(PERSISTENT_THRESHOLD, PERSISTENT_BUMP);

        token::StellarAssetClient::new(&env, &config.token).mint(&player, &amount);

        Awarded {
            player,
            achievement,
            amount,
        }
        .publish(&env);
        Ok(())
    }

    pub fn is_claimed(env: Env, player: Address, achievement: Symbol) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Claimed(player, achievement))
    }

    /// Lifetime STONE earned through this contract.
    pub fn total_earned(env: Env, player: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Earned(player))
            .unwrap_or(0)
    }

    /// STONE earned during the current UTC day.
    pub fn earned_today(env: Env, player: Address) -> i128 {
        let day = env.ledger().timestamp() / DAY_IN_SECONDS;
        env.storage()
            .temporary()
            .get(&DataKey::DayTotal(player, day))
            .unwrap_or(0)
    }

    pub fn config(env: Env) -> Config {
        let s = env.storage().instance();
        Config {
            admin: s.get(&DataKey::Admin).unwrap(),
            reporter: s.get(&DataKey::Reporter).unwrap(),
            token: s.get(&DataKey::Token).unwrap(),
            daily_cap: s.get(&DataKey::DailyCap).unwrap(),
        }
    }

    pub fn set_reporter(env: Env, reporter: Address) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Reporter, &reporter);
        Self::emit_config(&env);
    }

    pub fn set_daily_cap(env: Env, daily_cap: i128) -> Result<(), Error> {
        Self::require_admin(&env);
        if daily_cap <= 0 {
            return Err(Error::InvalidAmount);
        }
        env.storage().instance().set(&DataKey::DailyCap, &daily_cap);
        Self::emit_config(&env);
        Ok(())
    }

    pub fn set_admin(env: Env, admin: Address) {
        Self::require_admin(&env);
        env.storage().instance().set(&DataKey::Admin, &admin);
        Self::emit_config(&env);
    }

    /// Hand STONE's SAC admin role to `new_admin` (e.g. when migrating to a
    /// new rewards contract). After this call `award` can no longer mint.
    pub fn release_token_admin(env: Env, new_admin: Address) {
        Self::require_admin(&env);
        let token: Address = env.storage().instance().get(&DataKey::Token).unwrap();
        token::StellarAssetClient::new(&env, &token).set_admin(&new_admin);
    }

    pub fn upgrade(env: Env, wasm_hash: BytesN<32>) {
        Self::require_admin(&env);
        env.deployer()
            .update_current_contract(ContractExecutable::Wasm(wasm_hash));
    }
}

impl StoneRewards {
    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
    }

    fn emit_config(env: &Env) {
        ConfigUpdated {
            config: Self::config(env.clone()),
        }
        .publish(env);
    }
}

fn bump(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_THRESHOLD, PERSISTENT_BUMP);
}

#[cfg(test)]
mod test;
