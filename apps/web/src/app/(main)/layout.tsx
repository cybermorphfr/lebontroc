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
    </>
  );
}
