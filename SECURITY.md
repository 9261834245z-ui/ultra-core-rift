```markdown
# Security.md

# UltraCore Rift (SIRM Master-Build) — Security Policy

**UltraCore Rift** is a formally-oriented economic state machine on Solana implementing the **Stable Invariant Rift Model (SIRM)**.

## Project Overview

UltraCore Rift is a high-integrity deterministic economic engine built on the **Stable Invariant Rift Model (SIRM)**. The protocol maintains perfect economic integrity through a single, rigorously enforced mathematical invariant that binds every state transition on-chain:

```math
total\_supply = total\_base\_sum + global\_field \times p
```

This invariant, together with `total_supply = total_minted - total_burned` and strict checked arithmetic, forms the foundation of the system's security model.

## Security Philosophy

Security in UltraCore Rift is not layered heuristically but **anchored in mathematics**. The core design guarantees that:

- Every instruction that mutates state ends with an explicit `check_invariant()` call.
- All arithmetic uses Rust's `checked_*` operations, preventing silent overflows/underflows.
- The proportional debt limit (`-total_supply / (p × 10)`) and participant exit rules ("The Great Exit") are enforced deterministically.
- PDA seeds, authority checks, and gate authorization eliminate common Solana attack vectors.

The system is engineered to behave as a closed thermodynamic-like economic model where value conservation is mathematically unbreakable under normal execution.

## Reporting a Vulnerability

If you discover a security vulnerability in UltraCore Rift, please report it responsibly.

**Preferred reporting channels:**
- Email: `security@ultracore.rift`
- Alternative: `security@riftcore.dev`

Please include:
- A clear description of the vulnerability
- Steps to reproduce (including transaction signatures if applicable)
- Potential impact on the invariant or funds
- Any suggested mitigation

We aim to acknowledge reports within 48 hours and provide updates on triage and resolution.

## Critical Security Zones (Scope)

The following areas represent the highest-priority targets for security research and adversarial testing:

- **Invariant Breaking**: Any state transition (register, unregister, transfer, transfer_with_edge, redistribute, apply_neg_entropy) that bypasses or violates `check_invariant()`.
- **Unsafe Debt Creation**: Any mechanism allowing a user to exceed the proportional debt limit enforced via `CoreState::debt_limit()`.
- **The Great Exit (p → 0)**: Exploits in participant removal logic that could create discrepancies between `total_base_sum`, `global_field`, and `total_supply` when the participant count drops to zero.
- **Negative Entropy Overflow**: Attacks that force `apply_neg_entropy` into undefined behavior, physical overflow limits, or invariant violations through extreme repeated application.

Additional areas of interest include:
- PDA seed manipulation and authority validation in `TransferCtx` and `TransferWithEdge`
- Edge weight handling and mint/burn accounting in `perform_transfer`
- Dust accumulator and redistribution conservation properties
- Gate authorization and privileged instruction controls

## Audit Status

The UltraCore Rift implementation has undergone rigorous adversarial validation:

- **Deterministic Fuzz Testing**: 3000+ iterations using a fixed PRNG seed (1337) via a custom 10/10 audit-grade fuzz engine.
- **Ultimate Stress Scenario**: Extreme negative entropy application (hundreds of iterations), aggressive debt creation, and malicious exit attempts.
- **Invariant Verification**: Full on-chain and shadow-model invariant checks after every action, with zero violations observed.

The fuzz suite explicitly validates the master equation, supply conservation, debt limits, and exit stability under maximum stress.

## Responsible Disclosure Policy

We ask security researchers to:

1. Refrain from public disclosure until a fix has been developed and deployed.
2. Avoid actions that could harm users, the protocol, or its participants.
3. Provide sufficient detail to reproduce and validate the issue.
4. Allow reasonable time for triage, patching, and coordinated disclosure.

In return, we commit to:
- Treating all reports confidentially.
- Crediting researchers (unless anonymity is requested).
- Working promptly to mitigate confirmed vulnerabilities.

UltraCore Rift is designed as a **Stable Invariant Rift Machine**. Any reported issue that threatens the core mathematical invariant will be treated with the highest priority.

---

*Last updated: April 2026*  
*Version: SIRM Master-Build*
```