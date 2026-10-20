//! # Wellness Badges
//!
//! Soulbound (non-transferable) NFTs that prove a player reached a wellness
//! milestone in Stoneale, e.g. "30 days of mood check-ins". Badges replace the
//! earlier EVM "winning hand" NFT minter.
//!
//! * Badges cannot be transferred or sold: they describe the player, not an
//!   asset. That keeps the incentive on the habit rather than on speculation.
//! * Only approved **minters** (the Stoneale game server, or another
//!   contract) can issue badges, and a player holds at most one badge per
//!   badge type.
//! * Players can always **burn** their own badges. Because this is
//!   wellness data, users must be able to remove it from their public profile.
#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN,
    ContractExecutable, Env, String, Vec,
};

const DAY_IN_LEDGERS: u32 = 17_280;
const BUMP: u32 = 120 * DAY_IN_LEDGERS;
const THRESHOLD: u32 = BUMP - 7 * DAY_IN_LEDGERS;

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Name,
    Symbol,
    BaseUri,
    NextId,
    Minter(Address),
    Badge(u32),
    Owned(Address),
    HasType(Address, u32),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    NotMinter = 1,
    AlreadyHasBadge = 2,
    NotFound = 3,
    NotOwner = 4,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Badge {
    pub owner: Address,
    pub badge_type: u32,
    pub minted_at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Minted {
    #[topic]
    pub to: Address,
    #[topic]
    pub badge_type: u32,
    pub token_id: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Burned {
    #[topic]
    pub owner: Address,
    pub token_id: u32,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinterSet {
    #[topic]
    pub minter: Address,
    pub enabled: bool,
}

#[contract]
pub struct WellnessBadges;

#[contractimpl]
impl WellnessBadges {
    pub fn __constructor(env: Env, admin: Address, name: String, symbol: String, base_uri: String) {
        let s = env.storage().instance();
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::Name, &name);
        s.set(&DataKey::Symbol, &symbol);
        s.set(&DataKey::BaseUri, &base_uri);
        s.set(&DataKey::NextId, &1u32);
    }

    /// Issue a badge of `badge_type` to `to`. Returns the new token id.
    pub fn mint(env: Env, minter: Address, to: Address, badge_type: u32) -> Result<u32, Error> {
        minter.require_auth();
        if !Self::is_minter(env.clone(), minter) {
            return Err(Error::NotMinter);
        }
        let has_key = DataKey::HasType(to.clone(), badge_type);
        if env.storage().persistent().has(&has_key) {
            return Err(Error::AlreadyHasBadge);
        }

        let token_id: u32 = env.storage().instance().get(&DataKey::NextId).unwrap();
        env.storage()
            .instance()
            .set(&DataKey::NextId, &(token_id + 1));
        env.storage().instance().extend_ttl(THRESHOLD, BUMP);

        let badge = Badge {
            owner: to.clone(),
            badge_type,
            minted_at: env.ledger().timestamp(),
        };
        set_persistent(&env, &DataKey::Badge(token_id), &badge);
        set_persistent(&env, &has_key, &token_id);

        let mut owned = Self::badges_of(env.clone(), to.clone());
        owned.push_back(token_id);
        set_persistent(&env, &DataKey::Owned(to.clone()), &owned);

        Minted {
            to,
            badge_type,
            token_id,
        }
        .publish(&env);
        Ok(token_id)
    }

    /// Permanently delete one of your own badges.
    pub fn burn(env: Env, owner: Address, token_id: u32) -> Result<(), Error> {
        owner.require_auth();
        let badge = Self::badge(env.clone(), token_id)?;
        if badge.owner != owner {
            return Err(Error::NotOwner);
        }

        let p = env.storage().persistent();
        p.remove(&DataKey::Badge(token_id));
        p.remove(&DataKey::HasType(owner.clone(), badge.badge_type));
        let mut owned = Self::badges_of(env.clone(), owner.clone());
        if let Some(i) = owned.first_index_of(token_id) {
            owned.remove(i);
        }
        set_persistent(&env, &DataKey::Owned(owner.clone()), &owned);

        Burned { owner, token_id }.publish(&env);
        Ok(())
    }

    pub fn badge(env: Env, token_id: u32) -> Result<Badge, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::Badge(token_id))
            .ok_or(Error::NotFound)
    }

    pub fn owner_of(env: Env, token_id: u32) -> Result<Address, Error> {
        Ok(Self::badge(env, token_id)?.owner)
    }

    pub fn badges_of(env: Env, owner: Address) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::Owned(owner))
            .unwrap_or(Vec::new(&env))
    }

    pub fn balance(env: Env, owner: Address) -> u32 {
        Self::badges_of(env, owner).len()
    }

    pub fn has_badge(env: Env, owner: Address, badge_type: u32) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::HasType(owner, badge_type))
    }

    pub fn is_minter(env: Env, minter: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Minter(minter))
            .unwrap_or(false)
    }

    pub fn name(env: Env) -> String {
        env.storage().instance().get(&DataKey::Name).unwrap()
    }

    pub fn symbol(env: Env) -> String {
        env.storage().instance().get(&DataKey::Symbol).unwrap()
    }

    /// Metadata lives at `{base_uri}{badge_type}.json`.
    pub fn base_uri(env: Env) -> String {
        env.storage().instance().get(&DataKey::BaseUri).unwrap()
    }

    pub fn admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    pub fn set_minter(env: Env, minter: Address, enabled: bool) {
        Self::admin(env.clone()).require_auth();
        if enabled {
            set_persistent(&env, &DataKey::Minter(minter.clone()), &true);
        } else {
            env.storage()
                .persistent()
                .remove(&DataKey::Minter(minter.clone()));
        }
        MinterSet { minter, enabled }.publish(&env);
    }

    pub fn set_base_uri(env: Env, base_uri: String) {
        Self::admin(env.clone()).require_auth();
        env.storage().instance().set(&DataKey::BaseUri, &base_uri);
    }

    pub fn set_admin(env: Env, admin: Address) {
        Self::admin(env.clone()).require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn upgrade(env: Env, wasm_hash: BytesN<32>) {
        Self::admin(env.clone()).require_auth();
        env.deployer()
            .update_current_contract(ContractExecutable::Wasm(wasm_hash));
    }
}

fn set_persistent<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(
    env: &Env,
    key: &DataKey,
    value: &V,
) {
    let p = env.storage().persistent();
    p.set(key, value);
    p.extend_ttl(key, THRESHOLD, BUMP);
}

#[cfg(test)]
mod test;
