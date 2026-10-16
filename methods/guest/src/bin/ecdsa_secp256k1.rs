//! LEZ v0.3.1 plan/apply guest. `plan` turns the instruction (the approval set) into
//! one effect on the first account; `apply` verifies every signature in that effect,
//! mirroring where `sequencer_stake::verify_approvals` runs (inside `apply`).
use lee_core::program::{Plan, PlanInput, run_program};
use lez_signature_bench_programs::VerifyInput;
#[allow(unused_imports)]
use lez_signature_bench_programs::verifier;

fn plan(input: &PlanInput, instruction: VerifyInput) -> Plan {
    let mut plan = Plan::new(input);
    plan.effect(&input.accounts[0], &instruction);
    plan
}

fn apply(effect: VerifyInput, _pre_data: &[u8]) -> Option<Vec<u8>> {
    verifier::ecdsa_k256::verify_all(&effect).expect("verify failed");
    None
}

fn main() {
    run_program(plan, apply)
}
