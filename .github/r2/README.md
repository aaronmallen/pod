# R2 CI cache cleanup

The CI cache lives in the Cloudflare R2 bucket `pod-ci-cache` under two prefixes,
`icons/` and `rust-cache/`. Its size is kept bounded by two independent mechanisms,
because this Cloudflare account rejects Worker Cron Triggers (see
`apps/telemetry/wrangler.toml`), so a Worker cron is never an option here.

## 1. R2 object lifecycle rule (the backstop)

`lifecycle.json` is the S3 lifecycle configuration that expires every `rust-cache/`
object 7 days after it is written and every `icons/` object 30 days after it is
written. This is the hard size backstop: even if the scheduled prune below never
runs, neither prefix can grow without bound.

It is applied out of band (like the bucket creation and the S3 token minting),
because rclone cannot set bucket lifecycle. Apply it with any S3 client pointed at
the R2 endpoint, for example the AWS CLI:

```sh
aws s3api put-bucket-lifecycle-configuration \
  --bucket pod-ci-cache \
  --lifecycle-configuration "file://$(git rev-parse --show-toplevel)/.github/r2/lifecycle.json" \
  --endpoint-url "https://${R2_ACCOUNT_ID}.r2.cloudflarestorage.com"
```

Or set the same rule from the Cloudflare dashboard under
R2 > pod-ci-cache > Settings > Object lifecycle rules: prefix `rust-cache/`,
"Delete uploaded objects", 7 days after upload; and prefix `icons/`, "Delete
uploaded objects", 30 days after upload.

Re-run the command (or re-check the dashboard) whenever `lifecycle.json` changes;
the rule replaces the bucket's lifecycle configuration wholesale.

## 2. Scheduled prune job (the trimmer)

`.github/workflows/prune-r2-cache.yml` runs `scripts/ci/r2-prune` daily. It trims
faster than the lifecycle rules by keeping only the newest `rust-cache/` object per
group and only the newest 2 `icons/<buildNumber>.tar.zst` archives (current build
plus one rollback). Its grouping heuristic is unvalidated (see the warning in the
script). Each delete is logged and verified: a wrong-but-successful delete never
breaks CI, but a delete that leaves its object present fails the job instead of
passing silently.
