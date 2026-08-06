"use server";

import { adminFetch } from "../adminFetch";

export type Suggestion = {
  pseudo: string;
  role: string;
  is_master: boolean;
  annonces: number;
};

/** Autocomplétion des pseudos — passe par le serveur, donc par les
 * cookies de l'admin : le navigateur n'a jamais à parler à l'API. */
export async function chercherPseudos(q: string): Promise<Suggestion[]> {
  if (q.trim().length < 2) return [];
  return (await adminFetch<Suggestion[]>(`/admin/users/suggest?q=${encodeURIComponent(q)}`)) ?? [];
}
