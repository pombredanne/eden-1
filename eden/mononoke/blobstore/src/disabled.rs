/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

use anyhow::{format_err, Error};
use context::CoreContext;
use futures_ext::{BoxFuture as BoxFuture01, FutureExt as FutureExt01};
use futures_old::future::IntoFuture as IntoFuture01;

use super::{Blobstore, BlobstoreBytes, BlobstoreGetData};

/// Disabled blobstore which fails all operations with a reason. Primarily used as a
/// placeholder for administratively disabled blobstores.
#[derive(Debug)]
pub struct DisabledBlob {
    reason: String,
}

impl DisabledBlob {
    pub fn new(reason: impl Into<String>) -> Self {
        DisabledBlob {
            reason: reason.into(),
        }
    }
}

impl Blobstore for DisabledBlob {
    fn get(&self, _ctx: CoreContext, _key: String) -> BoxFuture01<Option<BlobstoreGetData>, Error> {
        Err(format_err!("Blobstore disabled: {}", self.reason))
            .into_future()
            .boxify()
    }

    fn put(
        &self,
        _ctx: CoreContext,
        _key: String,
        _value: BlobstoreBytes,
    ) -> BoxFuture01<(), Error> {
        Err(format_err!("Blobstore disabled: {}", self.reason))
            .into_future()
            .boxify()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use fbinit::FacebookInit;

    #[fbinit::test]
    fn test_disabled(fb: FacebookInit) {
        let disabled = DisabledBlob::new("test");
        let ctx = CoreContext::test_mock(fb);

        let mut runtime = tokio_compat::runtime::Runtime::new().unwrap();

        match runtime.block_on(disabled.get(ctx.clone(), "foobar".to_string())) {
            Ok(_) => panic!("Unexpected success"),
            Err(err) => println!("Got error: {:?}", err),
        }

        match runtime.block_on(disabled.put(
            ctx,
            "foobar".to_string(),
            BlobstoreBytes::from_bytes(vec![]),
        )) {
            Ok(_) => panic!("Unexpected success"),
            Err(err) => println!("Got error: {:?}", err),
        }
    }
}
