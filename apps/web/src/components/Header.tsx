import Link from "next/link";
import type { UserResponse } from "@lebontroc/api-client";

import { NotificationBell } from "./NotificationBell";
import { TrocsLink } from "./TrocsLink";

/** En-tête minimal du Lot 0 : wordmark + état de connexion. */
export function Header({ user }: { user: UserResponse | null }) {
  return (
    <header className="flex items-center justify-between px-6 py-4 sm:px-12 lg:px-24">
      <Link href="/" className="font-display text-2xl text-encre">
        Lebontroc
      </Link>
      {user ? (
        <nav className="flex items-center gap-2">
          <SearchLink />
          <TrocsLink />
          <Link
            href="/favoris"
            aria-label="Mes favoris"
            className="flex size-9 items-center justify-center rounded-full text-encre transition-colors hover:bg-encre/7"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
              <path d="M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z" />
            </svg>
          </Link>
          <NotificationBell />
          <Link
            href="/publier"
            className="inline-flex items-center gap-1.5 rounded-full bg-[#c67139] px-4 py-2 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
              <circle cx="12" cy="12" r="10" />
              <path d="M8 12h8" />
              <path d="M12 8v8" />
            </svg>
            Publier
          </Link>
          <Link
            href="/profil"
            className="inline-flex items-center gap-2 rounded-full py-1 pl-1 pr-4 transition-colors hover:bg-encre/7"
          >
            <span
              aria-hidden
              className="flex size-8 items-center justify-center rounded-full bg-terracotta-100 font-display text-sm text-terracotta-800"
            >
              {user.pseudo.charAt(0).toUpperCase()}
            </span>
            <span className="text-sm font-semibold">{user.pseudo}</span>
          </Link>
        </nav>
      ) : (
        <nav className="flex items-center gap-2">
          <SearchLink />
          <Link
            href="/connexion"
            className="rounded-full px-3 py-2 font-display text-sm text-terracotta-700 transition-colors hover:bg-terracotta-500/10"
          >
            Me connecter
          </Link>
          <Link
            href="/inscription"
            className="rounded-full bg-[#c67139] px-4 py-2 font-display text-sm text-creme transition-colors hover:bg-terracotta-600"
          >
            M&apos;inscrire
          </Link>
        </nav>
      )}
    </header>
  );
}

function SearchLink() {
  return (
    <Link
      href="/recherche"
      aria-label="Rechercher"
      className="flex size-9 items-center justify-center rounded-full text-encre transition-colors hover:bg-encre/7"
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
        <circle cx="11" cy="11" r="8" />
        <path d="m21 21-4.3-4.3" />
      </svg>
    </Link>
  );
}
