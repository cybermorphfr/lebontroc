import Link from "next/link";

export default function AuthLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <div className="flex min-h-screen flex-col items-center px-6 py-10">
      <Link href="/" className="mb-8 font-display text-3xl text-encre">
        Lebontroc
      </Link>
      <div className="w-full max-w-md">{children}</div>
    </div>
  );
}
