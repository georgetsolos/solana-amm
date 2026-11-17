
use litesvm::LiteSVM;
use litesvm_token::{
    get_spl_account,
    spl_token::state::{Account as SplAccount, Mint as SplMint},
    CreateAssociatedTokenAccount, CreateMint, MintTo, TOKEN_ID,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::str::FromStr;

const PROGRAM_ID: &str = "uRMgTCNZMr6KURp4KVsAWXb1SU77TFdrDwuMqTr8LcA";

const ASSOCIATED_TOKEN_PROGRAM_ID: &str =
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

const INITIAL_TOKEN_AMOUNT: u64 = 2_000_000_000;
const INITIAL_LIQUIDITY: u64 = 1_000_000_000;

fn address(value: &str) -> Address {
    Address::from_str(value).unwrap()
}

fn derive_pda(seeds: &[&[u8]], program_id: &Address) -> Address {
    Address::find_program_address(seeds, program_id).0
}

fn associated_token_address(owner: &Address, mint: &Address) -> Address {
    spl_associated_token_account_interface::address::
        get_associated_token_address_with_program_id(
            owner,
            mint,
            &TOKEN_ID,
        )
}

fn instruction_data(name: &str, args: &[u8]) -> Vec<u8> {
    let hash = Sha256::digest(format!("global:{name}").as_bytes());

    let mut data = hash[..8].to_vec();
    data.extend_from_slice(args);
    data
}

fn writable(address: Address, signer: bool) -> AccountMeta {
    AccountMeta::new(address, signer)
}

fn readonly(address: Address, signer: bool) -> AccountMeta {
    AccountMeta::new_readonly(address, signer)
}

fn submit(
    svm: &mut LiteSVM,
    payer: &Keypair,
    extra_signers: &[&Keypair],
    instruction: Instruction,
) {
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra_signers);

    let payer_address = payer.pubkey();

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer_address),
        &signers,
        svm.latest_blockhash(),
    );

    svm.send_transaction(transaction).unwrap();
}

