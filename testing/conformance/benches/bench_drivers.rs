extern crate criterion;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use conformance_tests::test_driver::*;
use conformance_tests::vector::{ApplyMessage, MessageVector, Selector, TestVector, Variant};
use conformance_tests::vm::{TestKernel, TestMachine};
use criterion::*;
use fvm::executor::{ApplyKind, DefaultExecutor, Executor};
use fvm_shared::address::Protocol;
use fvm_shared::blockstore::MemoryBlockstore;
use fvm_shared::crypto::signature::SECP_SIG_LEN;
use fvm_shared::encoding::Cbor;
use fvm_shared::message::Message;

/// Applies a list of messages to the VM. Panics if one fails, but this is okay because the caller will test with these messages first.
///
/// # Arguments
///
/// * `messages` - mutable vector of (message, usize) tuples with the message and its raw length. will be removed from vector and applied in order
/// * `exec` - test executor
pub fn apply_messages(
    messages: &mut Vec<(Message, usize)>,
    exec: &mut DefaultExecutor<TestKernel>,
) {
    // Apply all messages in the vector.
    for (msg, raw_length) in messages.drain(..) {
        // Execute the message.
        // can assume this works because it passed a test before this ran
        exec.execute_message(msg, ApplyKind::Explicit, raw_length).unwrap();
    }
}

/// Benches one vector variant using criterion. Clones `MessageVector`, clones `Blockstore`, clones a prepared list of message bytes with lengths, creates a new machine, initializes its wasm cache by loading some code, creates an executor, then times applying the messages.
/// Currently needs some serious speedup, probably with respect to WASM caching and also machine setup/teardown.
pub fn bench_vector_variant(
    group: &mut BenchmarkGroup<measurement::WallTime>,
    name: String,
    variant: &Variant,
    vector: &MessageVector,
    messages_with_lengths: Vec<(Message, usize)>,
    bs: &MemoryBlockstore,
) {
    group.bench_function(name, move |b| {
        b.iter_batched_ref(
            || {
                let vector = &(*vector).clone();
                let bs = bs.clone();
                // TODO next few lines don't impact the benchmarks, but it might make them run waaaay more slowly... ought to make a base copy of the machine and exec and deepcopy them each time.
                let machine = TestMachine::new_for_vector(vector, variant, bs);
                // can assume this works because it passed a test before this ran
                machine.load_builtin_actors_modules().unwrap();
                let exec: DefaultExecutor<TestKernel> = DefaultExecutor::new(machine);
                (messages_with_lengths.clone(), exec)
            },
            |(messages, exec)| apply_messages(criterion::black_box(messages), exec),
            BatchSize::LargeInput,
        )
    });
}
/// This tells `bench_vector_file` how hard to do checks on whether things succeed before running benchmark
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum CheckStrength {
    FullTest,
    OnlyCheckSuccess,
    NoChecks,
}

pub fn bench_vector_file(
    group: &mut BenchmarkGroup<measurement::WallTime>,
    vector_path: PathBuf,
    replacement_apply_messages: Option<Vec<ApplyMessage>>,
    only_first_variant: bool,
    override_name: Option<String>,
    check_strength: CheckStrength,
) -> anyhow::Result<Vec<VariantResult>> {
    let file = File::open(&vector_path)?;
    let reader = BufReader::new(file);
    let vector: TestVector = serde_json::from_reader(reader)?;

    let TestVector::Message(mut vector) = vector;
    let skip = !vector.selector.as_ref().map_or(true, Selector::supported);
    if skip {
        return Ok(vector
            .preconditions
            .variants
            .iter()
            .map(|variant| VariantResult::Skipped {
                reason: "selector not supported.".parse().unwrap(),
                id: variant.id.clone(),
            })
            .collect());
    }

    if let Some(replacement_apply_messages) = replacement_apply_messages {
        vector.apply_messages = replacement_apply_messages;
    }
    if only_first_variant {
        vector.preconditions.variants = vec![vector.preconditions.variants[0].clone()];
    }

    let (bs, _) = async_std::task::block_on(vector.seed_blockstore()).unwrap();

    let mut ret = vec![];
    for variant in vector.preconditions.variants.iter() {
        let name = format!("{} | {}", vector_path.display(), variant.id);
        // this tests the variant before we run the benchmark and record the bench results to disk.
        // if we broke the test, it's not a valid optimization :P
        let testresult = match check_strength {
            CheckStrength::FullTest => run_variant(bs.clone(), &vector, variant, true)?,
            CheckStrength::OnlyCheckSuccess => {
                run_variant(bs.clone(), &vector, variant, false)?
            }
            CheckStrength::NoChecks => VariantResult::Ok {
                id: format!("{}: ATTENTION test not run!!", variant.id),
            },
        };
        let messages_with_lengths: Vec<(Message, usize)> = vector
            .apply_messages
            .iter()
            .map(|m| {
                let unmarshalled = Message::unmarshal_cbor(&m.bytes).unwrap();
                let mut raw_length = m.bytes.len();
                if unmarshalled.from.protocol() == Protocol::Secp256k1 {
                    // 65 bytes signature + 1 byte type + 3 bytes for field info.
                    raw_length += SECP_SIG_LEN + 4;
                }
                (unmarshalled, raw_length)
            })
            .collect();

        if let VariantResult::Ok { .. } = testresult {
            bench_vector_variant(
                group,
                override_name.as_ref().unwrap_or(&name).to_string(),
                variant,
                &vector,
                messages_with_lengths,
                &bs,
            );
        }
        ret.push(testresult);
    }
    Ok(ret)
}
