import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SimpleEscrow } from "../target/types/simple_escrow";
import { assert } from "chai";

// Helper to run transactions with detailed error + log output
async function sendTxWithLogs(
  txPromise: Promise<string>,
  connection: anchor.web3.Connection,
  label: string
): Promise<string> {
  try {
    const sig = await txPromise;
    console.log(`\n✅ [${label}] Transaction successful: ${sig}`);

    const tx = await connection.getTransaction(sig, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });

    if (tx?.meta?.logMessages) {
      console.log("📜 Logs:");
      tx.meta.logMessages.forEach((log) => console.log("   ", log));
    } else {
      console.log("⚠️ No logs found for transaction", sig);
    }

    return sig;
  } catch (e: any) {
    console.error(`\n❌ [${label}] Transaction failed`);
    if (e.logs) {
      console.error("📜 Logs:");
      e.logs.forEach((l: string) => console.error("   ", l));
    } else if (e.simulationResponse?.logs) {
      console.error("📜 Logs:");
      e.simulationResponse.logs.forEach((l: string) => console.error("   ", l));
    } else {
      console.error("⚠️ No logs found in error object.");
    }

    if (e.message) console.error("🧠 Reason:", e.message);
    if (e.toString) console.error("🔍 Error:", e.toString());
    throw e; // rethrow to let Mocha/Anchor test fail
  }
}

describe("simple_escrow (Devnet)", () => {
  const connection = new anchor.web3.Connection("https://api.devnet.solana.com", "confirmed");
  const wallet = anchor.Wallet.local(); // uses your ~/.config/solana/id.json
  const provider = new anchor.AnchorProvider(connection, wallet, {
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);

  const program = anchor.workspace.SimpleEscrow as Program<SimpleEscrow>;

  const initializer = provider.wallet;
  const recipient = anchor.web3.Keypair.generate();

  it("Create Escrow", async () => {
    const escrow = anchor.web3.Keypair.generate();
    const amount = 1_000_000_000; // 1 SOL

    const txSig = await sendTxWithLogs(
      program.methods
        .createEscrow(recipient.publicKey, new anchor.BN(amount))
        .accounts({
          escrow: escrow.publicKey,
          initializer: initializer.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([escrow])
        .rpc(),
      connection,
      "Create Escrow"
    );

    const escrowAccount = await program.account.escrowState.fetch(escrow.publicKey);
    assert.equal(escrowAccount.initializer.toBase58(), initializer.publicKey.toBase58());
    assert.equal(escrowAccount.recipient.toBase58(), recipient.publicKey.toBase58());
    assert.equal(escrowAccount.amount.toNumber(), amount);
    assert.equal(escrowAccount.released, false);
  });

  it("Release Escrow", async () => {
    const escrow = anchor.web3.Keypair.generate();
    const amount = 500_000_000; // 0.5 SOL

    const createTx = await sendTxWithLogs(
      program.methods
        .createEscrow(recipient.publicKey, new anchor.BN(amount))
        .accounts({
          escrow: escrow.publicKey,
          initializer: initializer.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([escrow])
        .rpc(),
      connection,
      "Create Escrow (for Release)"
    );

    const recipientBefore = await provider.connection.getBalance(recipient.publicKey);

    const releaseTx = await sendTxWithLogs(
      program.methods
        .release()
        .accounts({
          escrow: escrow.publicKey,
          recipient: recipient.publicKey,
          initializer: initializer.publicKey,
        })
        .rpc(),
      connection,
      "Release Escrow"
    );

    const recipientAfter = await provider.connection.getBalance(recipient.publicKey);
    const escrowAccount = await program.account.escrowState.fetch(escrow.publicKey);

    assert(escrowAccount.released === true);
    assert(recipientAfter > recipientBefore, "Recipient did not receive funds");
  });

  it("Cancel Escrow", async () => {
    const escrow = anchor.web3.Keypair.generate();
    const amount = 300_000_000; // 0.3 SOL

    const createTx = await sendTxWithLogs(
      program.methods
        .createEscrow(recipient.publicKey, new anchor.BN(amount))
        .accounts({
          escrow: escrow.publicKey,
          initializer: initializer.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([escrow])
        .rpc(),
      connection,
      "Create Escrow (for Cancel)"
    );

    const initBefore = await provider.connection.getBalance(initializer.publicKey);

    const cancelTx = await sendTxWithLogs(
      program.methods
        .cancel()
        .accounts({
          escrow: escrow.publicKey,
          initializer: initializer.publicKey,
        })
        .rpc(),
      connection,
      "Cancel Escrow"
    );

    const escrowAccountInfo = await provider.connection.getAccountInfo(escrow.publicKey);
    const initAfter = await provider.connection.getBalance(initializer.publicKey);

    assert.isNull(escrowAccountInfo, "Escrow account not closed");
    assert(initAfter >= initBefore, "Initializer did not get refunded");
  });
});
