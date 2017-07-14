==========================
Test rebase with obsolete
==========================

Enable obsolete

  $ cat >> $HGRCPATH << EOF
  > [ui]
  > logtemplate= {rev}:{node|short} {desc|firstline}
  > [experimental]
  > evolution=createmarkers,allowunstable
  > [phases]
  > publish=False
  > [extensions]
  > rebase=
  > drawdag=$TESTDIR/drawdag.py
  > EOF

Setup rebase canonical repo

  $ hg init base
  $ cd base
  $ hg unbundle "$TESTDIR/bundles/rebase.hg"
  adding changesets
  adding manifests
  adding file changes
  added 8 changesets with 7 changes to 7 files (+2 heads)
  (run 'hg heads' to see heads, 'hg merge' to merge)
  $ hg up tip
  3 files updated, 0 files merged, 0 files removed, 0 files unresolved
  $ hg log -G
  @  7:02de42196ebe H
  |
  | o  6:eea13746799a G
  |/|
  o |  5:24b6387c8c8c F
  | |
  | o  4:9520eea781bc E
  |/
  | o  3:32af7686d403 D
  | |
  | o  2:5fddd98957c8 C
  | |
  | o  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  
  $ cd ..

simple rebase
---------------------------------

  $ hg clone base simple
  updating to branch default
  3 files updated, 0 files merged, 0 files removed, 0 files unresolved
  $ cd simple
  $ hg up 32af7686d403
  3 files updated, 0 files merged, 2 files removed, 0 files unresolved
  $ hg rebase -d eea13746799a
  rebasing 1:42ccdea3bb16 "B"
  rebasing 2:5fddd98957c8 "C"
  rebasing 3:32af7686d403 "D"
  $ hg log -G
  @  10:8eeb3c33ad33 D
  |
  o  9:2327fea05063 C
  |
  o  8:e4e5be0395b2 B
  |
  | o  7:02de42196ebe H
  | |
  o |  6:eea13746799a G
  |\|
  | o  5:24b6387c8c8c F
  | |
  o |  4:9520eea781bc E
  |/
  o  0:cd010b8cd998 A
  
  $ hg log --hidden -G
  @  10:8eeb3c33ad33 D
  |
  o  9:2327fea05063 C
  |
  o  8:e4e5be0395b2 B
  |
  | o  7:02de42196ebe H
  | |
  o |  6:eea13746799a G
  |\|
  | o  5:24b6387c8c8c F
  | |
  o |  4:9520eea781bc E
  |/
  | x  3:32af7686d403 D
  | |
  | x  2:5fddd98957c8 C
  | |
  | x  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  
  $ hg debugobsolete
  42ccdea3bb16d28e1848c95fe2e44c000f3f21b1 e4e5be0395b2cbd471ed22a26b1b6a1a0658a794 0 (*) {'user': 'test'} (glob)
  5fddd98957c8a54a4d436dfe1da9d87f21a1b97b 2327fea05063f39961b14cb69435a9898dc9a245 0 (*) {'user': 'test'} (glob)
  32af7686d403cf45b5d95f2d70cebea587ac806a 8eeb3c33ad33d452c89e5dcf611c347f978fb42b 0 (*) {'user': 'test'} (glob)


  $ cd ..

empty changeset
---------------------------------

  $ hg clone base empty
  updating to branch default
  3 files updated, 0 files merged, 0 files removed, 0 files unresolved
  $ cd empty
  $ hg up eea13746799a
  1 files updated, 0 files merged, 1 files removed, 0 files unresolved

We make a copy of both the first changeset in the rebased and some other in the
set.

  $ hg graft 42ccdea3bb16 32af7686d403
  grafting 1:42ccdea3bb16 "B"
  grafting 3:32af7686d403 "D"
  $ hg rebase  -s 42ccdea3bb16 -d .
  rebasing 1:42ccdea3bb16 "B"
  note: rebase of 1:42ccdea3bb16 created no changes to commit
  rebasing 2:5fddd98957c8 "C"
  rebasing 3:32af7686d403 "D"
  note: rebase of 3:32af7686d403 created no changes to commit
  $ hg log -G
  o  10:5ae4c968c6ac C
  |
  @  9:08483444fef9 D
  |
  o  8:8877864f1edb B
  |
  | o  7:02de42196ebe H
  | |
  o |  6:eea13746799a G
  |\|
  | o  5:24b6387c8c8c F
  | |
  o |  4:9520eea781bc E
  |/
  o  0:cd010b8cd998 A
  
  $ hg log --hidden -G
  o  10:5ae4c968c6ac C
  |
  @  9:08483444fef9 D
  |
  o  8:8877864f1edb B
  |
  | o  7:02de42196ebe H
  | |
  o |  6:eea13746799a G
  |\|
  | o  5:24b6387c8c8c F
  | |
  o |  4:9520eea781bc E
  |/
  | x  3:32af7686d403 D
  | |
  | x  2:5fddd98957c8 C
  | |
  | x  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  
  $ hg debugobsolete
  42ccdea3bb16d28e1848c95fe2e44c000f3f21b1 0 {cd010b8cd998f3981a5a8115f94f8da4ab506089} (*) {'user': 'test'} (glob)
  5fddd98957c8a54a4d436dfe1da9d87f21a1b97b 5ae4c968c6aca831df823664e706c9d4aa34473d 0 (*) {'user': 'test'} (glob)
  32af7686d403cf45b5d95f2d70cebea587ac806a 0 {5fddd98957c8a54a4d436dfe1da9d87f21a1b97b} (*) {'user': 'test'} (glob)


