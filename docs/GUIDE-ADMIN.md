# Lebontroc — guide d'administration & d'exploitation

*Pour Brian. Tout ce qui suit vit sur https://lebontroc.brianplus.com/admin
(protégé par la basic auth Traefik — identifiants Mailpit — et par le token
`ADMIN_TOKEN` du `.env`, injecté automatiquement par les pages).*

## 1. Le back-office `/admin`

- **Hub `/admin`** : KPI des 7 derniers jours + recherche transverse (pseudo,
  e-mail, titre d'objet ou UUID de troc) → utilisateurs (avec **score de
  fiabilité** et sanctions en cours), objets, trocs.
- **`/admin/litiges`** : la file des dossiers. Chaque dossier montre les deux
  versions, les pièces photos (liens privés signés), les paiements et les
  scores. Tu tranches : **capture** (le troc va au bout, débits), **libération**
  (tout est annulé, zéro débit), **rejet** (classé, le parcours reprend).
  Le champ « pseudo en tort » alimente le score du fautif.
- **`/admin/signalements`** : clôture en un clic. **Fondé** = +2 au score du
  signalé (sanctions automatiques aux seuils). Rejeté = sans suite.
- **`/admin/audit`** : journal immuable de toutes tes actions (et du récap KPI).
- **`/admin/liens`** : les e-mails capturés par Mailpit (tant que le SMTP réel
  n'est pas branché).

## 2. Score de fiabilité et sanctions automatiques

| Événement | Points |
|---|---|
| Contrefaçon avérée | +15 |
| Litige perdu | +6 |
| Non-dépôt de colis (J+5) | +5 |
| No-show confirmé | +4 |
| Plainte abusive / signalement fondé | +2 |

Seuils **automatiques** : 5 = avertissement e-mail · 10 = restriction 30 j
(plus de nouvelles propositions) · 15 = **bannissement** (sessions révoquées,
connexion refusée). Tu es alerté par e-mail à chaque déclenchement. **Filet** :
`/admin` → chercher l'utilisateur, ou
`POST /api/admin/users/{pseudo}/lift-sanctions` (header `X-Admin-Token`) pour
tout lever.

## 3. Télémétrie (F6.2)

- **KPI hebdo** : e-mail automatique chaque lundi (inscriptions, objets,
  propositions, trocs, soultes, litiges). Visible aussi sur `/admin`.
- **PostHog** (à activer) : crée un compte gratuit sur https://eu.posthog.com,
  copie la clé projet dans `/srv/docker/sites/lebontroc/.env` :
  `POSTHOG_API_KEY=phc_…` puis `docker compose --profile mailpit up -d api`.
  L'export part ensuite tout seul toutes les 10 minutes (événements
  pseudonymisés). Dashboards à construire dans PostHog : funnel activation
  (signup → 1ᵉʳ objet → 1ᵉʳᵉ proposition → 1ᵉʳ troc), conversion
  proposition→troc, part des trocs avec soulte, taux de litige.

## 4. Exploitation du serveur

- **Déployer** : la CI publie sur GHCR à chaque merge sur `main`, puis
  `cd /srv/docker/sites/lebontroc && docker compose --profile mailpit pull && docker compose --profile mailpit up -d`.
- **Rollback** : `TAG=<sha-précédent> docker compose --profile mailpit up -d`.
- **Santé** : https://lebontroc.brianplus.com/api/health (version + db).
- **Logs** : `docker logs lebontroc-api --tail 50` (JSON structuré).
- **Base** : jamais exposée ; accès via
  `docker exec -it lebontroc-db psql -U lebontroc -d lebontroc`.
- **Sauvegardes** : dump quotidien (cron existant) — vérifier de temps en temps
  qu'un `pg_restore` fonctionne.
- **Disque** ⚠️ : installer le cron de nettoyage (incident du 5 août — disque à
  100 %) : `15 4 * * 0 docker builder prune -f --keep-storage 6GB && docker image prune -f`.
- **Variables d'env sensibles** (`.env`) : `ADMIN_TOKEN` (admin API),
  `ADMIN_EMAIL` + `SMTP_REPLY_TO` (ta boîte), `POSTHOG_API_KEY` (à créer),
  SMTP (à basculer sur Scaleway TEM : voir `SMTP-SCALEWAY.md` à côté du
  compose — c'est LE prérequis de la bêta ouverte).

## 5. Tâches de fond (automatiques, rien à faire)

| Cadence | Ce qui tourne |
|---|---|
| 10 min | expiration paiements (30 min/24 h), captures dues (main propre +48 h), auto-confirmation colis (+72 h), export PostHog |
| 1 h | expiration propositions J+7, rappels messages 24 h, rappels RDV J+7, annulation J+14, rappels dépôt J+2/J+4, échecs J+5, filet J+21, publication évaluations J+14, escalade litiges 72 h, purge notifications 90 j, KPI hebdo (lundi) |
| 6 h | purge des photos orphelines |

## 6. Comptes de démo

| Pseudo | E-mail | Mot de passe | CP |
|---|---|---|---|
| `camille_demo` | demo-camille@lebontroc.brianplus.com | `demo-lebontroc-2026` | 44000 |
| `theo_demo` | demo-theo@lebontroc.brianplus.com | `demo-lebontroc-2026` | 44300 |

8 articles chacun (puériculture, high-tech, meubles, vêtements…). Parfaits pour
dérouler un troc complet de bout en bout dans deux navigateurs. Les paiements
sont simulés : n'importe quel numéro de carte plausible passe
(`4970 0000 0000 0000`), la carte se terminant par `0002` simule un refus.
