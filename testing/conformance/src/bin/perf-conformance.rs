// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::env::var;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path};

use conformance_tests::driver::*;
use conformance_tests::vector::{Selector, TestVector};

fn main() {
    let my_path = match var("VECTOR") {
        Ok(v) => Path::new(v.as_str()).to_path_buf(),
        Err(_) => panic!("what are you perfing??"),
    };

    let file = File::open(&my_path).unwrap();
    let reader = BufReader::new(file);
    let vector: TestVector = serde_json::from_reader(reader).unwrap();

    if let TestVector::Message(vector) = vector {
        let skip = !vector.selector.as_ref().map_or(true, Selector::supported);
        if skip {
            println!("skipping because selector not supported");
            return;
        }
        let (bs, _) = async_std::task::block_on(vector.seed_blockstore()).unwrap();
        for variant in vector.preconditions.variants.iter() {
            // this tests the variant before we run the benchmark and record the bench results to disk.
            // if we broke the test, it's not a valid optimization :P
            let testresult = run_variant(bs.clone(), &vector, variant, false, true)
                .map_err(|e| {
                    anyhow::anyhow!("run_variant failed (probably a test parsing bug): {}", e)
                }).unwrap();

            if let VariantResult::Ok { .. } = testresult {
                continue;
            } else {
                panic!("a test failed during running");
            }
        }
    } else {
        println!("what did you just hand my code :(");
    }
}