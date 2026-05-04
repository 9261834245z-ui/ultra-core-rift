# UltraCore Rift — Architecture

**Document Version:** 2.5  
**Status:** Audit Ready  
**Protocol:** Stable Invariant Rift Model (SIRM)  
**Date:** May 2026

## 1. Protocol Overview

UltraCore Rift is a closed-loop deterministic invariant engine deployed on Solana. It operates as a self-contained economic environment where every state transition is strictly governed by a single global invariant.

User deposits are converted into base_balance. All internal operations — including transfers, redistribution, entropy management, and withdrawals — are executed under continuous invariant verification. The protocol deliberately limits external composability to maximize internal mathematical integrity and safety.

## 2. Core Invariants

### 2.1 Primary Supply Invariant
total_supply = total_base_sum + (global_field * current_p)

Where:
- total_supply — current circulating supply
- total_base_sum — sum of all users' base_balance
- global_field — global pressure coefficient (can be negative)
- current_p — number of active registered participants

This invariant is enforced by check_invariant() after every state-modifying instruction. Violation of the invariant causes immediate transaction reversion.

### 2.2 Conservation Invariant
total_minted - total_burned = total_supply

Both invariants serve as the fundamental safety guarantee of the protocol.

## 3. Negative Entropy Engine

Negative Entropy is the primary mechanism for active system compression and entropy control.

**Core Parameters:**
- Automated Trigger: apply_neg_entropy() is executed deterministically based on on-chain conditions (dust accumulation, debt pressure, or time interval).
- Hard Field Cap: global_field >= HARD_FIELD_CAP (default: -1.5 * 10^18)

The cap ensures bounded negative pressure and predictable user impact during exits.

## 4. Exit Protocol (Safe Withdrawal)

Exit is the only operation that converts internal system pressure into external assets.

### 4.1 Proportional Share Calculation

All computations use fixed-point arithmetic (int128) for determinism and precision.

user_share = user.base_balance / total_base_sum
claimable = user.base_balance + (user_share * global_field * current_p)

**Equivalent form:**
claimable = user.base_balance * (total_supply / total_base_sum)

### 4.2 Minimum Exit Ratio & Stabilization Fund

To protect users from extreme negative field scenarios:
- If the calculated claimable is below user.base_balance * MIN_EXIT_RATIO (default: 30%), it is raised to this floor.
- The deficit is covered by the Stabilization Fund only if sufficient balance exists. No artificial supply inflation is permitted.

**Additional safety rules:**
- DebtOnExitNotAllowed — users with active edge debts cannot exit.
- Dust and micro-debts are automatically swept to the Stabilization Fund.

## 5. Autonomous Governance

The protocol follows a clear three-phase decentralization roadmap:

**Phase 1 — Bootstrapping**  
Gate Authority manages initial system parameters and operations.

**Phase 2 — Hybrid**  
Gate actions are strictly limited by on-chain rules, timelocks, and predefined parameter bounds.

**Phase 3 — Autonomous Guardian**  
All critical operations (apply_neg_entropy, redistribute, dust cleanup) become fully automatic, executed by the smart contract according to deterministic on-chain triggers. No trusted authority remains.

## 6. Critical Code Specifications

### 6.1 exit_protocol
```cpp
void exit_protocol(UserAccount& user) {
    if (has_active_edge_debt(user)) {
        throw Error::DebtOnExitNotAllowed;
    }

    if (total_base_sum == 0) {
        throw Error::EmptySystem;
    }

    // Fixed-point arithmetic (PRECISION = 1e18)
    int128 user_share = (int128)user.base_balance * PRECISION / total_base_sum;
    int128 claimable_128 = (int128)user.base_balance + 
                           (user_share * global_field * current_p / PRECISION);

    int64_t claimable = (int64_t)claimable_128;

    // Minimum Exit Ratio Protection
    const int64_t MIN_EXIT_RATIO = 3000; // 30.00%
    int64_t floor = (user.base_balance * MIN_EXIT_RATIO) / 10000;

    if (claimable < floor) {
        int64_t deficit = floor - claimable;
        if (stabilization_fund >= deficit) {
            stabilization_fund -= deficit;
            claimable = floor;
        }
        // Otherwise exit with originally calculated amount (no supply inflation)
    }

    // Atomic state update
    total_base_sum -= user.base_balance;
    total_supply   -= claimable;
    current_p      -= 1;

    // Final safety guard
    if (!check_invariant()) {
        revert();
        throw Error::InvariantBroken;
    }

    transfer_to_external(user.wallet, claimable);
    dust_accumulator += user.dust;
    clear_user_account(user);

    // Stabilization Fund maintenance
    if (dust_accumulator >= DUST_THRESHOLD) {
        stabilization_fund += dust_accumulator;
        dust_accumulator = 0;
    }
}

bool apply_neg_entropy() {
    if (!should_apply_neg_entropy()) {
        return false;
    }

    if (global_field <= HARD_FIELD_CAP) {
        return false;
    }

    int128 temp_field = (int128)global_field + NEG_ENTROPY_CONSTANT;
    if (temp_field < INT64_MIN || temp_field > INT64_MAX) {
        return false;
    }

    int128 delta_base = NEG_ENTROPY_CONSTANT * (int128)current_p * -1LL;
    int128 new_base = (int128)total_base_sum + delta_base;

    if (new_base < INT64_MIN || new_base > INT64_MAX) {
        return false;
    }

    int64_t old_field = global_field;

    global_field = (int64_t)temp_field;
    total_base_sum = (int64_t)new_base;

    if (!check_invariant()) {
        global_field = old_field;
        total_base_sum -= (int64_t)delta_base;
        throw Error::InvariantBroken;
    }

    emit_event(NegEntropyApplied, {
        .old_field = old_field,
        .new_field = global_field,
        .p = current_p,
        .delta_base = (int64_t)delta_base
    });

    return true;
}
