# Lebontroc

Plateforme de troc d'objets entre particuliers — « ça contre ça », soulte
séquestrée optionnelle, remise en main propre ou envoi croisé.

- **Backlog produit** : `/srv/docker/sites/lebontroc/specs/BACKLOG.md` (source de vérité produit)
- **Design system** : `/srv/docker/sites/lebontroc/specs/` (base Organic, tokens, composants)
- **Prod** : https://lebontroc.brianplus.com

## Structure du monorepo

```
apps/api/              API Rust — Axum, SQLx, Tokio
  crates/api/          Routes/handlers (minces) + contrat OpenAPI (utoipa)
  crates/domain/       Logique métier pure, sans IO — les règles de troc vivent ici
  crates/infra/        PostgreSQL (SQLx), plus tard S3, Mangopay, logistique
  migrations/          Migrations SQLx versionnées (réversibles)
apps/web/              Front Next.js (App Router, TS strict, Tailwind)
packages/api-client/   openapi.json (contrat committé) + client TS généré
e2e/                   Tests Playwright (les Gherkin du backlog)
```

**Le contrat OpenAPI est la source de vérité entre Rust et TS** :
`cargo run --bin dump-openapi` → `packages/api-client/openapi.json` →
`npm run generate:client` → types TS. La CI échoue si le contrat committé
diverge du code Rust.

## Développement

Les dépendances tournent en Docker, l'API et le front en natif :

```bash
docker compose -f docker-compose.dev.yml up -d   # postgres :5433, minio :9002/:9003, mailpit :8025
cp .env.example .env
(cd apps/api && cargo run --bin lebontroc-api)   # nécessite Rust local
npm install && npm run generate:client
npm run dev --workspace=web
```

⚠️ **Jamais de compilation Rust sur le VPS de prod** (disque et RAM limités).
La CI est l'unique gate Rust : travailler en branche, pousser, lire la CI.
Ce qui peut être validé sans cargo : `npm run lint`, `npm run typecheck`,
`npm run generate:client`.

## CI / Déploiement

`push` sur `main` au vert → images poussées sur GHCR :
`ghcr.io/cybermorphfr/lebontroc-api` et `-web`, tags `latest` + `sha-<court>`.

Sur le VPS (`/srv/docker/sites/lebontroc/`) :

```bash
docker compose pull && docker compose up -d      # déployer la dernière version
TAG=sha-abc1234 docker compose up -d             # rollback en 30 s
```

## Conventions (backlog §0.3)

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`,
  `tsc --noEmit`, `eslint` : tout au vert avant merge.
- Tests unitaires dans `domain` (sans DB), intégration par endpoint
  (`sqlx::test`), Gherkin du backlog → Playwright.
- Migrations réversibles (`.up.sql` / `.down.sql`). Aucun secret en dur.
- Télémétrie : événements `snake_case` émis côté API (table `analytics_events`).
- `.sqlx/` (SQLX_OFFLINE) : à introduire avec la première macro `query!`
  (workflow CI `sqlx-prepare` — voir F0.2).