More complex case where part of the rebase set were already rebased

  $ hg rebase --rev 'desc(D)' --dest 'desc(H)'
  rebasing 9:08483444fef9 "D"
  $ hg debugobsolete
  42ccdea3bb16d28e1848c95fe2e44c000f3f21b1 0 {cd010b8cd998f3981a5a8115f94f8da4ab506089} (*) {'user': 'test'} (glob)
  5fddd98957c8a54a4d436dfe1da9d87f21a1b97b 5ae4c968c6aca831df823664e706c9d4aa34473d 0 (*) {'user': 'test'} (glob)
  32af7686d403cf45b5d95f2d70cebea587ac806a 0 {5fddd98957c8a54a4d436dfe1da9d87f21a1b97b} (*) {'user': 'test'} (glob)
  08483444fef91d6224f6655ee586a65d263ad34c 4596109a6a4328c398bde3a4a3b6737cfade3003 0 (*) {'user': 'test'} (glob)
  $ hg log -G
  @  11:4596109a6a43 D
  |
  | o  10:5ae4c968c6ac C
  | |
  | x  9:08483444fef9 D
  | |
  | o  8:8877864f1edb B
  | |
  o |  7:02de42196ebe H
  | |
  | o  6:eea13746799a G
  |/|
  o |  5:24b6387c8c8c F
  | |
  | o  4:9520eea781bc E
  |/
  o  0:cd010b8cd998 A
  
  $ hg rebase --source 'desc(B)' --dest 'tip' --config experimental.rebaseskipobsolete=True
  rebasing 8:8877864f1edb "B"
  note: not rebasing 9:08483444fef9 "D", already in destination as 11:4596109a6a43 "D"
  rebasing 10:5ae4c968c6ac "C"
  $ hg debugobsolete
  42ccdea3bb16d28e1848c95fe2e44c000f3f21b1 0 {cd010b8cd998f3981a5a8115f94f8da4ab506089} (*) {'user': 'test'} (glob)
  5fddd98957c8a54a4d436dfe1da9d87f21a1b97b 5ae4c968c6aca831df823664e706c9d4aa34473d 0 (*) {'user': 'test'} (glob)
  32af7686d403cf45b5d95f2d70cebea587ac806a 0 {5fddd98957c8a54a4d436dfe1da9d87f21a1b97b} (*) {'user': 'test'} (glob)
  08483444fef91d6224f6655ee586a65d263ad34c 4596109a6a4328c398bde3a4a3b6737cfade3003 0 (*) {'user': 'test'} (glob)
  8877864f1edb05d0e07dc4ba77b67a80a7b86672 462a34d07e599b87ea08676a449373fe4e2e1347 0 (*) {'user': 'test'} (glob)
  5ae4c968c6aca831df823664e706c9d4aa34473d 98f6af4ee9539e14da4465128f894c274900b6e5 0 (*) {'user': 'test'} (glob)
  $ hg log --rev 'divergent()'
  $ hg log -G
  o  13:98f6af4ee953 C
  |
  o  12:462a34d07e59 B
  |
  @  11:4596109a6a43 D
  |
  o  7:02de42196ebe H
  |
  | o  6:eea13746799a G
  |/|
  o |  5:24b6387c8c8c F
  | |
  | o  4:9520eea781bc E
  |/
  o  0:cd010b8cd998 A
  
  $ hg log --style default --debug -r 4596109a6a4328c398bde3a4a3b6737cfade3003
  changeset:   11:4596109a6a4328c398bde3a4a3b6737cfade3003
  phase:       draft
  parent:      7:02de42196ebee42ef284b6780a87cdc96e8eaab6
  parent:      -1:0000000000000000000000000000000000000000
  manifest:    11:a91006e3a02f1edf631f7018e6e5684cf27dd905
  user:        Nicolas Dumazet <nicdumz.commits@gmail.com>
  date:        Sat Apr 30 15:24:48 2011 +0200
  files+:      D
  extra:       branch=default
  extra:       rebase_source=08483444fef91d6224f6655ee586a65d263ad34c
  extra:       source=32af7686d403cf45b5d95f2d70cebea587ac806a
  description:
  D
  
  
  $ hg up -qr 'desc(G)'
  $ hg graft 4596109a6a4328c398bde3a4a3b6737cfade3003
  grafting 11:4596109a6a43 "D"
  $ hg up -qr 'desc(E)'
  $ hg rebase -s tip -d .
  rebasing 14:9e36056a46e3 "D" (tip)
  $ hg log --style default --debug -r tip
  changeset:   15:627d4614809036ba22b9e7cb31638ddc06ab99ab
  tag:         tip
  phase:       draft
  parent:      4:9520eea781bcca16c1e15acc0ba14335a0e8e5ba
  parent:      -1:0000000000000000000000000000000000000000
  manifest:    15:648e8ede73ae3e497d093d3a4c8fcc2daa864f42
  user:        Nicolas Dumazet <nicdumz.commits@gmail.com>
  date:        Sat Apr 30 15:24:48 2011 +0200
  files+:      D
  extra:       branch=default
  extra:       intermediate-source=4596109a6a4328c398bde3a4a3b6737cfade3003
  extra:       rebase_source=9e36056a46e37c9776168c7375734eebc70e294f
  extra:       source=32af7686d403cf45b5d95f2d70cebea587ac806a
  description:
  D
  
  
