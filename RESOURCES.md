# Cloudflare CI/CD Resources

## Knowledge

- [Cloudflare Workers Git integration](https://developers.cloudflare.com/workers/ci-cd/builds/git-integration/)
  官方總覽。用於確認連接 GitHub/GitLab 後，Cloudflare 何時會自動 build 與 deploy。
- [Cloudflare Workers Builds configuration](https://developers.cloudflare.com/workers/ci-cd/builds/configuration/)
  官方 Build command、Deploy command、root directory、build secrets 與 preview deploy 設定。
- [Cloudflare Workers GitHub integration](https://developers.cloudflare.com/workers/ci-cd/builds/git-integration/github-integration/)
  官方 GitHub App 權限、PR comments 與 check runs 說明。
- [Cloudflare Workers with GitHub Actions](https://developers.cloudflare.com/workers/ci-cd/external-cicd/github-actions/)
  官方外部 CI/CD 路線。用於自行控制測試、核准、migration 與 Wrangler deploy。
- [Wrangler Action](https://github.com/cloudflare/wrangler-action)
  Cloudflare 官方 GitHub Action。用於從 workflow 執行 Wrangler 命令。

## Wisdom (Communities)

- [Cloudflare Developers Discord](https://discord.cloudflare.com/)
  適合詢問 Workers Builds、Wrangler 或 GitHub App 的實際平台行為與近期變更。
- [Cloudflare Community](https://community.cloudflare.com/)
  適合搜尋部署錯誤、帳號權限與 build image 限制的既有案例。
