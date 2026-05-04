# TOKENOMICS

**UltraCore Rift — Stable Invariant Rift Model (SIRM)**  
**Rift Token Program** | Version 2.5 | May 2026

## 1. Economic Philosophy

UltraCore Rift is built as a deterministic closed-loop economic system. The protocol rejects speculative emission models and instead derives value from maintaining a strict mathematical invariant enforced on-chain after every instruction:

$$
\text{total\_supply} = \text{total\_base\_sum} + (\text{global\_field} \times p)
$$

All economic activity, including RIFT token issuance, is subordinated to this invariant and conservation laws. Economic expansion and holder returns emerge as mathematical consequences of entropy compression rather than from inflationary incentives.

## 2. Rift Token Utility

RIFT is the share token of the Rift Token program, directly linked to the core invariant engine (`ultra_core_rift`).

### Genesis Allocation (Founder Share)
At initialization, a fixed founder allocation of **3.14%** (`FOUNDER_SHARE_BPS = 314`) of the initial supply is minted to the `admin_vault`. This is the only genesis issuance. No hidden allocations or future team unlocks exist.

### The Multiplier Mechanic
The protocol uses a dynamic `rift_multiplier` to align incentives with system stability:

```rust
let field_pressure = core.global_field.unsigned_abs().max(1) as u128;
let new_multiplier = 1_000_000_000_000_000u128 / field_pressure;

When the Negative Entropy Engine compresses global_field (reduces pressure), the multiplier increases.
When pressure grows, the multiplier decreases.
This creates a mathematical lever: system stabilization directly increases the share value of existing RIFT holders through rebase mechanics.

Issue Logic
Users convert base assets into RIFT shares via issue_rift(base_amount):

A protocol fee (fee_bps, max 10 BPS) is charged in SOL and sent to admin_vault.
Remaining amount is multiplied by the current rift_multiplier to determine shares_to_mint.
The lower the field pressure, the higher the shares received per unit of base.

3. Revenue Streams & Real Yield
Protocol revenue is transparent and capped by code:

Protocol Fee: Maximum MAX_FEE_BPS = 10 (0.1%). All fees flow directly to admin_vault to accumulate the Stabilization Fund.
Stabilization Fund: Used exclusively to support the 30% exit floor.
Entropy Compression Yield: As global_field decreases through Negative Entropy, the rising rift_multiplier accretes value to all existing RIFT holders without new issuance.

All yield is real — it comes from improved system efficiency and value redistribution via the invariant, not from new token printing.
4. Scaling & Expansion
The global_field serves as the central coupling variable. As participation (p) grows:

The impact of global_field scales linearly within the main invariant.
The Negative Entropy Engine allows controlled compression, preventing runaway debt.
The rift_multiplier automatically adjusts, providing natural yield amplification during periods of successful entropy reduction.

The system is designed to scale while preserving mathematical coherence, even under high Solana network volatility, thanks to strict checked arithmetic and deterministic rules.
5. Investor Protections — Safe Exit Protocol
30% Floor
Exiting users are guaranteed a minimum claim of 30% of their base_balance through the Stabilization Fund, funded by protocol fees and dust accumulation. This mechanism is backed by real reserves, not inflation.
Mathematical Impossibility of Insolvency
While the core invariant holds:
$$\text{total\_supply} = \text{total\_base\_sum} + (\text{global\_field} \times p)$$
and with check_invariant() enforced on every operation, the system cannot accumulate obligations exceeding available supply. Any violation reverts the transaction via InvariantViolation.
Combined with DebtOnExitNotAllowed, catastrophic failure states are mathematically unreachable.

Conclusion
RIFT token derives its value from participation in a mathematically consistent economic engine. Holder returns are driven by system stabilization (via multiplier and entropy compression) and protected by hard on-chain rules rather than discretionary governance or emissions.
