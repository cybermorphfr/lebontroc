"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import Link from "next/link";
import type { MessageResponse, ProposalChainEntry } from "@lebontroc/api-client";

import { AvatarLetter } from "@/components/AvatarLetter";
import { MessageSuggestions } from "@/components/MessageSuggestions";
import { apiFetch, apiError } from "@/lib/client-api";
import { ECHANGE, euros, timeAgo } from "@/lib/format";
import { compressImage } from "@/lib/photos";
import { suggestionsConversation } from "@/lib/suggestions";
import { useRealtime } from "@/lib/realtime";

/** Messages consécutifs du même auteur à moins de 10 min : un seul groupe. */
const GROUPE_MS = 10 * 60 * 1000;

type Groupe = { mine: boolean; items: MessageResponse[] };

function grouper(messages: MessageResponse[], myPseudo: string): Groupe[] {
  const groupes: Groupe[] = [];
  for (const message of messages) {
    const mine = message.sender_pseudo === myPseudo;
    const dernier = groupes[groupes.length - 1];
    const precedent = dernier?.items[dernier.items.length - 1];
    const ecart = precedent
      ? new Date(message.created_at).getTime() - new Date(precedent.created_at).getTime()
      : Number.POSITIVE_INFINITY;
    if (dernier && dernier.mine === mine && ecart < GROUPE_MS) {
      dernier.items.push(message);
    } else {
      groupes.push({ mine, items: [message] });
    }
  }
  return groupes;
}

/**
 * Fil de conversation temps réel, dans les codes des messageries de
 * marketplace (Vinted) : en-tête interlocuteur, bulles groupées avec
 * avatar côté reçu, horodatage relatif centré, accusé « Vu », barre de
 * saisie en pilule avec photo et flèche d'envoi.
 */
