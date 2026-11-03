# Releasing the JS packages

`.github/workflows/js.yml` builds, tests and publishes `@acacia-rs/bot`. Only `@acacia-rs/bot` is published for now:
`@acacia-rs/mcp` depends on it through `file:../acacia-bot`, which would have to become a version range first.

What gets published: the main package (TS layer, generated loader) plus one package per `napi.targets` entry
(`@acacia-rs/bot-linux-x64-gnu`, …), which `napi prepublish` publishes first and pins as exact-version
`optionalDependencies`. npm can't publish several packages atomically, so a failure half-way leaves some
versions out; bump the version and run it again rather than unpublishing.

## One-time setup

1. **Scope.** The packages live under the npm org `acacia-rs` (`@acacia` was taken). Using another scope: set
   `NPM_SCOPE` in `js.yml`; `scripts/set-scope.mjs` renames the packages in CI before building.
2. **First publish.** npm only lets you add a trusted publisher to a package that exists. Publish a `0.0.0`
   placeholder of the main package and each platform package by hand (`npm publish --access public` with a
   granular token, then revoke it).
3. **Trusted publishers.** For every package, on npmjs.com → package → Settings → Trusted publishing: GitHub
   Actions, repository `acacia-rs/acacia`, workflow `js.yml`, environment `npm`. Then set "Require two-factor
   authentication and disallow tokens".
4. **GitHub environment.** Repository Settings → Environments → `npm`: required reviewers, and deployment
   branches limited to `master`.

## Cutting a `next` release

1. Bump `version` in `packages/acacia-bot/package.json` to a prerelease (`0.1.0-next.N`); merge to master.
2. Actions → JS → Run workflow on `master`, tick **publish**. Every job (drift, the three test runners, all
   seven builds) must pass; the publish job also fails if any platform dir has no binary.
3. Approve the `npm` environment deployment.

## Verifying

- `npm view @acacia-rs/bot dist-tags` shows `next` at the new version; `npm view @acacia-rs/bot@next optionalDependencies`
  lists every platform package at that exact version.
- The package page shows the provenance badge linking to the workflow run.
- In an empty directory: `npm i @acacia-rs/bot@next` and run the hello world from `acacia-bot/README.md` on at least
  Linux and one of macOS/Windows.
