# StudyVault 📚

> A Stellar-powered semester savings vault that helps SEA university students lock USDC tuition funds, earn savings streaks, and avoid impulsive spending.

---

## The Problem

A 19-year-old nursing student in Cebu, Philippines receives ₱15,000 in remittances from her OFW parent every 3 months. The money sits in an e-wallet and gets eroded by daily expenses before tuition is due — causing missed enrollment and lost academic progress.

## The Solution

StudyVault lets the student deposit USDC into a time-locked Soroban smart contract. Funds are frozen until the target date (e.g., enrollment week). Early withdrawals trigger a 5% penalty that flows to a university scholarship pool. A streak counter rewards consistent depositors — unlocking future eligibility for NGO top-up grants.

---

## Stellar Features Used

| Feature | Purpose |
|---|---|
| USDC (Stellar native) | Stablecoin savings — no PHP volatility risk |
| Soroban smart contracts | Time-lock logic + penalty enforcement |
| Trustlines | Student wallet must trust the USDC issuer |
| Token transfer (Stellar token interface) | Pull deposits, push payouts, route penalties |

---

## Target Users

- University students aged 17–24 in PH, ID, VN, TH
- Receiving remittances from parents or part-time income
- Using GCash, Maya, or OnafriqPay (future anchor integration)
- Motivated by tuition deadlines and scholarship eligibility

---

## MVP Core Feature (Demo Flow)

1. Student calls `open_vault(student, 500_000_000, 90, "tuition_sem2")`
2. Contract pulls 50 USDC from student wallet via `token.transfer`
3. Lock is set to 90 days from now
4. Student calls `deposit()` weekly → streak increments
5. At unlock date: `withdraw()` returns 100% of balance
6. Before unlock: `withdraw()` returns 95%, sends 5% to university admin wallet

**Demo runtime: ~90 seconds on Stellar testnet**

---

## Vision & Purpose

StudyVault targets a real gap: SEA students with irregular income who struggle to protect savings earmarked for education. The penalty mechanic inverts the usual incentive — keeping money locked is *rewarded*, not punished. The streak system creates a visible savings habit that NGOs and universities can verify on-chain to issue merit-based top-up grants.

---

## Prerequisites

- Rust (stable) + `wasm32-unknown-unknown` target
  ```bash
  rustup target add wasm32-unknown-unknown
  ```
- Stellar CLI v21+
  ```bash
  cargo install --locked stellar-cli --features opt
  ```
- Funded Stellar testnet keypair

---

## Build

```bash
stellar contract build
# Output: target/wasm32-unknown-unknown/release/study_vault.wasm
```

## Test

```bash
cargo test --features testutils
```

---

## Deploy to Testnet

```bash
# 1. Generate a keypair
stellar keys generate deployer --network testnet

# 2. Fund via Friendbot
stellar keys fund deployer --network testnet

# 3. Deploy contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/study_vault.wasm \
  --source deployer \
  --network testnet

# 4. Initialize (replace placeholders)
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --token_id <USDC_TOKEN_CONTRACT> \
  --penalty_bps 500
```

---

## Sample CLI Invocations

### Open a vault (lock 180 days, deposit 50 USDC)
```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source student-wallet \
  --network testnet \
  -- open_vault \
  --student <STUDENT_ADDRESS> \
  --amount 500000000 \
  --lock_days 180 \
  --goal_label tuition_sem2
```

### Add weekly deposit
```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source student-wallet \
  --network testnet \
  -- deposit \
  --student <STUDENT_ADDRESS> \
  --amount 100000000
```

### Withdraw (post-lock)
```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source student-wallet \
  --network testnet \
  -- withdraw \
  --student <STUDENT_ADDRESS>
```

### Check vault state
```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- get_vault \
  --student <STUDENT_ADDRESS>
```

---

## Deploy Reference

Full Stellar Bootcamp deployment guide:
https://github.com/armlynobinguar/Stellar-Bootcamp-2026

Example full-stack Soroban app:
https://github.com/armlynobinguar/community-treasury

Contract ID:
CDTLNLY5O4YI6NLJTNMA7MGJENCZRMBDP2WTLCR4MEUZDJ3NMGQW5R3E


---

## License

MIT © 2026 StudyVault Contributors