export function Conversation({
  proposalId,
  myPseudo,
  otherPseudo,
  initialMessages,
  chain,
  closed,
  contexte,
  actions,
}: {
  proposalId: string;
  myPseudo: string;
  otherPseudo: string;
  initialMessages: MessageResponse[];
  /// Toute la négociation : proposition initiale et contre-propositions.
  chain: ProposalChainEntry[];
  closed: boolean;
  /// Accepter / contre-proposer / refuser, sous l'offre en cours.
  actions?: React.ReactNode;
  /// Avancement du troc : pilote les suggestions de réponse.
  contexte: {
    proposalStatus: string;
    tradeStatus?: string | null;
    deliveryMode?: string | null;
    isProposer: boolean;
  };
}) {
  const [messages, setMessages] = useState<MessageResponse[]>(initialMessages);
  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [redactedNotice, setRedactedNotice] = useState(false);
  const bottom = useRef<HTMLDivElement | null>(null);
  const fileInput = useRef<HTMLInputElement | null>(null);
  const champ = useRef<HTMLInputElement | null>(null);

  const markRead = useCallback(() => {
    void apiFetch(`/proposals/${proposalId}/read`, { method: "POST" });
  }, [proposalId]);

  useEffect(() => {
    markRead();
  }, [markRead]);

  useEffect(() => {
    bottom.current?.scrollIntoView({ block: "nearest" });
  }, [messages.length]);

  useRealtime((event) => {
    if (!("proposal_id" in event) || event.proposal_id !== proposalId) return;
    if (event.type === "message") {
      const message = event.message as MessageResponse;
      setMessages((current) =>
        current.some((m) => m.id === message.id) ? current : [...current, message],
      );
      if (message.sender_pseudo !== myPseudo) markRead();
    }
    if (event.type === "read" && event.reader_pseudo !== myPseudo) {
      const now = new Date().toISOString();
      setMessages((current) =>
        current.map((m) =>
          m.sender_pseudo === myPseudo && m.read_at === null ? { ...m, read_at: now } : m,
        ),
      );
    }
  });

  async function send(photoId?: string) {
    if (sending) return;
    const body = draft.trim();
    if (!body && !photoId) return;
    setSending(true);
    setError(null);
    try {
      const response = await apiFetch(`/proposals/${proposalId}/messages`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ body: body || null, photo_id: photoId ?? null }),
      });
      if (!response.ok) {
        setError((await apiError(response)).message);
        return;
      }
      const message = (await response.json()) as MessageResponse;
      setMessages((current) =>
        current.some((m) => m.id === message.id) ? current : [...current, message],
      );
      if (message.redacted) setRedactedNotice(true);
      setDraft("");
    } finally {
      setSending(false);
    }
  }

  async function attachPhoto(file: File) {
    setSending(true);
    setError(null);
    try {
      const { blob, contentType } = await compressImage(file);
      const presign = await apiFetch("/items/photos/presign", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ files: [{ content_type: contentType, size: blob.size }] }),
      });
      if (!presign.ok) {
        setError((await apiError(presign)).message);
        return;
      }
      const [{ photo_id, upload_url }] = (await presign.json()) as {
        photo_id: string;
        upload_url: string;
      }[];
      const put = await fetch(upload_url, {
        method: "PUT",
        headers: { "Content-Type": contentType },
        body: blob,
      });
      if (!put.ok) {
        setError("La photo n'a pas pu être envoyée. Réessaie.");
        return;
      }
      setSending(false);
      await send(photo_id);
    } catch {
      setError("La photo n'a pas pu être lue.");
    } finally {
      setSending(false);
    }
  }

  // Un seul endroit pour négocier : les propositions s'intercalent entre
  // les messages, dans l'ordre chronologique.
  const frise: (
    | { genre: "groupe"; at: string; groupe: Groupe }
    | { genre: "proposition"; at: string; entree: ProposalChainEntry }
  )[] = [
    ...grouper(messages, myPseudo).map((groupe) => ({
      genre: "groupe" as const,
      at: groupe.items[0].created_at,
      groupe,
    })),
    ...chain.map((entree) => ({
      genre: "proposition" as const,
      at: entree.created_at,
      entree,
    })),
  ].sort((a, b) => a.at.localeCompare(b.at));
  // « Vu » se pose sous MON dernier message lu — il y reste même si
  // l'autre partie a répondu depuis.
  const dernierLuId = [...messages]
    .reverse()
    .find((m) => m.sender_pseudo === myPseudo && m.read_at !== null)?.id;

  return (
    <section
      aria-label={`Conversation avec ${otherPseudo}`}
      className="flex flex-col overflow-hidden rounded-[28px] bg-sable shadow-sm"
    >
      <header className="flex items-center gap-3 border-b border-encre/10 px-4 py-3">
        <AvatarLetter pseudo={otherPseudo} size="sm" />
        <h2 className="font-display text-base">
          <Link
            href={`/troqueur/${encodeURIComponent(otherPseudo)}`}
            className="text-encre hover:underline"
          >
            {otherPseudo}
          </Link>
        </h2>
        <Link
          href={`/troqueur/${encodeURIComponent(otherPseudo)}`}
          aria-label={`Voir le profil de ${otherPseudo}`}
          className="ml-auto flex size-8 items-center justify-center rounded-full text-neutre-700 transition-colors hover:bg-encre/7"
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.25" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
            <circle cx="12" cy="12" r="10" />
            <path d="M12 16v-4" />
            <path d="M12 8h.01" />
          </svg>
        </Link>
      </header>

      <div className="flex max-h-[52vh] min-h-40 flex-col gap-3 overflow-y-auto bg-creme/40 px-4 py-4">
        {messages.length === 0 ? (
          <p className="py-6 text-center text-sm text-neutre-700">
            Brise la glace — un petit mot met toutes les chances de ton côté.
          </p>
        ) : null}

        {frise.map((element) => {
          if (element.genre === "proposition") {
            return (
              <PropositionCard
                key={element.entree.id}
                entree={element.entree}
                otherPseudo={otherPseudo}
                courante={element.entree.id === proposalId}
                actions={element.entree.id === proposalId ? actions : null}
              />
            );
          }
          const groupe = element.groupe;
          const premier = groupe.items[0];
          const vu = groupe.items.some((m) => m.id === dernierLuId);
          return (
            <div key={premier.id} className="flex flex-col gap-1">
              <p className="text-center text-[11px] text-neutre-700">
                {timeAgo(premier.created_at)}
              </p>
              <div className={`flex items-end gap-2 ${groupe.mine ? "flex-row-reverse" : ""}`}>
                {!groupe.mine ? (
                  <span className="shrink-0">
                    <AvatarLetter pseudo={otherPseudo} size="sm" />
                  </span>
                ) : null}
                <div
                  className={`flex min-w-0 flex-col gap-1 ${
                    groupe.mine ? "items-end" : "items-start"
                  }`}
                >
                  {groupe.items.map((message) => (
                    <div
                      key={message.id}
                      className={`max-w-[min(28rem,80%)] rounded-3xl px-4 py-2.5 text-sm shadow-sm ${
                        groupe.mine
                          ? "rounded-br-lg bg-[#c67139] text-creme"
                          : "rounded-bl-lg bg-creme text-encre"
                      }`}
                    >
                      {message.photo_url ? (
                        // eslint-disable-next-line @next/next/no-img-element
                        <img
                          src={message.photo_url}
                          alt="Photo jointe"
                          className="mb-1 max-h-64 rounded-2xl object-contain"
                        />
                      ) : null}
                      {message.body ? (
                        <p className="whitespace-pre-line break-words">{message.body}</p>
                      ) : null}
                      {message.redacted ? (
                        <p
                          className={`mt-1 text-[10px] ${
                            groupe.mine ? "text-creme/80" : "text-neutre-700"
                          }`}
                        >
                          Coordonnées masquées
                        </p>
                      ) : null}
                    </div>
                  ))}
                </div>
              </div>
              {vu ? <p className="text-right text-[11px] text-neutre-700">Vu</p> : null}
            </div>
          );
        })}

        <div ref={bottom} aria-hidden />
      </div>

      {redactedNotice ? (
        <p className="border-t border-encre/10 bg-terracotta-100/70 px-4 py-2.5 text-xs text-terracotta-800">
          On a masqué des coordonnées dans ton message. Garder les échanges ici te protège :
          c&apos;est ta seule preuve en cas de souci, et c&apos;est ce qui fait vivre Lebontroc.
          Elles seront partagées automatiquement une fois le troc accepté.
        </p>
      ) : null}
      {error ? (
        <p className="border-t border-encre/10 bg-terracotta-100 px-4 py-2.5 text-sm text-terracotta-800">
          {error}
        </p>
      ) : null}

      <div className="border-t border-encre/10 p-3">
        {closed ? (
          <p className="px-2 py-1 text-center text-sm text-neutre-700">
            Cette proposition est close — la conversation est en lecture seule.
          </p>
        ) : (
          <form
            onSubmit={(e) => {
              e.preventDefault();
              void send();
            }}
            className="flex items-center gap-1 rounded-full border border-neutre-300 bg-creme py-1.5 pl-2 pr-1.5 transition-colors focus-within:border-terracotta-500"
          >
            <input
              ref={fileInput}
              type="file"
              accept="image/*"
              className="hidden"
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (file) void attachPhoto(file);
                e.target.value = "";
              }}
            />
            <button
              type="button"
              aria-label="Joindre une photo"
              onClick={() => fileInput.current?.click()}
              className="flex size-9 shrink-0 cursor-pointer items-center justify-center rounded-full text-neutre-700 transition-colors hover:bg-encre/7"
            >
              <svg width="19" height="19" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.25" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
                <path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z" />
                <circle cx="12" cy="13" r="3" />
              </svg>
            </button>
            <input
              ref={champ}
              aria-label="Ton message"
              placeholder="Envoyer un message"
              value={draft}
              onChange={(e) => setDraft(e.target.value.slice(0, 2000))}
              className="min-w-0 flex-1 bg-transparent px-1 text-sm outline-none placeholder:text-neutre-700"
            />
            <button
              type="submit"
              aria-label="Envoyer"
              disabled={sending || draft.trim() === ""}
              className="flex size-9 shrink-0 cursor-pointer items-center justify-center rounded-full bg-[#c67139] text-creme transition-colors hover:bg-terracotta-600 disabled:cursor-not-allowed disabled:opacity-40"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
                <path d="M5 12h14" />
                <path d="m12 5 7 7-7 7" />
              </svg>
            </button>
          </form>
        )}
        {!closed && draft.trim() === "" ? (
          <div className="mt-2">
            <MessageSuggestions
              suggestions={suggestionsConversation({
                ...contexte,
                aucunMessage: messages.length === 0,
              })}
              onPick={(suggestion) => {
                setDraft(suggestion);
                champ.current?.focus();
              }}
            />
          </div>
        ) : null}
      </div>
    </section>
  );
}

