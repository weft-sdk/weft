# Workflow

How any session (a person or a coding agent) picks up Weft, works on it and hands it off, so that no work depends on one chat remembering anything. Everything a session needs is in the repository.

## Where state lives

| What | Where | Describes |
| --- | --- | --- |
| What is built, what is next | [Roadmap › Current status](roadmap.md#current-status) and the phase tables | `main` only |
| The work on one branch | `docs/work/<branch>.md` on that branch ([template](work/README.md)) | That branch only |
| Why things are as they are | [Decision records](decisions/) | Accepted decisions |
| What is undecided | [Open questions](open-questions.md) | Decisions waiting on the maintainer |
| How we got here, what was rejected | [History](history.md) | The path so far |

Two rules keep parallel work from colliding:

- **Current status describes `main`.** A branch changes it only in the pull request that finishes its task.
- **Everything in progress is in the branch's work file.** Each branch has its own file, so sessions on different branches never edit the same one.

A conversation, a session's private memory, or anything said only in a chat is not state. If it matters, it goes into a work file, a decision record, an open question or a design doc before the session ends.

## Tasks and branches

Tasks have ids (`W3`, `P1.2`) and dependencies in the [roadmap](roadmap.md). A task whose dependencies are all on `main` can be started.

Branch names: `<type>/<task id>-<short-name>`, lowercase, for example `feat/w2-site-skeleton`, `docs/w1-visual-direction`. `<type>` is one of `feat`, `fix`, `docs`, `refactor`, `test`, `chore`.

Small changes that belong to no task (a typo, a broken link) can go on a `chore/<short-name>` branch without a work file.

## Starting a session

1. `git status`, `git branch --list`, `git worktree list`, and `git fetch` to see remote branches.
2. **On a task branch:** read `docs/work/<branch with / replaced by ->.md` and continue from its **Next step**. Do not re-plan what it already decided.
3. **On `main`:**
   - Read [Roadmap › Current status](roadmap.md#current-status), then the active phase's tasks.
   - Take the first task whose dependencies are done and that nobody has taken: no local branch or worktree with its id (`git branch --list '*w3*'`, `git worktree list`) and none on the remote (`git branch -r`).
   - If it depends on an [open question](open-questions.md), stop and ask the maintainer.
   - Create the branch (in its own worktree if another session may be running), and its work file from the [template](work/README.md) with the goal and a plan. Commit the work file first.
4. Read the documents [CLAUDE.md](../CLAUDE.md) lists, as far as the task needs.

## Parallel sessions

Two sessions never share a working tree. Give each its own worktree:

```bash
git worktree add ../weft-w3 -b feat/w3-landing-page main
cd ../weft-w3
```

Only one session runs a dev server on the default port at a time. When the branch is merged: `git worktree remove ../weft-w3`.

## During a session

- Small commits, each one building and passing its checks.
- Update the work file as decisions are made, not only at the end. A session can stop at any moment.
- A decision that outlives the branch gets a [decision record](decisions/) on the same branch.
- Anything out of scope goes in the work file's **Found, not done** list.
- Anything only the maintainer can do (below) is written in the work file's **Waiting on**, and the session moves to something else or stops.

## Ending a session

Before stopping, even part-way:

1. Everything is committed. Nothing is left only in the working tree.
2. The work file says, in plain words:
   - **Done:** what is finished and checked, by commit.
   - **Next step:** the very next thing to do, specific enough to start on cold.
   - **Waiting on:** anything only the maintainer can do.
   - **How to check:** the commands that show the work so far is sound.
3. Push the branch if the maintainer has approved pushing for this task. Otherwise say it is not pushed.

## Finishing a task

Ready when:

- The task's "done when" in the roadmap holds, and its checks pass.
- The docs describe what now exists: the roadmap task marked done, design docs accurate, decision records for new decisions, open questions removed when answered.
- [Current status](roadmap.md#current-status) is updated for what `main` will be after the merge.
- The work file's lasting content has moved into those documents, and the work file is **deleted** in the same pull request. On `main`, `docs/work/` holds only its README.
- The pull request description says what changed, why, how it was checked, and what was not done.

If two finished branches both changed Current status, the second to merge rebases and rewrites the status to include both.

## What only the maintainer does

- Decide open questions and accept decision records that change direction.
- Approve design: the website's visual direction and anything user-facing that sets the brand.
- Merge to `main`. Push and open pull requests, unless approved for the task.
- Buy domains, create accounts and organisations, reserve or publish packages, deploy to production.
- Anything that costs money.
