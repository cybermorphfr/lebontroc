/**
 * Fetch côté navigateur vers `/api`, avec un rejeu unique après refresh de
 * session si l'access token a expiré (cookie 15 min).
 */
export async function apiFetch(path: string, init?: RequestInit): Promise<Response> {
  const response = await fetch(`/api${path}`, init);
  if (response.status !== 401) return response;

  const refreshed = await fetch("/api/auth/refresh", { method: "POST" });
  if (!refreshed.ok) return response;
  return fetch(`/api${path}`, init);
}

/** Extrait le code et le message d'une réponse d'erreur API. */
export async function apiError(response: Response): Promise<{ code: string; message: string }> {
  try {
    const body = (await response.json()) as { error?: { code?: string; message?: string } };
    return {
      code: body.error?.code ?? "erreur_inconnue",
      message: body.error?.message ?? "Un souci de notre côté. Réessaie dans un instant.",
    };
  } catch {
    return { code: "erreur_inconnue", message: "Un souci de notre côté. Réessaie dans un instant." };
  }
}
