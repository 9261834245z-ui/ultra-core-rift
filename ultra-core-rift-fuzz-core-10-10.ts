import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { startAnchor, BankrunProvider } from "anchor-bankrun";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { assert } from "chai";
import { UltraCoreRift } from "../target/types/ultra_core_rift";

// ============================================================================
// DETERMINISTIC PRNG
// ============================================================================
const SEED = 1337;
function mulberry32(a: number) {
  return function () {
    let t = (a += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const random = mulberry32(SEED);

const ITERATIONS = 3000;
const USERS_COUNT = 100;

const NEG_E = -2718281828459045235n;
const MIN_ABS_DEBT = -1000000000000000000n;
const I128_MAX = (1n << 127n) - 1n;
const I128_MIN = -(1n << 127n);
const MAX_PARTICIPANTS = 1000000000000n;

// ============================================================================
// HISTORY
// ============================================================================
const history: string[] = [];
const log = (msg: string) => {
  history.push(msg);
  if (history.length > 200) history.shift();
};

// ============================================================================
// SUPER 10/10 CORE FUZZ ENGINE
// ============================================================================
describe("UltraCoreRift V2.2 — SUPER 10/10 Audit-Grade Fuzz Core", () => {
  let provider: BankrunProvider;
  let program: Program<UltraCoreRift>;
  let gate: Keypair;
  let coreState: Keypair;

  const unregisteredUsers: Keypair[] = Array.from({ length: USERS_COUNT }, () => Keypair.generate());
  const registeredUsers: Keypair[] = [];
  const initializedEdges = new Map<string, bigint>();

  const shadowBalances = new Map<string, bigint>();
  let shadowP = 0n;
  let shadowGlobalField = 0n;
  let shadowTotalBaseSum = 0n;
  let shadowTotalSupply = 0n;
  let shadowTotalMinted = 0n;
  let shadowTotalBurned = 0n;
  let shadowDustAccumulator = 0n;

  before(async () => {
    const context = await startAnchor("", [], []);
    provider = new BankrunProvider(context);
    anchor.setProvider(provider);
    program = anchor.workspace.UltraCoreRift as Program<UltraCoreRift>;
    gate = provider.wallet.payer;
    coreState = Keypair.generate();

    await program.methods
      .initialize(gate.publicKey)
      .accounts({
        coreState: coreState.publicKey,
        payer: gate.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([coreState])
      .rpc();

    log("CoreState initialized");
  });

  const getUserPda = (pubkey: PublicKey) =>
    PublicKey.findProgramAddressSync([Buffer.from("user"), pubkey.toBuffer()], program.programId)[0];

  const getEdgePda = (from: PublicKey, to: PublicKey) =>
    PublicKey.findProgramAddressSync([Buffer.from("edge"), from.toBuffer(), to.toBuffer()], program.programId)[0];

  const getDebtLimit = (): bigint => {
    const factor = shadowP * 10n;
    return factor === 0n ? MIN_ABS_DEBT : -(shadowTotalSupply / factor);
  };

  // ============================================================================
  // SAFE EXECUTION (сердце 10/10)
  // ============================================================================
  const safeExec = async (
    execute: () => Promise<any>,
    expectedError: string | null,
    onSuccess: () => void
  ) => {
    try {
      await execute();
      if (expectedError) assert.fail(`Expected ${expectedError} but succeeded`);
      onSuccess();
    } catch (e: any) {
      if (expectedError) {
        assert.include(e.message, expectedError, `Expected ${expectedError}, got: ${e.message}`);
      } else {
        throw e;
      }
    }
  };

  // ============================================================================
  // INVARIANTS
  // ============================================================================
  const verifyInvariants = async () => {
    const state = await program.account.coreState.fetch(coreState.publicKey);

    const supply = BigInt(state.totalSupply.toString());
    const baseSum = BigInt(state.totalBaseSum.toString());
    const field = BigInt(state.globalField.toString());
    const p = BigInt(state.p.toString());
    const minted = BigInt(state.totalMinted.toString());
    const burned = BigInt(state.totalBurned.toString());
    const dust = BigInt(state.dustAccumulator.toString());

    assert.equal(p, shadowP);
    assert.equal(field, shadowGlobalField);
    assert.equal(baseSum, shadowTotalBaseSum);
    assert.equal(supply, shadowTotalSupply);
    assert.equal(minted, shadowTotalMinted);
    assert.equal(burned, shadowTotalBurned);
    assert.equal(dust, shadowDustAccumulator);

    assert.equal(supply, baseSum + field * p, "CRITICAL: Master Equation Violated");
    assert.isTrue(minted >= burned);
    assert.equal(supply, minted - burned, "CRITICAL: Conservation Violated");

    if (p > 0n) assert.isTrue(dust < p);

    let sumBase = 0n;
    let sumEffective = 0n;
    const dl = getDebtLimit();

    for (const user of registeredUsers) {
      const userStr = user.publicKey.toBase58();
      const acc = await program.account.userAccount.fetch(getUserPda(user.publicKey));
      const base = BigInt(acc.baseBalance.toString());
      const shadowBase = shadowBalances.get(userStr)!;

      assert.equal(base, shadowBase);
      assert.isTrue(base >= dl);

      sumBase += base;
      sumEffective += base + field;
    }

    assert.equal(sumBase, shadowTotalBaseSum);
    assert.equal(sumEffective, shadowTotalSupply, "CRITICAL: Effective sum mismatch");
  };

  // ============================================================================
  // MAIN FUZZ LOOP — чистый и безопасный
  // ============================================================================
  it(`Executes ${ITERATIONS} SUPER 10/10 iterations`, async () => {
    try {
      for (let i = 0; i < ITERATIONS; i++) {
        let action = Math.floor(random() * 6);

        // Global safety guards
        if (unregisteredUsers.length === 0 && action === 0) action = 1;
        if (registeredUsers.length < 2 && [1, 2].includes(action)) action = 0;
        if (shadowP === 0n && action === 3) action = 0;

        if (action === 0) { // REGISTER
          if (unregisteredUsers.length === 0) continue;
          const idx = Math.floor(random() * unregisteredUsers.length);
          const user = unregisteredUsers[idx];
          const userStr = user.publicKey.toBase58();

          const predicted = shadowP >= MAX_PARTICIPANTS ? "MaxParticipantsReached" : null;

          await safeExec(
            () => program.methods.register(user.publicKey).accounts({
              coreState: coreState.publicKey,
              userAccount: getUserPda(user.publicKey),
              gate: gate.publicKey,
              systemProgram: SystemProgram.programId,
            }).rpc(),
            predicted,
            () => {
              unregisteredUsers.splice(idx, 1);
              registeredUsers.push(user);
              shadowBalances.set(userStr, 0n);
              shadowP += 1n;
              shadowTotalBaseSum -= shadowGlobalField;
              log(`[REGISTER] ${userStr.slice(0, 8)}`);
            }
          );
        } else if (action === 1) { // TRANSFER
          if (registeredUsers.length < 2) continue;

          const from = registeredUsers[Math.floor(random() * registeredUsers.length)];
          let to = registeredUsers[Math.floor(random() * registeredUsers.length)];
          while (to.publicKey.equals(from.publicKey)) to = registeredUsers[Math.floor(random() * registeredUsers.length)];

          const fromStr = from.publicKey.toBase58();
          const toStr = to.publicKey.toBase58();
          const edgeKey = `${fromStr}-${toStr}`;
          const edgePda = getEdgePda(from.publicKey, to.publicKey);

          let edgeCost = initializedEdges.get(edgeKey) || 0n;
          if (random() < 0.35) {
            const weight = Math.floor(random() * 2000) - 1000;
            await program.methods.setEdge(from.publicKey, to.publicKey, new BN(weight)).accounts({
              coreState: coreState.publicKey,
              edgeAccount: edgePda,
              gate: gate.publicKey,
              systemProgram: SystemProgram.programId,
            }).rpc();
            edgeCost = BigInt(weight);
            initializedEdges.set(edgeKey, edgeCost);
          }

          const amt = BigInt(Math.floor(random() * 8000) + 1);
          const amountBn = new BN(amt.toString());
          const edgeOpt = initializedEdges.has(edgeKey) ? edgePda : null;

          const current = shadowBalances.get(fromStr)!;
          const newBase = current - amt - edgeCost;
          const dl = getDebtLimit();

          let predicted: string | null = null;
          if (newBase < dl) predicted = "DebtLimitExceeded";
          else if (edgeCost > 0n && edgeCost > shadowTotalSupply) predicted = "SupplyUnderflow";
          else if (newBase < I128_MIN || newBase > I128_MAX) predicted = "MathOverflow";

          await safeExec(
            () => program.methods.transfer(amountBn).accounts({
              coreState: coreState.publicKey,
              fromUser: getUserPda(from.publicKey),
              toUser: getUserPda(to.publicKey),
              fromAuthority: from.publicKey,
              toAuthority: to.publicKey,
              edgeAccount: edgeOpt,
            }).signers([from]).rpc(),
            predicted,
            () => {
              shadowBalances.set(fromStr, newBase);
              shadowBalances.set(toStr, shadowBalances.get(toStr)! + amt);
              shadowTotalBaseSum -= edgeCost;

              if (edgeCost > 0n) {
                shadowTotalSupply -= edgeCost;
                shadowTotalBurned += edgeCost;
              } else if (edgeCost < 0n) {
                const mint = -edgeCost;
                shadowTotalSupply += mint;
                shadowTotalMinted += mint;
              }
              log(`[TRANSFER] ${amt} (edge=${edgeCost})`);
            }
          );
        } else if (action === 2) { // UNREGISTER
          if (registeredUsers.length === 0) continue;
          const idx = Math.floor(random() * registeredUsers.length);
          const user = registeredUsers[idx];
          const userStr = user.publicKey.toBase58();
          const balance = shadowBalances.get(userStr)!;

          let predicted: string | null = null;
          if (balance < 0n) predicted = "DebtOnExitNotAllowed";
          else if (balance > shadowTotalSupply) predicted = "SupplyUnderflow";

          await safeExec(
            () => program.methods.unregister().accounts({
              coreState: coreState.publicKey,
              userAccount: getUserPda(user.publicKey),
              gate: gate.publicKey,
            }).rpc(),
            predicted,
            () => {
              shadowTotalBaseSum = shadowTotalBaseSum - balance + shadowGlobalField;
              shadowP -= 1n;
              if (balance > 0n) {
                shadowTotalSupply -= balance;
                shadowTotalBurned += balance;
              }
              shadowBalances.delete(userStr);
              registeredUsers.splice(idx, 1);
              unregisteredUsers.push(user);
              log(`[UNREGISTER] ${userStr.slice(0, 8)}`);
            }
          );
        } else if (action === 3) { // REDISTRIBUTE
          if (shadowP === 0n) continue;
          const amt = BigInt(Math.floor(random() * 15000) + 1);
          const amountBn = new BN(amt.toString());

          const predicted = shadowP === 0n ? "ZeroParticipants" : null;

          await safeExec(
            () => program.methods.redistribute(amountBn).accounts({
              coreState: coreState.publicKey,
              gate: gate.publicKey,
            }).rpc(),
            predicted,
            () => {
              const total = amt + shadowDustAccumulator;
              const q = total / shadowP;
              const r = total % shadowP;
              shadowGlobalField += q;
              shadowTotalSupply += q * shadowP;
              shadowTotalMinted += q * shadowP;
              shadowDustAccumulator = r;
              log(`[REDISTRIBUTE] ${amt} (dust=${r})`);
            }
          );
        } else if (action === 4) { // NEGATIVE ENTROPY
          const predicted = shadowP > I128_MAX / -NEG_E ? "PhysicalOverflowLimit" : null;

          await safeExec(
            () => program.methods.applyNegEntropy().accounts({
              coreState: coreState.publicKey,
              gate: gate.publicKey,
            }).rpc(),
            predicted,
            () => {
              shadowGlobalField += NEG_E;
              shadowTotalBaseSum -= NEG_E * shadowP;
              log("[NEG_ENTROPY]");
            }
          );
        } else if (action === 5) { // SECURITY
          if (registeredUsers.length === 0) continue;
          const victim = registeredUsers[0];

          const unregisterIx = await program.methods.unregister().accounts({
            coreState: coreState.publicKey,
            userAccount: getUserPda(victim.publicKey),
            gate: gate.publicKey,
          }).instruction();
          await trySpoofSigner(unregisterIx);

          const fakeEdge = Keypair.generate().publicKey;
          const fakeTransferIx = await program.methods.transfer(new BN(100)).accounts({
            coreState: coreState.publicKey,
            fromUser: getUserPda(victim.publicKey),
            toUser: getUserPda(victim.publicKey),
            fromAuthority: victim.publicKey,
            toAuthority: victim.publicKey,
            edgeAccount: fakeEdge,
          }).instruction();
          await tryFakePda(fakeTransferIx);

          log("[SECURITY] Authority & PDA attacks tested");
        }

        await verifyInvariants();
      }
    } catch (e: any) {
      console.error("\n=== 10/10 TEST FAILED ===\nHistory:");
      console.error(history.join("\n"));
      throw e;
    }
  });

  // ============================================================================
  // ULTIMATE STRESS
  // ============================================================================
  it("Ultimate Stress: Extreme Entropy + Debt + Malicious Exit", async () => {
    while (registeredUsers.length < 3) {
      const u = unregisteredUsers.pop()!;
      await program.methods.register(u.publicKey).accounts({
        coreState: coreState.publicKey,
        userAccount: getUserPda(u.publicKey),
        gate: gate.publicKey,
        systemProgram: SystemProgram.programId,
      }).rpc();
      registeredUsers.push(u);
      shadowBalances.set(u.publicKey.toBase58(), 0n);
      shadowP += 1n;
      shadowTotalBaseSum -= shadowGlobalField;
    }

    const debtor = registeredUsers[0];
    const receiver = registeredUsers[1];

    await program.methods.transfer(new BN(40000)).accounts({
      coreState: coreState.publicKey,
      fromUser: getUserPda(debtor.publicKey),
      toUser: getUserPda(receiver.publicKey),
      fromAuthority: debtor.publicKey,
      toAuthority: receiver.publicKey,
      edgeAccount: null,
    }).signers([debtor]).rpc();

    shadowBalances.set(debtor.publicKey.toBase58(), shadowBalances.get(debtor.publicKey.toBase58())! - 40000n);

    for (let i = 0; i < 300; i++) {
      try {
        await program.methods.applyNegEntropy().accounts({ coreState: coreState.publicKey, gate: gate.publicKey }).rpc();
        shadowGlobalField += NEG_E;
        shadowTotalBaseSum -= NEG_E * shadowP;
      } catch (e: any) {
        if (!e.message.includes("PhysicalOverflowLimit") && !e.message.includes("MathOverflow")) throw e;
        break;
      }
    }

    try {
      await program.methods.unregister().accounts({
        coreState: coreState.publicKey,
        userAccount: getUserPda(debtor.publicKey),
        gate: gate.publicKey,
      }).rpc();
      assert.fail("CRITICAL: Unregister with negative balance succeeded!");
    } catch (e: any) {
      assert.include(e.message, "DebtOnExitNotAllowed");
    }

    await verifyInvariants();
    log("ULTIMATE STRESS PASSED");
  });
});