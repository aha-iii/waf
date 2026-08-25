# Releasing Nerve WAF

## Version bump

1. Update `VERSION` and workspace `Cargo.toml` version (if applicable).
2. Update `deploy/helm/nerve-waf/Chart.yaml` (`version` + `appVersion`).
3. Update `deploy/.env.example` `TAG=`.
4. Add entry to `CHANGELOG.md`.

## Tag & publish

```bash
git tag nerve-waf-v1.0.0
git push origin nerve-waf-v1.0.0
```

GitHub Actions workflow `nerve-waf-release.yml` will:

- Build standalone Docker image (`Dockerfile.standalone`, no Nerve-ui)
- Push to `ghcr.io/<owner>/nerve-waf:<version>` and `:latest`
- Create a GitHub Release with changelog

## Helm

```bash
helm package deploy/helm/nerve-waf -d dist/
# attach dist/nerve-waf-1.0.0.tgz to GitHub Release (optional)
```

## Post-release

Update README badge / image references if `YOUR-ORG` placeholders were replaced.
