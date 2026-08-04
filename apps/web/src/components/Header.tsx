import Link from "next/link";
import type { UserResponse } from "@lebontroc/api-client";

/** En-tête minimal du Lot 0 : wordmark + état de connexion. */
export function Header({ user }: { user: UserResponse | null }) {
  return (
    <header className="flex items-center justify-between px-6 py-4 sm:px-12 lg:px-24">
      <Link href="/" className="font-display text-2xl text-encre">
        Lebontroc
      </Link>
      {user ? (
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
      ) : (
        <nav className="flex items-center gap-2">
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
