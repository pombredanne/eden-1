/*
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

#![deny(warnings)]

use anyhow::{format_err, Error};
use blobrepo::BlobRepo;
use blobstore::Loadable;
use cloned::cloned;
use context::CoreContext;
use derived_data::{BonsaiDerived, BonsaiDerivedMapping};
use filenodes::{FilenodeInfo, FilenodeResult, PreparedFilenode};
use futures::{
    compat::{Future01CompatExt, Stream01CompatExt},
    future::try_join_all,
    stream, FutureExt, StreamExt, TryFutureExt, TryStreamExt,
};
use futures_ext::{BoxFuture, FutureExt as OldFutureExt};
use futures_old::{future as old_future, Future};
use futures_util::try_join;
use itertools::{Either, Itertools};
use manifest::{find_intersection_of_diffs_and_parents, Entry};
use mercurial_types::{
    blobs::File, fetch_manifest_envelope, HgChangesetId, HgFileEnvelope, HgFileNodeId,
    HgManifestEnvelope, HgManifestId, NULL_HASH,
};
use mononoke_types::{BonsaiChangeset, ChangesetId, MPath, RepoPath};
use std::{collections::HashMap, convert::TryFrom};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedRootFilenode {
    pub filenode: HgFileNodeId,
    pub p1: Option<HgFileNodeId>,
    pub p2: Option<HgFileNodeId>,
    pub copyfrom: Option<(RepoPath, HgFileNodeId)>,
    pub linknode: HgChangesetId,
}

impl From<PreparedRootFilenode> for PreparedFilenode {
    fn from(root_filenode: PreparedRootFilenode) -> Self {
        let PreparedRootFilenode {
            filenode,
            p1,
            p2,
            copyfrom,
            linknode,
        } = root_filenode;

        PreparedFilenode {
            path: RepoPath::RootPath,
            info: FilenodeInfo {
                filenode,
                p1,
                p2,
                copyfrom,
                linknode,
            },
        }
    }
}

impl TryFrom<PreparedFilenode> for PreparedRootFilenode {
    type Error = Error;

    fn try_from(filenode: PreparedFilenode) -> Result<Self, Self::Error> {
        let PreparedFilenode { path, info } = filenode;

        let FilenodeInfo {
            filenode,
            p1,
            p2,
            copyfrom,
            linknode,
        } = info;

        if path != RepoPath::RootPath {
            return Err(format_err!("unexpected path for root filenode: {:?}", path));
        }
        Ok(Self {
            filenode,
            p1,
            p2,
            copyfrom,
            linknode,
        })
    }
}

/// Derives filenodes that are stores in Filenodes object (usually in a database).
/// Note: that should be derived only for public commits!
///
/// Filenodes might be disabled, in that case FilenodesOnlyPublic will always return
/// FilenodesOnlyPublic::Disabled enum variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilenodesOnlyPublic {
    Present {
        root_filenode: Option<PreparedRootFilenode>,
    },
    Disabled,
}

impl BonsaiDerived for FilenodesOnlyPublic {
    const NAME: &'static str = "filenodes";
    type Mapping = FilenodesOnlyPublicMapping;

    fn mapping(_ctx: &CoreContext, repo: &BlobRepo) -> Self::Mapping {
        FilenodesOnlyPublicMapping::new(repo.clone())
    }

    fn derive_from_parents(
        ctx: CoreContext,
        repo: BlobRepo,
        bonsai: BonsaiChangeset,
        _parents: Vec<Self>,
    ) -> BoxFuture<Self, Error> {
        async move {
            let filenodes = generate_all_filenodes(&ctx, &repo, bonsai.get_changeset_id()).await?;

            if filenodes.is_empty() {
                // This commit didn't create any new filenodes, and it's root manifest is the
                // same as one of the parents (that can happen if this commit is empty).
                // In that case we don't need to insert a root filenode - it will be inserted
                // when parent is derived.
                Ok(FilenodesOnlyPublic::Present {
                    root_filenode: None,
                })
            } else {
                let (roots, non_roots): (Vec<_>, Vec<_>) =
                    filenodes.into_iter().partition_map(classify_filenode);
                let mut roots = roots.into_iter();

                match (roots.next(), roots.next()) {
                    (Some(root_filenode), None) => {
                        let filenodes = repo.get_filenodes();
                        let repo_id = repo.get_repoid();
                        let filenode_res = filenodes
                            .add_filenodes(ctx.clone(), non_roots, repo_id)
                            .compat()
                            .await?;

                        match filenode_res {
                            FilenodeResult::Present(()) => Ok(FilenodesOnlyPublic::Present {
                                root_filenode: Some(root_filenode),
                            }),
                            FilenodeResult::Disabled => Ok(FilenodesOnlyPublic::Disabled),
                        }
                    }
                    _ => Err(format_err!("expected exactly one root, found {:?}", roots)),
                }
            }
        }
        .boxed()
        .compat()
        .boxify()
    }
}

fn classify_filenode(filenode: PreparedFilenode) -> Either<PreparedRootFilenode, PreparedFilenode> {
    if filenode.path == RepoPath::RootPath {
        let FilenodeInfo {
            filenode,
            p1,
            p2,
            copyfrom,
            linknode,
        } = filenode.info;

        Either::Left(PreparedRootFilenode {
            filenode,
            p1,
            p2,
            copyfrom,
            linknode,
        })
    } else {
        Either::Right(filenode)
    }
}

pub async fn generate_all_filenodes(
    ctx: &CoreContext,
    repo: &BlobRepo,
    cs_id: ChangesetId,
) -> Result<Vec<PreparedFilenode>, Error> {
    let parents = repo
        .get_changeset_parents_by_bonsai(ctx.clone(), cs_id)
        .compat()
        .await?;

    let root_mf = fetch_root_manifest_id(&ctx, &cs_id, &repo);
    // Bonsai might have > 2 parents, while mercurial supports at most 2.
    // That's fine for us - we just won't generate filenodes for paths that came from
    // stepparents. That means that linknode for these filenodes will point to a stepparent
    let parents = try_join_all(
        parents
            .iter()
            .map(|p| fetch_root_manifest_id(&ctx, p, &repo)),
    );

    let linknode = repo
        .get_hg_from_bonsai_changeset(ctx.clone(), cs_id)
        .compat();

    let (root_mf, parents, linknode) = try_join!(root_mf, parents, linknode)?;
    let blobstore = repo.get_blobstore().boxed();
    find_intersection_of_diffs_and_parents(
        ctx.clone(),
        repo.get_blobstore(),
        root_mf,
        parents.clone(),
    )
    .compat()
    .try_filter_map(|(path, entry, parent_entries)| {
        async move {
            // file entry has file type and file node id. If file type is different but filenode is
            // the same we don't want to create a new filenode, and this filter removes
            // all entries where at least one parent has the same filenode id.
            if let Entry::Leaf((_, hg_filenode_id)) = entry {
                for parent_entry in parent_entries {
                    if let Entry::Leaf((_, parent_filenode_id)) = parent_entry {
                        if parent_filenode_id == hg_filenode_id {
                            return Ok(None);
                        }
                    }
                }
            }
            Ok(Some((path, entry)))
        }
    })
    .map_ok(move |(path, entry)| match entry {
        Entry::Tree(hg_mf_id) => fetch_manifest_envelope(ctx.clone(), &blobstore, hg_mf_id)
            .map(move |envelope| create_manifest_filenode(path, envelope, linknode))
            .left_future()
            .compat(),
        Entry::Leaf((_, hg_filenode_id)) => hg_filenode_id
            .load(ctx.clone(), &blobstore)
            .from_err()
            .and_then(move |envelope| create_file_filenode(path, envelope, linknode))
            .right_future()
            .compat(),
    })
    .try_buffer_unordered(100)
    .try_collect()
    .await
}

fn create_manifest_filenode(
    path: Option<MPath>,
    envelope: HgManifestEnvelope,
    linknode: HgChangesetId,
) -> PreparedFilenode {
    let path = match path {
        Some(path) => RepoPath::DirectoryPath(path),
        None => RepoPath::RootPath,
    };
    let filenode = HgFileNodeId::new(envelope.node_id());
    let (p1, p2) = envelope.parents();
    let p1 = p1.map(HgFileNodeId::new);
    let p2 = p2.map(HgFileNodeId::new);

    PreparedFilenode {
        path,
        info: FilenodeInfo {
            filenode,
            p1,
            p2,
            copyfrom: None,
            linknode,
        },
    }
}

fn create_file_filenode(
    path: Option<MPath>,
    envelope: HgFileEnvelope,
    linknode: HgChangesetId,
) -> Result<PreparedFilenode, Error> {
    let path = match path {
        Some(path) => RepoPath::FilePath(path),
        None => {
            return Err(format_err!("unexpected empty file path"));
        }
    };
    let filenode = envelope.node_id();
    let (p1, p2) = envelope.parents();
    let copyfrom = File::extract_copied_from(envelope.metadata())?
        .map(|(path, node)| (RepoPath::FilePath(path), node));

    Ok(PreparedFilenode {
        path,
        info: FilenodeInfo {
            filenode,
            p1,
            p2,
            copyfrom,
            linknode,
        },
    })
}

#[derive(Clone)]
pub struct FilenodesOnlyPublicMapping {
    repo: BlobRepo,
}

impl FilenodesOnlyPublicMapping {
    pub fn new(repo: BlobRepo) -> Self {
        Self { repo }
    }
}

impl BonsaiDerivedMapping for FilenodesOnlyPublicMapping {
    type Value = FilenodesOnlyPublic;

    fn get(
        &self,
        ctx: CoreContext,
        csids: Vec<ChangesetId>,
    ) -> BoxFuture<HashMap<ChangesetId, Self::Value>, Error> {
        cloned!(self.repo);
        async move {
            stream::iter(csids.into_iter())
                .map({
                    let repo = &repo;
                    let ctx = &ctx;
                    move |cs_id| async move {
                        let filenode_res = fetch_root_filenode(&ctx, cs_id, &repo).await?;
                        let maybe_root_filenode = match filenode_res {
                            FilenodeResult::Present(maybe_root_filenode) => maybe_root_filenode,
                            FilenodeResult::Disabled => {
                                return Ok(Some((cs_id, FilenodesOnlyPublic::Disabled)))
                            }
                        };

                        Ok(maybe_root_filenode.map(move |filenode| {
                            (
                                cs_id,
                                FilenodesOnlyPublic::Present {
                                    root_filenode: Some(filenode),
                                },
                            )
                        }))
                    }
                })
                .buffer_unordered(100)
                .try_filter_map(|x| async { Ok(x) })
                .try_collect()
                .await
        }
        .boxed()
        .compat()
        .boxify()
    }

    fn put(&self, ctx: CoreContext, _csid: ChangesetId, id: Self::Value) -> BoxFuture<(), Error> {
        let filenodes = self.repo.get_filenodes();
        let repo_id = self.repo.get_repoid();

        let root_filenode = match id {
            FilenodesOnlyPublic::Present { root_filenode } => root_filenode,
            FilenodesOnlyPublic::Disabled => None,
        };

        match root_filenode {
            Some(root_filenode) => filenodes
                .add_filenodes(ctx.clone(), vec![root_filenode.into()], repo_id)
                .map(|res| match res {
                    // If filenodes are disabled then just return success
                    // but use explicit match here in case we add more variants
                    // to FilenodeResult enum
                    FilenodeResult::Present(()) | FilenodeResult::Disabled => (),
                })
                .boxify(),
            None => old_future::ok(()).boxify(),
        }
    }
}

async fn fetch_root_filenode(
    ctx: &CoreContext,
    cs_id: ChangesetId,
    repo: &BlobRepo,
) -> Result<FilenodeResult<Option<PreparedRootFilenode>>, Error> {
    // If hg changeset is not generated, then root filenode can't possible be generated
    // Check it and return None if hg changeset is not generated
    let maybe_hg_cs_id = repo
        .get_bonsai_hg_mapping()
        .get_hg_from_bonsai(ctx.clone(), repo.get_repoid(), cs_id.clone())
        .compat()
        .await?;
    if maybe_hg_cs_id.is_none() {
        return Ok(FilenodeResult::Present(None));
    }

    let mf_id = fetch_root_manifest_id(ctx, &cs_id, repo).await?;

    // Special case null manifest id if we run into it
    let mf_id = mf_id.into_nodehash();
    let filenodes = repo.get_filenodes();
    if mf_id == NULL_HASH {
        Ok(FilenodeResult::Present(Some(PreparedRootFilenode {
            filenode: HgFileNodeId::new(NULL_HASH),
            p1: None,
            p2: None,
            copyfrom: None,
            linknode: HgChangesetId::new(NULL_HASH),
        })))
    } else {
        let filenode_res = filenodes
            .get_filenode(
                ctx.clone(),
                &RepoPath::RootPath,
                HgFileNodeId::new(mf_id),
                repo.get_repoid(),
            )
            .compat()
            .await?;

        match filenode_res {
            FilenodeResult::Present(maybe_info) => {
                let info = maybe_info
                    .map(|info| {
                        PreparedRootFilenode::try_from(PreparedFilenode {
                            path: RepoPath::RootPath,
                            info,
                        })
                    })
                    .transpose()?;
                Ok(FilenodeResult::Present(info))
            }
            FilenodeResult::Disabled => Ok(FilenodeResult::Disabled),
        }
    }
}

async fn fetch_root_manifest_id(
    ctx: &CoreContext,
    cs_id: &ChangesetId,
    repo: &BlobRepo,
) -> Result<HgManifestId, Error> {
    let hg_cs_id = repo
        .get_hg_from_bonsai_changeset(ctx.clone(), *cs_id)
        .compat()
        .await?;

    let hg_cs = hg_cs_id
        .load(ctx.clone(), repo.blobstore())
        .compat()
        .await?;

    Ok(hg_cs.manifestid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fbinit::FacebookInit;
    use maplit::hashmap;
    use mononoke_types::FileType;
    use slog::info;
    use tests_utils::CreateCommitContext;
    use tunables::{with_tunables, MononokeTunables};

    async fn verify_filenodes(
        ctx: &CoreContext,
        repo: &BlobRepo,
        cs_id: ChangesetId,
        expected_paths: Vec<RepoPath>,
    ) -> Result<(), Error> {
        let filenodes = generate_all_filenodes(&ctx, &repo, cs_id).await?;

        assert_eq!(filenodes.len(), expected_paths.len());
        for path in expected_paths {
            assert!(filenodes
                .iter()
                .find(|filenode| filenode.path == path)
                .is_some());
        }

        let linknode = repo
            .get_hg_from_bonsai_changeset(ctx.clone(), cs_id)
            .compat()
            .await?;

        for filenode in filenodes {
            assert_eq!(filenode.info.linknode, linknode);
        }
        Ok(())
    }

    async fn test_generate_filenodes_simple(fb: FacebookInit) -> Result<(), Error> {
        let ctx = CoreContext::test_mock(fb);
        let repo = blobrepo_factory::new_memblob_empty(None)?;
        let filename = "path";
        let commit = CreateCommitContext::new_root(&ctx, &repo)
            .add_file(filename, "content")
            .commit()
            .await?;

        // Two filenodes - one for root manifest, another for a file
        verify_filenodes(
            &ctx,
            &repo,
            commit,
            vec![RepoPath::RootPath, RepoPath::file(filename)?],
        )
        .await?;

        Ok(())
    }

    #[fbinit::test]
    fn generate_filenodes_simple(fb: FacebookInit) -> Result<(), Error> {
        let mut runtime = tokio_compat::runtime::Runtime::new()?;
        runtime.block_on_std(test_generate_filenodes_simple(fb))
    }

    async fn test_generate_filenodes_merge(fb: FacebookInit) -> Result<(), Error> {
        let ctx = CoreContext::test_mock(fb);
        let repo = blobrepo_factory::new_memblob_empty(None)?;
        let first_p1 = CreateCommitContext::new_root(&ctx, &repo)
            .add_file("path1", "content")
            .commit()
            .await?;

        let first_p2 = CreateCommitContext::new_root(&ctx, &repo)
            .add_file("path2", "content")
            .commit()
            .await?;

        let merge = CreateCommitContext::new(&ctx, &repo, vec![first_p1, first_p2])
            .commit()
            .await?;

        // Only root filenode was added - other filenodes were reused from parents
        verify_filenodes(&ctx, &repo, merge, vec![RepoPath::RootPath]).await?;

        Ok(())
    }

    #[fbinit::test]
    fn generate_filenodes_merge(fb: FacebookInit) -> Result<(), Error> {
        let mut runtime = tokio_compat::runtime::Runtime::new()?;
        runtime.block_on_std(test_generate_filenodes_merge(fb))
    }

    async fn test_generate_type_change(fb: FacebookInit) -> Result<(), Error> {
        let ctx = CoreContext::test_mock(fb);
        let repo = blobrepo_factory::new_memblob_empty(None)?;
        let parent = CreateCommitContext::new_root(&ctx, &repo)
            .add_file("path", "content")
            .commit()
            .await?;

        let child = CreateCommitContext::new(&ctx, &repo, vec![parent])
            .add_file_with_type("path", "content", FileType::Executable)
            .commit()
            .await?;

        // Only root filenode should be changed - change of file type doesn't change filenode
        verify_filenodes(&ctx, &repo, child, vec![RepoPath::RootPath]).await?;

        Ok(())
    }

    #[fbinit::test]
    fn generate_filenodes_type_change(fb: FacebookInit) -> Result<(), Error> {
        let mut runtime = tokio_compat::runtime::Runtime::new()?;
        runtime.block_on_std(test_generate_type_change(fb))
    }

    async fn test_many_parents(fb: FacebookInit) -> Result<(), Error> {
        let ctx = CoreContext::test_mock(fb);
        let repo = blobrepo_factory::new_memblob_empty(None)?;
        let p1 = CreateCommitContext::new_root(&ctx, &repo)
            .add_file("path1", "content")
            .commit()
            .await?;
        let p2 = CreateCommitContext::new_root(&ctx, &repo)
            .add_file("path2", "content")
            .commit()
            .await?;
        let p3 = CreateCommitContext::new_root(&ctx, &repo)
            .add_file("path3", "content")
            .commit()
            .await?;

        let merge = CreateCommitContext::new(&ctx, &repo, vec![p1, p2, p3])
            .commit()
            .await?;

        info!(ctx.logger(), "checking filenodes for {}", p3);
        verify_filenodes(
            &ctx,
            &repo,
            p3,
            vec![RepoPath::RootPath, RepoPath::file("path3")?],
        )
        .await?;

        info!(ctx.logger(), "checking filenodes for {}", merge);
        verify_filenodes(&ctx, &repo, merge, vec![RepoPath::RootPath]).await?;

        Ok(())
    }

    #[fbinit::test]
    fn many_parents(fb: FacebookInit) -> Result<(), Error> {
        let mut runtime = tokio_compat::runtime::Runtime::new()?;
        runtime.block_on_std(test_many_parents(fb))
    }

    async fn test_derive_empty_commits(fb: FacebookInit) -> Result<(), Error> {
        let ctx = CoreContext::test_mock(fb);
        let repo = blobrepo_factory::new_memblob_empty(None)?;
        let parent_empty = CreateCommitContext::new_root(&ctx, &repo).commit().await?;

        let child_empty = CreateCommitContext::new(&ctx, &repo, vec![parent_empty])
            .add_file("file", "content")
            .commit()
            .await?;

        FilenodesOnlyPublic::derive(ctx.clone(), repo.clone(), child_empty)
            .compat()
            .await?;

        // Make sure they are in the mapping
        let maps = FilenodesOnlyPublic::mapping(&ctx, &repo)
            .get(ctx.clone(), vec![parent_empty, child_empty])
            .compat()
            .await?;

        assert_eq!(maps.len(), 2);
        Ok(())
    }

    #[fbinit::test]
    fn derive_empty_commits(fb: FacebookInit) -> Result<(), Error> {
        let mut runtime = tokio_compat::runtime::Runtime::new()?;
        runtime.block_on_std(test_derive_empty_commits(fb))
    }

    async fn test_derive_only_empty_commits(fb: FacebookInit) -> Result<(), Error> {
        let ctx = CoreContext::test_mock(fb);
        let repo = blobrepo_factory::new_memblob_empty(None)?;

        let parent_empty = CreateCommitContext::new_root(&ctx, &repo).commit().await?;
        let child_empty = CreateCommitContext::new(&ctx, &repo, vec![parent_empty])
            .commit()
            .await?;

        let mapping = FilenodesOnlyPublic::mapping(&ctx, &repo);
        FilenodesOnlyPublic::derive(ctx.clone(), repo.clone(), child_empty)
            .compat()
            .await?;

        // Make sure they are in the mapping
        let maps = mapping
            .get(ctx.clone(), vec![child_empty, parent_empty])
            .compat()
            .await?;
        assert_eq!(maps.len(), 2);
        Ok(())
    }

    #[fbinit::test]
    fn derive_only_empty_commits(fb: FacebookInit) -> Result<(), Error> {
        let mut runtime = tokio_compat::runtime::Runtime::new()?;
        runtime.block_on_std(test_derive_only_empty_commits(fb))
    }

    #[fbinit::test]
    fn derive_disabled_filenodes(fb: FacebookInit) -> Result<(), Error> {
        let tunables = MononokeTunables::default();
        tunables.update_bools(&hashmap! {"filenodes_disabled".to_string() => true});

        with_tunables(tunables, || {
            let mut runtime = tokio_compat::runtime::Runtime::new()?;
            runtime.block_on_std(test_derive_disabled_filenodes(fb))
        })
    }

    async fn test_derive_disabled_filenodes(fb: FacebookInit) -> Result<(), Error> {
        let ctx = CoreContext::test_mock(fb);
        let repo = blobrepo_factory::new_memblob_empty(None)?;
        let cs = CreateCommitContext::new_root(&ctx, &repo).commit().await?;
        let derived = FilenodesOnlyPublic::derive(ctx.clone(), repo.clone(), cs)
            .compat()
            .await?;
        assert_eq!(derived, FilenodesOnlyPublic::Disabled);

        let mapping = FilenodesOnlyPublic::mapping(&ctx, &repo);
        let res = mapping.get(ctx.clone(), vec![cs]).compat().await?;

        assert_eq!(res.get(&cs).unwrap(), &FilenodesOnlyPublic::Disabled);

        Ok(())
    }
}
