```markdown
# OPS.md

# UltraCore Rift (SIRM Master-Build) — Operations Manual

**For the Protocol Operator (Gate Authority)**

This manual provides tactical, actionable procedures for managing the UltraCore Rift economic state machine on Solana.

## 1. Deployment & Initialization

### Initialize the Core State

Deploy and initialize the protocol using the `initialize` instruction:

```rust
program.methods
  .initialize(gate_public_key)
  .accounts({
    coreState: coreStatePda,        // New account to be initialized
    payer: gate.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .signers([coreStateKeypair])
  .rpc();
```

**Post-initialization verification** (zero-state):
- `p = 0`
- `global_field = 0`
- `total_base_sum = 0`
- `total_supply = 0`
- `total_minted = 0`
- `total_burned = 0`
- `dust_accumulator = 0`
- `paused = false`

The `check_invariant()` will pass as `0 = 0 + 0 × 0`.

The `gate` authority set during initialization has exclusive control over privileged instructions.

## 2. Participant Management

### Register New Strategic Participants

To onboard a new participant:

```rust
program.methods
  .register(user_public_key)
  .accounts({
    coreState: coreState.publicKey,
    userAccount: userPda,                    // PDA: ["user", user_public_key]
    gate: gate.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

- Automatically adjusts `total_base_sum -= global_field` to preserve the invariant.
- Fails if `p == MAX_PARTICIPANTS`.

### Unregister Participants (The Great Exit)

To remove a participant:

```rust
program.methods
  .unregister()
  .accounts({
    coreState: coreState.publicKey,
    userAccount: userPda,          // Will be closed
    gate: gate.publicKey,
  })
  .rpc();
```

**Critical requirement**: The participant's `base_balance` **must be ≥ 0**. Negative balances (debt) are not allowed on exit (`DebtOnExitNotAllowed`).

- If `base_balance > 0`, the amount is burned from `total_supply`.
- `total_base_sum` is adjusted by `-base_balance + global_field` and `p` is decremented.
- The invariant remains intact even when `p → 0`.

## 3. Global Field Operations

### Apply Negative Entropy

Use to compress the global field:

```rust
program.methods
  .applyNegEntropy()
  .accounts({
    coreState: coreState.publicKey,
    gate: gate.publicKey,
  })
  .rpc();
```

**Effect**:
- `global_field += NEG_E` (where `NEG_E ≈ -e × 10¹⁸`)
- `total_base_sum -= NEG_E × p`

**Operational guidance**:
- Use to drive deep negative field regimes or to counteract excessive positive field growth.
- Monitor for `PhysicalOverflowLimit` — occurs when `p > i128::MAX / -NEG_E`.
- Repeated application can push the system into extreme negative territory while preserving the invariant.

### Value Redistribution

To mint and distribute value uniformly across all participants:

```rust
program.methods
  .redistribute(amount)
  .accounts({
    coreState: coreState.publicKey,
    gate: gate.publicKey,
  })
  .rpc();
```

**Mechanics**:
- `total = amount + dust_accumulator`
- `q = total / p`, `r = total % p`
- `global_field += q`
- `total_supply += q × p`, `total_minted += q × p`
- `dust_accumulator = r`

This ensures no value leakage. Use when injecting new supply or rebalancing the field.

## 4. Edge & Parameter Tuning

### Configure Edge Weights

Set directed interaction costs between two users:

```rust
program.methods
  .setEdge(from_public_key, to_public_key, weight)
  .accounts({
    coreState: coreState.publicKey,
    edgeAccount: edgePda,        // PDA: ["edge", from, to]
    gate: gate.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

- `weight` must satisfy `-MAX_EDGE_COST ≤ weight ≤ MAX_EDGE_COST`.
- Positive weight burns supply on transfer; negative weight mints supply.
- Used in `transfer_with_edge` to apply dynamic costs.

### Pause / Unpause the Protocol

The `paused` flag is part of `CoreState`. While the current implementation does not expose a dedicated toggle instruction in the provided interface, the gate can coordinate upgrades or use governance mechanisms if implemented in future extensions. In emergency situations, monitor for `ProtocolPaused` reverts.

## 5. Emergency Protocols

### Handling Suspected Invariant Pressure

1. Immediately call `applyNegEntropy()` or `redistribute()` as needed to stabilize field dynamics.
2. Monitor transaction logs for `InvariantViolation` errors — any such failure indicates a critical state anomaly.
3. Use `pause` logic (if extended) or gate-controlled restrictions to halt non-essential operations.
4. Verify participant debt levels against the proportional debt limit:
   ```math
   debt_limit = p > 0 ? -(total_supply / (p × 10)) : MIN_ABS_DEBT
   ```

### Monitoring & Verification

- After every gate-initiated operation, confirm `check_invariant()` succeeded.
- Track `total_minted ≥ total_burned` and exact equality `total_supply = total_minted - total_burned`.
- Watch `dust_accumulator < p` when `p > 0`.
- Use the shadow model from fuzz tests for off-chain validation of complex sequences.

**Tactical Note**: The combination of checked arithmetic and post-instruction invariant verification makes silent corruption extremely unlikely. Any `InvariantViolation` should trigger immediate investigation.

---

**SIRM Master-Build**  
Operational procedures derived directly from on-chain implementation. All instructions, account seeds, and error conditions match the `lib.rs` source of truth.
```