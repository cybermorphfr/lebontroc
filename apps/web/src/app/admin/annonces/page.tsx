import Link from "next/link";
import { revalidatePath } from "next/cache";

import { euros } from "@/lib/format";

import { adminFetch, adminPost } from "../adminFetch";
import { Carte, Pastille, champ, selecteur } from "../ui";

export const dynamic = "force-dynamic";

// Modération du catalogue. L'entrée normale est l'arborescence : on part
// du membre qui publie, pas d'une liste plate de milliers d'objets.

type Annonce = {
  id: string;
  title: string;
  status: string;
  value_cents: number;
  category: string;
  condition: string;
  owner_pseudo: string;
  owner_banned: boolean;
  photo_url: string | null;
  signalements: number;
  signalements_ouverts: number;
  created_at: string;
};

type Branche = {
  pseudo: string;
  role: string;
  banned: boolean;
  total: number;
  disponibles: number;
  masquees: number;
  signalees: number;
  derniere_publication: string | null;
};

const STATUTS: Record<string, string> = {
  disponible: "En ligne",
  reserve: "Réservée",
  troque: "Troquée",
  masque: "Masquée",
  supprime: "Supprimée",
};

async function moderer(formData: FormData) {
  "use server";
  const id = String(formData.get("id") ?? "");
  const masquer = formData.get("masquer") === "1";
  const motif = String(formData.get("motif") ?? "").trim();
  if (id) {
    await adminPost(`/admin/items/${id}/moderer`, { masquer, motif: motif || null });
  }
  revalidatePath("/admin/annonces");
}

