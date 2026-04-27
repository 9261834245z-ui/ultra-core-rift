```markdown
# SPEC.md

# UltraCore Rift — Technical Specification (SIRM Master-Build)

**Stable Invariant Rift Model (SIRM)** — A formally verified economic state machine on Solana.

## 1. Formal Model Definition

### State Space

The system maintains the following core state:

- **`CoreState`**:
  - `gate: Pubkey` — Authorized controller
  - `paused: bool` — Global pause flag
  - `global_field: i128` — Global coupling coefficient
  - `total_base_sum: i128` — Aggregate of all user base balances
  - `total_supply: u128` — Circulating supply
  - `total_minted: u128` — Lifetime minted amount
  - `total_burned: u128` — Lifetime burned amount
  - `p: u64` — Number of registered participants
  - `dust_accumulator: u128` — Residual from integer division in redistribution

- **`UserAccount`** (PDA seeded by `["user", authority]`):
  - `authority: Pubkey`
  - `base_balance: i128`

- **`EdgeAccount`** (PDA seeded by `["edge", from, to]`):
  - `weight: i128`

### Core Invariant (Master Equation)

After every state transition, the following equality is strictly enforced:

```math
total\_supply = total\_base\_sum + global\_field \times p
```

Where:
- `total_supply ∈ [0, MAX_SUPPLY]`
- `total_base_sum ∈ [MIN_ABS_DEBT × p, …]` (subject to debt limit)
- `global_field ∈ i128`
- `p ∈ [0, MAX_PARTICIPANTS]`

### Supply Conservation

```math
total\_supply = total\_minted - total\_burned
```

with the additional constraint `total_minted ≥ total_burned`. Both equalities are verified in `CoreState::check_invariant()`.

Additional invariant:
```math
dust\_accumulator < p \quad (\text{if } p > 0)
```

## 2. State Transitions Logic

### Registration (`register`)

When a new participant joins (`p → p + 1`):

1. `total_base_sum := total_base_sum - global_field`
2. `p := p + 1`

This adjustment ensures the invariant holds: the new participant’s implicit field contribution (`global_field`) is subtracted from the aggregate base sum, so their effective balance starts at zero.

### Unregistration (`unregister`) — The Great Exit

Participant removal is only allowed if `base_balance ≥ 0`.

- If `base_balance > 0`:
  - Burn `base_balance` from `total_supply` and increment `total_burned`
- Always:
  - `total_base_sum := total_base_sum - base_balance + global_field`
  - `p := p - 1`

**Proof of stability at p → 0**:

Before exit (with `k` participants):
```math
total\_supply = total\_base\_sum + global\_field × k
```

After removing one participant with balance `b`:
```math
total\_base\_sum' = total\_base\_sum - b + global\_field
p' = k - 1
```

New invariant:
```math
total\_supply' = (total\_base\_sum - b + global\_field) + global\_field × (k - 1)
               = total\_base\_sum + global\_field × k - b
               = total\_supply - b
```

When `b` is burned, `total_supply` is also reduced by `b`, preserving equality. When `p = 0`, the field term vanishes and only base balances (now zero for the last user if burned) remain.

### Proportional Debt Limit

Enforced on every transfer:

```math
debt\_limit = 
\begin{cases}
\text{MIN\_ABS\_DEBT} & \text{if } p = 0 \\
-\left\lfloor \frac{total\_supply}{p \times 10} \right\rfloor & \text{otherwise}
\end{cases}
```

A transfer resulting in `new_from_balance < debt_limit` reverts with `DebtLimitExceeded`.

## 3. Physical Constants & Operations

### Negative Entropy (`apply_neg_entropy`)

Applies a deterministic compression using:

```math
\text{NEG\_E} = -2\_718\_281\_828\_459\_045\_235 \approx -e \times 10^{18}
```

Operation:
1. `global_field := global_field + NEG_E`
2. `total_base_sum := total_base_sum - (NEG_E × p)`

**Invariant preservation**:
```math
\Delta = \text{NEG\_E} \times p
```
```math
total\_base\_sum' + global\_field' × p = (total\_base\_sum - \Delta) + (global\_field + \text{NEG\_E}) × p
                                     = total\_base\_sum + global\_field × p - \text{NEG\_E}×p + \text{NEG\_E}×p
                                     = total\_supply
```

A physical overflow guard prevents `p > i128::MAX / -NEG_E`.

### Redistribution (`redistribute`)

Distributes `amount` uniformly across all participants:

```math
total = amount + dust\_accumulator
q = total ÷ p
r = total mod p
```

Then:
- `global_field := global_field + q`
- `total_supply := total_supply + q × p`
- `total_minted := total_minted + q × p`
- `dust_accumulator := r`

This guarantees exact conservation: only the integer portion is minted and distributed; residual dust is carried forward.

## 4. Edge-Weighted Transfers

### `transfer` and `transfer_with_edge`

Core transfer logic (`perform_transfer`):

1. `new_from = from.base_balance - amount - edge_cost`
2. Require `new_from ≥ debt_limit`
3. Update `from.base_balance` and `to.base_balance += amount`

If `edge_cost ≠ 0`:
- `total_base_sum -= edge_cost`

Edge cost handling:
- If `edge_cost > 0`: Burn `edge_cost` from supply (`total_supply -= edge_cost`, `total_burned += edge_cost`)
- If `edge_cost < 0`: Mint `|edge_cost|` (`total_supply += |edge_cost|`, `total_minted += |edge_cost|`)

`transfer_with_edge` additionally validates the target authority and loads the directed `EdgeAccount` PDA.

All operations use checked arithmetic and end with `check_invariant()`.

## 5. Error Handling & Bounds

### Physical Limits

- `MAX_PARTICIPANTS = 1_000_000_000_000`
- `MAX_EDGE_COST = 1_000_000_000_000_000_000_000`
- `MIN_ABS_DEBT = -1_000_000_000_000_000_000`
- `MAX_SUPPLY = i128::MAX as u128`

### Arithmetic Policy

- All operations employ `checked_add`, `checked_sub`, `checked_mul`, `checked_div`, `checked_rem`.
- `try_into` for safe `u128 → i128` conversion.
- Explicit overflow guards in `apply_neg_entropy`.
- `check_invariant()` reverts with `InvariantViolation` on any discrepancy.

### Error Codes

- `InvariantViolation`
- `DebtLimitExceeded`
- `DebtOnExitNotAllowed`
- `MathOverflow`
- `SupplyUnderflow`
- `PhysicalOverflowLimit`
- `MaxParticipantsReached`
- `ProtocolPaused`
- `UnauthorizedGate`
- `UnauthorizedAuthority`
- `EdgeLimitExceeded`
- `ZeroParticipants`

## 6. Security Architecture Summary

- **PDA Seed Validation**: Strict derivation for UserAccount and EdgeAccount.
- **Authority Checks**: Signer validation on sensitive operations.
- **Gate Authorization**: All privileged instructions (`register`, `unregister`, `redistribute`, `apply_neg_entropy`, `set_edge`) require the designated gate signer.
- **Post-Transition Verification**: `check_invariant()` called after every mutation.

The combination of checked arithmetic, mathematical invariant enforcement, and deterministic operations ensures UltraCore Rift maintains perfect economic integrity under all valid execution paths.

---

**SIRM Master-Build** — Mathematically Closed Economic System
```

```