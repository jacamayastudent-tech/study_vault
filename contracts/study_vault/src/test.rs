#[cfg(test)]
mod tests {
    use soroban_sdk::{
        testutils::{Address as _, Ledger, LedgerInfo},
        Address, Env, Symbol,
        token,
    };
    use crate::{StudyVaultContract, StudyVaultContractClient};

    // Helper: deploy a mock USDC token and mint to an address
    fn create_token(env: &Env, admin: &Address) -> Address {
        let token_id = env.register_stellar_asset_contract(admin.clone());
        let token_client = token::StellarAssetClient::new(env, &token_id);
        token_client.mint(admin, &10_000_000_000); // 1000 USDC to admin
        token_id
    }

    fn setup() -> (Env, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin   = Address::generate(&env);
        let student = Address::generate(&env);
        let token_id = create_token(&env, &admin);
        // Fund student with 100 USDC
        let token_client = token::StellarAssetClient::new(&env, &token_id);
        token_client.mint(&student, &1_000_000_000); // 100 USDC
        (env, admin, student, token_id)
    }

    // ─── Test 1: Happy path ──────────────────────────────────────────────────
    // Student opens a vault, deposits extra, then withdraws after lock expires.
    // Full balance should be returned, no penalty.
    #[test]
    fn test_happy_path_open_deposit_withdraw() {
        let (env, admin, student, token_id) = setup();
        let contract_id = env.register_contract(None, StudyVaultContract);
        let client = StudyVaultContractClient::new(&env, &contract_id);

        client.initialize(&admin, &token_id, &500u32); // 5% penalty

        // Open vault: lock 30 days, deposit 50 USDC
        client.open_vault(
            &student,
            &500_000_000i128,       // 50 USDC
            &30u64,
            &Symbol::new(&env, "tuition_sem1"),
        );

        // Second deposit of 20 USDC
        client.deposit(&student, &200_000_000i128);

        let vault = client.get_vault(&student);
        assert_eq!(vault.balance, 700_000_000i128); // 70 USDC total

        // Fast-forward 31 days past the lock
        env.ledger().set(LedgerInfo {
            timestamp: env.ledger().timestamp() + 31 * 86_400,
            ..env.ledger().get()
        });

        let payout = client.withdraw(&student);
        assert_eq!(payout, 700_000_000i128, "full payout after lock");

        let vault_after = client.get_vault(&student);
        assert!(vault_after.closed, "vault should be marked closed");
    }

    // ─── Test 2: Edge case – early withdrawal penalty ────────────────────────
    // Student withdraws before the lock period ends.
    // Payout should equal balance minus 5% penalty.
    #[test]
    fn test_early_withdrawal_applies_penalty() {
        let (env, admin, student, token_id) = setup();
        let contract_id = env.register_contract(None, StudyVaultContract);
        let client = StudyVaultContractClient::new(&env, &contract_id);

        client.initialize(&admin, &token_id, &500u32); // 5%

        client.open_vault(
            &student,
            &1_000_000_000i128, // 100 USDC
            &180u64,            // 6-month lock
            &Symbol::new(&env, "tuition_sem2"),
        );

        // Withdraw immediately (day 0 – well before unlock)
        let payout = client.withdraw(&student);
        let expected = 1_000_000_000i128 - (1_000_000_000i128 * 500 / 10_000); // 95 USDC
        assert_eq!(payout, expected, "5% penalty must be applied on early withdrawal");
    }

    // ─── Test 3: State verification ─────────────────────────────────────────
    // After open_vault, storage must reflect correct balance, lock, and streak.
    #[test]
    fn test_state_after_open_vault() {
        let (env, admin, student, token_id) = setup();
        let contract_id = env.register_contract(None, StudyVaultContract);
        let client = StudyVaultContractClient::new(&env, &contract_id);

        client.initialize(&admin, &token_id, &500u32);

        let now = env.ledger().timestamp();
        client.open_vault(
            &student,
            &300_000_000i128, // 30 USDC
            &90u64,
            &Symbol::new(&env, "laptop_fund"),
        );

        let vault = client.get_vault(&student);
        assert_eq!(vault.balance,    300_000_000i128);
        assert_eq!(vault.owner,      student);
        assert_eq!(vault.unlock_at,  now + 90 * 86_400);
        assert!(!vault.closed,       "new vault must not be closed");

        let streak = client.get_streak(&student);
        assert_eq!(streak, 1u32, "streak should start at 1 after first deposit");
    }

    // ─── Test 4: Duplicate vault rejected ───────────────────────────────────
    // A student trying to open a second vault should panic.
    #[test]
    #[should_panic(expected = "vault already exists")]
    fn test_cannot_open_duplicate_vault() {
        let (env, admin, student, token_id) = setup();
        let contract_id = env.register_contract(None, StudyVaultContract);
        let client = StudyVaultContractClient::new(&env, &contract_id);

        client.initialize(&admin, &token_id, &500u32);
        client.open_vault(
            &student,
            &100_000_000i128,
            &30u64,
            &Symbol::new(&env, "goal_a"),
        );
        // Second open_vault should panic
        client.open_vault(
            &student,
            &100_000_000i128,
            &30u64,
            &Symbol::new(&env, "goal_b"),
        );
    }

    // ─── Test 5: Streak increments with each deposit ─────────────────────────
    // After 3 deposits the streak counter should equal 3.
    #[test]
    fn test_streak_increments_on_deposit() {
        let (env, admin, student, token_id) = setup();
        let token_client = token::StellarAssetClient::new(&env, &token_id);
        token_client.mint(&student, &5_000_000_000i128); // extra funds

        let contract_id = env.register_contract(None, StudyVaultContract);
        let client = StudyVaultContractClient::new(&env, &contract_id);

        client.initialize(&admin, &token_id, &500u32);
        client.open_vault(
            &student,
            &100_000_000i128,
            &365u64,
            &Symbol::new(&env, "yearly_goal"),
        );

        // streak is 1 after open_vault
        assert_eq!(client.get_streak(&student), 1u32);

        client.deposit(&student, &50_000_000i128);
        assert_eq!(client.get_streak(&student), 2u32);

        client.deposit(&student, &50_000_000i128);
        assert_eq!(client.get_streak(&student), 3u32);
    }
}