const LIBELLE_STATUT: Record<string, string> = {
  envoyee: "En attente de réponse",
  vue: "En attente de réponse",
  acceptee: "Acceptée ✓",
  refusee: "Refusée",
  contre_proposee: "Remplacée par une contre-proposition",
  expiree: "Expirée",
  caduque: "Caduque",
};

/**
 * Une offre dans le fil. Lecture toujours identique quel que soit
 * l'auteur : à GAUCHE ce que l'autre propose, à DROITE ce que je donne en
 * échange — on ne se demande jamais « dans quel sens lire ».
 */
function PropositionCard({
  entree,
  otherPseudo,
  courante,
  actions,
}: {
  entree: ProposalChainEntry;
  otherPseudo: string;
  courante: boolean;
  actions?: React.ReactNode;
}) {
  // L'auteur « offre » ce qu'il donne : si c'est moi, sa colonne devient
  // la mienne (droite) et ce qu'il demande vient de l'autre (gauche).
  const sien = entree.is_mine ? entree.requested : entree.offered;
  const mien = entree.is_mine ? entree.offered : entree.requested;
  const soulteCents = entree.cash_cents;
  // La soulte suit celui qui la verse : le proposant est l'auteur.
  const soulteDeLAuteur = entree.cash_direction === "du_proposant";
  const soulteSienne = soulteDeLAuteur !== entree.is_mine ? soulteCents : 0;
  const soulteMienne = soulteDeLAuteur === entree.is_mine ? soulteCents : 0;

  const total = (items: ProposalChainEntry["offered"]) =>
    items.reduce((somme, item) => somme + item.value_cents, 0);
  // Ce que je reçois = ses objets + la soulte qu'il verse ; ce que je
  // donne = mes objets + la soulte que je verse.
  const recu = total(sien) + soulteSienne;
  const donne = total(mien) + soulteMienne;
  const ecart = recu - donne;

  return (
    <div className="flex flex-col gap-1">
      <p className="text-center text-[11px] text-neutre-700">{timeAgo(entree.created_at)}</p>
      <div
        className={`rounded-3xl border p-3 shadow-sm ${
          courante ? "border-terracotta-500 bg-creme" : "border-neutre-300 bg-creme/60 opacity-80"
        }`}
      >
        <p className="mb-2 font-display text-sm">
          🔁 {entree.is_mine ? "Ta proposition" : `Proposition de ${entree.author_pseudo}`}
        </p>
        <div className="flex items-stretch gap-2">
          <Colonne
            titre={`${otherPseudo} propose`}
            items={sien}
            soulte={soulteSienne}
            ton="recoit"
          />
          <span aria-hidden className="self-center text-lg text-neutre-700">
            ↔
          </span>
          <Colonne titre="Tu donnes" items={mien} soulte={soulteMienne} ton="donne" align="right" />
        </div>

        <div className="mt-2 flex items-center justify-between gap-2 border-t border-encre/10 pt-2 text-[11px]">
          <span className={ECHANGE.recoit.texte}>Tu reçois ~{euros(recu)}</span>
          <span className="font-semibold text-neutre-700">
            {ecart === 0
              ? "Échange équilibré"
              : ecart > 0
                ? `À ton avantage : +${euros(ecart)}`
                : `À ton désavantage : −${euros(-ecart)}`}
          </span>
          <span className={ECHANGE.donne.texte}>Tu donnes ~{euros(donne)}</span>
        </div>

        {entree.message ? (
          <p className="mt-2 whitespace-pre-line text-sm text-neutre-700">« {entree.message} »</p>
        ) : null}
        <p className="mt-2 text-[11px] text-neutre-700">
          {LIBELLE_STATUT[entree.status] ?? entree.status}
        </p>
        {actions ? <div className="mt-2 border-t border-encre/10 pt-2">{actions}</div> : null}
      </div>
    </div>
  );
}

