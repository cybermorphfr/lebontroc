import type { Metadata } from "next";
import { Caprasimo, Figtree } from "next/font/google";

import "./globals.css";

const caprasimo = Caprasimo({
  weight: "400",
  subsets: ["latin"],
  variable: "--font-caprasimo",
});

const figtree = Figtree({
  subsets: ["latin"],
  variable: "--font-figtree",
});

export const metadata: Metadata = {
  title: "Lebontroc — échange tes objets, sans argent",
  description:
    "Lebontroc, la plateforme de troc entre particuliers : propose tes objets, découvre ceux des autres, échange « ça contre ça ».",
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="fr" className={`${caprasimo.variable} ${figtree.variable}`}>
      <body className="font-sans antialiased">{children}</body>
    </html>
  );
}
