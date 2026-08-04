/**
 * Client API Lebontroc — typé par le contrat OpenAPI (source de vérité).
 *
 * `openapi.json` est régénéré depuis le code Rust (`cargo run --bin
 * dump-openapi`) ; `src/schema.d.ts` est régénéré depuis `openapi.json`
 * (`npm run generate`). La CI garantit la synchronisation des trois.
 */
import createClient from "openapi-fetch";

import type { components, paths } from "./schema";

export type HealthResponse = components["schemas"]["HealthResponse"];
export type UserResponse = components["schemas"]["UserResponse"];
export type SessionResponse = components["schemas"]["SessionResponse"];
export type ErrorResponse = components["schemas"]["ErrorResponse"];
export type CategoryNode = components["schemas"]["CategoryNode"];
export type ItemResponse = components["schemas"]["ItemResponse"];
export type ItemPhotoResponse = components["schemas"]["ItemPhotoResponse"];
export type FeedResponse = components["schemas"]["FeedResponse"];
export type FeedCard = components["schemas"]["FeedCard"];
export type ItemDetailResponse = components["schemas"]["ItemDetailResponse"];
export type SearchResponse = components["schemas"]["SearchResponse"];
export type WishlistEntry = components["schemas"]["WishlistEntryDto"];

/**
 * Crée un client typé.
 *
 * @param baseUrl — côté navigateur : `/api` (routé par Traefik) ;
 *                  côté serveur (SSR) : `http://api:8080` (réseau interne).
 */
export function createApiClient(baseUrl: string, headers?: Record<string, string>) {
  return createClient<paths>({ baseUrl, headers });
}
