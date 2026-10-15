# npm 发布

`hyper-vibekanban` 用 GitHub Actions 的 OIDC Trusted Publishing 发到 npm。不使用 `NPM_TOKEN`，也不再上传 Cloudflare R2。

## 预发布

在 GitHub Actions 打开 **Create GitHub Pre-Release**（`.github/workflows/pre-release.yml`），`version_type` 选 `prerelease`，在默认分支上运行。

workflow 会编译、创建 GitHub pre-release，并由这个文件里的 `publish-npm` job 执行 `npm publish`。版本形如 `0.1.50-main.4`，dist-tag 是 `main`（版本号里的 preid）。**不会**移动 `latest`。

`patch` / `minor` / `major` 会直接失败。以前选它们会打出不带预发布号的版本，由 `pre-release.yml` 标成 `latest`，同时 GitHub release 仍是 pre-release，`publish.yml` 不会跑。

## 提升到 latest

合并包含 `release-stable.yml` 的改动之后，在默认分支上打开 **Cut stable npm release**（`.github/workflows/release-stable.yml`）。`source_tag` 留空，会选最新的 npm 预发布（形如 `v0.1.50-main.4.20261014164856`）。也可以填一个这样的 tag。

这个 workflow **不**执行 `npm publish`。它把已发布预发布包的版本改成稳定号（`0.1.50-main.4` → `0.1.50`），用那个预发布所在的 commit 创建**非** pre-release 的 GitHub release `v0.1.50`，再 `workflow_dispatch` 启动 `.github/workflows/publish.yml`。

`publish.yml` 下载该 release 上的 `.tgz`，用 OIDC 执行 `npm publish`。没有连字符的版本，dist-tag 才是 `latest`。用 `GITHUB_TOKEN` 创建的 release 不会触发 `on: release`，所以必须由 `release-stable.yml` 去启动 `publish.yml`。

包里的 `BINARY_TAG` 仍指向原来的预发布 tag，二进制从那里下载。不要删除那个 pre-release。

成功后，workflow 把默认分支上的 `package.json`、`npx-cli/package.json`、`npx-cli/package-lock.json`、`packages/local-web/package.json` 写成这个稳定版本。下一次预发布才会是 `0.1.51-main.0`，而不是又一个 `0.1.50-main.N`。crate 版本留到下一次预发布里的 `cargo set-version` 对齐。

## 不要重跑

- 不要重跑旧的 release job，包括失败的 `0.1.45`。那次失败是因为相对路径的 `.tgz` 被 `npm publish` 当成 git 仓库。
- 不要为了发 npm 去推 `remote-v*` 或 `relay-v*` tag。那是镜像构建。
- 稳定版 tag 已经存在时，不要再跑 `release-stable.yml`。npm 失败只重试 **Publish to npm**（`publish.yml`）的 workflow_dispatch，填写 `tag_name` 和 `release_id`。对预发布 tag 重试 `publish.yml` 也只会走原来的 channel dist-tag，不会移动 `latest`。
- 稳定版的版本号提交出现在默认分支之前，或者 `publish.yml` 还没成功时，不要再开一轮预发布。
- 这条路径不编译，也不增加 macOS x64。

## 权限

`publish.yml` 的 workflow `permissions` 包含 `id-token: write`。它没有 `environment:`，也不读取 secret。

预发布的 `npm publish` 在 `pre-release.yml` 的 `publish-npm` job，该 job 自己的 `permissions` 包含 `id-token: write`。Trusted Publisher 要分别覆盖这两个 workflow 文件名：`pre-release.yml` 和 `publish.yml`。
