import Link from "next/link";

import { adminFetch } from "./adminFetch";

// Le panneau d'administration adopte le design system du site : mêmes
// fonds, mêmes arrondis, même typographie — l'admin est une pièce de la
// maison, pas un sous-sol.

const ONGLETS = [
  ["/admin", "Tableau de bord"],
  ["/admin/annonces", "Annonces"],
  ["/admin/litiges", "Litiges"],
  ["/admin/signalements", "Signalements"],
  ["/admin/equipe", "Équipe"],
  ["/admin/liens", "E-mails"],
  ["/admin/audit", "Journal"],
  ["/admin/securite", "Sécurité"],
] as const;

export default async function AdminLayout({ children }: { children: React.ReactNode }) {
  // Un simple sondage : la 2FA de la session est-elle vérifiée ?
  const totp = await adminFetch<{ enabled: boolean; session_verified: boolean }>("/me/totp");

  return (
    <div className="min-h-screen bg-creme text-encre">
      <header className="mx-auto flex w-full max-w-4xl flex-wrap items-center gap-4 px-6 py-5">
        <Link href="/admin" className="font-display text-2xl">
          Lebontroc <span className="text-terracotta-700">Admin</span>
        </Link>
        <nav className="flex flex-wrap gap-1.5 text-sm">
          {ONGLETS.map(([href, label]) => (
            <Link
              key={href}
              href={href}
              className="rounded-full px-3.5 py-1.5 font-semibold text-encre transition-colors hover:bg-sable"
            >
              {label}
            </Link>
          ))}
        </nav>
        <Link href="/" className="ml-auto text-xs text-neutre-700 underline">
          ← Retour au site
        </Link>
      </header>

      {totp && totp.enabled === false ? (
        <p className="mx-auto mb-4 w-full max-w-4xl px-6">
          <span className="block rounded-3xl bg-terracotta-100 px-4 py-2.5 text-sm text-terracotta-800">
            🔐 Ta double authentification n&apos;est pas activée — c&apos;est la vraie serrure de
            cet espace.{" "}
            <Link href="/admin/securite" className="font-semibold underline">
              L&apos;activer maintenant
            </Link>
          </span>
        </p>
      ) : null}

      <div className="mx-auto w-full max-w-4xl px-6 pb-16">{children}</div>
    </div>
  );
}
