import { cookies } from "next/headers";

/**
 * Appel d'un endpoint d'administration au nom de l'utilisateur connecté :
 * ce sont ses cookies — donc son rôle — qui décident, et le journal
 * d'audit retient son nom. La clé de service reste réservée aux scripts
 * d'exploitation.
 */
export async function adminFetch<T>(path: string): Promise<T | null> {
  const jar = await cookies();
  const response = await fetch(
    `${process.env.API_INTERNAL_URL ?? "http://localhost:8080"}${path}`,
    {
      headers: {
        cookie: jar
          .getAll()
          .map((c) => `${c.name}=${c.value}`)
          .join("; "),
      },
      cache: "no-store",
    },
  );
  return response.ok ? ((await response.json()) as T) : null;
}

/** Action d'administration (POST) au nom de l'utilisateur connecté. */
export async function adminPost(path: string, body?: unknown): Promise<boolean> {
  const jar = await cookies();
  const response = await fetch(
    `${process.env.API_INTERNAL_URL ?? "http://localhost:8080"}${path}`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        cookie: jar
          .getAll()
          .map((c) => `${c.name}=${c.value}`)
          .join("; "),
      },
      ...(body !== undefined ? { body: JSON.stringify(body) } : {}),
    },
  );
  return response.ok;
}

/**
 * Variante qui rend aussi le code HTTP : une page ne peut pas répondre
 * « ce membre n'existe pas » quand le vrai problème est une session
 * expirée ou un rôle insuffisant.
 */
export async function adminFetchStatus<T>(
  path: string,
): Promise<{ data: T | null; status: number }> {
  const jar = await cookies();
  const response = await fetch(
    `${process.env.API_INTERNAL_URL ?? "http://localhost:8080"}${path}`,
    {
      headers: {
        cookie: jar
          .getAll()
          .map((c) => `${c.name}=${c.value}`)
          .join("; "),
      },
      cache: "no-store",
    },
  );
  return {
    data: response.ok ? ((await response.json()) as T) : null,
    status: response.status,
  };
}
