/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

//! Tests run against all blobstore implementations.

#![deny(warnings)]
#![feature(never_type)]

use std::sync::Arc;

use anyhow::Error;
use bytes::Bytes;
use fbinit::FacebookInit;
use futures::compat::Future01CompatExt;
use tempdir::TempDir;

use blobstore::{Blobstore, BlobstoreWithLink};
use context::CoreContext;
use fileblob::Fileblob;
use memblob::{EagerMemblob, LazyMemblob};
use mononoke_types::BlobstoreBytes;

async fn roundtrip_and_link<B: BlobstoreWithLink>(
    fb: FacebookInit,
    blobstore: B,
    has_ctime: bool,
) -> Result<(), Error> {
    let ctx = CoreContext::test_mock(fb);

    let key = "randomkey".to_string();
    let value = BlobstoreBytes::from_bytes(Bytes::copy_from_slice(b"appleveldata"));

    // Roundtrip
    blobstore
        .put(ctx.clone(), key.clone(), value.clone())
        .compat()
        .await?;

    let roundtrip = blobstore
        .get(ctx.clone(), key.clone())
        .compat()
        .await?
        .unwrap();

    let orig_ctime = roundtrip.as_meta().as_ctime().clone();

    assert_eq!(orig_ctime.is_some(), has_ctime);
    assert_eq!(value, roundtrip.into_bytes());

    let newkey = "newkey".to_string();

    // And now the link
    blobstore
        .link(ctx.clone(), key.clone(), newkey.clone())
        .await?;

    let newvalue = blobstore
        .get(ctx.clone(), newkey.clone())
        .compat()
        .await?
        .unwrap();

    let new_ctime = newvalue.as_meta().as_ctime().clone();
    assert_eq!(new_ctime.is_some(), has_ctime);
    assert_eq!(orig_ctime, new_ctime);
    assert_eq!(value, newvalue.into_bytes());

    let newkey_is_present = blobstore
        .is_present(ctx.clone(), newkey.clone())
        .compat()
        .await?;

    assert!(newkey_is_present);

    Ok(())
}

async fn missing<B: Blobstore>(fb: FacebookInit, blobstore: B) -> Result<(), Error> {
    let ctx = CoreContext::test_mock(fb);

    let key = "missing".to_string();
    let out = blobstore.get(ctx, key).compat().await?;

    assert!(out.is_none());
    Ok(())
}

macro_rules! blobstore_test_impl {
    ($mod_name: ident => {
        state: $state: expr,
        new: $new_cb: expr,
        persistent: $persistent: expr,
        has_ctime: $has_ctime: expr,
    }) => {
        mod $mod_name {
            use super::*;

            #[fbinit::compat_test]
            async fn test_roundtrip_and_link(fb: FacebookInit) -> Result<(), Error> {
                let state = $state;
                let has_ctime = $has_ctime;
                let factory = $new_cb;
                roundtrip_and_link(fb, factory(state.clone())?, has_ctime).await
            }

            #[fbinit::compat_test]
            async fn test_missing(fb: FacebookInit) -> Result<(), Error> {
                let state = $state;
                let factory = $new_cb;
                missing(fb, factory(state)?).await
            }

            #[fbinit::compat_test]
            async fn test_boxable(_fb: FacebookInit) -> Result<(), Error> {
                let state = $state;
                let factory = $new_cb;
                // This is really just checking that the constructed type is Sized
                Box::new(factory(state)?);
                Ok(())
            }
        }
    };
}

blobstore_test_impl! {
    eager_memblob_test => {
        state: (),
        new: move |_| Ok::<_,Error>(EagerMemblob::new()),
        persistent: false,
        has_ctime: false,
    }
}

blobstore_test_impl! {
    box_blobstore_test => {
        state: (),
        new: move |_| Ok::<_,Error>(Box::new(EagerMemblob::new())),
        persistent: false,
        has_ctime: false,
    }
}

blobstore_test_impl! {
    lazy_memblob_test => {
        state: (),
        new: move |_| Ok::<_,Error>(LazyMemblob::new()),
        persistent: false,
        has_ctime: false,
    }
}

blobstore_test_impl! {
    fileblob_test => {
        state: Arc::new(TempDir::new("fileblob_test").unwrap()),
        new: move |dir: Arc<TempDir>| Fileblob::open(&*dir),
        persistent: true,
        has_ctime: true,
    }
}
