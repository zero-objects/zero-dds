#!/usr/bin/env bash
# Build what is about to be committed, not what is on disk.
#
# Three times this cycle a commit shipped a module file and left the
# `pub mod` line that declares it unstaged. HEAD did not build. Every
# check in scripts/pre-commit.sh — fmt, clippy, the tests, the
# silent-drop guard — reads the **working tree**, where the declaration
# was present, so all four were green while the commit they were
# guarding was not. Each of the three was repaired by hand, and each
# time by a different person noticing.
#
# The class is wider than one missing line. Anything a build needs that
# is on disk and not in the index produces it: a manifest that was never
# `git add`ed, a `Cargo.lock` left behind by a dependency change, a
# generated file nobody tracked. `crates/lang-python` was removed from
# the index by a commit while staying on disk and in the workspace
# member list, and nothing noticed for hours — cargo read the file it
# could see.
#
# So this guard reads the **index**, in a tree of its own, and never the
# working tree. It runs two checks:
#
#   1. every workspace member has a tracked manifest. Fast, and it
#      names the member — cargo's own error for this is a `failed to
#      read` on an absolute path inside a temporary directory, which
#      says nothing about which member list entry is wrong;
#   2. `cargo check --workspace --all-targets --locked` over the staged
#      tree. `check` and not `build` because every failure of this class
#      is a name that does not resolve, which `check` reaches at a third
#      of the cost; `--locked` because a manifest staged without its
#      `Cargo.lock` is the same hole one level up.
#
#   bash tools/staged-tree-guard.sh                    check the index
#   bash tools/staged-tree-guard.sh --head             check HEAD
#   bash tools/staged-tree-guard.sh --paths a.rs b.rs  check HEAD + those
#
# `--paths` exists because `git commit -- <path>…` does not commit the
# index: it commits HEAD with those paths' staged content laid over it.
# Work here is committed that way — several agents share this checkout
# and each stages only its own files — so a guard that only understood
# whole-index commits would answer for a tree nobody is about to write.
# It builds the tree in a temporary index file and never touches the
# real one.
#
# Why an extracted tree and not `git worktree add`: a worktree writes an
# entry under `.git/worktrees/` that outlives an interrupted run, and
# this repository is shared with other agents — a guard must not leave
# repository state behind when it is killed. `git archive` of the index
# tree writes nothing into `.git` at all, and the extraction directory
# is under `target/`, which is ignored.
#
# Wired into scripts/pre-commit.sh, .github/workflows/ci.yml and
# .gitlab-ci.yml, all three calling this same script so the gate cannot
# drift between them — the same arrangement tools/silent-drop-guard.sh
# has.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT=$(pwd)

SOURCE=index
PATHS=()
case ${1:-} in
    "") ;;
    --head) SOURCE=HEAD ;;
    --paths)
        shift
        SOURCE="HEAD plus staged paths"
        PATHS=("$@")
        if ((${#PATHS[@]} == 0)); then
            echo "usage: bash tools/staged-tree-guard.sh --paths <path>..." >&2
            exit 2
        fi
        ;;
    *)
        echo "usage: bash tools/staged-tree-guard.sh [--head | --paths <path>...]" >&2
        exit 2
        ;;
esac

# A fixed path rather than `mktemp -d`, so that cargo's fingerprints —
# which record absolute source paths — match between runs and a second
# invocation recompiles only what changed. The entire extraction AND check
# must own a lock: Cargo's target lock cannot stop another guard from deleting
# this source tree before that other guard reaches Cargo.
GUARD_ROOT=$ROOT/target/staged-tree-guard
mkdir -p "$GUARD_ROOT"
if command -v flock >/dev/null 2>&1; then
    exec 9>"$GUARD_ROOT/snapshot.lock"
    echo "Waiting for this snapshot directory's guard lock: $GUARD_ROOT"
    flock 9
    TREE=$GUARD_ROOT/tree
else
    # macOS does not ship flock. An isolated source directory keeps concurrent
    # checks safe there, at the cost of cache reuse for absolute source paths.
    # It is retained on failure like the fixed Linux snapshot.
    TREE=$(mktemp -d "$GUARD_ROOT/tree.XXXXXX")
fi
# A target directory of its own, so a concurrent `cargo` in the working
# tree neither waits on this one's lock nor has its artefacts evicted by
# it. STAGED_TREE_TARGET_DIR points it somewhere a CI cache already
# keeps, where that is worth more than the isolation.
CARGO_TARGET_DIR=${STAGED_TREE_TARGET_DIR:-$ROOT/target/staged-tree-guard/target}
export CARGO_TARGET_DIR

case $SOURCE in
    index)
        # write-tree turns the index into a tree object without touching
        # the index, HEAD, or any ref.
        TREEISH=$(git write-tree)
        ;;
    HEAD)
        TREEISH=$(git rev-parse 'HEAD^{tree}')
        ;;
    *)
        # HEAD with the named paths' staged content laid over it, built
        # in an index file of its own. GIT_INDEX_FILE keeps every write
        # below out of .git/index, so a concurrent agent's staging is
        # neither read nor disturbed.
        SCRATCH_INDEX=$(mktemp -t staged-tree-guard-index)
        trap 'rm -f "$SCRATCH_INDEX"' EXIT
        GIT_INDEX_FILE=$SCRATCH_INDEX
        export GIT_INDEX_FILE
        git read-tree HEAD
        # `--` and `git add` rather than `update-index`, so a path that
        # is staged-deleted is applied as a deletion too.
        git add --all -- "${PATHS[@]}"
        TREEISH=$(git write-tree)
        unset GIT_INDEX_FILE
        ;;
