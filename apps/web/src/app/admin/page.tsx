import Link from "next/link";

import { euros } from "@/lib/format";

import { adminFetch } from "./adminFetch";
import { Carte, Pastille, Sparkline, Statistique, Variation, champ } from "./ui";

export const dynamic = "force-dynamic";

type Point = {
  jour: string;
  inscriptions: number;
  annonces: number;
  propositions: number;
  trocs_finalises: number;
  volume_soulte_cents: number;
};

type Dashboard = {
  series: Point[];
  activite: {
    inscrits_total: number;
    comptes_supprimes: number;
    comptes_bannis: number;
    comptes_restreints: number;
    dau: number;
    wau: number;
    mau: number;
    recherches_7j: number;
    messages_7j: number;
    favoris_total: number;
    notifications_ouvertes_7j: number;
  };
  marketplace: {
    annonces_actives: number;
    annonces_reservees: number;
    annonces_troquees: number;
    propositions_total: number;
    contre_propositions: number;
    taux_acceptation_pct: number;
    heures_moyennes_avant_accord: number | null;
    valeur_echangee_cents: number;
    heures_avant_premier_message: number | null;
  };
  top_categories: { libelle: string; total: number }[];
  top_communes: { libelle: string; total: number }[];
  qualite: {
    litiges_ouverts: number;
    litiges_en_examen: number;
    litiges_tranches: number;
    heures_moyennes_resolution: number | null;
    signalements_en_attente: number;
    note_moyenne: number | null;
  };
  finances_beta: {
    soultes_capturees_cents: number;
    soultes_sequestrees_cents: number;
    frais_service_percus_cents: number;
    transport_encaisse_cents: number;
    commissions_cents: number;
    paiements_echoues: number;
    jours_moyens_finalisation: number | null;
    colis_expedies: number;
    trocs_envoi_litigieux: number;
  };
  systeme: {
    version: string;
    taille_base: string;
    evenements_telemetrie: number;
    evenements_non_exportes: number;
    notifications_stockees: number;
    sessions_actives: number;
  };
  tendance: {
    litiges_7j: number;
    litiges_7j_precedents: number;
    trocs_7j: number;
    trocs_7j_precedents: number;
    echecs_paiement_7j: number;
  };
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

const heures = (h: number | null) =>
  h == null ? "—" : h < 48 ? `${Math.round(h)} h` : `${(h / 24).toFixed(1)} j`;

export default async function AdminHub({
  searchParams,
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const { q } = await searchParams;
  const [dash, results] = await Promise.all([
    adminFetch<Dashboard>("/admin/dashboard"),
    q ? adminFetch<Search>(`/admin/search?q=${encodeURIComponent(q)}`) : Promise.resolve(null),
  ]);

  return (
    <div className="flex flex-col gap-4">
      {dash ? (
        <>
          {/* Vision exécutive : la santé de la plateforme en un regard. */}
          <Carte titre="Vue d'ensemble">
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-5">
              <Statistique valeur={dash.activite.inscrits_total} libelle="membres" />
              <Statistique valeur={dash.activite.mau} libelle="actifs sur 30 j" />
              <Statistique
                valeur={euros(dash.marketplace.valeur_echangee_cents)}
                libelle="valeur échangée"
              />
              <Statistique
                valeur={`${Math.round(dash.marketplace.taux_acceptation_pct)} %`}
                libelle="propositions acceptées"
              />
              <Statistique valeur={dash.qualite.litiges_ouverts} libelle="litiges ouverts" />
            </div>
            <div className="flex flex-wrap items-center gap-2 text-sm">
              <span className="text-neutre-700">Trocs sur 7 jours :</span>
              <span className="font-semibold">{dash.tendance.trocs_7j}</span>
              <Variation
                actuel={dash.tendance.trocs_7j}
                precedent={dash.tendance.trocs_7j_precedents}
              />
              <span className="ml-3 text-neutre-700">Litiges sur 7 jours :</span>
              <span className="font-semibold">{dash.tendance.litiges_7j}</span>
              <Variation
                actuel={dash.tendance.litiges_7j}
                precedent={dash.tendance.litiges_7j_precedents}
                inverse
              />
              {dash.tendance.echecs_paiement_7j > 0 ? (
                <Pastille ton="attente">
                  {dash.tendance.echecs_paiement_7j} échec(s) de paiement
                </Pastille>
              ) : null}
            </div>
          </Carte>

          <Carte titre="Les 30 derniers jours">
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-5">
              <Sparkline
                valeurs={dash.series.map((p) => p.inscriptions)}
                libelle="inscriptions"
              />
              <Sparkline valeurs={dash.series.map((p) => p.annonces)} libelle="objets publiés" />
              <Sparkline
                valeurs={dash.series.map((p) => p.propositions)}
                libelle="propositions"
              />
              <Sparkline
                valeurs={dash.series.map((p) => p.trocs_finalises)}
                libelle="trocs finalisés"
              />
              <Sparkline
                valeurs={dash.series.map((p) => p.volume_soulte_cents / 100)}
                libelle="volume de soultes (€)"
              />
            </div>
          </Carte>

          <div className="grid gap-4 sm:grid-cols-2">
            <Carte titre="Activité & engagement">
              <div className="grid grid-cols-3 gap-2">
                <Statistique valeur={dash.activite.dau} libelle="actifs 24 h" />
                <Statistique valeur={dash.activite.wau} libelle="actifs 7 j" />
                <Statistique valeur={dash.activite.mau} libelle="actifs 30 j" />
              </div>
              <ul className="flex flex-col gap-1 text-sm text-neutre-800">
                <li>🔍 {dash.activite.recherches_7j} recherches sur 7 jours</li>
                <li>💬 {dash.activite.messages_7j} messages sur 7 jours</li>
                <li>❤️ {dash.activite.favoris_total} favoris posés au total</li>
                <li>
                  🔔 {dash.activite.notifications_ouvertes_7j} notifications ouvertes sur 7 jours
                </li>
                <li className="pt-1 text-neutre-700">
                  {dash.activite.comptes_bannis} bannis · {dash.activite.comptes_restreints}{" "}
                  restreints · {dash.activite.comptes_supprimes} comptes supprimés
                </li>
              </ul>
            </Carte>

            <Carte titre="Marketplace">
              <div className="grid grid-cols-3 gap-2">
                <Statistique valeur={dash.marketplace.annonces_actives} libelle="annonces actives" />
                <Statistique valeur={dash.marketplace.annonces_reservees} libelle="réservées" />
                <Statistique valeur={dash.marketplace.annonces_troquees} libelle="troquées" />
              </div>
              <ul className="flex flex-col gap-1 text-sm text-neutre-800">
                <li>
                  🤝 {dash.marketplace.propositions_total} propositions, dont{" "}
                  {dash.marketplace.contre_propositions} contre-propositions
                </li>
                <li>
                  ⏱️ Accord conclu en {heures(dash.marketplace.heures_moyennes_avant_accord)} en
                  moyenne
                </li>
                <li>
                  ✉️ Premier message {heures(dash.marketplace.heures_avant_premier_message)} après
                  la proposition
                </li>
              </ul>
            </Carte>

            <Carte titre="Tops">
              <div className="grid grid-cols-2 gap-4 text-sm">
                <div>
                  <h3 className="mb-1 font-bold">Catégories</h3>
                  <ol className="flex flex-col gap-1">
                    {dash.top_categories.map((t) => (
                      <li key={t.libelle} className="flex justify-between gap-2">
                        <span className="truncate">{t.libelle}</span>
                        <span className="font-semibold text-terracotta-800">{t.total}</span>
                      </li>
                    ))}
                  </ol>
                </div>
                <div>
                  <h3 className="mb-1 font-bold">Communes</h3>
                  <ol className="flex flex-col gap-1">
                    {dash.top_communes.map((t) => (
                      <li key={t.libelle} className="flex justify-between gap-2">
                        <span className="truncate">{t.libelle}</span>
                        <span className="font-semibold text-terracotta-800">{t.total}</span>
                      </li>
                    ))}
                  </ol>
                </div>
              </div>
            </Carte>

            <Carte titre="Support & qualité">
              <div className="grid grid-cols-3 gap-2">
                <Statistique valeur={dash.qualite.litiges_ouverts} libelle="litiges ouverts" />
                <Statistique valeur={dash.qualite.litiges_en_examen} libelle="en examen" />
                <Statistique valeur={dash.qualite.litiges_tranches} libelle="tranchés" />
              </div>
              <ul className="flex flex-col gap-1 text-sm text-neutre-800">
                <li>
                  ⏳ Résolution en {heures(dash.qualite.heures_moyennes_resolution)} en moyenne
                </li>
                <li>🚩 {dash.qualite.signalements_en_attente} signalements en attente</li>
                <li>
                  ⭐{" "}
                  {dash.qualite.note_moyenne == null
                    ? "Pas encore d'évaluations"
                    : `${dash.qualite.note_moyenne.toFixed(1)} / 5 de note moyenne`}
                </li>
              </ul>
            </Carte>
          </div>

          <Carte titre="Finances (bêta — paiements simulés)">
            <p className="text-xs text-neutre-700">
              Aucun argent réel ne circule pendant la bêta : ces montants mesurent les parcours,
              pas la trésorerie. Ils deviendront comptables à l&apos;arrivée du prestataire de
              paiement.
            </p>
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-5">
              <Statistique
                valeur={euros(dash.finances_beta.soultes_capturees_cents)}
                libelle="soultes capturées"
              />
              <Statistique
                valeur={euros(dash.finances_beta.soultes_sequestrees_cents)}
                libelle="sous séquestre"
              />
              <Statistique
                valeur={euros(dash.finances_beta.frais_service_percus_cents)}
                libelle="frais de service"
              />
              <Statistique
                valeur={euros(dash.finances_beta.transport_encaisse_cents)}
                libelle="transport encaissé"
              />
              <Statistique
                valeur={euros(dash.finances_beta.commissions_cents)}
                libelle="commissions"
              />
            </div>
            <ul className="flex flex-col gap-1 text-sm text-neutre-800">
              <li>
                📦 {dash.finances_beta.colis_expedies} colis expédiés ·{" "}
                {dash.finances_beta.trocs_envoi_litigieux} troc(s) d&apos;envoi en litige
              </li>
              <li>
                ⏱️ Finalisation en{" "}
                {dash.finances_beta.jours_moyens_finalisation == null
                  ? "—"
                  : `${dash.finances_beta.jours_moyens_finalisation.toFixed(1)} j`}{" "}
                en moyenne · {dash.finances_beta.paiements_echoues} paiement(s) en échec
              </li>
            </ul>
          </Carte>

          <Carte titre="Système">
            <div className="flex flex-wrap gap-x-5 gap-y-1 text-sm text-neutre-800">
              <span>🏷️ version {dash.systeme.version}</span>
              <span>🗄️ base {dash.systeme.taille_base}</span>
              <span>📈 {dash.systeme.evenements_telemetrie} événements de télémétrie</span>
              <span>📤 {dash.systeme.evenements_non_exportes} en attente d&apos;export</span>
              <span>🔔 {dash.systeme.notifications_stockees} notifications stockées</span>
              <span>🔑 {dash.systeme.sessions_actives} sessions actives</span>
            </div>
          </Carte>
        </>
      ) : (
        <Carte>
          <p className="text-sm text-neutre-700">
            Le tableau de bord est réservé aux super-administrateurs — ou ta session doit
            revérifier sa double authentification (reconnecte-toi).
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
        Les alertes automatiques (hausse des litiges, chute des trocs, échecs de paiement en
        série) préviennent l&apos;équipe chaque heure, par notification et par e-mail. Les
        e-mails de la bêta restent consultables sur{" "}
        <a href="/admin/liens" className="underline">
          la boîte Mailpit
        </a>
        .
      </p>
    </div>
  );
}
