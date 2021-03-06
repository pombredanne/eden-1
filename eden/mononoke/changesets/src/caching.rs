/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

use super::{ChangesetEntry, ChangesetInsert, Changesets, SqlChangesets};
use anyhow::Error;
use bytes::Bytes;
#[cfg(test)]
use caching_ext::MockStoreStats;
use caching_ext::{
    cache_all_determinator, CachelibHandler, GetOrFillMultipleFromCacheLayers, McErrorKind,
    McResult, MemcacheHandler,
};
use changeset_entry_thrift as thrift;
use context::CoreContext;
use fbinit::FacebookInit;
use fbthrift::compact_protocol;
use futures::Future;
use futures_ext::{BoxFuture, FutureExt};
use maplit::hashset;
use memcache::{KeyGen, MemcacheClient};
use mononoke_types::{
    ChangesetId, ChangesetIdPrefix, ChangesetIdsResolvedFromPrefix, RepositoryId,
};
use stats::prelude::*;
use std::collections::HashSet;
use std::iter::FromIterator;
use std::sync::Arc;

define_stats! {
    prefix = "mononoke.changesets";
    memcache_hit: timeseries("memcache.hit"; Rate, Sum),
    memcache_miss: timeseries("memcache.miss"; Rate, Sum),
    memcache_internal_err: timeseries("memcache.internal_err"; Rate, Sum),
    memcache_deserialize_err: timeseries("memcache.deserialize_err"; Rate, Sum),
}

pub fn get_cache_key(repo_id: RepositoryId, cs_id: &ChangesetId) -> String {
    format!("{}.{}", repo_id.prefix(), cs_id).to_string()
}

pub struct CachingChangesets {
    changesets: Arc<dyn Changesets>,
    cachelib: CachelibHandler<ChangesetEntry>,
    memcache: MemcacheHandler,
    keygen: KeyGen,
}

fn get_keygen() -> KeyGen {
    let key_prefix = "scm.mononoke.changesets";

    KeyGen::new(
        key_prefix,
        thrift::MC_CODEVER as u32,
        thrift::MC_SITEVER as u32,
    )
}

impl CachingChangesets {
    pub fn new(
        fb: FacebookInit,
        changesets: Arc<dyn Changesets>,
        cache_pool: cachelib::VolatileLruCachePool,
    ) -> Self {
        Self {
            changesets,
            cachelib: cache_pool.into(),
            memcache: MemcacheClient::new(fb)
                .expect("Memcache initialization failed")
                .into(),
            keygen: get_keygen(),
        }
    }

    #[cfg(test)]
    pub fn mocked(changesets: Arc<dyn Changesets>) -> Self {
        let cachelib = CachelibHandler::create_mock();
        let memcache = MemcacheHandler::create_mock();

        Self {
            changesets,
            cachelib,
            memcache,
            keygen: get_keygen(),
        }
    }

    #[cfg(test)]
    pub fn fork_cachelib(&self) -> Self {
        Self {
            changesets: self.changesets.clone(),
            cachelib: CachelibHandler::create_mock(),
            memcache: self.memcache.clone(),
            keygen: self.keygen.clone(),
        }
    }

    #[cfg(test)]
    pub fn cachelib_stats(&self) -> MockStoreStats {
        match self.cachelib {
            CachelibHandler::Real(_) => unimplemented!(),
            CachelibHandler::Mock(ref mock) => mock.stats(),
        }
    }

    #[cfg(test)]
    pub fn memcache_stats(&self) -> MockStoreStats {
        match self.memcache {
            MemcacheHandler::Real(_) => unimplemented!(),
            MemcacheHandler::Mock(ref mock) => mock.stats(),
        }
    }

