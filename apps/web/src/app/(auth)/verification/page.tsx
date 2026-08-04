import type { Metadata } from "next";
import Link from "next/link";

import { ResendVerification } from "@/components/ResendVerification";
import { getCurrentUser } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Vérifie ta boîte mail — Lebontroc",
};

export const dynamic = "force-dynamic";

function IconCircle({ children, tone }: { children: React.ReactNode; tone: "sauge" | "terracotta" }) {
  return (
    <div
      className={`flex size-14 items-center justify-center rounded-full ${
        tone === "sauge" ? "bg-sauge-100 text-sauge-700" : "bg-terracotta-100 text-terracotta-700"
      }`}
      aria-hidden
    >
      {children}
    </div>
  );
}

function MailCheckIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M22 13V6a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2v12c0 1.1.9 2 2 2h8" />
      <path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7" />
      <path d="m16 19 2 2 4-4" />
    </svg>
  );
}

function ShieldCheckIcon() {
  return (
    <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z" />
      <path d="m9 12 2 2 4-4" />
    </svg>
  );
}

export default async function VerificationPage({
  searchParams,
}: {
  searchParams: Promise<{ statut?: string }>;
}) {
  const { statut } = await searchParams;
  const user = await getCurrentUser();

  if (statut === "ok") {
    return (
      <Carte>
        <IconCircle tone="sauge">
          <ShieldCheckIcon />
        </IconCircle>
        <h1 className="font-display text-3xl">C&apos;est tout bon !</h1>
        <p className="text-neutre-700">
          Ton e-mail est vérifié. Ton compte est prêt — bienvenue chez les troqueurs.
        </p>
        <Link
          href="/"
          className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-7 py-3 font-display text-base text-creme transition-colors hover:bg-terracotta-600"
        >
          C&apos;est parti
        </Link>
      </Carte>
    );
  }

  if (statut === "expire" || statut === "invalide") {
    const expire = statut === "expire";
    return (
      <Carte>
        <IconCircle tone="terracotta">
          <MailCheckIcon />
        </IconCircle>
        <h1 className="font-display text-3xl">
          {expire ? "Ce lien a expiré" : "Ce lien ne fonctionne pas"}
        </h1>
        <p className="text-neutre-700">
          {expire
            ? "Pas de panique : les liens ne durent que 24 heures, pour ta sécurité. On t'en renvoie un ?"
            : "Il a peut-être déjà servi, ou il est incomplet. Demande un nouveau lien depuis ton compte."}
        </p>
        {user ? (
          <ResendVerification asButton />
        ) : (
          <Link
            href="/connexion"
            className="inline-flex items-center justify-center rounded-full bg-[#c67139] px-7 py-3 font-display text-base text-creme transition-colors hover:bg-terracotta-600"
          >
            Recevoir un nouveau lien
          </Link>
        )}
      </Carte>
    );
  }

  return (
    <Carte>
      <IconCircle tone="sauge">
        <MailCheckIcon />
      </IconCircle>
      <h1 className="font-display text-3xl">Vérifie ta boîte mail</h1>
      <p className="text-neutre-700">
        On vient d&apos;envoyer un lien à{" "}
        <strong className="text-encre">{user?.email ?? "ton adresse"}</strong>. Clique dessus pour
        activer ton compte — pense à vérifier tes spams.
      </p>
      <ResendVerification asButton />
      <Link href="/" className="text-sm text-neutre-700 underline">
        Continuer vers Lebontroc
      </Link>
    </Carte>
  );
}

function Carte({ children }: { children: React.ReactNode }) {
  return (
    <section className="flex flex-col items-start gap-4 rounded-[32px] bg-sable p-6 shadow-sm">
      {children}
    </section>
  );
}
