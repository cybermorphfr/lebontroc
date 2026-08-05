import Link from "next/link";

import { ConsentBanner } from "@/components/ConsentBanner";
import { Header } from "@/components/Header";
import { SessionKeeper } from "@/components/SessionKeeper";
import { VerifyEmailBanner } from "@/components/VerifyEmailBanner";
import { getCurrentUser } from "@/lib/server-api";

export default async function MainLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  const user = await getCurrentUser();
  return (
    <>
      <SessionKeeper loggedOut={user === null} />
      <Header user={user} />
      {user && !user.email_verified ? <VerifyEmailBanner /> : null}
      {children}
      <footer className="mx-auto flex w-full max-w-4xl flex-wrap items-center gap-x-4 gap-y-1 px-6 py-8 text-xs text-neutre-700">
        <span>© Lebontroc — le troc, près de chez toi</span>
        <Link href="/cgu" className="underline">CGU</Link>
        <Link href="/confidentialite" className="underline">Confidentialité</Link>
        <Link href="/mentions-legales" className="underline">Mentions légales</Link>
      </footer>
      <ConsentBanner />
    </>
  );
}