Start rebase from a commit that is obsolete but not hidden only because it's
a working copy parent. We should be moved back to the starting commit as usual
even though it is hidden (until we're moved there).

  $ hg --hidden up -qr 'first(hidden())'
  $ hg rebase --rev 13 --dest 15
  rebasing 13:98f6af4ee953 "C"
  $ hg log -G
  o  16:294a2b93eb4d C
  |
  o  15:627d46148090 D
  |
  | o  12:462a34d07e59 B
  | |
  | o  11:4596109a6a43 D
  | |
  | o  7:02de42196ebe H
  | |
  +---o  6:eea13746799a G
  | |/
  | o  5:24b6387c8c8c F
  | |
  o |  4:9520eea781bc E
  |/
  | @  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  

  $ cd ..

collapse rebase
---------------------------------

  $ hg clone base collapse
  updating to branch default
  3 files updated, 0 files merged, 0 files removed, 0 files unresolved
  $ cd collapse
  $ hg rebase  -s 42ccdea3bb16 -d eea13746799a --collapse
  rebasing 1:42ccdea3bb16 "B"
  rebasing 2:5fddd98957c8 "C"
  rebasing 3:32af7686d403 "D"
  $ hg log -G
  o  8:4dc2197e807b Collapsed revision
  |
  | @  7:02de42196ebe H
  | |
  o |  6:eea13746799a G
  |\|
  | o  5:24b6387c8c8c F
  | |
  o |  4:9520eea781bc E
  |/
  o  0:cd010b8cd998 A
  
  $ hg log --hidden -G
  o  8:4dc2197e807b Collapsed revision
  |
  | @  7:02de42196ebe H
  | |
  o |  6:eea13746799a G
  |\|
  | o  5:24b6387c8c8c F
  | |
  o |  4:9520eea781bc E
  |/
  | x  3:32af7686d403 D
  | |
  | x  2:5fddd98957c8 C
  | |
  | x  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  
  $ hg id --debug -r tip
  4dc2197e807bae9817f09905b50ab288be2dbbcf tip
  $ hg debugobsolete
  42ccdea3bb16d28e1848c95fe2e44c000f3f21b1 4dc2197e807bae9817f09905b50ab288be2dbbcf 0 (*) {'user': 'test'} (glob)
  5fddd98957c8a54a4d436dfe1da9d87f21a1b97b 4dc2197e807bae9817f09905b50ab288be2dbbcf 0 (*) {'user': 'test'} (glob)
  32af7686d403cf45b5d95f2d70cebea587ac806a 4dc2197e807bae9817f09905b50ab288be2dbbcf 0 (*) {'user': 'test'} (glob)

  $ cd ..

Rebase set has hidden descendants
---------------------------------

We rebase a changeset which has a hidden changeset. The hidden changeset must
not be rebased.

  $ hg clone base hidden
  updating to branch default
  3 files updated, 0 files merged, 0 files removed, 0 files unresolved
  $ cd hidden
  $ hg rebase -s 5fddd98957c8 -d eea13746799a
  rebasing 2:5fddd98957c8 "C"
  rebasing 3:32af7686d403 "D"
  $ hg rebase -s 42ccdea3bb16 -d 02de42196ebe
  rebasing 1:42ccdea3bb16 "B"
  $ hg log -G
  o  10:7c6027df6a99 B
  |
  | o  9:cf44d2f5a9f4 D
  | |
  | o  8:e273c5e7d2d2 C
  | |
  @ |  7:02de42196ebe H
  | |
  | o  6:eea13746799a G
  |/|
  o |  5:24b6387c8c8c F
  | |
  | o  4:9520eea781bc E
  |/
  o  0:cd010b8cd998 A
  
  $ hg log --hidden -G
  o  10:7c6027df6a99 B
  |
  | o  9:cf44d2f5a9f4 D
  | |
  | o  8:e273c5e7d2d2 C
  | |
  @ |  7:02de42196ebe H
  | |
  | o  6:eea13746799a G
  |/|
  o |  5:24b6387c8c8c F
  | |
  | o  4:9520eea781bc E
  |/
  | x  3:32af7686d403 D
  | |
  | x  2:5fddd98957c8 C
  | |
  | x  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  
  $ hg debugobsolete
  5fddd98957c8a54a4d436dfe1da9d87f21a1b97b e273c5e7d2d29df783dce9f9eaa3ac4adc69c15d 0 (*) {'user': 'test'} (glob)
  32af7686d403cf45b5d95f2d70cebea587ac806a cf44d2f5a9f4297a62be94cbdd3dff7c7dc54258 0 (*) {'user': 'test'} (glob)
  42ccdea3bb16d28e1848c95fe2e44c000f3f21b1 7c6027df6a99d93f461868e5433f63bde20b6dfb 0 (*) {'user': 'test'} (glob)

Test that rewriting leaving instability behind is allowed
---------------------------------------------------------------------

  $ hg log -r 'children(8)'
  9:cf44d2f5a9f4 D (no-eol)
  $ hg rebase -r 8
  rebasing 8:e273c5e7d2d2 "C"
  $ hg log -G
  o  11:0d8f238b634c C
  |
  o  10:7c6027df6a99 B
  |
  | o  9:cf44d2f5a9f4 D
  | |
  | x  8:e273c5e7d2d2 C
  | |
  @ |  7:02de42196ebe H
  | |
  | o  6:eea13746799a G
  |/|
  o |  5:24b6387c8c8c F
  | |
  | o  4:9520eea781bc E
  |/
  o  0:cd010b8cd998 A
  


Test multiple root handling
------------------------------------

  $ hg rebase --dest 4 --rev '7+11+9'
  rebasing 9:cf44d2f5a9f4 "D"
  rebasing 7:02de42196ebe "H"
  not rebasing ignored 10:7c6027df6a99 "B"
  rebasing 11:0d8f238b634c "C" (tip)
  $ hg log -G
  o  14:1e8370e38cca C
  |
  @  13:bfe264faf697 H
  |
  | o  12:102b4c1d889b D
  |/
  | o  10:7c6027df6a99 B
  | |
  | x  7:02de42196ebe H
  | |
  +---o  6:eea13746799a G
  | |/
  | o  5:24b6387c8c8c F
  | |
  o |  4:9520eea781bc E
  |/
  o  0:cd010b8cd998 A
  
  $ cd ..

Detach both parents

  $ hg init double-detach
  $ cd double-detach

  $ hg debugdrawdag <<EOF
  >   F
  >  /|
  > C E
  > | |
  > B D G
  >  \|/
  >   A
  > EOF

BROKEN: This raises an exception
  $ hg rebase -d G -r 'B + D + F' 2>&1 | grep '^AssertionError'
  AssertionError: no base found to rebase on (defineparents called wrong)

  $ cd ..

test on rebase dropping a merge

(setup)

  $ hg init dropmerge
  $ cd dropmerge
  $ hg unbundle "$TESTDIR/bundles/rebase.hg"
  adding changesets
  adding manifests
  adding file changes
  added 8 changesets with 7 changes to 7 files (+2 heads)
  (run 'hg heads' to see heads, 'hg merge' to merge)
  $ hg up 3
  4 files updated, 0 files merged, 0 files removed, 0 files unresolved
  $ hg merge 7
  2 files updated, 0 files merged, 0 files removed, 0 files unresolved
  (branch merge, don't forget to commit)
  $ hg ci -m 'M'
  $ echo I > I
  $ hg add I
  $ hg ci -m I
  $ hg log -G
  @  9:4bde274eefcf I
  |
  o    8:53a6a128b2b7 M
  |\
  | o  7:02de42196ebe H
  | |
  | | o  6:eea13746799a G
  | |/|
  | o |  5:24b6387c8c8c F
  | | |
  | | o  4:9520eea781bc E
  | |/
  o |  3:32af7686d403 D
  | |
  o |  2:5fddd98957c8 C
  | |
  o |  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  
(actual test)

  $ hg rebase --dest 6 --rev '((desc(H) + desc(D))::) - desc(M)'
  rebasing 3:32af7686d403 "D"
  rebasing 7:02de42196ebe "H"
  not rebasing ignored 8:53a6a128b2b7 "M"
  rebasing 9:4bde274eefcf "I" (tip)
  $ hg log -G
  @  12:acd174b7ab39 I
  |
  o  11:6c11a6218c97 H
  |
  | o  10:b5313c85b22e D
  |/
  | o    8:53a6a128b2b7 M
  | |\
  | | x  7:02de42196ebe H
  | | |
  o---+  6:eea13746799a G
  | | |
  | | o  5:24b6387c8c8c F
  | | |
  o---+  4:9520eea781bc E
   / /
  x |  3:32af7686d403 D
  | |
  o |  2:5fddd98957c8 C
  | |
  o |  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  

Test hidden changesets in the rebase set (issue4504)

  $ hg up --hidden 9
  3 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo J > J
  $ hg add J
  $ hg commit -m J
  $ hg debugobsolete `hg log --rev . -T '{node}'`
  obsoleted 1 changesets

  $ hg rebase --rev .~1::. --dest 'max(desc(D))' --traceback --config experimental.rebaseskipobsolete=off
  rebasing 9:4bde274eefcf "I"
  rebasing 13:06edfc82198f "J" (tip)
  $ hg log -G
  @  15:5ae8a643467b J
  |
  o  14:9ad579b4a5de I
  |
  | o  12:acd174b7ab39 I
  | |
  | o  11:6c11a6218c97 H
  | |
  o |  10:b5313c85b22e D
  |/
  | o    8:53a6a128b2b7 M
  | |\
  | | x  7:02de42196ebe H
  | | |
  o---+  6:eea13746799a G
  | | |
  | | o  5:24b6387c8c8c F
  | | |
  o---+  4:9520eea781bc E
   / /
  x |  3:32af7686d403 D
  | |
  o |  2:5fddd98957c8 C
  | |
  o |  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  
  $ hg up 14 -C
  0 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo "K" > K
  $ hg add K
  $ hg commit --amend -m "K"
  $ echo "L" > L
  $ hg add L
  $ hg commit -m "L"
  $ hg up '.^'
  0 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo "M" > M
  $ hg add M
  $ hg commit --amend -m "M"
  $ hg log -G
  @  20:bfaedf8eb73b M
  |
  | o  18:97219452e4bd L
  | |
  | x  17:fc37a630c901 K
  |/
  | o  15:5ae8a643467b J
  | |
  | x  14:9ad579b4a5de I
  |/
  | o  12:acd174b7ab39 I
  | |
  | o  11:6c11a6218c97 H
  | |
  o |  10:b5313c85b22e D
  |/
  | o    8:53a6a128b2b7 M
  | |\
  | | x  7:02de42196ebe H
  | | |
  o---+  6:eea13746799a G
  | | |
  | | o  5:24b6387c8c8c F
  | | |
  o---+  4:9520eea781bc E
   / /
  x |  3:32af7686d403 D
  | |
  o |  2:5fddd98957c8 C
  | |
  o |  1:42ccdea3bb16 B
  |/
  o  0:cd010b8cd998 A
  
  $ hg rebase -s 14 -d 18 --config experimental.rebaseskipobsolete=True
  note: not rebasing 14:9ad579b4a5de "I", already in destination as 17:fc37a630c901 "K"
  rebasing 15:5ae8a643467b "J"

  $ cd ..

Skip obsolete changeset even with multiple hops
-----------------------------------------------

setup

  $ hg init obsskip
  $ cd obsskip
  $ cat << EOF >> .hg/hgrc
  > [experimental]
  > rebaseskipobsolete = True
  > [extensions]
  > strip =
  > EOF
  $ echo A > A
  $ hg add A
  $ hg commit -m A
  $ echo B > B
  $ hg add B
  $ hg commit -m B0
  $ hg commit --amend -m B1
  $ hg commit --amend -m B2
  $ hg up --hidden 'desc(B0)'
  0 files updated, 0 files merged, 0 files removed, 0 files unresolved
  $ echo C > C
  $ hg add C
  $ hg commit -m C

Rebase finds its way in a chain of marker

  $ hg rebase -d 'desc(B2)'
  note: not rebasing 1:a8b11f55fb19 "B0", already in destination as 3:261e70097290 "B2"
  rebasing 4:212cb178bcbb "C" (tip)

Even when the chain include missing node

  $ hg up --hidden 'desc(B0)'
  0 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo D > D
  $ hg add D
  $ hg commit -m D
  $ hg --hidden strip -r 'desc(B1)'
  saved backup bundle to $TESTTMP/obsskip/.hg/strip-backup/86f6414ccda7-b1c452ee-backup.hg (glob)

  $ hg rebase -d 'desc(B2)'
  note: not rebasing 1:a8b11f55fb19 "B0", already in destination as 2:261e70097290 "B2"
  rebasing 5:1a79b7535141 "D" (tip)
  $ hg up 4
  1 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo "O" > O
  $ hg add O
  $ hg commit -m O
  $ echo "P" > P
  $ hg add P
  $ hg commit -m P
  $ hg log -G
  @  8:8d47583e023f P
  |
  o  7:360bbaa7d3ce O
  |
  | o  6:9c48361117de D
  | |
  o |  4:ff2c4d47b71d C
  |/
  o  2:261e70097290 B2
  |
  o  0:4a2df7238c3b A
  
  $ hg debugobsolete `hg log -r 7 -T '{node}\n'` --config experimental.evolution=all
  obsoleted 1 changesets
  $ hg rebase -d 6 -r "4::"
  rebasing 4:ff2c4d47b71d "C"
  note: not rebasing 7:360bbaa7d3ce "O", it has no successor
  rebasing 8:8d47583e023f "P" (tip)

If all the changeset to be rebased are obsolete and present in the destination, we
should display a friendly error message

  $ hg log -G
  @  10:121d9e3bc4c6 P
  |
  o  9:4be60e099a77 C
  |
  o  6:9c48361117de D
  |
  o  2:261e70097290 B2
  |
  o  0:4a2df7238c3b A
  

  $ hg up 9
  0 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo "non-relevant change" > nonrelevant
  $ hg add nonrelevant
  $ hg commit -m nonrelevant
  created new head
  $ hg debugobsolete `hg log -r 11 -T '{node}\n'` --config experimental.evolution=all
  obsoleted 1 changesets
  $ hg rebase -r . -d 10
  note: not rebasing 11:f44da1f4954c "nonrelevant" (tip), it has no successor

If a rebase is going to create divergence, it should abort

  $ hg log -G
  @  11:f44da1f4954c nonrelevant
  |
  | o  10:121d9e3bc4c6 P
  |/
  o  9:4be60e099a77 C
  |
  o  6:9c48361117de D
  |
  o  2:261e70097290 B2
  |
  o  0:4a2df7238c3b A
  

  $ hg up 9
  0 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo "john" > doe
  $ hg add doe
  $ hg commit -m "john doe"
  created new head
  $ hg up 10
  1 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo "foo" > bar
  $ hg add bar
  $ hg commit --amend -m "10'"
  $ hg up 10 --hidden
  0 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo "bar" > foo
  $ hg add foo
  $ hg commit -m "bar foo"
  $ hg log -G
  @  15:73568ab6879d bar foo
  |
  | o  14:77d874d096a2 10'
  | |
  | | o  12:3eb461388009 john doe
  | |/
  x |  10:121d9e3bc4c6 P
  |/
  o  9:4be60e099a77 C
  |
  o  6:9c48361117de D
  |
  o  2:261e70097290 B2
  |
  o  0:4a2df7238c3b A
  
  $ hg summary
  parent: 15:73568ab6879d tip (orphan)
   bar foo
  branch: default
  commit: (clean)
  update: 2 new changesets, 3 branch heads (merge)
  phases: 8 draft
  orphan: 1 changesets
  $ hg rebase -s 10 -d 12
  abort: this rebase will cause divergences from: 121d9e3bc4c6
  (to force the rebase please set experimental.allowdivergence=True)
  [255]
  $ hg log -G
  @  15:73568ab6879d bar foo
  |
  | o  14:77d874d096a2 10'
  | |
  | | o  12:3eb461388009 john doe
  | |/
  x |  10:121d9e3bc4c6 P
  |/
  o  9:4be60e099a77 C
  |
  o  6:9c48361117de D
  |
  o  2:261e70097290 B2
  |
  o  0:4a2df7238c3b A
  
With experimental.allowdivergence=True, rebase can create divergence

  $ hg rebase -s 10 -d 12 --config experimental.allowdivergence=True
  rebasing 10:121d9e3bc4c6 "P"
  rebasing 15:73568ab6879d "bar foo" (tip)
  $ hg summary
  parent: 17:61bd55f69bc4 tip
   bar foo
  branch: default
  commit: (clean)
  update: 1 new changesets, 2 branch heads (merge)
  phases: 8 draft
  divergent: 2 changesets

rebase --continue + skipped rev because their successors are in destination
we make a change in trunk and work on conflicting changes to make rebase abort.

  $ hg log -G -r 17::
  @  17:61bd55f69bc4 bar foo
  |
  ~

Create the two changes in trunk
  $ printf "a" > willconflict
  $ hg add willconflict
  $ hg commit -m "willconflict first version"

  $ printf "dummy" > C
  $ hg commit -m "dummy change successor"

Create the changes that we will rebase
  $ hg update -C 17 -q
  $ printf "b" > willconflict
  $ hg add willconflict
  $ hg commit -m "willconflict second version"
  created new head
  $ printf "dummy" > K
  $ hg add K
  $ hg commit -m "dummy change"
  $ printf "dummy" > L
  $ hg add L
  $ hg commit -m "dummy change"
  $ hg debugobsolete `hg log -r ".^" -T '{node}'` `hg log -r 19 -T '{node}'` --config experimental.evolution=all
  obsoleted 1 changesets

  $ hg log -G -r 17::
  @  22:7bdc8a87673d dummy change
  |
  x  21:8b31da3c4919 dummy change
  |
  o  20:b82fb57ea638 willconflict second version
  |
  | o  19:601db7a18f51 dummy change successor
  | |
  | o  18:357ddf1602d5 willconflict first version
  |/
  o  17:61bd55f69bc4 bar foo
  |
  ~
  $ hg rebase -r ".^^ + .^ + ." -d 19
  rebasing 20:b82fb57ea638 "willconflict second version"
  merging willconflict
  warning: conflicts while merging willconflict! (edit, then use 'hg resolve --mark')
  unresolved conflicts (see hg resolve, then hg rebase --continue)
  [1]

  $ hg resolve --mark willconflict
  (no more unresolved files)
  continue: hg rebase --continue
  $ hg rebase --continue
  rebasing 20:b82fb57ea638 "willconflict second version"
  note: not rebasing 21:8b31da3c4919 "dummy change", already in destination as 19:601db7a18f51 "dummy change successor"
  rebasing 22:7bdc8a87673d "dummy change" (tip)
  $ cd ..

Rebase merge where successor of one parent is equal to destination (issue5198)

  $ hg init p1-succ-is-dest
  $ cd p1-succ-is-dest

  $ hg debugdrawdag <<EOF
  >   F
  >  /|
  > E D B # replace: D -> B
  >  \|/
  >   A
  > EOF

  $ hg rebase -d B -s D
  note: not rebasing 2:b18e25de2cf5 "D" (D), already in destination as 1:112478962961 "B"
  rebasing 4:66f1a38021c9 "F" (F tip)
  $ hg log -G
  o    5:50e9d60b99c6 F
  |\
  | | x  4:66f1a38021c9 F
  | |/|
  | o |  3:7fb047a69f22 E
  | | |
  | | x  2:b18e25de2cf5 D
  | |/
  o |  1:112478962961 B
  |/
  o  0:426bada5c675 A
  
  $ cd ..

Rebase merge where successor of other parent is equal to destination

  $ hg init p2-succ-is-dest
  $ cd p2-succ-is-dest

  $ hg debugdrawdag <<EOF
  >   F
  >  /|
  > E D B # replace: E -> B
  >  \|/
  >   A
  > EOF

BROKEN: Raises an exception
  $ hg rebase -d B -s E 2>&1 | grep AssertionError:
  AssertionError: no base found to rebase on (defineparents called wrong)
  $ hg log -G
  o    4:66f1a38021c9 F
  |\
  | x  3:7fb047a69f22 E
  | |
  o |  2:b18e25de2cf5 D
  |/
  | o  1:112478962961 B
  |/
  o  0:426bada5c675 A
  
  $ cd ..

Rebase merge where successor of one parent is ancestor of destination

  $ hg init p1-succ-in-dest
  $ cd p1-succ-in-dest

  $ hg debugdrawdag <<EOF
  >   F C
  >  /| |
  > E D B # replace: D -> B
  >  \|/
  >   A
  > EOF

  $ hg rebase -d C -s D
  note: not rebasing 2:b18e25de2cf5 "D" (D), already in destination as 1:112478962961 "B"
  rebasing 5:66f1a38021c9 "F" (F tip)
BROKEN: not rebased on top of requested destination (C)
  $ hg log -G
  o    6:50e9d60b99c6 F
  |\
  | | x  5:66f1a38021c9 F
  | |/|
  +-----o  4:26805aba1e60 C
  | | |
  | o |  3:7fb047a69f22 E
  | | |
  | | x  2:b18e25de2cf5 D
  | |/
  o |  1:112478962961 B
  |/
  o  0:426bada5c675 A
  
  $ cd ..

Rebase merge where successor of other parent is ancestor of destination

  $ hg init p2-succ-in-dest
  $ cd p2-succ-in-dest

  $ hg debugdrawdag <<EOF
  >   F C
  >  /| |
  > E D B # replace: E -> B
  >  \|/
  >   A
  > EOF

BROKEN: Raises an exception
  $ hg rebase -d C -s E 2>&1 | grep AssertionError:
  AssertionError: no base found to rebase on (defineparents called wrong)
  $ hg log -G
  o    5:66f1a38021c9 F
  |\
  | | o  4:26805aba1e60 C
  | | |
  | x |  3:7fb047a69f22 E
  | | |
  o | |  2:b18e25de2cf5 D
  |/ /
  | o  1:112478962961 B
  |/
  o  0:426bada5c675 A
  
  $ cd ..

Rebase merge where successor of one parent is ancestor of destination

  $ hg init p1-succ-in-dest-b
  $ cd p1-succ-in-dest-b

  $ hg debugdrawdag <<EOF
  >   F C
  >  /| |
  > E D B # replace: E -> B
  >  \|/
  >   A
  > EOF

  $ hg rebase -d C -b F
  rebasing 2:b18e25de2cf5 "D" (D)
  note: not rebasing 3:7fb047a69f22 "E" (E), already in destination as 1:112478962961 "B"
  rebasing 5:66f1a38021c9 "F" (F tip)
  $ hg log -G
  o  7:9ed45af61fa0 F
  |
  o  6:8f47515dda15 D
  |
  | x    5:66f1a38021c9 F
  | |\
  o | |  4:26805aba1e60 C
  | | |
  | | x  3:7fb047a69f22 E
  | | |
  | x |  2:b18e25de2cf5 D
  | |/
  o /  1:112478962961 B
  |/
  o  0:426bada5c675 A
  
  $ cd ..

Rebase merge where successor of other parent is ancestor of destination

  $ hg init p2-succ-in-dest-b
  $ cd p2-succ-in-dest-b

  $ hg debugdrawdag <<EOF
  >   F C
  >  /| |
  > E D B # replace: D -> B
  >  \|/
  >   A
  > EOF

  $ hg rebase -d C -b F
  note: not rebasing 2:b18e25de2cf5 "D" (D), already in destination as 1:112478962961 "B"
  rebasing 3:7fb047a69f22 "E" (E)
  rebasing 5:66f1a38021c9 "F" (F tip)
BROKEN: This should have resulted in a rebased F with one parent, just like in
the test case above
  $ hg log -G
  o    7:c1e6f26e339d F
  |\
  | o  6:533690786a86 E
  |/
  | x    5:66f1a38021c9 F
  | |\
  o | |  4:26805aba1e60 C
  | | |
  | | x  3:7fb047a69f22 E
  | | |
  | x |  2:b18e25de2cf5 D
  | |/
  o /  1:112478962961 B
  |/
  o  0:426bada5c675 A
  
  $ cd ..

Test that bookmark is moved and working dir is updated when all changesets have
equivalents in destination
  $ hg init rbsrepo && cd rbsrepo
  $ echo "[experimental]" > .hg/hgrc
  $ echo "evolution=all" >> .hg/hgrc
  $ echo "rebaseskipobsolete=on" >> .hg/hgrc
  $ echo root > root && hg ci -Am root
  adding root
  $ echo a > a && hg ci -Am a
  adding a
  $ hg up 0
  0 files updated, 0 files merged, 1 files removed, 0 files unresolved
  $ echo b > b && hg ci -Am b
  adding b
  created new head
  $ hg rebase -r 2 -d 1
  rebasing 2:1e9a3c00cbe9 "b" (tip)
  $ hg log -r .  # working dir is at rev 3 (successor of 2)
  3:be1832deae9a b (no-eol)
  $ hg book -r 2 mybook --hidden  # rev 2 has a bookmark on it now
  $ hg up 2 && hg log -r .  # working dir is at rev 2 again
  0 files updated, 0 files merged, 1 files removed, 0 files unresolved
  2:1e9a3c00cbe9 b (no-eol)
  $ hg rebase -r 2 -d 3 --config experimental.evolution.track-operation=1
  note: not rebasing 2:1e9a3c00cbe9 "b" (mybook), already in destination as 3:be1832deae9a "b"
Check that working directory was updated to rev 3 although rev 2 was skipped
during the rebase operation
  $ hg log -r .
  3:be1832deae9a b (no-eol)

Check that bookmark was not moved to rev 3 if rev 2 was skipped during the
rebase operation. This makes sense because if rev 2 has a successor, the
operation generating that successor (ex. rebase) should be responsible for
moving bookmarks. If the bookmark is on a precursor, like rev 2, that means the
user manually moved it back. In that case we should not move it again.
  $ hg bookmarks
     mybook                    2:1e9a3c00cbe9
  $ hg debugobsolete --rev tip
  1e9a3c00cbe90d236ac05ef61efcc5e40b7412bc be1832deae9ac531caa7438b8dcf6055a122cd8e 0 (*) {'user': 'test'} (glob)
