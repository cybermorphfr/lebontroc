import Link from "next/link";
import { revalidatePath } from "next/cache";

import { adminFetch, adminPost } from "../adminFetch";
import { Carte, Pastille } from "../ui";

export const dynamic = "force-dynamic";

// File des signalements (F6.1). Chaque ligne dit qui est visé et sur
// quoi — et mène au dossier du membre : trancher sans contexte, c'est
// trancher au hasard.

type Report = {
  id: string;
  reporter_pseudo: string;
  target_type: string;
  target_id: string;
  target_label: string | null;
  target_pseudo: string | null;
  reason: string;
  comment: string | null;
  status: string;
  outcome: string | null;
  created_at: string;
};

const MOTIFS: Record<string, string> = {
  contrefacon: "Contrefaçon",
  interdit_vente: "Objet interdit",
  annonce_trompeuse: "Annonce trompeuse",
  contenu_inapproprie: "Contenu inapproprié",
  spam_doublon: "Spam ou doublon",
  arnaque_suspectee: "Arnaque suspectée",
  comportement_inapproprie: "Comportement inapproprié",
  contournement_plateforme: "Incite à sortir de la plateforme",
  usurpation_faux_profil: "Usurpation ou faux profil",
  harcelement_insultes: "Harcèlement ou insultes",
  tentative_arnaque: "Tentative d'arnaque",
  contournement_masquage: "Contourne le masquage des coordonnées",
  autre: "Autre",
};

async function closeReport(formData: FormData) {
  "use server";
  await adminPost(`/admin/reports/${formData.get("id")}/close`, {
    outcome: formData.get("outcome"),
  });
  revalidatePath("/admin/signalements");
}

export default async function AdminSignalementsPage() {
  const reports: Report[] = (await adminFetch<Report[]>("/admin/reports")) ?? [];
  const nouveaux = reports.filter((r) => r.status === "nouveau").length;

  return (
    <div className="flex flex-col gap-4">
      <h1 className="font-display text-2xl">
        Signalements ({nouveaux} nouveau{nouveaux > 1 ? "x" : ""})
      </h1>

      {reports.length === 0 ? (
        <Carte>
          <p className="text-sm text-neutre-700">Aucun signalement. Tout va bien.</p>
        </Carte>
      ) : null}

      {reports.map((r) => (
        <Carte key={r.id}>
          <div className="flex flex-wrap items-center gap-2 text-sm">
            <Pastille ton={r.status === "nouveau" ? "attente" : r.outcome === "fonde" ? "alerte" : "neutre"}>
              {r.status === "nouveau" ? "à traiter" : (r.outcome ?? r.status)}
            </Pastille>
            <span className="font-semibold">{MOTIFS[r.reason] ?? r.reason}</span>
            <span className="text-neutre-700">
              sur {r.target_type === "objet" ? "une annonce" : r.target_type === "message" ? "un message" : "un profil"}
            </span>
            <span className="ml-auto text-xs text-neutre-700">
              signalé par{" "}
              <Link
                href={`/admin/membre/${encodeURIComponent(r.reporter_pseudo)}`}
                className="underline"
              >
                {r.reporter_pseudo}
              </Link>{" "}
              le {new Date(r.created_at).toLocaleDateString("fr-FR")}
            </span>
          </div>

          {/* La cible, en clair et cliquable — plus d'UUID à copier-coller. */}
          <div className="flex flex-wrap items-center gap-2 rounded-2xl bg-creme px-3 py-2 text-sm">
            {r.target_type === "objet" ? (
              <>
                <span className="text-neutre-700">Annonce :</span>
                <Link href={`/objet/${r.target_id}`} className="font-semibold hover:underline">
                  {r.target_label ?? "annonce supprimée"}
                </Link>
              </>
            ) : r.target_type === "message" ? (
              <>
                <span className="text-neutre-700">Message :</span>
                <span className="italic">« {r.target_label ?? "message supprimé"} »</span>
              </>
            ) : (
              <span className="text-neutre-700">Profil signalé :</span>
            )}
            {r.target_pseudo ? (
              <>
                <Link
                  href={`/admin/membre/${encodeURIComponent(r.target_pseudo)}`}
                  className="font-semibold text-terracotta-700 hover:underline"
                >
                  {r.target_pseudo}
                </Link>
                <Link
                  href={`/admin/membre/${encodeURIComponent(r.target_pseudo)}`}
                  className="ml-auto text-xs underline"
                >
                  ouvrir son dossier complet →
                </Link>
              </>
            ) : (
              <span className="text-xs text-neutre-700">membre introuvable ({r.target_id})</span>
            )}
          </div>

          {r.comment ? <p className="text-sm text-neutre-700">« {r.comment} »</p> : null}

          {r.status === "nouveau" ? (
            <div className="flex flex-wrap gap-2">
              <form action={closeReport}>
                <input type="hidden" name="id" value={r.id} />
                <input type="hidden" name="outcome" value="fonde" />
                <button
                  type="submit"
                  className="cursor-pointer rounded-full bg-[#c67139] px-4 py-1.5 font-display text-xs text-creme hover:bg-terracotta-600"
                >
                  Fondé (+2 au score)
                </button>
              </form>
              <form action={closeReport}>
                <input type="hidden" name="id" value={r.id} />
                <input type="hidden" name="outcome" value="rejete" />
                <button
                  type="submit"
                  className="cursor-pointer rounded-full border border-neutre-300 px-4 py-1.5 text-xs hover:bg-encre/7"
                >
                  Rejeter
                </button>
              </form>
              {r.target_type === "objet" ? (
                <Link
                  href={`/admin/annonces?q=${encodeURIComponent(r.target_label ?? "")}`}
                  className="flex items-center rounded-full border border-neutre-300 px-4 py-1.5 text-xs hover:bg-encre/7"
                >
                  Modérer cette annonce
                </Link>
              ) : null}
            </div>
          ) : null}
        </Carte>
      ))}
    </div>
  );
}
