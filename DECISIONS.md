# Decisions

Dated entries. Each quotes the person's request word for word, then says what the agent decided and where it is recorded. Grants of autonomy go here too ("you may publish"), with their scope: this run only, or until withdrawn.

## 2026-10-14

> Use sgilson7.github.io builder setting, which can be found one directory up. I approve the creation of a public github repo, and a public github pages to host the tool. I approve deployment to the website in order to update the tools section.  Use the shared technical setting within to create the following tool for me: an mp4 to zoom style transcript, that assigns the speakers in the transcript anonymous ids

The agent decided:

- The tool is a Builder project copied from `builder-setting/template/` (0.2.0) into `Mp4ToTranscript/`, the directory the person made for it.
- Grant to publish, until withdrawn: create the public repository `sgilson7/mp4-to-transcript` (the person named none; the name follows the tool's title and the site's other tool repositories) and publish it to GitHub Pages at `https://sgilson7.github.io/mp4-to-transcript/` with `./scripts/publish.sh` once `./scripts/check.sh` passes.
- Grant to update the website, until withdrawn: list the tool in the tools section of `sgilson7.github.io` (the `website` repository, `content/projects.yaml` under research tools, as YouTube to MP3 was listed) and deploy the site once `make check` and the site's gate pass.
- "Anonymous ids" are `Speaker 1`, `Speaker 2` and so on, numbered in the order the voices first speak. Nothing about a person is inferred or kept.
- "Zoom style" means both layouts Zoom gives: the WebVTT file of a cloud recording, and the text file of a saved meeting transcript.