esac

echo "== staged-tree guard: $SOURCE, tree $TREEISH =="

rm -rf "$TREE"
mkdir -p "$TREE"
git archive --format=tar "$TREEISH" | tar -x -C "$TREE"

# --- 1. every workspace member has a tracked manifest ----------------
#
# Read the member list out of the staged manifest rather than the
# working-tree one: a member added on disk and not staged is not part of
# what is being committed, and a member removed on disk but still staged
# is.
if [[ ! -f $TREE/Cargo.toml ]]; then
    echo "FAIL: the $SOURCE tree has no Cargo.toml at the root." >&2
    exit 1
fi

# Everything between `members = [` and the closing `]` of the
# `[workspace]` table, as one quoted path per line. `exclude` is read
# the same way and is not checked: an excluded path is deliberately not
# built.
members=$(
    awk '
        /^\[/            { in_ws = ($0 ~ /^\[workspace\]/); next }
        !in_ws           { next }
        /^members[ \t]*=/ { collecting = 1 }
        collecting {
            # A comment is not a member: `(The Cargo Book, "Workspaces")`
            # in the list read as a member named `Workspaces`.
            line = $0
            sub(/[ \t]*#.*$/, "", line)
            print line
            if (line ~ /\]/) { collecting = 0 }
        }
    ' "$TREE/Cargo.toml" | grep -o '"[^"]*"' | tr -d '"'
)

if [[ -z $members ]]; then
    echo "FAIL: no [workspace] members found in the $SOURCE tree's Cargo.toml." >&2
    exit 1
fi

missing=()
member_count=0
while IFS= read -r member; do
    [[ -z $member ]] && continue
    member_count=$((member_count + 1))
    if [[ $member == *"*"* ]]; then
        # A glob member. It is a hole of the same class when it matches
        # nothing: the member list names crates that are not there.
        # shellcheck disable=SC2086
        matches=$(compgen -G "$TREE/$member/Cargo.toml" || true)
        if [[ -z $matches ]]; then
            missing+=("$member (glob matched no Cargo.toml)")
        fi
        continue
    fi
    if [[ ! -f "$TREE/$member/Cargo.toml" ]]; then
        missing+=("$member")
    fi
done <<<"$members"

if ((${#missing[@]} > 0)); then
    {
        echo
        echo "FAIL: $((${#missing[@]})) workspace member(s) have no tracked Cargo.toml"
        echo "      in the $SOURCE tree:"
        echo
        for m in "${missing[@]}"; do
            echo "  $m"
        done
        cat <<'MSG'

The manifest is on disk — cargo builds here — and it is not in what is
about to be committed, so it will not be there for anyone else. Stage
it:

    git add <member>/Cargo.toml

or, if the crate is meant to be gone, remove it from the `members` list
in Cargo.toml and stage that.
MSG
    } >&2
    exit 1
fi

echo "   $member_count workspace member(s), each with a tracked manifest"

# --- 2. the staged tree compiles -------------------------------------
#
# `--offline` is deliberately not passed: a dependency added in this
# commit has to be fetchable, and a guard that could not see the network
# would report a green for a manifest nobody else can build.
echo "== cargo check (staged tree, --workspace --all-targets --locked) =="
if ! (cd "$TREE" && cargo check --workspace --all-targets --locked); then
    cat >&2 <<MSG

FAIL: the $SOURCE tree does not compile, while the working tree may.

Every difference between the two is a file that is on disk and not in
what is about to be committed. The usual one is a module file staged
without the \`pub mod\` line that declares it, or the reverse. To see the
difference:

    git diff --name-only          # changed on disk, not staged
    git status --short

The failing tree is left at $TREE for inspection.
MSG
    exit 1
fi

echo "The $SOURCE tree builds."
