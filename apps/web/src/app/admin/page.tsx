import Link from "next/link";

import { adminFetch } from "./adminFetch";
import { Carte, Pastille, Statistique, champ } from "./ui";

export const dynamic = "force-dynamic";

type Kpis = {
  signups: number;
  items_published: number;
  proposals_sent: number;
  trades_created: number;
  trades_finalized: number;
  trades_with_cash: number;
  disputes_opened: number;
};

type Search = {
  users: {
    id: string;
    pseudo: string;
    role: string;
    is_master: boolean;
    email: string;
    score: number;
    restricted_until: string | null;
    banned_at: string | null;
  }[];
  items: { id: string; title: string; status: string; owner_pseudo: string }[];
  trades: {
    id: string;
    status: string;
    delivery_mode: string;
    proposer_pseudo: string;
    recipient_pseudo: string;
  }[];
};

export default async function AdminHub({
  searchParams,
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const { q } = await searchParams;
  const [kpis, results] = await Promise.all([
    adminFetch<Kpis>("/admin/kpis"),
    q ? adminFetch<Search>(`/admin/search?q=${encodeURIComponent(q)}`) : Promise.resolve(null),
  ]);

  return (
    <div className="flex flex-col gap-4">
      {kpis ? (
        <Carte titre="Les 7 derniers jours">
          <div className="grid grid-cols-3 gap-2 sm:grid-cols-7">
            <Statistique valeur={kpis.signups} libelle="inscriptions" />
            <Statistique valeur={kpis.items_published} libelle="objets publiés" />
            <Statistique valeur={kpis.proposals_sent} libelle="propositions" />
            <Statistique valeur={kpis.trades_created} libelle="trocs créés" />
            <Statistique valeur={kpis.trades_finalized} libelle="finalisés" />
            <Statistique valeur={kpis.trades_with_cash} libelle="avec soulte" />
            <Statistique valeur={kpis.disputes_opened} libelle="litiges" />
          </div>
        </Carte>
      ) : (
        <Carte>
          <p className="text-sm text-neutre-700">
            Les indicateurs et la recherche sont réservés aux super-administrateurs — ou ta
            session doit revérifier sa double authentification (reconnecte-toi).
          </p>
        </Carte>
      )}

      <Carte titre="Rechercher">
        <form action="/admin" method="get" className="flex gap-2">
          <input
            name="q"
            defaultValue={q ?? ""}
            placeholder="Pseudo, e-mail, titre d'objet ou identifiant de troc…"
            className={`flex-1 ${champ}`}
          />
          <button
            type="submit"
            className="flex min-h-10 cursor-pointer items-center rounded-full bg-[#c67139] px-5 font-display text-sm text-creme hover:bg-terracotta-600"
          >
            Chercher
          </button>
        </form>

        {results ? (
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
              <h3 className="text-sm font-bold">Utilisateurs ({results.users.length})</h3>
              {results.users.map((u) => (
                <div
                  key={u.id}
                  className="flex flex-wrap items-center gap-2 rounded-2xl bg-creme px-3 py-2 text-sm"
                >
                  <Link href={`/troqueur/${u.pseudo}`} className="font-semibold hover:underline">
                    {u.pseudo}
                  </Link>
                  <span className="text-neutre-700">{u.email}</span>
                  {u.role !== "utilisateur" ? <Pastille ton="ok">{u.role}</Pastille> : null}
                  {u.is_master ? <Pastille ton="ok">🔒 maître</Pastille> : null}
                  <Pastille ton={u.score >= 5 ? "alerte" : "neutre"}>score {u.score}</Pastille>
                  {u.banned_at ? <Pastille ton="alerte">banni</Pastille> : null}
                  {u.restricted_until ? <Pastille ton="attente">restreint</Pastille> : null}
                </div>
              ))}
            </div>
            <div className="flex flex-col gap-1.5">
              <h3 className="text-sm font-bold">Objets ({results.items.length})</h3>
              {results.items.map((i) => (
                <div
                  key={i.id}
                  className="flex flex-wrap items-center gap-2 rounded-2xl bg-creme px-3 py-2 text-sm"
                >
                  <Link href={`/objet/${i.id}`} className="font-semibold hover:underline">
                    {i.title}
                  </Link>
                  <Pastille ton="neutre">{i.status}</Pastille>
                  <span className="text-neutre-700">@{i.owner_pseudo}</span>
                </div>
              ))}
            </div>
            <div className="flex flex-col gap-1.5">
              <h3 className="text-sm font-bold">Trocs ({results.trades.length})</h3>
              {results.trades.map((t) => (
                <div
                  key={t.id}
                  className="flex flex-wrap items-center gap-2 rounded-2xl bg-creme px-3 py-2 text-sm"
                >
                  <span className="font-semibold">
                    {t.proposer_pseudo} ↔ {t.recipient_pseudo}
                  </span>
                  <Pastille ton="neutre">{t.status}</Pastille>
                  <span className="text-neutre-700">{t.delivery_mode}</span>
                  <span className="truncate text-[11px] text-neutre-700">{t.id}</span>
                </div>
              ))}
            </div>
          </div>
        ) : null}
      </Carte>

      <p className="text-xs text-neutre-700">
        Les e-mails de la bêta restent consultables sur{" "}
        <a href="/admin/liens" className="underline">
          la boîte Mailpit
        </a>
        .
      </p>
    </div>
  );
}
