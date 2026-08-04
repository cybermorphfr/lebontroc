// Réservé aux Server Components — s'appuie sur next/headers.
import { cookies } from "next/headers";
import {
  createApiClient,
  type SessionResponse,
  type UserResponse,
} from "@lebontroc/api-client";

function apiBase(): string {
  return process.env.API_INTERNAL_URL ?? "http://localhost:8080";
}

/** Client API côté serveur, cookies de la requête transmis. */
async function serverClient() {
  const jar = await cookies();
  const cookieHeader = jar
    .getAll()
    .map((c) => `${c.name}=${c.value}`)
    .join("; ");
  return createApiClient(apiBase(), { cookie: cookieHeader });
}

/** Utilisateur connecté, ou `null` (jamais d'exception). */
export async function getCurrentUser(): Promise<UserResponse | null> {
  try {
    const client = await serverClient();
    const { data } = await client.GET("/me", { cache: "no-store" });
    return data ?? null;
  } catch {
    return null;
  }
}

export async function getSessions(): Promise<SessionResponse[]> {
  try {
    const client = await serverClient();
    const { data } = await client.GET("/auth/sessions", { cache: "no-store" });
    return data ?? [];
  } catch {
    return [];
  }
}
