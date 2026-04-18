# Poddyclip Backend

Phoenix app for the Munchy Cow web product.

It owns:
- guest and signed-in user sessions,
- auth and account settings,
- billing and Polar webhook handling,
- upload orchestration and job persistence,
- the server-rendered shell plus the SolidJS app mounted under `/app`,
- admin/internal HTTP surfaces.

It does not do the heavy audio processing itself. Phoenix submits work to the Rust
`poddyclip-api` service and stores the job state/results around that workflow.

## Local Development

From `backend/`:

- `mix setup` installs deps, sets up the database, and builds assets
- `mix phx.server` starts the Phoenix app on `localhost:4000`
- `mix test` runs the backend test suite

From the repo root, the normal three-terminal workflow is:

- `./dev.sh infra` for Postgres + MinIO
- `./dev.sh api` for the Rust processing API
- `./dev.sh web` for Phoenix

## Runtime Shape

- `/app` serves the main SolidJS processing UI
- `/api` serves the authenticated JSON API used by the SolidJS app
- `/api/internal` receives callbacks from the Rust service
- `/api/webhooks/polar` handles Polar billing webhooks
- `/account`, `/users/settings`, `/feedback`, `/help` are Phoenix-rendered pages/live views
- admin routes are served from the separate admin endpoint when enabled

## Current Product Notes

- Guest users get a browser-trimmed 30-second preview via `/api/preview`; that path is synchronous and does not use S3, Oban, or persisted jobs.
- Signed-in users keep the full upload flow: `/api/presign-upload` → direct upload → `/api/jobs` → Rust async processing.
- Legal pages (`terms`, `privacy`, `legal`) are parked in code but intentionally not exposed in the public router.
- Standalone registration has been removed; the public auth flow is unified magic-link sign-in.
- `Past Munchings` is available to signed-in users only.

## Related Areas

- `lib/poddyclip_backend/` contains business logic, billing, storage, and workers
- `lib/poddyclip_backend_web/` contains controllers, LiveViews, router, endpoint, and plugs
- `assets/js/solid/` contains the client app bundled into Phoenix static assets
