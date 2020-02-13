/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

#![deny(warnings)]
#![type_length_limit = "4715995"]

use anyhow::Error;
use blobrepo::BlobRepo;
use context::CoreContext;
use derived_data::BonsaiDerived;
use futures::Future;
use futures_ext::{BoxFuture, FutureExt};
use mononoke_types::{ChangesetId, ContentId, FsnodeId};
use thiserror::Error;

mod derive;
mod mapping;

pub use mapping::{RootFsnodeId, RootFsnodeMapping};

#[derive(Debug, Error)]
pub enum ErrorKind {
    #[error("Invalid bonsai changeset: {0}")]
    InvalidBonsai(String),
    #[error("Missing content: {0}")]
    MissingContent(ContentId),
    #[error("Missing fsnode parent: {0}")]
    MissingParent(FsnodeId),
    #[error("Missing fsnode subentry for '{0}': {1}")]
    MissingSubentry(String, FsnodeId),
}

pub fn derive_fsnodes(
    ctx: CoreContext,
    repo: BlobRepo,
    cs_id: ChangesetId,
) -> BoxFuture<(), Error> {
    RootFsnodeId::derive(ctx, repo, cs_id).map(|_| ()).boxify()
}