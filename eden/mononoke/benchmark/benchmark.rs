/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

//! This benchmark generates linear stack with specified parameters, and then
//! measures how log it takes to convert it from Bonsai to Hg.

#![deny(warnings)]

use anyhow::{bail, format_err, Error, Result};
use benchmark_lib::{new_benchmark_repo, GenManifest};
use blobrepo::BlobRepo;
use clap::{App, Arg};
use cmdlib::args;
use context::CoreContext;
use derived_data::BonsaiDerived;
use fbinit::FacebookInit;
use fsnodes::RootFsnodeId;
use futures::compat::Future01CompatExt;
use futures_ext::{BoxFuture as OldBoxFuture, FutureExt as OldFutureExt};
use futures_old::Future;
use futures_stats::futures03::TimedFutureExt;
use mononoke_types::ChangesetId;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use std::sync::Arc;
use tokio_compat::runtime::Runtime;
use unodes::RootUnodeManifestId;

const HG_CHANGESET_TYPE: &str = "hg-changeset";
const ARG_SEED: &str = "seed";
const ARG_TYPE: &str = "type";
const ARG_STACK_SIZE: &str = "stack-size";

type DeriveFn = Arc<dyn Fn(ChangesetId) -> OldBoxFuture<String, Error> + Send + Sync + 'static>;

async fn run(
    ctx: CoreContext,
    repo: BlobRepo,
    rng_seed: u64,
    stack_size: usize,
    derive: DeriveFn,
) -> Result<(), Error> {
    println!("rng seed: {}", rng_seed);
    let mut rng = XorShiftRng::seed_from_u64(rng_seed); // reproducable Rng

    let mut gen = GenManifest::new();
    let settings = Default::default();
    let (stats, csidq) = gen
        .gen_stack(
            ctx,
            repo,
            &mut rng,
            &settings,
            None,
            std::iter::repeat(16).take(stack_size),
        )
        .compat()
        .timed()
        .await;
    println!("stack generated: {:?} {:?}", gen.size(), stats);

    let csid = csidq?;
    let (stats2, result) = derive(csid).compat().timed().await;
    println!("bonsai conversion: {:?}", stats2);
    println!("{:?} -> {:?}", csid.to_string(), result);
    Ok(())
}

fn derive_fn(ctx: CoreContext, repo: BlobRepo, derive_type: Option<&str>) -> Result<DeriveFn> {
    match derive_type {
        None => bail!("required `type` argument is missing"),
        Some(HG_CHANGESET_TYPE) => {
            let derive_hg_changeset = move |csid| {
                repo.get_hg_from_bonsai_changeset(ctx.clone(), csid)
                    .map(|hg_csid| hg_csid.to_string())
                    .boxify()
            };
            Ok(Arc::new(derive_hg_changeset))
        }
        Some(RootUnodeManifestId::NAME) => {
            let derive_unodes = move |csid| {
                RootUnodeManifestId::derive(ctx.clone(), repo.clone(), csid)
                    .from_err()
                    .map(|root| root.manifest_unode_id().to_string())
                    .boxify()
            };
            Ok(Arc::new(derive_unodes))
        }
        Some(RootFsnodeId::NAME) => {
            let derive_fsnodes = move |csid| {
                RootFsnodeId::derive(ctx.clone(), repo.clone(), csid)
                    .from_err()
                    .map(|root| root.fsnode_id().to_string())
                    .boxify()
            };
            Ok(Arc::new(derive_fsnodes))
        }
        Some(derived_type) => Err(format_err!("unknown derived data type: {}", derived_type)),
    }
}

#[fbinit::main]
fn main(fb: FacebookInit) -> Result<()> {
    let app = {
        let app = App::new("mononoke benchmark")
            .arg(
                Arg::with_name(ARG_SEED)
                    .short("s")
                    .long(ARG_SEED)
                    .takes_value(true)
                    .value_name(ARG_SEED)
                    .help("seed changeset generator for u64 seed"),
            )
            .arg(
                Arg::with_name(ARG_STACK_SIZE)
                    .long(ARG_STACK_SIZE)
                    .takes_value(true)
                    .value_name(ARG_STACK_SIZE)
                    .help("Size of the generated stack"),
            )
            .arg(
                Arg::with_name(ARG_TYPE)
                    .required(true)
                    .index(1)
                    .possible_values(&[
                        HG_CHANGESET_TYPE,
                        RootUnodeManifestId::NAME,
                        RootFsnodeId::NAME,
                    ])
                    .help("derived data type"),
            );
        let app = args::add_logger_args(app);
        args::add_cachelib_args(app, true /* hide_advanced_args */)
    };
    let matches = app.get_matches();

    args::init_cachelib(fb, &matches, None);
    let logger = args::init_logging(fb, &matches);
    let ctx = CoreContext::new_with_logger(fb, logger.clone());
    let repo = new_benchmark_repo(fb, Default::default())?;

    let seed = matches
        .value_of(ARG_SEED)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or_else(|| rand::random());

    let stack_size: usize = matches
        .value_of(ARG_STACK_SIZE)
        .unwrap_or("50")
        .parse()
        .expect("stack size must be a positive integer");

    let derive = derive_fn(ctx.clone(), repo.clone(), matches.value_of(ARG_TYPE))?;

    let mut runtime = Runtime::new()?;
    runtime.block_on_std(run(ctx, repo, seed, stack_size, derive))
}
