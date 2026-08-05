# Lebontroc — panorama des features (référence technique)

*Backlog MVP livré à 100 % le 5 août 2026 — 17 features, 7 lots.
Stack : Rust (Axum + SQLx/PostgreSQL) · Next.js 15 · Traefik · MinIO ·
Mailpit · CI GitHub Actions → GHCR → VPS. Architecture en 3 crates :
`domain` (règles pures testées), `infra` (SQL/S3/SMTP/PSP), `api` (handlers).
Les services externes (paiement, transport) sont derrière des **traits**
(`PaymentProvider`, `ShippingProvider`) : simulateurs en bêta, bascule
Mangopay/Boxtal sans toucher aux handlers.*

## Lot 0-1 — Socle et catalogue

- **F0.1 Squelette** : monorepo, healthcheck versionné, CI complète
  (fmt/clippy/tests + lint/tsc/build + E2E Playwright sur stack Docker +
  publication GHCR), déploiement compose + Traefik (`/api` strippé).
- **F0.2 Comptes** : signup/login (Argon2, JWT httpOnly + refresh rotatif),
  vérification e-mail, anti-bruteforce (verrouillage 15 min), gestion des
  sessions, télémétrie first-party pseudonymisée (`analytics_events`, sel).
- **F1.1 Publication** : 1-8 photos (upload direct S3 par URL présignées,
  types/taille contrôlés, purge des orphelines), catégories 2 niveaux,
  valeur indicative 1-2000 €, préférence de remise, soulte acceptée o/n.
- **F1.2 Dressing** : statuts disponible/réservé/troqué/masqué, édition,
  soft-delete, « ce que je cherche » (wishlist 3 lignes).

## Lot 2 — Découverte

- **F2.1 Fil** : tri proximité (haversine sur communes) + fraîcheur, scroll
  infini, cartes photo/titre/distance/fraîcheur/état/valeur.
- **F2.2 Recherche** : plein texte français tolérant aux fautes (tsvector +
  trigrammes), filtres (catégorie récursive, état, distance, mode, soulte),
  3 tris, historique local.
- **F2.3 Favoris + profils publics** : cœur overlay, page favoris, profil
  public (commune, ancienneté, dressing, réputation).
- **Home marketplace** (hors backlog, ajoutée au Lot 6) : search-first
  (Leboncoin) + supply-first (Vinted) — barre de recherche proéminente,
  chips catégories, hero « publie ton premier objet », rails conditionnels
  « Dans tes recherches » (wishlist) et « Tes favoris toujours dispo »
  (seuil 4 objets), carte in-feed « Publier » si dressing vide.

## Lot 3 — Négociation

- **F3.1 Propositions** : multi-objets contre multi-objets, soulte plafonnée
  à 30 % du panier le moins cher, expiration J+7 avec e-mail, refus,
  invalidation en cascade (objet réservé ailleurs → caduque).
- **F3.2 Messagerie** : WebSocket temps réel (broker mémoire par user),
  photos dans les messages, accusés de lecture, rappel e-mail 24 h,
  **masquage automatique des coordonnées** avant acceptation.
- **F3.3 Acceptation atomique** : transaction unique — troc créé, objets
  réservés, propositions concurrentes caduquées (notifiées), codes générés.

## Lot 4 — Transaction

- **F4.1 Main propre** : codes croisés à 6 chiffres, double confirmation,
  rappel J+7, auto-annulation J+14 (main propre uniquement).
- **F4.2 Soulte séquestrée** : préautorisation à l'acceptation (30 min si le
  payeur accepte, 24 h sinon), séquestre, capture à la bonne fin — depuis
  F5.2 : **48 h après la remise** (fenêtre de contestation). Cartes de test,
  échecs simulés, commission paramétrable (0 en bêta), maintenance de
  rattrapage. Multi-payeurs (UNIQUE trade+payer) depuis F4.3.
- **F4.3 Envoi croisé** : 2 colis par troc, formats forfaitaires S/M/L
  (4,50/6,90/9,90 € + 2 € service), une préauth par partie (transport +
  service + soulte), étiquettes quand les deux ont payé, machine à états
  préparation→étiquette→déposé→arrivé→retiré→confirmé, fenêtre 72 h après
  retrait, rappels J+2/J+4, échec J+5 (annulation totale ou gel partiel),
  filet J+21. Simulateur relais/étiquettes déterministe (cible : Boxtal).

## Lot 5 — Confiance

- **F5.1 Évaluations** : 1-5 ⭐ + commentaire 500 c., **publication simultanée
  anti-représailles** (embargo jusqu'à la note de l'autre ou J+14), réponse
  publique unique, réputation sur profil (moyenne, volume, délai d'expédition
  moyen calculé sur les dépôts réels).
- **F5.2 Litiges & modération** : dossier unique par troc (motifs typés,
  5 photos en **bucket S3 privé** accès présigné, contradictoire 72 h,
  examen), fenêtres envoi/no-show J+3/post-remise 48 h, résolution admin
  par token (capture/libération/rejet via traits PSP), **score de fiabilité**
  (somme pondérée de `dispute_events`) avec **sanctions automatiques**
  (5/10/15 → avertissement/restriction 30 j/bannissement + levée admin),
  blocage bidirectionnel discret, signalements typés (LCEN/DSA).
- **F5.3 Notifications** : taxonomie fermée 10 types, hub `notify()` (in-app
  systématique + badge WebSocket temps réel), préférences e-mail par type
  (5 désactivables, l'argent/colis/litiges verrouillés), 6 nouveaux e-mails
  (proposition reçue/acceptée/refusée, évaluations, favori réservé/de
  retour), purge 90 j, Reply-To réel. SMTP paramétrable par env
  (Mailpit en bêta → Scaleway TEM, voir `SMTP-SCALEWAY.md`).

## Lot 6 — Ops et conformité

- **F6.1 Back-office** : hub `/admin` (recherche transverse + scores, KPI),
  file signalements (fondé = +2 score), **journal d'audit immuable**,
  télémétrie `admin_action`. Auth : `X-Admin-Token` (comparaison temps
  constant) + basic auth Traefik.
- **F6.2 Télémétrie produit** : export batch PostHog Cloud EU (10 min,
  curseur `exported_at`, inactif sans clé), récap KPI hebdo e-mail (lundi,
  idempotent), `GET /admin/kpis`.
- **F6.3 RGPD** : export JSON complet (`GET /me/export`), suppression de
  compte anonymisante (mot de passe requis, bloquée si trocs actifs, trocs
  finalisés conservés anonymisés), bannière cookies informative (aucun
  traceur tiers), pages CGU/confidentialité/mentions légales, préparation
  DAC7 dans les CGU.

## Chiffres

- ~110 tests Rust (64 intégration sqlx sur les Gherkin du backlog, 46
  unitaires domaine) + 21 scénarios E2E Playwright multi-navigateurs.
- 6 migrations SQL réversibles (up/down), 60+ endpoints OpenAPI générés
  (source de vérité du client TypeScript).
- ~25 templates e-mail transactionnels en français.

## Ce qui attend après le MVP

Bascule SMTP réelle (action fondateur — verrou bêta ouverte) · compte
PostHog · PSP réel Mangopay (webhooks = poke+refetch, Idempotency-Key,
reauthorize pour cycles > 30 j) · transporteur Boxtal · web push PWA ·
digest quotidien · partage de soulte · retours organisés · app admin
séparée avec rôles · Meilisearch si la recherche PG sature.
