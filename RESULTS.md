# In-guest signature verification cost vs M (LEZ v0.3.1 / RISC Zero 3.0.5)

> Measured by Vacuumlabs for its Logos RFP-005 multisig proposal (Quorate), requirement P.2.
> Single pass per cell, one machine, synthetic fixtures, CPU prover. Not audited.
> **What is proven is the LEZ `apply` call that verifies M approvals, not a full LEZ transaction.**
> The rest of a private execute (proving each private account, the target call, composition) is
> not measured here. Use these numbers for the per-approval cost, not as transaction times.

## TL;DR

- **Proving the `apply` call that verifies M approvals** takes, on this consumer CPU,
  **1.5 min (P-256), 2.3 min (Schnorr-secp256k1) and 7.5 min (Ed25519, RISC0-accelerated fork) at M=5**.
  M=8 takes 3.0 / 3.4 / 11.2 min. **Ed25519 as LEZ v0.3.1 ships it (unpatched curve25519-dalek) takes
  26.7 min at M=5 and 42 min at M=8 (18 / 29 segments).** These are not full-transaction times
  (see Caveats).
- **Cost is linear in M with no measurable fixed overhead beyond the wrapper.** Marginal
  user cycles per signature: **P-256 ≈ 193K, Schnorr-secp256k1 ≈ 269K, Ed25519 (accelerated) ≈ 895K,
  Ed25519 (unpatched, as LEZ v0.3.1 ships it) ≈ 3.51M**.
  Prove time grows with the number of 2^20-cycle segments, at about 90 s per full segment here.
- **LEZ v0.3.1's own `sequencer_stake::verify_approvals` (M-of-N Ed25519 inside `apply`) uses
  crates.io `ed25519-dalek 2.2.0` / `curve25519-dalek 4.1.3` with NO RISC0 patch.** In the
  zkVM that is about 3.9× the cycles of the risc0 `curve25519-4.1.3-risczero.0` fork. If the
  multisig uses Ed25519, it should pin the RISC0 fork (or use P-256/Schnorr).
- GPU proving was **not** available on this machine (details below), so all numbers are CPU.

## Versions

| Item | Value |
|---|---|
| LEZ | `logos-blockchain/logos-execution-zone` tag **v0.3.1** (commit `1a003bfb`), `lee_core` used as the guest/host wrapper |
| risc0-zkvm / risc0-build | **3.0.5 / 3.0.5** (same as LEZ v0.3.1 `Cargo.lock`; also same as upstream bench) |
| risc0-circuit-rv32im / risc0-zkp | 4.0.4 / 3.0.4 (LEZ lock) |
| r0vm / cargo-risczero | 3.0.5 (rzup 0.5.0, installed in `~/.risc0`) |
| Guest toolchain | risc0 rust 1.97.0 (rzup), cpp 2024.1.5 |
| Host rustc | 1.98.1 (LEZ v0.3.1 `rust-toolchain.toml`) |
| sha2 | `risc0/RustCrypto-hashes` tag `sha2-v0.10.9-risczero.0` |
| k256 | `risc0/RustCrypto-elliptic-curves` tag `k256/v0.13.4-risczero.1` |
| p256 | `risc0/RustCrypto-elliptic-curves` tag `p256/v0.13.2-risczero.1` |
| crypto-bigint | `risc0/RustCrypto-crypto-bigint` tag `v0.5.5-risczero.0` |
| ed25519 ("accelerated") | `ed25519-dalek 2.1.1` + `curve25519-dalek 4.1.3` from `risc0/curve25519-dalek` tag `curve25519-4.1.3-risczero.0` |
| ed25519 ("unpatched", LEZ as-is) | crates.io `ed25519-dalek 2.2.0` + `curve25519-dalek 4.1.3` (what LEZ v0.3.1 resolves) |
| borsh | 1.6.1 |

**LEZ v0.3.1 crypto patches:** the LEZ workspace `[patch.crates-io]` contains only
`keccak` → `logos-blockchain/sponges@3a56e99`. There are **no** RISC0 patches for sha2/k256/p256/
crypto-bigint/curve25519. `lez/programs/sequencer_stake` (`verify_approvals`, which calls
`VerifyingKey::verify_strict` per approval inside `apply`) therefore runs software
Ed25519 in the guest. The "accelerated" patch set above is the one the upstream bench uses, and
those patches are compatible with risc0-zkvm 3.0.5.