function Colonne({
  titre,
  items,
  soulte,
  ton,
  align = "left",
}: {
  titre: string;
  items: ProposalChainEntry["offered"];
  soulte: number;
  ton: "donne" | "recoit";
  align?: "left" | "right";
}) {
  const style = ECHANGE[ton];
  return (
    <div
      className={`flex min-w-0 flex-1 flex-col gap-1 rounded-2xl p-2 ${style.fond} ${
        align === "right" ? "items-end text-right" : ""
      }`}
    >
      <span className={`text-[11px] font-semibold uppercase tracking-wide ${style.texte}`}>
        {titre}
      </span>
      <ul className="flex w-full flex-col gap-1">
        {items.map((item) => (
          <li
            key={item.item_id}
            className={`flex items-center gap-1.5 text-sm ${
              align === "right" ? "flex-row-reverse text-right" : ""
            }`}
          >
            {item.photo_url ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img src={item.photo_url} alt="" className="size-8 shrink-0 rounded-lg object-cover" />
            ) : null}
            <span className="truncate">{item.title}</span>
          </li>
        ))}
        {items.length === 0 ? <li className="text-sm text-neutre-700">—</li> : null}
      </ul>
      {soulte > 0 ? (
        <span className={`text-sm font-semibold ${style.texte}`}>+ {euros(soulte)}</span>
      ) : null}
    </div>
  );
}
