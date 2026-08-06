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
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-4 p-6 font-mono text-sm">
      <h1 className="text-xl font-bold">Équipe d&apos;administration</h1>
      <a href="/admin" className="text-xs underline">
        ← Admin
      </a>

      <section className="rounded border p-3">
        <h2 className="mb-2 font-bold">Accès en cours ({equipe.length})</h2>
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
                  <form action={changerRole} className="flex items-center gap-1">
                    <input type="hidden" name="pseudo" value={membre.pseudo} />
                    <select name="role" defaultValue={membre.role} className="border px-1 py-0.5">
                      <option value="utilisateur">retirer l&apos;accès</option>
                      <option value="admin">administrateur</option>
                      <option value="super_admin">super-administrateur</option>
                    </select>
                    <button type="submit" className="border px-2 py-0.5 hover:bg-black/5">
                      appliquer
                    </button>
                  </form>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>

      <form action={changerRole} className="flex flex-wrap items-center gap-2 rounded border p-3">
        <span className="font-bold">Donner un accès :</span>
        <input name="pseudo" placeholder="pseudo" className="border px-2 py-1" required />
        <select name="role" defaultValue="admin" className="border px-2 py-1">
          <option value="admin">administrateur</option>
          <option value="super_admin">super-administrateur</option>
        </select>
        <button type="submit" className="border bg-black px-3 py-1 text-white">
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
