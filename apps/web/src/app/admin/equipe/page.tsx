import { revalidatePath } from "next/cache";

import { adminFetch, adminPost } from "../adminFetch";

export const dynamic = "force-dynamic";

// Gestion des accès au panneau (super-admin) : promouvoir, rétrograder,
// retirer l'accès. Le compte maître est affiché mais verrouillé.

type Membre = { id: string; pseudo: string; role: string; is_master: boolean };

const LIBELLE: Record<string, string> = {
  utilisateur: "Utilisateur (aucun accès)",
  admin: "Administrateur",
  super_admin: "Super-administrateur",
};

async function reinitialiser2fa(formData: FormData) {
  "use server";
  const pseudo = String(formData.get("pseudo") ?? "").trim();
  if (pseudo) {
    await adminPost(`/admin/users/${encodeURIComponent(pseudo)}/reset-2fa`);
  }
  revalidatePath("/admin/equipe");
}

async function changerRole(formData: FormData) {
  "use server";
  const pseudo = String(formData.get("pseudo") ?? "").trim();
  const role = String(formData.get("role") ?? "");
  if (pseudo && role) {
    await adminPost(`/admin/users/${encodeURIComponent(pseudo)}/role`, { role });
  }
  revalidatePath("/admin/equipe");
}

export default async function AdminEquipePage() {
  const equipe = (await adminFetch<Membre[]>("/admin/staff")) ?? [];

  return (
    <main className="flex flex-col gap-4 text-sm">
      <h1 className="font-display text-2xl">Équipe d&apos;administration</h1>

      <section className="flex flex-col gap-2 rounded-[28px] bg-sable p-5 shadow-sm">
        <h2 className="font-display text-lg">Accès en cours ({equipe.length})</h2>
        {equipe.length === 0 ? (
          <p>Personne d&apos;autre que la clé de service.</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {equipe.map((membre) => (
              <li key={membre.id} className="flex flex-wrap items-center gap-2">
                <span className="font-bold">{membre.pseudo}</span>
                <span>· {LIBELLE[membre.role] ?? membre.role}</span>
                {membre.is_master ? (
                  <span title="Compte maître : ni rétrogradable, ni sanctionnable">
                    · 🔒 compte maître
                  </span>
                ) : (
                  <span className="flex flex-wrap items-center gap-1.5">
                    <form action={changerRole} className="flex items-center gap-1.5">
                      <input type="hidden" name="pseudo" value={membre.pseudo} />
                      <select
                        name="role"
                        defaultValue={membre.role}
                        className="rounded-full border border-neutre-300 bg-creme px-2.5 py-1 text-xs outline-none"
                      >
                        <option value="utilisateur">retirer l&apos;accès</option>
                        <option value="admin">administrateur</option>
                        <option value="super_admin">super-administrateur</option>
                      </select>
                      <button
                        type="submit"
                        className="cursor-pointer rounded-full bg-[#c67139] px-3 py-1 font-display text-xs text-creme hover:bg-terracotta-600"
                      >
                        Appliquer
                      </button>
                    </form>
                    <form action={reinitialiser2fa}>
                      <input type="hidden" name="pseudo" value={membre.pseudo} />
                      <button
                        type="submit"
                        title="Récupération : réinitialise la double authentification (compte maître uniquement)"
                        className="cursor-pointer rounded-full border border-neutre-300 px-3 py-1 text-xs text-encre hover:bg-encre/7"
                      >
                        Réinitialiser la 2FA
                      </button>
                    </form>
                  </span>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      <form action={changerRole} className="flex flex-wrap items-center gap-2 rounded-[28px] bg-sable p-5 shadow-sm">
        <span className="font-display text-base">Donner un accès :</span>
        <input name="pseudo" placeholder="pseudo" className="rounded-full border border-neutre-300 bg-creme px-3.5 py-2 text-sm outline-none focus:border-terracotta-500" required />
        <select name="role" defaultValue="admin" className="rounded-full border border-neutre-300 bg-creme px-3 py-2 text-sm outline-none">
          <option value="admin">administrateur</option>
          <option value="super_admin">super-administrateur</option>
        </select>
        <button type="submit" className="cursor-pointer rounded-full bg-[#c67139] px-5 py-2 font-display text-sm text-creme hover:bg-terracotta-600">
          Promouvoir
        </button>
      </form>

      <p className="text-xs">
        Règles : seul un super-administrateur gère les rôles ; le compte maître est intouchable ;
        personne ne modifie son propre rôle. Chaque changement est inscrit au{" "}
        <a href="/admin/audit" className="underline">
          journal d&apos;audit
        </a>{" "}
        (auteur, cible, ancien rôle, nouveau rôle, horodatage).
      </p>
    </main>
  );
}
