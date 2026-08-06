import Link from "next/link";
import { notFound } from "next/navigation";

import { euros, timeAgo } from "@/lib/format";

import { adminFetch } from "../../adminFetch";
import { Carte, Pastille, Statistique } from "../../ui";

export const dynamic = "force-dynamic";

// Le dossier d'un membre : tout ce que la plateforme sait de lui, en une
// page. C'est ce qu'on ouvre quand un signalement tombe — juger sur une
// ligne de file d'attente, c'est juger à l'aveugle.

type Annonce = {
  id: string;
  title: string;
  status: string;
  value_cents: number;
  category: string;
  photo_url: string | null;
  signalements_ouverts: number;
  created_at: string;
};

type Troc = {
  id: string;
  status: string;
  delivery_mode: string;
  role: string;
  autre_pseudo: string;
  cash_cents: number;
  litige: boolean;
  created_at: string;
};

type Signalement = {
  id: string;
  sens: string;
  target_type: string;
  cible: string | null;
  autre_pseudo: string | null;
  reason: string;
  comment: string | null;
  status: string;
  outcome: string | null;
  created_at: string;
};

type Activite = {
  profil: {
    id: string;
    pseudo: string;
    email: string;
    postal_code: string;
    commune: string | null;
    role: string;
    is_master: boolean;
    email_verified: boolean;
    banned_at: string | null;
    restricted_until: string | null;
    deleted_at: string | null;
    totp_actif: boolean;
    created_at: string;
    derniere_activite: string | null;
  };
  compteurs: {
    annonces: number;
    annonces_masquees: number;
    propositions_envoyees: number;
    propositions_recues: number;
    trocs: number;
    trocs_finalises: number;
    trocs_annules: number;
    messages: number;
    litiges_ouverts_par_lui: number;
    litiges_subis: number;
    signalements_emis: number;
    signalements_recus: number;
    signalements_fondes: number;
    note_moyenne: number | null;
    evaluations: number;
    favoris: number;
    blocages_subis: number;
  };
  annonces: Annonce[];
  trocs: Troc[];
  signalements: Signalement[];
  sanctions: { event_type: string; details: string | null; created_at: string }[];
  score: number;
};

const STATUTS_ANNONCE: Record<string, string> = {
  disponible: "En ligne",
  reserve: "Réservée",
  troque: "Troquée",
  masque: "Masquée",
  supprime: "Supprimée",
};

const date = (iso: string) => new Date(iso).toLocaleDateString("fr-FR");