    fn req(
        &self,
        ctx: CoreContext,
        repo_id: RepositoryId,
    ) -> GetOrFillMultipleFromCacheLayers<ChangesetId, ChangesetEntry> {
        let get_cache_key = Arc::new(get_cache_key);

        let changesets = self.changesets.clone();

        let get_from_db = move |keys: HashSet<ChangesetId>| {
            changesets
                .get_many(ctx.clone(), repo_id, keys.into_iter().collect())
                .map(|entries| entries.into_iter().map(|e| (e.cs_id, e)).collect())
                .boxify()
        };

        GetOrFillMultipleFromCacheLayers {
            repo_id,
            get_cache_key,
            cachelib: self.cachelib.clone(),
            keygen: self.keygen.clone(),
            memcache: self.memcache.clone(),
            deserialize: Arc::new(deserialize_changeset_entry),
            serialize: Arc::new(serialize_changeset_entry),
            report_mc_result: Arc::new(report_mc_result),
            get_from_db: Arc::new(get_from_db),
            determinator: cache_all_determinator::<ChangesetEntry>,
        }
    }
}

impl Changesets for CachingChangesets {
    fn add(&self, ctx: CoreContext, cs: ChangesetInsert) -> BoxFuture<bool, Error> {
        self.changesets.add(ctx, cs)
    }

    fn get(
        &self,
        ctx: CoreContext,
        repo_id: RepositoryId,
        cs_id: ChangesetId,
    ) -> BoxFuture<Option<ChangesetEntry>, Error> {
        self.req(ctx, repo_id)
            .run(hashset![cs_id])
            .map(move |mut map| map.remove(&cs_id))
            .boxify()
    }

    fn get_many(
        &self,
        ctx: CoreContext,
        repo_id: RepositoryId,
        cs_ids: Vec<ChangesetId>,
    ) -> BoxFuture<Vec<ChangesetEntry>, Error> {
        let keys = HashSet::from_iter(cs_ids);

        self.req(ctx, repo_id)
            .run(keys)
            .map(|map| map.into_iter().map(|(_, val)| val).collect())
            .boxify()
    }

    /// Use caching for the full changeset ids and slower path otherwise.
    fn get_many_by_prefix(
        &self,
        ctx: CoreContext,
        repo_id: RepositoryId,
        cs_prefix: ChangesetIdPrefix,
        limit: usize,
    ) -> BoxFuture<ChangesetIdsResolvedFromPrefix, Error> {
        if let Some(id) = cs_prefix.into_changeset_id() {
            return self
                .get(ctx, repo_id, id)
                .map(move |res| match res {
                    Some(_) if limit > 0 => ChangesetIdsResolvedFromPrefix::Single(id),
                    _ => ChangesetIdsResolvedFromPrefix::NoMatch,
                })
                .boxify();
        }
        self.changesets
            .get_many_by_prefix(ctx, repo_id, cs_prefix, limit)
            .boxify()
    }

    fn prime_cache(&self, _ctx: &CoreContext, changesets: &[ChangesetEntry]) {
        for cs in changesets {
            let key = get_cache_key(cs.repo_id, &cs.cs_id);
            let _ = self.cachelib.set_cached(&key, &cs);
        }
    }

    fn get_sql_changesets(&self) -> &SqlChangesets {
        self.changesets.get_sql_changesets()
    }
}

fn deserialize_changeset_entry(bytes: Bytes) -> Result<ChangesetEntry, ()> {
    compact_protocol::deserialize(bytes)
        .and_then(|entry| ChangesetEntry::from_thrift(entry))
        .map_err(|_| ())
}

fn serialize_changeset_entry(entry: &ChangesetEntry) -> Bytes {
    compact_protocol::serialize(&entry.clone().into_thrift())
}

fn report_mc_result<T>(res: McResult<T>) {
    match res {
        Ok(..) => STATS::memcache_hit.add_value(1),
        Err(McErrorKind::MemcacheInternal) => STATS::memcache_internal_err.add_value(1),
        Err(McErrorKind::Missing) => STATS::memcache_miss.add_value(1),
        Err(McErrorKind::Deserialization) => STATS::memcache_deserialize_err.add_value(1),
    };
}
