#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short,
    Address, Env, Symbol, Map, Vec, token,
};

// ─── Storage Key Types ───────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Maps student Address → their Vault struct
    Vault(Address),
    /// Maps student Address → cumulative streak (days)
    Streak(Address),
    /// Contract admin (e.g. university or NGO anchor)
    Admin,
    /// The accepted stablecoin token contract (USDC on Stellar)
    TokenId,
    /// Penalty rate in basis points (e.g. 500 = 5%)
    PenaltyBps,
}

// ─── Data Structs ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct Vault {
    /// Student who owns this vault
    pub owner: Address,
    /// Total USDC deposited (in stroops: 1 USDC = 10_000_000)
    pub balance: i128,
    /// Unix timestamp of first deposit
    pub created_at: u64,
    /// Unlock date: funds can be penalty-free withdrawn after this
    pub unlock_at: u64,
    /// Whether the vault has been closed
    pub closed: bool,
    /// Savings goal label (e.g. "Tuition Semester 2")
    pub goal_label: Symbol,
}

// ─── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct StudyVaultContract;

#[contractimpl]
impl StudyVaultContract {

    /// Initialize the contract.
    /// admin       - university/NGO wallet that can configure parameters
    /// token_id    - the Stellar USDC token contract address
    /// penalty_bps - early-withdrawal penalty in basis points (e.g. 500 = 5%)
    pub fn initialize(
        env: Env,
        admin: Address,
        token_id: Address,
        penalty_bps: u32,
    ) {
        // Prevent re-initialization
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::TokenId, &token_id);
        env.storage().instance().set(&DataKey::PenaltyBps, &penalty_bps);
    }

    /// Student opens a savings vault and makes an initial deposit.
    /// student     - student wallet (must sign)
    /// amount      - USDC amount in stroops
    /// lock_days   - how many days to lock (e.g. 180 = one semester)
    /// goal_label  - a short label like "tuition_sem2"
    pub fn open_vault(
        env: Env,
        student: Address,
        amount: i128,
        lock_days: u64,
        goal_label: Symbol,
    ) {
        student.require_auth();
        assert!(amount > 0, "amount must be positive");
        assert!(lock_days > 0, "lock must be at least 1 day");
        // Student must not already have an open vault
        assert!(
            !env.storage().persistent().has(&DataKey::Vault(student.clone())),
            "vault already exists"
        );

        let token_id: Address = env.storage().instance().get(&DataKey::TokenId).unwrap();
        let token = token::Client::new(&env, &token_id);

        // Pull USDC from student wallet into the contract
        token.transfer(&student, &env.current_contract_address(), &amount);

        let now = env.ledger().timestamp();
        let vault = Vault {
            owner:      student.clone(),
            balance:    amount,
            created_at: now,
            unlock_at:  now + lock_days * 86_400, // seconds per day
            closed:     false,
            goal_label,
        };

        env.storage().persistent().set(&DataKey::Vault(student.clone()), &vault);

        // Initialize streak to 1 on first deposit
        env.storage().persistent().set(&DataKey::Streak(student), &1u32);

        env.events().publish(
            (symbol_short!("vault_op"), symbol_short!("open")),
            amount,
        );
    }

    /// Student deposits additional USDC into their existing vault.
    pub fn deposit(env: Env, student: Address, amount: i128) {
        student.require_auth();
        assert!(amount > 0, "amount must be positive");

        let mut vault: Vault = env
            .storage()
            .persistent()
            .get(&DataKey::Vault(student.clone()))
            .expect("vault not found");

        assert!(!vault.closed, "vault is closed");

        let token_id: Address = env.storage().instance().get(&DataKey::TokenId).unwrap();
        let token = token::Client::new(&env, &token_id);
        token.transfer(&student, &env.current_contract_address(), &amount);

        vault.balance += amount;
        env.storage().persistent().set(&DataKey::Vault(student.clone()), &vault);

        // Increment streak for consecutive weekly deposits (simplified: every deposit = +1)
        let streak: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::Streak(student.clone()))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::Streak(student), &(streak + 1));

        env.events().publish(
            (symbol_short!("vault_op"), symbol_short!("deposit")),
            amount,
        );
    }

    /// Student withdraws from their vault.
    /// If withdrawn before unlock_at, a penalty is deducted and sent to admin.
    /// After unlock_at, full balance is returned.
    pub fn withdraw(env: Env, student: Address) -> i128 {
        student.require_auth();

        let mut vault: Vault = env
            .storage()
            .persistent()
            .get(&DataKey::Vault(student.clone()))
            .expect("vault not found");

        assert!(!vault.closed, "vault already closed");

        let token_id: Address = env.storage().instance().get(&DataKey::TokenId).unwrap();
        let token = token::Client::new(&env, &token_id);
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();

        let now = env.ledger().timestamp();
        let payout: i128;

        if now >= vault.unlock_at {
            // Penalty-free: return full balance
            payout = vault.balance;
        } else {
            // Early withdrawal: apply penalty
            let penalty_bps: u32 = env
                .storage()
                .instance()
                .get(&DataKey::PenaltyBps)
                .unwrap_or(500);
            let penalty = vault.balance * penalty_bps as i128 / 10_000;
            payout = vault.balance - penalty;
            // Send penalty to admin wallet (could fund a scholarship pool)
            if penalty > 0 {
                token.transfer(&env.current_contract_address(), &admin, &penalty);
            }
        }

        // Send payout to student
        token.transfer(&env.current_contract_address(), &student, &payout);

        vault.balance = 0;
        vault.closed  = true;
        env.storage().persistent().set(&DataKey::Vault(student.clone()), &vault);

        env.events().publish(
            (symbol_short!("vault_op"), symbol_short!("withdraw")),
            payout,
        );

        payout
    }

    // ─── Read-only Helpers ────────────────────────────────────────────────────

    /// Returns the student's vault data.
    pub fn get_vault(env: Env, student: Address) -> Vault {
        env.storage()
            .persistent()
            .get(&DataKey::Vault(student))
            .expect("vault not found")
    }

    /// Returns the student's deposit streak count.
    pub fn get_streak(env: Env, student: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::Streak(student))
            .unwrap_or(0)
    }

    /// Returns current contract admin.
    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }
}