/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

use crate::error::SubcommandError;

use anyhow::{bail, Error};
use blobrepo::BlobRepo;
use blobstore::Loadable;
use clap::{App, Arg, ArgMatches, SubCommand};
use cloned::cloned;
use cmdlib::{args, helpers};
use context::CoreContext;
use derived_data::BonsaiDerived;
use fbinit::FacebookInit;
use futures::compat::Future01CompatExt;
use futures_ext::{FutureExt, StreamExt};
use futures_old::{future, Future, IntoFuture, Stream};
use manifest::{Entry, ManifestOps, PathOrPrefix};

use mononoke_types::{ChangesetId, MPath};
use revset::AncestorsNodeStream;
use slog::Logger;
use std::collections::BTreeSet;
use unodes::RootUnodeManifestId;

pub const UNODES: &str = "unodes";
const COMMAND_TREE: &str = "tree";
const COMMAND_VERIFY: &str = "verify";
const ARG_CSID: &str = "csid";
const ARG_PATH: &str = "path";
const ARG_LIMIT: &str = "limit";
const ARG_TRACE: &str = "trace";

fn path_resolve(path: &str) -> Result<Option<MPath>, Error> {
    match path {
        "/" => Ok(None),
        _ => Ok(Some(MPath::new(path)?)),
    }
}

pub fn build_subcommand<'a, 'b>() -> App<'a, 'b> {
    let csid_arg = Arg::with_name(ARG_CSID)
        .help("{hg|boinsai} changset id or bookmark name")
        .index(1)
        .required(true);

    let path_arg = Arg::with_name(ARG_PATH)
        .help("path")
        .index(2)
        .default_value("/");

    SubCommand::with_name(UNODES)
        .about("inspect and interact with unodes")
        .arg(
            Arg::with_name(ARG_TRACE)
                .help("upload trace to manifold")
                .long("trace"),
        )
        .subcommand(
            SubCommand::with_name(COMMAND_TREE)
                .help("recursively list all unode entries starting with prefix")
                .arg(csid_arg.clone())
                .arg(path_arg.clone()),
        )
        .subcommand(
            SubCommand::with_name(COMMAND_VERIFY)
                .help("verify unode tree agains hg-manifest")
                .arg(csid_arg.clone())
                .arg(
                    Arg::with_name(ARG_LIMIT)
                        .help("number of commits to be verified")
                        .takes_value(true)
                        .required(true),
                ),
        )
}

pub async fn subcommand_unodes<'a>(
    fb: FacebookInit,
    logger: Logger,
    matches: &'a ArgMatches<'_>,
    sub_matches: &'a ArgMatches<'_>,
) -> Result<(), SubcommandError> {
    let tracing_enable = sub_matches.is_present(ARG_TRACE);
    if tracing_enable {
        tracing::enable();
    }

    args::init_cachelib(fb, &matches, None);

    let repo = args::open_repo(fb, &logger, &matches);
    let ctx = CoreContext::new_with_logger(fb, logger.clone());

    let run = match sub_matches.subcommand() {
        (COMMAND_TREE, Some(matches)) => {
            let hash_or_bookmark = String::from(matches.value_of(ARG_CSID).unwrap());
            let path = path_resolve(matches.value_of(ARG_PATH).unwrap());
            cloned!(ctx);
            (repo, path)
                .into_future()
                .and_then(move |(repo, path)| {
                    helpers::csid_resolve(ctx.clone(), repo.clone(), hash_or_bookmark)
                        .and_then(move |csid| subcommand_tree(ctx, repo, csid, path))
                })
                .from_err()
                .boxify()
        }
        (COMMAND_VERIFY, Some(matches)) => {
            let hash_or_bookmark = String::from(matches.value_of(ARG_CSID).unwrap());
            let limit = matches
                .value_of(ARG_LIMIT)
                .unwrap()
                .parse::<u64>()
                .expect("limit must be an integer");
            cloned!(ctx);
            repo.into_future()
                .and_then(move |repo| {
                    helpers::csid_resolve(ctx.clone(), repo.clone(), hash_or_bookmark)
                        .and_then(move |csid| subcommand_verify(ctx, repo, csid, limit))
                })
                .from_err()
                .boxify()
        }
        _ => future::err(SubcommandError::InvalidArgs).boxify(),
    };

    if tracing_enable {
        run.then(move |result| ctx.trace_upload().then(move |_| result))
            .boxify()
    } else {
        run
    }
    .compat()
    .await
}

fn subcommand_tree(
    ctx: CoreContext,
    repo: BlobRepo,
    csid: ChangesetId,
    path: Option<MPath>,
) -> impl Future<Item = (), Error = Error> {
    RootUnodeManifestId::derive(ctx.clone(), repo.clone(), csid)
        .from_err()
        .and_then(move |root| {
            println!("ROOT: {:?}", root);
            println!("PATH: {:?}", path);
            root.manifest_unode_id()
                .find_entries(ctx, repo.get_blobstore(), vec![PathOrPrefix::Prefix(path)])
                .for_each(|(path, entry)| {
                    match entry {
                        Entry::Tree(tree_id) => {
                            println!("{}/ {:?}", MPath::display_opt(path.as_ref()), tree_id);
                        }
                        Entry::Leaf(leaf_id) => {
                            println!("{} {:?}", MPath::display_opt(path.as_ref()), leaf_id);
                        }
                    }
                    Ok(())
                })
        })
}

fn subcommand_verify(
    ctx: CoreContext,
    repo: BlobRepo,
    csid: ChangesetId,
    limit: u64,
) -> impl Future<Item = (), Error = Error> {
    AncestorsNodeStream::new(ctx.clone(), &repo.get_changeset_fetcher(), csid)
        .take(limit)
        .for_each(move |csid| single_verify(ctx.clone(), repo.clone(), csid))
}

fn single_verify(
    ctx: CoreContext,
    repo: BlobRepo,
    csid: ChangesetId,
) -> impl Future<Item = (), Error = Error> {
    let hg_paths = repo
        .get_hg_from_bonsai_changeset(ctx.clone(), csid)
        .and_then({
            cloned!(ctx, repo);
            move |hg_csid| {
                println!("CHANGESET: hg_csid:{:?} csid:{:?}", hg_csid, csid);
                hg_csid.load(ctx.clone(), repo.blobstore()).from_err()
            }
        })
        .and_then({
            cloned!(ctx, repo);
            move |hg_changeset| {
                hg_changeset
                    .manifestid()
                    .find_entries(ctx, repo.get_blobstore(), vec![PathOrPrefix::Prefix(None)])
                    .filter_map(|(path, _)| path)
                    .collect_to::<BTreeSet<_>>()
            }
        });

    let unode_paths = RootUnodeManifestId::derive(ctx.clone(), repo.clone(), csid)
        .from_err()
        .and_then(move |tree_id| {
            tree_id
                .manifest_unode_id()
                .find_entries(ctx, repo.get_blobstore(), vec![PathOrPrefix::Prefix(None)])
                .filter_map(|(path, _)| path)
                .collect_to::<BTreeSet<_>>()
        });

    (hg_paths, unode_paths)
        .into_future()
        .and_then(|(hg_paths, unode_paths)| {
            if hg_paths == unode_paths {
                Ok(())
            } else {
                println!("DIFFERENT: +hg -unode");
                for path in hg_paths.difference(&unode_paths) {
                    println!("+ {}", path);
                }
                for path in unode_paths.difference(&hg_paths) {
                    println!("- {}", path);
                }
                bail!("failed")
            }
        })
}
