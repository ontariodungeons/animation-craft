# AI–Human MOPPETS · Magic Studio cast

Open **Learning → MOPPETS cast**. Five departments each have twelve original
candidates and three suggested leads. Jesse leads the Review Desk. Everyone
starts at audition; casting edits a profile, and never starts an agent.

Each `departments/<department>/AGENTS.md` is the shared source: JSON frontmatter
holds the allowed roles, lead and MOP profiles; Markdown holds the working rules.
The page edits and saves that same file. Candidates and the wings are an idea
pool; only cast members can lead. Reserves wait for an assignment.

`mc exec crew.list` reads the same documents. `mc commands crew` describes saving.
For a scripted edit, send `crew.save` through `mc run` with the returned `workspace`,
`department`, `expected_revision`, and edited `document`. A stale revision fails
without overwriting the file. The page retains drafts across navigation and
offers recovery after loading a saved version. Saves do not commit or deploy.

Models and capabilities are profile choices, not usage measurements. These
auditions do not claim live sessions or token totals. Department instructions
supplement the repository and the human’s current instructions.

The older HTML in `../muppet-agent-studio` was a layout reference. Its named
characters, agent launch workflow and external CDN dependencies are not used here.
