import type { Metadata } from "next";

import { adminFetchStatus } from "../adminFetch";
import { Carte, Pastille } from "../ui";

export const metadata: Metadata = {
  title: "E-mails — Admin Lebontroc",
};

export const dynamic = "force-dynamic";

// La boîte d'envoi de la bêta. Deux usages : agir vite sur les liens de
// vérification, et pouvoir tout relire — d'où la boîte Mailpit complète
// intégrée en bas. Réservé aux super-administrateurs : le contenu des
// e-mails, ce sont des jetons de connexion et des données personnelles.

const MAILPIT_PUBLIC = "/mailpit/";

type MailpitMessage = {
  ID: string;
  To: Array<{ Address: string }>;
  Subject: string;
  Created: string;
  Snippet: string;
  Read: boolean;
};

type Entree = {
  id: string;
  destinataire: string;
  sujet: string;
  apercu: string;
  date: string;
  lien: string | null;
  lu: boolean;
};

/** Les liens actionnables qu'un e-mail transactionnel peut porter. */
const MOTIFS_LIEN = [
  /https?:\/\/[^\s"<>]+verify-email\?token=[A-Za-z0-9_-]+/,
  /https?:\/\/[^\s"<>]+token=[A-Za-z0-9_-]{16,}/,
];

async function chargerBoite(): Promise<Entree[] | null> {
  const base = process.env.MAILPIT_URL ?? "http://localhost:8025";
  try {
    const response = await fetch(`${base}/api/v1/messages?limit=100`, { cache: "no-store" });
    if (!response.ok) return null;
    const data = (await response.json()) as { messages?: MailpitMessage[] };
    const messages = data.messages ?? [];
    return await Promise.all(
      messages.map(async (message) => {
        let lien: string | null = null;
        try {
          const detail = await fetch(`${base}/api/v1/message/${message.ID}`, {
            cache: "no-store",
          });
          const corps = (await detail.json()) as { Text?: string; HTML?: string };
          const texte = `${corps.Text ?? ""}\n${corps.HTML ?? ""}`;
          for (const motif of MOTIFS_LIEN) {
            const trouve = texte.match(motif)?.[0];
            if (trouve) {
              lien = trouve;
              break;
            }
          }
        } catch {
          // message illisible : on liste quand même l'entrée
        }
        return {
          id: message.ID,
          destinataire: message.To[0]?.Address ?? "?",
          sujet: message.Subject,
          apercu: message.Snippet,
          date: new Date(message.Created).toLocaleString("fr-FR", {
            dateStyle: "short",
            timeStyle: "short",
          }),
          lien,
          lu: message.Read,
        };
      }),
    );
  } catch {
    return null;
  }
}

export default async function EmailsAdminPage() {
  // Le rôle décide, pas seulement le mot de passe du proxy : /admin/staff
  // n'est ouvert qu'aux super-administrateurs.
  const garde = await adminFetchStatus<unknown>("/admin/staff");
  if (garde.status !== 200) {
    return (
      <Carte>
        <p className="text-sm text-neutre-700">
          Les e-mails contiennent des jetons de connexion et des données personnelles : leur
          lecture est réservée aux super-administrateurs. Si tu en es un, ta session doit
          revérifier sa double authentification (reconnecte-toi).
        </p>
      </Carte>
    );
  }

  const boite = await chargerBoite();
  const enAttente = boite?.filter((e) => e.lien) ?? [];

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="font-display text-2xl">E-mails</h1>
        <a
          href={MAILPIT_PUBLIC}
          target="_blank"
          rel="noreferrer"
          className="ml-auto text-sm text-terracotta-700 underline"
        >
          ouvrir la boîte dans un onglet ↗
        </a>
      </div>

      <Carte>
        <p className="text-sm text-neutre-700">
          Pendant la bêta, aucun e-mail ne part vraiment : ils sont tous capturés ici. C&apos;est
          aussi le moyen d&apos;activer un compte à la main quand un membre ne reçoit pas son lien
          de vérification.
        </p>
      </Carte>

      {boite === null ? (
        <Carte>
          <p className="text-sm text-terracotta-800">
            Mailpit est injoignable pour le moment — la boîte ci-dessous restera vide.
          </p>
        </Carte>
      ) : (
        <Carte titre={`Liens à activer (${enAttente.length})`}>
          {enAttente.length === 0 ? (
            <p className="text-sm text-neutre-700">
              Aucun lien de vérification ou de réinitialisation en attente.
            </p>
          ) : (
            <ul className="flex flex-col gap-2">
              {enAttente.map((e) => (
                <li
                  key={e.id}
                  className="flex flex-wrap items-center gap-2 rounded-2xl bg-creme px-3 py-2"
                >
                  <div className="flex min-w-48 flex-1 flex-col">
                    <span className="truncate text-sm font-semibold">{e.destinataire}</span>
                    <span className="truncate text-xs text-neutre-700">
                      {e.sujet} · {e.date}
                    </span>
                  </div>
                  {!e.lu ? <Pastille ton="attente">non lu</Pastille> : null}
                  <a
                    href={e.lien ?? "#"}
                    className="rounded-full bg-[#c67139] px-4 py-1.5 font-display text-xs text-creme transition-colors hover:bg-terracotta-600"
                  >
                    Ouvrir le lien
                  </a>
                </li>
              ))}
            </ul>
          )}
        </Carte>
      )}

      {boite && boite.length > 0 ? (
        <Carte titre={`Tous les envois (${boite.length})`}>
          <ul className="flex max-h-80 flex-col gap-1.5 overflow-y-auto">
            {boite.map((e) => (
              <li key={e.id} className="flex flex-col rounded-2xl bg-creme px-3 py-2 text-sm">
                <span className="flex flex-wrap items-baseline gap-2">
                  <span className="font-semibold">{e.sujet}</span>
                  <span className="text-xs text-neutre-700">
                    → {e.destinataire} · {e.date}
                  </span>
                </span>
                <span className="truncate text-xs text-neutre-700">{e.apercu}</span>
              </li>
            ))}
          </ul>
        </Carte>
      ) : null}

      {/* La boîte Mailpit complète, ici même : lire un message entier, ses
          en-têtes, sa version HTML, sans quitter l'administration. */}
      <Carte titre="Boîte complète">
        <iframe
          src={MAILPIT_PUBLIC}
          title="Boîte de réception Mailpit"
          className="h-[42rem] w-full rounded-3xl border border-neutre-300 bg-creme"
        />
      </Carte>
    </div>
  );
}