## Hardware and prover

- AMD Ryzen 7 9800X3D (8C/16T), 15 GiB RAM visible to WSL2 (+4 GiB swap), Linux 6.18 (WSL2).
- GPU: NVIDIA RTX 5070 12 GB (compute capability 12.0, driver 610.88). **Not used.**
- **Prover: CPU**, `risc0_zkvm::default_prover()` → external `r0vm 3.0.5` (the same path the
  upstream bench used), `RISC0_DEV_MODE=0`, segment po2 = 20 (default). Segments are proven
  sequentially. The output is a composite receipt (no succinct/groth16 compression).
- **Why no GPU:** risc0 `cuda` needs `nvcc` (CUDA toolkit) to compile its kernels at build time.
  The rzup-distributed `r0vm` is CPU-only (no libcuda linkage). Only the WSL driver libs
  (`/usr/lib/wsl/lib/libcuda.so`) are present, with no toolkit installed.
  Also note the RTX 5070 is sm_120 (Blackwell), which needs CUDA ≥ 12.8. Check that risc0 3.0.5's
  kernels build for sm_120 before relying on GPU numbers.
- Memory: peak RSS ≈ 9.4 GB for any proof with a full 2^20 segment. No OOM, so po2 was not lowered.

## Methodology

- This repository is a fork of [`fryorcraken/lez-signature-bench`](https://github.com/fryorcraken/lez-signature-bench)
  at `f784e0c`, ported to LEZ v0.3.1. The Ed25519 "unpatched" variant is the same tree with the
  curve25519/ed25519 RISC0 patch removed: `variants/ed25519-unpatched.patch` plus its `Cargo.lock`.
- **Guest = real LEZ v0.3.1 program shape.** Each scheme is a `lee_core::program::run_program(plan, apply)`
  guest. `plan` borsh-decodes the approval set (instruction) and emits it as one effect. `apply`
  borsh-decodes the effect and verifies all M signatures (the same place
  `sequencer_stake::verify_approvals` runs), then keeps the shard unchanged.
- **What is proven per row: the `apply` invocation.** The host writes the exact LEZ framing
  (`to_borsh_frame(CallKind::Apply)` + `to_frame(borsh(ApplyInput))`, which is what
  `lee::program::write_apply_inputs` does), proves it, then **verifies the receipt against the image
  ID** (all rows `verified=true`). Receipt size = `bincode(receipt).len()`. Prove time = wall
  clock around `prove()`. Peak RSS = `/usr/bin/time -v` max RSS (dominated by the r0vm child).
- **Fixture:** M **distinct** signers (deterministic seeds: the 3 upstream seeds plus
  `sha256("lez-signature-bench/seed/"‖i)`), all signing the same message (`b"hello redstone"`).
  Upstream already used distinct keys (3 seeds), but only up to N=3. We extended it to 8 and added a test
  asserting 8 distinct pubkeys. Prehash: Schnorr/P-256 sha256(msg) once, then `verify_prehash` per
  signer. Ed25519 uses `verify_strict(msg)` (changed from upstream's `verify` to mirror LEZ).
- `noop` = the same plan/apply guest, decoding an M-signer Ed25519 approval set and verifying nothing.
  This is the LEZ apply-wrapper baseline. Per-signature cost = (user − noop(M=1)) / M.
- Single pass per cell, machine otherwise idle, rows run sequentially.

## Results

| ed25519 crate | scheme | M | total cycles | user cycles | user − noop | per-sig (user − noop)/M | segments | prove time | receipt (B) | peak RSS (MB) |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| risc0 fork (accelerated) | `noop` | 1 | 65,536 | 7,373 | 0 |  | 1 | 5.8 s (0:05) | 221,690 | 603 |
| risc0 fork (accelerated) | `noop` | 8 | 65,536 | 14,169 | 6,796 |  | 1 | 5.9 s (0:05) | 223,146 | 604 |
| risc0 fork (accelerated) | `schnorr-secp256k1` | 1 | 524,288 | 276,996 | 269,623 | 269,623 | 1 | 52.1 s (0:52) | 268,666 | 4702 |
| risc0 fork (accelerated) | `schnorr-secp256k1` | 3 | 1,048,576 | 817,140 | 809,767 | 269,922 | 1 | 93.9 s (1:33) | 282,330 | 9377 |
| risc0 fork (accelerated) | `schnorr-secp256k1` | 5 | 1,572,864 | 1,357,215 | 1,349,842 | 269,968 | 2 | 136.5 s (2:16) | 550,812 | 9391 |
| risc0 fork (accelerated) | `schnorr-secp256k1` | 8 | 2,359,296 | 2,159,580 | 2,152,207 | 269,025 | 3 | 205.7 s (3:25) | 820,526 | 9391 |
| risc0 fork (accelerated) | `ecdsa-p256` | 1 | 262,144 | 203,604 | 196,231 | 196,231 | 1 | 22.4 s (0:22) | 256,444 | 2359 |
| risc0 fork (accelerated) | `ecdsa-p256` | 3 | 1,048,576 | 587,386 | 580,013 | 193,337 | 1 | 90.0 s (1:30) | 282,336 | 9360 |
| risc0 fork (accelerated) | `ecdsa-p256` | 5 | 1,048,576 | 974,261 | 966,888 | 193,377 | 1 | 90.7 s (1:30) | 282,756 | 9396 |
| risc0 fork (accelerated) | `ecdsa-p256` | 8 | 2,097,152 | 1,547,069 | 1,539,696 | 192,462 | 2 | 181.5 s (3:01) | 564,700 | 9398 |
| risc0 fork (accelerated) | `ed25519` | 1 | 1,048,576 | 904,711 | 897,338 | 897,338 | 1 | 90.0 s (1:30) | 281,914 | 9384 |
| risc0 fork (accelerated) | `ed25519` | 3 | 3,145,728 | 2,700,446 | 2,693,073 | 897,691 | 3 | 271.6 s (4:31) | 844,958 | 9388 |
| risc0 fork (accelerated) | `ed25519` | 5 | 5,242,880 | 4,484,210 | 4,476,837 | 895,367 | 5 | 449.3 s (7:29) | 1,408,002 | 9388 |
| risc0 fork (accelerated) | `ed25519` | 8 | 7,864,320 | 7,161,917 | 7,154,544 | 894,318 | 8 | 671.9 s (11:11) | 2,239,320 | 9389 |
| crates.io 2.2.0 (as LEZ v0.3.1) | `noop` | 1 | 65,536 | 7,373 | 0 |  | 1 | 5.5 s (0:05) | 221,690 | 604 |
| crates.io 2.2.0 (as LEZ v0.3.1) | `ed25519` | 1 | 4,194,304 | 3,526,968 | 3,519,595 | 3,519,595 | 4 | 357.6 s (5:57) | 1,125,856 | 9385 |
| crates.io 2.2.0 (as LEZ v0.3.1) | `ed25519` | 3 | 11,534,336 | 10,564,988 | 10,557,615 | 3,519,205 | 11 | 984.7 s (16:24) | 3,095,470 | 9397 |
| crates.io 2.2.0 (as LEZ v0.3.1) | `ed25519` | 5 | 18,874,368 | 17,559,376 | 17,552,003 | 3,510,400 | 18 | 1602.6 s (26:42) | 5,065,084 | 9397 |
| crates.io 2.2.0 (as LEZ v0.3.1) | `ed25519` | 8 | 29,622,272 | 28,058,686 | 28,051,313 | 3,506,414 | 29 | 2528.9 s (42:08) | 8,134,690 | 9398 |

Plan invocation (for completeness, M=8 Ed25519 payload): total 65,536 / user 22,058 cycles, 1 segment, prove 5.5 s, receipt 225,016 B, verified.

Raw CSV: `results/raw.csv` (merged), `raw-patched.csv`, `raw-unpatched.csv`, `raw-plan.csv`, `rss.csv`;
per-row logs `results/log-*.txt` (absolute paths in the logs shortened to repository-relative ones).

### Prove time of the apply proof (CPU, this machine)

| M | P-256 | Schnorr-secp256k1 | Ed25519 (risc0 fork) | Ed25519 (unpatched, LEZ as-is) |
|---:|---:|---:|---:|---:|
| 1 | 22 s (0:22), 1 seg | 52 s (0:52), 1 seg | 90 s (1:30), 1 seg | 358 s (5:57), 4 seg |
| 3 | 90 s (1:30), 1 seg | 94 s (1:33), 1 seg | 272 s (4:31), 3 seg | 985 s (16:24), 11 seg |
| 5 | 91 s (1:30), 1 seg | 136 s (2:16), 2 seg | 449 s (7:29), 5 seg | 1603 s (26:42), 18 seg |
| 8 | 182 s (3:01), 2 seg | 206 s (3:25), 3 seg | 672 s (11:11), 8 seg | 2529 s (42:08), 29 seg |

## Comparison to upstream (v0.2.0-rc3, NSSA wrapper, Ryzen 9 7940HS, CPU)

| scheme | N | upstream user cycles | this fork user cycles | upstream prove | this fork prove |
|---|---:|---:|---:|---:|---:|
| noop | 1 | 38,623 | 7,373 | 9.0 s | 5.8 s |
| schnorr-secp256k1 | 1 | 309,274 | 276,996 | 72.5 s | 52.1 s |
| schnorr-secp256k1 | 3 | 903,957 | 817,140 | 146.7 s | 93.9 s |
| ecdsa-p256 | 1 | 236,869 | 203,604 | 69.5 s | 22.4 s |
| ecdsa-p256 | 3 | 677,267 | 587,386 | 142.3 s | 90.0 s |
| ed25519 | 1 | 841,867 | 904,711 | 160.4 s | 90.0 s |
| ed25519 | 3 | 2,501,609 | 2,700,446 | 460.7 s | 271.6 s |

- The **wrapper baseline dropped** from about 38.6K to about 7.4K user cycles. The v0.3 `apply` path
  (two length-prefixed borsh frames in, one borsh `ApplyOutput` committed) is lighter than v0.2's
  `read_nssa_inputs` + `ProgramOutput` with risc0-serde. That accounts for most of the
  ~30K/row drop for Schnorr and P-256.
- **Per-signature cost is essentially unchanged** (same risc0 3.0.5 + same patches):
  Schnorr 270–288K → 269K, P-256 198–213K → 193K. Ed25519 803–821K → 895K. The ~+9% is
  is most likely from switching `verify` → `verify_strict` (extra small-order and canonicity checks).
  The crate pin (2.1.1 fork) is identical to upstream's. We didn't isolate it.
- Prove times are 1.3–3× faster than upstream purely due to hardware (9800X3D vs 7940HS) and
  segment-size rounding (P-256 N=1 fits in a 2^18 segment here). Cycles are the portable metric.

## Caveats (read before quoting)

1. **LEZ wrapper: included (apply path only).** The guest is a genuine `lee_core` v0.3.1
   plan/apply program, and the measured proof is the `apply` call. **Not included:** the `plan`
   proof (cheap, ~1 segment, reported above), the LEE state-machine composition, the
   privacy-preserving circuit that wraps program receipts for private transactions, succinct/groth16 compression, the sequencer roundtrip, and the
   committee-membership and duplicate-signer checks `verify_approvals` also does (those are O(M·N) cheap
   comparisons). **This is not a full private LEZ transaction.**
2. Message is 14 bytes. `sequencer_stake`'s `slash_approval_message` is ~128 bytes, which adds roughly
   one extra SHA-512 block per Ed25519 signature (small next to the curve work).
3. Single pass, no repetition. Upstream saw ±10% prove-time run-to-run variance.
4. CPU only, segments proven sequentially. Prove time is roughly segments × ~90 s here. A GPU
   or parallel/remote segment proving would change wall-clock time dramatically but not cycles.
5. No batch verification (Ed25519/Schnorr batch could reduce per-sig cost). This is naive M× verify.
6. ecdsa-secp256k1 and LMS were not re-run (out of scope for the M sweep). See upstream for N=1/3.
