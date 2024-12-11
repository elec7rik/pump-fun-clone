import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PumpFun } from "../target/types/pump_fun";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { SystemProgram } from "@solana/web3.js";
import { assert } from "chai";

describe("PumpFun Token Program", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.PumpFun as Program<PumpFun>;
  
  // Test accounts
  let tokenMint: anchor.web3.PublicKey;
  let tokenMetadata: anchor.web3.PublicKey;
  let bondingCurve: anchor.web3.PublicKey;
  let programConfig: anchor.web3.PublicKey;
  let feeCollector: anchor.web3.Keypair;

  // Test data
  const TOKEN_NAME = "Test Token";
  const TOKEN_SYMBOL = "TEST";
  const TOKEN_DESCRIPTION = "Test Description";
  const TOKEN_IMAGE = "https://picsum.photos/id/237/200/300";
  const INITIAL_SUPPLY = new anchor.BN(1_000_000);

  before(async () => {
    // Generate necessary keypairs
    feeCollector = anchor.web3.Keypair.generate();
    
    // Initialize program config
    const programConfigKeypair = anchor.web3.Keypair.generate();
    programConfig = programConfigKeypair.publicKey;
    
    await program.methods
      .initializeProgramConfig(
        feeCollector.publicKey,
        provider.wallet.publicKey
      )
      .accounts({
        authority: provider.wallet.publicKey,
        programConfig: programConfig,
        systemProgram: SystemProgram.programId,
      })
      .signers([programConfigKeypair])
      .rpc();
  });

  it("Creates a new token with correct metadata", async () => {
    // Generate token mint
    const mintKeypair = anchor.web3.Keypair.generate();
    tokenMint = mintKeypair.publicKey;

    // Derive PDAs
    [tokenMetadata] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("metadata"), tokenMint.toBuffer()],
      program.programId
    );

    [bondingCurve] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("curve"), tokenMint.toBuffer()],
      program.programId
    );

    await program.methods
      .createToken(
        TOKEN_NAME,
        TOKEN_SYMBOL,
        TOKEN_DESCRIPTION,
        TOKEN_IMAGE,
        INITIAL_SUPPLY
      )
      .accounts({
        authority: provider.wallet.publicKey,
        programConfig: programConfig,
        tokenMint: tokenMint,
        tokenMetadata: tokenMetadata,
        bondingCurve: bondingCurve,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([mintKeypair])
      .rpc();

    // Verify metadata
    const metadata = await program.account.tokenMetadata.fetch(tokenMetadata);
    assert.equal(metadata.name, TOKEN_NAME);
    assert.equal(metadata.symbol, TOKEN_SYMBOL);
    assert.equal(metadata.description, TOKEN_DESCRIPTION);
    assert.equal(metadata.imageUrl, TOKEN_IMAGE);
    assert.ok(metadata.creator.equals(provider.wallet.publicKey));
  });
});
