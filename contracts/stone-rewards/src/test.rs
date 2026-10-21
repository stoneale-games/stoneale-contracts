#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, AuthorizedFunction, Ledger, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    IntoVal,
};

const STONE: i128 = 10_000_000; // 1 STONE = 10^7 stroops
const CAP: i128 = 1_000 * STONE;

struct Setup<'a> {
    env: Env,
    rewards: StoneRewardsClient<'a>,
    token: TokenClient<'a>,
    admin: Address,
    reporter: Address,
}

fn setup<'a>() -> Setup<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let issuer = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(issuer);
    let admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    let rewards_id = env.register(
        StoneRewards,
        (admin.clone(), reporter.clone(), sac.address(), CAP),
    );
    StellarAssetClient::new(&env, &sac.address()).set_admin(&rewards_id);

    Setup {
        rewards: StoneRewardsClient::new(&env, &rewards_id),
        token: TokenClient::new(&env, &sac.address()),
        env,
        admin,
        reporter,
    }
}

#[test]
fn award_mints_stone_and_records_claim() {
    let s = setup();
    let player = Address::generate(&s.env);
    let achievement = symbol_short!("breathe1");

    s.rewards.award(&player, &achievement, &(50 * STONE));

    assert_eq!(s.token.balance(&player), 50 * STONE);
    assert!(s.rewards.is_claimed(&player, &achievement));
    assert_eq!(s.rewards.total_earned(&player), 50 * STONE);
    assert_eq!(s.rewards.earned_today(&player), 50 * STONE);
}

#[test]
fn award_requires_reporter_auth() {
    let s = setup();
    let player = Address::generate(&s.env);
    let achievement = symbol_short!("streak7");

    s.rewards.award(&player, &achievement, &10);

    let auths = s.env.auths();
    assert_eq!(auths[0].0, s.reporter);
    assert_eq!(
        auths[0].1.function,
        AuthorizedFunction::Contract((
            s.rewards.address.clone(),
            Symbol::new(&s.env, "award"),
            (player, achievement, 10_i128).into_val(&s.env),
        ))
    );
}

#[test]
fn award_without_auth_fails() {
    let s = setup();
    s.env.set_auths(&[]);
    let player = Address::generate(&s.env);
    assert!(s
        .rewards
        .try_award(&player, &symbol_short!("x"), &10)
        .is_err());
}

#[test]
fn achievement_can_only_be_claimed_once() {
    let s = setup();
    let player = Address::generate(&s.env);
    let achievement = symbol_short!("journal");

    s.rewards.award(&player, &achievement, &10);
    assert_eq!(
        s.rewards.try_award(&player, &achievement, &10),
        Err(Ok(Error::AlreadyClaimed))
    );
    assert_eq!(s.token.balance(&player), 10);
}

#[test]
fn rejects_non_positive_amounts() {
    let s = setup();
    let player = Address::generate(&s.env);
    assert_eq!(
        s.rewards.try_award(&player, &symbol_short!("a"), &0),
        Err(Ok(Error::InvalidAmount))
    );
    assert_eq!(
        s.rewards.try_award(&player, &symbol_short!("a"), &-5),
        Err(Ok(Error::InvalidAmount))
    );
}

#[test]
fn daily_cap_is_enforced_and_resets_next_day() {
    let s = setup();
    let player = Address::generate(&s.env);

    s.rewards.award(&player, &symbol_short!("a"), &(CAP - 1));
    assert_eq!(
        s.rewards.try_award(&player, &symbol_short!("b"), &2),
        Err(Ok(Error::DailyCapExceeded))
    );
    // Cap is per player.
    let other = Address::generate(&s.env);
    s.rewards.award(&other, &symbol_short!("b"), &CAP);

    s.env.ledger().with_mut(|l| l.timestamp += DAY_IN_SECONDS);
    assert_eq!(s.rewards.earned_today(&player), 0);
    s.rewards.award(&player, &symbol_short!("b"), &2);
    assert_eq!(s.rewards.total_earned(&player), CAP + 1);
}

#[test]
fn admin_can_rotate_reporter_and_cap() {
    let s = setup();
    let new_reporter = Address::generate(&s.env);

    s.rewards.set_reporter(&new_reporter);
    s.rewards.set_daily_cap(&5);
    assert_eq!(s.env.auths()[0].0, s.admin);

    let config = s.rewards.config();
    assert_eq!(config.reporter, new_reporter);
    assert_eq!(config.daily_cap, 5);

    let player = Address::generate(&s.env);
    assert_eq!(
        s.rewards.try_award(&player, &symbol_short!("a"), &6),
        Err(Ok(Error::DailyCapExceeded))
    );
    s.rewards.award(&player, &symbol_short!("a"), &5);
    assert_eq!(s.env.auths()[0].0, new_reporter);
}

#[test]
fn release_token_admin_stops_minting() {
    let s = setup();
    let new_admin = Address::generate(&s.env);
    s.rewards.release_token_admin(&new_admin);

    let sac = StellarAssetClient::new(&s.env, &s.token.address);
    assert_eq!(sac.admin(), new_admin);

    // Only the reporter signs: the SAC now wants `new_admin`, not this contract.
    let player = Address::generate(&s.env);
    s.env.mock_auths(&[MockAuth {
        address: &s.reporter,
        invoke: &MockAuthInvoke {
            contract: &s.rewards.address,
            fn_name: "award",
            args: (player.clone(), symbol_short!("a"), 1_i128).into_val(&s.env),
            sub_invokes: &[],
        },
    }]);
    assert!(s
        .rewards
        .try_award(&player, &symbol_short!("a"), &1)
        .is_err());
}