export default async function AdminAnnoncesPage({
  searchParams,
}: {
  searchParams: Promise<{ q?: string; status?: string; owner?: string; signalees?: string }>;
}) {
  const { q, status, owner, signalees } = await searchParams;
  const filtre = new URLSearchParams();
  if (q) filtre.set("q", q);
  if (status) filtre.set("status", status);
  if (owner) filtre.set("owner", owner);
  if (signalees) filtre.set("signalees", "true");
  const cible = filtre.toString();

  const [annonces, arborescence] = await Promise.all([
    adminFetch<Annonce[]>(`/admin/items${cible ? `?${cible}` : ""}`),
    adminFetch<Branche[]>("/admin/items/arborescence"),
  ]);

  if (!annonces || !arborescence) {
    return (
      <Carte>
        <p className="text-sm text-neutre-700">
          La modération des annonces est réservée aux super-administrateurs — ou ta session doit
          revérifier sa double authentification (reconnecte-toi).
        </p>
      </Carte>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">Annonces</h1>

      <Carte titre="Filtrer">
        <form action="/admin/annonces" method="get" className="flex flex-wrap items-center gap-2">
          <input
            name="q"
            defaultValue={q ?? ""}
            placeholder="Titre de l'annonce…"
            className={`min-w-52 flex-1 ${champ}`}
          />
          <input
            name="owner"
            defaultValue={owner ?? ""}
            placeholder="Pseudo du membre"
            className={champ}
          />
          <select name="status" defaultValue={status ?? ""} className={selecteur}>
            <option value="">Tous les statuts</option>
            {Object.entries(STATUTS).map(([valeur, label]) => (
              <option key={valeur} value={valeur}>
                {label}
              </option>
            ))}
          </select>
          <label className="flex items-center gap-1.5 text-sm">
            <input type="checkbox" name="signalees" value="1" defaultChecked={Boolean(signalees)} />
            Signalées seulement
          </label>
          <button
            type="submit"
            className="flex min-h-10 cursor-pointer items-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme hover:bg-terracotta-600"
          >
            Filtrer
          </button>
          {cible ? (
            <Link href="/admin/annonces" className="text-sm underline">
              tout effacer
            </Link>
          ) : null}
        </form>
      </Carte>

      {/* L'arborescence : chaque membre, ses annonces, sa charge de signalements. */}
      <Carte titre={`Par membre (${arborescence.length})`}>
        <ul className="flex flex-col gap-1.5">
          {arborescence.map((b) => (
            <li
              key={b.pseudo}
              className={`flex flex-wrap items-center gap-2 rounded-2xl px-3 py-2 text-sm ${
                owner === b.pseudo ? "bg-terracotta-100" : "bg-creme"
              }`}
            >
              <Link
                href={`/admin/annonces?owner=${encodeURIComponent(b.pseudo)}`}
                className="font-semibold hover:underline"
              >
                {b.pseudo}
              </Link>
              <span className="text-neutre-700">
                {b.total} annonce{b.total > 1 ? "s" : ""} · {b.disponibles} en ligne
                {b.masquees > 0 ? ` · ${b.masquees} masquée${b.masquees > 1 ? "s" : ""}` : ""}
              </span>
              {b.signalees > 0 ? (
                <Pastille ton="alerte">
                  {b.signalees} signalement{b.signalees > 1 ? "s" : ""}
                </Pastille>
              ) : null}
              {b.banned ? <Pastille ton="alerte">banni</Pastille> : null}
              {b.role !== "utilisateur" ? <Pastille ton="ok">{b.role}</Pastille> : null}
              <Link
                href={`/admin/membre/${encodeURIComponent(b.pseudo)}`}
                className="ml-auto text-xs text-terracotta-700 underline"
              >
                son dossier
              </Link>
            </li>
          ))}
        </ul>
      </Carte>

      <Carte
        titre={
          owner
            ? `Annonces de ${owner} (${annonces.length})`
            : `Annonces (${annonces.length}${annonces.length === 200 ? ", 200 max" : ""})`
        }
      >
        {annonces.length === 0 ? (
          <p className="text-sm text-neutre-700">Aucune annonce ne correspond à ce filtre.</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {annonces.map((a) => (
              <li key={a.id} className="flex flex-wrap items-center gap-3 rounded-2xl bg-creme p-3">
                {a.photo_url ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img
                    src={a.photo_url}
                    alt=""
                    className="h-14 w-14 shrink-0 rounded-2xl object-cover"
                  />
                ) : (
                  <span className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-sable text-xs text-neutre-700">
                    sans photo
                  </span>
                )}
                <div className="flex min-w-52 flex-1 flex-col gap-0.5">
                  <Link href={`/objet/${a.id}`} className="font-semibold hover:underline">
                    {a.title}
                  </Link>
                  <span className="text-xs text-neutre-700">
                    {a.category} · {euros(a.value_cents)} · par{" "}
                    <Link
                      href={`/admin/membre/${encodeURIComponent(a.owner_pseudo)}`}
                      className="underline"
                    >
                      {a.owner_pseudo}
                    </Link>
                  </span>
                </div>
                <Pastille ton={a.status === "masque" ? "attente" : "neutre"}>
                  {STATUTS[a.status] ?? a.status}
                </Pastille>
                {a.signalements_ouverts > 0 ? (
                  <Pastille ton="alerte">{a.signalements_ouverts} à traiter</Pastille>
                ) : a.signalements > 0 ? (
                  <Pastille ton="neutre">{a.signalements} traité(s)</Pastille>
                ) : null}
                {a.status === "disponible" || a.status === "masque" ? (
                  <form action={moderer} className="flex items-center gap-1.5">
                    <input type="hidden" name="id" value={a.id} />
                    <input type="hidden" name="masquer" value={a.status === "masque" ? "0" : "1"} />
                    {a.status === "masque" ? null : (
                      <input
                        name="motif"
                        placeholder="motif"
                        aria-label={`Motif du retrait de ${a.title}`}
                        className="w-28 rounded-full border border-neutre-300 bg-creme px-3 py-1 text-xs outline-none"
                      />
                    )}
                    <button
                      type="submit"
                      className="cursor-pointer rounded-full border border-neutre-300 px-3 py-1 text-xs hover:bg-encre/7"
                    >
                      {a.status === "masque" ? "Remettre en ligne" : "Retirer"}
                    </button>
                  </form>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </Carte>

      <p className="text-xs text-neutre-700">
        Retirer une annonce la masque pour tout le monde et prévient son propriétaire avec le
        motif. L&apos;action est inscrite au{" "}
        <Link href="/admin/audit" className="underline">
          journal d&apos;audit
        </Link>
        . Une annonce déjà troquée n&apos;est plus modifiable.
      </p>
    </div>
  );
}