#[test]
fn test_complete_amm_lifecycle() {
    let program_id = address(PROGRAM_ID);
    let associated_token_program = address(ASSOCIATED_TOKEN_PROGRAM_ID);
    let system_program = address(SYSTEM_PROGRAM_ID);

    let mut svm = LiteSVM::new();

    // Load the unstripped SBPF release artifact.
    svm.add_program_from_file(
        program_id,
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/sbpfv3-solana-solana/release/solana_amm.so"
        ),
    )
    .expect("Could not load the AMM program");

    let payer = Keypair::new();
    let treasury_authority = Keypair::new();

    let payer_address = payer.pubkey();
    let treasury_address = treasury_authority.pubkey();

    svm.airdrop(&payer_address, 10_000_000_000)
        .unwrap();

    // Create token mints.
    let mint_a = CreateMint::new(&mut svm, &payer)
        .authority(&payer_address)
        .decimals(6)
        .send()
        .expect("Could not create mint A");

    let mint_b = CreateMint::new(&mut svm, &payer)
        .authority(&payer_address)
        .decimals(6)
        .send()
        .expect("Could not create mint B");

    assert_ne!(mint_a, mint_b);

    // Create user token accounts.
    let user_token_a = CreateAssociatedTokenAccount::new(
        &mut svm,
        &payer,
        &mint_a,
    )
    .owner(&payer_address)
    .send()
    .expect("Could not create user token A account");

    let user_token_b = CreateAssociatedTokenAccount::new(
        &mut svm,
        &payer,
        &mint_b,
    )
    .owner(&payer_address)
    .send()
    .expect("Could not create user token B account");

    // Mint initial tokens to the user.
    MintTo::new(
        &mut svm,
        &payer,
        &mint_a,
        &user_token_a,
        INITIAL_TOKEN_AMOUNT,
    )
    .owner(&payer)
    .send()
    .expect("Could not mint token A");

    MintTo::new(
        &mut svm,
        &payer,
        &mint_b,
        &user_token_b,
        INITIAL_TOKEN_AMOUNT,
    )
    .owner(&payer)
    .send()
    .expect("Could not mint token B");

    // Derive AMM PDAs.
    let pool = derive_pda(
        &[b"pool", mint_a.as_ref(), mint_b.as_ref()],
        &program_id,
    );

    let authority = derive_pda(
        &[b"authority", pool.as_ref()],
        &program_id,
    );

    let lp_mint = derive_pda(
        &[b"lp_mint", pool.as_ref()],
        &program_id,
    );

    // Derive token accounts.
    let vault_a = associated_token_address(&authority, &mint_a);
    let vault_b = associated_token_address(&authority, &mint_b);

    let treasury_a = associated_token_address(&treasury_address, &mint_a);
    let treasury_b = associated_token_address(&treasury_address, &mint_b);

    // ---------------------------------------------------------
    // 1. INITIALIZE AMM POOL
    // ---------------------------------------------------------

    let initialize_accounts = vec![
        writable(payer_address, true),
        readonly(treasury_address, true),
        readonly(mint_a, false),
        readonly(mint_b, false),
        writable(pool, false),
        readonly(authority, false),
        writable(vault_a, false),
        writable(vault_b, false),
        writable(lp_mint, false),
        writable(treasury_a, false),
        writable(treasury_b, false),
        readonly(TOKEN_ID, false),
        readonly(associated_token_program, false),
        readonly(system_program, false),
    ];

    let initialize_instruction = Instruction {
        program_id,
        accounts: initialize_accounts,
        data: instruction_data("initialize", &[]),
    };

    submit(
        &mut svm,
        &payer,
        &[&treasury_authority],
        initialize_instruction,
    );

    println!("PASS: AMM pool initialization");

    // Create user's LP token account.
    let user_lp_token = CreateAssociatedTokenAccount::new(
        &mut svm,
        &payer,
        &lp_mint,
    )
    .owner(&payer_address)
    .send()
    .expect("Could not create LP token account");

    // ---------------------------------------------------------
    // 2. DEPOSIT LIQUIDITY
    // ---------------------------------------------------------

    let mut deposit_args = Vec::new();
    deposit_args.extend_from_slice(&INITIAL_LIQUIDITY.to_le_bytes());
    deposit_args.extend_from_slice(&INITIAL_LIQUIDITY.to_le_bytes());

    let deposit_accounts = vec![
        writable(payer_address, true),
        writable(pool, false),
        readonly(authority, false),
        readonly(mint_a, false),
        readonly(mint_b, false),
        writable(vault_a, false),
        writable(vault_b, false),
        writable(user_token_a, false),
        writable(user_token_b, false),
        writable(lp_mint, false),
        writable(user_lp_token, false),
        readonly(TOKEN_ID, false),
    ];

    let deposit_instruction = Instruction {
        program_id,
        accounts: deposit_accounts,
        data: instruction_data("deposit", &deposit_args),
    };

    submit(&mut svm, &payer, &[], deposit_instruction);

    let lp_account: SplAccount =
        get_spl_account(&svm, &user_lp_token).unwrap();

    assert_eq!(lp_account.amount, INITIAL_LIQUIDITY);

    let mint_after_deposit: SplMint =
        get_spl_account(&svm, &lp_mint).unwrap();

    assert_eq!(mint_after_deposit.supply, INITIAL_LIQUIDITY);

    println!("PASS: liquidity deposit and LP minting");

    // ---------------------------------------------------------
    // 3. SWAP TOKEN A FOR TOKEN B
    // ---------------------------------------------------------

    let swap_a_to_b_accounts = vec![
        writable(payer_address, true),
        readonly(pool, false),
        readonly(authority, false),
        readonly(mint_a, false),
        readonly(mint_b, false),
        writable(vault_a, false),
        writable(vault_b, false),
        writable(treasury_a, false),
        writable(treasury_b, false),
        writable(user_token_a, false),
        writable(user_token_b, false),
        readonly(TOKEN_ID, false),
    ];

    let mut swap_args = Vec::new();
    swap_args.extend_from_slice(&100_000_000u64.to_le_bytes());
    swap_args.extend_from_slice(&1u64.to_le_bytes());
    swap_args.push(1);

    let swap_a_to_b_instruction = Instruction {
        program_id,
        accounts: swap_a_to_b_accounts,
        data: instruction_data("swap", &swap_args),
    };

    submit(&mut svm, &payer, &[], swap_a_to_b_instruction);

    let treasury_a_account: SplAccount =
        get_spl_account(&svm, &treasury_a).unwrap();

    assert!(treasury_a_account.amount > 0);

    println!("PASS: A-to-B swap and protocol fee");

    // ---------------------------------------------------------
    // 4. SWAP TOKEN B FOR TOKEN A
    // ---------------------------------------------------------

    let mut reverse_swap_args = Vec::new();
    reverse_swap_args.extend_from_slice(&50_000_000u64.to_le_bytes());
    reverse_swap_args.extend_from_slice(&1u64.to_le_bytes());
    reverse_swap_args.push(0);

    let swap_b_to_a_instruction = Instruction {
        program_id,
        accounts: vec![
            writable(payer_address, true),
            readonly(pool, false),
            readonly(authority, false),
            readonly(mint_a, false),
            readonly(mint_b, false),
            writable(vault_a, false),
            writable(vault_b, false),
            writable(treasury_a, false),
            writable(treasury_b, false),
            writable(user_token_a, false),
            writable(user_token_b, false),
            readonly(TOKEN_ID, false),
        ],
        data: instruction_data("swap", &reverse_swap_args),
    };

    submit(&mut svm, &payer, &[], swap_b_to_a_instruction);

    let treasury_b_account: SplAccount =
        get_spl_account(&svm, &treasury_b).unwrap();

    assert!(treasury_b_account.amount > 0);

    println!("PASS: B-to-A swap and protocol fee");

    // ---------------------------------------------------------
    // 5. WITHDRAW LIQUIDITY
    // ---------------------------------------------------------

    let mut withdraw_args = Vec::new();
    withdraw_args.extend_from_slice(&500_000_000u64.to_le_bytes());
    withdraw_args.extend_from_slice(&1u64.to_le_bytes());
    withdraw_args.extend_from_slice(&1u64.to_le_bytes());

    let withdraw_instruction = Instruction {
        program_id,
        accounts: vec![
            writable(payer_address, true),
            writable(pool, false),
            readonly(authority, false),
            readonly(mint_a, false),
            readonly(mint_b, false),
            writable(vault_a, false),
            writable(vault_b, false),
            writable(user_token_a, false),
            writable(user_token_b, false),
            writable(lp_mint, false),
            writable(user_lp_token, false),
            readonly(TOKEN_ID, false),
        ],
        data: instruction_data("withdraw", &withdraw_args),
    };

    submit(&mut svm, &payer, &[], withdraw_instruction);

    let lp_account_after: SplAccount =
        get_spl_account(&svm, &user_lp_token).unwrap();

    assert_eq!(lp_account_after.amount, 500_000_000);

    let mint_after_withdraw: SplMint =
        get_spl_account(&svm, &lp_mint).unwrap();

    assert_eq!(mint_after_withdraw.supply, 500_000_000);

    let user_a_after: SplAccount =
        get_spl_account(&svm, &user_token_a).unwrap();

    let user_b_after: SplAccount =
        get_spl_account(&svm, &user_token_b).unwrap();

    assert!(user_a_after.amount > 0);
    assert!(user_b_after.amount > 0);

    println!("PASS: liquidity withdrawal and LP burning");
    println!("PASS: complete AMM lifecycle");
}
