//! Local real-prove bench for one (scheme, M) point inside a LEZ v0.3.1
//! plan/apply guest (lee_core::program::run_program).
//!
//! By default it proves the **apply** invocation, which is where signature
//! verification runs (mirrors `sequencer_stake::verify_approvals`). `--call plan`
//! proves the plan invocation instead (decode + re-emit the approval set as an effect).
//!
//! Each invocation proves one row, verifies the receipt against the image ID, and
//! appends one CSV line to `--csv`. Run one process per row so that peak RSS can be
//! measured externally (e.g. `/usr/bin/time -v`).
//!
//! Requires `RISC0_DEV_MODE=0` for real numbers.

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use lee_core::{
    account::{AccountId, ProgramShardSelector, ShardData},
    program::{AccountMeta, ApplyInput, CallKind, PlanInput},
    to_borsh_frame, to_frame,
};
use lez_signature_bench::{Scheme, VerifyInput, make_test_vector};
use lez_signature_bench_methods::{
    ECDSA_P256_ELF, ECDSA_P256_ID, ECDSA_SECP256K1_ELF, ECDSA_SECP256K1_ID, ED25519_ELF,
    ED25519_ID, LMS_ELF, LMS_ID, NOOP_ELF, NOOP_ID, SCHNORR_SECP256K1_ELF, SCHNORR_SECP256K1_ID,
};
use risc0_zkvm::{ExecutorEnv, default_prover};

#[derive(Parser, Debug)]
#[command(about = "Real-prove one (scheme, M) point in a LEZ v0.3.1 plan/apply guest.")]
struct Cli {
    /// ecdsa-secp256k1 | schnorr-secp256k1 | ed25519 | ecdsa-p256 | lms | noop
    #[arg(long)]
    scheme: String,
    /// Number of distinct signers (M). For noop, the payload is an ed25519 set of size M.
    #[arg(long)]
    n: usize,
    /// Which LEZ invocation to prove.
    #[arg(long, default_value = "apply")]
    call: String,
    /// Segment limit po2 (risc0 default 20).
    #[arg(long)]
    po2: Option<u32>,
    /// CSV file to append the row to.
    #[arg(long, default_value = "results/raw.csv")]
    csv: PathBuf,
    /// Free-form label (e.g. prover backend) stored in the CSV.
    #[arg(long, default_value = "cpu")]
    prover: String,
}

fn elf_for(slug: &str) -> (&'static [u8], [u32; 8]) {
    match slug {
        "ecdsa-secp256k1" => (ECDSA_SECP256K1_ELF, ECDSA_SECP256K1_ID),
        "schnorr-secp256k1" => (SCHNORR_SECP256K1_ELF, SCHNORR_SECP256K1_ID),
        "ed25519" => (ED25519_ELF, ED25519_ID),
        "ecdsa-p256" => (ECDSA_P256_ELF, ECDSA_P256_ID),
        "lms" => (LMS_ELF, LMS_ID),
        "noop" => (NOOP_ELF, NOOP_ID),
        _ => panic!("unknown scheme {slug}"),
    }
}

fn input_bytes(call: &str, input: &VerifyInput) -> Vec<u8> {
    let self_id = AccountId::new([0x11; 32]);
    let account = AccountId::new([0x22; 32]);
    let mut bytes = Vec::new();
    match call {
        "apply" => {
            let apply = ApplyInput {
                self_account_id: self_id,
                selector: ProgramShardSelector::new(account, self_id),
                pre_data: ShardData::empty(),
                effect_data: borsh::to_vec(input).unwrap(),
            };
            bytes.extend(to_borsh_frame(&CallKind::Apply));
            bytes.extend(to_frame(&borsh::to_vec(&apply).unwrap()));
        }
        "plan" => {
            let plan = PlanInput {
                self_account_id: self_id,
                caller_account_id: None,
                accounts: vec![AccountMeta::new(account, true, self_id)],
                instruction_data: borsh::to_vec(input).unwrap(),
            };
            bytes.extend(to_borsh_frame(&CallKind::Plan));
            bytes.extend(to_frame(&borsh::to_vec(&plan).unwrap()));
        }
        _ => panic!("--call must be apply or plan"),
    }
    bytes
}

fn main() {
    let cli = Cli::parse();
    let dev = std::env::var("RISC0_DEV_MODE").unwrap_or_default();
    if dev == "1" || dev.eq_ignore_ascii_case("true") {
        eprintln!("warning: RISC0_DEV_MODE set — numbers are NOT real measurements.");
    }

    let input = if cli.scheme == "noop" {
        make_test_vector(Scheme::Ed25519, cli.n)
    } else {
        make_test_vector(Scheme::parse(&cli.scheme).expect("scheme"), cli.n)
    };
    let (elf, image_id) = elf_for(&cli.scheme);
    let bytes = input_bytes(&cli.call, &input);

    let mut b = ExecutorEnv::builder();
    b.write_slice(&bytes);
    if let Some(po2) = cli.po2 {
        b.segment_limit_po2(po2);
    }
    let env = b.build().unwrap();

    let t0 = Instant::now();
    let info = default_prover().prove(env, elf).expect("prove");
    let prove_s = t0.elapsed().as_secs_f64();

    let t1 = Instant::now();
    info.receipt.verify(image_id).expect("receipt verify");
    let verify_s = t1.elapsed().as_secs_f64();

    let receipt_bytes = bincode::serialize(&info.receipt).unwrap().len();
    let st = &info.stats;
    let po2 = cli.po2.unwrap_or(20);
    println!(
        "{} M={} call={} total={} user={} paging={} segs={} po2={} prove={:.2}s verify={:.3}s receipt={}B verified=true",
        cli.scheme,
        cli.n,
        cli.call,
        st.total_cycles,
        st.user_cycles,
        st.paging_cycles,
        st.segments,
        po2,
        prove_s,
        verify_s,
        receipt_bytes
    );

    if let Some(dir) = cli.csv.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let new = !cli.csv.exists();
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cli.csv)
        .unwrap();
    if new {
        writeln!(f, "scheme,m,call,prover,po2,total_cycles,user_cycles,paging_cycles,segments,prove_s,verify_s,receipt_bytes,verified").unwrap();
    }
    writeln!(
        f,
        "{},{},{},{},{},{},{},{},{},{:.2},{:.3},{},true",
        cli.scheme,
        cli.n,
        cli.call,
        cli.prover,
        po2,
        st.total_cycles,
        st.user_cycles,
        st.paging_cycles,
        st.segments,
        prove_s,
        verify_s,
        receipt_bytes
    )
    .unwrap();
}