export default async function AdminMembrePage({
  params,
}: {
  params: Promise<{ pseudo: string }>;
}) {
  const { pseudo } = await params;
  const activite = await adminFetch<Activite>(
    `/admin/users/${encodeURIComponent(pseudo)}/activite`,
  );
  if (!activite) notFound();
  const { profil, compteurs, annonces, trocs, signalements, sanctions, score } = activite;
  const recus = signalements.filter((s) => s.sens === "recu");
  const emis = signalements.filter((s) => s.sens === "emis");

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="font-display text-2xl">{profil.pseudo}</h1>
        {profil.is_master ? <Pastille ton="ok">🔒 compte maître</Pastille> : null}
        {profil.role !== "utilisateur" ? <Pastille ton="ok">{profil.role}</Pastille> : null}
        {profil.banned_at ? <Pastille ton="alerte">banni</Pastille> : null}
        {profil.restricted_until ? <Pastille ton="attente">restreint</Pastille> : null}
        {profil.deleted_at ? <Pastille ton="neutre">compte supprimé</Pastille> : null}
        <Pastille ton={score >= 5 ? "alerte" : "neutre"}>score de fiabilité {score}</Pastille>
        <Link
          href={`/troqueur/${encodeURIComponent(profil.pseudo)}`}
          className="ml-auto text-sm text-terracotta-700 underline"
        >
          voir son profil public
        </Link>
      </div>

      <Carte titre="Identité">
        <dl className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-sm sm:grid-cols-3">
          <div>
            <dt className="text-xs text-neutre-700">E-mail</dt>
            <dd>
              {profil.email}{" "}
              {profil.email_verified ? (
                <span className="text-sauge-800">✓ vérifié</span>
              ) : (
                <span className="text-terracotta-800">non vérifié</span>
              )}
            </dd>
          </div>
          <div>
            <dt className="text-xs text-neutre-700">Localisation</dt>
            <dd>
              {profil.commune ?? "—"} ({profil.postal_code})
            </dd>
          </div>
          <div>
            <dt className="text-xs text-neutre-700">Inscrit</dt>
            <dd>{date(profil.created_at)}</dd>
          </div>
          <div>
            <dt className="text-xs text-neutre-700">Dernière connexion</dt>
            <dd>{profil.derniere_activite ? timeAgo(profil.derniere_activite) : "jamais"}</dd>
          </div>
          <div>
            <dt className="text-xs text-neutre-700">Double authentification</dt>
            <dd>{profil.totp_actif ? "active" : "inactive"}</dd>
          </div>
          <div>
            <dt className="text-xs text-neutre-700">Bloqué par</dt>
            <dd>
              {compteurs.blocages_subis} membre{compteurs.blocages_subis > 1 ? "s" : ""}
            </dd>
          </div>
        </dl>
      </Carte>

      <Carte titre="Son activité en chiffres">
        <div className="grid grid-cols-3 gap-2 sm:grid-cols-6">
          <Statistique valeur={compteurs.annonces} libelle="annonces" />
          <Statistique valeur={compteurs.propositions_envoyees} libelle="propositions émises" />
          <Statistique valeur={compteurs.propositions_recues} libelle="propositions reçues" />
          <Statistique valeur={compteurs.trocs_finalises} libelle="trocs finalisés" />
          <Statistique valeur={compteurs.messages} libelle="messages" />
          <Statistique
            valeur={
              compteurs.note_moyenne == null ? "—" : `${compteurs.note_moyenne.toFixed(1)}/5`
            }
            libelle={`note (${compteurs.evaluations})`}
          />
        </div>
        <ul className="flex flex-col gap-1 text-sm text-neutre-800">
          <li>
            📦 {compteurs.trocs} troc(s) au total · {compteurs.trocs_annules} annulé(s) ·{" "}
            {compteurs.annonces_masquees} annonce(s) masquée(s)
          </li>
          <li>
            ⚖️ {compteurs.litiges_ouverts_par_lui} litige(s) ouvert(s) par lui ·{" "}
            {compteurs.litiges_subis} subi(s)
          </li>
          <li>
            🚩 {compteurs.signalements_recus} signalement(s) reçu(s) dont{" "}
            {compteurs.signalements_fondes} fondé(s) · {compteurs.signalements_emis} émis
          </li>
        </ul>
      </Carte>

      <Carte titre={`Ses annonces (${annonces.length})`}>
        {annonces.length === 0 ? (
          <p className="text-sm text-neutre-700">Ce membre n&apos;a jamais publié.</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {annonces.map((a) => (
              <li key={a.id} className="flex flex-wrap items-center gap-3 rounded-2xl bg-creme p-2.5">
                {a.photo_url ? (
                  // eslint-disable-next-line @next/next/no-img-element
                  <img src={a.photo_url} alt="" className="h-11 w-11 rounded-xl object-cover" />
                ) : null}
                <Link href={`/objet/${a.id}`} className="font-semibold hover:underline">
                  {a.title}
                </Link>
                <span className="text-xs text-neutre-700">
                  {a.category} · {euros(a.value_cents)} · {date(a.created_at)}
                </span>
                <Pastille ton={a.status === "masque" ? "attente" : "neutre"}>
                  {STATUTS_ANNONCE[a.status] ?? a.status}
                </Pastille>
                {a.signalements_ouverts > 0 ? (
                  <Pastille ton="alerte">{a.signalements_ouverts} signalement(s)</Pastille>
                ) : null}
              </li>
            ))}
          </ul>
        )}
        <Link
          href={`/admin/annonces?owner=${encodeURIComponent(profil.pseudo)}`}
          className="w-fit text-xs text-terracotta-700 underline"
        >
          modérer ses annonces
        </Link>
      </Carte>

      <Carte titre={`Ses trocs (${trocs.length})`}>
        {trocs.length === 0 ? (
          <p className="text-sm text-neutre-700">Aucun troc engagé.</p>
        ) : (
          <ul className="flex flex-col gap-1.5">
            {trocs.map((t) => (
              <li
                key={t.id}
                className="flex flex-wrap items-center gap-2 rounded-2xl bg-creme px-3 py-2 text-sm"
              >
                <span className="font-semibold">
                  avec{" "}
                  <Link
                    href={`/admin/membre/${encodeURIComponent(t.autre_pseudo)}`}
                    className="underline"
                  >
                    {t.autre_pseudo}
                  </Link>
                </span>
                <Pastille ton={t.status === "finalise" ? "ok" : "neutre"}>{t.status}</Pastille>
                <span className="text-neutre-700">
                  {t.role} · {t.delivery_mode === "envoi" ? "envoi" : "main propre"}
                  {t.cash_cents > 0 ? ` · soulte ${euros(t.cash_cents)}` : ""}
                </span>
                {t.litige ? <Pastille ton="alerte">litige</Pastille> : null}
                <span className="ml-auto text-xs text-neutre-700">{date(t.created_at)}</span>
              </li>
            ))}
          </ul>
        )}
      </Carte>

      <div className="grid gap-4 sm:grid-cols-2">
        <Carte titre={`Signalements reçus (${recus.length})`}>
          {recus.length === 0 ? (
            <p className="text-sm text-neutre-700">Aucun. Bon signe.</p>
          ) : (
            <ul className="flex flex-col gap-2">
              {recus.map((s) => (
                <li key={s.id} className="flex flex-col gap-0.5 rounded-2xl bg-creme p-3 text-sm">
                  <span className="flex flex-wrap items-center gap-1.5">
                    <Pastille ton={s.status === "nouveau" ? "attente" : "neutre"}>
                      {s.outcome ?? s.status}
                    </Pastille>
                    <span className="font-semibold">{s.reason.replace(/_/g, " ")}</span>
                    <span className="text-xs text-neutre-700">
                      sur {s.target_type}
                      {s.cible ? ` « ${s.cible} »` : ""} · par {s.autre_pseudo ?? "inconnu"} ·{" "}
                      {date(s.created_at)}
                    </span>
                  </span>
                  {s.comment ? (
                    <span className="text-xs text-neutre-700">« {s.comment} »</span>
                  ) : null}
                </li>
              ))}
            </ul>
          )}
        </Carte>

        <Carte titre={`Signalements qu'il a émis (${emis.length})`}>
          {emis.length === 0 ? (
            <p className="text-sm text-neutre-700">Aucun.</p>
          ) : (
            <ul className="flex flex-col gap-2">
              {emis.map((s) => (
                <li key={s.id} className="flex flex-col gap-0.5 rounded-2xl bg-creme p-3 text-sm">
                  <span className="flex flex-wrap items-center gap-1.5">
                    <Pastille ton={s.status === "nouveau" ? "attente" : "neutre"}>
                      {s.outcome ?? s.status}
                    </Pastille>
                    <span className="font-semibold">{s.reason.replace(/_/g, " ")}</span>
                    <span className="text-xs text-neutre-700">
                      contre {s.autre_pseudo ?? "?"}
                      {s.cible ? ` « ${s.cible} »` : ""} · {date(s.created_at)}
                    </span>
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Carte>
      </div>

      <Carte titre={`Historique de modération (${sanctions.length})`}>
        {sanctions.length === 0 ? (
          <p className="text-sm text-neutre-700">Aucun incident enregistré.</p>
        ) : (
          <ul className="flex flex-col gap-1.5 text-sm">
            {sanctions.map((s, i) => (
              <li key={`${s.created_at}-${i}`} className="flex flex-wrap items-center gap-2">
                <span className="font-semibold">{s.event_type.replace(/_/g, " ")}</span>
                {s.details ? <span className="text-neutre-700">{s.details}</span> : null}
                <span className="ml-auto text-xs text-neutre-700">{date(s.created_at)}</span>
              </li>
            ))}
          </ul>
        )}
      </Carte>
    </div>
  );
}
