import Link from "next/link";
import { notFound } from "next/navigation";

import { euros } from "@/lib/format";

import { adminFetchStatus } from "../../adminFetch";
import { Carte, Pastille } from "../../ui";

export const dynamic = "force-dynamic";

// Le fil complet d'un échange, vu de la modération. C'est de la
// correspondance privée : la page le dit, et chaque ouverture est
// inscrite au journal d'audit côté API.

type Message = {
  id: string;
  sender_pseudo: string;
  body: string;
  photo_url: string | null;
  redacted: boolean;
  signale: boolean;
  created_at: string;
  read_at: string | null;
};

type Conversation = {
  proposal_id: string;
  statut: string;
  proposer_pseudo: string;
  recipient_pseudo: string;
  cash_cents: number;
  cash_direction: string;
  created_at: string;
  objets_demandes: string | null;
  objets_offerts: string | null;
  messages: Message[];
};

const horodatage = (iso: string) =>
  new Date(iso).toLocaleString("fr-FR", {
    day: "2-digit",
    month: "2-digit",
    year: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });

export default async function AdminConversationPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  const reponse = await adminFetchStatus<Conversation>(
    `/admin/conversations/${encodeURIComponent(id)}`,
  );
  if (reponse.status === 404) notFound();
  const fil = reponse.data;
  if (!fil) {
    return (
      <Carte>
        <p className="text-sm text-neutre-700">
          La lecture des conversations est réservée aux super-administrateurs — ou ta session doit
          revérifier sa double authentification (reconnecte-toi).
        </p>
      </Carte>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="font-display text-2xl">
          <Link
            href={`/admin/membre/${encodeURIComponent(fil.proposer_pseudo)}`}
            className="hover:underline"
          >
            {fil.proposer_pseudo}
          </Link>{" "}
          ↔{" "}
          <Link
            href={`/admin/membre/${encodeURIComponent(fil.recipient_pseudo)}`}
            className="hover:underline"
          >
            {fil.recipient_pseudo}
          </Link>
        </h1>
        <Pastille ton={fil.statut === "acceptee" ? "ok" : "neutre"}>{fil.statut}</Pastille>
      </div>

      <Carte titre="L'échange négocié">
        <dl className="grid gap-x-4 gap-y-1.5 text-sm sm:grid-cols-3">
          <div>
            <dt className="text-xs text-neutre-700">{fil.proposer_pseudo} propose</dt>
            <dd>{fil.objets_offerts ?? "—"}</dd>
          </div>
          <div>
            <dt className="text-xs text-neutre-700">et demande</dt>
            <dd>{fil.objets_demandes ?? "—"}</dd>
          </div>
          <div>
            <dt className="text-xs text-neutre-700">Soulte</dt>
            <dd>
              {fil.cash_cents > 0
                ? `${euros(fil.cash_cents)} (${fil.cash_direction.replace(/_/g, " ")})`
                : "aucune"}
            </dd>
          </div>
        </dl>
        <p className="text-xs text-neutre-700">
          Échange ouvert le {horodatage(fil.created_at)} · {fil.messages.length} message
          {fil.messages.length > 1 ? "s" : ""}
        </p>
      </Carte>

      <Carte titre="Le fil">
        {fil.messages.length === 0 ? (
          <p className="text-sm text-neutre-700">Aucun message échangé.</p>
        ) : (
          <ol className="flex flex-col gap-2">
            {fil.messages.map((m) => {
              const duProposant = m.sender_pseudo === fil.proposer_pseudo;
              return (
                <li
                  key={m.id}
                  className={`flex max-w-[85%] flex-col gap-1 rounded-3xl px-4 py-2.5 text-sm ${
                    duProposant
                      ? "self-start bg-creme"
                      : "self-end bg-terracotta-100/70 text-right"
                  }`}
                >
                  <span className="flex flex-wrap items-center gap-1.5 text-xs text-neutre-700">
                    <span className="font-semibold text-encre">{m.sender_pseudo}</span>
                    <span>{horodatage(m.created_at)}</span>
                    {m.redacted ? <Pastille ton="attente">coordonnées masquées</Pastille> : null}
                    {m.signale ? <Pastille ton="alerte">signalé</Pastille> : null}
                    {m.read_at ? <span>· lu</span> : <span>· non lu</span>}
                  </span>
                  <span className="whitespace-pre-wrap">{m.body}</span>
                  {m.photo_url ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={m.photo_url}
                      alt="Pièce jointe"
                      className="mt-1 max-h-56 w-fit rounded-2xl object-cover"
                    />
                  ) : null}
                </li>
              );
            })}
          </ol>
        )}
      </Carte>

      <p className="text-xs text-neutre-700">
        Cette correspondance est privée. Son ouverture vient d&apos;être inscrite au{" "}
        <Link href="/admin/audit" className="underline">
          journal d&apos;audit
        </Link>{" "}
        avec ton nom : ne la consulte que pour traiter un signalement ou un litige.
      </p>
    </div>
  );
}
