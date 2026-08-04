import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Liens de vérification — Admin Lebontroc",
};

export const dynamic = "force-dynamic";

// Panneau d'administration de la bêta fermée : liste les e-mails capturés
// par Mailpit et en extrait les liens de vérification. Protégé par basic
// auth au niveau Traefik (même mot de passe que /mailpit).

type MailpitMessage = {
  ID: string;
  To: Array<{ Address: string }>;
  Subject: string;
  Created: string;
};

type LienVerification = {
  email: string;
  date: string;
  lien: string | null;
};

async function fetchLiens(): Promise<LienVerification[] | null> {
  const base = process.env.MAILPIT_URL ?? "http://localhost:8025";
  try {
    const response = await fetch(`${base}/api/v1/messages?limit=30`, { cache: "no-store" });
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
          const body = (await detail.json()) as { Text?: string };
          lien =
            body.Text?.match(/https?:\/\/[^\s]+verify-email\?token=[A-Za-z0-9_-]+/)?.[0] ?? null;
        } catch {
          // message illisible : on liste quand même l'entrée
        }
        return {
          email: message.To[0]?.Address ?? "?",
          date: new Date(message.Created).toLocaleString("fr-FR", {
            dateStyle: "short",
            timeStyle: "short",
          }),
          lien,
        };
      }),
    );
  } catch {
    return null;
  }
}

export default async function LiensAdminPage() {
  const liens = await fetchLiens();

  return (
    <main className="mx-auto flex w-full max-w-2xl flex-col gap-4 px-6 py-10">
      <header className="flex flex-col gap-1">
        <p className="font-display text-2xl">Lebontroc — admin</p>
        <h1 className="font-display text-xl">Liens de vérification</h1>
        <p className="text-sm text-neutre-700">
          Les e-mails de la bêta fermée sont capturés ici au lieu d&apos;être envoyés. Clique sur
          « Vérifier » pour activer le compte correspondant, ou consulte la boîte complète sur{" "}
          <a href="/mailpit" className="underline">
            /mailpit
          </a>
          .
        </p>
      </header>

      {liens === null ? (
        <p className="rounded-full bg-terracotta-100 px-4 py-2 text-sm text-terracotta-800">
          Mailpit est injoignable pour le moment.
        </p>
      ) : liens.length === 0 ? (
        <p className="text-sm text-neutre-700">Aucun e-mail capturé pour l&apos;instant.</p>
      ) : (
        <ul className="flex flex-col gap-2">
          {liens.map((entry, index) => (
            <li
              key={index}
              className="flex flex-wrap items-center justify-between gap-2 rounded-3xl bg-sable px-4 py-3"
            >
              <div className="flex min-w-0 flex-col">
                <span className="truncate text-sm font-semibold">{entry.email}</span>
                <span className="text-xs text-neutre-700">{entry.date}</span>
              </div>
              {entry.lien ? (
                <a
                  href={entry.lien}
                  className="rounded-full bg-[#c67139] px-4 py-1.5 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
                >
                  Vérifier
                </a>
              ) : (
                <span className="text-xs text-neutre-500">Pas de lien dans ce message</span>
              )}
            </li>
          ))}
        </ul>
      )}
    </main>
  );
}
