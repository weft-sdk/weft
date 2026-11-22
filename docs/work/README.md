# Work files

One file per branch in progress, named after the branch with `/` replaced by `-` (branch `feat/w2-site-skeleton` → `docs/work/feat-w2-site-skeleton.md`). It is the handoff between sessions: whoever picks up the branch reads it first and continues from **Next step**. How it fits with the rest: [14-workflow.md](../14-workflow.md).

The file is the branch's first commit, and is deleted in the pull request that merges the branch, once its lasting content is in the roadmap, a design doc or a decision record. On `main` this directory holds only this README.

## Template

```markdown
# <task id>: <title>

- Branch: `<branch>`
- Task: <task id> in the roadmap, needs <dependencies>
- Started: YYYY-MM-DD · Last session: YYYY-MM-DD
- Pushed: yes / no

## Goal

What is true when this is done, in one or two sentences, and how it is shown (the task's "done when").

## Plan

1. Step, with the files it touches.
2. ...

## Decisions made on this branch

- Decision, and why. (Anything that outlives the branch also gets a decision record.)

## Done

- What is finished and checked, by commit.

## Next step

The very next thing to do, specific enough to start on cold.

## Waiting on

- Anything only the maintainer can do. "Nothing" if so.

## How to check

    <commands that show the work so far is sound>

## Found, not done

- Out-of-scope things noticed on the way, for a later task.
```
