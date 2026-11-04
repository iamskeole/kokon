# BFL prompt bundle — FLUX.3 Image (Klein) prompt upsampler

These files are contract assets of BFL's FLUX.3 Image (Klein) prompt
upsampler (PUP): the four upsampler system prompts, the chat template,
and the tokenizer / preprocessor configs.

- **License**: Apache-2.0 (as published by the source).
- **Source**: `black-forest-labs/flux-3-image-klein-pup` on Hugging Face,
  commit `3f3edb52b2c55de0d49feab28598b68f7f8e4830` (last modified
  2026-10-15; this copy was taken from the source on 2026-11-01 and
  verified byte-for-byte against the repo's tokenizer).
- If BFL re-releases the prompts: re-copy the files and bump the
  recorded commit in this note.

## Files (sha256)

| file | sha256 |
|---|---|
| system_prompts.json | b5da3ed8bc04ab59dbdb747930f7dbff847e868903fed8028f85ee1a4ac82fb4 |
| chat_template.jinja | a4aee8afcf2e0711942cf848899be66016f8d14a889ff9ede07bca099c28f715 |
| preprocessor_config.json | 3a159dfec9978a186a72ba085e0ad6a050f3968d8b364218d7bd13f5c89381f2 |
| processor_config.json | d89ef49ce9cd37fbf510158e13c1ef063d9286411c1ec9049932dbe0487143b1 |
| tokenizer_config.json | 7488550a894dcf741616126e322ea31d66cf9457b59ec5825f4c7efdced421f8 |
| config.json | 7d7fc026d8bcb9f1fc505af1004779330fe59bb3256fe0271ec2568e8dc89db1 |
| request_format.json* | 347a691187996af71da34f3ea0977dc8d8574c3628a1d6291379d6f8804ff00a |

\* `request_format.json` is Kokon-authored, not part of the source repo: it carries
the two request-line formats BFL's service sends (the t2i canvas line and the
image-scaffold), kept here as data so their text is read at runtime instead of being
compiled in.

The repo's `tokenizer.json` (20 MB, BPE, sha256
`06b9509352d2af50381ab2247e083b80d32d5c0aba91c272ca9ff729b6a0e523`) is
**not committed** — the repo does not carry large files, and the sidecar
tokenizes from its GGUF. It remains available from the source repo
above.

**Contract silence**: these assets are BFL contract material. They live
in the repository only as data files, are read at runtime by the
service, and their contents are never echoed in logs, chat, docs, or
diffs.
