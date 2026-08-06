import { adminFetch, adminPost } from "./adminFetch";
export const dynamic = "force-dynamic";

// Hub d'administration (F6.1) — brut, derrière basic auth Traefik + token.


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
    <main className="mx-auto flex w-full max-w-3xl flex-col gap-4 p-6 font-mono text-sm">
      <h1 className="text-xl font-bold">Admin Lebontroc</h1>
      <nav className="flex gap-3 text-xs underline">
        <a href="/admin/litiges">Litiges</a>
        <a href="/admin/signalements">Signalements</a>
        <a href="/admin/audit">Audit</a>
        <a href="/admin/equipe">Équipe</a>
        <a href="/admin/liens">Mailpit</a>
      </nav>

      {kpis ? (
        <section className="rounded border p-3">
          <h2 className="font-bold">7 derniers jours</h2>
          <p>
            {kpis.signups} inscriptions · {kpis.items_published} objets · {kpis.proposals_sent}{" "}
            propositions · {kpis.trades_created} trocs créés · {kpis.trades_finalized} finalisés
            (dont {kpis.trades_with_cash} avec soulte) · {kpis.disputes_opened} litiges
          </p>
        </section>
      ) : null}

      <form action="/admin" method="get" className="flex gap-2">
        <input
          name="q"
          defaultValue={q ?? ""}
          placeholder="pseudo, e-mail, titre ou UUID de troc"
          className="flex-1 border px-2 py-1"
        />
        <button type="submit" className="border bg-black px-3 py-1 text-white">
          Chercher
        </button>
      </form>

      {results ? (
        <section className="flex flex-col gap-3">
          <div>
            <h2 className="font-bold">Utilisateurs ({results.users.length})</h2>
            {results.users.map((u) => (
              <p key={u.id}>
                {u.pseudo} · {u.email} · {u.role}
                {u.is_master ? " 🔒" : ""} · score {u.score}
                {u.banned_at ? " · ⛔ BANNI" : u.restricted_until ? " · ⚠️ restreint" : ""} ·{" "}
                {u.id}
              </p>
            ))}
          </div>
          <div>
            <h2 className="font-bold">Objets ({results.items.length})</h2>
            {results.items.map((i) => (
              <p key={i.id}>
                {i.title} · {i.status} · @{i.owner_pseudo} ·{" "}
                <a href={`/objet/${i.id}`} className="underline">
                  voir
                </a>
              </p>
            ))}
          </div>
          <div>
            <h2 className="font-bold">Trocs ({results.trades.length})</h2>
            {results.trades.map((t) => (
              <p key={t.id}>
                [{t.status}] {t.proposer_pseudo} ↔ {t.recipient_pseudo} · {t.delivery_mode} ·{" "}
                {t.id}
              </p>
            ))}
          </div>
        </section>
      ) : null}
    </main>
  );
}
