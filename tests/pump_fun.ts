import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PumpFun } from "../target/types/pump_fun";

describe("pump_fun", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.PumpFun as Program<PumpFun>;

  it("Can create a new token", async () => {
    // Generate a new keypair for the token mint
    const tokenMint = anchor.web3.Keypair.generate();

    // Derive PDA for token metadata
    const [tokenMetadataKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), tokenMint.publicKey.toBuffer()],
      program.programId
    );

    // Derive PDA for bonding curve
    const [bondingCurveKey] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("curve"), tokenMint.publicKey.toBuffer()],
      program.programId
    );

    try {
      const tx = await program.methods
        .createToken(
          "Test Token",      // name
          "TEST",           // symbol
          "Test Description", // description
          "https://example.com/image.png", // image_url
          new anchor.BN(1000000) // initial_supply
        )
        .accounts({
          authority: provider.wallet.publicKey,
          tokenMint: tokenMint.publicKey,
          tokenMetadata: tokenMetadataKey,
          bondingCurve: bondingCurveKey,
          tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .signers([tokenMint])
        .rpc();

      console.log("Transaction signature:", tx);
    } catch (error) {
      console.error("Error:", error);
      throw error;
    }
  });
});
