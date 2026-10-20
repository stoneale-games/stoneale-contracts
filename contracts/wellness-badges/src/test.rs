#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{testutils::Address as _, vec};

const MOOD_STREAK_30: u32 = 1;
const FIRST_BREATH: u32 = 2;

fn setup<'a>() -> (Env, WellnessBadgesClient<'a>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let id = env.register(
        WellnessBadges,
        (
            admin.clone(),
            String::from_str(&env, "Stoneale Wellness Badges"),
            String::from_str(&env, "STNB"),
            String::from_str(&env, "https://stoneale.com/badges/"),
        ),
    );
    let client = WellnessBadgesClient::new(&env, &id);
    let minter = Address::generate(&env);
    client.set_minter(&minter, &true);
    (env, client, admin, minter)
}

#[test]
fn metadata() {
    let (env, c, admin, _) = setup();
    assert_eq!(c.name(), String::from_str(&env, "Stoneale Wellness Badges"));
    assert_eq!(c.symbol(), String::from_str(&env, "STNB"));
    assert_eq!(
        c.base_uri(),
        String::from_str(&env, "https://stoneale.com/badges/")
    );
    assert_eq!(c.admin(), admin);
}

#[test]
fn minter_can_mint() {
    let (env, c, _, minter) = setup();
    let player = Address::generate(&env);

    let a = c.mint(&minter, &player, &MOOD_STREAK_30);
    let b = c.mint(&minter, &player, &FIRST_BREATH);

    assert_eq!((a, b), (1, 2));
    assert_eq!(env.auths()[0].0, minter);
    assert_eq!(c.owner_of(&a), player);
    assert_eq!(c.badge(&b).badge_type, FIRST_BREATH);
    assert_eq!(c.badges_of(&player), vec![&env, 1, 2]);
    assert_eq!(c.balance(&player), 2);
    assert!(c.has_badge(&player, &MOOD_STREAK_30));
}

#[test]
fn non_minter_cannot_mint() {
    let (env, c, _, _) = setup();
    let rando = Address::generate(&env);
    let player = Address::generate(&env);
    assert_eq!(
        c.try_mint(&rando, &player, &MOOD_STREAK_30),
        Err(Ok(Error::NotMinter))
    );
}

#[test]
fn one_badge_per_type() {
    let (env, c, _, minter) = setup();
    let player = Address::generate(&env);
    c.mint(&minter, &player, &MOOD_STREAK_30);
    assert_eq!(
        c.try_mint(&minter, &player, &MOOD_STREAK_30),
        Err(Ok(Error::AlreadyHasBadge))
    );
}

#[test]
fn revoked_minter_cannot_mint() {
    let (env, c, _, minter) = setup();
    c.set_minter(&minter, &false);
    assert!(!c.is_minter(&minter));
    let player = Address::generate(&env);
    assert_eq!(
        c.try_mint(&minter, &player, &MOOD_STREAK_30),
        Err(Ok(Error::NotMinter))
    );
}

#[test]
fn owner_can_burn_and_re_earn() {
    let (env, c, _, minter) = setup();
    let player = Address::generate(&env);
    let id = c.mint(&minter, &player, &MOOD_STREAK_30);
    c.mint(&minter, &player, &FIRST_BREATH);

    c.burn(&player, &id);
    assert_eq!(env.auths()[0].0, player);
    assert_eq!(c.try_owner_of(&id), Err(Ok(Error::NotFound)));
    assert_eq!(c.badges_of(&player), vec![&env, 2]);
    assert!(!c.has_badge(&player, &MOOD_STREAK_30));

    let again = c.mint(&minter, &player, &MOOD_STREAK_30);
    assert_eq!(again, 3);
}

#[test]
fn cannot_burn_someone_elses_badge() {
    let (env, c, _, minter) = setup();
    let player = Address::generate(&env);
    let thief = Address::generate(&env);
    let id = c.mint(&minter, &player, &MOOD_STREAK_30);
    assert_eq!(c.try_burn(&thief, &id), Err(Ok(Error::NotOwner)));
    assert_eq!(c.try_burn(&thief, &99), Err(Ok(Error::NotFound)));
}
